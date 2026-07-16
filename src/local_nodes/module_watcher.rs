use std::{
    collections::BTreeSet,
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    modules::logos_core::{LogoscoreCliTransport, ModuleTransportEvent},
    source_routing::{ManagedNodeAction, ManagedNodeContract},
    support::{command_runner::CommandControl, time::now_millis},
};

use super::{
    NodeAction, NodeKind, NodeLifecycleState,
    action_engine::LocalNodeActionEngine,
    action_workspace::write_devnet_manifest,
    adapters::adapter_for,
    lifecycle::acquire_state_lock,
    model::{LocalDevnetRecord, LocalNodeConfigRecord, LocalNodesState},
};

const DAEMON_POLL_INTERVAL: Duration = Duration::from_secs(10);
const DAEMON_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const PENDING_LIFECYCLE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DAEMON_STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const NODE_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const SUBSCRIBER_QUEUE_CAPACITY: usize = 128;
const LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS: u64 = 30_000;
const RUNTIME_MODULE: &str = "logoscore_runtime";
const STORAGE_LISTEN_PORT: u16 = 8091;

/// Polling fallback for LogosCore installations whose CLI event stream is not
/// safe or available. The watcher owns daemon/module polling, lifecycle
/// endpoint checks, transition synthesis, and fanout behind start, subscribe,
/// and stop.
pub struct LocalNodeModuleWatcher {
    cancellation: CancellationToken,
    worker: Option<thread::JoinHandle<()>>,
    subscribers: Arc<Mutex<SubscriberHub>>,
}

/// Receives synthesized module events from a [`LocalNodeModuleWatcher`].
pub struct LocalNodeModuleSubscription {
    receiver: mpsc::Receiver<ModuleTransportEvent>,
}

#[derive(Default)]
struct SubscriberHub {
    senders: Vec<mpsc::SyncSender<ModuleTransportEvent>>,
    snapshot: Vec<ModuleTransportEvent>,
}

impl LocalNodeModuleWatcher {
    /// Starts polling before any module is available so later daemon changes are observed.
    pub fn start() -> Result<Self> {
        let cancellation = CancellationToken::new();
        let worker_cancellation = cancellation.clone();
        let subscribers = Arc::new(Mutex::new(SubscriberHub::default()));
        let worker_subscribers = Arc::clone(&subscribers);
        let worker = thread::Builder::new()
            .name("logoscore-module-poll-watcher".to_owned())
            .spawn(move || run_module_watcher(worker_cancellation, worker_subscribers))
            .context("failed to start LogosCore module polling watcher")?;
        Ok(Self {
            cancellation,
            worker: Some(worker),
            subscribers,
        })
    }

    /// Creates a bounded event subscription for lifecycle and module transitions.
    pub fn subscribe(&self) -> Result<LocalNodeModuleSubscription> {
        let mut subscribers = self
            .subscribers
            .lock()
            .map_err(|_| anyhow::anyhow!("LogosCore module watcher subscribers are unavailable"))?;
        subscribe_to_hub(&mut subscribers)
    }

    /// Stops polling and waits for the worker to exit.
    pub fn stop(&mut self) -> Result<()> {
        self.cancellation.cancel();
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("LogosCore module polling watcher panicked"))?;
        }
        let mut subscribers = self
            .subscribers
            .lock()
            .map_err(|_| anyhow::anyhow!("LogosCore module watcher subscribers are unavailable"))?;
        subscribers.senders.clear();
        subscribers.snapshot.clear();
        Ok(())
    }
}

impl Drop for LocalNodeModuleWatcher {
    fn drop(&mut self) {
        let _result = self.stop();
    }
}

impl LocalNodeModuleSubscription {
    /// Waits for one event without blocking longer than `timeout`.
    pub fn next_within(&mut self, timeout: Duration) -> Result<Option<ModuleTransportEvent>> {
        match self.receiver.recv_timeout(timeout) {
            Ok(event) => Ok(Some(event)),
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("LogosCore module watcher subscription is closed")
            }
        }
    }
}

fn subscribe_to_hub(hub: &mut SubscriberHub) -> Result<LocalNodeModuleSubscription> {
    let capacity = SUBSCRIBER_QUEUE_CAPACITY.max(hub.snapshot.len().saturating_add(1));
    let (sender, receiver) = mpsc::sync_channel(capacity);
    for event in &hub.snapshot {
        sender.send(event.clone()).map_err(|_| {
            anyhow::anyhow!("LogosCore module watcher subscription closed during setup")
        })?;
    }
    hub.senders.push(sender);
    Ok(LocalNodeModuleSubscription { receiver })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DaemonState {
    Running,
    Stopped,
    Unavailable,
}

#[derive(Default)]
struct WatcherObservation {
    daemon: Option<DaemonState>,
    loaded_modules: BTreeSet<String>,
}

pub(super) struct ModuleWatcherPoll {
    daemon: DaemonState,
    loaded_modules: BTreeSet<String>,
    lifecycle_events: Vec<ModuleTransportEvent>,
    poll_interval: Duration,
}

struct NodeLivenessProbe {
    topology_id: String,
    record_updated_at: u64,
    kind: NodeKind,
    lifecycle_state: NodeLifecycleState,
    contract: &'static ManagedNodeContract,
    module: &'static str,
    started_event: &'static str,
    stopped_event: &'static str,
    module_availability: ModuleAvailability,
    alive: bool,
}

impl NodeLivenessProbe {
    fn is_pending(&self) -> bool {
        self.lifecycle_state.is_pending()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModuleAvailability {
    Loaded,
    Unavailable,
    Unknown,
}

fn run_module_watcher(cancellation: CancellationToken, subscribers: Arc<Mutex<SubscriberHub>>) {
    let mut observation = WatcherObservation::default();
    while !cancellation.is_cancelled() {
        let poll = LocalNodeActionEngine::system()
            .and_then(|engine| engine.poll_module_watcher(&cancellation));
        if cancellation.is_cancelled() {
            break;
        }
        let interval = match poll {
            Ok(poll) => {
                let snapshot = snapshot_events(&poll);
                let mut events = observation_events(&mut observation, &poll);
                events.extend(poll.lifecycle_events.iter().cloned());
                publish_events(&subscribers, events, snapshot);
                poll.poll_interval
            }
            Err(_) => DAEMON_RETRY_INTERVAL,
        };
        wait_for_poll_interval(&cancellation, interval);
    }
}

fn observation_events(
    observation: &mut WatcherObservation,
    poll: &ModuleWatcherPoll,
) -> Vec<ModuleTransportEvent> {
    let previous_daemon = observation.daemon.replace(poll.daemon);
    let mut events = Vec::new();
    match (previous_daemon, poll.daemon) {
        (None, DaemonState::Running) | (Some(DaemonState::Stopped), DaemonState::Running) => {
            append_runtime_event(&mut events, "daemonStarted", "daemon is reachable");
        }
        (None, DaemonState::Stopped) | (Some(DaemonState::Running), DaemonState::Stopped) => {
            append_runtime_event(&mut events, "daemonStopped", "daemon reported stopped");
        }
        (Some(DaemonState::Running), DaemonState::Unavailable)
        | (Some(DaemonState::Stopped), DaemonState::Unavailable)
        | (None, DaemonState::Unavailable) => {
            append_runtime_event(
                &mut events,
                "daemonUnavailable",
                "daemon status poll failed",
            );
        }
        (Some(DaemonState::Unavailable), DaemonState::Running) => {
            append_runtime_event(&mut events, "daemonStarted", "daemon became reachable");
        }
        (Some(DaemonState::Unavailable), DaemonState::Stopped) => {
            append_runtime_event(&mut events, "daemonStopped", "daemon reported stopped");
        }
        _ => {}
    }

    if poll.daemon != DaemonState::Unavailable {
        let previous_modules =
            std::mem::replace(&mut observation.loaded_modules, poll.loaded_modules.clone());
        for module in poll.loaded_modules.difference(&previous_modules) {
            append_module_status_event(&mut events, module, "moduleReady", "loaded");
        }
        for module in previous_modules.difference(&poll.loaded_modules) {
            append_module_status_event(&mut events, module, "moduleUnavailable", "unavailable");
        }
    }
    events
}

fn snapshot_events(poll: &ModuleWatcherPoll) -> Vec<ModuleTransportEvent> {
    let mut events = Vec::new();
    match poll.daemon {
        DaemonState::Running => {
            append_runtime_event(&mut events, "daemonStarted", "daemon is reachable");
        }
        DaemonState::Stopped => {
            append_runtime_event(&mut events, "daemonStopped", "daemon reported stopped");
        }
        DaemonState::Unavailable => {
            append_runtime_event(
                &mut events,
                "daemonUnavailable",
                "daemon status poll failed",
            );
        }
    }
    for module in &poll.loaded_modules {
        append_module_status_event(&mut events, module, "moduleReady", "loaded");
    }
    events
}

fn append_module_status_event(
    events: &mut Vec<ModuleTransportEvent>,
    module: &str,
    event_name: &str,
    status: &str,
) {
    let event = ModuleTransportEvent::new(
        module,
        event_name,
        vec![json!({
            "simulated": true,
            "source": "poll",
            "status": status,
            "timestamp": now_millis(),
        })],
    );
    if let Ok(event) = event {
        events.push(event);
    }
}

fn append_runtime_event(events: &mut Vec<ModuleTransportEvent>, name: &str, detail: &str) {
    let event = ModuleTransportEvent::new(
        RUNTIME_MODULE,
        name,
        vec![json!({
            "simulated": true,
            "source": "poll",
            "message": detail,
            "timestamp": now_millis(),
        })],
    );
    if let Ok(event) = event {
        events.push(event);
    }
}

fn publish_events(
    subscribers: &Arc<Mutex<SubscriberHub>>,
    events: Vec<ModuleTransportEvent>,
    snapshot: Vec<ModuleTransportEvent>,
) {
    let Ok(mut subscribers) = subscribers.lock() else {
        return;
    };
    subscribers.snapshot = merge_snapshot(&subscribers.snapshot, snapshot, &events);
    if events.is_empty() {
        return;
    }
    subscribers.senders.retain(|subscriber| {
        for event in &events {
            match subscriber.try_send(event.clone()) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(_)) => break,
                Err(mpsc::TrySendError::Disconnected(_)) => return false,
            }
        }
        true
    });
}

fn merge_snapshot(
    previous: &[ModuleTransportEvent],
    mut baseline: Vec<ModuleTransportEvent>,
    events: &[ModuleTransportEvent],
) -> Vec<ModuleTransportEvent> {
    let mut lifecycle = previous
        .iter()
        .filter(|event| is_lifecycle_status_event(event))
        .cloned()
        .collect::<Vec<_>>();
    for event in events
        .iter()
        .filter(|event| is_lifecycle_status_event(event))
    {
        lifecycle.retain(|current| current.module() != event.module());
        lifecycle.push(event.clone());
    }
    baseline.extend(lifecycle);
    baseline
}

fn is_lifecycle_status_event(event: &ModuleTransportEvent) -> bool {
    matches!(
        event.event(),
        "nodeStarted" | "nodeStopped" | "nodeUnavailable" | "storageStart" | "storageStop"
    )
}

fn wait_for_poll_interval(cancellation: &CancellationToken, interval: Duration) {
    let deadline = Instant::now()
        .checked_add(interval)
        .unwrap_or_else(Instant::now);
    while !cancellation.is_cancelled() && Instant::now() < deadline {
        thread::sleep(
            Duration::from_millis(25).min(deadline.saturating_duration_since(Instant::now())),
        );
    }
}

impl LocalNodeActionEngine {
    pub(super) fn poll_module_watcher(
        &self,
        cancellation: &CancellationToken,
    ) -> Result<ModuleWatcherPoll> {
        let state = {
            let _state_lock = acquire_state_lock()?;
            self.store.load()?
        };
        let observation = observe_modules(&state, cancellation);
        let lifecycle_events = self.apply_liveness_observation(&observation.probes)?;
        Ok(ModuleWatcherPoll {
            daemon: observation.daemon,
            loaded_modules: observation.loaded_modules,
            lifecycle_events,
            poll_interval: if observation.needs_fast_poll {
                PENDING_LIFECYCLE_POLL_INTERVAL
            } else if observation.daemon == DaemonState::Unavailable {
                DAEMON_RETRY_INTERVAL
            } else {
                DAEMON_POLL_INTERVAL
            },
        })
    }

    fn apply_liveness_observation(
        &self,
        probes: &[NodeLivenessProbe],
    ) -> Result<Vec<ModuleTransportEvent>> {
        let _state_lock = acquire_state_lock()?;
        let mut state = self.store.load()?;
        let (changed, events) = apply_liveness_observation(&mut state, probes)?;
        if changed {
            self.store.save(&state)?;
        }
        Ok(events)
    }
}

struct ModuleObservation {
    daemon: DaemonState,
    loaded_modules: BTreeSet<String>,
    probes: Vec<NodeLivenessProbe>,
    needs_fast_poll: bool,
}

fn observe_modules(state: &LocalNodesState, cancellation: &CancellationToken) -> ModuleObservation {
    let runtime = LogoscoreCliTransport::default().runtime();
    let Ok(runtime) = runtime else {
        return unavailable_observation(state);
    };
    let now = Instant::now();
    let deadline = now.checked_add(DAEMON_STATUS_TIMEOUT).unwrap_or(now);
    let control = CommandControl::new(cancellation.clone(), deadline);
    let output = runtime.status_controlled(control);
    let Ok(output) = output else {
        return unavailable_observation(state);
    };
    let daemon = match output
        .value
        .pointer("/daemon/status")
        .and_then(Value::as_str)
    {
        Some("running") => DaemonState::Running,
        Some("stopped") => DaemonState::Stopped,
        _ => DaemonState::Unavailable,
    };
    let loaded_modules = loaded_modules(&output.value);
    let probes =
        collect_liveness_probes(state, &loaded_modules, daemon != DaemonState::Unavailable);
    let needs_fast_poll = probes.iter().any(|probe| probe.is_pending())
        || (daemon == DaemonState::Stopped
            && probes
                .iter()
                .any(|probe| probe.lifecycle_state != NodeLifecycleState::Stopped));
    ModuleObservation {
        daemon,
        loaded_modules,
        probes,
        needs_fast_poll,
    }
}

fn unavailable_observation(state: &LocalNodesState) -> ModuleObservation {
    let probes = collect_liveness_probes(state, &BTreeSet::new(), false);
    ModuleObservation {
        daemon: DaemonState::Unavailable,
        loaded_modules: BTreeSet::new(),
        needs_fast_poll: probes.iter().any(NodeLivenessProbe::is_pending),
        probes,
    }
}

fn loaded_modules(value: &Value) -> BTreeSet<String> {
    value
        .get("modules")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|module| module.get("status").and_then(Value::as_str) == Some("loaded"))
        .filter_map(|module| module.get("name").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

fn collect_liveness_probes(
    state: &LocalNodesState,
    loaded_modules: &BTreeSet<String>,
    module_status_known: bool,
) -> Vec<NodeLivenessProbe> {
    [NodeKind::Messaging, NodeKind::Storage]
        .into_iter()
        .filter_map(|kind| {
            let record = state.module_context_topology(kind)?;
            let node = record.nodes.iter().find(|node| node.kind == kind)?;
            liveness_probe(record, node, loaded_modules, module_status_known)
        })
        .collect()
}

fn liveness_probe(
    record: &LocalDevnetRecord,
    node: &LocalNodeConfigRecord,
    loaded_modules: &BTreeSet<String>,
    module_status_known: bool,
) -> Option<NodeLivenessProbe> {
    if !node.installed || !node.lifecycle_state.has_module_context() {
        return None;
    }
    let port = match node.kind {
        NodeKind::Messaging => node.port.unwrap_or(8645),
        NodeKind::Storage => STORAGE_LISTEN_PORT,
        NodeKind::Bedrock | NodeKind::Sequencer | NodeKind::Indexer => return None,
    };
    let contract = adapter_for(node.kind).managed_contract()?;
    let module = contract.module_id();
    let started_event = contract.lifecycle_event(ManagedNodeAction::Start)?;
    let stopped_event = contract.lifecycle_event(ManagedNodeAction::Stop)?;
    Some(NodeLivenessProbe {
        topology_id: record.id.clone(),
        record_updated_at: record.updated_at,
        kind: node.kind,
        lifecycle_state: node.lifecycle_state,
        contract,
        module,
        started_event,
        stopped_event,
        module_availability: if module_status_known {
            if loaded_modules.contains(module) {
                ModuleAvailability::Loaded
            } else {
                ModuleAvailability::Unavailable
            }
        } else {
            ModuleAvailability::Unknown
        },
        alive: tcp_port_is_open(port),
    })
}

fn tcp_port_is_open(port: u16) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&address, NODE_CONNECT_TIMEOUT).is_ok()
}

fn apply_liveness_observation(
    state: &mut LocalNodesState,
    probes: &[NodeLivenessProbe],
) -> Result<(bool, Vec<ModuleTransportEvent>)> {
    let mut changed_records = BTreeSet::new();
    let mut events = Vec::new();
    let observed_at = now_millis();
    for probe in probes {
        if state.module_context_topology_id(probe.kind) != Some(probe.topology_id.as_str()) {
            continue;
        }
        let Some(record) = state.topology_mut(&probe.topology_id) else {
            continue;
        };
        if record.updated_at != probe.record_updated_at {
            continue;
        }
        let Some(node) = record.nodes.iter_mut().find(|node| node.kind == probe.kind) else {
            continue;
        };
        let confirmation_timed_out =
            observed_at.saturating_sub(record.updated_at) >= LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS;
        let event = match (
            node.lifecycle_state,
            node.pending_lifecycle_action,
            probe.alive,
        ) {
            (NodeLifecycleState::Starting, Some(NodeAction::Start), true)
            | (NodeLifecycleState::Unknown, None, true) => {
                node.lifecycle_state = NodeLifecycleState::Running;
                node.pending_lifecycle_action = None;
                Some(lifecycle_event(probe, true, true, "endpoint is reachable")?)
            }
            (NodeLifecycleState::Starting, Some(NodeAction::Start), false)
                if probe.module_availability == ModuleAvailability::Unavailable =>
            {
                node.lifecycle_state = NodeLifecycleState::Failed;
                node.pending_lifecycle_action = None;
                Some(lifecycle_event(
                    probe,
                    true,
                    false,
                    "module is unavailable and endpoint is closed",
                )?)
            }
            (NodeLifecycleState::Starting, Some(NodeAction::Start), false)
                if confirmation_timed_out =>
            {
                node.lifecycle_state = NodeLifecycleState::Failed;
                node.pending_lifecycle_action = None;
                Some(lifecycle_event(
                    probe,
                    true,
                    false,
                    "endpoint did not become reachable before confirmation timeout",
                )?)
            }
            (NodeLifecycleState::Stopping, Some(NodeAction::Stop), false) => {
                node.lifecycle_state = NodeLifecycleState::Stopped;
                node.pending_lifecycle_action = None;
                Some(lifecycle_event(probe, false, true, "endpoint is closed")?)
            }
            (NodeLifecycleState::Stopping, Some(NodeAction::Stop), true)
                if probe.module_availability == ModuleAvailability::Unavailable =>
            {
                node.lifecycle_state = NodeLifecycleState::Failed;
                node.pending_lifecycle_action = None;
                Some(lifecycle_event(
                    probe,
                    false,
                    false,
                    "module is unavailable while endpoint remains reachable",
                )?)
            }
            (NodeLifecycleState::Stopping, Some(NodeAction::Stop), true)
                if confirmation_timed_out =>
            {
                node.lifecycle_state = NodeLifecycleState::Failed;
                node.pending_lifecycle_action = None;
                Some(lifecycle_event(
                    probe,
                    false,
                    false,
                    "endpoint did not close before confirmation timeout",
                )?)
            }
            (NodeLifecycleState::Running, None, false) => {
                node.lifecycle_state = NodeLifecycleState::Unknown;
                Some(unavailable_event(
                    probe,
                    if probe.module_availability == ModuleAvailability::Unavailable {
                        "module is unavailable and endpoint is closed"
                    } else {
                        "endpoint is no longer reachable"
                    },
                )?)
            }
            (NodeLifecycleState::Unknown, None, false) => {
                node.lifecycle_state = NodeLifecycleState::Stopped;
                Some(lifecycle_event(
                    probe,
                    false,
                    true,
                    "endpoint remains closed",
                )?)
            }
            (NodeLifecycleState::Stopped, None, true) => {
                node.lifecycle_state = NodeLifecycleState::Running;
                Some(lifecycle_event(probe, true, true, "endpoint is reachable")?)
            }
            _ => None,
        };
        if let Some(event) = event {
            record.updated_at = observed_at;
            changed_records.insert(record.id.clone());
            events.push(event);
        }
    }
    for record_id in changed_records {
        let Some(record) = state.topology_mut(&record_id) else {
            continue;
        };
        write_devnet_manifest(record)?;
    }
    Ok((!events.is_empty(), events))
}

fn lifecycle_event(
    probe: &NodeLivenessProbe,
    started: bool,
    success: bool,
    detail: &str,
) -> Result<ModuleTransportEvent> {
    let marker = json!({
        "success": success,
        "simulated": true,
        "source": "poll",
        "timestamp": now_millis(),
    });
    let args = if probe.kind == NodeKind::Storage {
        let payload = json!({
            "success": success,
            "message": detail,
        });
        vec![
            Value::String(
                serde_json::to_string(&payload)
                    .context("failed to serialize simulated storage lifecycle event")?,
            ),
            marker,
        ]
    } else {
        vec![
            Value::Bool(success),
            Value::String(detail.to_owned()),
            marker,
        ]
    };
    let data = args
        .iter()
        .enumerate()
        .map(|(index, value)| (format!("arg{index}"), value.clone()))
        .collect::<Map<_, _>>();
    let outcome = probe
        .contract
        .decode_lifecycle_event(&data)
        .context("simulated lifecycle payload does not match the managed module contract")?;
    anyhow::ensure!(
        outcome.success == success && outcome.detail == detail,
        "simulated lifecycle payload did not validate for {}",
        probe.module
    );
    ModuleTransportEvent::new(
        probe.module,
        if started {
            probe.started_event
        } else {
            probe.stopped_event
        },
        args,
    )
}

fn unavailable_event(probe: &NodeLivenessProbe, detail: &str) -> Result<ModuleTransportEvent> {
    ModuleTransportEvent::new(
        probe.module,
        "nodeUnavailable",
        vec![json!({
            "simulated": true,
            "source": "poll",
            "message": detail,
            "timestamp": now_millis(),
        })],
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::{Context as _, Result};

    use super::*;
    use crate::local_nodes::{LocalNodeConfigRecord, LocalNodeDeployment};

    fn state_with_node(
        directory: &tempfile::TempDir,
        kind: NodeKind,
        lifecycle_state: NodeLifecycleState,
        pending: Option<NodeAction>,
    ) -> LocalNodesState {
        let workspace = directory.path().display().to_string();
        LocalNodesState {
            version: 3,
            active_devnet: Some("devnet".to_owned()),
            module_context_topology_by_kind: BTreeMap::from([(kind, "devnet".to_owned())]),
            testnet: None,
            managed_workspace_root: workspace.clone(),
            devnets: vec![LocalDevnetRecord {
                deployment: LocalNodeDeployment::LocalDevnet,
                id: "devnet".to_owned(),
                label: "Devnet".to_owned(),
                workspace: workspace.clone(),
                manifest_path: directory
                    .path()
                    .join("local-network.json")
                    .display()
                    .to_string(),
                created_at: 0,
                updated_at: 0,
                nodes: vec![LocalNodeConfigRecord {
                    kind,
                    config_path: directory.path().join("config.json").display().to_string(),
                    initialization_config_path: None,
                    data_dir: directory.path().join("data").display().to_string(),
                    endpoint: None,
                    port: Some(8645),
                    package_path: None,
                    module_path: None,
                    process_id: None,
                    installed: true,
                    lifecycle_state,
                    pending_lifecycle_action: pending,
                }],
            }],
            operations: Vec::new(),
        }
    }

    fn probe_for(
        kind: NodeKind,
        lifecycle_state: NodeLifecycleState,
        alive: bool,
        module_availability: ModuleAvailability,
    ) -> Result<NodeLivenessProbe> {
        let contract = super::adapter_for(kind)
            .managed_contract()
            .with_context(|| format!("{} managed contract is missing", kind.as_str()))?;
        let (module, started_event, stopped_event) = match kind {
            NodeKind::Messaging => ("delivery_module", "nodeStarted", "nodeStopped"),
            NodeKind::Storage => ("storage_module", "storageStart", "storageStop"),
            NodeKind::Bedrock | NodeKind::Sequencer | NodeKind::Indexer => {
                anyhow::bail!("{} has no pollable lifecycle contract", kind.as_str())
            }
        };
        Ok(NodeLivenessProbe {
            topology_id: "devnet".to_owned(),
            record_updated_at: 0,
            kind,
            lifecycle_state,
            contract,
            module,
            started_event,
            stopped_event,
            module_availability,
            alive,
        })
    }

    #[test]
    fn liveness_start_synthesizes_delivery_started_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            true,
            ModuleAvailability::Loaded,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(
            events[0].module() == "delivery_module" && events[0].event() == "nodeStarted"
        );
        anyhow::ensure!(
            events[0]
                .args()
                .iter()
                .find_map(|payload| payload.get("simulated"))
                .and_then(Value::as_bool)
                == Some(true)
        );
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Running
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn liveness_stop_synthesizes_storage_stopped_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Storage,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        let probe = probe_for(
            NodeKind::Storage,
            NodeLifecycleState::Stopping,
            false,
            ModuleAvailability::Loaded,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(
            events[0].module() == "storage_module" && events[0].event() == "storageStop"
        );
        anyhow::ensure!(events[0].args()[0].is_string());
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Stopped
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn repeated_closed_delivery_endpoint_synthesizes_stopped_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Running,
            None,
        );
        let first_probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Running,
            false,
            ModuleAvailability::Loaded,
        )?;

        let (first_changed, first_events) =
            apply_liveness_observation(&mut state, std::slice::from_ref(&first_probe))?;

        anyhow::ensure!(first_changed && first_events.len() == 1);
        anyhow::ensure!(
            first_events
                .first()
                .is_some_and(|event| event.event() == "nodeUnavailable")
        );
        let mut second_probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Unknown,
            false,
            ModuleAvailability::Loaded,
        )?;
        second_probe.record_updated_at = state.devnets[0].updated_at;

        let (second_changed, second_events) =
            apply_liveness_observation(&mut state, std::slice::from_ref(&second_probe))?;

        anyhow::ensure!(second_changed && second_events.len() == 1);
        anyhow::ensure!(
            second_events
                .first()
                .is_some_and(|event| event.event() == "nodeStopped")
        );
        let node = state
            .devnets
            .first()
            .and_then(|record| record.nodes.first())
            .context("missing node")?;
        anyhow::ensure!(node.lifecycle_state == NodeLifecycleState::Stopped);
        Ok(())
    }

    #[test]
    fn unavailable_module_fails_pending_start_without_leaving_it_stuck() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            false,
            ModuleAvailability::Unavailable,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(events[0].event() == "nodeStarted");
        anyhow::ensure!(events[0].args().first() == Some(&Value::Bool(false)));
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Failed
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn bound_stopped_endpoint_synthesizes_started_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Stopped,
            None,
        );
        let probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Stopped,
            true,
            ModuleAvailability::Loaded,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(events[0].event() == "nodeStarted");
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(node.lifecycle_state == NodeLifecycleState::Running);
        Ok(())
    }

    #[test]
    fn probes_only_use_the_bound_module_context() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Running,
            None,
        );
        let mut testnet = state.devnets[0].clone();
        testnet.id = "logos-testnet".to_owned();
        state.testnet = Some(testnet);
        state
            .module_context_topology_by_kind
            .insert(NodeKind::Messaging, "logos-testnet".to_owned());

        let probes = collect_liveness_probes(
            &state,
            &BTreeSet::from(["delivery_module".to_owned()]),
            true,
        );

        anyhow::ensure!(probes.len() == 1);
        anyhow::ensure!(probes[0].topology_id == "logos-testnet");
        Ok(())
    }

    #[test]
    fn unbound_module_context_is_not_probed() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Running,
            None,
        );
        state
            .module_context_topology_by_kind
            .remove(&NodeKind::Messaging);

        let probes = collect_liveness_probes(
            &state,
            &BTreeSet::from(["delivery_module".to_owned()]),
            true,
        );

        anyhow::ensure!(probes.is_empty());
        Ok(())
    }

    #[test]
    fn store_migration_binds_only_an_unambiguous_context() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Running,
            None,
        );
        state
            .module_context_topology_by_kind
            .remove(&NodeKind::Messaging);

        anyhow::ensure!(state.infer_unambiguous_module_context_topologies());
        anyhow::ensure!(state.module_context_topology_id(NodeKind::Messaging) == Some("devnet"));

        state
            .module_context_topology_by_kind
            .remove(&NodeKind::Messaging);
        let mut testnet = state.devnets[0].clone();
        testnet.id = "logos-testnet".to_owned();
        state.testnet = Some(testnet);

        anyhow::ensure!(!state.infer_unambiguous_module_context_topologies());
        anyhow::ensure!(
            state
                .module_context_topology_id(NodeKind::Messaging)
                .is_none()
        );
        Ok(())
    }

    #[test]
    fn late_subscription_replays_current_module_snapshot() -> Result<()> {
        let runtime_event = ModuleTransportEvent::new(
            RUNTIME_MODULE,
            "daemonStarted",
            vec![json!({ "simulated": true })],
        )?;
        let mut hub = SubscriberHub {
            senders: Vec::new(),
            snapshot: vec![runtime_event],
        };
        let mut subscription = subscribe_to_hub(&mut hub)?;

        let event = subscription
            .next_within(Duration::from_millis(25))?
            .context("missing replayed watcher snapshot")?;

        anyhow::ensure!(event.module() == RUNTIME_MODULE && event.event() == "daemonStarted");
        Ok(())
    }

    #[test]
    fn lifecycle_snapshot_keeps_only_the_latest_node_state() -> Result<()> {
        let started =
            ModuleTransportEvent::new("delivery_module", "nodeStarted", vec![Value::Bool(true)])?;
        let stopped =
            ModuleTransportEvent::new("delivery_module", "nodeStopped", vec![Value::Bool(true)])?;
        let baseline = vec![ModuleTransportEvent::new(
            RUNTIME_MODULE,
            "daemonStarted",
            vec![json!({ "simulated": true })],
        )?];

        let snapshot = merge_snapshot(&[started], baseline, &[stopped]);

        anyhow::ensure!(snapshot.iter().any(|event| event.event() == "nodeStopped"));
        anyhow::ensure!(!snapshot.iter().any(|event| event.event() == "nodeStarted"));
        Ok(())
    }

    #[test]
    fn watcher_observation_reports_module_ready_once() {
        let poll = ModuleWatcherPoll {
            daemon: DaemonState::Running,
            loaded_modules: ["delivery_module".to_owned()].into_iter().collect(),
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_POLL_INTERVAL,
        };
        let mut observation = WatcherObservation::default();

        let initial = observation_events(&mut observation, &poll);
        let repeat = observation_events(&mut observation, &poll);

        assert!(initial.iter().any(|event| event.event() == "daemonStarted"));
        assert!(initial.iter().any(|event| event.event() == "moduleReady"));
        assert!(repeat.is_empty());
    }

    #[test]
    fn watcher_observation_reports_initial_stopped_daemon() {
        let poll = ModuleWatcherPoll {
            daemon: DaemonState::Stopped,
            loaded_modules: BTreeSet::new(),
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_POLL_INTERVAL,
        };
        let mut observation = WatcherObservation::default();

        let events = observation_events(&mut observation, &poll);

        assert!(events.iter().any(|event| event.event() == "daemonStopped"));
    }

    #[test]
    fn unavailable_daemon_does_not_claim_modules_were_unloaded() {
        let loaded = ModuleWatcherPoll {
            daemon: DaemonState::Running,
            loaded_modules: BTreeSet::from(["delivery_module".to_owned()]),
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_POLL_INTERVAL,
        };
        let unavailable = ModuleWatcherPoll {
            daemon: DaemonState::Unavailable,
            loaded_modules: BTreeSet::new(),
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_RETRY_INTERVAL,
        };
        let mut observation = WatcherObservation::default();

        let _initial = observation_events(&mut observation, &loaded);
        let events = observation_events(&mut observation, &unavailable);

        assert!(
            events
                .iter()
                .any(|event| event.event() == "daemonUnavailable")
        );
        assert!(
            !events
                .iter()
                .any(|event| event.event() == "moduleUnavailable")
        );
    }
}
