use std::{
    collections::HashMap,
    io::{BufRead as _, BufReader},
    process::{Command, Stdio},
    sync::{Mutex, MutexGuard, OnceLock},
    thread,
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::{
    source_routing::{messaging_layer, storage_layer},
    support::time::now_millis,
};

use super::{
    NodeAction, NodeKind, NodeLifecycleState,
    commands::managed_action,
    model::LocalNodesState,
    process::{process_is_alive, stop_process},
    runtime::LogoscoreRuntimeProfile,
};

static STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
static WATCHERS: OnceLock<Mutex<HashMap<String, u32>>> = OnceLock::new();

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LifecycleTarget {
    pub(super) network_id: String,
    pub(super) kind: NodeKind,
    pub(super) action: NodeAction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ModuleEventSpec {
    module: &'static str,
    event: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct ModuleLifecycleEvent {
    target: LifecycleTarget,
    success: bool,
    detail: String,
}

#[derive(Debug, Clone)]
pub(super) struct LifecycleWatchRegistration {
    key: String,
    process_id: u32,
}

pub(super) fn acquire_state_lock() -> Result<MutexGuard<'static, ()>> {
    STATE_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|_| anyhow::anyhow!("local node state lock is poisoned"))
}

#[must_use]
pub(super) fn has_event_contract(kind: NodeKind, action: NodeAction) -> bool {
    event_spec(kind, action).is_some()
}

pub(super) fn start_event_watch(
    runtime: &LogoscoreRuntimeProfile,
    target: LifecycleTarget,
    on_event: impl Fn(ModuleLifecycleEvent) -> Result<()> + Send + 'static,
) -> Result<LifecycleWatchRegistration> {
    let spec = event_spec(target.kind, target.action).context("no lifecycle event contract")?;
    let key = watcher_key(runtime, &target, &spec);
    {
        let watchers = watcher_registry()?;
        if watchers.contains_key(&key) {
            bail!(
                "a lifecycle event watch is already active for {}",
                target.kind.label()
            );
        }
    }

    let mut command = runtime
        .cli_runtime()?
        .watch_command(spec.module, spec.event);
    let mut child = spawn_watch_command(&mut command, target.kind.label())?;
    let process_id = child.id();
    let stdout = child
        .stdout
        .take()
        .context("logoscore lifecycle watch did not expose stdout")?;
    {
        let mut watchers = watcher_registry()?;
        if watchers.insert(key.clone(), process_id).is_some() {
            if process_is_alive(process_id) {
                let _ignored = stop_process(process_id);
            }
            bail!(
                "a lifecycle event watch is already active for {}",
                target.kind.label()
            );
        }
    }

    let thread_key = key.clone();
    thread::Builder::new()
        .name(format!("logoscore-{}-watch", target.kind.as_str()))
        .spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines().map_while(Result::ok) {
                let Ok(Some(event)) = parse_watch_event(&target, &spec, &line) else {
                    continue;
                };
                let _ignored = on_event(event);
                break;
            }
            if process_is_alive(process_id) {
                let _ignored = stop_process(process_id);
            }
            let _ignored = child.wait();
            if let Ok(mut watchers) = watcher_registry() {
                watchers.remove(&thread_key);
            }
        })
        .context("failed to start logoscore lifecycle watch worker")?;

    Ok(LifecycleWatchRegistration { key, process_id })
}

pub(super) fn cancel_event_watch(registration: LifecycleWatchRegistration) {
    let _key = registration.key;
    if process_is_alive(registration.process_id) {
        let _ignored = stop_process(registration.process_id);
    }
}

pub(super) fn apply_event(state: &mut LocalNodesState, event: &ModuleLifecycleEvent) -> bool {
    let Some(record) = state.devnet_mut(&event.target.network_id) else {
        return false;
    };
    let Some(node) = record
        .nodes
        .iter_mut()
        .find(|node| node.kind == event.target.kind)
    else {
        return false;
    };
    if node.pending_lifecycle_action != Some(event.target.action) {
        return false;
    }

    node.pending_lifecycle_action = None;
    node.lifecycle_state = if event.success {
        match event.target.action {
            NodeAction::Start => NodeLifecycleState::Running,
            NodeAction::Stop => NodeLifecycleState::Stopped,
            _ => NodeLifecycleState::Unknown,
        }
    } else {
        NodeLifecycleState::Failed
    };
    record.updated_at = now_millis();
    true
}

pub(super) fn reset_module_contexts(state: &mut LocalNodesState) {
    for record in &mut state.devnets {
        for node in &mut record.nodes {
            if node.kind == NodeKind::Sequencer {
                continue;
            }
            node.installed = false;
            node.process_id = None;
            node.lifecycle_state = NodeLifecycleState::NotInitialized;
            node.pending_lifecycle_action = None;
        }
        record.updated_at = now_millis();
    }
}

fn event_spec(kind: NodeKind, action: NodeAction) -> Option<ModuleEventSpec> {
    let action = managed_action(action)?;
    let (module, event) = match kind {
        NodeKind::Storage => (
            storage_layer::module_id(),
            storage_layer::managed_lifecycle_event(action)?,
        ),
        NodeKind::Messaging => (
            messaging_layer::module_id(),
            messaging_layer::managed_lifecycle_event(action)?,
        ),
        NodeKind::Bedrock | NodeKind::Sequencer | NodeKind::Indexer => return None,
    };
    Some(ModuleEventSpec { module, event })
}

fn parse_watch_event(
    target: &LifecycleTarget,
    spec: &ModuleEventSpec,
    line: &str,
) -> Result<Option<ModuleLifecycleEvent>> {
    let value: Value = serde_json::from_str(line).context("invalid logoscore watch JSON")?;
    if value.get("module").and_then(Value::as_str) != Some(spec.module)
        || value.get("event").and_then(Value::as_str) != Some(spec.event)
    {
        return Ok(None);
    }

    let data = value
        .get("data")
        .and_then(Value::as_object)
        .context("logoscore lifecycle event has no data object")?;
    let outcome = match target.kind {
        NodeKind::Storage => storage_layer::managed_lifecycle_outcome(data.get("arg0"))?,
        NodeKind::Messaging => {
            messaging_layer::managed_lifecycle_outcome(data.get("arg0"), data.get("arg1"))?
        }
        _ => return Ok(None),
    };
    Ok(Some(ModuleLifecycleEvent {
        target: target.clone(),
        success: outcome.success,
        detail: outcome.detail,
    }))
}

fn spawn_watch_command(command: &mut Command, label: &str) -> Result<std::process::Child> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;

        command.process_group(0);
    }
    command
        .spawn()
        .with_context(|| format!("failed to start logoscore lifecycle watch for {label}"))
}

fn watcher_key(
    runtime: &LogoscoreRuntimeProfile,
    target: &LifecycleTarget,
    spec: &ModuleEventSpec,
) -> String {
    format!(
        "{}:{}:{}:{}",
        runtime.config_dir, target.network_id, spec.module, spec.event
    )
}

fn watcher_registry() -> Result<MutexGuard<'static, HashMap<String, u32>>> {
    WATCHERS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| anyhow::anyhow!("logoscore lifecycle watch registry is poisoned"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_nodes::LocalNodeConfigRecord;

    fn target(kind: NodeKind, action: NodeAction) -> LifecycleTarget {
        LifecycleTarget {
            network_id: "devnet".to_owned(),
            kind,
            action,
        }
    }

    #[test]
    fn storage_start_event_transitions_pending_node_to_running() -> Result<()> {
        let target = target(NodeKind::Storage, NodeAction::Start);
        let spec = event_spec(target.kind, target.action).context("missing event spec")?;
        let line = serde_json::json!({
            "module": "storage_module",
            "event": "storageStart",
            "data": {
                "arg0": "{\"success\":true,\"message\":\"ready\"}",
            },
        })
        .to_string();
        let event = parse_watch_event(&target, &spec, &line)?.context("missing parsed event")?;
        let mut state = state_with_pending(target.clone());

        if !apply_event(&mut state, &event) {
            bail!("storage event did not update the pending node");
        }
        let node = state
            .devnets
            .first()
            .and_then(|record| record.nodes.first())
            .context("missing storage node")?;
        if node.lifecycle_state != NodeLifecycleState::Running
            || node.pending_lifecycle_action.is_some()
            || event.detail != "ready"
        {
            bail!("unexpected storage lifecycle state: {node:?}");
        }
        Ok(())
    }

    #[test]
    fn delivery_stop_failure_transitions_pending_node_to_failed() -> Result<()> {
        let target = target(NodeKind::Messaging, NodeAction::Stop);
        let spec = event_spec(target.kind, target.action).context("missing event spec")?;
        let event = parse_watch_event(
            &target,
            &spec,
            r#"{"module":"delivery_module","event":"nodeStopped","data":{"arg0":false,"arg1":"shutdown failed"}}"#,
        )?
        .context("missing parsed event")?;
        let mut state = state_with_pending(target);

        if !apply_event(&mut state, &event) {
            bail!("delivery event did not update the pending node");
        }
        let state = state
            .devnets
            .first()
            .and_then(|record| record.nodes.first())
            .map(|node| node.lifecycle_state)
            .context("missing delivery node")?;
        if state != NodeLifecycleState::Failed || event.detail != "shutdown failed" {
            bail!("unexpected delivery lifecycle state: {state:?}");
        }
        Ok(())
    }

    #[test]
    fn ignores_event_for_other_module() -> Result<()> {
        let target = target(NodeKind::Storage, NodeAction::Start);
        let spec = event_spec(target.kind, target.action).context("missing event spec")?;

        if parse_watch_event(
            &target,
            &spec,
            r#"{"module":"delivery_module","event":"nodeStarted","data":{"arg0":true}}"#,
        )?
        .is_some()
        {
            bail!("unrelated module event was accepted");
        }
        Ok(())
    }

    fn state_with_pending(target: LifecycleTarget) -> LocalNodesState {
        LocalNodesState {
            version: 1,
            active_devnet: Some(target.network_id.clone()),
            managed_workspace_root: "/tmp/local-nodes".to_owned(),
            devnets: vec![super::super::model::LocalDevnetRecord {
                id: target.network_id,
                label: "Devnet".to_owned(),
                workspace: "/tmp/local-nodes/devnet".to_owned(),
                manifest_path: "/tmp/local-nodes/devnet/local-network.json".to_owned(),
                created_at: 0,
                updated_at: 0,
                nodes: vec![LocalNodeConfigRecord {
                    kind: target.kind,
                    config_path: "/tmp/config.json".to_owned(),
                    data_dir: "/tmp/data".to_owned(),
                    endpoint: None,
                    port: None,
                    package_path: None,
                    module_path: None,
                    process_id: None,
                    installed: true,
                    lifecycle_state: match target.action {
                        NodeAction::Start => NodeLifecycleState::Starting,
                        NodeAction::Stop => NodeLifecycleState::Stopping,
                        _ => NodeLifecycleState::Unknown,
                    },
                    pending_lifecycle_action: Some(target.action),
                }],
            }],
            operations: Vec::new(),
        }
    }
}
