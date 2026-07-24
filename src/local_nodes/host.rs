use std::{fs, path::Path};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::{
    modules::logos_core::{
        ModuleCall, ModuleCallTerminated, ModuleTransportClosed, ModuleTransportKind,
        SharedModuleTransport, dispatch_module_call,
    },
    source_routing::{ManagedModuleCallSpec, ManagedNodeAction},
    support::{confirmation::ConfirmationPolicy, state_store::config_dir, time::now_millis},
};

use super::{
    action_engine::LocalNodeStore,
    action_workspace::write_devnet_manifest,
    adapters::{adapter_for, managed_action},
    lifecycle::acquire_state_lock,
    model::{
        LocalNodeActionRequest, LocalNodeConfigRecord, LocalNodeOperationReport, LocalNodeReport,
        LocalNodeStatus, LocalNodeSummary, LocalNodeTools, LocalNodesState, NodeAction, NodeKind,
        NodeLifecycleState, ToolStatus,
    },
    presentation,
    runtime::LogoscoreRuntimeStatus,
    workflow::{LocalNodeWorkflow, normalized_profile},
};

const HOST_NODE_KINDS: [NodeKind; 3] = [NodeKind::Bedrock, NodeKind::Storage, NodeKind::Messaging];

#[derive(Debug)]
struct HostNodeObservation {
    kind: NodeKind,
    module_available: bool,
    contract_error: Option<String>,
    liveness: Option<bool>,
    liveness_error: Option<String>,
}

impl HostNodeObservation {
    fn unavailable(kind: NodeKind, error: impl std::fmt::Display) -> Self {
        Self {
            kind,
            module_available: false,
            contract_error: Some(error.to_string()),
            liveness: None,
            liveness_error: None,
        }
    }

    fn contract_ready(&self) -> bool {
        self.module_available && self.contract_error.is_none()
    }
}

#[derive(Debug)]
struct PreparedHostAction {
    kind: NodeKind,
    action: NodeAction,
    module: &'static str,
    call: ManagedModuleCallSpec,
    args: Vec<Value>,
}

pub(super) async fn status(
    profile: &str,
    module_transport: &SharedModuleTransport,
) -> Result<LocalNodeReport> {
    ensure_host_transport(module_transport)?;
    let store = LocalNodeStore::for_config_dir(config_dir()?);
    status_with_store(profile, module_transport, &store).await
}

pub(super) async fn action(
    profile: &str,
    request: LocalNodeActionRequest,
    confirmation: Option<&str>,
    module_transport: &SharedModuleTransport,
) -> Result<LocalNodeReport> {
    ensure_host_transport(module_transport)?;
    ConfirmationPolicy::LocalNodeAction.require(confirmation)?;
    let store = LocalNodeStore::for_config_dir(config_dir()?);
    action_with_store(profile, request, module_transport, &store).await
}

async fn status_with_store(
    profile: &str,
    module_transport: &SharedModuleTransport,
    store: &LocalNodeStore,
) -> Result<LocalNodeReport> {
    {
        let _state_lock = acquire_state_lock()?;
        let _state = store.load()?;
    }

    let (bedrock, storage, messaging) = tokio::join!(
        observe_node(NodeKind::Bedrock, module_transport),
        observe_node(NodeKind::Storage, module_transport),
        observe_node(NodeKind::Messaging, module_transport),
    );
    let observations = [bedrock, storage, messaging];

    let _state_lock = acquire_state_lock()?;
    let mut state = store.load()?;
    reconcile_observations(&mut state, profile, &observations, store)?;
    Ok(project_report(profile, &state, &observations))
}

async fn action_with_store(
    profile: &str,
    request: LocalNodeActionRequest,
    module_transport: &SharedModuleTransport,
    store: &LocalNodeStore,
) -> Result<LocalNodeReport> {
    let plan = {
        let _state_lock = acquire_state_lock()?;
        let state = store.load()?;
        prepare_action(profile, &state, &request)?
    };

    let execution = match execute_host_action(&plan, module_transport).await {
        Err(error) if is_transport_interruption(&error) => return Err(error),
        result => result,
    };

    {
        let _state_lock = acquire_state_lock()?;
        let mut state = store.load()?;
        record_action_result(&mut state, profile, &request, &plan, execution, store)?;
    }

    status_with_store(profile, module_transport, store).await
}

fn ensure_host_transport(module_transport: &SharedModuleTransport) -> Result<()> {
    if module_transport.kind() != ModuleTransportKind::Module {
        bail!(
            "Basecamp Local Nodes requires the host module transport; active transport is `{}`",
            module_transport.kind().as_str()
        );
    }
    Ok(())
}

async fn observe_node(
    kind: NodeKind,
    module_transport: &SharedModuleTransport,
) -> HostNodeObservation {
    let Some(contract) = adapter_for(kind).managed_contract() else {
        return HostNodeObservation::unavailable(kind, "node has no managed module contract");
    };
    let module = contract.module_id();
    let metadata = match module_transport.module_info(module.to_owned()).await {
        Ok(metadata) => metadata,
        Err(error) => return HostNodeObservation::unavailable(kind, error),
    };
    let contract_error = validate_lifecycle_contract(kind, &metadata)
        .err()
        .map(|error| error.to_string());
    if contract_error.is_some() {
        return HostNodeObservation {
            kind,
            module_available: true,
            contract_error,
            liveness: None,
            liveness_error: None,
        };
    }

    let Some((method, signature, args)) = liveness_call(kind) else {
        return HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            liveness: None,
            liveness_error: None,
        };
    };
    if let Err(error) = require_method(&metadata, module, method, signature) {
        return HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            liveness: None,
            liveness_error: Some(error.to_string()),
        };
    }
    let call = match ModuleCall::new(ModuleTransportKind::Module, module, method, args) {
        Ok(call) => call,
        Err(error) => {
            return HostNodeObservation {
                kind,
                module_available: true,
                contract_error: None,
                liveness: None,
                liveness_error: Some(error.to_string()),
            };
        }
    };
    match dispatch_module_call(module_transport.as_ref(), call).await {
        Ok(_) => HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            liveness: Some(true),
            liveness_error: None,
        },
        Err(error) => HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            liveness: Some(false),
            liveness_error: Some(error.to_string()),
        },
    }
}

fn validate_lifecycle_contract(kind: NodeKind, metadata: &Value) -> Result<()> {
    let adapter = adapter_for(kind);
    let contract = adapter
        .managed_contract()
        .context("node has no managed module contract")?;
    for action in [NodeAction::Initialize, NodeAction::Start, NodeAction::Stop] {
        let managed_action = managed_action(action).context("managed action is unavailable")?;
        let spec = contract
            .call_spec(managed_action, "")
            .with_context(|| format!("{} has no {action:?} module call", adapter.label()))?;
        require_method(metadata, contract.module_id(), spec.method, spec.signature)?;
    }
    if kind == NodeKind::Storage {
        let spec = contract
            .call_spec(ManagedNodeAction::Destroy, "")
            .context("Storage has no destroy module call")?;
        require_method(metadata, contract.module_id(), spec.method, spec.signature)?;
    }
    Ok(())
}

fn require_method(metadata: &Value, module: &str, method: &str, signature: &str) -> Result<()> {
    let methods = metadata
        .get("methods")
        .and_then(Value::as_array)
        .with_context(|| format!("Basecamp module `{module}` metadata has no method list"))?;
    if methods.iter().any(|candidate| {
        candidate.get("name").and_then(Value::as_str) == Some(method)
            && candidate.get("signature").and_then(Value::as_str) == Some(signature)
            && candidate.get("isInvokable").and_then(Value::as_bool) != Some(false)
    }) {
        return Ok(());
    }
    bail!("Basecamp module `{module}` does not expose `{signature}`")
}

fn liveness_call(kind: NodeKind) -> Option<(&'static str, &'static str, Vec<Value>)> {
    match kind {
        NodeKind::Bedrock => Some(("get_cryptarchia_info", "get_cryptarchia_info()", Vec::new())),
        NodeKind::Storage => Some(("space", "space()", Vec::new())),
        NodeKind::Messaging => Some((
            "getNodeInfo",
            "getNodeInfo(QString)",
            vec![json!("MyPeerId")],
        )),
        NodeKind::Sequencer | NodeKind::Indexer => None,
    }
}

fn prepare_action(
    profile: &str,
    state: &LocalNodesState,
    request: &LocalNodeActionRequest,
) -> Result<PreparedHostAction> {
    let workflow = LocalNodeWorkflow::for_state(profile, state);
    workflow.validate_request(request)?;
    let kind = request.node.context("node kind is required")?;
    if !HOST_NODE_KINDS.contains(&kind) {
        bail!(
            "{} is not hosted by the Basecamp Local Nodes surface; configure it from its Zone",
            adapter_for(kind).label()
        );
    }
    let action = request.action;
    let managed_action = managed_action(action).with_context(|| {
        format!(
            "{} is not a Basecamp module lifecycle action",
            action.as_str()
        )
    })?;
    let adapter = adapter_for(kind);
    let contract = adapter
        .managed_contract()
        .context("node has no managed module contract")?;
    let profile = normalized_profile(profile);
    let topology = state
        .active_topology(profile)
        .context("active local node topology is required")?;
    let config = topology
        .nodes
        .iter()
        .find(|node| node.kind == kind)
        .with_context(|| format!("{} config is not available", adapter.label()))?;
    validate_action_state(config, action)?;
    if action == NodeAction::Initialize && kind == NodeKind::Messaging {
        let action_path = initialization_config_path(config);
        let _preparation = super::messaging_identity::prepare_existing_config(
            Path::new(&topology.workspace),
            Path::new(action_path),
        )?;
    }
    let action_path = if action == NodeAction::Initialize {
        initialization_config_path(config)
    } else {
        &config.config_path
    };
    let call = contract
        .call_spec(managed_action, action_path)
        .with_context(|| format!("{} {} is not implemented", adapter.label(), action.as_str()))?;
    let args = native_args(&call.args)?;
    Ok(PreparedHostAction {
        kind,
        action,
        module: contract.module_id(),
        call,
        args,
    })
}

fn validate_action_state(config: &LocalNodeConfigRecord, action: NodeAction) -> Result<()> {
    if config.lifecycle_state.is_pending() {
        bail!("a Basecamp module lifecycle action is already pending confirmation");
    }
    match action {
        NodeAction::Initialize if config.installed => {
            bail!("module context is already initialized")
        }
        NodeAction::Start | NodeAction::Stop | NodeAction::Uninstall if !config.installed => {
            bail!("initialize the module node before {}", action.as_str())
        }
        NodeAction::Start
            if !matches!(
                config.lifecycle_state,
                NodeLifecycleState::Stopped
                    | NodeLifecycleState::Unknown
                    | NodeLifecycleState::Failed
            ) =>
        {
            bail!("module node must be stopped before start")
        }
        NodeAction::Stop
            if !matches!(
                config.lifecycle_state,
                NodeLifecycleState::Running
                    | NodeLifecycleState::Unknown
                    | NodeLifecycleState::Failed
            ) =>
        {
            bail!("module node is not running")
        }
        NodeAction::Uninstall
            if !matches!(
                config.lifecycle_state,
                NodeLifecycleState::Stopped
                    | NodeLifecycleState::Unknown
                    | NodeLifecycleState::Failed
            ) =>
        {
            bail!("stop the module node before removing its context")
        }
        NodeAction::Initialize | NodeAction::Start | NodeAction::Stop | NodeAction::Uninstall => {}
        _ => bail!(
            "{} is not supported by the Basecamp module host",
            action.as_str()
        ),
    }
    Ok(())
}

fn initialization_config_path(config: &LocalNodeConfigRecord) -> &str {
    config
        .initialization_config_path
        .as_deref()
        .unwrap_or(&config.config_path)
}

fn native_args(args: &[String]) -> Result<Vec<Value>> {
    args.iter()
        .map(|argument| {
            let value = if let Some(path) = argument.strip_prefix('@') {
                fs::read_to_string(path)
                    .with_context(|| format!("failed to read Basecamp module config `{path}`"))?
            } else {
                argument.clone()
            };
            Ok(Value::String(value))
        })
        .collect()
}

async fn execute_host_action(
    plan: &PreparedHostAction,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    let metadata = module_transport.module_info(plan.module.to_owned()).await?;
    require_method(
        &metadata,
        plan.module,
        plan.call.method,
        plan.call.signature,
    )?;
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        plan.module,
        plan.call.method,
        plan.args.clone(),
    )?;
    Ok(dispatch_module_call(module_transport.as_ref(), call)
        .await?
        .into_value())
}

fn is_transport_interruption(error: &anyhow::Error) -> bool {
    error.downcast_ref::<ModuleCallTerminated>().is_some()
        || error.downcast_ref::<ModuleTransportClosed>().is_some()
}

fn record_action_result(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
    plan: &PreparedHostAction,
    execution: Result<Value>,
    store: &LocalNodeStore,
) -> Result<()> {
    let timestamp = now_millis();
    let (status, detail, succeeded) = match execution {
        Ok(_) => (
            action_success_status(plan.action).to_owned(),
            format!(
                "Basecamp host accepted {}.{}",
                plan.module, plan.call.method
            ),
            true,
        ),
        Err(error) => ("failed".to_owned(), format!("{error:#}"), false),
    };
    if succeeded {
        apply_successful_action(state, profile, plan)?;
    }
    state.push_operation(LocalNodeOperationReport {
        id: format!("op-{timestamp}"),
        time: timestamp.to_string(),
        timestamp_millis: timestamp,
        action: request.action,
        node: request.node,
        network_id: request.network_id.clone(),
        status,
        detail,
        command: Some(format!(
            "Basecamp host call {}.{}",
            plan.module, plan.call.method
        )),
    });
    store.save(state)
}

fn action_success_status(action: NodeAction) -> &'static str {
    match action {
        NodeAction::Initialize => "initialized",
        NodeAction::Start => "starting",
        NodeAction::Stop => "stopping",
        NodeAction::Uninstall => "uninstalled",
        _ => "completed",
    }
}

fn apply_successful_action(
    state: &mut LocalNodesState,
    profile: &str,
    plan: &PreparedHostAction,
) -> Result<()> {
    let profile = normalized_profile(profile);
    let topology_id = state
        .active_topology(profile)
        .map(|topology| topology.id.clone())
        .context("active local node topology is required")?;
    let record = state
        .active_topology_mut(profile)
        .context("active local node topology is required")?;
    let config = record
        .nodes
        .iter_mut()
        .find(|node| node.kind == plan.kind)
        .with_context(|| format!("{} config is not available", adapter_for(plan.kind).label()))?;
    match plan.action {
        NodeAction::Initialize => {
            config.installed = true;
            config.package_path = Some(plan.module.to_owned());
            config.lifecycle_state = NodeLifecycleState::Stopped;
            config.pending_lifecycle_action = None;
        }
        NodeAction::Start => {
            config.installed = true;
            config.lifecycle_state = NodeLifecycleState::Starting;
            config.pending_lifecycle_action = Some(NodeAction::Start);
        }
        NodeAction::Stop => {
            config.lifecycle_state = NodeLifecycleState::Stopping;
            config.pending_lifecycle_action = Some(NodeAction::Stop);
        }
        NodeAction::Uninstall => clear_module_context(config),
        _ => {}
    }
    record.updated_at = now_millis();
    write_devnet_manifest(record)?;
    match plan.action {
        NodeAction::Initialize => {
            state
                .module_context_topology_by_kind
                .insert(plan.kind, topology_id);
        }
        NodeAction::Uninstall => state.clear_module_context_topology(plan.kind),
        _ => {}
    }
    Ok(())
}

fn reconcile_observations(
    state: &mut LocalNodesState,
    profile: &str,
    observations: &[HostNodeObservation],
    store: &LocalNodeStore,
) -> Result<()> {
    let profile = normalized_profile(profile);
    let topology_id = state
        .active_topology(profile)
        .map(|topology| topology.id.clone());
    let Some(record) = state.active_topology_mut(profile) else {
        return Ok(());
    };
    let mut changed = false;
    for observation in observations {
        if !observation.contract_ready() {
            continue;
        }
        let Some(config) = record
            .nodes
            .iter_mut()
            .find(|node| node.kind == observation.kind)
        else {
            continue;
        };
        match observation.liveness {
            Some(true) if config.lifecycle_state == NodeLifecycleState::Stopping => {}
            Some(true) => {
                if !config.installed
                    || config.lifecycle_state != NodeLifecycleState::Running
                    || config.pending_lifecycle_action.is_some()
                {
                    config.installed = true;
                    config.package_path = Some(
                        adapter_for(observation.kind)
                            .managed_contract()
                            .map(|contract| contract.module_id())
                            .unwrap_or_default()
                            .to_owned(),
                    );
                    config.lifecycle_state = NodeLifecycleState::Running;
                    config.pending_lifecycle_action = None;
                    changed = true;
                }
            }
            Some(false) if config.lifecycle_state == NodeLifecycleState::Stopping => {
                config.lifecycle_state = NodeLifecycleState::Stopped;
                config.pending_lifecycle_action = None;
                changed = true;
            }
            Some(false) if config.lifecycle_state == NodeLifecycleState::Running => {
                config.lifecycle_state = NodeLifecycleState::Unknown;
                config.pending_lifecycle_action = None;
                changed = true;
            }
            Some(false) | None => {}
        }
    }
    if !changed {
        return Ok(());
    }
    record.updated_at = now_millis();
    write_devnet_manifest(record)?;
    if let Some(topology_id) = topology_id {
        for observation in observations
            .iter()
            .filter(|observation| observation.liveness == Some(true))
        {
            state
                .module_context_topology_by_kind
                .insert(observation.kind, topology_id.clone());
        }
    }
    store.save(state)
}

fn project_report(
    profile: &str,
    state: &LocalNodesState,
    observations: &[HostNodeObservation],
) -> LocalNodeReport {
    let profile = normalized_profile(profile);
    let active = state.active_topology(profile);
    let nodes = observations
        .iter()
        .filter_map(|observation| {
            active
                .and_then(|record| {
                    record
                        .nodes
                        .iter()
                        .find(|node| node.kind == observation.kind)
                })
                .map(|config| project_node(state, config, observation))
        })
        .collect::<Vec<_>>();
    let installed = nodes
        .iter()
        .filter(|node| node.install_state == "installed")
        .count();
    let running = nodes
        .iter()
        .filter(|node| node.run_state == "running")
        .count();
    let needs_configuration = nodes
        .iter()
        .filter(|node| node.install_state == "needs_configuration")
        .count();
    LocalNodeReport {
        profile: profile.to_owned(),
        mode: presentation::mode_for_profile(profile).to_owned(),
        available_network_actions: Vec::new(),
        available_runtime_actions: Vec::new(),
        primary_problem: None,
        active_devnet: active.map(|record| record.id.clone()),
        workspace_root: state.managed_workspace_root.clone(),
        summary: LocalNodeSummary {
            total: nodes.len(),
            installed,
            running,
            needs_configuration,
        },
        nodes,
        operations: state.operations.clone(),
        tools: basecamp_tools(),
        runtime: basecamp_runtime_status(),
    }
}

fn project_node(
    state: &LocalNodesState,
    config: &LocalNodeConfigRecord,
    observation: &HostNodeObservation,
) -> LocalNodeStatus {
    let compatible = observation.contract_ready();
    let install_state = if compatible && config.installed {
        "installed"
    } else {
        "needs_configuration"
    };
    LocalNodeStatus {
        kind: config.kind,
        key: config.kind.as_str().to_owned(),
        label: adapter_for(config.kind).label().to_owned(),
        install_state: install_state.to_owned(),
        run_state: config.lifecycle_state.as_str().to_owned(),
        ownership: if observation.module_available {
            "inspector_managed"
        } else {
            "external"
        }
        .to_owned(),
        endpoint: config.endpoint.clone(),
        data_dir: Some(config.data_dir.clone()),
        config_path: Some(config.config_path.clone()),
        package_path: config.package_path.clone(),
        package_version: config.package_version.clone(),
        managed_channel_id: None,
        indexer_state: None,
        indexer_head: None,
        indexer_error: None,
        process_id: None,
        last_action: state
            .operations
            .iter()
            .rev()
            .find(|operation| operation.node == Some(config.kind))
            .cloned(),
        available_actions: host_available_actions(config, observation),
        detail: host_node_detail(config, observation),
    }
}

fn host_available_actions(
    config: &LocalNodeConfigRecord,
    observation: &HostNodeObservation,
) -> Vec<NodeAction> {
    if !observation.contract_ready() || config.lifecycle_state.is_pending() {
        return Vec::new();
    }
    if !config.installed {
        return vec![NodeAction::Initialize];
    }
    match config.lifecycle_state {
        NodeLifecycleState::Stopped => {
            let mut actions = vec![NodeAction::Start];
            if config.kind == NodeKind::Storage {
                actions.push(NodeAction::Uninstall);
            }
            actions
        }
        NodeLifecycleState::Running | NodeLifecycleState::Unknown | NodeLifecycleState::Failed => {
            vec![NodeAction::Stop]
        }
        NodeLifecycleState::NotInitialized => vec![NodeAction::Initialize],
        NodeLifecycleState::Initializing
        | NodeLifecycleState::Starting
        | NodeLifecycleState::Stopping => Vec::new(),
    }
}

fn host_node_detail(config: &LocalNodeConfigRecord, observation: &HostNodeObservation) -> String {
    if !observation.module_available {
        return observation
            .contract_error
            .clone()
            .unwrap_or_else(|| "Basecamp dependency module is unavailable".to_owned());
    }
    if let Some(error) = observation.contract_error.as_deref() {
        return error.to_owned();
    }
    match config.lifecycle_state {
        NodeLifecycleState::NotInitialized => {
            "Basecamp module is loaded; initialize its node context".to_owned()
        }
        NodeLifecycleState::Initializing => "Basecamp module is initializing".to_owned(),
        NodeLifecycleState::Starting => "Basecamp module is starting".to_owned(),
        NodeLifecycleState::Running => "Basecamp module is running".to_owned(),
        NodeLifecycleState::Stopping => "Basecamp module is stopping".to_owned(),
        NodeLifecycleState::Stopped => "Basecamp module is stopped".to_owned(),
        NodeLifecycleState::Unknown | NodeLifecycleState::Failed => observation
            .liveness_error
            .clone()
            .unwrap_or_else(|| "Basecamp module liveness is not confirmed".to_owned()),
    }
}

fn basecamp_tools() -> LocalNodeTools {
    LocalNodeTools {
        logoscore: ToolStatus {
            available: true,
            command: "Basecamp host".to_owned(),
            path: None,
        },
        lgpd: ToolStatus {
            available: false,
            command: "lgpd".to_owned(),
            path: None,
        },
        lgpm: ToolStatus {
            available: false,
            command: "lgpm".to_owned(),
            path: None,
        },
    }
}

fn basecamp_runtime_status() -> LogoscoreRuntimeStatus {
    LogoscoreRuntimeStatus {
        ownership: "basecamp_host".to_owned(),
        run_state: "running".to_owned(),
        id: Some("basecamp".to_owned()),
        binary_path: None,
        config_dir: None,
        modules_dir: None,
        persistence_path: None,
        process_id: None,
        service_unit: None,
        detail: "Bedrock, Messaging, and Storage are owned by Basecamp".to_owned(),
    }
}

fn clear_module_context(config: &mut LocalNodeConfigRecord) {
    config.installed = false;
    config.package_path = None;
    config.package_version = None;
    config.package_root_hash = None;
    config.module_path = None;
    config.process_id = None;
    config.lifecycle_state = NodeLifecycleState::NotInitialized;
    config.pending_lifecycle_action = None;
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use anyhow::{Result, bail};

    use crate::modules::logos_core::{
        ModuleCallFuture, ModuleCallReply, ModuleDiagnosticFuture, ModuleTransport,
    };

    use super::*;

    #[derive(Debug, Clone)]
    struct RecordingHostTransport {
        calls: Arc<Mutex<Vec<ModuleCall>>>,
    }

    impl RecordingHostTransport {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn calls(&self) -> Result<Vec<ModuleCall>> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|_| anyhow::anyhow!("recording call lock is poisoned"))
        }
    }

    impl ModuleTransport for RecordingHostTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let result = self
                .calls
                .lock()
                .map_err(|_| anyhow::anyhow!("recording call lock is poisoned"))
                .map(|mut calls| {
                    calls.push(call.clone());
                    match (call.module(), call.method()) {
                        ("blockchain_module", "get_cryptarchia_info")
                        | ("storage_module", "space")
                        | ("delivery_module", "getNodeInfo") => {
                            bail!("node context is not running")
                        }
                        _ => Ok(ModuleCallReply::new(
                            ModuleTransportKind::Module,
                            json!(true),
                        )),
                    }
                });
            Box::pin(async move { result? })
        }

        fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
            Box::pin(async move { Ok(module_metadata(&module)) })
        }
    }

    fn module_metadata(module: &str) -> Value {
        let methods = match module {
            "blockchain_module" => vec![
                json!({"name":"generate_user_config","signature":"generate_user_config(QString)","isInvokable":true}),
                json!({"name":"start","signature":"start(QString,QString)","isInvokable":true}),
                json!({"name":"stop","signature":"stop()","isInvokable":true}),
                json!({"name":"get_cryptarchia_info","signature":"get_cryptarchia_info()","isInvokable":true}),
            ],
            "storage_module" => vec![
                json!({"name":"init","signature":"init(QString)","isInvokable":true}),
                json!({"name":"start","signature":"start()","isInvokable":true}),
                json!({"name":"stop","signature":"stop()","isInvokable":true}),
                json!({"name":"destroy","signature":"destroy()","isInvokable":true}),
                json!({"name":"space","signature":"space()","isInvokable":true}),
            ],
            "delivery_module" => vec![
                json!({"name":"createNode","signature":"createNode(QString)","isInvokable":true}),
                json!({"name":"start","signature":"start()","isInvokable":true}),
                json!({"name":"stop","signature":"stop()","isInvokable":true}),
                json!({"name":"getNodeInfo","signature":"getNodeInfo(QString)","isInvokable":true}),
            ],
            _ => Vec::new(),
        };
        json!({"name":module,"methods":methods,"events":[]})
    }

    fn initialize_request(kind: NodeKind) -> LocalNodeActionRequest {
        LocalNodeActionRequest {
            action: NodeAction::Initialize,
            node: Some(kind),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            allow_identity_rotation: false,
            label: None,
        }
    }

    #[tokio::test]
    async fn basecamp_initialize_dispatches_config_contents_through_host_module() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::new();
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = action_with_store(
            "default",
            initialize_request(NodeKind::Messaging),
            &transport,
            &store,
        )
        .await?;

        let create = transport_impl
            .calls()?
            .into_iter()
            .find(|call| call.module() == "delivery_module" && call.method() == "createNode")
            .context("Basecamp Messaging initialize did not call delivery_module.createNode")?;
        let config = create
            .args()
            .first()
            .and_then(Value::as_str)
            .context("Basecamp Messaging initialize did not pass config text")?;
        if config.starts_with('@') || serde_json::from_str::<Value>(config).is_err() {
            bail!("Basecamp Messaging initialize did not expand its config file");
        }
        let messaging = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging")?;
        if messaging.ownership != "inspector_managed"
            || messaging.install_state != "installed"
            || messaging.run_state != "stopped"
            || report.runtime.ownership != "basecamp_host"
            || !report.available_runtime_actions.is_empty()
        {
            bail!("Basecamp action returned standalone Local Nodes state: {report:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_status_lists_only_host_owned_dependency_modules() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport: SharedModuleTransport = Arc::new(RecordingHostTransport::new());

        let report = status_with_store("default", &transport, &store).await?;
        let kinds = report
            .nodes
            .iter()
            .map(|node| node.kind)
            .collect::<Vec<_>>();

        if kinds != HOST_NODE_KINDS
            || report
                .nodes
                .iter()
                .any(|node| node.ownership != "inspector_managed")
            || report.primary_problem.is_some()
        {
            bail!("Basecamp report exposed standalone-only nodes or runtime problems: {report:?}");
        }
        Ok(())
    }

    #[test]
    fn basecamp_stop_remains_pending_while_module_is_still_live() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let mut state = store.load()?;
        let messaging = state
            .active_topology_mut("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter_mut()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        messaging.installed = true;
        messaging.lifecycle_state = NodeLifecycleState::Stopping;
        messaging.pending_lifecycle_action = Some(NodeAction::Stop);
        store.save(&state)?;

        reconcile_observations(
            &mut state,
            "default",
            &[HostNodeObservation {
                kind: NodeKind::Messaging,
                module_available: true,
                contract_error: None,
                liveness: Some(true),
                liveness_error: None,
            }],
            &store,
        )?;

        let state = store.load()?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Stopping
            || messaging.pending_lifecycle_action != Some(NodeAction::Stop)
        {
            bail!("live stop probe cleared the pending stop: {messaging:?}");
        }
        Ok(())
    }
}
