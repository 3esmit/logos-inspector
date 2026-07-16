use std::{
    collections::BTreeSet,
    io::{ErrorKind, Read as _, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::{Arc, Mutex, mpsc},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    modules::logos_core::{LogoscoreCliRuntime, LogoscoreCliTransport, ModuleTransportEvent},
    source_routing::{ManagedNodeAction, ManagedNodeContract},
    support::{command_runner::CommandControl, time::now_millis},
};

use super::{
    NodeAction, NodeKind, NodeLifecycleState,
    action_engine::LocalNodeActionEngine,
    action_workspace::{MessagingContextProbe, probe_messaging_context, write_devnet_manifest},
    adapters::{NodeLifecycle, adapter_for},
    lifecycle::acquire_state_lock,
    model::{LocalDevnetRecord, LocalNodeConfigRecord, LocalNodesState},
    process::process_group_has_live_members,
    runtime::LogoscoreRuntimeProfile,
};

const DAEMON_POLL_INTERVAL: Duration = Duration::from_secs(10);
const DAEMON_RETRY_INTERVAL: Duration = Duration::from_secs(5);
const PENDING_LIFECYCLE_POLL_INTERVAL: Duration = Duration::from_secs(1);
const DAEMON_STATUS_TIMEOUT: Duration = Duration::from_secs(2);
const NODE_CONNECT_TIMEOUT: Duration = Duration::from_millis(250);
const MESSAGING_CONTEXT_PROBE_TIMEOUT: Duration = Duration::from_secs(1);
const MESSAGING_HEALTH_TIMEOUT: Duration = Duration::from_secs(1);
const MESSAGING_HEALTH_RESPONSE_LIMIT: u64 = 64 * 1024;
const SUBSCRIBER_QUEUE_CAPACITY: usize = 128;
const LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS: u64 = 30_000;
const RUNTIME_MODULE: &str = "logoscore_runtime";
const STORAGE_LISTEN_PORT: u16 = 8091;
const INDEXER_WATCHER_MODULE: &str = "indexer_service";
const SEQUENCER_WATCHER_MODULE: &str = "sequencer_service";

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
    port: Option<u16>,
    lifecycle_state: NodeLifecycleState,
    module: &'static str,
    event_route: LifecycleEventRoute,
    requires_module_context: bool,
    process_backed: bool,
    module_availability: ModuleAvailability,
    alive: bool,
    liveness_known: bool,
    unavailable_detail: Option<&'static str>,
}

#[derive(Debug, Clone, Copy)]
enum LifecycleEventRoute {
    Contract {
        contract: &'static ManagedNodeContract,
        started_event: &'static str,
        stopped_event: &'static str,
    },
    Synthetic,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MessagingHealth {
    Ready,
    Initializing,
    ShuttingDown,
    EventLoopLagging,
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
        lifecycle.retain(|current| !replaces_lifecycle_snapshot(current, event));
        lifecycle.push(event.clone());
    }
    baseline.extend(lifecycle);
    baseline
}

fn replaces_lifecycle_snapshot(
    current: &ModuleTransportEvent,
    replacement: &ModuleTransportEvent,
) -> bool {
    current.module() == replacement.module()
        && match (
            synthetic_lifecycle_identity(current),
            synthetic_lifecycle_identity(replacement),
        ) {
            (Some(current), Some(replacement)) => current == replacement,
            _ => true,
        }
}

fn synthetic_lifecycle_identity(event: &ModuleTransportEvent) -> Option<(&str, &str)> {
    event
        .args()
        .iter()
        .find_map(|arg| Some((arg.get("node")?.as_str()?, arg.get("network_id")?.as_str()?)))
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
        let (state, runtime) = {
            let _state_lock = acquire_state_lock()?;
            (self.store.load()?, self.runtime_profile()?)
        };
        let observation = observe_modules(&state, runtime.as_ref(), cancellation);
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

fn observe_modules(
    state: &LocalNodesState,
    runtime_profile: Option<&LogoscoreRuntimeProfile>,
    cancellation: &CancellationToken,
) -> ModuleObservation {
    let managed_runtime = runtime_profile.filter(|profile| profile.is_managed());
    let runtime = managed_runtime
        .map(LogoscoreRuntimeProfile::cli_runtime)
        .transpose()
        .and_then(|runtime| runtime.map_or_else(|| LogoscoreCliTransport::default().runtime(), Ok));
    let Ok(runtime) = runtime else {
        return unavailable_observation(state, managed_runtime.is_some());
    };
    let now = Instant::now();
    let deadline = now.checked_add(DAEMON_STATUS_TIMEOUT).unwrap_or(now);
    let control = CommandControl::new(cancellation.clone(), deadline);
    let output = runtime.status_controlled(control);
    let Ok(output) = output else {
        return unavailable_observation(state, managed_runtime.is_some());
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
    let mut probes =
        collect_liveness_probes(state, &loaded_modules, daemon != DaemonState::Unavailable);
    if managed_runtime.is_some() {
        reconcile_messaging_liveness(&mut probes, &runtime, cancellation);
    }
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

fn unavailable_observation(
    state: &LocalNodesState,
    managed_runtime_unavailable: bool,
) -> ModuleObservation {
    let mut probes = collect_liveness_probes(state, &BTreeSet::new(), false);
    if managed_runtime_unavailable {
        for probe in probes
            .iter_mut()
            .filter(|probe| probe.requires_module_context)
        {
            probe.liveness_known = false;
            probe.unavailable_detail = Some("Inspector-managed runtime status is unavailable");
        }
    }
    ModuleObservation {
        daemon: DaemonState::Unavailable,
        loaded_modules: BTreeSet::new(),
        needs_fast_poll: probes.iter().any(NodeLivenessProbe::is_pending),
        probes,
    }
}

fn reconcile_messaging_liveness(
    probes: &mut [NodeLivenessProbe],
    runtime: &LogoscoreCliRuntime,
    cancellation: &CancellationToken,
) {
    for probe in probes.iter_mut().filter(|probe| {
        probe.kind == NodeKind::Messaging && probe.module_availability == ModuleAvailability::Loaded
    }) {
        let health = apply_messaging_health_probe(probe);
        if probe.is_pending()
            || !matches!(
                health,
                MessagingHealth::Ready
                    | MessagingHealth::Initializing
                    | MessagingHealth::EventLoopLagging
            )
        {
            continue;
        }
        let now = Instant::now();
        let deadline = now
            .checked_add(MESSAGING_CONTEXT_PROBE_TIMEOUT)
            .unwrap_or(now);
        let control = CommandControl::new(cancellation.clone(), deadline);
        match probe_messaging_context(runtime, control) {
            MessagingContextProbe::Available => {}
            context => apply_messaging_context_probe(probe, context),
        }
    }
}

fn apply_messaging_context_probe(probe: &mut NodeLivenessProbe, context: MessagingContextProbe) {
    match context {
        MessagingContextProbe::Available => {}
        MessagingContextProbe::Absent => {
            probe.alive = false;
            probe.unavailable_detail =
                Some("Messaging context is not initialized in the Inspector-managed runtime");
        }
        MessagingContextProbe::Unknown => {
            probe.liveness_known = false;
            probe.unavailable_detail =
                Some("Messaging context could not be verified in the Inspector-managed runtime");
        }
    }
}

fn apply_messaging_health_probe(probe: &mut NodeLivenessProbe) -> MessagingHealth {
    let Some(port) = liveness_port_for_kind(probe.kind, probe.port) else {
        probe.liveness_known = false;
        probe.unavailable_detail = Some("Messaging REST health endpoint has no configured port");
        return MessagingHealth::Unknown;
    };
    let health = messaging_health(port);
    apply_messaging_health(probe, health);
    health
}

fn apply_messaging_health(probe: &mut NodeLivenessProbe, health: MessagingHealth) {
    match health {
        MessagingHealth::Ready | MessagingHealth::EventLoopLagging => {
            probe.alive = true;
            probe.liveness_known = true;
            probe.unavailable_detail = None;
        }
        MessagingHealth::Initializing => {
            probe.alive = false;
            probe.liveness_known = true;
            probe.unavailable_detail = Some("Messaging REST health is INITIALIZING");
        }
        MessagingHealth::ShuttingDown => {
            probe.alive = false;
            probe.liveness_known = false;
            probe.unavailable_detail = Some("Messaging REST health is SHUTTING_DOWN");
        }
        MessagingHealth::Unavailable => {
            probe.alive = false;
            probe.liveness_known = true;
            probe.unavailable_detail = Some("Messaging REST health endpoint is unavailable");
        }
        MessagingHealth::Unknown => {
            probe.liveness_known = false;
            probe.unavailable_detail = Some("Messaging REST health could not be verified");
        }
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
    state
        .testnet
        .iter()
        .chain(state.devnets.iter())
        .flat_map(|record| {
            record.nodes.iter().filter_map(|node| {
                liveness_probe(state, record, node, loaded_modules, module_status_known)
            })
        })
        .collect()
}

fn liveness_probe(
    state: &LocalNodesState,
    record: &LocalDevnetRecord,
    node: &LocalNodeConfigRecord,
    loaded_modules: &BTreeSet<String>,
    module_status_known: bool,
) -> Option<NodeLivenessProbe> {
    let lifecycle = adapter_for(node.kind).lifecycle();
    let requires_module_context = matches!(lifecycle, NodeLifecycle::InitializedModule(_));
    if requires_module_context {
        if state.module_context_topology_id(node.kind) != Some(record.id.as_str())
            || !node.installed
            || !node.lifecycle_state.has_module_context()
        {
            return None;
        }
    } else if !node.installed && node.process_id.is_none() {
        return None;
    }
    let (module, event_route) = lifecycle_event_route(node.kind)?;
    let process_backed = matches!(lifecycle, NodeLifecycle::RegisteredProcess { .. });
    let (alive, liveness_known, unavailable_detail) = if process_backed {
        (
            node.process_id.is_some_and(process_group_has_live_members),
            true,
            None,
        )
    } else if node.kind == NodeKind::Messaging {
        (
            false,
            false,
            Some("Messaging lifecycle requires managed REST health verification"),
        )
    } else {
        (tcp_port_is_open(liveness_port(node)?), true, None)
    };
    Some(NodeLivenessProbe {
        topology_id: record.id.clone(),
        record_updated_at: record.updated_at,
        kind: node.kind,
        port: node.port,
        lifecycle_state: node.lifecycle_state,
        module,
        event_route,
        requires_module_context,
        process_backed,
        module_availability: if module_status_known && requires_module_context {
            if loaded_modules.contains(module) {
                ModuleAvailability::Loaded
            } else {
                ModuleAvailability::Unavailable
            }
        } else {
            ModuleAvailability::Unknown
        },
        alive,
        liveness_known,
        unavailable_detail,
    })
}

fn lifecycle_event_route(kind: NodeKind) -> Option<(&'static str, LifecycleEventRoute)> {
    match adapter_for(kind).lifecycle() {
        NodeLifecycle::InitializedModule(contract) => {
            let route = match (
                contract.lifecycle_event(ManagedNodeAction::Start),
                contract.lifecycle_event(ManagedNodeAction::Stop),
            ) {
                (Some(started_event), Some(stopped_event)) => LifecycleEventRoute::Contract {
                    contract,
                    started_event,
                    stopped_event,
                },
                _ => LifecycleEventRoute::Synthetic,
            };
            Some((contract.module_id(), route))
        }
        NodeLifecycle::RegisteredProcess { .. } => match kind {
            NodeKind::Indexer => Some((INDEXER_WATCHER_MODULE, LifecycleEventRoute::Synthetic)),
            NodeKind::Sequencer => Some((SEQUENCER_WATCHER_MODULE, LifecycleEventRoute::Synthetic)),
            NodeKind::Bedrock | NodeKind::Storage | NodeKind::Messaging => None,
        },
    }
}

fn liveness_port(node: &LocalNodeConfigRecord) -> Option<u16> {
    liveness_port_for_kind(node.kind, node.port)
}

fn liveness_port_for_kind(kind: NodeKind, configured_port: Option<u16>) -> Option<u16> {
    match kind {
        NodeKind::Storage => Some(STORAGE_LISTEN_PORT),
        NodeKind::Bedrock | NodeKind::Sequencer | NodeKind::Indexer | NodeKind::Messaging => {
            configured_port.or_else(|| adapter_for(kind).default_port())
        }
    }
}

fn tcp_port_is_open(port: u16) -> bool {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    TcpStream::connect_timeout(&address, NODE_CONNECT_TIMEOUT).is_ok()
}

fn messaging_health(port: u16) -> MessagingHealth {
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let mut stream = match TcpStream::connect_timeout(&address, NODE_CONNECT_TIMEOUT) {
        Ok(stream) => stream,
        Err(error) => {
            return match error.kind() {
                ErrorKind::ConnectionRefused
                | ErrorKind::ConnectionAborted
                | ErrorKind::NotConnected => MessagingHealth::Unavailable,
                _ => MessagingHealth::Unknown,
            };
        }
    };
    if stream
        .set_read_timeout(Some(MESSAGING_HEALTH_TIMEOUT))
        .is_err()
        || stream
            .set_write_timeout(Some(MESSAGING_HEALTH_TIMEOUT))
            .is_err()
    {
        return MessagingHealth::Unknown;
    }
    let request =
        format!("GET /health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nConnection: close\r\n\r\n");
    if stream.write_all(request.as_bytes()).is_err() {
        return MessagingHealth::Unknown;
    }
    let mut response = Vec::new();
    if stream
        .take(MESSAGING_HEALTH_RESPONSE_LIMIT)
        .read_to_end(&mut response)
        .is_err()
    {
        return MessagingHealth::Unknown;
    }
    messaging_health_from_http_response(&response)
}

fn messaging_health_from_http_response(response: &[u8]) -> MessagingHealth {
    let Ok(response) = std::str::from_utf8(response) else {
        return MessagingHealth::Unknown;
    };
    let Some((head, body)) = response.split_once("\r\n\r\n") else {
        return MessagingHealth::Unknown;
    };
    if !head.starts_with("HTTP/1.1 200") && !head.starts_with("HTTP/1.0 200") {
        return MessagingHealth::Unknown;
    }
    let Ok(value) = serde_json::from_str::<Value>(body) else {
        return MessagingHealth::Unknown;
    };
    match value.get("nodeHealth").and_then(Value::as_str) {
        Some("READY") => MessagingHealth::Ready,
        Some("INITIALIZING") => MessagingHealth::Initializing,
        Some("SHUTTING_DOWN") => MessagingHealth::ShuttingDown,
        Some("EVENT_LOOP_LAGGING") => MessagingHealth::EventLoopLagging,
        _ => MessagingHealth::Unknown,
    }
}

fn apply_liveness_observation(
    state: &mut LocalNodesState,
    probes: &[NodeLivenessProbe],
) -> Result<(bool, Vec<ModuleTransportEvent>)> {
    let mut changed_records = BTreeSet::new();
    let mut accepted_record_versions = BTreeSet::new();
    let mut events = Vec::new();
    let observed_at = now_millis();
    for probe in probes {
        if probe.requires_module_context
            && state.module_context_topology_id(probe.kind) != Some(probe.topology_id.as_str())
        {
            continue;
        }
        let Some(record) = state.topology_mut(&probe.topology_id) else {
            continue;
        };
        if !accepted_record_versions.contains(&probe.topology_id)
            && record.updated_at != probe.record_updated_at
        {
            continue;
        }
        accepted_record_versions.insert(probe.topology_id.clone());
        let Some(node) = record.nodes.iter_mut().find(|node| node.kind == probe.kind) else {
            continue;
        };
        let confirmation_timed_out = observed_at.saturating_sub(probe.record_updated_at)
            >= LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS;
        let event = if !probe.liveness_known {
            match (node.lifecycle_state, node.pending_lifecycle_action) {
                (NodeLifecycleState::Starting, Some(NodeAction::Start))
                    if confirmation_timed_out =>
                {
                    node.lifecycle_state = NodeLifecycleState::Failed;
                    node.pending_lifecycle_action = None;
                    Some(lifecycle_event(
                        probe,
                        true,
                        false,
                        probe.unavailable_detail.unwrap_or(
                            "node liveness could not be verified before confirmation timeout",
                        ),
                    )?)
                }
                (NodeLifecycleState::Stopping, Some(NodeAction::Stop))
                    if confirmation_timed_out =>
                {
                    node.lifecycle_state = NodeLifecycleState::Failed;
                    node.pending_lifecycle_action = None;
                    Some(lifecycle_event(
                        probe,
                        false,
                        false,
                        probe.unavailable_detail.unwrap_or(
                            "node liveness could not be verified before confirmation timeout",
                        ),
                    )?)
                }
                _ => None,
            }
        } else {
            match (
                node.lifecycle_state,
                node.pending_lifecycle_action,
                probe.alive,
            ) {
                (NodeLifecycleState::Starting, Some(NodeAction::Start), true)
                | (NodeLifecycleState::Unknown | NodeLifecycleState::Failed, None, true) => {
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
                    if probe.process_backed =>
                {
                    node.process_id = None;
                    node.lifecycle_state = NodeLifecycleState::Failed;
                    node.pending_lifecycle_action = None;
                    Some(lifecycle_event(
                        probe,
                        true,
                        false,
                        "Inspector-owned process exited before lifecycle confirmation",
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
                        probe.unavailable_detail.unwrap_or(
                            "endpoint did not become reachable before confirmation timeout",
                        ),
                    )?)
                }
                (NodeLifecycleState::Stopping, Some(NodeAction::Stop), false) => {
                    if probe.process_backed {
                        node.process_id = None;
                    }
                    node.lifecycle_state = NodeLifecycleState::Stopped;
                    node.pending_lifecycle_action = None;
                    Some(lifecycle_event(
                        probe,
                        false,
                        true,
                        probe.unavailable_detail.unwrap_or("endpoint is closed"),
                    )?)
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
                        probe
                            .unavailable_detail
                            .unwrap_or("endpoint did not close before confirmation timeout"),
                    )?)
                }
                (NodeLifecycleState::Running, None, false) => {
                    if probe.process_backed {
                        node.process_id = None;
                    }
                    node.lifecycle_state = NodeLifecycleState::Unknown;
                    Some(unavailable_event(
                        probe,
                        probe.unavailable_detail.unwrap_or(
                            if probe.module_availability == ModuleAvailability::Unavailable {
                                "module is unavailable and endpoint is closed"
                            } else {
                                "endpoint is no longer reachable"
                            },
                        ),
                    )?)
                }
                (NodeLifecycleState::Unknown, None, false) => {
                    if probe.process_backed {
                        node.process_id = None;
                    }
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
            }
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
    match probe.event_route {
        LifecycleEventRoute::Contract {
            contract,
            started_event,
            stopped_event,
        } => contract_lifecycle_event(
            probe,
            contract,
            started_event,
            stopped_event,
            started,
            success,
            detail,
        ),
        LifecycleEventRoute::Synthetic => {
            synthetic_lifecycle_event(probe, started, success, detail)
        }
    }
}

fn contract_lifecycle_event(
    probe: &NodeLivenessProbe,
    contract: &'static ManagedNodeContract,
    started_event: &'static str,
    stopped_event: &'static str,
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
    let outcome = contract
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
            started_event
        } else {
            stopped_event
        },
        args,
    )
}

fn synthetic_lifecycle_event(
    probe: &NodeLivenessProbe,
    started: bool,
    success: bool,
    detail: &str,
) -> Result<ModuleTransportEvent> {
    ModuleTransportEvent::new(
        probe.module,
        if started {
            "nodeStarted"
        } else {
            "nodeStopped"
        },
        vec![json!({
            "node": probe.kind.as_str(),
            "network_id": probe.topology_id,
            "success": success,
            "simulated": true,
            "source": "poll",
            "message": detail,
            "timestamp": now_millis(),
        })],
    )
}

fn unavailable_event(probe: &NodeLivenessProbe, detail: &str) -> Result<ModuleTransportEvent> {
    ModuleTransportEvent::new(
        probe.module,
        "nodeUnavailable",
        vec![json!({
            "node": probe.kind.as_str(),
            "network_id": probe.topology_id,
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
                    port: adapter_for(kind).default_port(),
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

    fn only_node(state: &LocalNodesState) -> Result<&LocalNodeConfigRecord> {
        state
            .devnets
            .first()
            .and_then(|record| record.nodes.first())
            .context("missing node")
    }

    fn probe_for(
        kind: NodeKind,
        lifecycle_state: NodeLifecycleState,
        alive: bool,
        module_availability: ModuleAvailability,
    ) -> Result<NodeLivenessProbe> {
        let (module, event_route) = lifecycle_event_route(kind)
            .with_context(|| format!("{} event route is missing", kind.as_str()))?;
        Ok(NodeLivenessProbe {
            topology_id: "devnet".to_owned(),
            record_updated_at: 0,
            kind,
            port: adapter_for(kind).default_port(),
            lifecycle_state,
            module,
            event_route,
            requires_module_context: adapter_for(kind).managed_contract().is_some(),
            process_backed: matches!(
                adapter_for(kind).lifecycle(),
                NodeLifecycle::RegisteredProcess { .. }
            ),
            module_availability,
            alive,
            liveness_known: true,
            unavailable_detail: None,
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
    fn liveness_start_synthesizes_bedrock_started_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Bedrock,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let probe = probe_for(
            NodeKind::Bedrock,
            NodeLifecycleState::Starting,
            true,
            ModuleAvailability::Loaded,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(
            events[0].module() == "blockchain_module" && events[0].event() == "nodeStarted"
        );
        anyhow::ensure!(
            events[0].args()[0].get("node").and_then(Value::as_str) == Some("bedrock")
                && events[0].args()[0]
                    .get("network_id")
                    .and_then(Value::as_str)
                    == Some("devnet")
                && events[0].args()[0]
                    .get("simulated")
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
    fn failed_bedrock_context_recovers_when_its_endpoint_becomes_reachable() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Bedrock,
            NodeLifecycleState::Failed,
            None,
        );
        let probes = collect_liveness_probes(
            &state,
            &BTreeSet::from(["blockchain_module".to_owned()]),
            true,
        );
        anyhow::ensure!(probes.len() == 1);

        let probe = probe_for(
            NodeKind::Bedrock,
            NodeLifecycleState::Failed,
            true,
            ModuleAvailability::Loaded,
        )?;
        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(
            events[0].module() == "blockchain_module" && events[0].event() == "nodeStarted"
        );
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Running
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn liveness_start_synthesizes_indexer_started_event() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Starting,
            true,
            ModuleAvailability::Unknown,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(
            events[0].module() == INDEXER_WATCHER_MODULE && events[0].event() == "nodeStarted"
        );
        anyhow::ensure!(events[0].args()[0].get("node").and_then(Value::as_str) == Some("indexer"));
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Running
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn liveness_stop_confirms_indexer_process_group_exit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        state.devnets[0].nodes[0].process_id = Some(4242);
        let probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            false,
            ModuleAvailability::Unknown,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(
            events[0].module() == INDEXER_WATCHER_MODULE && events[0].event() == "nodeStopped"
        );
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Stopped
                && node.pending_lifecycle_action.is_none()
                && node.process_id.is_none()
        );
        Ok(())
    }

    #[test]
    fn liveness_stop_waits_for_indexer_process_group_exit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        state.devnets[0].nodes[0].process_id = Some(4242);
        let updated_at = now_millis();
        state.devnets[0].updated_at = updated_at;
        let mut probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            true,
            ModuleAvailability::Unknown,
        )?;
        probe.record_updated_at = updated_at;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(!changed && events.is_empty());
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Stopping
                && node.pending_lifecycle_action == Some(NodeAction::Stop)
                && node.process_id == Some(4242)
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
    fn messaging_health_parses_semantic_node_state() {
        let response = |node_health: &str| {
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{{\"nodeHealth\":\"{node_health}\"}}"
            )
        };

        assert_eq!(
            messaging_health_from_http_response(response("READY").as_bytes()),
            MessagingHealth::Ready
        );
        assert_eq!(
            messaging_health_from_http_response(response("INITIALIZING").as_bytes()),
            MessagingHealth::Initializing
        );
        assert_eq!(
            messaging_health_from_http_response(response("SHUTTING_DOWN").as_bytes()),
            MessagingHealth::ShuttingDown
        );
        assert_eq!(
            messaging_health_from_http_response(response("EVENT_LOOP_LAGGING").as_bytes()),
            MessagingHealth::EventLoopLagging
        );
        assert_eq!(
            messaging_health_from_http_response(b"HTTP/1.1 503 Service Unavailable\r\n\r\n{}"),
            MessagingHealth::Unknown
        );
    }

    #[test]
    fn messaging_initializing_health_does_not_promote_stopped_node() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Stopped,
            None,
        );
        let mut probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Stopped,
            true,
            ModuleAvailability::Loaded,
        )?;

        apply_messaging_health(&mut probe, MessagingHealth::Initializing);
        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(!changed && events.is_empty());
        let node = only_node(&state)?;
        anyhow::ensure!(node.lifecycle_state == NodeLifecycleState::Stopped);
        Ok(())
    }

    #[test]
    fn messaging_ready_health_confirms_pending_start() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let mut probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            false,
            ModuleAvailability::Loaded,
        )?;

        apply_messaging_health(&mut probe, MessagingHealth::Ready);
        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(
            events
                .first()
                .is_some_and(|event| event.event() == "nodeStarted")
        );
        let node = only_node(&state)?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Running
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn messaging_shutting_down_health_keeps_pending_stop_unconfirmed() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        let updated_at = now_millis();
        state
            .devnets
            .first_mut()
            .context("missing devnet")?
            .updated_at = updated_at;
        let mut probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Stopping,
            true,
            ModuleAvailability::Loaded,
        )?;
        probe.record_updated_at = updated_at;

        apply_messaging_health(&mut probe, MessagingHealth::ShuttingDown);
        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(!changed && events.is_empty());
        let node = only_node(&state)?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Stopping
                && node.pending_lifecycle_action == Some(NodeAction::Stop)
        );
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
    fn missing_messaging_context_does_not_treat_a_foreign_listener_as_running() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Running,
            None,
        );
        let mut probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Running,
            true,
            ModuleAvailability::Loaded,
        )?;

        apply_messaging_context_probe(&mut probe, MessagingContextProbe::Absent);
        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(events[0].event() == "nodeUnavailable");
        anyhow::ensure!(
            events[0]
                .args()
                .first()
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.contains("not initialized"))
        );
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(node.lifecycle_state == NodeLifecycleState::Unknown);
        Ok(())
    }

    #[test]
    fn unknown_messaging_context_fails_pending_start_after_confirmation_timeout() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let current = now_millis();
        state.devnets[0].updated_at = current;
        let mut probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Starting,
            true,
            ModuleAvailability::Loaded,
        )?;
        probe.record_updated_at = current;
        apply_messaging_context_probe(&mut probe, MessagingContextProbe::Unknown);

        let (pending_changed, pending_events) =
            apply_liveness_observation(&mut state, std::slice::from_ref(&probe))?;

        anyhow::ensure!(!pending_changed && pending_events.is_empty());
        let pending_node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            pending_node.lifecycle_state == NodeLifecycleState::Starting
                && pending_node.pending_lifecycle_action == Some(NodeAction::Start)
        );

        let expired = now_millis().saturating_sub(LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS + 1);
        state.devnets[0].updated_at = expired;
        probe.record_updated_at = expired;
        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(events[0].event() == "nodeStarted");
        anyhow::ensure!(events[0].args().first() == Some(&Value::Bool(false)));
        anyhow::ensure!(
            events[0]
                .args()
                .get(1)
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.contains("could not be verified"))
        );
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Failed
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn unknown_messaging_context_fails_pending_stop_after_confirmation_timeout() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        let expired = now_millis().saturating_sub(LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS + 1);
        state.devnets[0].updated_at = expired;
        let mut probe = probe_for(
            NodeKind::Messaging,
            NodeLifecycleState::Stopping,
            true,
            ModuleAvailability::Loaded,
        )?;
        probe.record_updated_at = expired;
        apply_messaging_context_probe(&mut probe, MessagingContextProbe::Unknown);

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        anyhow::ensure!(events[0].event() == "nodeStopped");
        anyhow::ensure!(events[0].args().first() == Some(&Value::Bool(false)));
        anyhow::ensure!(
            events[0]
                .args()
                .get(1)
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.contains("could not be verified"))
        );
        let node = state.devnets[0].nodes.first().context("missing node")?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Failed
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn unavailable_managed_runtime_never_uses_tcp_as_module_liveness() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let state = state_with_node(
            &directory,
            NodeKind::Messaging,
            NodeLifecycleState::Stopped,
            None,
        );

        let observation = unavailable_observation(&state, true);
        let probe = observation
            .probes
            .first()
            .context("missing Messaging liveness probe")?;

        anyhow::ensure!(
            !probe.liveness_known
                && probe.unavailable_detail
                    == Some("Inspector-managed runtime status is unavailable")
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
    fn bound_bedrock_context_is_polled_with_synthetic_lifecycle_route() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let state = state_with_node(
            &directory,
            NodeKind::Bedrock,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );

        let probes = collect_liveness_probes(
            &state,
            &BTreeSet::from(["blockchain_module".to_owned()]),
            true,
        );

        anyhow::ensure!(probes.len() == 1);
        anyhow::ensure!(
            probes[0].module == "blockchain_module"
                && matches!(probes[0].event_route, LifecycleEventRoute::Synthetic)
                && probes[0].requires_module_context
        );
        Ok(())
    }

    #[test]
    fn detached_nodes_are_polled_without_module_context() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Running,
            None,
        );
        state
            .module_context_topology_by_kind
            .remove(&NodeKind::Indexer);
        let mut sequencer = state.devnets[0].nodes[0].clone();
        sequencer.kind = NodeKind::Sequencer;
        sequencer.port = adapter_for(NodeKind::Sequencer).default_port();
        state.devnets[0].nodes.push(sequencer);

        let probes = collect_liveness_probes(&state, &BTreeSet::new(), false);

        anyhow::ensure!(probes.len() == 2);
        anyhow::ensure!(probes.iter().all(|probe| {
            probe.process_backed
                && !probe.requires_module_context
                && matches!(probe.event_route, LifecycleEventRoute::Synthetic)
        }));
        Ok(())
    }

    #[test]
    fn liveness_batch_confirms_all_nodes_from_one_topology_snapshot() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Bedrock,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        for kind in [NodeKind::Messaging, NodeKind::Storage] {
            let mut node = state.devnets[0].nodes[0].clone();
            node.kind = kind;
            node.port = adapter_for(kind).default_port();
            state.devnets[0].nodes.push(node);
            state
                .module_context_topology_by_kind
                .insert(kind, "devnet".to_owned());
        }
        let probes = [
            probe_for(
                NodeKind::Bedrock,
                NodeLifecycleState::Starting,
                true,
                ModuleAvailability::Loaded,
            )?,
            probe_for(
                NodeKind::Messaging,
                NodeLifecycleState::Starting,
                true,
                ModuleAvailability::Loaded,
            )?,
            probe_for(
                NodeKind::Storage,
                NodeLifecycleState::Starting,
                true,
                ModuleAvailability::Loaded,
            )?,
        ];

        let (changed, events) = apply_liveness_observation(&mut state, &probes)?;

        anyhow::ensure!(changed && events.len() == 3);
        anyhow::ensure!(
            state.devnets[0]
                .nodes
                .iter()
                .all(|node| node.lifecycle_state == NodeLifecycleState::Running)
        );
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
    fn store_migration_binds_an_unambiguous_bedrock_context() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Bedrock,
            NodeLifecycleState::Running,
            None,
        );
        state
            .module_context_topology_by_kind
            .remove(&NodeKind::Bedrock);

        anyhow::ensure!(state.infer_unambiguous_module_context_topologies());
        anyhow::ensure!(state.module_context_topology_id(NodeKind::Bedrock) == Some("devnet"));
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
    fn lifecycle_snapshot_keeps_detached_nodes_for_each_network() -> Result<()> {
        let alpha = ModuleTransportEvent::new(
            INDEXER_WATCHER_MODULE,
            "nodeStarted",
            vec![json!({ "node": "indexer", "network_id": "alpha" })],
        )?;
        let beta = ModuleTransportEvent::new(
            INDEXER_WATCHER_MODULE,
            "nodeStarted",
            vec![json!({ "node": "indexer", "network_id": "beta" })],
        )?;
        let alpha_stopped = ModuleTransportEvent::new(
            INDEXER_WATCHER_MODULE,
            "nodeStopped",
            vec![json!({ "node": "indexer", "network_id": "alpha" })],
        )?;

        let snapshot = merge_snapshot(&[alpha], Vec::new(), &[beta]);
        let snapshot = merge_snapshot(&snapshot, Vec::new(), &[alpha_stopped]);

        anyhow::ensure!(snapshot.len() == 2);
        anyhow::ensure!(snapshot.iter().any(|event| {
            event.event() == "nodeStopped"
                && event.args()[0].get("network_id").and_then(Value::as_str) == Some("alpha")
        }));
        anyhow::ensure!(snapshot.iter().any(|event| {
            event.event() == "nodeStarted"
                && event.args()[0].get("network_id").and_then(Value::as_str) == Some("beta")
        }));
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
