use std::{
    fs,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    modules::logos_core::{
        BoxedModuleEventSubscription, ModuleCall, ModuleCallReply, ModuleCallTerminated,
        ModuleTransportClosed, ModuleTransportEvent, ModuleTransportKind, SharedModuleTransport,
        dispatch_module_call, normalize_module_call_value,
    },
    source_routing::{
        ManagedModuleCallSpec, ManagedNodeAction,
        storage::{
            StorageNodeLifecycleSnapshot, managed_lifecycle_status, storage_node_lifecycle_snapshot,
        },
    },
    support::{confirmation::ConfirmationPolicy, state_store::config_dir, time::now_millis},
};

use super::{
    action_engine::LocalNodeStore,
    action_workspace::write_devnet_manifest,
    adapters::{adapter_for, managed_action},
    lifecycle::acquire_state_lock,
    messaging_health::probe as probe_messaging_health,
    model::{
        LocalNodeActionRequest, LocalNodeConfigRecord, LocalNodeOperationReport, LocalNodeReport,
        LocalNodeStatus, LocalNodeSummary, LocalNodeTools, LocalNodesState, NodeAction, NodeKind,
        NodeLifecycleState, ToolStatus,
    },
    presentation,
    runtime::LogoscoreRuntimeStatus,
    workflow::normalized_profile,
};

const HOST_NODE_KINDS: [NodeKind; 3] = [NodeKind::Bedrock, NodeKind::Storage, NodeKind::Messaging];
const SERVICE_TRANSITION_TIMEOUT: Duration = Duration::from_secs(5);
const SERVICE_TRANSITION_POLL_INTERVAL: Duration = Duration::from_millis(100);
const HOST_LIFECYCLE_EVENT_TIMEOUT: Duration = Duration::from_secs(30);
const STORAGE_NODE_STATUS_METHOD: &str = "nodeStatus";
const STORAGE_NODE_STATUS_SIGNATURE: &str = "nodeStatus()";
const STORAGE_NODE_ACTION_METHOD: &str = "nodeAction";
const STORAGE_NODE_ACTION_SIGNATURE: &str = "nodeAction(QString)";
const STORAGE_NODE_CHANGED_EVENT: &str = "nodeChanged";
const STORAGE_NODE_CHANGED_SIGNATURE: &str = "nodeChanged(QString)";
static STORAGE_NODE_LIFECYCLE_OPERATION_SERIAL: AtomicU64 = AtomicU64::new(0);
const BEDROCK_NODE_STATUS_METHOD: &str = "nodeStatus";
const BEDROCK_NODE_STATUS_SIGNATURE: &str = "nodeStatus()";
const BEDROCK_NODE_ACTION_METHOD: &str = "nodeAction";
const BEDROCK_NODE_ACTION_SIGNATURE: &str = "nodeAction(QString)";
const BEDROCK_NODE_CHANGED_EVENT: &str = "nodeChanged";
const BEDROCK_NODE_CHANGED_SIGNATURE: &str = "nodeChanged(QString)";
static BEDROCK_NODE_LIFECYCLE_OPERATION_SERIAL: AtomicU64 = AtomicU64::new(0);
const MESSAGING_NODE_STATUS_METHOD: &str = "nodeStatus";
const MESSAGING_NODE_STATUS_SIGNATURE: &str = "nodeStatus()";
const MESSAGING_NODE_ACTION_METHOD: &str = "nodeAction";
const MESSAGING_NODE_ACTION_SIGNATURE: &str = "nodeAction(QString)";
const MESSAGING_NODE_CHANGED_EVENT: &str = "nodeChanged";
const MESSAGING_NODE_CHANGED_SIGNATURE: &str = "nodeChanged(QString)";
static MESSAGING_NODE_LIFECYCLE_OPERATION_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
struct HostNodeObservation {
    kind: NodeKind,
    module_available: bool,
    contract_error: Option<String>,
    context_initialized: Option<bool>,
    liveness: Option<bool>,
    liveness_error: Option<String>,
    managed_actions: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ServiceLiveness {
    observed: Option<bool>,
    detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ServiceProbe {
    Known(bool),
    Inconclusive(&'static str),
}

impl HostNodeObservation {
    fn unavailable(kind: NodeKind, error: impl std::fmt::Display) -> Self {
        Self {
            kind,
            module_available: false,
            contract_error: Some(error.to_string()),
            context_initialized: None,
            liveness: None,
            liveness_error: None,
            managed_actions: None,
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
    call: Option<ManagedModuleCallSpec>,
    args: Vec<Value>,
}

struct HostActionExecution {
    lifecycle_detail: Option<String>,
    dispatched_method: &'static str,
}

struct HostLifecycleSubscription {
    event: &'static str,
    subscription: BoxedModuleEventSubscription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedNodeLifecycleState {
    Uninitialized,
    Initializing,
    Stopped,
    Starting,
    Running,
    Stopping,
    Destroying,
}

impl ManagedNodeLifecycleState {
    fn parse(value: &str, label: &str) -> Result<Self> {
        match value {
            "uninitialized" => Ok(Self::Uninitialized),
            "initializing" => Ok(Self::Initializing),
            "stopped" => Ok(Self::Stopped),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "destroying" => Ok(Self::Destroying),
            state => bail!("{label} returned unknown lifecycle state `{state}`"),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Uninitialized => "uninitialized",
            Self::Initializing => "initializing",
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Destroying => "destroying",
        }
    }

    const fn context_initialized(self) -> Option<bool> {
        match self {
            Self::Uninitialized => Some(false),
            Self::Initializing => None,
            Self::Stopped | Self::Starting | Self::Running | Self::Stopping | Self::Destroying => {
                Some(true)
            }
        }
    }

    const fn liveness(self) -> Option<bool> {
        match self {
            Self::Uninitialized | Self::Stopped => Some(false),
            Self::Running => Some(true),
            Self::Initializing | Self::Starting | Self::Stopping | Self::Destroying => None,
        }
    }

    const fn expected_actions(self) -> &'static [&'static str] {
        match self {
            Self::Uninitialized => &["initialize"],
            Self::Stopped => &["start"],
            Self::Running => &["stop"],
            Self::Initializing | Self::Starting | Self::Stopping | Self::Destroying => &[],
        }
    }

    const fn allowed_actions(self) -> &'static [&'static str] {
        match self {
            Self::Uninitialized => &["initialize"],
            Self::Stopped => &["start", "destroy"],
            Self::Running => &["stop"],
            Self::Initializing | Self::Starting | Self::Stopping | Self::Destroying => &[],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ManagedNodeLifecycleSnapshot {
    instance_id: String,
    epoch: u64,
    sequence: u64,
    state: ManagedNodeLifecycleState,
    supported_actions: Vec<String>,
    last_error: Option<String>,
}

#[derive(Debug, Clone, Copy)]
struct ManagedNodeLifecycleAction {
    wire_name: &'static str,
    expected_transition: &'static str,
    expected_terminal: &'static str,
}

impl ManagedNodeLifecycleSnapshot {
    fn instance_id(&self) -> &str {
        &self.instance_id
    }

    const fn epoch(&self) -> u64 {
        self.epoch
    }

    const fn sequence(&self) -> u64 {
        self.sequence
    }

    const fn state(&self) -> ManagedNodeLifecycleState {
        self.state
    }

    fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    fn supports_action(&self, action: &str) -> bool {
        self.supported_actions
            .iter()
            .any(|candidate| candidate == action)
    }
}

#[derive(Debug, Deserialize)]
struct ManagedNodeLifecycleSnapshotPayload {
    schema: String,
    version: u64,
    instance_id: String,
    epoch: u64,
    sequence: u64,
    scope: ManagedNodeLifecycleScope,
    state: String,
    health: String,
    supported_actions: Vec<String>,
    #[serde(default)]
    last_error: Option<ManagedNodeLifecycleError>,
}

#[derive(Debug, Deserialize)]
struct ManagedNodeLifecycleScope {
    kind: String,
}

#[derive(Debug, Deserialize)]
struct ManagedNodeLifecycleError {
    code: String,
    message: String,
}

fn managed_node_lifecycle_snapshot(
    value: &Value,
    expected_kind: &str,
    label: &str,
) -> Result<ManagedNodeLifecycleSnapshot> {
    let value = match value {
        Value::String(text) => serde_json::from_str(text)
            .with_context(|| format!("{label} response is not valid JSON"))?,
        value => value.clone(),
    };
    let payload: ManagedNodeLifecycleSnapshotPayload = serde_json::from_value(value)
        .with_context(|| format!("{label} response has an invalid shape"))?;
    anyhow::ensure!(
        payload.schema == "logos.managed_node_lifecycle.snapshot" && payload.version == 1,
        "{label} response has an unsupported schema or version"
    );
    anyhow::ensure!(
        !payload.instance_id.trim().is_empty(),
        "{label} response has an empty instance_id"
    );
    anyhow::ensure!(
        payload.scope.kind == expected_kind,
        "{label} response has an unexpected scope"
    );
    anyhow::ensure!(
        matches!(
            payload.health.as_str(),
            "unknown" | "healthy" | "degraded" | "unhealthy" | "not_applicable"
        ),
        "{label} response has an unknown health state"
    );
    let state = ManagedNodeLifecycleState::parse(&payload.state, label)?;
    let actions = payload
        .supported_actions
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>();
    anyhow::ensure!(
        actions
            .iter()
            .all(|action| state.allowed_actions().contains(action))
            && state
                .expected_actions()
                .iter()
                .all(|expected| actions.contains(expected))
            && actions
                .iter()
                .enumerate()
                .all(|(index, action)| !actions.iter().take(index).any(|prior| prior == action)),
        "{label} response has actions inconsistent with its lifecycle state"
    );
    let last_error = payload
        .last_error
        .map(|error| {
            let code = error.code.trim();
            let message = error.message.trim();
            anyhow::ensure!(
                !code.is_empty() && !message.is_empty(),
                "{label} response has an incomplete last_error"
            );
            Ok(format!("{code}: {message}"))
        })
        .transpose()?;
    Ok(ManagedNodeLifecycleSnapshot {
        instance_id: payload.instance_id,
        epoch: payload.epoch,
        sequence: payload.sequence,
        state,
        supported_actions: payload.supported_actions,
        last_error,
    })
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
    let configs = {
        let _state_lock = acquire_state_lock()?;
        let state = store.load()?;
        HOST_NODE_KINDS.map(|kind| {
            state
                .active_topology(profile)
                .and_then(|record| record.nodes.iter().find(|node| node.kind == kind))
                .cloned()
        })
    };

    let (bedrock, storage, messaging) = tokio::join!(
        observe_node(NodeKind::Bedrock, configs[0].as_ref(), module_transport),
        observe_node(NodeKind::Storage, configs[1].as_ref(), module_transport),
        observe_node(NodeKind::Messaging, configs[2].as_ref(), module_transport),
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
    config: Option<&LocalNodeConfigRecord>,
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
    if supports_managed_node_lifecycle_v1(kind, &metadata) {
        return observe_managed_node_lifecycle_v1(kind, module_transport, module).await;
    }

    let contract_error = validate_lifecycle_contract(kind, &metadata)
        .err()
        .map(|error| error.to_string());
    if contract_error.is_some() {
        return HostNodeObservation {
            kind,
            module_available: true,
            contract_error,
            context_initialized: None,
            liveness: None,
            liveness_error: None,
            managed_actions: None,
        };
    }

    if kind == NodeKind::Storage {
        return observe_storage_lifecycle(module_transport, &metadata, module).await;
    }

    let Some((method, signature, args)) = liveness_call(kind) else {
        return HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            context_initialized: None,
            liveness: None,
            liveness_error: None,
            managed_actions: None,
        };
    };
    if let Err(error) = require_method(&metadata, module, method, signature) {
        return HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            context_initialized: None,
            liveness: None,
            liveness_error: Some(error.to_string()),
            managed_actions: None,
        };
    }
    let call = match ModuleCall::new(ModuleTransportKind::Module, module, method, args) {
        Ok(call) => call,
        Err(error) => {
            return HostNodeObservation {
                kind,
                module_available: true,
                contract_error: None,
                context_initialized: None,
                liveness: None,
                liveness_error: Some(error.to_string()),
                managed_actions: None,
            };
        }
    };
    match dispatch_module_call(module_transport.as_ref(), call).await {
        Ok(_) => match service_liveness(kind, config).await {
            Ok(service) => HostNodeObservation {
                kind,
                module_available: true,
                contract_error: None,
                context_initialized: Some(true),
                liveness: service
                    .observed
                    .or_else(|| (kind == NodeKind::Bedrock).then_some(true)),
                liveness_error: service.detail,
                managed_actions: None,
            },
            Err(error) => HostNodeObservation {
                kind,
                module_available: true,
                contract_error: None,
                context_initialized: Some(true),
                liveness: None,
                liveness_error: Some(error.to_string()),
                managed_actions: None,
            },
        },
        Err(error) => {
            let context_missing = is_context_not_initialized(&error);
            let node_stopped = kind == NodeKind::Bedrock && is_node_not_running(&error);
            let context_initialized = if node_stopped {
                Some(config.is_some_and(|config| config.installed))
            } else {
                context_missing.then_some(false)
            };
            HostNodeObservation {
                kind,
                module_available: true,
                contract_error: None,
                context_initialized,
                liveness: node_stopped
                    .then_some(false)
                    .or_else(|| context_missing.then_some(false)),
                liveness_error: Some(error.to_string()),
                managed_actions: None,
            }
        }
    }
}

async fn observe_managed_node_lifecycle_v1(
    kind: NodeKind,
    module_transport: &SharedModuleTransport,
    module: &str,
) -> HostNodeObservation {
    let Some(contract) = managed_node_lifecycle_v1_contract(kind) else {
        return HostNodeObservation::unavailable(kind, "node has no V1 lifecycle contract");
    };
    let label = adapter_for(kind).label();
    let call = match ModuleCall::new(
        ModuleTransportKind::Module,
        module,
        contract.status_method,
        Vec::new(),
    ) {
        Ok(call) => call,
        Err(error) => {
            return HostNodeObservation {
                kind,
                module_available: true,
                contract_error: None,
                context_initialized: None,
                liveness: None,
                liveness_error: Some(error.to_string()),
                managed_actions: None,
            };
        }
    };
    let snapshot = dispatch_module_call(module_transport.as_ref(), call)
        .await
        .map(ModuleCallReply::into_value)
        .and_then(|value| normalize_module_call_value(module, contract.status_method, value))
        .and_then(|value| {
            managed_node_lifecycle_snapshot(&value, contract.scope, &format!("{label} nodeStatus"))
        });
    match snapshot {
        Ok(snapshot) => HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            context_initialized: snapshot.state().context_initialized(),
            liveness: snapshot.state().liveness(),
            liveness_error: snapshot.last_error().map(ToOwned::to_owned),
            managed_actions: Some(snapshot.supported_actions),
        },
        Err(error) => HostNodeObservation {
            kind,
            module_available: true,
            contract_error: None,
            context_initialized: None,
            liveness: None,
            liveness_error: Some(error.to_string()),
            managed_actions: None,
        },
    }
}

async fn observe_storage_lifecycle(
    module_transport: &SharedModuleTransport,
    metadata: &Value,
    module: &str,
) -> HostNodeObservation {
    if supports_storage_node_lifecycle_v1(metadata) {
        return observe_storage_node_lifecycle_v1(module_transport, module).await;
    }
    const METHOD: &str = "lifecycleStatus";
    const SIGNATURE: &str = "lifecycleStatus()";
    if let Err(error) = require_method(metadata, module, METHOD, SIGNATURE) {
        return HostNodeObservation {
            kind: NodeKind::Storage,
            module_available: true,
            contract_error: Some(error.to_string()),
            context_initialized: None,
            liveness: None,
            liveness_error: None,
            managed_actions: None,
        };
    }
    let call = match ModuleCall::new(ModuleTransportKind::Module, module, METHOD, Vec::new()) {
        Ok(call) => call,
        Err(error) => {
            return HostNodeObservation {
                kind: NodeKind::Storage,
                module_available: true,
                contract_error: None,
                context_initialized: None,
                liveness: None,
                liveness_error: Some(error.to_string()),
                managed_actions: None,
            };
        }
    };
    let status = dispatch_module_call(module_transport.as_ref(), call)
        .await
        .map(ModuleCallReply::into_value)
        .and_then(|value| normalize_module_call_value(module, METHOD, value))
        .and_then(|value| managed_lifecycle_status(&value));
    match status {
        Ok(status) => HostNodeObservation {
            kind: NodeKind::Storage,
            module_available: true,
            contract_error: None,
            context_initialized: Some(status.initialized()),
            liveness: status.liveness(),
            liveness_error: None,
            managed_actions: None,
        },
        Err(error) => HostNodeObservation {
            kind: NodeKind::Storage,
            module_available: true,
            contract_error: None,
            context_initialized: None,
            liveness: None,
            liveness_error: Some(error.to_string()),
            managed_actions: None,
        },
    }
}

async fn observe_storage_node_lifecycle_v1(
    module_transport: &SharedModuleTransport,
    module: &str,
) -> HostNodeObservation {
    let call = match ModuleCall::new(
        ModuleTransportKind::Module,
        module,
        STORAGE_NODE_STATUS_METHOD,
        Vec::new(),
    ) {
        Ok(call) => call,
        Err(error) => {
            return HostNodeObservation {
                kind: NodeKind::Storage,
                module_available: true,
                contract_error: None,
                context_initialized: None,
                liveness: None,
                liveness_error: Some(error.to_string()),
                managed_actions: None,
            };
        }
    };
    let snapshot = dispatch_module_call(module_transport.as_ref(), call)
        .await
        .map(ModuleCallReply::into_value)
        .and_then(|value| normalize_module_call_value(module, STORAGE_NODE_STATUS_METHOD, value))
        .and_then(|value| storage_node_lifecycle_snapshot(&value));
    match snapshot {
        Ok(snapshot) => storage_node_lifecycle_observation(snapshot),
        Err(error) => HostNodeObservation {
            kind: NodeKind::Storage,
            module_available: true,
            contract_error: None,
            context_initialized: None,
            liveness: None,
            liveness_error: Some(error.to_string()),
            managed_actions: None,
        },
    }
}

fn storage_node_lifecycle_observation(
    snapshot: StorageNodeLifecycleSnapshot,
) -> HostNodeObservation {
    HostNodeObservation {
        kind: NodeKind::Storage,
        module_available: true,
        contract_error: None,
        context_initialized: snapshot.state().context_initialized(),
        liveness: snapshot.state().liveness(),
        liveness_error: snapshot.last_error().map(ToOwned::to_owned),
        managed_actions: None,
    }
}

fn is_context_not_initialized(error: &anyhow::Error) -> bool {
    format!("{error:#}")
        .to_ascii_lowercase()
        .contains("context not initialized")
}

fn is_node_not_running(error: &anyhow::Error) -> bool {
    format!("{error:#}")
        .to_ascii_lowercase()
        .contains("node is not running")
}

async fn service_liveness(
    kind: NodeKind,
    config: Option<&LocalNodeConfigRecord>,
) -> Result<ServiceLiveness> {
    service_liveness_with_timeout(kind, config, SERVICE_TRANSITION_TIMEOUT).await
}

async fn service_liveness_with_timeout(
    kind: NodeKind,
    config: Option<&LocalNodeConfigRecord>,
    transition_timeout: Duration,
) -> Result<ServiceLiveness> {
    if kind != NodeKind::Messaging {
        return Ok(ServiceLiveness {
            observed: None,
            detail: None,
        });
    }
    let config = config.context("Basecamp node config is unavailable")?;
    let address = service_address(kind, &config.config_path)?;
    let desired = match config.pending_lifecycle_action {
        Some(NodeAction::Start) => Some(true),
        Some(NodeAction::Stop) => Some(false),
        _ => None,
    };
    let deadline = Instant::now() + transition_timeout;
    loop {
        let probe = tokio::task::spawn_blocking(move || service_liveness_at(kind, address))
            .await
            .context("Basecamp service liveness worker failed")?;
        match probe {
            ServiceProbe::Known(observed) => {
                if desired.is_none_or(|expected| expected == observed) {
                    return Ok(ServiceLiveness {
                        observed: Some(observed),
                        detail: None,
                    });
                }
                if Instant::now() >= deadline {
                    return Ok(ServiceLiveness {
                        observed: Some(observed),
                        detail: Some(transition_timeout_detail(kind, desired, None)),
                    });
                }
            }
            ServiceProbe::Inconclusive(health) => {
                if desired.is_none() {
                    return Ok(ServiceLiveness {
                        observed: None,
                        detail: Some(format!(
                            "Basecamp {} REST health is {health}",
                            adapter_for(kind).label()
                        )),
                    });
                }
                if Instant::now() >= deadline {
                    return Ok(ServiceLiveness {
                        observed: None,
                        detail: Some(transition_timeout_detail(kind, desired, Some(health))),
                    });
                }
            }
        }
        tokio::time::sleep(SERVICE_TRANSITION_POLL_INTERVAL).await;
    }
}

fn transition_timeout_detail(
    kind: NodeKind,
    desired: Option<bool>,
    health: Option<&str>,
) -> String {
    let expected = match desired {
        Some(true) => "running",
        Some(false) => "stopped",
        None => "a confirmed state",
    };
    let suffix = health.map_or_else(String::new, |health| format!("; last health: {health}"));
    format!(
        "Basecamp {} did not reach {expected} before the lifecycle confirmation timeout{suffix}",
        adapter_for(kind).label()
    )
}

fn service_liveness_at(kind: NodeKind, address: SocketAddr) -> ServiceProbe {
    match kind {
        NodeKind::Storage => ServiceProbe::Inconclusive("unsupported"),
        NodeKind::Messaging => {
            let health = probe_messaging_health(address);
            health.liveness().map_or_else(
                || ServiceProbe::Inconclusive(health.as_str()),
                ServiceProbe::Known,
            )
        }
        NodeKind::Bedrock | NodeKind::Sequencer | NodeKind::Indexer => {
            ServiceProbe::Inconclusive("unsupported")
        }
    }
}

fn service_address(kind: NodeKind, config_path: &str) -> Result<SocketAddr> {
    let config_text = fs::read_to_string(config_path)
        .with_context(|| format!("failed to read Basecamp node config `{config_path}`"))?;
    let config = serde_json::from_str::<Value>(&config_text)
        .with_context(|| format!("failed to parse Basecamp node config `{config_path}`"))?;
    let (host_key, port_key) = match kind {
        NodeKind::Storage => ("listen-ip", "listen-port"),
        NodeKind::Messaging => ("restAddress", "restPort"),
        NodeKind::Bedrock | NodeKind::Sequencer | NodeKind::Indexer => {
            bail!("{} has no local service liveness target", kind.as_str())
        }
    };
    let host = config
        .get(host_key)
        .and_then(Value::as_str)
        .with_context(|| format!("Basecamp node config has no `{host_key}`"))?;
    let port = config
        .get(port_key)
        .and_then(Value::as_u64)
        .and_then(|value| u16::try_from(value).ok())
        .with_context(|| format!("Basecamp node config has no valid `{port_key}`"))?;
    let ip = host
        .parse::<IpAddr>()
        .with_context(|| format!("Basecamp node config has invalid `{host_key}`"))?;
    let ip = match ip {
        IpAddr::V4(address) if address.is_unspecified() => IpAddr::V4(Ipv4Addr::LOCALHOST),
        IpAddr::V6(address) if address.is_unspecified() => IpAddr::V6(Ipv6Addr::LOCALHOST),
        address => address,
    };
    Ok(SocketAddr::new(ip, port))
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
        if let Some(event) = contract.lifecycle_event(managed_action) {
            let signature = contract
                .lifecycle_event_signature(managed_action)
                .with_context(|| {
                    format!(
                        "Basecamp {} lifecycle event `{event}` has no declared signature",
                        adapter.label()
                    )
                })?;
            require_event(metadata, contract.module_id(), event, signature)?;
        }
    }
    if kind == NodeKind::Storage {
        require_method(
            metadata,
            contract.module_id(),
            "lifecycleStatus",
            "lifecycleStatus()",
        )?;
        let spec = contract
            .call_spec(ManagedNodeAction::Destroy, "")
            .context("Storage has no destroy module call")?;
        require_method(metadata, contract.module_id(), spec.method, spec.signature)?;
    }
    Ok(())
}

fn require_method(metadata: &Value, module: &str, method: &str, signature: &str) -> Result<()> {
    if metadata_has_method(metadata, method, signature) {
        return Ok(());
    }
    bail!("Basecamp module `{module}` does not expose `{signature}`")
}

fn metadata_has_method(metadata: &Value, method: &str, signature: &str) -> bool {
    let methods = metadata.get("methods").and_then(Value::as_array);
    methods.is_some_and(|methods| {
        methods.iter().any(|candidate| {
            candidate.get("name").and_then(Value::as_str) == Some(method)
                && candidate.get("signature").and_then(Value::as_str) == Some(signature)
                && candidate.get("isInvokable").and_then(Value::as_bool) != Some(false)
        })
    })
}

fn require_event(metadata: &Value, module: &str, event: &str, signature: &str) -> Result<()> {
    if metadata_has_event(metadata, event, signature) {
        return Ok(());
    }
    bail!("Basecamp module `{module}` does not expose lifecycle event `{signature}`")
}

fn metadata_has_event(metadata: &Value, event: &str, signature: &str) -> bool {
    let events = metadata.get("events").and_then(Value::as_array);
    events.is_some_and(|events| {
        events.iter().any(|candidate| {
            candidate.get("name").and_then(Value::as_str) == Some(event)
                && candidate.get("signature").and_then(Value::as_str) == Some(signature)
        })
    })
}

fn supports_storage_node_lifecycle_v1(metadata: &Value) -> bool {
    metadata_has_method(
        metadata,
        STORAGE_NODE_STATUS_METHOD,
        STORAGE_NODE_STATUS_SIGNATURE,
    ) && metadata_has_method(
        metadata,
        STORAGE_NODE_ACTION_METHOD,
        STORAGE_NODE_ACTION_SIGNATURE,
    ) && metadata_has_event(
        metadata,
        STORAGE_NODE_CHANGED_EVENT,
        STORAGE_NODE_CHANGED_SIGNATURE,
    )
}

#[derive(Clone, Copy)]
struct ManagedNodeLifecycleV1Contract {
    scope: &'static str,
    status_method: &'static str,
    status_signature: &'static str,
    action_method: &'static str,
    action_signature: &'static str,
    changed_event: &'static str,
    changed_signature: &'static str,
    operation_serial: &'static AtomicU64,
}

fn managed_node_lifecycle_v1_contract(kind: NodeKind) -> Option<ManagedNodeLifecycleV1Contract> {
    match kind {
        NodeKind::Bedrock => Some(ManagedNodeLifecycleV1Contract {
            scope: "bedrock",
            status_method: BEDROCK_NODE_STATUS_METHOD,
            status_signature: BEDROCK_NODE_STATUS_SIGNATURE,
            action_method: BEDROCK_NODE_ACTION_METHOD,
            action_signature: BEDROCK_NODE_ACTION_SIGNATURE,
            changed_event: BEDROCK_NODE_CHANGED_EVENT,
            changed_signature: BEDROCK_NODE_CHANGED_SIGNATURE,
            operation_serial: &BEDROCK_NODE_LIFECYCLE_OPERATION_SERIAL,
        }),
        NodeKind::Messaging => Some(ManagedNodeLifecycleV1Contract {
            scope: "messaging",
            status_method: MESSAGING_NODE_STATUS_METHOD,
            status_signature: MESSAGING_NODE_STATUS_SIGNATURE,
            action_method: MESSAGING_NODE_ACTION_METHOD,
            action_signature: MESSAGING_NODE_ACTION_SIGNATURE,
            changed_event: MESSAGING_NODE_CHANGED_EVENT,
            changed_signature: MESSAGING_NODE_CHANGED_SIGNATURE,
            operation_serial: &MESSAGING_NODE_LIFECYCLE_OPERATION_SERIAL,
        }),
        NodeKind::Storage | NodeKind::Sequencer | NodeKind::Indexer => None,
    }
}

fn supports_managed_node_lifecycle_v1(kind: NodeKind, metadata: &Value) -> bool {
    let Some(contract) = managed_node_lifecycle_v1_contract(kind) else {
        return false;
    };
    metadata_has_method(metadata, contract.status_method, contract.status_signature)
        && metadata_has_method(metadata, contract.action_method, contract.action_signature)
        && managed_node_lifecycle_event_metadata_compatible(metadata, contract)
}

fn managed_node_lifecycle_event_metadata_compatible(
    metadata: &Value,
    contract: ManagedNodeLifecycleV1Contract,
) -> bool {
    let Some(events) = metadata.get("events").and_then(Value::as_array) else {
        return true;
    };
    events.is_empty()
        || metadata_has_event(metadata, contract.changed_event, contract.changed_signature)
}

fn liveness_call(kind: NodeKind) -> Option<(&'static str, &'static str, Vec<Value>)> {
    match kind {
        NodeKind::Bedrock => Some(("get_cryptarchia_info", "get_cryptarchia_info()", Vec::new())),
        NodeKind::Storage => None,
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
    let kind = request.node.context("node kind is required")?;
    if !HOST_NODE_KINDS.contains(&kind) {
        bail!(
            "{} is not hosted by the Basecamp Local Nodes surface; configure it from its Zone",
            adapter_for(kind).label()
        );
    }
    let action = request.action;
    anyhow::ensure!(
        matches!(
            action,
            NodeAction::Initialize | NodeAction::Start | NodeAction::Stop | NodeAction::Uninstall
        ),
        "{} is not a Basecamp module lifecycle action",
        action.as_str()
    );
    anyhow::ensure!(
        !request.allow_identity_rotation
            || matches!((action, kind), (NodeAction::Stop, NodeKind::Messaging)),
        "identity rotation acknowledgement only applies to Messaging Stop"
    );
    // V1 module capabilities come from nodeStatus, including optional actions
    // such as destroy. Static workflow actions remain for the local CLI path.
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
    let call = contract.call_spec(managed_action, action_path);
    anyhow::ensure!(
        call.is_some() || action == NodeAction::Uninstall,
        "{} {} is not implemented",
        adapter.label(),
        action.as_str()
    );
    let args = call
        .as_ref()
        .map(|call| native_args(&call.args))
        .transpose()?
        .unwrap_or_default();
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
) -> Result<HostActionExecution> {
    execute_host_action_with_lifecycle_timeout(plan, module_transport, HOST_LIFECYCLE_EVENT_TIMEOUT)
        .await
}

async fn execute_host_action_with_lifecycle_timeout(
    plan: &PreparedHostAction,
    module_transport: &SharedModuleTransport,
    lifecycle_timeout: Duration,
) -> Result<HostActionExecution> {
    let metadata = module_transport.module_info(plan.module.to_owned()).await?;
    if supports_managed_node_lifecycle_v1(plan.kind, &metadata) {
        return execute_managed_node_lifecycle_v1_action(
            plan,
            module_transport,
            &metadata,
            lifecycle_timeout,
        )
        .await;
    }
    validate_lifecycle_contract(plan.kind, &metadata)?;
    if plan.kind == NodeKind::Storage
        && matches!(plan.action, NodeAction::Start | NodeAction::Stop)
        && supports_storage_node_lifecycle_v1(&metadata)
    {
        return execute_storage_node_lifecycle_v1_action(plan, module_transport).await;
    }
    let call = plan.call.as_ref().with_context(|| {
        format!(
            "{} {} is not implemented",
            adapter_for(plan.kind).label(),
            plan.action.as_str()
        )
    })?;
    require_method(&metadata, plan.module, call.method, call.signature)?;
    let lifecycle = subscribe_host_lifecycle_event(plan, module_transport, &metadata)?;
    let module_call = ModuleCall::new(
        ModuleTransportKind::Module,
        plan.module,
        call.method,
        plan.args.clone(),
    )?;
    let value = dispatch_module_call(module_transport.as_ref(), module_call)
        .await?
        .into_value();
    validate_host_action_result(plan, &value)?;
    let lifecycle_detail = match lifecycle {
        Some(lifecycle) => {
            Some(wait_for_host_lifecycle_event(plan, lifecycle, lifecycle_timeout).await?)
        }
        None => None,
    };
    Ok(HostActionExecution {
        lifecycle_detail,
        dispatched_method: call.method,
    })
}

async fn execute_managed_node_lifecycle_v1_action(
    plan: &PreparedHostAction,
    module_transport: &SharedModuleTransport,
    metadata: &Value,
    lifecycle_timeout: Duration,
) -> Result<HostActionExecution> {
    let contract = managed_node_lifecycle_v1_contract(plan.kind)
        .context("node has no V1 lifecycle contract")?;
    let label = adapter_for(plan.kind).label();
    anyhow::ensure!(
        module_transport.native_runtime_module_events_ready(),
        "Basecamp host does not own healthy native lifecycle event ingress"
    );
    let snapshot =
        managed_node_lifecycle_snapshot_for_action(plan.kind, plan.module, module_transport)
            .await?;
    let (lifecycle_action, parameters) = managed_node_lifecycle_action_request(plan)?;
    anyhow::ensure!(
        snapshot.supports_action(lifecycle_action.wire_name),
        "{label} V1 nodeStatus does not allow `{}` in its current state",
        lifecycle_action.wire_name
    );
    let lifecycle = subscribe_managed_node_lifecycle_event(plan, module_transport, metadata)?;
    let serial = contract.operation_serial.fetch_add(1, Ordering::Relaxed);
    let operation_id = format!(
        "logos-inspector-{}-{}-{}-{serial}",
        contract.scope,
        lifecycle_action.wire_name,
        now_millis()
    );
    let request = json!({
        "schema": "logos.managed_node_lifecycle.command",
        "version": 1,
        "operation_id": operation_id,
        "action": lifecycle_action.wire_name,
        "expected": {
            "instance_id": snapshot.instance_id(),
            "epoch": snapshot.epoch(),
            "sequence": snapshot.sequence(),
        },
        "parameters": parameters,
    });
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        plan.module,
        contract.action_method,
        vec![Value::String(request.to_string())],
    )?;
    let value = dispatch_module_call(module_transport.as_ref(), call)
        .await?
        .into_value();
    let value = normalize_module_call_value(plan.module, contract.action_method, value)?;
    let acknowledged = validate_managed_node_lifecycle_ack(
        plan.kind,
        &value,
        &operation_id,
        &snapshot,
        lifecycle_action.expected_transition,
    )?;
    let lifecycle_detail = if acknowledged {
        wait_for_managed_node_lifecycle_event(
            plan,
            lifecycle,
            &operation_id,
            &snapshot,
            lifecycle_action,
            lifecycle_timeout,
        )
        .await?
    } else {
        wait_for_managed_node_lifecycle_status(
            plan,
            module_transport,
            &snapshot,
            lifecycle_action,
            lifecycle_timeout,
        )
        .await?
    };
    Ok(HostActionExecution {
        lifecycle_detail: Some(lifecycle_detail),
        dispatched_method: contract.action_method,
    })
}

fn managed_node_lifecycle_action_request(
    plan: &PreparedHostAction,
) -> Result<(ManagedNodeLifecycleAction, Value)> {
    let label = adapter_for(plan.kind).label();
    match plan.action {
        NodeAction::Initialize => {
            let config = plan.args.first().and_then(Value::as_str).with_context(|| {
                format!("{label} initialization has no generated configuration")
            })?;
            let parsed: Value = serde_json::from_str(config).with_context(|| {
                format!("{label} initialization configuration is not valid JSON")
            })?;
            anyhow::ensure!(
                parsed.is_object(),
                "{label} initialization configuration must be a JSON object"
            );
            Ok((
                ManagedNodeLifecycleAction {
                    wire_name: "initialize",
                    expected_transition: "initializing",
                    expected_terminal: "stopped",
                },
                json!({ "config": config }),
            ))
        }
        NodeAction::Start => {
            let parameters = match plan.kind {
                NodeKind::Bedrock => {
                    let deployment = plan.args.get(1).and_then(Value::as_str).unwrap_or_default();
                    if deployment.is_empty() {
                        json!({})
                    } else {
                        json!({ "deployment": deployment })
                    }
                }
                NodeKind::Messaging => json!({}),
                NodeKind::Storage | NodeKind::Sequencer | NodeKind::Indexer => {
                    bail!("{label} V1 lifecycle is not supported")
                }
            };
            Ok((
                ManagedNodeLifecycleAction {
                    wire_name: "start",
                    expected_transition: "starting",
                    expected_terminal: "running",
                },
                parameters,
            ))
        }
        NodeAction::Stop => Ok((
            ManagedNodeLifecycleAction {
                wire_name: "stop",
                expected_transition: "stopping",
                expected_terminal: "stopped",
            },
            json!({}),
        )),
        NodeAction::Uninstall => Ok((
            ManagedNodeLifecycleAction {
                wire_name: "destroy",
                expected_transition: "destroying",
                expected_terminal: "uninitialized",
            },
            json!({}),
        )),
        action => bail!("{label} V1 lifecycle does not support {}", action.as_str()),
    }
}

async fn managed_node_lifecycle_snapshot_for_action(
    kind: NodeKind,
    module: &str,
    module_transport: &SharedModuleTransport,
) -> Result<ManagedNodeLifecycleSnapshot> {
    let contract =
        managed_node_lifecycle_v1_contract(kind).context("node has no V1 lifecycle contract")?;
    let label = adapter_for(kind).label();
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        module,
        contract.status_method,
        Vec::new(),
    )?;
    let value = dispatch_module_call(module_transport.as_ref(), call)
        .await?
        .into_value();
    let value = normalize_module_call_value(module, contract.status_method, value)?;
    managed_node_lifecycle_snapshot(&value, contract.scope, &format!("{label} nodeStatus"))
}

fn subscribe_managed_node_lifecycle_event(
    plan: &PreparedHostAction,
    module_transport: &SharedModuleTransport,
    metadata: &Value,
) -> Result<HostLifecycleSubscription> {
    let contract = managed_node_lifecycle_v1_contract(plan.kind)
        .context("node has no V1 lifecycle contract")?;
    anyhow::ensure!(
        managed_node_lifecycle_event_metadata_compatible(metadata, contract),
        "Basecamp module `{}` advertises lifecycle events but omits `{}`",
        plan.module,
        contract.changed_signature,
    );
    let subscription = module_transport
        .subscribe_module_event(plan.module, contract.changed_event)
        .with_context(|| {
            format!(
                "Basecamp host cannot subscribe to {} nodeChanged confirmation",
                adapter_for(plan.kind).label()
            )
        })?;
    Ok(HostLifecycleSubscription {
        event: contract.changed_event,
        subscription,
    })
}

fn validate_managed_node_lifecycle_ack(
    kind: NodeKind,
    value: &Value,
    operation_id: &str,
    snapshot: &ManagedNodeLifecycleSnapshot,
    expected_state: &str,
) -> Result<bool> {
    let label = adapter_for(kind).label();
    let acknowledgement = match value {
        Value::String(text) => serde_json::from_str(text)
            .with_context(|| format!("{label} nodeAction response is not valid JSON"))?,
        value => value.clone(),
    };
    // Basecamp's Qt module adapter cannot marshal a plugin method whose return
    // type is `std::string` into QVariant. A null reply therefore means only
    // that the invocation result is unavailable; confirmation must come from
    // the module's event or status path after dispatch.
    if acknowledgement.is_null() {
        return Ok(false);
    }
    anyhow::ensure!(
        acknowledgement.get("schema").and_then(Value::as_str)
            == Some("logos.managed_node_lifecycle.ack")
            && acknowledgement.get("version").and_then(Value::as_u64) == Some(1),
        "{label} nodeAction response has an unsupported schema or version: {}",
        crate::response_excerpt(&acknowledgement.to_string())
    );
    anyhow::ensure!(
        acknowledgement.get("operation_id").and_then(Value::as_str) == Some(operation_id),
        "{label} nodeAction response does not acknowledge the submitted operation"
    );
    anyhow::ensure!(
        acknowledgement.get("duplicate").and_then(Value::as_bool) == Some(false),
        "{label} nodeAction response unexpectedly reused the operation"
    );
    let accepted = acknowledgement
        .get("accepted")
        .and_then(Value::as_bool)
        .with_context(|| format!("{label} nodeAction response has no accepted field"))?;
    if !accepted {
        let detail = acknowledgement
            .get("error")
            .and_then(Value::as_object)
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .unwrap_or("module rejected the lifecycle request");
        bail!("{label} nodeAction rejected the lifecycle request: {detail}");
    }
    let instance_id = acknowledgement
        .get("instance_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .with_context(|| format!("{label} nodeAction response has no instance_id"))?;
    let epoch = acknowledgement
        .get("epoch")
        .and_then(Value::as_u64)
        .with_context(|| format!("{label} nodeAction response has no epoch"))?;
    let sequence = acknowledgement
        .get("sequence")
        .and_then(Value::as_u64)
        .with_context(|| format!("{label} nodeAction response has no sequence"))?;
    anyhow::ensure!(
        instance_id == snapshot.instance_id()
            && epoch == snapshot.epoch()
            && sequence > snapshot.sequence(),
        "{label} nodeAction response has an invalid lifecycle cursor"
    );
    anyhow::ensure!(
        acknowledgement.get("state").and_then(Value::as_str) == Some(expected_state),
        "{label} nodeAction response has an unexpected lifecycle state"
    );
    Ok(true)
}

async fn wait_for_managed_node_lifecycle_status(
    plan: &PreparedHostAction,
    module_transport: &SharedModuleTransport,
    snapshot: &ManagedNodeLifecycleSnapshot,
    expected: ManagedNodeLifecycleAction,
    timeout: Duration,
) -> Result<String> {
    let deadline = Instant::now() + timeout;
    let label = adapter_for(plan.kind).label();
    loop {
        let current =
            managed_node_lifecycle_snapshot_for_action(plan.kind, plan.module, module_transport)
                .await?;
        if current.sequence() > snapshot.sequence()
            && current.state().as_str() == expected.expected_terminal
        {
            if let Some(error) = current.last_error() {
                bail!(
                    "Basecamp {label} V1 {} failed: {error}",
                    plan.action.as_str()
                );
            }
            return Ok(format!(
                "V1 {} transition confirmed by nodeStatus after an unmarshalable host reply",
                plan.action.as_str()
            ));
        }
        let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
            bail!(
                "Basecamp {label} nodeStatus did not confirm {} before lifecycle confirmation timeout",
                plan.action.as_str()
            );
        };
        tokio::time::sleep(remaining.min(Duration::from_millis(250))).await;
    }
}

#[derive(Debug, Deserialize)]
struct ManagedNodeLifecycleEventPayload {
    schema: String,
    version: u64,
    instance_id: String,
    epoch: u64,
    sequence: u64,
    scope: ManagedNodeLifecycleScope,
    operation_id: Option<String>,
    action: String,
    phase: String,
    outcome: String,
    previous_state: String,
    status: Value,
    #[serde(default)]
    error: Option<ManagedNodeLifecycleError>,
    emitted_at_ms: i64,
}

struct ManagedNodeLifecycleEvent {
    instance_id: String,
    epoch: u64,
    sequence: u64,
    operation_id: Option<String>,
    action: String,
    phase: String,
    outcome: String,
    status: ManagedNodeLifecycleSnapshot,
    error: Option<String>,
}

fn managed_node_lifecycle_event(
    kind: NodeKind,
    module: &str,
    expected_event: &str,
    event: &ModuleTransportEvent,
) -> Result<ManagedNodeLifecycleEvent> {
    let contract =
        managed_node_lifecycle_v1_contract(kind).context("node has no V1 lifecycle contract")?;
    let label = adapter_for(kind).label();
    anyhow::ensure!(
        event.module() == module && event.event() == expected_event,
        "Basecamp {label} lifecycle subscription received an unexpected event"
    );
    anyhow::ensure!(
        event.args().len() == 1,
        "Basecamp {label} nodeChanged has an invalid argument count"
    );
    let payload = event
        .args()
        .first()
        .and_then(Value::as_str)
        .with_context(|| format!("Basecamp {label} nodeChanged payload is not a string"))?;
    let payload: ManagedNodeLifecycleEventPayload = serde_json::from_str(payload)
        .with_context(|| format!("Basecamp {label} nodeChanged payload is not valid JSON"))?;
    anyhow::ensure!(
        payload.schema == "logos.managed_node_lifecycle.event" && payload.version == 1,
        "Basecamp {label} nodeChanged has an unsupported schema or version"
    );
    anyhow::ensure!(
        payload.scope.kind == contract.scope && !payload.instance_id.trim().is_empty(),
        "Basecamp {label} nodeChanged has an invalid scope or instance_id"
    );
    anyhow::ensure!(
        payload.emitted_at_ms >= 0,
        "Basecamp {label} nodeChanged has an invalid timestamp"
    );
    let _previous_state = ManagedNodeLifecycleState::parse(
        &payload.previous_state,
        &format!("Basecamp {label} nodeChanged"),
    )?;
    let status = managed_node_lifecycle_snapshot(
        &payload.status,
        contract.scope,
        &format!("Basecamp {label} nodeChanged status"),
    )?;
    anyhow::ensure!(
        status.instance_id() == payload.instance_id
            && status.epoch() == payload.epoch
            && status.sequence() == payload.sequence,
        "Basecamp {label} nodeChanged status cursor does not match its event"
    );
    let error = payload
        .error
        .map(|error| {
            let code = error.code.trim();
            let message = error.message.trim();
            anyhow::ensure!(
                !code.is_empty() && !message.is_empty(),
                "Basecamp {label} nodeChanged error is incomplete"
            );
            Ok(format!("{code}: {message}"))
        })
        .transpose()?;
    Ok(ManagedNodeLifecycleEvent {
        instance_id: payload.instance_id,
        epoch: payload.epoch,
        sequence: payload.sequence,
        operation_id: payload.operation_id,
        action: payload.action,
        phase: payload.phase,
        outcome: payload.outcome,
        status,
        error,
    })
}

async fn wait_for_managed_node_lifecycle_event(
    plan: &PreparedHostAction,
    mut lifecycle: HostLifecycleSubscription,
    operation_id: &str,
    snapshot: &ManagedNodeLifecycleSnapshot,
    expected: ManagedNodeLifecycleAction,
    timeout: Duration,
) -> Result<String> {
    let operation_id = operation_id.to_owned();
    let initial_instance_id = snapshot.instance_id().to_owned();
    let initial_epoch = snapshot.epoch();
    let initial_sequence = snapshot.sequence();
    let action = plan.action.as_str();
    let expected_action = expected.wire_name.to_owned();
    let expected_transition = expected.expected_transition.to_owned();
    let expected_terminal = expected.expected_terminal.to_owned();
    let kind = plan.kind;
    let module = plan.module.to_owned();
    let label = adapter_for(kind).label();
    tokio::task::spawn_blocking(move || {
        let deadline = Instant::now() + timeout;
        let mut accepted = false;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let event = lifecycle
                .subscription
                .next_within(remaining)?
                .with_context(|| {
                    format!(
                        "Basecamp {label} did not emit {} before lifecycle confirmation timeout",
                        lifecycle.event
                    )
                })?;
            let event = managed_node_lifecycle_event(kind, &module, lifecycle.event, &event)?;
            if event.operation_id.as_deref() != Some(operation_id.as_str()) {
                continue;
            }
            anyhow::ensure!(
                event.instance_id == initial_instance_id
                    && event.epoch >= initial_epoch
                    && event.sequence > initial_sequence,
                "Basecamp {label} nodeChanged has an invalid lifecycle cursor"
            );
            anyhow::ensure!(
                event.action == expected_action,
                "Basecamp {label} nodeChanged does not match the requested action"
            );
            match event.phase.as_str() {
                "accepted" => {
                    anyhow::ensure!(
                        event.outcome == "accepted"
                            && event.status.state().as_str() == expected_transition,
                        "Basecamp {label} nodeChanged has an invalid accepted transition"
                    );
                    accepted = true;
                }
                "settled" => {
                    anyhow::ensure!(
                        accepted,
                        "Basecamp {label} nodeChanged settled before its accepted transition"
                    );
                    if !matches!(event.outcome.as_str(), "succeeded" | "no_op") {
                        let detail = event
                            .error
                            .as_deref()
                            .or(event.status.last_error())
                            .unwrap_or("module reported a lifecycle failure");
                        bail!("Basecamp {label} V1 {action} failed: {detail}");
                    }
                    anyhow::ensure!(
                        event.status.state().as_str() == expected_terminal,
                        "Basecamp {label} nodeChanged has an unexpected terminal state"
                    );
                    return Ok(format!("V1 {action} transition settled"));
                }
                _ => bail!("Basecamp {label} nodeChanged has an unknown lifecycle phase"),
            }
        }
    })
    .await
    .context("Basecamp Bedrock lifecycle event worker failed")?
}

async fn execute_storage_node_lifecycle_v1_action(
    plan: &PreparedHostAction,
    module_transport: &SharedModuleTransport,
) -> Result<HostActionExecution> {
    anyhow::ensure!(
        module_transport.native_runtime_module_events_ready(),
        "Basecamp host does not own healthy native lifecycle event ingress"
    );
    let snapshot =
        storage_node_lifecycle_snapshot_for_action(plan.module, module_transport).await?;
    let action = match plan.action {
        NodeAction::Start => "start",
        NodeAction::Stop => "stop",
        _ => bail!("Storage V1 lifecycle does not support this action"),
    };
    let expected_state = match plan.action {
        NodeAction::Start => "starting",
        NodeAction::Stop => "stopping",
        _ => bail!("Storage V1 lifecycle does not support this action"),
    };
    let serial = STORAGE_NODE_LIFECYCLE_OPERATION_SERIAL.fetch_add(1, Ordering::Relaxed);
    let operation_id = format!("logos-inspector-storage-{action}-{}-{serial}", now_millis());
    let request = json!({
        "schema": "logos.managed_node_lifecycle.command",
        "version": 1,
        "operation_id": operation_id,
        "action": action,
        "expected": {
            "instance_id": snapshot.instance_id(),
            "epoch": snapshot.epoch(),
            "sequence": snapshot.sequence(),
        },
    });
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        plan.module,
        STORAGE_NODE_ACTION_METHOD,
        vec![Value::String(request.to_string())],
    )?;
    let value = dispatch_module_call(module_transport.as_ref(), call)
        .await?
        .into_value();
    let value = normalize_module_call_value(plan.module, STORAGE_NODE_ACTION_METHOD, value)?;
    validate_storage_node_lifecycle_ack(&value, &operation_id, &snapshot, expected_state)?;
    Ok(HostActionExecution {
        lifecycle_detail: None,
        dispatched_method: STORAGE_NODE_ACTION_METHOD,
    })
}

async fn storage_node_lifecycle_snapshot_for_action(
    module: &str,
    module_transport: &SharedModuleTransport,
) -> Result<StorageNodeLifecycleSnapshot> {
    let call = ModuleCall::new(
        ModuleTransportKind::Module,
        module,
        STORAGE_NODE_STATUS_METHOD,
        Vec::new(),
    )?;
    let value = dispatch_module_call(module_transport.as_ref(), call)
        .await?
        .into_value();
    let value = normalize_module_call_value(module, STORAGE_NODE_STATUS_METHOD, value)?;
    storage_node_lifecycle_snapshot(&value)
}

fn validate_storage_node_lifecycle_ack(
    value: &Value,
    operation_id: &str,
    snapshot: &StorageNodeLifecycleSnapshot,
    expected_state: &str,
) -> Result<()> {
    let acknowledgement = match value {
        Value::String(text) => {
            serde_json::from_str(text).context("Storage nodeAction response is not valid JSON")?
        }
        value => value.clone(),
    };
    let schema = acknowledgement.get("schema").and_then(Value::as_str);
    let version = acknowledgement.get("version").and_then(Value::as_u64);
    anyhow::ensure!(
        schema == Some("logos.managed_node_lifecycle.ack") && version == Some(1),
        "Storage nodeAction response has an unsupported schema or version"
    );
    anyhow::ensure!(
        acknowledgement.get("operation_id").and_then(Value::as_str) == Some(operation_id),
        "Storage nodeAction response does not acknowledge the submitted operation"
    );
    let accepted = acknowledgement
        .get("accepted")
        .and_then(Value::as_bool)
        .context("Storage nodeAction response has no accepted field")?;
    if !accepted {
        let detail = acknowledgement
            .get("error")
            .and_then(Value::as_object)
            .and_then(|error| error.get("message"))
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .unwrap_or("Storage rejected the lifecycle request");
        bail!("Storage nodeAction rejected the lifecycle request: {detail}");
    }
    let instance_id = acknowledgement
        .get("instance_id")
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .context("Storage nodeAction response has no instance_id")?;
    let epoch = acknowledgement
        .get("epoch")
        .and_then(Value::as_u64)
        .context("Storage nodeAction response has no epoch")?;
    let sequence = acknowledgement
        .get("sequence")
        .and_then(Value::as_u64)
        .context("Storage nodeAction response has no sequence")?;
    anyhow::ensure!(
        instance_id == snapshot.instance_id()
            && epoch == snapshot.epoch()
            && sequence > snapshot.sequence(),
        "Storage nodeAction response has an invalid lifecycle cursor"
    );
    anyhow::ensure!(
        acknowledgement.get("state").and_then(Value::as_str) == Some(expected_state),
        "Storage nodeAction response has an unexpected lifecycle state"
    );
    Ok(())
}

fn subscribe_host_lifecycle_event(
    plan: &PreparedHostAction,
    module_transport: &SharedModuleTransport,
    metadata: &Value,
) -> Result<Option<HostLifecycleSubscription>> {
    let contract = adapter_for(plan.kind)
        .managed_contract()
        .context("node has no managed module contract")?;
    let action = managed_action(plan.action).context("managed lifecycle action is unavailable")?;
    let Some(event) = contract.lifecycle_event(action) else {
        return Ok(None);
    };
    let signature = contract
        .lifecycle_event_signature(action)
        .with_context(|| {
            format!(
                "Basecamp {} lifecycle event `{event}` has no declared signature",
                adapter_for(plan.kind).label()
            )
        })?;
    anyhow::ensure!(
        module_transport.native_runtime_module_events_ready(),
        "Basecamp host does not own healthy native lifecycle event ingress"
    );
    require_event(metadata, plan.module, event, signature)?;
    let subscription = module_transport
        .subscribe_module_event(plan.module, event)
        .with_context(|| {
            format!(
                "Basecamp host cannot subscribe to {} {} confirmation",
                adapter_for(plan.kind).label(),
                event
            )
        })?;
    Ok(Some(HostLifecycleSubscription {
        event,
        subscription,
    }))
}

async fn wait_for_host_lifecycle_event(
    plan: &PreparedHostAction,
    mut lifecycle: HostLifecycleSubscription,
    timeout: Duration,
) -> Result<String> {
    let kind = plan.kind;
    let module = plan.module;
    let expected_event = lifecycle.event;
    tokio::task::spawn_blocking(move || {
        let event = lifecycle
            .subscription
            .next_within(timeout)?
            .with_context(|| {
                format!(
                    "Basecamp {} did not emit {} before lifecycle confirmation timeout",
                    adapter_for(kind).label(),
                    expected_event
                )
            })?;
        validate_host_lifecycle_event(kind, module, expected_event, &event)
    })
    .await
    .context("Basecamp lifecycle event worker failed")?
}

fn validate_host_lifecycle_event(
    kind: NodeKind,
    module: &str,
    expected_event: &str,
    event: &ModuleTransportEvent,
) -> Result<String> {
    anyhow::ensure!(
        event.module() == module && event.event() == expected_event,
        "Basecamp {} lifecycle subscription received an unexpected event",
        adapter_for(kind).label()
    );
    let data = event
        .args()
        .iter()
        .enumerate()
        .map(|(index, value)| (format!("arg{index}"), value.clone()))
        .collect();
    let outcome = adapter_for(kind)
        .managed_contract()
        .context("node has no managed module contract")?
        .decode_lifecycle_event(&data)
        .with_context(|| {
            format!(
                "Basecamp {} {} payload is invalid",
                adapter_for(kind).label(),
                expected_event
            )
        })?;
    anyhow::ensure!(
        outcome.success,
        "Basecamp {} {} reported failure{}",
        adapter_for(kind).label(),
        expected_event,
        if outcome.detail.is_empty() {
            String::new()
        } else {
            format!(": {}", outcome.detail)
        }
    );
    Ok(outcome.detail)
}

fn validate_host_action_result(plan: &PreparedHostAction, value: &Value) -> Result<()> {
    if plan.kind == NodeKind::Storage
        && matches!(plan.action, NodeAction::Initialize | NodeAction::Start)
        && value.as_bool() != Some(true)
    {
        bail!(
            "{}.{} did not accept the lifecycle action",
            plan.module,
            plan.call.as_ref().map_or("nodeAction", |call| call.method)
        );
    }
    Ok(())
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
    execution: Result<HostActionExecution>,
    store: &LocalNodeStore,
) -> Result<()> {
    let timestamp = now_millis();
    let (status, detail, succeeded, lifecycle_detail, dispatched_method) = match execution {
        Ok(execution) => {
            let lifecycle_confirmed = execution.lifecycle_detail.is_some();
            let detail = execution.lifecycle_detail.as_deref().map_or_else(
                || {
                    if execution.dispatched_method == STORAGE_NODE_ACTION_METHOD {
                        format!(
                            "Basecamp {} acknowledged V1 {} transition",
                            adapter_for(plan.kind).label(),
                            plan.action.as_str()
                        )
                    } else {
                        format!(
                            "Basecamp host accepted {}.{}",
                            plan.module, execution.dispatched_method
                        )
                    }
                },
                |detail| {
                    if detail.is_empty() {
                        format!(
                            "Basecamp host confirmed {}.{} completion",
                            plan.module, execution.dispatched_method
                        )
                    } else {
                        format!(
                            "Basecamp host confirmed {}.{} completion: {detail}",
                            plan.module, execution.dispatched_method
                        )
                    }
                },
            );
            (
                action_success_status(plan.action, lifecycle_confirmed).to_owned(),
                detail,
                true,
                execution.lifecycle_detail,
                execution.dispatched_method,
            )
        }
        Err(error) => (
            "failed".to_owned(),
            format!("{error:#}"),
            false,
            None,
            plan.call.as_ref().map_or("nodeAction", |call| call.method),
        ),
    };
    if succeeded {
        apply_successful_action(state, profile, plan, lifecycle_detail.as_deref())?;
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
            plan.module, dispatched_method
        )),
    });
    store.save(state)
}

fn action_success_status(action: NodeAction, lifecycle_confirmed: bool) -> &'static str {
    match action {
        NodeAction::Initialize => "initialized",
        NodeAction::Start if lifecycle_confirmed => "running",
        NodeAction::Stop if lifecycle_confirmed => "stopped",
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
    lifecycle_detail: Option<&str>,
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
            if lifecycle_detail.is_some() {
                config.lifecycle_state = NodeLifecycleState::Running;
                config.pending_lifecycle_action = None;
            } else {
                config.lifecycle_state = NodeLifecycleState::Starting;
                config.pending_lifecycle_action = Some(NodeAction::Start);
            }
        }
        NodeAction::Stop => {
            if lifecycle_detail.is_some() {
                config.lifecycle_state = NodeLifecycleState::Stopped;
                config.pending_lifecycle_action = None;
            } else {
                config.lifecycle_state = NodeLifecycleState::Stopping;
                config.pending_lifecycle_action = Some(NodeAction::Stop);
            }
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
    let mut cleared_contexts = Vec::new();
    let mut failed_lifecycle_actions = Vec::new();
    let mut settled_lifecycle_actions = Vec::new();
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
        if (matches!(observation.kind, NodeKind::Storage | NodeKind::Messaging)
            || (observation.kind == NodeKind::Bedrock && observation.liveness_error.is_none()))
            && observation.context_initialized == Some(false)
            && config.installed
        {
            clear_module_context(config);
            cleared_contexts.push(observation.kind);
            changed = true;
            continue;
        }
        if observation.context_initialized == Some(true) && !config.installed {
            config.installed = true;
            config.package_path = adapter_for(observation.kind)
                .managed_contract()
                .map(|contract| contract.module_id().to_owned());
            config.lifecycle_state = match observation.liveness {
                Some(true) => NodeLifecycleState::Running,
                Some(false) => NodeLifecycleState::Stopped,
                None => NodeLifecycleState::Unknown,
            };
            config.pending_lifecycle_action = None;
            changed = true;
        }
        match observation.liveness {
            Some(true) if config.lifecycle_state == NodeLifecycleState::Stopping => {}
            Some(true) => {
                if config.lifecycle_state != NodeLifecycleState::Running
                    || config.pending_lifecycle_action.is_some()
                {
                    let pending_action = config.pending_lifecycle_action;
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
                    if pending_action == Some(NodeAction::Start) {
                        settled_lifecycle_actions.push((observation.kind, NodeAction::Start));
                    }
                    changed = true;
                }
            }
            Some(false) if config.lifecycle_state == NodeLifecycleState::Stopping => {
                let pending_action = config.pending_lifecycle_action;
                config.lifecycle_state = NodeLifecycleState::Stopped;
                config.pending_lifecycle_action = None;
                if pending_action == Some(NodeAction::Stop) {
                    settled_lifecycle_actions.push((observation.kind, NodeAction::Stop));
                }
                changed = true;
            }
            Some(false) if config.lifecycle_state == NodeLifecycleState::Starting => {
                let pending_action = config.pending_lifecycle_action.take();
                if let Some(detail) = observation.liveness_error.clone() {
                    config.lifecycle_state = NodeLifecycleState::Failed;
                    if let Some(action) = pending_action {
                        failed_lifecycle_actions.push((observation.kind, action, detail));
                    }
                } else {
                    config.lifecycle_state = NodeLifecycleState::Stopped;
                }
                changed = true;
            }
            Some(false)
                if observation.kind == NodeKind::Messaging
                    && config.lifecycle_state == NodeLifecycleState::Running =>
            {
                config.lifecycle_state = NodeLifecycleState::Stopped;
                config.pending_lifecycle_action = None;
                changed = true;
            }
            Some(false)
                if matches!(observation.kind, NodeKind::Storage | NodeKind::Messaging)
                    && matches!(
                        config.lifecycle_state,
                        NodeLifecycleState::Running
                            | NodeLifecycleState::Unknown
                            | NodeLifecycleState::Failed
                    ) =>
            {
                config.lifecycle_state = NodeLifecycleState::Stopped;
                config.pending_lifecycle_action = None;
                changed = true;
            }
            Some(false)
                if observation.kind == NodeKind::Bedrock
                    && observation.context_initialized == Some(true)
                    && matches!(
                        config.lifecycle_state,
                        NodeLifecycleState::Running
                            | NodeLifecycleState::Unknown
                            | NodeLifecycleState::Failed
                    ) =>
            {
                config.lifecycle_state = NodeLifecycleState::Stopped;
                config.pending_lifecycle_action = None;
                changed = true;
            }
            Some(false) if config.lifecycle_state == NodeLifecycleState::Running => {
                config.lifecycle_state = NodeLifecycleState::Unknown;
                config.pending_lifecycle_action = None;
                changed = true;
            }
            None if config.lifecycle_state == NodeLifecycleState::Running
                && observation.liveness_error.is_some() =>
            {
                config.lifecycle_state = NodeLifecycleState::Unknown;
                config.pending_lifecycle_action = None;
                changed = true;
            }
            None if config.lifecycle_state.is_pending() && observation.liveness_error.is_some() => {
                let pending_action = config.pending_lifecycle_action.take();
                config.lifecycle_state = NodeLifecycleState::Failed;
                if let (Some(action), Some(detail)) =
                    (pending_action, observation.liveness_error.clone())
                {
                    failed_lifecycle_actions.push((observation.kind, action, detail));
                }
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
    for (kind, action, detail) in failed_lifecycle_actions {
        if let Some(operation) = state.operations.iter_mut().rev().find(|operation| {
            operation.node == Some(kind)
                && operation.action == action
                && operation.status == action_success_status(action, false)
        }) {
            operation.status = "failed".to_owned();
            operation.detail = detail;
        }
    }
    for (kind, action) in settled_lifecycle_actions {
        if let Some(operation) = state.operations.iter_mut().rev().find(|operation| {
            operation.node == Some(kind)
                && operation.action == action
                && operation.status == action_success_status(action, false)
        }) {
            operation.status = action_success_status(action, true).to_owned();
            operation.detail = format!(
                "Basecamp {} {} completed after lifecycle status confirmation",
                adapter_for(kind).label(),
                action.as_str()
            );
        }
    }
    for kind in cleared_contexts {
        state.clear_module_context_topology(kind);
    }
    if let Some(topology_id) = topology_id {
        for observation in observations.iter().filter(|observation| {
            observation.context_initialized == Some(true) || observation.liveness == Some(true)
        }) {
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
    let run_state = if compatible {
        config.lifecycle_state.as_str()
    } else {
        NodeLifecycleState::Unknown.as_str()
    };
    LocalNodeStatus {
        kind: config.kind,
        key: config.kind.as_str().to_owned(),
        label: adapter_for(config.kind).label().to_owned(),
        install_state: install_state.to_owned(),
        run_state: run_state.to_owned(),
        ownership: if observation.module_available {
            "inspector_managed"
        } else {
            "external"
        }
        .to_owned(),
        endpoint: config.endpoint.clone(),
        data_dir: Some(config.data_dir.clone()),
        config_path: Some(config.config_path.clone()),
        initialization_configuration_ready: None,
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
    if let Some(actions) = observation.managed_actions.as_deref() {
        return actions
            .iter()
            .filter_map(|action| match action.as_str() {
                "initialize" => Some(NodeAction::Initialize),
                "start" => Some(NodeAction::Start),
                "stop" => Some(NodeAction::Stop),
                "destroy" => Some(NodeAction::Uninstall),
                _ => None,
            })
            .collect();
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
        NodeLifecycleState::Running | NodeLifecycleState::Unknown => vec![NodeAction::Stop],
        NodeLifecycleState::Failed if observation.liveness == Some(false) => {
            vec![NodeAction::Start]
        }
        NodeLifecycleState::Failed => vec![NodeAction::Stop],
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
        return lifecycle_contract_detail(config.kind, error);
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

fn lifecycle_contract_detail(kind: NodeKind, error: &str) -> String {
    if kind == NodeKind::Storage && error.contains("lifecycleStatus()") {
        return "The installed Storage module cannot report its lifecycle state. Update Storage, then restart Basecamp."
            .to_owned();
    }
    error.to_owned()
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
    use std::{
        fs,
        io::{Read as _, Write as _},
        net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener},
        sync::{
            Arc, Mutex,
            mpsc::{self, Receiver, SyncSender},
        },
        thread,
    };

    use anyhow::{Result, bail};

    use crate::modules::logos_core::{
        BoxedModuleEventSubscription, ModuleCallFuture, ModuleCallReply, ModuleDiagnosticFuture,
        ModuleEventSubscription, ModuleTransport, ModuleTransportEvent, ModuleTransportResult,
    };
    use crate::source_routing::storage::StorageLifecycleState;

    use super::*;

    #[derive(Debug, Clone)]
    struct RecordingHostTransport {
        calls: Arc<Mutex<Vec<ModuleCall>>>,
        event_subscribers: Arc<Mutex<Vec<RecordingEventSubscriber>>>,
        storage_lifecycle_state: Arc<Mutex<StorageLifecycleState>>,
        bedrock_node_stopped: bool,
        reject_storage_initialize: bool,
        storage_lifecycle_status_available: bool,
        storage_node_lifecycle_v1_available: bool,
        emits_lifecycle_events: bool,
        lifecycle_event_success: bool,
        delivery_node_stopped_signature: Option<&'static str>,
        native_events_ready: bool,
    }

    #[derive(Debug)]
    struct RecordingEventSubscriber {
        module: String,
        event: String,
        sender: SyncSender<ModuleTransportEvent>,
    }

    struct RecordingEventSubscription {
        receiver: Receiver<ModuleTransportEvent>,
    }

    impl ModuleEventSubscription for RecordingEventSubscription {
        fn next_within(&mut self, timeout: Duration) -> Result<Option<ModuleTransportEvent>> {
            match self.receiver.recv_timeout(timeout) {
                Ok(event) => Ok(Some(event)),
                Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    bail!("recording lifecycle event subscription disconnected")
                }
            }
        }
    }

    impl RecordingHostTransport {
        fn new() -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                event_subscribers: Arc::new(Mutex::new(Vec::new())),
                storage_lifecycle_state: Arc::new(Mutex::new(
                    StorageLifecycleState::NotInitialized,
                )),
                bedrock_node_stopped: false,
                reject_storage_initialize: false,
                storage_lifecycle_status_available: true,
                storage_node_lifecycle_v1_available: false,
                emits_lifecycle_events: true,
                lifecycle_event_success: true,
                delivery_node_stopped_signature: Some("nodeStopped(bool,QString,int)"),
                native_events_ready: true,
            }
        }

        fn rejecting_storage_initialize() -> Self {
            Self {
                reject_storage_initialize: true,
                ..Self::new()
            }
        }

        fn without_storage_lifecycle_status() -> Self {
            Self {
                storage_lifecycle_status_available: false,
                ..Self::new()
            }
        }

        fn with_storage_node_lifecycle_v1() -> Self {
            Self {
                storage_node_lifecycle_v1_available: true,
                ..Self::new()
            }
        }

        fn with_stopped_bedrock() -> Self {
            Self {
                bedrock_node_stopped: true,
                ..Self::new()
            }
        }

        fn lifecycle_failure() -> Self {
            Self {
                lifecycle_event_success: false,
                ..Self::new()
            }
        }

        fn without_lifecycle_events() -> Self {
            Self {
                emits_lifecycle_events: false,
                ..Self::new()
            }
        }

        fn with_mismatched_stop_event_signature() -> Self {
            Self {
                delivery_node_stopped_signature: Some("nodeStopped(bool,QString)"),
                ..Self::new()
            }
        }

        fn without_stop_event_metadata() -> Self {
            Self {
                delivery_node_stopped_signature: None,
                ..Self::new()
            }
        }

        fn calls(&self) -> Result<Vec<ModuleCall>> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|_| anyhow::anyhow!("recording call lock is poisoned"))
        }

        fn storage_lifecycle_status(&self) -> Result<ModuleCallReply> {
            let state = *self
                .storage_lifecycle_state
                .lock()
                .map_err(|_| anyhow::anyhow!("recording Storage lifecycle lock is poisoned"))?;
            Ok(ModuleCallReply::new(
                ModuleTransportKind::Module,
                json!({
                    "initialized": state != StorageLifecycleState::NotInitialized,
                    "running": state == StorageLifecycleState::Running,
                    "state": state.as_str(),
                }),
            ))
        }

        fn storage_node_lifecycle_snapshot(&self) -> Result<ModuleCallReply> {
            let state = *self
                .storage_lifecycle_state
                .lock()
                .map_err(|_| anyhow::anyhow!("recording Storage lifecycle lock is poisoned"))?;
            let state = match state {
                StorageLifecycleState::NotInitialized => "uninitialized",
                StorageLifecycleState::Stopped => "stopped",
                StorageLifecycleState::Starting => "starting",
                StorageLifecycleState::Running => "running",
                StorageLifecycleState::Stopping => "stopping",
            };
            Ok(ModuleCallReply::new(
                ModuleTransportKind::Module,
                json!({
                    "schema": "logos.managed_node_lifecycle.snapshot",
                    "version": 1,
                    "instance_id": "recording-storage-instance",
                    "epoch": 1,
                    "sequence": 1,
                    "scope": { "kind": "storage" },
                    "state": state,
                    "last_error": null,
                }),
            ))
        }

        fn set_storage_lifecycle_state(&self, state: StorageLifecycleState) -> Result<()> {
            *self
                .storage_lifecycle_state
                .lock()
                .map_err(|_| anyhow::anyhow!("recording Storage lifecycle lock is poisoned"))? =
                state;
            Ok(())
        }

        fn publish_lifecycle_event(&self, module: &str, event: &str) {
            if !self.emits_lifecycle_events {
                return;
            }
            let arguments = if module == "storage_module" {
                vec![Value::String(
                    json!({
                        "success": self.lifecycle_event_success,
                        "message": "recorded terminal lifecycle event",
                    })
                    .to_string(),
                )]
            } else {
                vec![
                    Value::Bool(self.lifecycle_event_success),
                    Value::String("recorded terminal lifecycle event".to_owned()),
                ]
            };
            let Ok(event) = ModuleTransportEvent::new(module, event, arguments) else {
                return;
            };
            let Ok(mut subscribers) = self.event_subscribers.lock() else {
                return;
            };
            subscribers.retain(|subscriber| {
                if subscriber.module != module || subscriber.event != event.event() {
                    return true;
                }
                !matches!(
                    subscriber.sender.try_send(event.clone()),
                    Err(mpsc::TrySendError::Disconnected(_))
                )
            });
        }
    }

    impl ModuleTransport for RecordingHostTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let reject_storage_initialize = self.reject_storage_initialize;
            let bedrock_node_stopped = self.bedrock_node_stopped;
            let storage_node_lifecycle_v1_available = self.storage_node_lifecycle_v1_available;
            let mut storage_lifecycle_state = None;
            let lifecycle_event = match (call.module(), call.method()) {
                ("delivery_module", "start") => Some("nodeStarted"),
                ("delivery_module", "stop") => Some("nodeStopped"),
                ("storage_module", "start") => Some("storageStart"),
                ("storage_module", "stop") => Some("storageStop"),
                _ => None,
            };
            let result = self
                .calls
                .lock()
                .map_err(|_| anyhow::anyhow!("recording call lock is poisoned"))
                .and_then(|mut calls| {
                    calls.push(call.clone());
                    match (call.module(), call.method()) {
                        ("blockchain_module", "get_cryptarchia_info")
                            if bedrock_node_stopped =>
                        {
                            bail!(
                                "blockchain_module.get_cryptarchia_info failed: The node is not running."
                            )
                        }
                        ("storage_module", "init") if reject_storage_initialize => Ok(
                            ModuleCallReply::new(ModuleTransportKind::Module, json!(false)),
                        ),
                        ("storage_module", "init") => {
                            storage_lifecycle_state = Some(StorageLifecycleState::Stopped);
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!(true),
                            ))
                        }
                        ("storage_module", "start") => {
                            storage_lifecycle_state = Some(StorageLifecycleState::Running);
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!(true),
                            ))
                        }
                        ("storage_module", "stop") => {
                            storage_lifecycle_state = Some(StorageLifecycleState::Stopped);
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!(true),
                            ))
                        }
                        ("storage_module", "destroy") => {
                            storage_lifecycle_state = Some(StorageLifecycleState::NotInitialized);
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!(true),
                            ))
                        }
                        ("storage_module", "lifecycleStatus") => self.storage_lifecycle_status(),
                        ("storage_module", "nodeStatus") if storage_node_lifecycle_v1_available => {
                            self.storage_node_lifecycle_snapshot()
                        }
                        ("storage_module", "nodeAction") if storage_node_lifecycle_v1_available => {
                            let request = call
                                .args()
                                .first()
                                .and_then(Value::as_str)
                                .context("recording Storage nodeAction request is missing")
                                .and_then(|request| {
                                    serde_json::from_str::<Value>(request)
                                        .context("recording Storage nodeAction request is invalid")
                                })?;
                            let operation_id = request
                                .get("operation_id")
                                .and_then(Value::as_str)
                                .context("recording Storage nodeAction has no operation_id")?;
                            let action = request
                                .get("action")
                                .and_then(Value::as_str)
                                .context("recording Storage nodeAction has no action")?;
                            storage_lifecycle_state = Some(match action {
                                "start" => StorageLifecycleState::Starting,
                                "stop" => StorageLifecycleState::Stopping,
                                _ => bail!("recording Storage nodeAction has unsupported action `{action}`"),
                            });
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!({
                                    "schema": "logos.managed_node_lifecycle.ack",
                                    "version": 1,
                                    "operation_id": operation_id,
                                    "accepted": true,
                                    "duplicate": false,
                                    "instance_id": "recording-storage-instance",
                                    "epoch": 1,
                                    "sequence": 2,
                                    "state": if action == "start" {
                                        "starting"
                                    } else {
                                        "stopping"
                                    },
                                    "error": null,
                                }),
                            ))
                        }
                        ("blockchain_module", "get_cryptarchia_info")
                            if calls.iter().any(|candidate| {
                                candidate.module() == "blockchain_module"
                                    && candidate.method() == "start"
                            }) =>
                        {
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!({"tip": 1}),
                            ))
                        }
                        ("storage_module", "space")
                            if !reject_storage_initialize
                                && calls.iter().any(|candidate| {
                                    candidate.module() == "storage_module"
                                        && candidate.method() == "init"
                                }) =>
                        {
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!({"total": 1, "used": 0}),
                            ))
                        }
                        ("delivery_module", "getNodeInfo")
                            if calls.iter().any(|candidate| {
                                candidate.module() == "delivery_module"
                                    && candidate.method() == "createNode"
                            }) =>
                        {
                            Ok(ModuleCallReply::new(
                                ModuleTransportKind::Module,
                                json!("peer-id"),
                            ))
                        }
                        ("blockchain_module", "get_cryptarchia_info")
                        | ("storage_module", "space")
                        | ("delivery_module", "getNodeInfo") => {
                            bail!("node context is not initialized")
                        }
                        _ => Ok(ModuleCallReply::new(
                            ModuleTransportKind::Module,
                            json!(true),
                        )),
                    }
                })
                .and_then(|reply| {
                    if let Some(state) = storage_lifecycle_state {
                        self.set_storage_lifecycle_state(state)?;
                    }
                    Ok(reply)
                });
            if result.is_ok()
                && let Some(event) = lifecycle_event
            {
                self.publish_lifecycle_event(call.module(), event);
            }
            Box::pin(async move { result })
        }

        fn subscribe_module_event(
            &self,
            module: &str,
            event: &str,
        ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
            let (sender, receiver) = mpsc::sync_channel(1);
            self.event_subscribers
                .lock()
                .map_err(|_| anyhow::anyhow!("recording lifecycle event subscribers unavailable"))?
                .push(RecordingEventSubscriber {
                    module: module.to_owned(),
                    event: event.to_owned(),
                    sender,
                });
            Ok(Box::new(RecordingEventSubscription { receiver }))
        }

        fn native_runtime_module_events_ready(&self) -> bool {
            self.native_events_ready
        }

        fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
            let mut metadata = module_metadata(
                &module,
                self.delivery_node_stopped_signature,
                self.storage_node_lifecycle_v1_available,
            );
            if module == "storage_module"
                && !self.storage_lifecycle_status_available
                && let Some(methods) = metadata.get_mut("methods").and_then(Value::as_array_mut)
            {
                methods.retain(|method| {
                    method.get("name").and_then(Value::as_str) != Some("lifecycleStatus")
                });
            }
            Box::pin(async move { Ok(metadata) })
        }
    }

    #[derive(Debug, Clone, Copy)]
    struct RecordingManagedNodeLifecycleState {
        state: ManagedNodeLifecycleState,
        epoch: u64,
        sequence: u64,
    }

    #[derive(Debug, Clone)]
    struct RecordingManagedNodeLifecycleTransport {
        calls: Arc<Mutex<Vec<ModuleCall>>>,
        event_subscribers: Arc<Mutex<Vec<RecordingEventSubscriber>>>,
        state: Arc<Mutex<RecordingManagedNodeLifecycleState>>,
        native_events_ready: bool,
        event_metadata_available: bool,
        supports_destroy: bool,
        module: &'static str,
        scope: &'static str,
        instance_id: &'static str,
        label: &'static str,
    }

    impl RecordingManagedNodeLifecycleTransport {
        fn new(native_events_ready: bool) -> Self {
            Self::with_identity(
                native_events_ready,
                "blockchain_module",
                "bedrock",
                "recording-bedrock-instance",
                "Bedrock",
                false,
            )
        }

        fn messaging(native_events_ready: bool) -> Self {
            Self::with_identity(
                native_events_ready,
                "delivery_module",
                "messaging",
                "recording-messaging-instance",
                "Messaging",
                true,
            )
        }

        fn with_identity(
            native_events_ready: bool,
            module: &'static str,
            scope: &'static str,
            instance_id: &'static str,
            label: &'static str,
            supports_destroy: bool,
        ) -> Self {
            Self {
                calls: Arc::new(Mutex::new(Vec::new())),
                event_subscribers: Arc::new(Mutex::new(Vec::new())),
                state: Arc::new(Mutex::new(RecordingManagedNodeLifecycleState {
                    state: ManagedNodeLifecycleState::Uninitialized,
                    epoch: 0,
                    sequence: 0,
                })),
                native_events_ready,
                event_metadata_available: true,
                supports_destroy,
                module,
                scope,
                instance_id,
                label,
            }
        }

        fn messaging_without_event_metadata(native_events_ready: bool) -> Self {
            let mut transport = Self::messaging(native_events_ready);
            transport.event_metadata_available = false;
            transport
        }

        fn calls(&self) -> Result<Vec<ModuleCall>> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|_| anyhow::anyhow!("recording {} call lock is poisoned", self.label))
        }

        fn set_state(&self, state: ManagedNodeLifecycleState) -> Result<()> {
            self.state
                .lock()
                .map_err(|_| {
                    anyhow::anyhow!("recording {} lifecycle lock is poisoned", self.label)
                })?
                .state = state;
            Ok(())
        }

        fn snapshot_value(&self, state: RecordingManagedNodeLifecycleState) -> Value {
            json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": self.instance_id,
                "epoch": state.epoch,
                "sequence": state.sequence,
                "scope": { "kind": self.scope },
                "state": state.state.as_str(),
                "health": if state.state == ManagedNodeLifecycleState::Running {
                    "healthy"
                } else {
                    "unknown"
                },
                "supported_actions": self.supported_actions(state.state),
                "pending_operation": null,
                "last_completed_operation": null,
                "last_error": null,
                "updated_at_ms": 1,
            })
        }

        fn supported_actions(&self, state: ManagedNodeLifecycleState) -> &'static [&'static str] {
            if self.supports_destroy && state == ManagedNodeLifecycleState::Stopped {
                &["start", "destroy"]
            } else {
                state.expected_actions()
            }
        }

        fn lifecycle_event(
            &self,
            action: &str,
            operation_id: &str,
            phase: &str,
            outcome: &str,
            previous_state: ManagedNodeLifecycleState,
            state: RecordingManagedNodeLifecycleState,
        ) -> Value {
            json!({
                "schema": "logos.managed_node_lifecycle.event",
                "version": 1,
                "instance_id": self.instance_id,
                "epoch": state.epoch,
                "sequence": state.sequence,
                "scope": { "kind": self.scope },
                "operation_id": operation_id,
                "action": action,
                "phase": phase,
                "outcome": outcome,
                "previous_state": previous_state.as_str(),
                "status": self.snapshot_value(state),
                "error": null,
                "emitted_at_ms": 1,
            })
        }

        fn publish_node_changed(&self, event: Value) {
            let Ok(event) = ModuleTransportEvent::new(
                self.module,
                "nodeChanged",
                vec![Value::String(event.to_string())],
            ) else {
                return;
            };
            let Ok(mut subscribers) = self.event_subscribers.lock() else {
                return;
            };
            subscribers.retain(|subscriber| {
                if subscriber.module != self.module || subscriber.event != "nodeChanged" {
                    return true;
                }
                !matches!(
                    subscriber.sender.try_send(event.clone()),
                    Err(mpsc::TrySendError::Disconnected(_))
                )
            });
        }

        fn node_status(&self) -> Result<ModuleCallReply> {
            let state = *self.state.lock().map_err(|_| {
                anyhow::anyhow!("recording {} lifecycle lock is poisoned", self.label)
            })?;
            Ok(ModuleCallReply::new(
                ModuleTransportKind::Module,
                Value::String(self.snapshot_value(state).to_string()),
            ))
        }

        fn node_action(&self, call: &ModuleCall) -> Result<ModuleCallReply> {
            let request = call
                .args()
                .first()
                .and_then(Value::as_str)
                .with_context(|| {
                    format!("recording {} nodeAction request is missing", self.label)
                })?;
            let request: Value = serde_json::from_str(request).with_context(|| {
                format!("recording {} nodeAction request is invalid", self.label)
            })?;
            let operation_id = request
                .get("operation_id")
                .and_then(Value::as_str)
                .with_context(|| {
                    format!("recording {} nodeAction has no operation_id", self.label)
                })?;
            let action = request
                .get("action")
                .and_then(Value::as_str)
                .with_context(|| format!("recording {} nodeAction has no action", self.label))?;
            let expected = request
                .get("expected")
                .and_then(Value::as_object)
                .with_context(|| {
                    format!("recording {} nodeAction has no expected cursor", self.label)
                })?;
            let mut state = self.state.lock().map_err(|_| {
                anyhow::anyhow!("recording {} lifecycle lock is poisoned", self.label)
            })?;
            anyhow::ensure!(
                expected.get("instance_id").and_then(Value::as_str) == Some(self.instance_id)
                    && expected.get("epoch").and_then(Value::as_u64) == Some(state.epoch)
                    && expected.get("sequence").and_then(Value::as_u64) == Some(state.sequence),
                "recording {} nodeAction received a stale expected cursor",
                self.label
            );
            let previous_state = state.state;
            let (transition, settled) = match action {
                "initialize" if state.state == ManagedNodeLifecycleState::Uninitialized => {
                    let config = request
                        .get("parameters")
                        .and_then(|parameters| parameters.get("config"))
                        .and_then(Value::as_str)
                        .with_context(|| {
                            format!("recording {} initialize has no config", self.label)
                        })?;
                    anyhow::ensure!(
                        serde_json::from_str::<Value>(config)
                            .map(|value| value.is_object())
                            .unwrap_or(false),
                        "recording {} initialize config is invalid",
                        self.label
                    );
                    (
                        ManagedNodeLifecycleState::Initializing,
                        ManagedNodeLifecycleState::Stopped,
                    )
                }
                "start" if state.state == ManagedNodeLifecycleState::Stopped => {
                    anyhow::ensure!(
                        request
                            .get("parameters")
                            .is_some_and(|parameters| parameters.is_object()),
                        "recording {} start parameters are invalid",
                        self.label
                    );
                    (
                        ManagedNodeLifecycleState::Starting,
                        ManagedNodeLifecycleState::Running,
                    )
                }
                "stop" if state.state == ManagedNodeLifecycleState::Running => {
                    anyhow::ensure!(
                        request
                            .get("parameters")
                            .is_some_and(|parameters| parameters
                                .as_object()
                                .is_some_and(|object| object.is_empty())),
                        "recording {} stop parameters are invalid",
                        self.label
                    );
                    (
                        ManagedNodeLifecycleState::Stopping,
                        ManagedNodeLifecycleState::Stopped,
                    )
                }
                "destroy"
                    if self.supports_destroy
                        && state.state == ManagedNodeLifecycleState::Stopped =>
                {
                    anyhow::ensure!(
                        request
                            .get("parameters")
                            .is_some_and(|parameters| parameters
                                .as_object()
                                .is_some_and(|object| object.is_empty())),
                        "recording {} destroy parameters are invalid",
                        self.label
                    );
                    (
                        ManagedNodeLifecycleState::Destroying,
                        ManagedNodeLifecycleState::Uninitialized,
                    )
                }
                _ => bail!(
                    "recording {} nodeAction has invalid action or state",
                    self.label
                ),
            };
            state.state = transition;
            state.sequence += 1;
            let acknowledgement = json!({
                "schema": "logos.managed_node_lifecycle.ack",
                "version": 1,
                "operation_id": operation_id,
                "accepted": true,
                "duplicate": false,
                "instance_id": self.instance_id,
                "epoch": state.epoch,
                "sequence": state.sequence,
                "state": transition.as_str(),
                "error": null,
            });
            let accepted = self.lifecycle_event(
                action,
                operation_id,
                "accepted",
                "accepted",
                previous_state,
                *state,
            );
            state.state = settled;
            if action == "initialize" {
                state.epoch += 1;
            }
            state.sequence += 1;
            let settled = self.lifecycle_event(
                action,
                operation_id,
                "settled",
                "succeeded",
                previous_state,
                *state,
            );
            drop(state);
            self.publish_node_changed(accepted);
            self.publish_node_changed(settled);
            Ok(ModuleCallReply::new(
                ModuleTransportKind::Module,
                Value::String(acknowledgement.to_string()),
            ))
        }
    }

    impl ModuleTransport for RecordingManagedNodeLifecycleTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let result = self
                .calls
                .lock()
                .map_err(|_| anyhow::anyhow!("recording {} call lock is poisoned", self.label))
                .and_then(|mut calls| {
                    calls.push(call.clone());
                    match call.method() {
                        "nodeStatus" if call.module() == self.module => self.node_status(),
                        "nodeAction" if call.module() == self.module => self.node_action(&call),
                        "get_cryptarchia_info" if self.module == "blockchain_module" => {
                            bail!(
                                "recording {} V1 transport must not use legacy liveness",
                                self.label
                            )
                        }
                        "getNodeInfo" if self.module == "delivery_module" => {
                            bail!(
                                "recording {} V1 transport must not use legacy liveness",
                                self.label
                            )
                        }
                        _ => bail!(
                            "recording {} V1 transport received an unexpected call",
                            self.label
                        ),
                    }
                });
            Box::pin(async move { result })
        }

        fn subscribe_module_event(
            &self,
            module: &str,
            event: &str,
        ) -> ModuleTransportResult<BoxedModuleEventSubscription> {
            anyhow::ensure!(
                module == self.module && event == "nodeChanged",
                "recording {} V1 transport received an unexpected event subscription",
                self.label
            );
            let (sender, receiver) = mpsc::sync_channel(8);
            self.event_subscribers
                .lock()
                .map_err(|_| {
                    anyhow::anyhow!("recording Bedrock event subscriber lock is poisoned")
                })?
                .push(RecordingEventSubscriber {
                    module: module.to_owned(),
                    event: event.to_owned(),
                    sender,
                });
            Ok(Box::new(RecordingEventSubscription { receiver }))
        }

        fn native_runtime_module_events_ready(&self) -> bool {
            self.native_events_ready
        }

        fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
            let expected_module = self.module;
            let label = self.label;
            let event_metadata_available = self.event_metadata_available;
            Box::pin(async move {
                if module != expected_module {
                    bail!("recording {label} V1 transport does not host `{module}`")
                }
                Ok(json!({
                    "name": expected_module,
                    "methods": [
                        { "name": "nodeStatus", "signature": "nodeStatus()", "isInvokable": true },
                        { "name": "nodeAction", "signature": "nodeAction(QString)", "isInvokable": true }
                    ],
                    "events": if event_metadata_available {
                        json!([
                            { "name": "nodeChanged", "signature": "nodeChanged(QString)" }
                        ])
                    } else {
                        json!([])
                    }
                }))
            })
        }
    }

    fn module_metadata(
        module: &str,
        delivery_node_stopped_signature: Option<&str>,
        storage_node_lifecycle_v1_available: bool,
    ) -> Value {
        let mut methods = match module {
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
                json!({"name":"lifecycleStatus","signature":"lifecycleStatus()","isInvokable":true}),
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
        if module == "storage_module" && storage_node_lifecycle_v1_available {
            methods.extend([
                json!({"name":"nodeStatus","signature":"nodeStatus()","isInvokable":true}),
                json!({"name":"nodeAction","signature":"nodeAction(QString)","isInvokable":true}),
            ]);
        }
        let mut events = match module {
            "storage_module" => vec![
                json!({"name":"storageStart","signature":"storageStart(QString)"}),
                json!({"name":"storageStop","signature":"storageStop(QString)"}),
            ],
            "delivery_module" => {
                let mut events =
                    vec![json!({"name":"nodeStarted","signature":"nodeStarted(bool,QString,int)"})];
                if let Some(signature) = delivery_node_stopped_signature {
                    events.push(json!({"name":"nodeStopped","signature":signature}));
                }
                events
            }
            _ => Vec::new(),
        };
        if module == "storage_module" && storage_node_lifecycle_v1_available {
            events.push(json!({"name":"nodeChanged","signature":"nodeChanged(QString)"}));
        }
        json!({"name":module,"methods":methods,"events":events})
    }

    #[test]
    fn basecamp_storage_requires_authoritative_lifecycle_status() -> Result<()> {
        let mut metadata = module_metadata(
            "storage_module",
            Some("nodeStopped(bool,QString,int)"),
            false,
        );
        metadata
            .get_mut("methods")
            .and_then(Value::as_array_mut)
            .context("Storage metadata has no method list")?
            .retain(|method| method.get("name").and_then(Value::as_str) != Some("lifecycleStatus"));

        let error = validate_lifecycle_contract(NodeKind::Storage, &metadata)
            .err()
            .context("Storage contract accepted a module without lifecycleStatus()")?;
        anyhow::ensure!(error.to_string().contains("lifecycleStatus()"));
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_storage_without_lifecycle_status_reports_incompatible_module() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::without_storage_lifecycle_status();
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = status_with_store("default", &transport, &store).await?;
        let storage = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted Storage")?;
        if storage.install_state != "needs_configuration"
            || storage.run_state != "unknown"
            || !storage.available_actions.is_empty()
            || storage.detail
                != "The installed Storage module cannot report its lifecycle state. Update Storage, then restart Basecamp."
        {
            bail!("incompatible Storage lifecycle API was projected misleadingly: {report:?}");
        }
        if transport_impl
            .calls()?
            .iter()
            .any(|call| call.module() == "storage_module" && call.method() == "lifecycleStatus")
        {
            bail!("incompatible Storage metadata still dispatched lifecycleStatus()");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_storage_incompatible_lifecycle_contract_blocks_initialize_dispatch()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::without_storage_lifecycle_status();
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = action_with_store(
            "default",
            initialize_request(NodeKind::Storage),
            &transport,
            &store,
        )
        .await?;
        let storage = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted Storage")?;
        let operation = report
            .operations
            .last()
            .context("Basecamp Storage Initialize operation was not recorded")?;
        if storage.run_state != "unknown"
            || !storage.available_actions.is_empty()
            || operation.status != "failed"
            || !operation.detail.contains("lifecycleStatus()")
        {
            bail!("incompatible Storage API accepted Initialize: {report:?}");
        }
        if transport_impl
            .calls()?
            .iter()
            .any(|call| call.module() == "storage_module" && call.method() == "init")
        {
            bail!("incompatible Storage API dispatched storage_module.init");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_storage_lifecycle_status_exposes_initialize_for_fresh_context() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport: SharedModuleTransport = Arc::new(RecordingHostTransport::new());

        let report = status_with_store("default", &transport, &store).await?;
        let storage = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted Storage")?;
        if storage.install_state != "needs_configuration"
            || storage.run_state != "not_initialized"
            || storage.available_actions != vec![NodeAction::Initialize]
        {
            bail!("compatible Storage lifecycle API did not expose Initialize: {report:?}");
        }
        Ok(())
    }

    fn initialize_request(kind: NodeKind) -> LocalNodeActionRequest {
        node_action_request(kind, NodeAction::Initialize)
    }

    fn node_action_request(kind: NodeKind, action: NodeAction) -> LocalNodeActionRequest {
        LocalNodeActionRequest {
            action,
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

    fn set_node_config_port(
        store: &LocalNodeStore,
        kind: NodeKind,
        key: &str,
        port: u16,
    ) -> Result<()> {
        let state = store.load()?;
        let path = state
            .active_topology("default")
            .and_then(|topology| topology.nodes.iter().find(|node| node.kind == kind))
            .map(|node| node.config_path.clone())
            .context("default topology omitted node config")?;
        let mut config = serde_json::from_str::<Value>(&fs::read_to_string(&path)?)?;
        config
            .as_object_mut()
            .context("default node config is not a JSON object")?
            .insert(key.to_owned(), json!(port));
        fs::write(path, serde_json::to_vec_pretty(&config)?)?;
        Ok(())
    }

    fn messaging_health_server(
        health: &'static str,
    ) -> Result<(SocketAddr, thread::JoinHandle<std::io::Result<()>>)> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let worker = thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request)?;
            stream.write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"nodeHealth\":\"{health}\"}}"
                )
                .as_bytes(),
            )
        });
        Ok((address, worker))
    }

    fn unresponsive_messaging_listener()
    -> Result<(SocketAddr, thread::JoinHandle<std::io::Result<()>>)> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let worker = thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request)?;
            thread::sleep(Duration::from_secs(2));
            Ok(())
        });
        Ok((address, worker))
    }

    fn configure_messaging_health(
        store: &LocalNodeStore,
        address: SocketAddr,
        lifecycle_state: NodeLifecycleState,
        pending_lifecycle_action: Option<NodeAction>,
    ) -> Result<LocalNodeConfigRecord> {
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
        fs::write(
            &messaging.config_path,
            serde_json::to_vec(&json!({
                "restAddress": address.ip().to_string(),
                "restPort": address.port(),
            }))?,
        )?;
        messaging.installed = true;
        messaging.lifecycle_state = lifecycle_state;
        messaging.pending_lifecycle_action = pending_lifecycle_action;
        let config = messaging.clone();
        store.save(&state)?;
        Ok(config)
    }

    #[tokio::test]
    async fn basecamp_initialize_dispatches_config_contents_through_host_module() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::new();
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());
        set_node_config_port(&store, NodeKind::Messaging, "restPort", 0)?;

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
    async fn rejected_storage_initialize_is_recorded_without_installing_context() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport: SharedModuleTransport =
            Arc::new(RecordingHostTransport::rejecting_storage_initialize());

        let report = action_with_store(
            "default",
            initialize_request(NodeKind::Storage),
            &transport,
            &store,
        )
        .await?;
        let storage = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted Storage")?;
        if storage.install_state != "needs_configuration"
            || storage.run_state != "not_initialized"
            || report
                .operations
                .last()
                .is_none_or(|operation| operation.status != "failed")
        {
            bail!("rejected Storage initialize mutated lifecycle state: {report:?}");
        }
        Ok(())
    }

    #[test]
    fn service_addresses_use_loopback_for_unspecified_bind_addresses() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let storage_path = directory.path().join("storage.json");
        fs::write(
            &storage_path,
            serde_json::to_vec(&json!({"listen-ip":"0.0.0.0","listen-port":8091}))?,
        )?;
        let messaging_path = directory.path().join("messaging.json");
        fs::write(
            &messaging_path,
            serde_json::to_vec(&json!({"restAddress":"127.0.0.1","restPort":8645}))?,
        )?;

        if service_address(NodeKind::Storage, storage_path.to_str().unwrap_or_default())?
            != SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8091)
            || service_address(
                NodeKind::Messaging,
                messaging_path.to_str().unwrap_or_default(),
            )? != SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 8645)
        {
            bail!("Basecamp service address did not normalize to loopback");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_initializing_health_listener_stays_stopped() -> Result<()> {
        let (address, worker) = messaging_health_server("INITIALIZING")?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let config =
            configure_messaging_health(&store, address, NodeLifecycleState::Running, None)?;
        let liveness = service_liveness(NodeKind::Messaging, Some(&config)).await?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("Messaging health test server panicked"))??;
        if liveness.observed != Some(false) || liveness.detail.is_some() {
            bail!("INITIALIZING Messaging health was treated as running: {liveness:?}");
        }

        let mut state = store.load()?;
        let observations = [HostNodeObservation {
            kind: NodeKind::Messaging,
            module_available: true,
            contract_error: None,
            context_initialized: Some(true),
            liveness: liveness.observed,
            liveness_error: liveness.detail,
            managed_actions: None,
        }];
        reconcile_observations(&mut state, "default", &observations, &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Stopped
            || host_available_actions(messaging, &observations[0]) != vec![NodeAction::Start]
        {
            bail!("INITIALIZING Messaging health changed state: {messaging:?}");
        }
        Ok(())
    }

    #[test]
    fn basecamp_messaging_inconclusive_health_does_not_claim_running() -> Result<()> {
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
        messaging.lifecycle_state = NodeLifecycleState::Running;
        let observations = [HostNodeObservation {
            kind: NodeKind::Messaging,
            module_available: true,
            contract_error: None,
            context_initialized: Some(true),
            liveness: None,
            liveness_error: Some("Basecamp Messaging REST health is NOT_READY".to_owned()),
            managed_actions: None,
        }];
        reconcile_observations(&mut state, "default", &observations, &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Unknown
            || host_node_detail(messaging, &observations[0])
                != "Basecamp Messaging REST health is NOT_READY"
        {
            bail!("inconclusive Messaging health still claimed running: {messaging:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_start_timeout_is_failed_and_retryable() -> Result<()> {
        let (address, worker) = messaging_health_server("INITIALIZING")?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let config = configure_messaging_health(
            &store,
            address,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        )?;
        let liveness =
            service_liveness_with_timeout(NodeKind::Messaging, Some(&config), Duration::ZERO)
                .await?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("Messaging health test server panicked"))??;
        if liveness.observed != Some(false)
            || !liveness
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("did not reach running"))
        {
            bail!("INITIALIZING start timeout was not recorded: {liveness:?}");
        }

        let mut state = store.load()?;
        state.push_operation(LocalNodeOperationReport {
            id: "start-messaging".to_owned(),
            time: "1".to_owned(),
            timestamp_millis: 1,
            action: NodeAction::Start,
            node: Some(NodeKind::Messaging),
            network_id: None,
            status: "starting".to_owned(),
            detail: "host accepted start".to_owned(),
            command: None,
        });
        let observations = [HostNodeObservation {
            kind: NodeKind::Messaging,
            module_available: true,
            contract_error: None,
            context_initialized: Some(true),
            liveness: liveness.observed,
            liveness_error: liveness.detail,
            managed_actions: None,
        }];
        reconcile_observations(&mut state, "default", &observations, &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Failed
            || messaging.pending_lifecycle_action.is_some()
            || host_available_actions(messaging, &observations[0]) != vec![NodeAction::Start]
            || state
                .operations
                .last()
                .is_none_or(|operation| operation.status != "failed")
        {
            bail!("Messaging start timeout left an unusable lifecycle state: {state:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_stop_waits_for_health_listener_to_close() -> Result<()> {
        let (address, worker) = messaging_health_server("SHUTTING_DOWN")?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let config = configure_messaging_health(
            &store,
            address,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        )?;
        let liveness = service_liveness_with_timeout(
            NodeKind::Messaging,
            Some(&config),
            Duration::from_secs(1),
        )
        .await?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("Messaging health test server panicked"))??;
        if liveness.observed != Some(false) || liveness.detail.is_some() {
            bail!("SHUTTING_DOWN did not wait for REST listener close: {liveness:?}");
        }

        let mut state = store.load()?;
        let observations = [HostNodeObservation {
            kind: NodeKind::Messaging,
            module_available: true,
            contract_error: None,
            context_initialized: Some(true),
            liveness: liveness.observed,
            liveness_error: liveness.detail,
            managed_actions: None,
        }];
        reconcile_observations(&mut state, "default", &observations, &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Stopped
            || messaging.pending_lifecycle_action.is_some()
        {
            bail!("Messaging stop did not settle after REST listener close: {messaging:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_stop_timeout_is_failed() -> Result<()> {
        let (address, worker) = messaging_health_server("SHUTTING_DOWN")?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let config = configure_messaging_health(
            &store,
            address,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        )?;
        let liveness =
            service_liveness_with_timeout(NodeKind::Messaging, Some(&config), Duration::ZERO)
                .await?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("Messaging health test server panicked"))??;
        if liveness.observed.is_some()
            || !liveness
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("SHUTTING_DOWN"))
        {
            bail!("SHUTTING_DOWN stop timeout was not recorded: {liveness:?}");
        }

        let mut state = store.load()?;
        let observations = [HostNodeObservation {
            kind: NodeKind::Messaging,
            module_available: true,
            contract_error: None,
            context_initialized: Some(true),
            liveness: liveness.observed,
            liveness_error: liveness.detail,
            managed_actions: None,
        }];
        reconcile_observations(&mut state, "default", &observations, &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Failed
            || messaging.pending_lifecycle_action.is_some()
            || host_available_actions(messaging, &observations[0]) != vec![NodeAction::Stop]
        {
            bail!("Messaging stop timeout left an unusable lifecycle state: {messaging:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_unresponsive_listener_never_confirms_stop() -> Result<()> {
        let (address, worker) = unresponsive_messaging_listener()?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let config = configure_messaging_health(
            &store,
            address,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        )?;
        let liveness =
            service_liveness_with_timeout(NodeKind::Messaging, Some(&config), Duration::ZERO)
                .await?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("unresponsive Messaging listener panicked"))??;
        if liveness.observed.is_some()
            || !liveness
                .detail
                .as_deref()
                .is_some_and(|detail| detail.contains("unresponsive"))
        {
            bail!("unresponsive Messaging listener became terminal evidence: {liveness:?}");
        }

        let mut state = store.load()?;
        let observations = [HostNodeObservation {
            kind: NodeKind::Messaging,
            module_available: true,
            contract_error: None,
            context_initialized: Some(true),
            liveness: liveness.observed,
            liveness_error: liveness.detail,
            managed_actions: None,
        }];
        reconcile_observations(&mut state, "default", &observations, &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state == NodeLifecycleState::Stopped
            || messaging.pending_lifecycle_action.is_some()
        {
            bail!("unresponsive Messaging listener settled Stop: {messaging:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_stop_uses_native_event_when_health_listener_is_unresponsive()
    -> Result<()> {
        let (address, worker) = unresponsive_messaging_listener()?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::new();
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());
        set_node_config_port(&store, NodeKind::Messaging, "restPort", 0)?;
        action_with_store(
            "default",
            initialize_request(NodeKind::Messaging),
            &transport,
            &store,
        )
        .await?;
        configure_messaging_health(&store, address, NodeLifecycleState::Running, None)?;

        let report = action_with_store(
            "default",
            node_action_request(NodeKind::Messaging, NodeAction::Stop),
            &transport,
            &store,
        )
        .await?;
        worker
            .join()
            .map_err(|_| anyhow::anyhow!("unresponsive Messaging listener panicked"))??;
        let messaging = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging")?;
        let operation = report
            .operations
            .last()
            .context("Basecamp Stop operation was not recorded")?;
        if messaging.run_state != "stopped"
            || messaging.available_actions != vec![NodeAction::Start]
            || operation.status != "stopped"
            || !operation
                .detail
                .contains("confirmed delivery_module.stop completion")
        {
            bail!("native nodeStopped did not settle unresponsive Messaging Stop: {report:?}");
        }
        Ok(())
    }

    fn prepared_messaging_stop(
        store: &LocalNodeStore,
    ) -> Result<(LocalNodeActionRequest, PreparedHostAction)> {
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
        messaging.package_path = Some("delivery_module".to_owned());
        messaging.lifecycle_state = NodeLifecycleState::Running;
        messaging.pending_lifecycle_action = None;
        store.save(&state)?;
        let request = node_action_request(NodeKind::Messaging, NodeAction::Stop);
        let plan = prepare_action("default", &state, &request)?;
        Ok((request, plan))
    }

    async fn assert_messaging_stop_metadata_rejects_before_dispatch(
        transport_impl: RecordingHostTransport,
    ) -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let (_, plan) = prepared_messaging_stop(&store)?;
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());
        let error =
            match execute_host_action_with_lifecycle_timeout(&plan, &transport, Duration::ZERO)
                .await
            {
                Ok(_) => bail!("invalid native nodeStopped metadata was accepted"),
                Err(error) => error,
            };
        if !format!("{error:#}").contains("nodeStopped(bool,QString,int)") {
            bail!("invalid nodeStopped metadata lost its contract detail: {error:#}");
        }
        if transport_impl
            .calls()?
            .iter()
            .any(|call| call.module() == "delivery_module" && call.method() == "stop")
        {
            bail!("invalid nodeStopped metadata dispatched delivery_module.stop");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_missing_native_stop_metadata_blocks_dispatch() -> Result<()> {
        assert_messaging_stop_metadata_rejects_before_dispatch(
            RecordingHostTransport::without_stop_event_metadata(),
        )
        .await
    }

    #[tokio::test]
    async fn basecamp_messaging_mismatched_native_stop_signature_blocks_dispatch() -> Result<()> {
        assert_messaging_stop_metadata_rejects_before_dispatch(
            RecordingHostTransport::with_mismatched_stop_event_signature(),
        )
        .await
    }

    #[tokio::test]
    async fn basecamp_messaging_failed_native_stop_event_never_settles_stopped() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let (request, plan) = prepared_messaging_stop(&store)?;
        let transport: SharedModuleTransport =
            Arc::new(RecordingHostTransport::lifecycle_failure());
        let error =
            match execute_host_action_with_lifecycle_timeout(&plan, &transport, Duration::ZERO)
                .await
            {
                Ok(_) => bail!("failed native nodeStopped was accepted"),
                Err(error) => error,
            };
        if !format!("{error:#}").contains("nodeStopped reported failure") {
            bail!("failed native nodeStopped lost its terminal detail: {error:#}");
        }
        let mut state = store.load()?;
        record_action_result(&mut state, "default", &request, &plan, Err(error), &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Running
            || messaging.pending_lifecycle_action.is_some()
        {
            bail!("failed native nodeStopped changed Messaging to a terminal Stop: {messaging:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_missing_native_stop_event_never_settles_stopped() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let (request, plan) = prepared_messaging_stop(&store)?;
        let transport: SharedModuleTransport =
            Arc::new(RecordingHostTransport::without_lifecycle_events());
        let error =
            match execute_host_action_with_lifecycle_timeout(&plan, &transport, Duration::ZERO)
                .await
            {
                Ok(_) => bail!("missing native nodeStopped was accepted"),
                Err(error) => error,
            };
        if !format!("{error:#}").contains("did not emit nodeStopped") {
            bail!("missing native nodeStopped lost its timeout detail: {error:#}");
        }
        let mut state = store.load()?;
        record_action_result(&mut state, "default", &request, &plan, Err(error), &store)?;
        let messaging = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Messaging)
            })
            .context("default topology omitted Messaging")?;
        if messaging.lifecycle_state != NodeLifecycleState::Running
            || messaging.pending_lifecycle_action.is_some()
        {
            bail!("missing native nodeStopped changed Messaging to a terminal Stop: {messaging:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_storage_status_ignores_unowned_tcp_listener() -> Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::new();
        let transport: SharedModuleTransport = Arc::new(transport_impl);
        set_node_config_port(&store, NodeKind::Storage, "listen-port", address.port())?;
        action_with_store(
            "default",
            initialize_request(NodeKind::Storage),
            &transport,
            &store,
        )
        .await?;
        let report = status_with_store("default", &transport, &store).await?;
        drop(listener);
        let storage = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted Storage")?;
        if storage.run_state != "stopped"
            || storage.available_actions.contains(&NodeAction::Stop)
            || !storage.available_actions.contains(&NodeAction::Start)
        {
            bail!("Storage status accepted an unowned TCP listener: {report:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_storage_stop_uses_native_event_when_tcp_listener_is_occupied() -> Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        let address = listener.local_addr()?;
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::new();
        let transport: SharedModuleTransport = Arc::new(transport_impl);
        set_node_config_port(&store, NodeKind::Storage, "listen-port", address.port())?;

        action_with_store(
            "default",
            initialize_request(NodeKind::Storage),
            &transport,
            &store,
        )
        .await?;
        let started = action_with_store(
            "default",
            node_action_request(NodeKind::Storage, NodeAction::Start),
            &transport,
            &store,
        )
        .await?;
        let started_storage = started
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted Storage after Start")?;
        if started_storage.run_state != "running" {
            bail!("native storageStart did not settle Storage running: {started:?}");
        }

        let stopped = action_with_store(
            "default",
            node_action_request(NodeKind::Storage, NodeAction::Stop),
            &transport,
            &store,
        )
        .await?;
        drop(listener);
        let storage = stopped
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted Storage after Stop")?;
        let operation = stopped
            .operations
            .last()
            .context("Basecamp Storage Stop operation was not recorded")?;
        if storage.run_state != "stopped"
            || storage.available_actions.contains(&NodeAction::Stop)
            || !storage.available_actions.contains(&NodeAction::Start)
            || operation.status != "stopped"
            || !operation
                .detail
                .contains("confirmed storage_module.stop completion")
        {
            bail!("native storageStop did not override an unrelated TCP listener: {stopped:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_storage_v1_acknowledges_delayed_start_and_reconciles_terminal_state()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingHostTransport::with_storage_node_lifecycle_v1();
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        action_with_store(
            "default",
            initialize_request(NodeKind::Storage),
            &transport,
            &store,
        )
        .await?;
        let started = tokio::time::timeout(
            Duration::from_millis(50),
            action_with_store(
                "default",
                node_action_request(NodeKind::Storage, NodeAction::Start),
                &transport,
                &store,
            ),
        )
        .await
        .context("Storage V1 Start waited for a terminal event")??;
        let storage = started
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted V1 Storage after Start")?;
        let start_operation = started
            .operations
            .last()
            .context("Basecamp Storage V1 Start operation was not recorded")?;
        if storage.run_state != "starting"
            || start_operation.status != "starting"
            || !start_operation
                .detail
                .contains("acknowledged V1 start transition")
            || !transport_impl
                .calls()?
                .iter()
                .any(|call| call.module() == "storage_module" && call.method() == "nodeAction")
            || transport_impl
                .calls()?
                .iter()
                .any(|call| call.module() == "storage_module" && call.method() == "start")
        {
            bail!("Storage V1 Start did not remain acknowledged and pending: {started:?}");
        }

        transport_impl.set_storage_lifecycle_state(StorageLifecycleState::Running)?;
        let settled = status_with_store("default", &transport, &store).await?;
        let storage = settled
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted V1 Storage after settlement")?;
        let start_operation = settled
            .operations
            .last()
            .context("Basecamp Storage V1 Start operation disappeared")?;
        if storage.run_state != "running"
            || start_operation.status != "running"
            || !start_operation
                .detail
                .contains("completed after lifecycle status confirmation")
        {
            bail!("Storage V1 terminal state did not settle the pending Start: {settled:?}");
        }

        let stopped = tokio::time::timeout(
            Duration::from_millis(50),
            action_with_store(
                "default",
                node_action_request(NodeKind::Storage, NodeAction::Stop),
                &transport,
                &store,
            ),
        )
        .await
        .context("Storage V1 Stop waited for a terminal event")??;
        let storage = stopped
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted V1 Storage after Stop")?;
        let stop_operation = stopped
            .operations
            .last()
            .context("Basecamp Storage V1 Stop operation was not recorded")?;
        if storage.run_state != "stopping"
            || stop_operation.status != "stopping"
            || !stop_operation
                .detail
                .contains("acknowledged V1 stop transition")
            || transport_impl
                .calls()?
                .iter()
                .any(|call| call.module() == "storage_module" && call.method() == "stop")
        {
            bail!("Storage V1 Stop did not remain acknowledged and pending: {stopped:?}");
        }

        transport_impl.set_storage_lifecycle_state(StorageLifecycleState::Stopped)?;
        let stopped = status_with_store("default", &transport, &store).await?;
        let storage = stopped
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Storage)
            .context("Basecamp report omitted V1 Storage after Stop settlement")?;
        let stop_operation = stopped
            .operations
            .last()
            .context("Basecamp Storage V1 Stop operation disappeared")?;
        if storage.run_state != "stopped"
            || stop_operation.status != "stopped"
            || !stop_operation
                .detail
                .contains("completed after lifecycle status confirmation")
        {
            bail!("Storage V1 terminal state did not settle the pending Stop: {stopped:?}");
        }
        Ok(())
    }

    #[test]
    fn storage_v1_ack_requires_the_expected_lifecycle_cursor() -> Result<()> {
        let snapshot = storage_node_lifecycle_snapshot(&json!({
            "schema": "logos.managed_node_lifecycle.snapshot",
            "version": 1,
            "instance_id": "storage-instance-1",
            "epoch": 3,
            "sequence": 8,
            "scope": { "kind": "storage" },
            "state": "stopped",
            "last_error": null,
        }))?;
        let error = validate_storage_node_lifecycle_ack(
            &json!({
                "schema": "logos.managed_node_lifecycle.ack",
                "version": 1,
                "operation_id": "storage-start-1",
                "accepted": true,
                "instance_id": "storage-instance-1",
                "epoch": 3,
                "sequence": 8,
                "state": "starting",
            }),
            "storage-start-1",
            &snapshot,
            "starting",
        )
        .err()
        .context("Storage V1 acknowledgement accepted a stale cursor")?;
        anyhow::ensure!(error.to_string().contains("invalid lifecycle cursor"));
        Ok(())
    }

    #[test]
    fn only_explicit_context_errors_clear_host_bindings() -> Result<()> {
        if !is_context_not_initialized(&anyhow::Error::msg(
            "storage_module.space failed: Storage context not initialized.",
        )) || is_context_not_initialized(&anyhow::Error::msg(
            "storage_module.space failed: request timed out",
        )) {
            bail!("Basecamp context error classification was not conservative");
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

    #[tokio::test]
    async fn basecamp_stopped_bedrock_is_startable() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let mut state = store.load()?;
        let bedrock = state
            .active_topology_mut("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter_mut()
                    .find(|node| node.kind == NodeKind::Bedrock)
            })
            .context("default topology omitted Bedrock")?;
        bedrock.installed = true;
        bedrock.package_path = Some("blockchain_module".to_owned());
        bedrock.lifecycle_state = NodeLifecycleState::Running;
        store.save(&state)?;
        let transport: SharedModuleTransport =
            Arc::new(RecordingHostTransport::with_stopped_bedrock());

        let report = status_with_store("default", &transport, &store).await?;
        let bedrock = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("Basecamp report omitted Bedrock")?;
        if bedrock.run_state != "stopped"
            || bedrock.available_actions != vec![NodeAction::Start]
            || bedrock.available_actions.contains(&NodeAction::Stop)
        {
            bail!("stopped Basecamp Bedrock remained non-startable: {bedrock:?}");
        }
        Ok(())
    }

    #[test]
    fn bedrock_v1_snapshot_rejects_an_inconsistent_action_set() -> Result<()> {
        let error = managed_node_lifecycle_snapshot(
            &json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": "bedrock-instance-1",
                "epoch": 1,
                "sequence": 2,
                "scope": { "kind": "bedrock" },
                "state": "stopped",
                "health": "unhealthy",
                "supported_actions": ["stop"],
                "last_error": null,
            }),
            "bedrock",
            "Bedrock nodeStatus",
        )
        .err()
        .context("Bedrock nodeStatus accepted an inconsistent action set")?;
        anyhow::ensure!(error.to_string().contains("inconsistent"));
        Ok(())
    }

    #[test]
    fn messaging_v1_snapshot_accepts_stopped_destroy_extension() -> Result<()> {
        let snapshot = managed_node_lifecycle_snapshot(
            &json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": "messaging-instance-1",
                "epoch": 1,
                "sequence": 2,
                "scope": { "kind": "messaging" },
                "state": "stopped",
                "health": "unknown",
                "supported_actions": ["start", "destroy"],
                "last_error": null,
            }),
            "messaging",
            "Messaging nodeStatus",
        )?;
        anyhow::ensure!(snapshot.supports_action("start"));
        anyhow::ensure!(snapshot.supports_action("destroy"));
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_bedrock_v1_status_uses_module_state_without_legacy_liveness() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let mut state = store.load()?;
        let bedrock = state
            .active_topology_mut("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter_mut()
                    .find(|node| node.kind == NodeKind::Bedrock)
            })
            .context("default topology omitted Bedrock")?;
        bedrock.installed = true;
        bedrock.package_path = Some("blockchain_module".to_owned());
        bedrock.lifecycle_state = NodeLifecycleState::Running;
        store.save(&state)?;
        let transport_impl = RecordingManagedNodeLifecycleTransport::new(true);
        transport_impl.set_state(ManagedNodeLifecycleState::Stopped)?;
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = status_with_store("default", &transport, &store).await?;
        let bedrock = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("Basecamp report omitted V1 Bedrock")?;
        if bedrock.run_state != "stopped"
            || bedrock.available_actions != vec![NodeAction::Start]
            || transport_impl.calls()?.iter().any(|call| {
                call.module() == "blockchain_module" && call.method() == "get_cryptarchia_info"
            })
        {
            bail!("Basecamp Bedrock V1 status inferred legacy liveness: {report:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_bedrock_v1_actions_use_terminal_events() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingManagedNodeLifecycleTransport::new(true);
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let initialized = action_with_store(
            "default",
            initialize_request(NodeKind::Bedrock),
            &transport,
            &store,
        )
        .await?;
        let started = action_with_store(
            "default",
            node_action_request(NodeKind::Bedrock, NodeAction::Start),
            &transport,
            &store,
        )
        .await?;
        let stopped = action_with_store(
            "default",
            node_action_request(NodeKind::Bedrock, NodeAction::Stop),
            &transport,
            &store,
        )
        .await?;

        let initialized_bedrock = initialized
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("Basecamp report omitted Bedrock after V1 initialize")?;
        let started_bedrock = started
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("Basecamp report omitted Bedrock after V1 start")?;
        let stopped_bedrock = stopped
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("Basecamp report omitted Bedrock after V1 stop")?;
        let operations = &stopped.operations;
        let (Some(initialized_operation), Some(started_operation), Some(stopped_operation)) = (
            operations.iter().rev().nth(2),
            operations.iter().rev().nth(1),
            operations.last(),
        ) else {
            bail!("Basecamp Bedrock V1 actions omitted lifecycle operations: {stopped:?}");
        };
        if initialized_bedrock.run_state != "stopped"
            || started_bedrock.run_state != "running"
            || stopped_bedrock.run_state != "stopped"
            || initialized_operation.status != "initialized"
            || started_operation.status != "running"
            || stopped_operation.status != "stopped"
            || !stopped_operation
                .detail
                .contains("V1 stop transition settled")
        {
            bail!("Basecamp Bedrock V1 actions did not settle from terminal events: {stopped:?}");
        }

        let calls = transport_impl.calls()?;
        let node_actions = calls
            .iter()
            .filter(|call| {
                call.module() == "blockchain_module" && call.method() == BEDROCK_NODE_ACTION_METHOD
            })
            .collect::<Vec<_>>();
        if node_actions.len() != 3
            || calls.iter().any(|call| {
                call.module() == "blockchain_module"
                    && matches!(call.method(), "generate_user_config" | "start" | "stop")
            })
        {
            bail!("Basecamp Bedrock V1 actions used the legacy lifecycle path: {calls:?}");
        }
        let initialize_request = node_actions
            .first()
            .context("Basecamp Bedrock V1 did not record initialize")?
            .args()
            .first()
            .and_then(Value::as_str)
            .context("Basecamp Bedrock V1 initialize did not send a request")?;
        let initialize_request: Value = serde_json::from_str(initialize_request)?;
        let config = initialize_request
            .get("parameters")
            .and_then(|parameters| parameters.get("config"))
            .and_then(Value::as_str)
            .context("Basecamp Bedrock V1 initialize did not send config text")?;
        if config.starts_with('@') || !serde_json::from_str::<Value>(config)?.is_object() {
            bail!("Basecamp Bedrock V1 initialize leaked a config path instead of config text");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_bedrock_v1_blocks_actions_without_native_event_ingress() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingManagedNodeLifecycleTransport::new(false);
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = action_with_store(
            "default",
            initialize_request(NodeKind::Bedrock),
            &transport,
            &store,
        )
        .await?;
        let bedrock = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("Basecamp report omitted Bedrock after rejected V1 initialize")?;
        let operation = report
            .operations
            .last()
            .context("Basecamp Bedrock V1 initialize was not recorded")?;
        if bedrock.install_state != "needs_configuration"
            || operation.status != "failed"
            || !operation.detail.contains("native lifecycle event ingress")
            || transport_impl.calls()?.iter().any(|call| {
                call.module() == "blockchain_module" && call.method() == BEDROCK_NODE_ACTION_METHOD
            })
        {
            bail!(
                "Basecamp Bedrock V1 accepted an action without native event ingress: {report:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_v1_status_uses_module_state_without_legacy_liveness() -> Result<()>
    {
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
        messaging.package_path = Some("delivery_module".to_owned());
        messaging.lifecycle_state = NodeLifecycleState::Running;
        store.save(&state)?;
        let transport_impl = RecordingManagedNodeLifecycleTransport::messaging(true);
        transport_impl.set_state(ManagedNodeLifecycleState::Stopped)?;
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = status_with_store("default", &transport, &store).await?;
        let messaging = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted V1 Messaging")?;
        if messaging.run_state != "stopped"
            || messaging.available_actions != vec![NodeAction::Start, NodeAction::Uninstall]
            || transport_impl
                .calls()?
                .iter()
                .any(|call| call.module() == "delivery_module" && call.method() == "getNodeInfo")
        {
            bail!("Basecamp Messaging V1 status inferred legacy liveness: {report:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_v1_destroy_uses_advertised_action_and_allows_reinitialization()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingManagedNodeLifecycleTransport::messaging(true);
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        action_with_store(
            "default",
            initialize_request(NodeKind::Messaging),
            &transport,
            &store,
        )
        .await?;
        action_with_store(
            "default",
            node_action_request(NodeKind::Messaging, NodeAction::Start),
            &transport,
            &store,
        )
        .await?;
        let stopped = action_with_store(
            "default",
            node_action_request(NodeKind::Messaging, NodeAction::Stop),
            &transport,
            &store,
        )
        .await?;
        let stopped_messaging = stopped
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging before V1 destroy")?;
        if stopped_messaging.available_actions != vec![NodeAction::Start, NodeAction::Uninstall] {
            bail!("Basecamp Messaging did not project advertised destroy: {stopped:?}");
        }

        let uninstalled = action_with_store(
            "default",
            node_action_request(NodeKind::Messaging, NodeAction::Uninstall),
            &transport,
            &store,
        )
        .await?;
        let uninstalled_messaging = uninstalled
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging after V1 destroy")?;
        let uninstalled_operation = uninstalled
            .operations
            .last()
            .context("Basecamp Messaging V1 destroy was not recorded")?;
        if uninstalled_messaging.install_state != "needs_configuration"
            || uninstalled_messaging.run_state != "not_initialized"
            || uninstalled_messaging.available_actions != vec![NodeAction::Initialize]
            || uninstalled_operation.status != "uninstalled"
            || !uninstalled_operation
                .detail
                .contains("V1 uninstall transition settled")
        {
            bail!("Basecamp Messaging V1 destroy did not clear its context: {uninstalled:?}");
        }

        let reinitialized = action_with_store(
            "default",
            initialize_request(NodeKind::Messaging),
            &transport,
            &store,
        )
        .await?;
        let reinitialized_messaging = reinitialized
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging after V1 reinitialize")?;
        if reinitialized_messaging.run_state != "stopped"
            || reinitialized_messaging.available_actions
                != vec![NodeAction::Start, NodeAction::Uninstall]
        {
            bail!("Basecamp Messaging V1 reinitialize did not settle: {reinitialized:?}");
        }

        let calls = transport_impl.calls()?;
        let node_actions = calls
            .iter()
            .filter(|call| {
                call.module() == "delivery_module" && call.method() == MESSAGING_NODE_ACTION_METHOD
            })
            .collect::<Vec<_>>();
        let actions = node_actions
            .iter()
            .map(|call| {
                call.args()
                    .first()
                    .and_then(Value::as_str)
                    .context("Basecamp Messaging V1 nodeAction did not send a request")
                    .and_then(|request| {
                        serde_json::from_str::<Value>(request)
                            .context("Basecamp Messaging V1 nodeAction request is invalid JSON")
                    })
                    .and_then(|request| {
                        request
                            .get("action")
                            .and_then(Value::as_str)
                            .map(ToOwned::to_owned)
                            .context("Basecamp Messaging V1 nodeAction request omitted action")
                    })
            })
            .collect::<Result<Vec<_>>>()?;
        if actions != ["initialize", "start", "stop", "destroy", "initialize"]
            || calls.iter().any(|call| {
                call.module() == "delivery_module"
                    && matches!(call.method(), "createNode" | "start" | "stop")
            })
        {
            bail!("Basecamp Messaging V1 destroy used the wrong lifecycle route: {calls:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_v1_actions_use_terminal_events() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingManagedNodeLifecycleTransport::messaging(true);
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let initialized = action_with_store(
            "default",
            initialize_request(NodeKind::Messaging),
            &transport,
            &store,
        )
        .await?;
        let started = action_with_store(
            "default",
            node_action_request(NodeKind::Messaging, NodeAction::Start),
            &transport,
            &store,
        )
        .await?;
        let stopped = action_with_store(
            "default",
            node_action_request(NodeKind::Messaging, NodeAction::Stop),
            &transport,
            &store,
        )
        .await?;

        let initialized_messaging = initialized
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging after V1 initialize")?;
        let started_messaging = started
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging after V1 start")?;
        let stopped_messaging = stopped
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging after V1 stop")?;
        let operations = &stopped.operations;
        let (Some(initialized_operation), Some(started_operation), Some(stopped_operation)) = (
            operations.iter().rev().nth(2),
            operations.iter().rev().nth(1),
            operations.last(),
        ) else {
            bail!("Basecamp Messaging V1 actions omitted lifecycle operations: {stopped:?}");
        };
        if initialized_messaging.run_state != "stopped"
            || started_messaging.run_state != "running"
            || stopped_messaging.run_state != "stopped"
            || initialized_operation.status != "initialized"
            || started_operation.status != "running"
            || stopped_operation.status != "stopped"
            || !stopped_operation
                .detail
                .contains("V1 stop transition settled")
        {
            bail!("Basecamp Messaging V1 actions did not settle from terminal events: {stopped:?}");
        }

        let calls = transport_impl.calls()?;
        let node_actions = calls
            .iter()
            .filter(|call| {
                call.module() == "delivery_module" && call.method() == MESSAGING_NODE_ACTION_METHOD
            })
            .collect::<Vec<_>>();
        if node_actions.len() != 3
            || calls.iter().any(|call| {
                call.module() == "delivery_module"
                    && matches!(call.method(), "createNode" | "start" | "stop")
            })
        {
            bail!("Basecamp Messaging V1 actions used the legacy lifecycle path: {calls:?}");
        }
        let initialize_request = node_actions
            .first()
            .context("Basecamp Messaging V1 did not record initialize")?
            .args()
            .first()
            .and_then(Value::as_str)
            .context("Basecamp Messaging V1 initialize did not send a request")?;
        let initialize_request: Value = serde_json::from_str(initialize_request)?;
        let config = initialize_request
            .get("parameters")
            .and_then(|parameters| parameters.get("config"))
            .and_then(Value::as_str)
            .context("Basecamp Messaging V1 initialize did not send config text")?;
        if config.starts_with('@') || !serde_json::from_str::<Value>(config)?.is_object() {
            bail!("Basecamp Messaging V1 initialize leaked a config path instead of config text");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_v1_accepts_empty_host_event_metadata() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl =
            RecordingManagedNodeLifecycleTransport::messaging_without_event_metadata(true);
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = action_with_store(
            "default",
            initialize_request(NodeKind::Messaging),
            &transport,
            &store,
        )
        .await?;
        let operation = report
            .operations
            .last()
            .context("Basecamp Messaging V1 initialize was not recorded")?;
        if operation.status != "initialized"
            || operation.command.as_deref() != Some("Basecamp host call delivery_module.nodeAction")
        {
            bail!("Basecamp Messaging V1 rejected empty host event metadata: {report:?}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_messaging_v1_blocks_actions_without_native_event_ingress() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport_impl = RecordingManagedNodeLifecycleTransport::messaging(false);
        let transport: SharedModuleTransport = Arc::new(transport_impl.clone());

        let report = action_with_store(
            "default",
            initialize_request(NodeKind::Messaging),
            &transport,
            &store,
        )
        .await?;
        let messaging = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("Basecamp report omitted Messaging after rejected V1 initialize")?;
        let operation = report
            .operations
            .last()
            .context("Basecamp Messaging V1 initialize was not recorded")?;
        if messaging.install_state != "needs_configuration"
            || operation.status != "failed"
            || !operation.detail.contains("native lifecycle event ingress")
            || transport_impl.calls()?.iter().any(|call| {
                call.module() == "delivery_module" && call.method() == MESSAGING_NODE_ACTION_METHOD
            })
        {
            bail!(
                "Basecamp Messaging V1 accepted an action without native event ingress: {report:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_uninitialized_bedrock_remains_initializable() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let transport: SharedModuleTransport =
            Arc::new(RecordingHostTransport::with_stopped_bedrock());

        let report = status_with_store("default", &transport, &store).await?;
        let bedrock = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Bedrock)
            .context("Basecamp report omitted Bedrock")?;
        if bedrock.install_state != "needs_configuration"
            || bedrock.run_state != "not_initialized"
            || bedrock.available_actions != vec![NodeAction::Initialize]
        {
            bail!("uninitialized Basecamp Bedrock became incorrectly startable: {bedrock:?}");
        }
        Ok(())
    }

    #[test]
    fn basecamp_unconfirmed_bedrock_liveness_is_unknown() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let mut state = store.load()?;
        let bedrock = state
            .active_topology_mut("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter_mut()
                    .find(|node| node.kind == NodeKind::Bedrock)
            })
            .context("default topology omitted Bedrock")?;
        bedrock.installed = true;
        bedrock.lifecycle_state = NodeLifecycleState::Running;

        reconcile_observations(
            &mut state,
            "default",
            &[HostNodeObservation {
                kind: NodeKind::Bedrock,
                module_available: true,
                contract_error: None,
                context_initialized: None,
                liveness: None,
                liveness_error: Some("Basecamp Bedrock probe timed out".to_owned()),
                managed_actions: None,
            }],
            &store,
        )?;

        let state = store.load()?;
        let bedrock = state
            .active_topology("default")
            .and_then(|topology| {
                topology
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Bedrock)
            })
            .context("default topology omitted Bedrock")?;
        if bedrock.lifecycle_state != NodeLifecycleState::Unknown {
            bail!("unconfirmed Basecamp Bedrock liveness became terminal: {bedrock:?}");
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
                context_initialized: Some(true),
                liveness: Some(true),
                liveness_error: None,
                managed_actions: None,
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

    #[test]
    fn missing_host_context_clears_stale_persisted_binding() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        let mut state = store.load()?;
        let topology_id = state
            .active_topology("default")
            .map(|topology| topology.id.clone())
            .context("default topology is unavailable")?;
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
        messaging.package_path = Some("delivery_module".to_owned());
        messaging.lifecycle_state = NodeLifecycleState::Running;
        state
            .module_context_topology_by_kind
            .insert(NodeKind::Messaging, topology_id);
        store.save(&state)?;

        reconcile_observations(
            &mut state,
            "default",
            &[HostNodeObservation {
                kind: NodeKind::Messaging,
                module_available: true,
                contract_error: None,
                context_initialized: Some(false),
                liveness: Some(false),
                liveness_error: Some("Context not initialized".to_owned()),
                managed_actions: None,
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
        if messaging.installed
            || messaging.lifecycle_state != NodeLifecycleState::NotInitialized
            || state
                .module_context_topology_id(NodeKind::Messaging)
                .is_some()
        {
            bail!("missing Basecamp context retained stale state: {messaging:?}");
        }
        Ok(())
    }
}
