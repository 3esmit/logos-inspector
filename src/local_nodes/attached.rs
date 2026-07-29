use std::{
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    modules::logos_core::{LogoscoreCliRuntime, normalize_module_call_value},
    support::{command_runner::CommandControl, time::now_millis},
};

use super::{
    LocalNodeActionRequest, LocalNodeOperationReport, LocalNodeReport, LocalNodeStatus,
    LocalNodeSummary, NodeAction, NodeKind, runtime::LogoscoreRuntimeProfile,
};

const ATTACHED_NODES: [AttachedNodeSpec; 3] = [
    AttachedNodeSpec {
        kind: NodeKind::Bedrock,
        module: "blockchain_module",
        scope: "bedrock",
    },
    AttachedNodeSpec {
        kind: NodeKind::Storage,
        module: "storage_module",
        scope: "storage",
    },
    AttachedNodeSpec {
        kind: NodeKind::Messaging,
        module: "delivery_module",
        scope: "messaging",
    },
];
const NODE_STATUS_METHOD: &str = "nodeStatus";
const NODE_STATUS_SIGNATURE: &str = "nodeStatus()";
const NODE_ACTION_METHOD: &str = "nodeAction";
const NODE_ACTION_SIGNATURE: &str = "nodeAction(QString)";
const NODE_CHANGED_EVENT: &str = "nodeChanged";
const NODE_CHANGED_SIGNATURE: &str = "nodeChanged(QString)";
const LIFECYCLE_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(30);
const LIFECYCLE_POLL_INTERVAL: Duration = Duration::from_millis(250);
static ATTACHED_OPERATION_SERIAL: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy)]
struct AttachedNodeSpec {
    kind: NodeKind,
    module: &'static str,
    scope: &'static str,
}

impl AttachedNodeSpec {
    fn for_kind(kind: NodeKind) -> Option<Self> {
        ATTACHED_NODES
            .iter()
            .copied()
            .find(|candidate| candidate.kind == kind)
    }

    const fn label(self) -> &'static str {
        match self.kind {
            NodeKind::Bedrock => "Bedrock",
            NodeKind::Storage => "Storage",
            NodeKind::Messaging => "Messaging",
            NodeKind::Sequencer | NodeKind::Indexer => "Node",
        }
    }
}

pub(super) fn overlay_report(
    report: &mut LocalNodeReport,
    runtime: Option<&LogoscoreRuntimeProfile>,
) -> Result<()> {
    let Some(runtime) = runtime.filter(|runtime| runtime.is_attached() && runtime.is_running())
    else {
        return Ok(());
    };
    let client = RuntimeClient::new(runtime.cli_runtime()?);
    let loaded = match loaded_module_versions(&client) {
        Ok(loaded) => loaded,
        Err(_) => return Ok(()),
    };

    for spec in ATTACHED_NODES {
        let Some(version) = loaded.get(spec.module) else {
            continue;
        };
        let Some(node) = report.nodes.iter_mut().find(|node| node.kind == spec.kind) else {
            continue;
        };
        let observation = inspect_node(&client, spec);
        let (config_path, initialization_configuration_ready) =
            attached_initialization_configuration(runtime, spec.kind);
        overlay_node(
            node,
            spec,
            version,
            observation,
            config_path,
            initialization_configuration_ready,
        );
    }
    refresh_summary(report);
    Ok(())
}

pub(super) fn apply(
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> Result<Option<LocalNodeOperationReport>> {
    let Some(runtime) = runtime.filter(|runtime| runtime.is_attached()) else {
        return Ok(None);
    };
    let Some(kind) = request.node else {
        return Ok(None);
    };
    let Some(spec) = AttachedNodeSpec::for_kind(kind) else {
        return Ok(None);
    };
    if !matches!(
        request.action,
        NodeAction::Initialize
            | NodeAction::Start
            | NodeAction::Stop
            | NodeAction::Uninstall
            | NodeAction::Purge
    ) {
        return Ok(None);
    }

    let timestamp = now_millis();
    let outcome = if !runtime.is_running() {
        Err(anyhow::anyhow!(
            "the local LogosCore service is not running; start the service before controlling {}",
            spec.label()
        ))
    } else {
        let client = RuntimeClient::new(runtime.cli_runtime()?);
        match request.action {
            NodeAction::Initialize => {
                let config =
                    super::config::load_attached_initialization_config(runtime, spec.kind)?;
                execute_lifecycle_action(&client, spec, request.action, Some(&config), control)
            }
            NodeAction::Start | NodeAction::Stop => {
                execute_lifecycle_action(&client, spec, request.action, None, control)
            }
            NodeAction::Uninstall | NodeAction::Purge => Err(anyhow::anyhow!(
                "{} destructive actions remain unavailable for an attached service",
                spec.label()
            )),
            NodeAction::StartRuntime
            | NodeAction::StopRuntime
            | NodeAction::Install
            | NodeAction::NewNetwork
            | NodeAction::LoadNetwork
            | NodeAction::DeleteNetwork
            | NodeAction::ResetNetwork => return Ok(None),
        }
    };
    let (status, detail) = match outcome {
        Ok(detail) => (
            match request.action {
                NodeAction::Start => "running",
                NodeAction::Stop => "stopped",
                NodeAction::Initialize => "initialized",
                NodeAction::Uninstall => "uninstalled",
                NodeAction::Purge => "purged",
                NodeAction::StartRuntime
                | NodeAction::StopRuntime
                | NodeAction::Install
                | NodeAction::NewNetwork
                | NodeAction::LoadNetwork
                | NodeAction::DeleteNetwork
                | NodeAction::ResetNetwork => "completed",
            }
            .to_owned(),
            detail,
        ),
        Err(error) => ("failed".to_owned(), format!("{error:#}")),
    };
    Ok(Some(LocalNodeOperationReport {
        id: format!("attached-op-{timestamp}"),
        time: timestamp.to_string(),
        timestamp_millis: timestamp,
        action: request.action,
        node: Some(spec.kind),
        network_id: request.network_id.clone(),
        status,
        detail,
        command: Some(format!(
            "LogosCore CLI call {}.{}",
            spec.module, NODE_ACTION_METHOD
        )),
    }))
}

fn overlay_node(
    node: &mut LocalNodeStatus,
    spec: AttachedNodeSpec,
    version: &str,
    observation: Result<Option<ManagedNodeSnapshot>>,
    config_path: Option<String>,
    initialization_configuration_ready: Option<bool>,
) {
    node.ownership = "local_attached".to_owned();
    node.endpoint = None;
    node.data_dir = None;
    node.config_path = config_path;
    node.initialization_configuration_ready = initialization_configuration_ready;
    node.package_path = Some(spec.module.to_owned());
    node.package_version = Some(version.to_owned());
    node.process_id = None;
    match observation {
        Ok(Some(snapshot)) => {
            node.install_state = if snapshot.state == ManagedNodeState::Uninitialized {
                "needs_configuration"
            } else {
                "installed"
            }
            .to_owned();
            node.run_state = snapshot.state.as_str().to_owned();
            node.available_actions = snapshot
                .supported_actions
                .iter()
                .filter_map(|action| match action.as_str() {
                    "initialize" => Some(NodeAction::Initialize),
                    "start" => Some(NodeAction::Start),
                    "stop" => Some(NodeAction::Stop),
                    _ => None,
                })
                .collect();
            node.detail = attached_detail(
                spec,
                &snapshot,
                node.initialization_configuration_ready == Some(true),
            );
        }
        Ok(None) => {
            node.install_state = "needs_configuration".to_owned();
            node.run_state = "unknown".to_owned();
            node.available_actions.clear();
            node.detail = format!(
                "Attached local {} module does not expose the V1 lifecycle contract. Update the module before controlling it.",
                spec.label()
            );
        }
        Err(error) => {
            node.install_state = "needs_configuration".to_owned();
            node.run_state = "unknown".to_owned();
            node.available_actions.clear();
            node.detail = format!(
                "Attached local {} lifecycle status is unavailable: {error:#}",
                spec.label()
            );
        }
    }
}

fn attached_detail(
    spec: AttachedNodeSpec,
    snapshot: &ManagedNodeSnapshot,
    initialization_configuration_ready: bool,
) -> String {
    if let Some(error) = snapshot.last_error.as_deref() {
        return format!("Attached local {} reported: {error}", spec.label());
    }
    match snapshot.state {
        ManagedNodeState::Uninitialized if initialization_configuration_ready => format!(
            "Attached local {} has no initialized context. Its saved Inspector-owned initialization configuration is ready.",
            spec.label()
        ),
        ManagedNodeState::Uninitialized => format!(
            "Attached local {} has no initialized context. Save an Inspector-owned initialization configuration before initializing; Inspector will not read or overwrite an unknown service configuration.",
            spec.label()
        ),
        ManagedNodeState::Initializing => {
            format!("Attached local {} is initializing.", spec.label())
        }
        ManagedNodeState::Stopped => format!(
            "Attached local {} is stopped; lifecycle is controlled through the local service.",
            spec.label()
        ),
        ManagedNodeState::Starting => format!("Attached local {} is starting.", spec.label()),
        ManagedNodeState::Running => format!("Attached local {} is running.", spec.label()),
        ManagedNodeState::Stopping => format!("Attached local {} is stopping.", spec.label()),
        ManagedNodeState::Destroying => {
            format!("Attached local {} is destroying its context.", spec.label())
        }
    }
}

fn attached_initialization_configuration(
    runtime: &LogoscoreRuntimeProfile,
    kind: NodeKind,
) -> (Option<String>, Option<bool>) {
    let Ok(path) = super::config::attached_initialization_config_path(runtime, kind) else {
        return (None, None);
    };
    let ready = super::config::attached_initialization_config_ready(runtime, kind).unwrap_or(false);
    (Some(path.display().to_string()), Some(ready))
}

fn refresh_summary(report: &mut LocalNodeReport) {
    report.summary = LocalNodeSummary {
        total: report.nodes.len(),
        installed: report
            .nodes
            .iter()
            .filter(|node| node.install_state == "installed")
            .count(),
        running: report
            .nodes
            .iter()
            .filter(|node| node.run_state == "running")
            .count(),
        needs_configuration: report
            .nodes
            .iter()
            .filter(|node| node.install_state == "needs_configuration")
            .count(),
    };
}

fn execute_lifecycle_action(
    client: &impl AttachedModuleClient,
    spec: AttachedNodeSpec,
    action: NodeAction,
    initialization_config: Option<&str>,
    control: Option<&CommandControl>,
) -> Result<String> {
    let metadata = client.module_info(spec.module)?;
    anyhow::ensure!(
        supports_v1_lifecycle(&metadata),
        "attached local {} module does not expose nodeStatus(), nodeAction(QString), and nodeChanged(QString); update the module before controlling it",
        spec.label()
    );
    let snapshot = node_snapshot(client, spec, control)?;
    let (action_name, expected_transition, expected_terminal) = match action {
        NodeAction::Initialize => ("initialize", "initializing", ManagedNodeState::Stopped),
        NodeAction::Start => ("start", "starting", ManagedNodeState::Running),
        NodeAction::Stop => ("stop", "stopping", ManagedNodeState::Stopped),
        _ => bail!("{} is not an attached V1 lifecycle action", action.as_str()),
    };
    anyhow::ensure!(
        snapshot.supports_action(action_name),
        "{} V1 nodeStatus does not allow `{action_name}` in its current state",
        spec.label()
    );

    let serial = ATTACHED_OPERATION_SERIAL.fetch_add(1, Ordering::Relaxed);
    let operation_id = format!(
        "logos-inspector-attached-{}-{action_name}-{}-{serial}",
        spec.kind.as_str(),
        now_millis()
    );
    let parameters = match action {
        NodeAction::Initialize => json!({
            "config": initialization_config.context(
                "attached initialization requires a saved Inspector-owned initialization configuration",
            )?,
        }),
        NodeAction::Start | NodeAction::Stop => json!({}),
        _ => bail!("{} is not an attached V1 lifecycle action", action.as_str()),
    };
    let request = json!({
        "schema": "logos.managed_node_lifecycle.command",
        "version": 1,
        "operation_id": operation_id,
        "action": action_name,
        "expected": {
            "instance_id": snapshot.instance_id,
            "epoch": snapshot.epoch,
            "sequence": snapshot.sequence,
        },
        "parameters": parameters,
    });
    let response = client.call(
        spec.module,
        NODE_ACTION_METHOD,
        &[request.to_string()],
        control,
    )?;
    let acknowledgement = normalize_module_call_value(spec.module, NODE_ACTION_METHOD, response)?;
    validate_acknowledgement(
        &acknowledgement,
        &operation_id,
        &snapshot,
        expected_transition,
        spec,
    )?;
    let terminal = wait_for_terminal_snapshot(
        client,
        spec,
        expected_terminal,
        control,
        LIFECYCLE_CONFIRMATION_TIMEOUT,
        LIFECYCLE_POLL_INTERVAL,
    )?;
    Ok(format!(
        "Attached local {} {} confirmed by V1 nodeStatus polling at lifecycle sequence {}.",
        spec.label(),
        action.as_str(),
        terminal.sequence
    ))
}

fn wait_for_terminal_snapshot(
    client: &impl AttachedModuleClient,
    spec: AttachedNodeSpec,
    expected_terminal: ManagedNodeState,
    control: Option<&CommandControl>,
    timeout: Duration,
    poll_interval: Duration,
) -> Result<ManagedNodeSnapshot> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("attached lifecycle confirmation deadline overflowed")?;
    loop {
        check_control(control)?;
        let snapshot = node_snapshot(client, spec, control)?;
        if snapshot.state == expected_terminal {
            return Ok(snapshot);
        }
        if let Some(error) = snapshot.last_error.as_deref() {
            bail!("{} V1 lifecycle operation failed: {error}", spec.label());
        }
        if Instant::now() >= deadline {
            bail!(
                "Attached local {} did not reach {} before lifecycle confirmation timeout",
                spec.label(),
                expected_terminal.as_str()
            );
        }
        thread::sleep(poll_interval);
    }
}

fn check_control(control: Option<&CommandControl>) -> Result<()> {
    if let Some(control) = control {
        control.check_active()?;
    }
    Ok(())
}

fn node_snapshot(
    client: &impl AttachedModuleClient,
    spec: AttachedNodeSpec,
    control: Option<&CommandControl>,
) -> Result<ManagedNodeSnapshot> {
    let value = client.call(spec.module, NODE_STATUS_METHOD, &[], control)?;
    let value = normalize_module_call_value(spec.module, NODE_STATUS_METHOD, value)?;
    ManagedNodeSnapshot::parse(&value, spec.scope, spec.label())
}

fn inspect_node(
    client: &impl AttachedModuleClient,
    spec: AttachedNodeSpec,
) -> Result<Option<ManagedNodeSnapshot>> {
    let metadata = client.module_info(spec.module)?;
    if !supports_v1_lifecycle(&metadata) {
        return Ok(None);
    }
    node_snapshot(client, spec, None).map(Some)
}

fn loaded_module_versions(
    client: &impl AttachedModuleClient,
) -> Result<std::collections::BTreeMap<String, String>> {
    let status = client.status()?;
    let modules = status
        .get("modules")
        .and_then(Value::as_array)
        .context("local LogosCore status has no module list")?;
    Ok(modules
        .iter()
        .filter(|module| module.get("status").and_then(Value::as_str) == Some("loaded"))
        .filter_map(|module| {
            let name = module.get("name").and_then(Value::as_str)?;
            let version = module
                .get("version")
                .and_then(Value::as_str)
                .unwrap_or("unknown");
            Some((name.to_owned(), version.to_owned()))
        })
        .collect())
}

fn supports_v1_lifecycle(metadata: &Value) -> bool {
    metadata_has_method(metadata, NODE_STATUS_METHOD, NODE_STATUS_SIGNATURE)
        && metadata_has_method(metadata, NODE_ACTION_METHOD, NODE_ACTION_SIGNATURE)
        && metadata_has_event(metadata, NODE_CHANGED_EVENT, NODE_CHANGED_SIGNATURE)
}

fn metadata_has_method(metadata: &Value, method: &str, signature: &str) -> bool {
    metadata
        .get("methods")
        .and_then(Value::as_array)
        .is_some_and(|methods| {
            methods.iter().any(|candidate| {
                candidate.get("name").and_then(Value::as_str) == Some(method)
                    && candidate.get("signature").and_then(Value::as_str) == Some(signature)
                    && candidate.get("isInvokable").and_then(Value::as_bool) != Some(false)
            })
        })
}

fn metadata_has_event(metadata: &Value, event: &str, signature: &str) -> bool {
    metadata
        .get("events")
        .and_then(Value::as_array)
        .is_some_and(|events| {
            events.iter().any(|candidate| {
                candidate.get("name").and_then(Value::as_str) == Some(event)
                    && candidate.get("signature").and_then(Value::as_str) == Some(signature)
            })
        })
}

fn validate_acknowledgement(
    value: &Value,
    operation_id: &str,
    snapshot: &ManagedNodeSnapshot,
    expected_state: &str,
    spec: AttachedNodeSpec,
) -> Result<()> {
    let acknowledgement = match value {
        Value::String(text) => serde_json::from_str::<Value>(text)
            .context("V1 nodeAction response is not valid JSON")?,
        value => value.clone(),
    };
    anyhow::ensure!(
        acknowledgement.get("schema").and_then(Value::as_str)
            == Some("logos.managed_node_lifecycle.ack")
            && acknowledgement.get("version").and_then(Value::as_u64) == Some(1),
        "{} V1 nodeAction response has an unsupported schema or version",
        spec.label()
    );
    anyhow::ensure!(
        acknowledgement.get("operation_id").and_then(Value::as_str) == Some(operation_id),
        "{} V1 nodeAction response does not acknowledge the submitted operation",
        spec.label()
    );
    let accepted = acknowledgement
        .get("accepted")
        .and_then(Value::as_bool)
        .context("V1 nodeAction response has no accepted field")?;
    if !accepted {
        let detail = acknowledgement
            .pointer("/error/message")
            .and_then(Value::as_str)
            .filter(|message| !message.trim().is_empty())
            .unwrap_or("the module rejected the lifecycle request");
        bail!(
            "{} V1 nodeAction rejected the lifecycle request: {detail}",
            spec.label()
        );
    }
    anyhow::ensure!(
        acknowledgement.get("instance_id").and_then(Value::as_str) == Some(&snapshot.instance_id)
            && acknowledgement.get("epoch").and_then(Value::as_u64) == Some(snapshot.epoch)
            && acknowledgement
                .get("sequence")
                .and_then(Value::as_u64)
                .is_some_and(|sequence| sequence > snapshot.sequence),
        "{} V1 nodeAction response has an invalid lifecycle cursor",
        spec.label()
    );
    anyhow::ensure!(
        acknowledgement.get("state").and_then(Value::as_str) == Some(expected_state),
        "{} V1 nodeAction response has an unexpected lifecycle state",
        spec.label()
    );
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedNodeState {
    Uninitialized,
    Initializing,
    Stopped,
    Starting,
    Running,
    Stopping,
    Destroying,
}

impl ManagedNodeState {
    fn parse(value: &str, label: &str) -> Result<Self> {
        match value {
            "uninitialized" => Ok(Self::Uninitialized),
            "initializing" => Ok(Self::Initializing),
            "stopped" => Ok(Self::Stopped),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "destroying" => Ok(Self::Destroying),
            state => bail!("{label} nodeStatus returned unknown lifecycle state `{state}`"),
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
            // Modules may expose destructive context cleanup while stopped.
            // Inspector intentionally filters it from attached-service controls.
            Self::Stopped => &["start", "destroy"],
            Self::Running => &["stop"],
            Self::Initializing | Self::Starting | Self::Stopping | Self::Destroying => &[],
        }
    }
}

#[derive(Debug, Clone)]
struct ManagedNodeSnapshot {
    instance_id: String,
    epoch: u64,
    sequence: u64,
    state: ManagedNodeState,
    supported_actions: Vec<String>,
    last_error: Option<String>,
}

impl ManagedNodeSnapshot {
    fn parse(value: &Value, expected_scope: &str, label: &str) -> Result<Self> {
        let value = match value {
            Value::String(text) => serde_json::from_str::<Value>(text)
                .with_context(|| format!("{label} nodeStatus response is not valid JSON"))?,
            value => value.clone(),
        };
        let payload: ManagedNodeSnapshotPayload = serde_json::from_value(value)
            .with_context(|| format!("{label} nodeStatus response has an invalid shape"))?;
        anyhow::ensure!(
            payload.schema == "logos.managed_node_lifecycle.snapshot" && payload.version == 1,
            "{label} nodeStatus response has an unsupported schema or version"
        );
        anyhow::ensure!(
            !payload.instance_id.trim().is_empty(),
            "{label} nodeStatus response has an empty instance_id"
        );
        anyhow::ensure!(
            payload.scope.kind == expected_scope,
            "{label} nodeStatus response has an unexpected scope"
        );
        anyhow::ensure!(
            matches!(
                payload.health.as_str(),
                "unknown" | "healthy" | "degraded" | "unhealthy" | "not_applicable"
            ),
            "{label} nodeStatus response has an unknown health state"
        );
        let state = ManagedNodeState::parse(&payload.state, label)?;
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
                && actions.iter().enumerate().all(|(index, action)| !actions
                    .iter()
                    .take(index)
                    .any(|prior| prior == action)),
            "{label} nodeStatus response has actions inconsistent with its lifecycle state"
        );
        let last_error = payload
            .last_error
            .map(|error| {
                let code = error.code.trim();
                let message = error.message.trim();
                anyhow::ensure!(
                    !code.is_empty() && !message.is_empty(),
                    "{label} nodeStatus response has an incomplete last_error"
                );
                Ok(format!("{code}: {message}"))
            })
            .transpose()?;
        Ok(Self {
            instance_id: payload.instance_id,
            epoch: payload.epoch,
            sequence: payload.sequence,
            state,
            supported_actions: payload.supported_actions,
            last_error,
        })
    }

    fn supports_action(&self, action: &str) -> bool {
        self.supported_actions
            .iter()
            .any(|candidate| candidate == action)
    }
}

#[derive(Deserialize)]
struct ManagedNodeSnapshotPayload {
    schema: String,
    version: u64,
    instance_id: String,
    epoch: u64,
    sequence: u64,
    scope: ManagedNodeScope,
    state: String,
    health: String,
    supported_actions: Vec<String>,
    #[serde(default)]
    last_error: Option<ManagedNodeError>,
}

#[derive(Deserialize)]
struct ManagedNodeScope {
    kind: String,
}

#[derive(Deserialize)]
struct ManagedNodeError {
    code: String,
    message: String,
}

trait AttachedModuleClient {
    fn status(&self) -> Result<Value>;
    fn module_info(&self, module: &str) -> Result<Value>;
    fn call(
        &self,
        module: &str,
        method: &str,
        args: &[String],
        control: Option<&CommandControl>,
    ) -> Result<Value>;
}

struct RuntimeClient {
    runtime: LogoscoreCliRuntime,
}

impl RuntimeClient {
    const fn new(runtime: LogoscoreCliRuntime) -> Self {
        Self { runtime }
    }
}

impl AttachedModuleClient for RuntimeClient {
    fn status(&self) -> Result<Value> {
        self.runtime.status().map(|output| output.value)
    }

    fn module_info(&self, module: &str) -> Result<Value> {
        self.runtime.module_info(module).map(|output| output.value)
    }

    fn call(
        &self,
        module: &str,
        method: &str,
        args: &[String],
        control: Option<&CommandControl>,
    ) -> Result<Value> {
        let output = match control {
            Some(control) => self
                .runtime
                .call_controlled(module, method, args, control.clone()),
            None => self.runtime.call(module, method, args),
        }?;
        Ok(output.value)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use anyhow::Result;

    use super::*;

    struct RecordingClient {
        calls: Mutex<Vec<(String, String, Vec<String>)>>,
        state: Mutex<ManagedNodeState>,
        sequence: Mutex<u64>,
        v1: bool,
    }

    impl RecordingClient {
        fn running() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                state: Mutex::new(ManagedNodeState::Running),
                sequence: Mutex::new(3),
                v1: true,
            }
        }

        fn uninitialized() -> Self {
            Self {
                calls: Mutex::new(Vec::new()),
                state: Mutex::new(ManagedNodeState::Uninitialized),
                sequence: Mutex::new(0),
                v1: true,
            }
        }

        fn calls(&self) -> Result<Vec<(String, String, Vec<String>)>> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|_| anyhow::anyhow!("recording client calls lock is poisoned"))
        }

        fn snapshot(&self, spec: AttachedNodeSpec) -> Result<Value> {
            let state = *self
                .state
                .lock()
                .map_err(|_| anyhow::anyhow!("recording client state lock is poisoned"))?;
            let sequence = *self
                .sequence
                .lock()
                .map_err(|_| anyhow::anyhow!("recording client sequence lock is poisoned"))?;
            Ok(json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": "recording-instance",
                "epoch": 1,
                "sequence": sequence,
                "scope": { "kind": spec.scope },
                "state": state.as_str(),
                "health": if state == ManagedNodeState::Running { "healthy" } else { "unknown" },
                "supported_actions": state.expected_actions(),
                "last_error": null,
            }))
        }
    }

    impl AttachedModuleClient for RecordingClient {
        fn status(&self) -> Result<Value> {
            Ok(json!({
                "modules": ATTACHED_NODES.map(|spec| json!({
                    "name": spec.module,
                    "status": "loaded",
                    "version": "test-v1",
                })),
            }))
        }

        fn module_info(&self, _module: &str) -> Result<Value> {
            let methods = if self.v1 {
                vec![
                    json!({"name": NODE_STATUS_METHOD, "signature": NODE_STATUS_SIGNATURE, "isInvokable": true}),
                    json!({"name": NODE_ACTION_METHOD, "signature": NODE_ACTION_SIGNATURE, "isInvokable": true}),
                ]
            } else {
                Vec::new()
            };
            let events = if self.v1 {
                vec![json!({"name": NODE_CHANGED_EVENT, "signature": NODE_CHANGED_SIGNATURE})]
            } else {
                Vec::new()
            };
            Ok(json!({"methods": methods, "events": events}))
        }

        fn call(
            &self,
            module: &str,
            method: &str,
            args: &[String],
            _control: Option<&CommandControl>,
        ) -> Result<Value> {
            self.calls
                .lock()
                .map_err(|_| anyhow::anyhow!("recording client calls lock is poisoned"))?
                .push((module.to_owned(), method.to_owned(), args.to_vec()));
            let spec = ATTACHED_NODES
                .iter()
                .copied()
                .find(|spec| spec.module == module)
                .context("recording client received unknown module")?;
            match method {
                NODE_STATUS_METHOD => self.snapshot(spec),
                NODE_ACTION_METHOD => {
                    let request = args
                        .first()
                        .context("recording lifecycle command has no request")?;
                    let request: Value = serde_json::from_str(request)
                        .context("recording lifecycle command is not valid JSON")?;
                    let action = request
                        .get("action")
                        .and_then(Value::as_str)
                        .context("recording lifecycle command has no action")?;
                    let parameters = request
                        .get("parameters")
                        .and_then(Value::as_object)
                        .context("recording lifecycle command has no parameters")?;
                    let expected = request
                        .get("expected")
                        .and_then(Value::as_object)
                        .context("recording lifecycle command has no expected cursor")?;
                    let previous = *self
                        .state
                        .lock()
                        .map_err(|_| anyhow::anyhow!("recording client state lock is poisoned"))?;
                    let mut sequence = self.sequence.lock().map_err(|_| {
                        anyhow::anyhow!("recording client sequence lock is poisoned")
                    })?;
                    anyhow::ensure!(
                        expected.get("instance_id").and_then(Value::as_str)
                            == Some("recording-instance")
                            && expected.get("epoch").and_then(Value::as_u64) == Some(1)
                            && expected.get("sequence").and_then(Value::as_u64) == Some(*sequence),
                        "recording lifecycle command has an invalid expected cursor"
                    );
                    let (transition, terminal) = match (action, previous) {
                        ("initialize", ManagedNodeState::Uninitialized) => {
                            anyhow::ensure!(
                                parameters
                                    .get("config")
                                    .and_then(Value::as_str)
                                    .is_some_and(|config| !config.trim().is_empty()),
                                "recording initialize command has no configuration"
                            );
                            (ManagedNodeState::Initializing, ManagedNodeState::Stopped)
                        }
                        ("start", ManagedNodeState::Stopped) => {
                            anyhow::ensure!(
                                parameters.is_empty(),
                                "recording start command unexpectedly has parameters"
                            );
                            (ManagedNodeState::Starting, ManagedNodeState::Running)
                        }
                        ("stop", ManagedNodeState::Running) => {
                            anyhow::ensure!(
                                parameters.is_empty(),
                                "recording stop command unexpectedly has parameters"
                            );
                            (ManagedNodeState::Stopping, ManagedNodeState::Stopped)
                        }
                        _ => bail!("recording lifecycle command is invalid for current state"),
                    };
                    *sequence += 1;
                    *self.state.lock().map_err(|_| {
                        anyhow::anyhow!("recording client state lock is poisoned")
                    })? = terminal;
                    Ok(json!({
                        "schema": "logos.managed_node_lifecycle.ack",
                        "version": 1,
                        "operation_id": request.get("operation_id"),
                        "accepted": true,
                        "instance_id": "recording-instance",
                        "epoch": 1,
                        "sequence": *sequence,
                        "state": transition.as_str(),
                    }))
                }
                method => bail!("recording client received unexpected method `{method}`"),
            }
        }
    }

    #[test]
    fn v1_stop_uses_only_node_action_and_confirms_by_polling() -> Result<()> {
        let client = RecordingClient::running();
        let spec = AttachedNodeSpec::for_kind(NodeKind::Messaging).context("missing spec")?;

        let detail = execute_lifecycle_action(&client, spec, NodeAction::Stop, None, None)?;

        anyhow::ensure!(detail.contains("confirmed by V1 nodeStatus polling"));
        let calls = client.calls()?;
        anyhow::ensure!(
            calls
                .iter()
                .any(|(_, method, _)| method == NODE_ACTION_METHOD)
                && calls
                    .iter()
                    .all(|(_, method, _)| method == NODE_STATUS_METHOD
                        || method == NODE_ACTION_METHOD),
            "attached lifecycle used a legacy module method: {calls:?}"
        );
        Ok(())
    }

    #[test]
    fn v1_initialize_sends_saved_configuration_and_confirms_stopped() -> Result<()> {
        let client = RecordingClient::uninitialized();
        let spec = AttachedNodeSpec::for_kind(NodeKind::Storage).context("missing spec")?;
        let config = r#"{"data-dir":"/var/lib/logos/storage","listen-ip":"0.0.0.0"}"#;

        let detail =
            execute_lifecycle_action(&client, spec, NodeAction::Initialize, Some(config), None)?;

        anyhow::ensure!(detail.contains("confirmed by V1 nodeStatus polling"));
        let calls = client.calls()?;
        let command = calls
            .iter()
            .find(|(_, method, _)| method == NODE_ACTION_METHOD)
            .context("recording client received no nodeAction call")?;
        let request: Value = serde_json::from_str(
            command
                .2
                .first()
                .context("recording nodeAction call has no request")?,
        )?;
        anyhow::ensure!(
            request.get("action").and_then(Value::as_str) == Some("initialize")
                && request
                    .pointer("/parameters/config")
                    .and_then(Value::as_str)
                    == Some(config),
            "attached initialize did not forward its saved configuration"
        );
        anyhow::ensure!(
            calls
                .iter()
                .all(|(_, method, _)| method == NODE_STATUS_METHOD || method == NODE_ACTION_METHOD),
            "attached initialize used a legacy module method: {calls:?}"
        );
        anyhow::ensure!(node_snapshot(&client, spec, None)?.state == ManagedNodeState::Stopped);
        Ok(())
    }

    #[test]
    fn v1_metadata_requires_every_contract_member() -> Result<()> {
        let client = RecordingClient {
            v1: false,
            ..RecordingClient::running()
        };
        let spec = AttachedNodeSpec::for_kind(NodeKind::Storage).context("missing spec")?;

        anyhow::ensure!(inspect_node(&client, spec)?.is_none());
        anyhow::ensure!(client.calls()?.is_empty());
        Ok(())
    }

    #[test]
    fn snapshot_rejects_actions_that_do_not_match_state() -> Result<()> {
        let error = ManagedNodeSnapshot::parse(
            &json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": "recording-instance",
                "epoch": 1,
                "sequence": 1,
                "scope": { "kind": "bedrock" },
                "state": "running",
                "health": "healthy",
                "supported_actions": ["start"],
                "last_error": null,
            }),
            "bedrock",
            "Bedrock",
        )
        .err()
        .context("inconsistent snapshot unexpectedly parsed")?;
        anyhow::ensure!(error.to_string().contains("inconsistent"));
        Ok(())
    }

    #[test]
    fn snapshot_rejects_duplicate_actions() -> Result<()> {
        let error = ManagedNodeSnapshot::parse(
            &json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": "recording-instance",
                "epoch": 1,
                "sequence": 1,
                "scope": { "kind": "bedrock" },
                "state": "stopped",
                "health": "unknown",
                "supported_actions": ["start", "start"],
                "last_error": null,
            }),
            "bedrock",
            "Bedrock",
        )
        .err()
        .context("duplicate action snapshot unexpectedly parsed")?;
        anyhow::ensure!(error.to_string().contains("inconsistent"));
        Ok(())
    }

    #[test]
    fn snapshot_accepts_stopped_destroy_extension() -> Result<()> {
        let snapshot = ManagedNodeSnapshot::parse(
            &json!({
                "schema": "logos.managed_node_lifecycle.snapshot",
                "version": 1,
                "instance_id": "recording-instance",
                "epoch": 1,
                "sequence": 1,
                "scope": { "kind": "storage" },
                "state": "stopped",
                "health": "unknown",
                "supported_actions": ["start", "destroy"],
                "last_error": null,
            }),
            "storage",
            "Storage",
        )?;
        anyhow::ensure!(snapshot.supports_action("start"));
        Ok(())
    }

    #[test]
    fn attached_report_exposes_v1_initialize_with_owned_configuration_state() -> Result<()> {
        let client = RecordingClient::uninitialized();
        let mut report = LocalNodeReport {
            profile: "default".to_owned(),
            mode: "testnet".to_owned(),
            available_network_actions: Vec::new(),
            available_runtime_actions: Vec::new(),
            primary_problem: None,
            active_devnet: None,
            workspace_root: String::new(),
            summary: LocalNodeSummary {
                total: 3,
                installed: 0,
                running: 0,
                needs_configuration: 3,
            },
            nodes: ATTACHED_NODES
                .iter()
                .map(|spec| LocalNodeStatus {
                    kind: spec.kind,
                    key: spec.kind.as_str().to_owned(),
                    label: spec.label().to_owned(),
                    install_state: "needs_configuration".to_owned(),
                    run_state: "unknown".to_owned(),
                    ownership: "external".to_owned(),
                    endpoint: None,
                    data_dir: None,
                    config_path: None,
                    initialization_configuration_ready: None,
                    package_path: None,
                    package_version: None,
                    managed_channel_id: None,
                    indexer_state: None,
                    indexer_head: None,
                    indexer_error: None,
                    process_id: None,
                    last_action: None,
                    available_actions: Vec::new(),
                    detail: String::new(),
                })
                .collect(),
            operations: Vec::new(),
            tools: super::super::model::LocalNodeTools {
                logoscore: super::super::model::ToolStatus {
                    available: true,
                    command: "logoscore".to_owned(),
                    path: None,
                },
                lgpd: super::super::model::ToolStatus {
                    available: false,
                    command: "lgpd".to_owned(),
                    path: None,
                },
                lgpm: super::super::model::ToolStatus {
                    available: false,
                    command: "lgpm".to_owned(),
                    path: None,
                },
            },
            runtime: super::super::runtime::LogoscoreRuntimeStatus {
                ownership: "local_attached".to_owned(),
                run_state: "running".to_owned(),
                id: None,
                detail: String::new(),
                process_id: None,
                binary_path: None,
                config_dir: None,
                modules_dir: None,
                persistence_path: None,
                service_unit: None,
            },
        };
        let versions = loaded_module_versions(&client)?;
        for spec in ATTACHED_NODES {
            let node = report
                .nodes
                .iter_mut()
                .find(|node| node.kind == spec.kind)
                .context("missing report node")?;
            overlay_node(
                node,
                spec,
                versions
                    .get(spec.module)
                    .context("missing module version")?,
                inspect_node(&client, spec),
                Some(format!(
                    "/var/lib/logoscore/inspector/attached-node-configs/{}.json",
                    spec.kind.as_str()
                )),
                Some(false),
            );
        }
        refresh_summary(&mut report);
        anyhow::ensure!(
            report
                .nodes
                .iter()
                .all(|node| node.ownership == "local_attached")
        );
        let messaging = report
            .nodes
            .iter()
            .find(|node| node.kind == NodeKind::Messaging)
            .context("missing messaging node")?;
        anyhow::ensure!(
            messaging.run_state == "uninitialized"
                && messaging.available_actions == vec![NodeAction::Initialize]
                && messaging.initialization_configuration_ready == Some(false)
                && messaging.config_path.as_deref()
                    == Some("/var/lib/logoscore/inspector/attached-node-configs/messaging.json")
        );
        Ok(())
    }
}
