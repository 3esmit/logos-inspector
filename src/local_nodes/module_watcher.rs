use std::{
    collections::BTreeSet,
    io::{ErrorKind, Read as _, Write as _},
    net::{Ipv4Addr, SocketAddr, TcpStream},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result};
use serde_json::{Map, Value, json};
use tokio_util::sync::CancellationToken;

use crate::{
    modules::logos_core::{
        LogoscoreCliRuntime, LogoscoreCliTransport, ModuleTransportEvent,
        module_transport_event_from_watch_frame, normalize_module_call_value,
    },
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
const INDEXER_STATUS_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const MESSAGING_HEALTH_TIMEOUT: Duration = Duration::from_secs(1);
const MESSAGING_HEALTH_RESPONSE_LIMIT: u64 = 64 * 1024;
const SUBSCRIBER_QUEUE_CAPACITY: usize = 128;
const LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS: u64 = 30_000;
const RUNTIME_MODULE: &str = "logoscore_runtime";
const DELIVERY_MODULE: &str = "delivery_module";
const DELIVERY_EVENT_STREAM_READY: &str = "eventStreamReady";
const DELIVERY_EVENT_STREAM_UNAVAILABLE: &str = "eventStreamUnavailable";
const DELIVERY_EVENT_STREAM_SOURCE: &str = "logoscore_cli_watch";
const DELIVERY_EVENT_READ_INTERVAL: Duration = Duration::from_millis(250);
const DELIVERY_WATCH_HEALTH_MAX_AGE: Duration = Duration::from_secs(15);
const DELIVERY_EVENT_REASON_LIMIT: usize = 512;
const DELIVERY_NATIVE_EVENTS: [&str; 5] = [
    "messageSent",
    "messageError",
    "messagePropagated",
    "messageReceived",
    "connectionStateChanged",
];
const STORAGE_LISTEN_PORT: u16 = 8091;
const INDEXER_WATCHER_MODULE: &str = "lez_indexer_module";
const SEQUENCER_WATCHER_MODULE: &str = "sequencer_service";

/// Owns daemon/module polling, lifecycle transition synthesis, the persistent
/// Delivery event stream, and bounded fanout behind start, subscribe, and stop.
pub struct LocalNodeModuleWatcher {
    cancellation: CancellationToken,
    poll_worker: Option<thread::JoinHandle<()>>,
    delivery_worker: Option<thread::JoinHandle<()>>,
    subscribers: Arc<Mutex<SubscriberHub>>,
}

type SharedDeliveryWatchHealth = Arc<Mutex<DeliveryWatchHealthObservation>>;

/// Receives synthesized module events from a [`LocalNodeModuleWatcher`].
pub struct LocalNodeModuleSubscription {
    receiver: mpsc::Receiver<ModuleTransportEvent>,
    queued: Arc<AtomicUsize>,
    delivery_gap_pending: Arc<AtomicBool>,
}

#[derive(Default)]
struct SubscriberHub {
    senders: Vec<SubscriberSender>,
    snapshot: Vec<ModuleTransportEvent>,
}

struct SubscriberSender {
    sender: mpsc::SyncSender<ModuleTransportEvent>,
    queued: Arc<AtomicUsize>,
    delivery_gap_pending: Arc<AtomicBool>,
    data_capacity: usize,
}

impl SubscriberSender {
    fn is_at_data_capacity(&self) -> bool {
        self.queued.load(Ordering::Acquire) >= self.data_capacity
    }

    fn can_accept_status(&self) -> bool {
        self.queued.load(Ordering::Acquire) < self.data_capacity.saturating_add(1)
    }

    fn try_send(
        &self,
        event: ModuleTransportEvent,
    ) -> std::result::Result<(), mpsc::TrySendError<ModuleTransportEvent>> {
        self.queued.fetch_add(1, Ordering::AcqRel);
        match self.sender.try_send(event) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.queued.fetch_sub(1, Ordering::AcqRel);
                Err(error)
            }
        }
    }

    fn send(
        &self,
        event: ModuleTransportEvent,
    ) -> std::result::Result<(), mpsc::SendError<ModuleTransportEvent>> {
        self.queued.fetch_add(1, Ordering::AcqRel);
        match self.sender.send(event) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.queued.fetch_sub(1, Ordering::AcqRel);
                Err(error)
            }
        }
    }

    fn has_delivery_gap_pending(&self) -> bool {
        self.delivery_gap_pending.load(Ordering::Acquire)
    }

    fn queue_delivery_gap(&self, event: ModuleTransportEvent) -> bool {
        if self
            .delivery_gap_pending
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return true;
        }
        if self.try_send(event).is_ok() {
            return true;
        }
        self.delivery_gap_pending.store(false, Ordering::Release);
        false
    }
}

impl LocalNodeModuleWatcher {
    /// Starts polling before any module is available so later daemon changes are observed.
    pub fn start() -> Result<Self> {
        let cancellation = CancellationToken::new();
        let delivery_watch_health = Arc::new(Mutex::new(DeliveryWatchHealthObservation::new(
            DeliveryWatchHealth::Unknown,
        )));
        let worker_cancellation = cancellation.clone();
        let subscribers = Arc::new(Mutex::new(SubscriberHub::default()));
        let worker_subscribers = Arc::clone(&subscribers);
        let worker_delivery_watch_health = Arc::clone(&delivery_watch_health);
        let worker = thread::Builder::new()
            .name("logoscore-module-poll-watcher".to_owned())
            .spawn(move || {
                run_module_watcher(
                    worker_cancellation,
                    worker_subscribers,
                    worker_delivery_watch_health,
                );
            })
            .context("failed to start LogosCore module polling watcher")?;
        let delivery_cancellation = cancellation.clone();
        let delivery_subscribers = Arc::clone(&subscribers);
        let delivery_worker_health = Arc::clone(&delivery_watch_health);
        let delivery_worker = match thread::Builder::new()
            .name("logoscore-delivery-event-watcher".to_owned())
            .spawn(move || {
                run_delivery_event_watcher(
                    delivery_cancellation,
                    delivery_subscribers,
                    delivery_worker_health,
                );
            }) {
            Ok(worker) => worker,
            Err(error) => {
                cancellation.cancel();
                let cleanup = worker.join();
                let mut error = anyhow::Error::new(error)
                    .context("failed to start LogosCore Delivery event watcher");
                if cleanup.is_err() {
                    error =
                        error.context("LogosCore module polling watcher panicked during cleanup");
                }
                return Err(error);
            }
        };
        Ok(Self {
            cancellation,
            poll_worker: Some(worker),
            delivery_worker: Some(delivery_worker),
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

    /// Stops polling and event streaming, then waits for both workers to exit.
    pub fn stop(&mut self) -> Result<()> {
        self.cancellation.cancel();
        let poll_result = self.poll_worker.take().map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("LogosCore module polling watcher panicked"))
        });
        let delivery_result = self.delivery_worker.take().map_or(Ok(()), |worker| {
            worker
                .join()
                .map_err(|_| anyhow::anyhow!("LogosCore Delivery event watcher panicked"))
        });
        let subscriber_result = self
            .subscribers
            .lock()
            .map_err(|_| anyhow::anyhow!("LogosCore module watcher subscribers are unavailable"))
            .map(|mut subscribers| {
                subscribers.senders.clear();
                subscribers.snapshot.clear();
            });
        poll_result.and(delivery_result).and(subscriber_result)
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
            Ok(event) => {
                let queued = self.queued.fetch_sub(1, Ordering::AcqRel);
                debug_assert!(queued > 0);
                if event.module() == DELIVERY_MODULE
                    && event.event() == DELIVERY_EVENT_STREAM_UNAVAILABLE
                {
                    self.delivery_gap_pending.store(false, Ordering::Release);
                }
                Ok(Some(event))
            }
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                anyhow::bail!("LogosCore module watcher subscription is closed")
            }
        }
    }
}

fn subscribe_to_hub(hub: &mut SubscriberHub) -> Result<LocalNodeModuleSubscription> {
    let data_capacity = SUBSCRIBER_QUEUE_CAPACITY.max(hub.snapshot.len().saturating_add(1));
    let (sender, receiver) = mpsc::sync_channel(data_capacity.saturating_add(1));
    let queued = Arc::new(AtomicUsize::new(0));
    let delivery_gap_pending = Arc::new(AtomicBool::new(false));
    let subscriber = SubscriberSender {
        sender,
        queued: Arc::clone(&queued),
        delivery_gap_pending: Arc::clone(&delivery_gap_pending),
        data_capacity,
    };
    for event in &hub.snapshot {
        subscriber.send(event.clone()).map_err(|_| {
            anyhow::anyhow!("LogosCore module watcher subscription closed during setup")
        })?;
    }
    hub.senders.push(subscriber);
    Ok(LocalNodeModuleSubscription {
        receiver,
        queued,
        delivery_gap_pending,
    })
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
    status_observed_at: Instant,
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
    indexer_status: Option<IndexerStatusObservation>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum IndexerModuleHealth {
    Running(IndexerStatusObservation),
    Stopped,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IndexerStatusObservation {
    state: String,
    indexed_block_id: Option<String>,
    last_error: Option<String>,
}

fn run_module_watcher(
    cancellation: CancellationToken,
    subscribers: Arc<Mutex<SubscriberHub>>,
    delivery_watch_health: SharedDeliveryWatchHealth,
) {
    let mut observation = WatcherObservation::default();
    while !cancellation.is_cancelled() {
        let poll = LocalNodeActionEngine::system()
            .and_then(|engine| engine.poll_module_watcher(&cancellation));
        if cancellation.is_cancelled() {
            break;
        }
        let interval = match poll {
            Ok(poll) => {
                record_delivery_watch_poll_health(&delivery_watch_health, &poll);
                let snapshot = snapshot_events(&poll);
                let mut events = observation_events(&mut observation, &poll);
                events.extend(poll.lifecycle_events.iter().cloned());
                publish_events(&subscribers, events, snapshot);
                poll.poll_interval
            }
            Err(_) => {
                record_delivery_watch_health(
                    &delivery_watch_health,
                    DeliveryWatchHealth::PollUnavailable,
                    Instant::now(),
                );
                DAEMON_RETRY_INTERVAL
            }
        };
        wait_for_poll_interval(&cancellation, interval);
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum DeliveryEventStreamState {
    #[default]
    Unknown,
    Ready,
    Unavailable,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
enum DeliveryWatchHealth {
    #[default]
    Unknown,
    Ready,
    DaemonStopped,
    DaemonUnavailable,
    ModuleUnavailable,
    PollUnavailable,
}

#[derive(Debug, Clone, Copy)]
struct DeliveryWatchHealthObservation {
    health: DeliveryWatchHealth,
    observed_at: Instant,
}

impl DeliveryWatchHealthObservation {
    fn new(health: DeliveryWatchHealth) -> Self {
        Self {
            health,
            observed_at: Instant::now(),
        }
    }
}

impl DeliveryWatchHealth {
    fn from_poll(poll: &ModuleWatcherPoll) -> Self {
        match poll.daemon {
            DaemonState::Stopped => Self::DaemonStopped,
            DaemonState::Unavailable => Self::DaemonUnavailable,
            DaemonState::Running if !poll.loaded_modules.contains(DELIVERY_MODULE) => {
                Self::ModuleUnavailable
            }
            DaemonState::Running => Self::Ready,
        }
    }

    fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            Self::Unknown | Self::Ready => None,
            Self::DaemonStopped => Some("LogosCore daemon stopped during Delivery event watch"),
            Self::DaemonUnavailable => {
                Some("LogosCore daemon became unavailable during Delivery event watch")
            }
            Self::ModuleUnavailable => Some("LogosCore Delivery module is no longer loaded"),
            Self::PollUnavailable => Some("LogosCore module status polling became unavailable"),
        }
    }
}

fn record_delivery_watch_poll_health(health: &SharedDeliveryWatchHealth, poll: &ModuleWatcherPoll) {
    record_delivery_watch_health(
        health,
        DeliveryWatchHealth::from_poll(poll),
        poll.status_observed_at,
    );
}

fn record_delivery_watch_health(
    health: &SharedDeliveryWatchHealth,
    next: DeliveryWatchHealth,
    observed_at: Instant,
) {
    if let Ok(mut current) = health.lock() {
        *current = DeliveryWatchHealthObservation {
            health: next,
            observed_at,
        };
    }
}

fn ensure_delivery_watch_health(health: &SharedDeliveryWatchHealth) -> Result<()> {
    let current = *health
        .lock()
        .map_err(|_| anyhow::anyhow!("LogosCore Delivery watch health is unavailable"))?;
    if let Some(reason) = current.health.unavailable_reason() {
        anyhow::bail!(reason);
    }
    anyhow::ensure!(
        current.observed_at.elapsed() <= DELIVERY_WATCH_HEALTH_MAX_AGE,
        "LogosCore module status polling is stale"
    );
    Ok(())
}

fn run_delivery_event_watcher(
    cancellation: CancellationToken,
    subscribers: Arc<Mutex<SubscriberHub>>,
    delivery_watch_health: SharedDeliveryWatchHealth,
) {
    let mut state = DeliveryEventStreamState::Unknown;
    while !cancellation.is_cancelled() {
        let result = watch_delivery_events_once(
            &cancellation,
            &subscribers,
            &delivery_watch_health,
            &mut state,
        );
        if cancellation.is_cancelled() {
            break;
        }
        let reason = result.err().map_or_else(
            || "event stream ended".to_owned(),
            |error| error.to_string(),
        );
        while !cancellation.is_cancelled()
            && !publish_delivery_stream_transition(
                &subscribers,
                &mut state,
                DeliveryEventStreamState::Unavailable,
                &reason,
            )
        {
            wait_for_poll_interval(&cancellation, DELIVERY_EVENT_READ_INTERVAL);
        }
        wait_for_poll_interval(&cancellation, DAEMON_RETRY_INTERVAL);
    }
}

fn watch_delivery_events_once(
    cancellation: &CancellationToken,
    subscribers: &Arc<Mutex<SubscriberHub>>,
    delivery_watch_health: &SharedDeliveryWatchHealth,
    state: &mut DeliveryEventStreamState,
) -> Result<()> {
    let runtime = ready_delivery_watch_runtime(cancellation)?;
    let startup_control = command_control(cancellation, DAEMON_STATUS_TIMEOUT);
    let mut watch = runtime.start_all_event_watch(DELIVERY_MODULE, &startup_control)?;
    let ready_result = watch.wait_ready(&startup_control);
    if let Err(error) = ready_result {
        let cleanup_result = watch.stop();
        return match cleanup_result {
            Ok(()) => Err(error),
            Err(cleanup) => Err(error.context(format!(
                "failed to clean up rejected LogosCore Delivery event watch: {cleanup:#}"
            ))),
        };
    }
    let stream_result = (|| {
        ensure_delivery_watch_health(delivery_watch_health)?;
        anyhow::ensure!(
            publish_delivery_stream_transition(
                subscribers,
                state,
                DeliveryEventStreamState::Ready,
                "subscription active",
            ),
            "Delivery event stream readiness could not reach every subscriber"
        );
        read_delivery_events(
            cancellation,
            subscribers,
            delivery_watch_health,
            &runtime,
            &mut watch,
        )
    })();
    let cleanup_result = watch.stop();
    stream_result.and(cleanup_result)
}

fn read_delivery_events(
    cancellation: &CancellationToken,
    subscribers: &Arc<Mutex<SubscriberHub>>,
    delivery_watch_health: &SharedDeliveryWatchHealth,
    runtime: &LogoscoreCliRuntime,
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
) -> Result<()> {
    let mut runtime_probe = Instant::now();
    while !cancellation.is_cancelled() {
        ensure_delivery_watch_health(delivery_watch_health)?;
        let control = command_control(cancellation, DAEMON_STATUS_TIMEOUT);
        if let Some(value) = watch.next_value_within(&control, DELIVERY_EVENT_READ_INTERVAL)? {
            let event = module_transport_event_from_watch_frame(&value, DELIVERY_MODULE)?;
            if DELIVERY_NATIVE_EVENTS.contains(&event.event()) {
                anyhow::ensure!(
                    publish_incremental_events(subscribers, vec![event]),
                    "Delivery subscriber queue overflowed"
                );
            }
        }
        if runtime_probe.elapsed() >= DAEMON_RETRY_INTERVAL {
            let current = delivery_watch_runtime()?;
            anyhow::ensure!(
                current == *runtime,
                "LogosCore Delivery event stream runtime changed"
            );
            runtime_probe = Instant::now();
        }
    }
    Ok(())
}

fn ready_delivery_watch_runtime(cancellation: &CancellationToken) -> Result<LogoscoreCliRuntime> {
    let runtime = delivery_watch_runtime()?;
    let control = command_control(cancellation, DAEMON_STATUS_TIMEOUT);
    let status = runtime.status_controlled(control)?;
    anyhow::ensure!(
        status
            .value
            .pointer("/daemon/status")
            .and_then(Value::as_str)
            == Some("running"),
        "LogosCore daemon is not running"
    );
    anyhow::ensure!(
        loaded_modules(&status.value).contains(DELIVERY_MODULE),
        "LogosCore Delivery module is not loaded"
    );
    Ok(runtime)
}

fn delivery_watch_runtime() -> Result<LogoscoreCliRuntime> {
    let engine = LocalNodeActionEngine::system()?;
    let profile = engine.runtime_profile()?;
    let managed = profile.as_ref().filter(|profile| profile.is_managed());
    managed
        .map(LogoscoreRuntimeProfile::cli_runtime)
        .transpose()?
        .map_or_else(|| LogoscoreCliTransport::default().runtime(), Ok)
}

fn command_control(cancellation: &CancellationToken, timeout: Duration) -> CommandControl {
    let now = Instant::now();
    let deadline = now.checked_add(timeout).unwrap_or(now);
    CommandControl::new(cancellation.clone(), deadline)
}

fn publish_delivery_stream_transition(
    subscribers: &Arc<Mutex<SubscriberHub>>,
    state: &mut DeliveryEventStreamState,
    next: DeliveryEventStreamState,
    reason: &str,
) -> bool {
    if *state == next || next == DeliveryEventStreamState::Unknown {
        return true;
    }
    let (event_name, status) = match next {
        DeliveryEventStreamState::Ready => (DELIVERY_EVENT_STREAM_READY, "ready"),
        DeliveryEventStreamState::Unavailable => (DELIVERY_EVENT_STREAM_UNAVAILABLE, "unavailable"),
        DeliveryEventStreamState::Unknown => return true,
    };
    let event = delivery_event_stream_status_event(event_name, status, reason);
    if let Ok(event) = event {
        let delivered = publish_incremental_events(subscribers, vec![event]);
        if delivered {
            *state = next;
        }
        return delivered;
    }
    false
}

fn delivery_event_stream_status_event(
    event_name: &str,
    status: &str,
    reason: &str,
) -> Result<ModuleTransportEvent> {
    ModuleTransportEvent::new(
        DELIVERY_MODULE,
        event_name,
        vec![json!({
            "source": DELIVERY_EVENT_STREAM_SOURCE,
            "status": status,
            "reason": bounded_delivery_event_reason(reason),
            "timestamp": now_millis(),
        })],
    )
}

fn bounded_delivery_event_reason(reason: &str) -> String {
    let mut reason = reason.trim().to_owned();
    if reason.is_empty() {
        reason = "event stream unavailable".to_owned();
    }
    if reason.len() <= DELIVERY_EVENT_REASON_LIMIT {
        return reason;
    }
    let mut end = DELIVERY_EVENT_REASON_LIMIT;
    while !reason.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    reason.truncate(end);
    reason
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
    let _all_events_delivered = fanout_events(&mut subscribers, &events);
}

fn publish_incremental_events(
    subscribers: &Arc<Mutex<SubscriberHub>>,
    events: Vec<ModuleTransportEvent>,
) -> bool {
    let Ok(mut subscribers) = subscribers.lock() else {
        return false;
    };
    let delivered = fanout_events(&mut subscribers, &events);
    if delivered {
        for event in events
            .iter()
            .filter(|event| is_delivery_event_stream_status(event))
        {
            subscribers
                .snapshot
                .retain(|current| !is_delivery_event_stream_status(current));
            subscribers.snapshot.push(event.clone());
        }
    }
    delivered
}

fn fanout_events(subscribers: &mut SubscriberHub, events: &[ModuleTransportEvent]) -> bool {
    if events.is_empty() {
        return true;
    }
    let delivery_gap = delivery_event_stream_status_event(
        DELIVERY_EVENT_STREAM_UNAVAILABLE,
        "unavailable",
        "subscriber queue overflowed; Delivery event stream will reconnect",
    )
    .ok();
    for event in events
        .iter()
        .filter(|event| is_delivery_event_stream_status(event))
    {
        let unavailable = event.event() == DELIVERY_EVENT_STREAM_UNAVAILABLE;
        if subscribers.senders.iter().any(|subscriber| {
            !(subscriber.can_accept_status()
                || unavailable && subscriber.has_delivery_gap_pending())
        }) {
            return false;
        }
    }
    let mut all_events_delivered = true;
    subscribers.senders.retain(|subscriber| {
        for event in events {
            if event.module() == DELIVERY_MODULE
                && event.event() == DELIVERY_EVENT_STREAM_UNAVAILABLE
                && subscriber.has_delivery_gap_pending()
            {
                continue;
            }
            if is_delivery_event_stream_status(event) {
                match subscriber.try_send(event.clone()) {
                    Ok(()) => {}
                    Err(mpsc::TrySendError::Full(_)) => {
                        all_events_delivered = false;
                        break;
                    }
                    Err(mpsc::TrySendError::Disconnected(_)) => return false,
                }
                continue;
            }
            if subscriber.is_at_data_capacity() {
                all_events_delivered = false;
                if is_delivery_native_event(event) {
                    let Some(gap) = delivery_gap.as_ref() else {
                        return false;
                    };
                    if !subscriber.queue_delivery_gap(gap.clone()) {
                        return false;
                    }
                }
                break;
            }
            match subscriber.try_send(event.clone()) {
                Ok(()) => {}
                Err(mpsc::TrySendError::Full(_)) => {
                    all_events_delivered = false;
                    break;
                }
                Err(mpsc::TrySendError::Disconnected(_)) => return false,
            }
        }
        true
    });
    all_events_delivered
}

fn merge_snapshot(
    previous: &[ModuleTransportEvent],
    mut baseline: Vec<ModuleTransportEvent>,
    events: &[ModuleTransportEvent],
) -> Vec<ModuleTransportEvent> {
    let mut retained = previous
        .iter()
        .filter(|event| is_retained_status_event(event))
        .cloned()
        .collect::<Vec<_>>();
    for event in events
        .iter()
        .filter(|event| is_retained_status_event(event))
    {
        retained.retain(|current| !replaces_retained_snapshot(current, event));
        retained.push(event.clone());
    }
    baseline.extend(retained);
    baseline
}

fn replaces_retained_snapshot(
    current: &ModuleTransportEvent,
    replacement: &ModuleTransportEvent,
) -> bool {
    if is_delivery_event_stream_status(current) && is_delivery_event_stream_status(replacement) {
        return true;
    }
    is_lifecycle_status_event(current)
        && is_lifecycle_status_event(replacement)
        && replaces_lifecycle_snapshot(current, replacement)
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

fn is_delivery_event_stream_status(event: &ModuleTransportEvent) -> bool {
    event.module() == DELIVERY_MODULE
        && matches!(
            event.event(),
            DELIVERY_EVENT_STREAM_READY | DELIVERY_EVENT_STREAM_UNAVAILABLE
        )
}

fn is_delivery_native_event(event: &ModuleTransportEvent) -> bool {
    event.module() == DELIVERY_MODULE && DELIVERY_NATIVE_EVENTS.contains(&event.event())
}

fn is_retained_status_event(event: &ModuleTransportEvent) -> bool {
    is_lifecycle_status_event(event) || is_delivery_event_stream_status(event)
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
            status_observed_at: observation.status_observed_at,
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
    status_observed_at: Instant,
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
    let status_observed_at = Instant::now();
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
        reconcile_indexer_liveness(&mut probes, &runtime, cancellation);
    }
    let needs_fast_poll = probes.iter().any(|probe| probe.is_pending())
        || (daemon == DaemonState::Stopped
            && probes
                .iter()
                .any(|probe| probe.lifecycle_state != NodeLifecycleState::Stopped));
    ModuleObservation {
        daemon,
        loaded_modules,
        status_observed_at,
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
        status_observed_at: Instant::now(),
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

fn reconcile_indexer_liveness(
    probes: &mut [NodeLivenessProbe],
    runtime: &LogoscoreCliRuntime,
    cancellation: &CancellationToken,
) {
    if !probes.iter().any(|probe| {
        probe.kind == NodeKind::Indexer && probe.module_availability == ModuleAvailability::Loaded
    }) {
        return;
    }
    let now = Instant::now();
    let deadline = now.checked_add(INDEXER_STATUS_PROBE_TIMEOUT).unwrap_or(now);
    let control = CommandControl::new(cancellation.clone(), deadline);
    let module =
        crate::source_routing::execution_zone_layer::managed_indexer_contract().module_id();
    debug_assert_eq!(module, INDEXER_WATCHER_MODULE);
    let health = runtime
        .call_controlled(module, "getStatus", &[], control)
        .and_then(|output| normalize_module_call_value(module, "getStatus", output.value))
        .map_or(IndexerModuleHealth::Unknown, |value| {
            indexer_module_health(&value)
        });
    for probe in probes.iter_mut().filter(|probe| {
        probe.kind == NodeKind::Indexer && probe.module_availability == ModuleAvailability::Loaded
    }) {
        apply_indexer_module_health(probe, &health);
    }
}

fn apply_indexer_module_health(probe: &mut NodeLivenessProbe, health: &IndexerModuleHealth) {
    match health {
        IndexerModuleHealth::Running(status) => {
            probe.alive = true;
            probe.liveness_known = true;
            probe.unavailable_detail = None;
            probe.indexer_status = Some(status.clone());
        }
        IndexerModuleHealth::Stopped => {
            probe.alive = false;
            probe.liveness_known = true;
            probe.unavailable_detail = Some("Indexer module reports no running indexer");
            probe.indexer_status = Some(IndexerStatusObservation {
                state: "stopped".to_owned(),
                indexed_block_id: None,
                last_error: None,
            });
        }
        IndexerModuleHealth::Unknown => {
            probe.alive = false;
            probe.liveness_known = false;
            probe.unavailable_detail = Some("Indexer module status could not be verified");
            probe.indexer_status = None;
        }
    }
}

fn indexer_module_health(value: &Value) -> IndexerModuleHealth {
    if let Some(text) = value.as_str() {
        let text = text.trim();
        if text.is_empty() {
            return IndexerModuleHealth::Stopped;
        }
        return serde_json::from_str::<Value>(text)
            .ok()
            .map_or(IndexerModuleHealth::Unknown, |value| {
                indexer_module_health(&value)
            });
    }
    let Some(state) = value.get("state").and_then(Value::as_str) else {
        return IndexerModuleHealth::Unknown;
    };
    let normalized = state
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    let state = match normalized.as_str() {
        "starting" => "starting",
        "syncing" => "syncing",
        "caughtup" => "caught_up",
        "error" => "error",
        "stalled" => "stalled",
        _ => return IndexerModuleHealth::Unknown,
    };
    IndexerModuleHealth::Running(IndexerStatusObservation {
        state: state.to_owned(),
        indexed_block_id: value
            .get("indexedBlockId")
            .or_else(|| value.get("indexed_block_id"))
            .and_then(indexer_status_scalar),
        last_error: value
            .get("lastError")
            .or_else(|| value.get("last_error"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned),
    })
}

fn indexer_status_scalar(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => Some(value.clone()),
        Value::Number(value) => Some(value.to_string()),
        Value::Null | Value::Bool(_) | Value::Array(_) | Value::Object(_) => None,
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
    } else if node.kind == NodeKind::Indexer {
        (
            false,
            false,
            Some("Indexer lifecycle requires managed module status verification"),
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
        indexer_status: None,
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
            NodeKind::Sequencer => Some((SEQUENCER_WATCHER_MODULE, LifecycleEventRoute::Synthetic)),
            NodeKind::Bedrock | NodeKind::Indexer | NodeKind::Storage | NodeKind::Messaging => None,
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
        let indexer_status_changed = apply_indexer_status_observation(node, probe);
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
                    if probe.process_backed || probe.kind == NodeKind::Indexer {
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
                    if probe.process_backed || probe.kind == NodeKind::Indexer {
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
                    if probe.process_backed || probe.kind == NodeKind::Indexer {
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
                (NodeLifecycleState::Failed, None, false)
                    if probe.kind == NodeKind::Indexer
                        && probe
                            .indexer_status
                            .as_ref()
                            .is_some_and(|status| status.state == "stopped") =>
                {
                    node.process_id = None;
                    node.lifecycle_state = NodeLifecycleState::Stopped;
                    Some(lifecycle_event(
                        probe,
                        false,
                        true,
                        probe
                            .unavailable_detail
                            .unwrap_or("Indexer module reports no running indexer"),
                    )?)
                }
                (NodeLifecycleState::Stopped, None, true) => {
                    node.lifecycle_state = NodeLifecycleState::Running;
                    Some(lifecycle_event(probe, true, true, "endpoint is reachable")?)
                }
                _ => None,
            }
        };
        if event.is_some() || indexer_status_changed {
            record.updated_at = observed_at;
            changed_records.insert(record.id.clone());
        }
        if let Some(event) = event {
            events.push(event);
        }
    }
    let changed = !changed_records.is_empty();
    for record_id in changed_records {
        let Some(record) = state.topology_mut(&record_id) else {
            continue;
        };
        write_devnet_manifest(record)?;
    }
    Ok((changed, events))
}

fn apply_indexer_status_observation(
    node: &mut LocalNodeConfigRecord,
    probe: &NodeLivenessProbe,
) -> bool {
    if node.kind != NodeKind::Indexer {
        return false;
    }
    let Some(status) = probe.indexer_status.as_ref() else {
        return false;
    };
    if node.indexer_state.as_deref() == Some(status.state.as_str())
        && node.indexer_head == status.indexed_block_id
        && node.indexer_error == status.last_error
    {
        return false;
    }
    node.indexer_state = Some(status.state.clone());
    node.indexer_head.clone_from(&status.indexed_block_id);
    node.indexer_error.clone_from(&status.last_error);
    true
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
                    package_version: None,
                    package_root_hash: None,
                    indexer_state: None,
                    indexer_head: None,
                    indexer_error: None,
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
        first_devnet(state)?.nodes.first().context("missing node")
    }

    fn only_node_mut(state: &mut LocalNodesState) -> Result<&mut LocalNodeConfigRecord> {
        first_devnet_mut(state)?
            .nodes
            .first_mut()
            .context("missing node")
    }

    fn first_devnet(state: &LocalNodesState) -> Result<&LocalDevnetRecord> {
        state.devnets.first().context("missing devnet")
    }

    fn first_devnet_mut(state: &mut LocalNodesState) -> Result<&mut LocalDevnetRecord> {
        state.devnets.first_mut().context("missing devnet")
    }

    fn only_event(events: &[ModuleTransportEvent]) -> Result<&ModuleTransportEvent> {
        let [event] = events else {
            anyhow::bail!("expected one module event, got {}", events.len());
        };
        Ok(event)
    }

    fn first_event_arg(event: &ModuleTransportEvent) -> Result<&Value> {
        event
            .args()
            .first()
            .context("missing module event argument")
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
            indexer_status: None,
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
        let event = only_event(&events)?;
        anyhow::ensure!(event.module() == "delivery_module" && event.event() == "nodeStarted");
        anyhow::ensure!(
            event
                .args()
                .iter()
                .find_map(|payload| payload.get("simulated"))
                .and_then(Value::as_bool)
                == Some(true)
        );
        let node = only_node(&state)?;
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
        let event = only_event(&events)?;
        let payload = first_event_arg(event)?;
        anyhow::ensure!(event.module() == "blockchain_module" && event.event() == "nodeStarted");
        anyhow::ensure!(
            payload.get("node").and_then(Value::as_str) == Some("bedrock")
                && payload.get("network_id").and_then(Value::as_str) == Some("devnet")
                && payload.get("simulated").and_then(Value::as_bool) == Some(true)
        );
        let node = only_node(&state)?;
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
        let event = only_event(&events)?;
        anyhow::ensure!(event.module() == "blockchain_module" && event.event() == "nodeStarted");
        let node = only_node(&state)?;
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
        let event = only_event(&events)?;
        anyhow::ensure!(event.module() == INDEXER_WATCHER_MODULE && event.event() == "nodeStarted");
        anyhow::ensure!(
            first_event_arg(event)?.get("node").and_then(Value::as_str) == Some("indexer")
        );
        let node = only_node(&state)?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Running
                && node.pending_lifecycle_action.is_none()
        );
        Ok(())
    }

    #[test]
    fn liveness_stop_confirms_empty_indexer_status_and_clears_legacy_pid() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        only_node_mut(&mut state)?.process_id = Some(4242);
        let probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            false,
            ModuleAvailability::Unknown,
        )?;

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        let event = only_event(&events)?;
        anyhow::ensure!(event.module() == INDEXER_WATCHER_MODULE && event.event() == "nodeStopped");
        let node = only_node(&state)?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Stopped
                && node.pending_lifecycle_action.is_none()
                && node.process_id.is_none()
        );
        Ok(())
    }

    #[test]
    fn liveness_stop_waits_while_indexer_module_reports_running() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        let updated_at = now_millis();
        first_devnet_mut(&mut state)?.updated_at = updated_at;
        let mut probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            true,
            ModuleAvailability::Unknown,
        )?;
        probe.record_updated_at = updated_at;

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
        let event = only_event(&events)?;
        anyhow::ensure!(event.module() == "storage_module" && event.event() == "storageStop");
        anyhow::ensure!(first_event_arg(event)?.is_string());
        let node = only_node(&state)?;
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
        second_probe.record_updated_at = first_devnet(&state)?.updated_at;

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
    fn indexer_module_health_parses_running_and_stopped_states() {
        for (state, expected) in [
            ("Starting", "starting"),
            ("Syncing", "syncing"),
            ("CaughtUp", "caught_up"),
            ("Error", "error"),
            ("Stalled", "stalled"),
        ] {
            let health = indexer_module_health(&json!({
                "state": state,
                "indexed_block_id": 42,
                "last_error": null,
            }));
            assert!(matches!(health, IndexerModuleHealth::Running(status)
                    if status.state == expected
                        && status.indexed_block_id.as_deref() == Some("42")
                        && status.last_error.is_none()));
        }
        assert_eq!(
            indexer_module_health(&Value::String(String::new())),
            IndexerModuleHealth::Stopped
        );
        assert!(matches!(
            indexer_module_health(&json!(
                "{\"state\":\"caught_up\",\"indexedBlockId\":42,\"lastError\":\"lagging\"}"
            )),
            IndexerModuleHealth::Running(status)
                if status.state == "caught_up"
                    && status.indexed_block_id.as_deref() == Some("42")
                    && status.last_error.as_deref() == Some("lagging")
        ));
        assert_eq!(
            indexer_module_health(&json!({ "state": "unexpected" })),
            IndexerModuleHealth::Unknown
        );
        assert_eq!(
            indexer_module_health(&json!("not-json")),
            IndexerModuleHealth::Unknown
        );
    }

    #[test]
    fn indexer_module_status_confirms_lifecycle_without_rpc_port() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let mut probes = collect_liveness_probes(
            &state,
            &BTreeSet::from([INDEXER_WATCHER_MODULE.to_owned()]),
            true,
        );
        let probe = probes.first_mut().context("missing Indexer module probe")?;
        anyhow::ensure!(probe.port.is_none() && !probe.liveness_known);
        apply_indexer_module_health(
            probe,
            &IndexerModuleHealth::Running(IndexerStatusObservation {
                state: "syncing".to_owned(),
                indexed_block_id: Some("42".to_owned()),
                last_error: None,
            }),
        );

        let (changed, events) = apply_liveness_observation(&mut state, &probes)?;

        anyhow::ensure!(changed && events.len() == 1);
        let event = only_event(&events)?;
        anyhow::ensure!(event.module() == INDEXER_WATCHER_MODULE && event.event() == "nodeStarted");
        let node = only_node(&state)?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Running
                && node.indexer_state.as_deref() == Some("syncing")
                && node.indexer_head.as_deref() == Some("42")
                && node.indexer_error.is_none()
        );
        Ok(())
    }

    #[test]
    fn indexer_status_updates_only_the_bound_topology() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Running,
            None,
        );
        let mut testnet = first_devnet(&state)?.clone();
        testnet.id = "logos-testnet".to_owned();
        state.testnet = Some(testnet);
        state
            .module_context_topology_by_kind
            .insert(NodeKind::Indexer, "logos-testnet".to_owned());
        let mut probes = collect_liveness_probes(
            &state,
            &BTreeSet::from([INDEXER_WATCHER_MODULE.to_owned()]),
            true,
        );
        anyhow::ensure!(probes.len() == 1);
        let probe = probes.first_mut().context("missing bound Indexer probe")?;
        anyhow::ensure!(probe.topology_id == "logos-testnet");
        apply_indexer_module_health(
            probe,
            &IndexerModuleHealth::Running(IndexerStatusObservation {
                state: "syncing".to_owned(),
                indexed_block_id: Some("42".to_owned()),
                last_error: None,
            }),
        );

        let (changed, events) = apply_liveness_observation(&mut state, &probes)?;

        anyhow::ensure!(changed && events.is_empty());
        let bound = state
            .testnet
            .as_ref()
            .and_then(|record| record.nodes.first())
            .context("missing bound Indexer")?;
        anyhow::ensure!(
            bound.indexer_state.as_deref() == Some("syncing")
                && bound.indexer_head.as_deref() == Some("42")
        );
        let unbound = first_devnet(&state)?
            .nodes
            .first()
            .context("missing unbound Indexer")?;
        anyhow::ensure!(
            unbound.indexer_state.is_none()
                && unbound.indexer_head.is_none()
                && unbound.indexer_error.is_none()
        );
        Ok(())
    }

    #[test]
    fn empty_indexer_module_status_confirms_stop_and_clears_observation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            Some(NodeAction::Stop),
        );
        {
            let node = only_node_mut(&mut state)?;
            node.indexer_state = Some("error".to_owned());
            node.indexer_head = Some("41".to_owned());
            node.indexer_error = Some("bedrock unavailable".to_owned());
        }
        let mut probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Stopping,
            false,
            ModuleAvailability::Loaded,
        )?;
        apply_indexer_module_health(&mut probe, &IndexerModuleHealth::Stopped);

        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        let node = only_node(&state)?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Stopped
                && node.indexer_state.as_deref() == Some("stopped")
                && node.indexer_head.is_none()
                && node.indexer_error.is_none()
        );
        Ok(())
    }

    #[test]
    fn failed_indexer_start_recovers_when_module_reports_stopped() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Indexer,
            NodeLifecycleState::Starting,
            Some(NodeAction::Start),
        );
        let expired = now_millis().saturating_sub(LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS + 1);
        first_devnet_mut(&mut state)?.updated_at = expired;
        let mut unknown_probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Starting,
            false,
            ModuleAvailability::Loaded,
        )?;
        unknown_probe.record_updated_at = expired;
        unknown_probe.liveness_known = false;
        unknown_probe.unavailable_detail = Some("Indexer module status could not be verified");

        let (failed_changed, failed_events) =
            apply_liveness_observation(&mut state, &[unknown_probe])?;

        anyhow::ensure!(failed_changed && failed_events.len() == 1);
        anyhow::ensure!(only_node(&state)?.lifecycle_state == NodeLifecycleState::Failed);

        let mut stopped_probe = probe_for(
            NodeKind::Indexer,
            NodeLifecycleState::Failed,
            false,
            ModuleAvailability::Loaded,
        )?;
        stopped_probe.record_updated_at = first_devnet(&state)?.updated_at;
        apply_indexer_module_health(&mut stopped_probe, &IndexerModuleHealth::Stopped);

        let (recovered_changed, recovered_events) =
            apply_liveness_observation(&mut state, &[stopped_probe])?;

        anyhow::ensure!(recovered_changed && recovered_events.len() == 1);
        let event = only_event(&recovered_events)?;
        anyhow::ensure!(event.event() == "nodeStopped");
        anyhow::ensure!(
            event
                .args()
                .first()
                .and_then(|value| value.get("success"))
                .and_then(Value::as_bool)
                == Some(true)
        );
        let node = only_node(&state)?;
        anyhow::ensure!(
            node.lifecycle_state == NodeLifecycleState::Stopped
                && node.pending_lifecycle_action.is_none()
                && node.indexer_state.as_deref() == Some("stopped")
        );
        Ok(())
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
        let event = only_event(&events)?;
        anyhow::ensure!(event.event() == "nodeStarted");
        anyhow::ensure!(event.args().first() == Some(&Value::Bool(false)));
        let node = only_node(&state)?;
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
        let event = only_event(&events)?;
        anyhow::ensure!(event.event() == "nodeUnavailable");
        anyhow::ensure!(
            event
                .args()
                .first()
                .and_then(|value| value.get("message"))
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.contains("not initialized"))
        );
        let node = only_node(&state)?;
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
        first_devnet_mut(&mut state)?.updated_at = current;
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
        let pending_node = only_node(&state)?;
        anyhow::ensure!(
            pending_node.lifecycle_state == NodeLifecycleState::Starting
                && pending_node.pending_lifecycle_action == Some(NodeAction::Start)
        );

        let expired = now_millis().saturating_sub(LIFECYCLE_CONFIRMATION_TIMEOUT_MILLIS + 1);
        first_devnet_mut(&mut state)?.updated_at = expired;
        probe.record_updated_at = expired;
        let (changed, events) = apply_liveness_observation(&mut state, &[probe])?;

        anyhow::ensure!(changed && events.len() == 1);
        let event = only_event(&events)?;
        anyhow::ensure!(event.event() == "nodeStarted");
        anyhow::ensure!(event.args().first() == Some(&Value::Bool(false)));
        anyhow::ensure!(
            event
                .args()
                .get(1)
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.contains("could not be verified"))
        );
        let node = only_node(&state)?;
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
        first_devnet_mut(&mut state)?.updated_at = expired;
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
        let event = only_event(&events)?;
        anyhow::ensure!(event.event() == "nodeStopped");
        anyhow::ensure!(event.args().first() == Some(&Value::Bool(false)));
        anyhow::ensure!(
            event
                .args()
                .get(1)
                .and_then(Value::as_str)
                .is_some_and(|detail| detail.contains("could not be verified"))
        );
        let node = only_node(&state)?;
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
        anyhow::ensure!(only_event(&events)?.event() == "nodeStarted");
        let node = only_node(&state)?;
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
        let probe = probes.first().context("missing Bedrock liveness probe")?;
        anyhow::ensure!(
            probe.module == "blockchain_module"
                && matches!(probe.event_route, LifecycleEventRoute::Synthetic)
                && probe.requires_module_context
        );
        Ok(())
    }

    #[test]
    fn registered_process_nodes_are_polled_without_module_context() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = state_with_node(
            &directory,
            NodeKind::Sequencer,
            NodeLifecycleState::Running,
            None,
        );
        state
            .module_context_topology_by_kind
            .remove(&NodeKind::Sequencer);

        let probes = collect_liveness_probes(&state, &BTreeSet::new(), false);

        anyhow::ensure!(probes.len() == 1);
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
            let mut node = only_node(&state)?.clone();
            node.kind = kind;
            node.port = adapter_for(kind).default_port();
            first_devnet_mut(&mut state)?.nodes.push(node);
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
            first_devnet(&state)?
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
        let mut testnet = first_devnet(&state)?.clone();
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
        anyhow::ensure!(
            probes
                .first()
                .context("missing bound Messaging liveness probe")?
                .topology_id
                == "logos-testnet"
        );
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
        let mut testnet = first_devnet(&state)?.clone();
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
    fn delivery_stream_status_is_retained_and_emitted_only_on_transition() -> Result<()> {
        let subscribers = Arc::new(Mutex::new(SubscriberHub::default()));
        let mut state = DeliveryEventStreamState::Unknown;
        publish_delivery_stream_transition(
            &subscribers,
            &mut state,
            DeliveryEventStreamState::Unavailable,
            "daemon unavailable",
        );
        publish_delivery_stream_transition(
            &subscribers,
            &mut state,
            DeliveryEventStreamState::Unavailable,
            "repeat must not emit",
        );
        let mut subscription = {
            let mut hub = subscribers
                .lock()
                .map_err(|_| anyhow::anyhow!("test subscriber hub is unavailable"))?;
            anyhow::ensure!(hub.snapshot.len() == 1);
            subscribe_to_hub(&mut hub)?
        };

        let unavailable = subscription
            .next_within(Duration::from_millis(25))?
            .context("missing retained Delivery stream status")?;
        anyhow::ensure!(unavailable.event() == DELIVERY_EVENT_STREAM_UNAVAILABLE);
        anyhow::ensure!(unavailable.args().len() == 1);
        anyhow::ensure!(
            unavailable
                .args()
                .first()
                .and_then(|arg| arg.get("status"))
                .and_then(Value::as_str)
                == Some("unavailable")
        );
        anyhow::ensure!(
            subscription
                .next_within(Duration::from_millis(1))?
                .is_none(),
            "unchanged unavailable state emitted repeatedly"
        );

        publish_delivery_stream_transition(
            &subscribers,
            &mut state,
            DeliveryEventStreamState::Ready,
            "subscription active",
        );
        let ready = subscription
            .next_within(Duration::from_millis(25))?
            .context("missing Delivery ready transition")?;
        anyhow::ensure!(ready.event() == DELIVERY_EVENT_STREAM_READY);
        let hub = subscribers
            .lock()
            .map_err(|_| anyhow::anyhow!("test subscriber hub is unavailable"))?;
        anyhow::ensure!(
            hub.snapshot
                .iter()
                .filter(|event| is_delivery_event_stream_status(event))
                .count()
                == 1
        );
        anyhow::ensure!(
            hub.snapshot
                .iter()
                .any(|event| event.event() == DELIVERY_EVENT_STREAM_READY)
        );
        Ok(())
    }

    #[test]
    fn delivery_stream_status_survives_poll_snapshot_refresh() -> Result<()> {
        let status = ModuleTransportEvent::new(
            DELIVERY_MODULE,
            DELIVERY_EVENT_STREAM_READY,
            vec![json!({
                "source": DELIVERY_EVENT_STREAM_SOURCE,
                "status": "ready",
                "reason": "subscription active",
                "timestamp": 1,
            })],
        )?;
        let baseline = vec![ModuleTransportEvent::new(
            RUNTIME_MODULE,
            "daemonStarted",
            vec![json!({ "simulated": true })],
        )?];

        let snapshot = merge_snapshot(&[status], baseline, &[]);

        anyhow::ensure!(
            snapshot
                .iter()
                .any(|event| event.event() == DELIVERY_EVENT_STREAM_READY)
        );
        anyhow::ensure!(
            snapshot
                .iter()
                .any(|event| event.event() == "daemonStarted")
        );
        Ok(())
    }

    #[test]
    fn delivery_stream_health_invalidates_ready_watch_after_module_loss() -> Result<()> {
        let health = Arc::new(Mutex::new(DeliveryWatchHealthObservation::new(
            DeliveryWatchHealth::Unknown,
        )));
        let ready = ModuleWatcherPoll {
            daemon: DaemonState::Running,
            loaded_modules: BTreeSet::from([DELIVERY_MODULE.to_owned()]),
            status_observed_at: Instant::now(),
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_POLL_INTERVAL,
        };
        record_delivery_watch_poll_health(&health, &ready);
        ensure_delivery_watch_health(&health)?;

        let module_lost = ModuleWatcherPoll {
            daemon: DaemonState::Running,
            loaded_modules: BTreeSet::new(),
            status_observed_at: Instant::now(),
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_POLL_INTERVAL,
        };
        record_delivery_watch_poll_health(&health, &module_lost);

        let Err(error) = ensure_delivery_watch_health(&health) else {
            anyhow::bail!("module loss did not invalidate an idle Delivery watch");
        };
        anyhow::ensure!(error.to_string().contains("no longer loaded"));
        Ok(())
    }

    #[test]
    fn delivery_stream_health_invalidates_ready_watch_after_poll_failure() -> Result<()> {
        let health = Arc::new(Mutex::new(DeliveryWatchHealthObservation::new(
            DeliveryWatchHealth::Ready,
        )));
        record_delivery_watch_health(
            &health,
            DeliveryWatchHealth::PollUnavailable,
            Instant::now(),
        );

        let Err(error) = ensure_delivery_watch_health(&health) else {
            anyhow::bail!("poll failure did not invalidate authoritative Delivery coverage");
        };
        anyhow::ensure!(error.to_string().contains("polling became unavailable"));
        Ok(())
    }

    #[test]
    fn delivery_stream_health_rejects_stale_ready_poll() -> Result<()> {
        let stale_age = DELIVERY_WATCH_HEALTH_MAX_AGE
            .checked_add(Duration::from_millis(1))
            .context("stale Delivery watch test duration overflowed")?;
        let observed_at = Instant::now()
            .checked_sub(stale_age)
            .context("stale Delivery watch test instant underflowed")?;
        let stale_poll = ModuleWatcherPoll {
            daemon: DaemonState::Running,
            loaded_modules: BTreeSet::from([DELIVERY_MODULE.to_owned()]),
            status_observed_at: observed_at,
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_POLL_INTERVAL,
        };
        let health = Arc::new(Mutex::new(DeliveryWatchHealthObservation::new(
            DeliveryWatchHealth::Unknown,
        )));
        record_delivery_watch_poll_health(&health, &stale_poll);

        let Err(error) = ensure_delivery_watch_health(&health) else {
            anyhow::bail!("stale poll health did not invalidate an idle Delivery watch");
        };
        anyhow::ensure!(error.to_string().contains("polling is stale"));
        Ok(())
    }

    #[test]
    fn delivery_ready_replay_stays_unavailable_until_every_live_subscriber_accepts_it() -> Result<()>
    {
        let subscribers = Arc::new(Mutex::new(SubscriberHub::default()));
        let mut state = DeliveryEventStreamState::Unknown;
        anyhow::ensure!(publish_delivery_stream_transition(
            &subscribers,
            &mut state,
            DeliveryEventStreamState::Unavailable,
            "watch exited",
        ));

        let (sender, receiver) = mpsc::sync_channel(2);
        let queued = Arc::new(AtomicUsize::new(0));
        let delivery_gap_pending = Arc::new(AtomicBool::new(false));
        let blocked = SubscriberSender {
            sender,
            queued: Arc::clone(&queued),
            delivery_gap_pending: Arc::clone(&delivery_gap_pending),
            data_capacity: 1,
        };
        blocked.send(delivery_event_stream_status_event(
            DELIVERY_EVENT_STREAM_UNAVAILABLE,
            "unavailable",
            "watch exited",
        )?)?;
        blocked.send(ModuleTransportEvent::new(
            RUNTIME_MODULE,
            "daemonStarted",
            vec![json!({ "simulated": true })],
        )?)?;
        {
            let mut hub = subscribers
                .lock()
                .map_err(|_| anyhow::anyhow!("test subscriber hub is unavailable"))?;
            hub.senders.push(blocked);
        }

        anyhow::ensure!(!publish_delivery_stream_transition(
            &subscribers,
            &mut state,
            DeliveryEventStreamState::Ready,
            "subscription active",
        ));
        anyhow::ensure!(state == DeliveryEventStreamState::Unavailable);

        let mut late = {
            let mut hub = subscribers
                .lock()
                .map_err(|_| anyhow::anyhow!("test subscriber hub is unavailable"))?;
            anyhow::ensure!(
                hub.snapshot
                    .iter()
                    .any(|event| event.event() == DELIVERY_EVENT_STREAM_UNAVAILABLE)
            );
            anyhow::ensure!(
                !hub.snapshot
                    .iter()
                    .any(|event| event.event() == DELIVERY_EVENT_STREAM_READY)
            );
            subscribe_to_hub(&mut hub)?
        };
        let replay = late
            .next_within(Duration::from_millis(25))?
            .context("missing retained Delivery stream status")?;
        anyhow::ensure!(replay.event() == DELIVERY_EVENT_STREAM_UNAVAILABLE);

        let mut blocked_subscription = LocalNodeModuleSubscription {
            receiver,
            queued,
            delivery_gap_pending,
        };
        anyhow::ensure!(
            blocked_subscription
                .next_within(Duration::from_millis(25))?
                .is_some()
        );
        anyhow::ensure!(
            blocked_subscription
                .next_within(Duration::from_millis(25))?
                .is_some()
        );
        anyhow::ensure!(publish_delivery_stream_transition(
            &subscribers,
            &mut state,
            DeliveryEventStreamState::Ready,
            "subscription active",
        ));
        anyhow::ensure!(state == DeliveryEventStreamState::Ready);
        anyhow::ensure!(
            blocked_subscription
                .next_within(Duration::from_millis(25))?
                .context("missing Delivery ready after queue drain")?
                .event()
                == DELIVERY_EVENT_STREAM_READY
        );
        anyhow::ensure!(
            late.next_within(Duration::from_millis(25))?
                .context("missing Delivery ready for late subscriber")?
                .event()
                == DELIVERY_EVENT_STREAM_READY
        );
        Ok(())
    }

    #[test]
    fn delivery_stream_reason_is_utf8_safe_and_bounded() {
        let reason = "é".repeat(DELIVERY_EVENT_REASON_LIMIT);
        let bounded = bounded_delivery_event_reason(&reason);

        assert!(bounded.len() <= DELIVERY_EVENT_REASON_LIMIT);
        assert!(bounded.is_char_boundary(bounded.len()));
    }

    #[test]
    fn delivery_subscriber_overflow_enqueues_gap_status_and_requests_reconnect() -> Result<()> {
        let (sender, receiver) = mpsc::sync_channel(2);
        let queued = Arc::new(AtomicUsize::new(0));
        let delivery_gap_pending = Arc::new(AtomicBool::new(false));
        let subscribers = Arc::new(Mutex::new(SubscriberHub {
            senders: vec![SubscriberSender {
                sender,
                queued: Arc::clone(&queued),
                delivery_gap_pending: Arc::clone(&delivery_gap_pending),
                data_capacity: 1,
            }],
            snapshot: Vec::new(),
        }));
        let mut subscription = LocalNodeModuleSubscription {
            receiver,
            queued,
            delivery_gap_pending,
        };
        let native_event = |request_id: &str| {
            ModuleTransportEvent::new(
                DELIVERY_MODULE,
                "messageSent",
                vec![Value::String(request_id.to_owned())],
            )
        };

        anyhow::ensure!(publish_incremental_events(
            &subscribers,
            vec![native_event("request-1")?],
        ));
        anyhow::ensure!(!publish_incremental_events(
            &subscribers,
            vec![native_event("request-2")?],
        ));

        let delivered = subscription
            .next_within(Duration::from_millis(25))?
            .context("missing queued Delivery event")?;
        anyhow::ensure!(delivered.event() == "messageSent");
        let gap = subscription
            .next_within(Duration::from_millis(25))?
            .context("missing Delivery overflow gap status")?;
        anyhow::ensure!(gap.event() == DELIVERY_EVENT_STREAM_UNAVAILABLE);
        anyhow::ensure!(
            gap.args()
                .first()
                .and_then(|arg| arg.get("reason"))
                .and_then(Value::as_str)
                .is_some_and(|reason| reason.contains("overflowed"))
        );
        anyhow::ensure!(
            subscription
                .next_within(Duration::from_millis(1))?
                .is_none(),
            "overflowed Delivery event was forwarded after its gap marker"
        );

        let mut state = DeliveryEventStreamState::Unavailable;
        anyhow::ensure!(publish_delivery_stream_transition(
            &subscribers,
            &mut state,
            DeliveryEventStreamState::Ready,
            "subscription active",
        ));
        let ready = subscription
            .next_within(Duration::from_millis(25))?
            .context("missing Delivery ready after overflow reconnect")?;
        anyhow::ensure!(ready.event() == DELIVERY_EVENT_STREAM_READY);
        anyhow::ensure!(publish_incremental_events(
            &subscribers,
            vec![native_event("request-3")?],
        ));
        let resumed = subscription
            .next_within(Duration::from_millis(25))?
            .context("missing Delivery event after overflow reconnect")?;
        anyhow::ensure!(resumed.event() == "messageSent");
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
                && event
                    .args()
                    .first()
                    .and_then(|payload| payload.get("network_id"))
                    .and_then(Value::as_str)
                    == Some("alpha")
        }));
        anyhow::ensure!(snapshot.iter().any(|event| {
            event.event() == "nodeStarted"
                && event
                    .args()
                    .first()
                    .and_then(|payload| payload.get("network_id"))
                    .and_then(Value::as_str)
                    == Some("beta")
        }));
        Ok(())
    }

    #[test]
    fn watcher_observation_reports_module_ready_once() {
        let poll = ModuleWatcherPoll {
            daemon: DaemonState::Running,
            loaded_modules: ["delivery_module".to_owned()].into_iter().collect(),
            status_observed_at: Instant::now(),
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
            status_observed_at: Instant::now(),
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
            status_observed_at: Instant::now(),
            lifecycle_events: Vec::new(),
            poll_interval: DAEMON_POLL_INTERVAL,
        };
        let unavailable = ModuleWatcherPoll {
            daemon: DaemonState::Unavailable,
            loaded_modules: BTreeSet::new(),
            status_observed_at: Instant::now(),
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
