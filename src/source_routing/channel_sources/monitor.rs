use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fmt,
    future::{Future, pending},
    pin::Pin,
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot, watch},
    task::{Id, JoinHandle, JoinSet},
    time::Instant,
};
use tokio_util::sync::CancellationToken;

use super::{
    ChannelSourceConfig, ChannelSourceRole, ChannelSourceTarget, ConfiguredIndexerSource,
    ConfiguredSequencerSource, PersistedSequencerAttestation,
    config::{normalize_channel_id, normalize_channel_source_configs},
    probe::{
        ChannelSourceBlock, ChannelSourceProbe, ChannelSourceProbeFact, ChannelSourceProbeFailure,
        ChannelSourceProbeOutput, ChannelSourceProbeRequest, DefaultChannelSourceProbe,
    },
};
use crate::inspection::NetworkScope;

const MONITOR_COMMAND_CAPACITY: usize = 8;
const MAX_CONCURRENT_PROBES: usize = 8;
const HEALTHY_INTERVAL_MILLIS: u64 = 30_000;
const MAX_BACKOFF_MILLIS: u64 = 300_000;
const MAX_JITTER_MILLIS: u64 = 5_000;

pub type ChannelSourceMonitorResult<T> = Result<T, ChannelSourceMonitorError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelSourceMonitorError {
    InvalidConfiguration(String),
    InvalidState(String),
    Join(String),
}

impl fmt::Display for ChannelSourceMonitorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(detail) => {
                write!(
                    formatter,
                    "invalid Channel source monitor configuration: {detail}"
                )
            }
            Self::InvalidState(detail) => {
                write!(formatter, "invalid Channel source monitor state: {detail}")
            }
            Self::Join(detail) => write!(formatter, "Channel source monitor task failed: {detail}"),
        }
    }
}

impl std::error::Error for ChannelSourceMonitorError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSourceBindingState {
    PersistedAttested,
    Pending,
    RuntimeAttested,
    ChannelMismatch,
}

impl ChannelSourceBindingState {
    #[must_use]
    pub fn is_read_eligible(self) -> bool {
        matches!(self, Self::PersistedAttested | Self::RuntimeAttested)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSourceHealthState {
    Pending,
    Reachable,
    Degraded,
    Unreachable,
    Incomplete,
    Unsupported,
    ChannelMismatch,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSourceProbeStage {
    Health,
    ChannelIdentity,
    Head,
    Task,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSourceCurrentFailure {
    pub kind: super::ChannelSourceFailureKind,
    pub stage: ChannelSourceProbeStage,
    pub diagnostic: String,
    pub failed_at_unix: u64,
    pub consecutive_failures: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSourceBlockObservation {
    pub block_id: u64,
    pub header_hash: Option<String>,
    pub parent_hash: Option<String>,
    pub observed_at_unix: u64,
    pub failure_kind: Option<super::ChannelSourceFailureKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSourceLastGood {
    pub observed_at_unix: u64,
    pub latency_millis: u64,
    pub health_ok: bool,
    pub reported_channel_id: Option<String>,
    pub head: Option<ChannelSourceBlockObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSourceObservation {
    pub source_id: String,
    pub role: ChannelSourceRole,
    pub selected: bool,
    pub binding_state: Option<ChannelSourceBindingState>,
    pub health: ChannelSourceHealthState,
    pub last_good: Option<ChannelSourceLastGood>,
    pub current_failure: Option<ChannelSourceCurrentFailure>,
    pub comparison_blocks: Vec<ChannelSourceBlockObservation>,
}

impl ChannelSourceObservation {
    #[must_use]
    pub fn is_read_eligible(&self) -> bool {
        match self.role {
            ChannelSourceRole::Sequencer => self
                .binding_state
                .is_some_and(ChannelSourceBindingState::is_read_eligible),
            ChannelSourceRole::Indexer => true,
        }
    }

    #[must_use]
    pub fn is_comparable(&self) -> bool {
        self.is_read_eligible()
            && matches!(
                self.health,
                ChannelSourceHealthState::Reachable | ChannelSourceHealthState::Degraded
            )
            && self
                .last_good
                .as_ref()
                .and_then(|observation| observation.head.as_ref())
                .is_some_and(|head| {
                    head.header_hash
                        .as_deref()
                        .is_some_and(|hash| !hash.is_empty())
                })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSourceObservationSet {
    pub channel_id: String,
    pub config_revision: u64,
    pub selected_sequencer_source_id: Option<String>,
    pub observations: Vec<ChannelSourceObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ChannelSourceMonitorSnapshot {
    pub network_scope: Option<NetworkScope>,
    pub catalog_verified: bool,
    pub observation_revision: u64,
    pub channels: Vec<ChannelSourceObservationSet>,
}

pub struct ChannelSourceMonitor {
    commands: mpsc::Sender<MonitorCommand>,
    reports: watch::Receiver<ChannelSourceMonitorSnapshot>,
    shutdown: CancellationToken,
    controller: Mutex<Option<JoinHandle<()>>>,
}

impl ChannelSourceMonitor {
    #[must_use]
    pub fn new(runtime: &Handle) -> Self {
        Self::with_dependencies(
            runtime,
            Arc::new(DefaultChannelSourceProbe::default()),
            Arc::new(TokioMonitorClock::new()),
        )
    }

    fn with_dependencies(
        runtime: &Handle,
        probe: Arc<dyn ChannelSourceProbe>,
        clock: Arc<dyn MonitorClock>,
    ) -> Self {
        let (commands, receiver) = mpsc::channel(MONITOR_COMMAND_CAPACITY);
        let (report_sender, reports) = watch::channel(ChannelSourceMonitorSnapshot::default());
        let shutdown = CancellationToken::new();
        let controller = runtime.spawn(run_monitor_controller(
            receiver,
            report_sender,
            probe,
            clock,
            shutdown.clone(),
        ));
        Self {
            commands,
            reports,
            shutdown,
            controller: Mutex::new(Some(controller)),
        }
    }

    #[must_use]
    pub fn snapshot(&self) -> ChannelSourceMonitorSnapshot {
        self.reports.borrow().clone()
    }

    pub async fn configure(
        &self,
        network_scope: NetworkScope,
        catalog_verified: bool,
        configs: Vec<ChannelSourceConfig>,
    ) -> ChannelSourceMonitorResult<u64> {
        let (response, result) = oneshot::channel();
        self.commands
            .send(MonitorCommand::Configure {
                network_scope,
                catalog_verified,
                configs,
                response,
            })
            .await
            .map_err(|_| {
                ChannelSourceMonitorError::InvalidState("monitor controller is stopped".to_owned())
            })?;
        result.await.map_err(|_| {
            ChannelSourceMonitorError::InvalidState(
                "monitor controller dropped its response".to_owned(),
            )
        })?
    }

    pub async fn shutdown(&self) -> ChannelSourceMonitorResult<()> {
        let controller = {
            let mut controller = self.controller.lock().map_err(|_| {
                ChannelSourceMonitorError::InvalidState(
                    "monitor controller lock is poisoned".to_owned(),
                )
            })?;
            controller.take()
        };
        let Some(controller) = controller else {
            return Ok(());
        };
        let (response, result) = oneshot::channel();
        let response_result = if self
            .commands
            .send(MonitorCommand::Shutdown { response })
            .await
            .is_ok()
        {
            result.await.map_err(|_| {
                ChannelSourceMonitorError::InvalidState(
                    "monitor shutdown response was dropped".to_owned(),
                )
            })?
        } else {
            Ok(())
        };
        controller
            .await
            .map_err(|error| ChannelSourceMonitorError::Join(error.to_string()))?;
        response_result
    }
}

impl Drop for ChannelSourceMonitor {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

enum MonitorCommand {
    Configure {
        network_scope: NetworkScope,
        catalog_verified: bool,
        configs: Vec<ChannelSourceConfig>,
        response: oneshot::Sender<ChannelSourceMonitorResult<u64>>,
    },
    Shutdown {
        response: oneshot::Sender<ChannelSourceMonitorResult<()>>,
    },
}

#[derive(Default)]
struct MonitorState {
    network_scope: Option<NetworkScope>,
    catalog_verified: bool,
    observation_revision: u64,
    channels: BTreeMap<String, ChannelRuntime>,
}

struct ChannelRuntime {
    config: ChannelSourceConfig,
    cancellation: CancellationToken,
    sources: BTreeMap<String, SourceRuntime>,
}

struct SourceRuntime {
    request: ChannelSourceProbeRequest,
    target_fingerprint: String,
    selected: bool,
    binding_state: Option<ChannelSourceBindingState>,
    last_good: Option<ChannelSourceLastGood>,
    current_failure: Option<ChannelSourceCurrentFailure>,
    comparison_blocks: BTreeMap<u64, ChannelSourceBlockObservation>,
    pending_samples: BTreeSet<u64>,
    consecutive_failures: u32,
    next_probe_at_millis: u64,
    in_flight: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProbeFence {
    network_scope: NetworkScope,
    channel_id: String,
    config_revision: u64,
    source_id: String,
    target_fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeJobKind {
    Regular,
    Block(u64),
}

struct ProbeJob {
    fence: ProbeFence,
    request: ChannelSourceProbeRequest,
    cancellation: CancellationToken,
    kind: ProbeJobKind,
}

struct ProbeTaskResult {
    fence: ProbeFence,
    kind: ProbeJobKind,
    observed_at_unix: u64,
    latency_millis: u64,
    completion: ProbeTaskCompletion,
}

enum ProbeTaskCompletion {
    Regular(ChannelSourceProbeOutput),
    Block(Result<Option<ChannelSourceBlock>, ChannelSourceProbeFailure>),
    Cancelled,
}

#[derive(Clone)]
struct TaskDescriptor {
    fence: ProbeFence,
    kind: ProbeJobKind,
}

async fn run_monitor_controller(
    mut commands: mpsc::Receiver<MonitorCommand>,
    reports: watch::Sender<ChannelSourceMonitorSnapshot>,
    probe: Arc<dyn ChannelSourceProbe>,
    clock: Arc<dyn MonitorClock>,
    shutdown: CancellationToken,
) {
    let mut state = MonitorState::default();
    let mut tasks = JoinSet::new();
    let mut task_descriptors = HashMap::new();

    loop {
        launch_ready_jobs(
            &mut state,
            &mut tasks,
            &mut task_descriptors,
            probe.clone(),
            clock.clone(),
        );
        let deadline = next_probe_deadline(&state, tasks.len());
        tokio::select! {
            () = shutdown.cancelled() => {
                cancel_all(&mut state);
                break;
            }
            command = commands.recv() => {
                match command {
                    Some(MonitorCommand::Configure {
                        network_scope,
                        catalog_verified,
                        configs,
                        response,
                    }) => {
                        let result = configure_monitor(
                            &mut state,
                            &reports,
                            &shutdown,
                            clock.monotonic_millis(),
                            network_scope,
                            catalog_verified,
                            configs,
                        );
                        drop(response.send(result));
                    }
                    Some(MonitorCommand::Shutdown { response }) => {
                        cancel_all(&mut state);
                        shutdown.cancel();
                        drop(response.send(Ok(())));
                        break;
                    }
                    None => {
                        cancel_all(&mut state);
                        break;
                    }
                }
            }
            joined = tasks.join_next_with_id(), if !tasks.is_empty() => {
                handle_joined_task(
                    &mut state,
                    &reports,
                    &mut task_descriptors,
                    clock.as_ref(),
                    joined,
                );
            }
            () = sleep_until(clock.clone(), deadline) => {}
        }
    }

    tasks.abort_all();
    while tasks.join_next().await.is_some() {}
}

fn configure_monitor(
    state: &mut MonitorState,
    reports: &watch::Sender<ChannelSourceMonitorSnapshot>,
    shutdown: &CancellationToken,
    now_millis: u64,
    network_scope: NetworkScope,
    catalog_verified: bool,
    configs: Vec<ChannelSourceConfig>,
) -> ChannelSourceMonitorResult<u64> {
    let configs = validate_monitor_configs(&network_scope, catalog_verified, configs)?;
    let scope_changed = state.network_scope.as_ref() != Some(&network_scope);
    let verification_changed = state.catalog_verified != catalog_verified;
    let mut changed = scope_changed || verification_changed;

    if catalog_verified && !scope_changed {
        for config in &configs {
            if let Some(runtime) = state.channels.get(&config.channel_id)
                && runtime.config != *config
                && runtime.config.config_revision == config.config_revision
            {
                return Err(ChannelSourceMonitorError::InvalidConfiguration(format!(
                    "Channel {} changed without a new configuration revision",
                    config.channel_id
                )));
            }
        }
    }

    if scope_changed || !catalog_verified {
        cancel_all(state);
        if !state.channels.is_empty() {
            changed = true;
        }
        state.channels.clear();
    }
    state.network_scope = Some(network_scope.clone());
    state.catalog_verified = catalog_verified;

    if catalog_verified {
        let desired_channels: BTreeSet<&str> = configs
            .iter()
            .map(|config| config.channel_id.as_str())
            .collect();
        let removed = state
            .channels
            .keys()
            .filter(|channel_id| !desired_channels.contains(channel_id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        for channel_id in removed {
            if let Some(runtime) = state.channels.remove(&channel_id) {
                runtime.cancellation.cancel();
                changed = true;
            }
        }

        for config in configs {
            let replace = match state.channels.get(&config.channel_id) {
                Some(runtime) if runtime.config == config => false,
                Some(_) | None => true,
            };
            if replace {
                if let Some(runtime) = state.channels.remove(&config.channel_id) {
                    runtime.cancellation.cancel();
                }
                state.channels.insert(
                    config.channel_id.clone(),
                    ChannelRuntime::new(config, shutdown.child_token(), now_millis),
                );
                changed = true;
            }
        }
    }

    if changed {
        publish_state(state, reports);
    }
    Ok(state.observation_revision)
}

fn validate_monitor_configs(
    network_scope: &NetworkScope,
    catalog_verified: bool,
    configs: Vec<ChannelSourceConfig>,
) -> ChannelSourceMonitorResult<Vec<ChannelSourceConfig>> {
    if !catalog_verified {
        return Ok(Vec::new());
    }
    let configs = normalize_channel_source_configs(configs)
        .map_err(|error| ChannelSourceMonitorError::InvalidConfiguration(error.to_string()))?;
    let mut source_ids = BTreeSet::new();
    for config in &configs {
        if &config.network_scope != network_scope {
            return Err(ChannelSourceMonitorError::InvalidConfiguration(format!(
                "Channel {} belongs to another network scope",
                config.channel_id
            )));
        }
        for source_id in config
            .sequencer_sources
            .iter()
            .map(|source| source.source_id.as_str())
            .chain(
                config
                    .indexer_source
                    .iter()
                    .map(|source| source.source_id.as_str()),
            )
        {
            if !source_ids.insert(source_id) {
                return Err(ChannelSourceMonitorError::InvalidConfiguration(format!(
                    "duplicate monitor source id `{source_id}`"
                )));
            }
        }
    }
    Ok(configs)
}

impl ChannelRuntime {
    fn new(config: ChannelSourceConfig, cancellation: CancellationToken, now_millis: u64) -> Self {
        let mut sources = BTreeMap::new();
        for source in &config.sequencer_sources {
            let selected =
                config.selected_sequencer_source_id.as_deref() == Some(source.source_id.as_str());
            sources.insert(
                source.source_id.clone(),
                SourceRuntime::sequencer(source, selected, now_millis),
            );
        }
        if let Some(source) = config.indexer_source.as_ref() {
            sources.insert(
                source.source_id.clone(),
                SourceRuntime::indexer(source, now_millis),
            );
        }
        Self {
            config,
            cancellation,
            sources,
        }
    }
}

impl SourceRuntime {
    fn sequencer(source: &ConfiguredSequencerSource, selected: bool, now_millis: u64) -> Self {
        let binding_state = match source.channel_attestation {
            PersistedSequencerAttestation::Pending => ChannelSourceBindingState::Pending,
            PersistedSequencerAttestation::PersistedAttested { .. } => {
                ChannelSourceBindingState::PersistedAttested
            }
        };
        Self::new(
            source.source_id.clone(),
            ChannelSourceRole::Sequencer,
            source.target.clone(),
            selected,
            Some(binding_state),
            now_millis,
        )
    }

    fn indexer(source: &ConfiguredIndexerSource, now_millis: u64) -> Self {
        Self::new(
            source.source_id.clone(),
            ChannelSourceRole::Indexer,
            source.target.clone(),
            false,
            None,
            now_millis,
        )
    }

    fn new(
        source_id: String,
        role: ChannelSourceRole,
        target: ChannelSourceTarget,
        selected: bool,
        binding_state: Option<ChannelSourceBindingState>,
        now_millis: u64,
    ) -> Self {
        let target_fingerprint = target.fingerprint();
        Self {
            request: ChannelSourceProbeRequest {
                source_id,
                role,
                target,
            },
            target_fingerprint,
            selected,
            binding_state,
            last_good: None,
            current_failure: None,
            comparison_blocks: BTreeMap::new(),
            pending_samples: BTreeSet::new(),
            consecutive_failures: 0,
            next_probe_at_millis: now_millis,
            in_flight: false,
        }
    }

    fn health(&self) -> ChannelSourceHealthState {
        if self.binding_state == Some(ChannelSourceBindingState::ChannelMismatch) {
            return ChannelSourceHealthState::ChannelMismatch;
        }
        match (&self.last_good, &self.current_failure) {
            (None, None) => ChannelSourceHealthState::Pending,
            (_, Some(failure)) if failure.kind == super::ChannelSourceFailureKind::Unsupported => {
                ChannelSourceHealthState::Unsupported
            }
            (_, Some(failure)) if failure.kind == super::ChannelSourceFailureKind::Incomplete => {
                ChannelSourceHealthState::Incomplete
            }
            (Some(_), Some(failure)) if failure.stage == ChannelSourceProbeStage::Health => {
                ChannelSourceHealthState::Degraded
            }
            (_, Some(_)) => ChannelSourceHealthState::Unreachable,
            (Some(last_good), None) if !last_good.health_ok => ChannelSourceHealthState::Degraded,
            (Some(_), None) => ChannelSourceHealthState::Reachable,
        }
    }

    fn is_read_eligible(&self) -> bool {
        match self.request.role {
            ChannelSourceRole::Sequencer => self
                .binding_state
                .is_some_and(ChannelSourceBindingState::is_read_eligible),
            ChannelSourceRole::Indexer => true,
        }
    }

    fn comparable_head(&self) -> Option<&ChannelSourceBlockObservation> {
        if !self.is_read_eligible()
            || !matches!(
                self.health(),
                ChannelSourceHealthState::Reachable | ChannelSourceHealthState::Degraded
            )
        {
            return None;
        }
        self.last_good.as_ref()?.head.as_ref().filter(|head| {
            head.header_hash
                .as_deref()
                .is_some_and(|hash| !hash.is_empty())
        })
    }

    fn snapshot(&self) -> ChannelSourceObservation {
        ChannelSourceObservation {
            source_id: self.request.source_id.clone(),
            role: self.request.role,
            selected: self.selected,
            binding_state: self.binding_state,
            health: self.health(),
            last_good: self.last_good.clone(),
            current_failure: self.current_failure.clone(),
            comparison_blocks: self.comparison_blocks.values().cloned().collect(),
        }
    }
}

fn launch_ready_jobs(
    state: &mut MonitorState,
    tasks: &mut JoinSet<ProbeTaskResult>,
    task_descriptors: &mut HashMap<Id, TaskDescriptor>,
    probe: Arc<dyn ChannelSourceProbe>,
    clock: Arc<dyn MonitorClock>,
) {
    let now_millis = clock.monotonic_millis();
    while tasks.len() < MAX_CONCURRENT_PROBES {
        let Some(job) = take_ready_job(state, now_millis) else {
            break;
        };
        let descriptor = TaskDescriptor {
            fence: job.fence.clone(),
            kind: job.kind,
        };
        let handle = tasks.spawn(run_probe_job(job, probe.clone(), clock.clone()));
        task_descriptors.insert(handle.id(), descriptor);
    }
}

fn take_ready_job(state: &mut MonitorState, now_millis: u64) -> Option<ProbeJob> {
    let network_scope = state.network_scope.clone()?;
    for channel in state.channels.values_mut() {
        let channel_id = channel.config.channel_id.clone();
        let config_revision = channel.config.config_revision;
        let cancellation = channel.cancellation.clone();
        for source in channel.sources.values_mut() {
            if !source.in_flight && source.next_probe_at_millis <= now_millis {
                source.in_flight = true;
                return Some(probe_job(
                    &network_scope,
                    &channel_id,
                    config_revision,
                    cancellation,
                    source,
                    ProbeJobKind::Regular,
                ));
            }
        }
    }
    for channel in state.channels.values_mut() {
        let channel_id = channel.config.channel_id.clone();
        let config_revision = channel.config.config_revision;
        let cancellation = channel.cancellation.clone();
        for source in channel.sources.values_mut() {
            if source.in_flight {
                continue;
            }
            if let Some(block_id) = source.pending_samples.pop_first() {
                source.in_flight = true;
                return Some(probe_job(
                    &network_scope,
                    &channel_id,
                    config_revision,
                    cancellation,
                    source,
                    ProbeJobKind::Block(block_id),
                ));
            }
        }
    }
    None
}

fn probe_job(
    network_scope: &NetworkScope,
    channel_id: &str,
    config_revision: u64,
    cancellation: CancellationToken,
    source: &SourceRuntime,
    kind: ProbeJobKind,
) -> ProbeJob {
    ProbeJob {
        fence: ProbeFence {
            network_scope: network_scope.clone(),
            channel_id: channel_id.to_owned(),
            config_revision,
            source_id: source.request.source_id.clone(),
            target_fingerprint: source.target_fingerprint.clone(),
        },
        request: source.request.clone(),
        cancellation,
        kind,
    }
}

async fn run_probe_job(
    job: ProbeJob,
    probe: Arc<dyn ChannelSourceProbe>,
    clock: Arc<dyn MonitorClock>,
) -> ProbeTaskResult {
    let started_at = clock.monotonic_millis();
    let completion = match job.kind {
        ProbeJobKind::Regular => tokio::select! {
            () = job.cancellation.cancelled() => ProbeTaskCompletion::Cancelled,
            output = probe.probe(job.request) => ProbeTaskCompletion::Regular(output),
        },
        ProbeJobKind::Block(block_id) => tokio::select! {
            () = job.cancellation.cancelled() => ProbeTaskCompletion::Cancelled,
            output = probe.block(job.request, block_id) => ProbeTaskCompletion::Block(output),
        },
    };
    ProbeTaskResult {
        fence: job.fence,
        kind: job.kind,
        observed_at_unix: clock.unix_seconds(),
        latency_millis: clock.monotonic_millis().saturating_sub(started_at),
        completion,
    }
}

fn handle_joined_task(
    state: &mut MonitorState,
    reports: &watch::Sender<ChannelSourceMonitorSnapshot>,
    task_descriptors: &mut HashMap<Id, TaskDescriptor>,
    clock: &dyn MonitorClock,
    joined: Option<Result<(Id, ProbeTaskResult), tokio::task::JoinError>>,
) {
    let Some(joined) = joined else {
        return;
    };
    match joined {
        Ok((task_id, result)) => {
            task_descriptors.remove(&task_id);
            if apply_probe_task_result(state, clock.monotonic_millis(), result) {
                publish_state(state, reports);
            }
        }
        Err(error) => {
            let descriptor = task_descriptors.remove(&error.id());
            if let Some(descriptor) = descriptor
                && apply_join_failure(state, clock, descriptor)
            {
                publish_state(state, reports);
            }
        }
    }
}

fn apply_probe_task_result(
    state: &mut MonitorState,
    now_millis: u64,
    result: ProbeTaskResult,
) -> bool {
    if matches!(result.completion, ProbeTaskCompletion::Cancelled) {
        return false;
    }
    let Some(channel) = current_channel_mut(state, &result.fence) else {
        return false;
    };
    let owner_channel_id = channel.config.channel_id.clone();
    let Some(source) = current_source_mut(channel, &result.fence) else {
        return false;
    };
    source.in_flight = false;
    match result.completion {
        ProbeTaskCompletion::Regular(mut output) => {
            normalize_probe_channel_id(&mut output);
            apply_regular_output(
                &owner_channel_id,
                source,
                output,
                result.observed_at_unix,
                result.latency_millis,
                now_millis,
            );
        }
        ProbeTaskCompletion::Block(output) => {
            let ProbeJobKind::Block(block_id) = result.kind else {
                return false;
            };
            apply_block_output(source, block_id, output, result.observed_at_unix);
        }
        ProbeTaskCompletion::Cancelled => return false,
    }
    refresh_sample_requirements(channel);
    true
}

fn normalize_probe_channel_id(output: &mut ChannelSourceProbeOutput) {
    let ChannelSourceProbeOutput::Sequencer(output) = output else {
        return;
    };
    let ChannelSourceProbeFact::Observed(channel_id) = &output.channel_id else {
        return;
    };
    output.channel_id = match normalize_channel_id(channel_id) {
        Ok(channel_id) => ChannelSourceProbeFact::Observed(channel_id),
        Err(_) => ChannelSourceProbeFact::Failed(ChannelSourceProbeFailure {
            kind: super::ChannelSourceFailureKind::Protocol,
            diagnostic: "Sequencer returned an invalid Channel identity".to_owned(),
        }),
    };
}

fn apply_regular_output(
    owner_channel_id: &str,
    source: &mut SourceRuntime,
    output: ChannelSourceProbeOutput,
    observed_at_unix: u64,
    latency_millis: u64,
    now_millis: u64,
) {
    apply_binding_state(owner_channel_id, source, &output);
    let failure = output_failure(&output);
    let complete = complete_observation(&output, observed_at_unix, latency_millis);
    if let Some(last_good) = complete {
        source.last_good = Some(last_good);
        source.comparison_blocks.clear();
        source.pending_samples.clear();
    }

    if let Some((stage, failure)) = failure {
        source.consecutive_failures = source.consecutive_failures.saturating_add(1);
        source.current_failure = Some(ChannelSourceCurrentFailure {
            kind: failure.kind,
            stage,
            diagnostic: failure.diagnostic,
            failed_at_unix: observed_at_unix,
            consecutive_failures: source.consecutive_failures,
        });
    } else {
        source.consecutive_failures = 0;
        source.current_failure = None;
    }
    let delay = next_probe_delay_millis(source.consecutive_failures, &source.request.source_id);
    source.next_probe_at_millis = now_millis.saturating_add(delay);
}

fn apply_binding_state(
    owner_channel_id: &str,
    source: &mut SourceRuntime,
    output: &ChannelSourceProbeOutput,
) {
    let ChannelSourceProbeOutput::Sequencer(output) = output else {
        return;
    };
    let ChannelSourceProbeFact::Observed(channel_id) = &output.channel_id else {
        return;
    };
    if channel_id != owner_channel_id {
        source.binding_state = Some(ChannelSourceBindingState::ChannelMismatch);
        return;
    }
    if source.binding_state == Some(ChannelSourceBindingState::Pending) {
        source.binding_state = Some(ChannelSourceBindingState::RuntimeAttested);
    }
}

fn complete_observation(
    output: &ChannelSourceProbeOutput,
    observed_at_unix: u64,
    latency_millis: u64,
) -> Option<ChannelSourceLastGood> {
    let (health_ok, reported_channel_id, head) = match output {
        ChannelSourceProbeOutput::Sequencer(output) => {
            let ChannelSourceProbeFact::Observed(channel_id) = &output.channel_id else {
                return None;
            };
            let ChannelSourceProbeFact::Observed(head) = &output.head else {
                return None;
            };
            (
                matches!(output.health, ChannelSourceProbeFact::Observed(())),
                Some(channel_id.clone()),
                Some(block_observation(head, observed_at_unix)),
            )
        }
        ChannelSourceProbeOutput::Indexer(output) => {
            let ChannelSourceProbeFact::Observed(head) = &output.head else {
                return None;
            };
            (
                matches!(output.health, ChannelSourceProbeFact::Observed(())),
                None,
                head.as_ref()
                    .map(|head| block_observation(head, observed_at_unix)),
            )
        }
    };
    Some(ChannelSourceLastGood {
        observed_at_unix,
        latency_millis,
        health_ok,
        reported_channel_id,
        head,
    })
}

fn output_failure(
    output: &ChannelSourceProbeOutput,
) -> Option<(ChannelSourceProbeStage, ChannelSourceProbeFailure)> {
    match output {
        ChannelSourceProbeOutput::Sequencer(output) => output
            .head
            .failure()
            .cloned()
            .map(|failure| (ChannelSourceProbeStage::Head, failure))
            .or_else(|| {
                output
                    .channel_id
                    .failure()
                    .cloned()
                    .map(|failure| (ChannelSourceProbeStage::ChannelIdentity, failure))
            })
            .or_else(|| {
                output
                    .health
                    .failure()
                    .cloned()
                    .map(|failure| (ChannelSourceProbeStage::Health, failure))
            }),
        ChannelSourceProbeOutput::Indexer(output) => output
            .head
            .failure()
            .cloned()
            .map(|failure| (ChannelSourceProbeStage::Head, failure))
            .or_else(|| {
                output
                    .health
                    .failure()
                    .cloned()
                    .map(|failure| (ChannelSourceProbeStage::Health, failure))
            }),
    }
}

fn apply_block_output(
    source: &mut SourceRuntime,
    requested_block_id: u64,
    output: Result<Option<ChannelSourceBlock>, ChannelSourceProbeFailure>,
    observed_at_unix: u64,
) {
    let observation = match output {
        Ok(Some(block))
            if block.block_id == requested_block_id && !block.header_hash.is_empty() =>
        {
            block_observation(&block, observed_at_unix)
        }
        Ok(Some(_)) => failed_block_observation(
            requested_block_id,
            observed_at_unix,
            super::ChannelSourceFailureKind::Protocol,
        ),
        Ok(None) => failed_block_observation(
            requested_block_id,
            observed_at_unix,
            super::ChannelSourceFailureKind::Incomplete,
        ),
        Err(failure) => {
            failed_block_observation(requested_block_id, observed_at_unix, failure.kind)
        }
    };
    source
        .comparison_blocks
        .insert(requested_block_id, observation);
}

fn block_observation(
    block: &ChannelSourceBlock,
    observed_at_unix: u64,
) -> ChannelSourceBlockObservation {
    ChannelSourceBlockObservation {
        block_id: block.block_id,
        header_hash: (!block.header_hash.is_empty()).then(|| block.header_hash.clone()),
        parent_hash: block.parent_hash.clone(),
        observed_at_unix,
        failure_kind: None,
    }
}

fn failed_block_observation(
    block_id: u64,
    observed_at_unix: u64,
    failure_kind: super::ChannelSourceFailureKind,
) -> ChannelSourceBlockObservation {
    ChannelSourceBlockObservation {
        block_id,
        header_hash: None,
        parent_hash: None,
        observed_at_unix,
        failure_kind: Some(failure_kind),
    }
}

fn refresh_sample_requirements(channel: &mut ChannelRuntime) {
    let sequencer_heads = channel
        .sources
        .values()
        .filter(|source| source.request.role == ChannelSourceRole::Sequencer)
        .filter_map(|source| {
            source
                .comparable_head()
                .map(|head| (source.request.source_id.clone(), head.block_id))
        })
        .collect::<Vec<_>>();
    let indexer_head = channel
        .sources
        .values()
        .find(|source| source.request.role == ChannelSourceRole::Indexer)
        .and_then(SourceRuntime::comparable_head)
        .map(|head| head.block_id);

    for source in channel.sources.values_mut().filter(|source| {
        source.request.role == ChannelSourceRole::Sequencer && source.comparable_head().is_some()
    }) {
        let Some(source_head) = source.comparable_head().map(|head| head.block_id) else {
            continue;
        };
        let mut required = sequencer_heads
            .iter()
            .filter(|(source_id, block_id)| {
                source_id != &source.request.source_id && *block_id < source_head
            })
            .map(|(_, block_id)| *block_id)
            .collect::<BTreeSet<_>>();
        if let Some(indexer_head) = indexer_head
            && indexer_head < source_head
        {
            required.insert(indexer_head);
        }
        required.retain(|block_id| !source.comparison_blocks.contains_key(block_id));
        source.pending_samples.extend(required);
    }
}

fn apply_join_failure(
    state: &mut MonitorState,
    clock: &dyn MonitorClock,
    descriptor: TaskDescriptor,
) -> bool {
    let Some(channel) = current_channel_mut(state, &descriptor.fence) else {
        return false;
    };
    let Some(source) = current_source_mut(channel, &descriptor.fence) else {
        return false;
    };
    source.in_flight = false;
    let failure = ChannelSourceProbeFailure {
        kind: super::ChannelSourceFailureKind::Unavailable,
        diagnostic: "source probe task failed".to_owned(),
    };
    match descriptor.kind {
        ProbeJobKind::Regular => {
            source.consecutive_failures = source.consecutive_failures.saturating_add(1);
            source.current_failure = Some(ChannelSourceCurrentFailure {
                kind: failure.kind,
                stage: ChannelSourceProbeStage::Task,
                diagnostic: failure.diagnostic,
                failed_at_unix: clock.unix_seconds(),
                consecutive_failures: source.consecutive_failures,
            });
            source.next_probe_at_millis =
                clock
                    .monotonic_millis()
                    .saturating_add(next_probe_delay_millis(
                        source.consecutive_failures,
                        &source.request.source_id,
                    ));
        }
        ProbeJobKind::Block(block_id) => {
            apply_block_output(source, block_id, Err(failure), clock.unix_seconds())
        }
    }
    true
}

fn current_channel_mut<'a>(
    state: &'a mut MonitorState,
    fence: &ProbeFence,
) -> Option<&'a mut ChannelRuntime> {
    if state.network_scope.as_ref() != Some(&fence.network_scope) {
        return None;
    }
    state.channels.get_mut(&fence.channel_id).filter(|channel| {
        channel.config.config_revision == fence.config_revision
            && channel.config.network_scope == fence.network_scope
    })
}

fn current_source_mut<'a>(
    channel: &'a mut ChannelRuntime,
    fence: &ProbeFence,
) -> Option<&'a mut SourceRuntime> {
    channel.sources.get_mut(&fence.source_id).filter(|source| {
        source.target_fingerprint == fence.target_fingerprint
            && source.request.source_id == fence.source_id
    })
}

fn next_probe_delay_millis(consecutive_failures: u32, source_id: &str) -> u64 {
    let base = match consecutive_failures {
        0 | 1 => HEALTHY_INTERVAL_MILLIS,
        2 => 60_000,
        3 => 120_000,
        4 => 240_000,
        _ => MAX_BACKOFF_MILLIS,
    };
    base.saturating_add(stable_jitter_millis(source_id))
        .min(MAX_BACKOFF_MILLIS)
}

fn stable_jitter_millis(source_id: &str) -> u64 {
    let digest = Sha256::digest(source_id.as_bytes());
    let first = digest.first().copied().map_or(0, u64::from);
    let second = digest.get(1).copied().map_or(0, u64::from);
    ((first << 8) | second) % (MAX_JITTER_MILLIS + 1)
}

fn next_probe_deadline(state: &MonitorState, active_tasks: usize) -> Option<u64> {
    if active_tasks >= MAX_CONCURRENT_PROBES {
        return None;
    }
    state
        .channels
        .values()
        .flat_map(|channel| channel.sources.values())
        .filter(|source| !source.in_flight)
        .map(|source| {
            if source.pending_samples.is_empty() {
                source.next_probe_at_millis
            } else {
                0
            }
        })
        .min()
}

fn cancel_all(state: &mut MonitorState) {
    for channel in state.channels.values() {
        channel.cancellation.cancel();
    }
}

fn publish_state(state: &mut MonitorState, reports: &watch::Sender<ChannelSourceMonitorSnapshot>) {
    state.observation_revision = state.observation_revision.saturating_add(1);
    let snapshot = ChannelSourceMonitorSnapshot {
        network_scope: state.network_scope.clone(),
        catalog_verified: state.catalog_verified,
        observation_revision: state.observation_revision,
        channels: state
            .channels
            .values()
            .map(|channel| ChannelSourceObservationSet {
                channel_id: channel.config.channel_id.clone(),
                config_revision: channel.config.config_revision,
                selected_sequencer_source_id: channel.config.selected_sequencer_source_id.clone(),
                observations: channel
                    .sources
                    .values()
                    .map(SourceRuntime::snapshot)
                    .collect(),
            })
            .collect(),
    };
    reports.send_replace(snapshot);
}

type MonitorClockFuture = Pin<Box<dyn Future<Output = ()> + Send + 'static>>;

trait MonitorClock: Send + Sync + 'static {
    fn monotonic_millis(&self) -> u64;
    fn unix_seconds(&self) -> u64;
    fn sleep_until(&self, deadline_millis: u64) -> MonitorClockFuture;
}

struct TokioMonitorClock {
    origin: Instant,
}

impl TokioMonitorClock {
    fn new() -> Self {
        Self {
            origin: Instant::now(),
        }
    }
}

impl MonitorClock for TokioMonitorClock {
    fn monotonic_millis(&self) -> u64 {
        u64::try_from(self.origin.elapsed().as_millis()).unwrap_or(u64::MAX)
    }

    fn unix_seconds(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs())
    }

    fn sleep_until(&self, deadline_millis: u64) -> MonitorClockFuture {
        let deadline = self
            .origin
            .checked_add(Duration::from_millis(deadline_millis));
        Box::pin(async move {
            if let Some(deadline) = deadline {
                tokio::time::sleep_until(deadline).await;
            } else {
                pending::<()>().await;
            }
        })
    }
}

fn sleep_until(clock: Arc<dyn MonitorClock>, deadline: Option<u64>) -> MonitorClockFuture {
    match deadline {
        Some(deadline) => clock.sleep_until(deadline),
        None => Box::pin(pending()),
    }
}

#[cfg(test)]
mod tests;
