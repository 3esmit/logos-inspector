use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use anyhow::{Result, bail, ensure};
use tokio::sync::{Notify, mpsc, oneshot};

use super::*;

#[tokio::test]
async fn monitoring_starts_only_for_verified_catalog_and_probes_immediately() -> Result<()> {
    let (monitor, clock, mut calls) = test_monitor();
    let config = config(7, 1, false);

    monitor
        .configure(scope(), false, vec![config.clone()])
        .await?;
    ensure_no_call(&mut calls).await?;
    monitor.configure(scope(), true, vec![config]).await?;
    let call = receive_call(&mut calls).await?;

    ensure!(call.source_id == source_id(1), "wrong initial source");
    ensure!(clock.monotonic_millis() == 0, "initial probe was delayed");
    call.respond_regular(success_output(&channel_id(), 10, "head"))?;
    let snapshot = wait_for_snapshot(&monitor, |snapshot| {
        first_observation(snapshot)
            .and_then(|observation| observation.last_good.as_ref())
            .is_some()
    })
    .await?;
    ensure!(snapshot.catalog_verified, "verified gate was not published");
    let serialized = serde_json::to_string(&snapshot)?;
    ensure!(
        !serialized.contains("localhost") && !serialized.contains("http://"),
        "runtime observation snapshot exposed a source target"
    );
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn initial_round_includes_configured_indexer() -> Result<()> {
    let (monitor, _clock, mut calls) = test_monitor();
    let mut config = config(8, 1, false);
    config.indexer_source = Some(ConfiguredIndexerSource {
        source_id: source_id(99),
        label: None,
        target: ChannelSourceTarget::Rpc {
            endpoint: "http://localhost:7080/".to_owned(),
        },
    });

    monitor.configure(scope(), true, vec![config]).await?;
    let first = receive_call(&mut calls).await?;
    let second = receive_call(&mut calls).await?;
    let source_ids = BTreeSet::from([first.source_id.clone(), second.source_id.clone()]);
    ensure!(
        source_ids == BTreeSet::from([source_id(1), source_id(99)]),
        "initial round omitted a configured source"
    );
    drop(first);
    drop(second);
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn scheduler_bounds_concurrency_and_excludes_duplicate_source_work() -> Result<()> {
    let (monitor, clock, mut calls) = test_monitor();
    monitor
        .configure(scope(), true, vec![config(3, 10, false)])
        .await?;

    let mut held = Vec::new();
    let mut source_ids = BTreeSet::new();
    for _ in 0..MAX_CONCURRENT_PROBES {
        let call = receive_call(&mut calls).await?;
        ensure!(
            source_ids.insert(call.source_id.clone()),
            "source received duplicate in-flight work"
        );
        held.push(call);
    }
    ensure_no_call(&mut calls).await?;
    clock.advance(600_000);
    ensure_no_call(&mut calls).await?;

    let Some(first) = held.pop() else {
        bail!("no held probe available");
    };
    first.respond_regular(success_output(&channel_id(), 10, "same"))?;
    let next = receive_call(&mut calls).await?;
    ensure!(
        source_ids.insert(next.source_id.clone()),
        "released permit did not start another source"
    );
    ensure!(
        source_ids.len() == MAX_CONCURRENT_PROBES + 1,
        "scheduler exceeded or underused concurrency bound"
    );
    drop(next);
    drop(held);
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn scheduler_uses_deterministic_jittered_failure_backoff() -> Result<()> {
    let (monitor, clock, mut calls) = test_monitor();
    let source_id = source_id(1);
    monitor
        .configure(scope(), true, vec![config(5, 1, false)])
        .await?;

    let first = receive_call(&mut calls).await?;
    first.respond_regular(failed_output(
        super::super::ChannelSourceFailureKind::Unavailable,
    ))?;
    wait_for_snapshot(&monitor, |snapshot| {
        first_observation(snapshot)
            .and_then(|observation| observation.current_failure.as_ref())
            .is_some_and(|failure| failure.consecutive_failures == 1)
    })
    .await?;

    let first_delay = next_probe_delay_millis(1, &source_id);
    ensure!(
        first_delay == HEALTHY_INTERVAL_MILLIS + stable_jitter_millis(&source_id),
        "first failure delay was not deterministic"
    );
    ensure!(
        next_probe_delay_millis(2, &source_id) == 60_000 + stable_jitter_millis(&source_id),
        "second failure did not double backoff"
    );
    ensure!(
        next_probe_delay_millis(3, &source_id) == 120_000 + stable_jitter_millis(&source_id),
        "third failure did not double backoff"
    );
    ensure!(
        next_probe_delay_millis(5, &source_id) == MAX_BACKOFF_MILLIS,
        "failure backoff exceeded maximum"
    );

    clock.advance(first_delay.saturating_sub(1));
    ensure_no_call(&mut calls).await?;
    clock.advance(1);
    let second = receive_call(&mut calls).await?;
    ensure!(second.source_id == source_id, "wrong source retried");
    drop(second);
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn last_good_and_runtime_attestation_survive_failure_but_mismatch_latches() -> Result<()> {
    let (monitor, clock, mut calls) = test_monitor();
    monitor
        .configure(scope(), true, vec![config(11, 1, true)])
        .await?;

    receive_call(&mut calls)
        .await?
        .respond_regular(success_output(&channel_id(), 10, "matching"))?;
    let runtime_attested = wait_for_snapshot(&monitor, |snapshot| {
        first_observation(snapshot).is_some_and(|observation| {
            observation.binding_state == Some(ChannelSourceBindingState::RuntimeAttested)
        })
    })
    .await?;
    let first_good = first_observation(&runtime_attested)
        .and_then(|observation| observation.last_good.clone())
        .ok_or_else(|| anyhow::anyhow!("matching observation was not retained"))?;

    clock.advance(next_probe_delay_millis(0, &source_id(1)));
    receive_call(&mut calls)
        .await?
        .respond_regular(failed_output(
            super::super::ChannelSourceFailureKind::Unavailable,
        ))?;
    let failed = wait_for_snapshot(&monitor, |snapshot| {
        first_observation(snapshot)
            .and_then(|observation| observation.current_failure.as_ref())
            .is_some()
    })
    .await?;
    let failed_observation =
        first_observation(&failed).ok_or_else(|| anyhow::anyhow!("failed source disappeared"))?;
    ensure!(
        failed_observation.last_good.as_ref() == Some(&first_good),
        "transient failure erased last-good observation"
    );
    ensure!(
        failed_observation.binding_state == Some(ChannelSourceBindingState::RuntimeAttested),
        "transient failure erased runtime attestation"
    );

    clock.advance(next_probe_delay_millis(1, &source_id(1)));
    receive_call(&mut calls)
        .await?
        .respond_regular(success_output(&id('9'), 11, "mismatch"))?;
    wait_for_snapshot(&monitor, |snapshot| {
        first_observation(snapshot).is_some_and(|observation| {
            observation.binding_state == Some(ChannelSourceBindingState::ChannelMismatch)
        })
    })
    .await?;

    clock.advance(next_probe_delay_millis(0, &source_id(1)));
    receive_call(&mut calls)
        .await?
        .respond_regular(success_output(&channel_id(), 12, "matching-again"))?;
    let latched = wait_for_snapshot(&monitor, |snapshot| {
        first_observation(snapshot)
            .and_then(|observation| observation.last_good.as_ref())
            .and_then(|last_good| last_good.head.as_ref())
            .is_some_and(|head| head.block_id == 12)
    })
    .await?;
    ensure!(
        first_observation(&latched).and_then(|observation| observation.binding_state)
            == Some(ChannelSourceBindingState::ChannelMismatch),
        "ordinary monitor success cleared latched mismatch"
    );
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn config_revision_cancels_old_work_and_rejects_stale_result() -> Result<()> {
    let (monitor, _clock, mut calls) = test_monitor();
    let old_config = config(2, 1, false);
    monitor
        .configure(scope(), true, vec![old_config.clone()])
        .await?;
    let old_call = receive_call(&mut calls).await?;

    let mut new_config = old_config;
    new_config.config_revision = 3;
    if let Some(source) = new_config.sequencer_sources.first_mut() {
        source.target = ChannelSourceTarget::Rpc {
            endpoint: "http://localhost:4999/".to_owned(),
        };
        source.channel_attestation = PersistedSequencerAttestation::Pending;
    }
    monitor
        .configure(scope(), true, vec![new_config.clone()])
        .await?;
    let new_call = receive_call(&mut calls).await?;
    ensure!(
        new_call.target
            == ChannelSourceTarget::Rpc {
                endpoint: "http://localhost:4999/".to_owned(),
            },
        "replacement probe used stale target"
    );

    let _ = old_call.try_respond_regular(success_output(&channel_id(), 90, "stale"));
    new_call.respond_regular(success_output(&channel_id(), 20, "current"))?;
    let snapshot = wait_for_snapshot(&monitor, |snapshot| {
        snapshot
            .channels
            .first()
            .is_some_and(|channel| channel.config_revision == 3)
            && first_observation(snapshot)
                .and_then(|observation| observation.last_good.as_ref())
                .and_then(|last_good| last_good.head.as_ref())
                .is_some_and(|head| head.block_id == 20)
    })
    .await?;
    ensure!(
        first_observation(&snapshot)
            .and_then(|observation| observation.last_good.as_ref())
            .and_then(|last_good| last_good.head.as_ref())
            .map(|head| head.block_id)
            != Some(90),
        "stale result crossed configuration fence"
    );
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn same_revision_content_change_is_rejected_without_mutating_runtime() -> Result<()> {
    let (monitor, _clock, mut calls) = test_monitor();
    let config = config(6, 1, false);
    monitor
        .configure(scope(), true, vec![config.clone()])
        .await?;
    let active_call = receive_call(&mut calls).await?;

    let mut invalid = config;
    if let Some(source) = invalid.sequencer_sources.first_mut() {
        source.target = ChannelSourceTarget::Rpc {
            endpoint: "http://localhost:4998/".to_owned(),
        };
        source.channel_attestation = PersistedSequencerAttestation::Pending;
    }
    let result = monitor.configure(scope(), true, vec![invalid]).await;
    ensure!(result.is_err(), "same-revision content change was accepted");
    ensure_no_call(&mut calls).await?;
    active_call.respond_regular(success_output(&channel_id(), 15, "still-current"))?;
    let snapshot = wait_for_snapshot(&monitor, |snapshot| {
        first_observation(snapshot)
            .and_then(|observation| observation.last_good.as_ref())
            .and_then(|last_good| last_good.head.as_ref())
            .is_some_and(|head| head.block_id == 15)
    })
    .await?;
    ensure!(
        snapshot
            .channels
            .first()
            .map(|channel| channel.config_revision)
            == Some(6),
        "rejected configuration changed runtime"
    );
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn network_scope_change_cancels_all_old_work() -> Result<()> {
    let (monitor, _clock, mut calls) = test_monitor();
    let old_config = config(2, 1, false);
    monitor
        .configure(scope(), true, vec![old_config.clone()])
        .await?;
    let old_call = receive_call(&mut calls).await?;

    let new_scope = NetworkScope::GenesisId {
        genesis_id: id('b'),
    };
    let mut new_config = old_config;
    new_config.network_scope = new_scope.clone();
    monitor
        .configure(new_scope.clone(), true, vec![new_config])
        .await?;
    let new_call = receive_call(&mut calls).await?;
    let _ = old_call.try_respond_regular(success_output(&channel_id(), 90, "stale-network"));
    new_call.respond_regular(success_output(&channel_id(), 30, "current-network"))?;

    let snapshot = wait_for_snapshot(&monitor, |snapshot| {
        snapshot.network_scope.as_ref() == Some(&new_scope)
            && first_observation(snapshot)
                .and_then(|observation| observation.last_good.as_ref())
                .and_then(|last_good| last_good.head.as_ref())
                .is_some_and(|head| head.block_id == 30)
    })
    .await?;
    ensure!(
        snapshot.network_scope == Some(new_scope),
        "monitor retained old network scope"
    );
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn scheduler_collects_required_overlap_sample_after_head_round() -> Result<()> {
    let (monitor, _clock, mut calls) = test_monitor();
    monitor
        .configure(scope(), true, vec![config(4, 2, false)])
        .await?;

    let first = receive_call(&mut calls).await?;
    let second = receive_call(&mut calls).await?;
    let (lower, higher) = if first.source_id < second.source_id {
        (first, second)
    } else {
        (second, first)
    };
    lower.respond_regular(success_output(&channel_id(), 10, "low"))?;
    higher.respond_regular(success_output(&channel_id(), 12, "high"))?;

    let sample = receive_call(&mut calls).await?;
    ensure!(sample.source_id == source_id(2), "wrong source sampled");
    ensure!(sample.block_id() == Some(10), "wrong overlap block sampled");
    sample.respond_block(Some(block(10, "low")))?;
    let snapshot = wait_for_snapshot(&monitor, |snapshot| {
        snapshot
            .channels
            .first()
            .and_then(|channel| {
                channel
                    .observations
                    .iter()
                    .find(|observation| observation.source_id == source_id(2))
            })
            .is_some_and(|observation| {
                observation.comparison_blocks.iter().any(|block| {
                    block.block_id == 10 && block.header_hash.as_deref() == Some("low")
                })
            })
    })
    .await?;
    ensure!(
        snapshot.observation_revision > 1,
        "sample did not publish independent observation revision"
    );
    monitor.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn observation_revision_is_independent_from_channel_config_revision() -> Result<()> {
    let (monitor, _clock, mut calls) = test_monitor();
    let config_revision = 77;
    let configured_revision = monitor
        .configure(scope(), true, vec![config(config_revision, 1, false)])
        .await?;
    ensure!(
        configured_revision != config_revision,
        "observation revision reused configuration revision"
    );
    receive_call(&mut calls)
        .await?
        .respond_regular(success_output(&channel_id(), 10, "head"))?;
    let snapshot = wait_for_snapshot(&monitor, |snapshot| {
        snapshot.observation_revision > configured_revision
    })
    .await?;
    ensure!(
        snapshot
            .channels
            .first()
            .map(|channel| channel.config_revision)
            == Some(config_revision),
        "source result changed configuration revision"
    );
    monitor.shutdown().await?;
    Ok(())
}

#[test]
fn only_attested_binding_states_are_read_eligible() {
    assert!(ChannelSourceBindingState::PersistedAttested.is_read_eligible());
    assert!(ChannelSourceBindingState::RuntimeAttested.is_read_eligible());
    assert!(!ChannelSourceBindingState::Pending.is_read_eligible());
    assert!(!ChannelSourceBindingState::ChannelMismatch.is_read_eligible());
}

struct ManualClockState {
    now_millis: AtomicU64,
    notify: Notify,
}

#[derive(Clone)]
struct ManualClock {
    state: Arc<ManualClockState>,
}

impl ManualClock {
    fn new() -> Self {
        Self {
            state: Arc::new(ManualClockState {
                now_millis: AtomicU64::new(0),
                notify: Notify::new(),
            }),
        }
    }

    fn advance(&self, millis: u64) {
        self.state.now_millis.fetch_add(millis, Ordering::SeqCst);
        self.state.notify.notify_one();
    }
}

impl MonitorClock for ManualClock {
    fn monotonic_millis(&self) -> u64 {
        self.state.now_millis.load(Ordering::SeqCst)
    }

    fn unix_seconds(&self) -> u64 {
        1_700_000_000_u64.saturating_add(self.monotonic_millis() / 1_000)
    }

    fn sleep_until(&self, deadline_millis: u64) -> MonitorClockFuture {
        let state = self.state.clone();
        Box::pin(async move {
            loop {
                if state.now_millis.load(Ordering::SeqCst) >= deadline_millis {
                    return;
                }
                state.notify.notified().await;
            }
        })
    }
}

struct ScriptedProbe {
    calls: mpsc::UnboundedSender<ProbeCall>,
}

impl ChannelSourceProbe for ScriptedProbe {
    fn probe(
        self: Arc<Self>,
        request: ChannelSourceProbeRequest,
    ) -> super::super::probe::ChannelSourceProbeFuture<ChannelSourceProbeOutput> {
        let (response, result) = oneshot::channel();
        let call = ProbeCall {
            source_id: request.source_id,
            target: request.target,
            action: ProbeCallAction::Regular(response),
        };
        let sent = self.calls.send(call).is_ok();
        Box::pin(async move {
            if !sent {
                return failed_output(super::super::ChannelSourceFailureKind::Unavailable);
            }
            result.await.unwrap_or_else(|_| {
                failed_output(super::super::ChannelSourceFailureKind::Unavailable)
            })
        })
    }

    fn block(
        self: Arc<Self>,
        request: ChannelSourceProbeRequest,
        block_id: u64,
    ) -> super::super::probe::ChannelSourceProbeFuture<
        Result<Option<ChannelSourceBlock>, ChannelSourceProbeFailure>,
    > {
        let (response, result) = oneshot::channel();
        let call = ProbeCall {
            source_id: request.source_id,
            target: request.target,
            action: ProbeCallAction::Block { block_id, response },
        };
        let sent = self.calls.send(call).is_ok();
        Box::pin(async move {
            if !sent {
                return Err(probe_failure(
                    super::super::ChannelSourceFailureKind::Unavailable,
                ));
            }
            result.await.unwrap_or_else(|_| {
                Err(probe_failure(
                    super::super::ChannelSourceFailureKind::Unavailable,
                ))
            })
        })
    }
}

struct ProbeCall {
    source_id: String,
    target: ChannelSourceTarget,
    action: ProbeCallAction,
}

enum ProbeCallAction {
    Regular(oneshot::Sender<ChannelSourceProbeOutput>),
    Block {
        block_id: u64,
        response: oneshot::Sender<Result<Option<ChannelSourceBlock>, ChannelSourceProbeFailure>>,
    },
}

impl ProbeCall {
    fn respond_regular(self, output: ChannelSourceProbeOutput) -> Result<()> {
        if self.try_respond_regular(output) {
            Ok(())
        } else {
            bail!("regular probe response was dropped")
        }
    }

    fn try_respond_regular(self, output: ChannelSourceProbeOutput) -> bool {
        match self.action {
            ProbeCallAction::Regular(response) => response.send(output).is_ok(),
            ProbeCallAction::Block { .. } => false,
        }
    }

    fn respond_block(self, block: Option<ChannelSourceBlock>) -> Result<()> {
        match self.action {
            ProbeCallAction::Block { response, .. } => response
                .send(Ok(block))
                .map_err(|_| anyhow::anyhow!("block probe response was dropped")),
            ProbeCallAction::Regular(_) => bail!("regular probe cannot accept block response"),
        }
    }

    fn block_id(&self) -> Option<u64> {
        match self.action {
            ProbeCallAction::Regular(_) => None,
            ProbeCallAction::Block { block_id, .. } => Some(block_id),
        }
    }
}

fn test_monitor() -> (
    ChannelSourceMonitor,
    Arc<ManualClock>,
    mpsc::UnboundedReceiver<ProbeCall>,
) {
    let (sender, calls) = mpsc::unbounded_channel();
    let clock = Arc::new(ManualClock::new());
    let monitor = ChannelSourceMonitor::with_dependencies(
        &Handle::current(),
        Arc::new(ScriptedProbe { calls: sender }),
        clock.clone(),
    );
    (monitor, clock, calls)
}

async fn receive_call(calls: &mut mpsc::UnboundedReceiver<ProbeCall>) -> Result<ProbeCall> {
    tokio::time::timeout(Duration::from_secs(1), calls.recv())
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for source probe"))?
        .ok_or_else(|| anyhow::anyhow!("source probe channel closed"))
}

async fn ensure_no_call(calls: &mut mpsc::UnboundedReceiver<ProbeCall>) -> Result<()> {
    let result = tokio::time::timeout(Duration::from_millis(20), calls.recv()).await;
    ensure!(result.is_err(), "unexpected source probe was scheduled");
    Ok(())
}

async fn wait_for_snapshot<F>(
    monitor: &ChannelSourceMonitor,
    predicate: F,
) -> Result<ChannelSourceMonitorSnapshot>
where
    F: Fn(&ChannelSourceMonitorSnapshot) -> bool,
{
    for _ in 0..100 {
        let snapshot = monitor.snapshot();
        if predicate(&snapshot) {
            return Ok(snapshot);
        }
        tokio::task::yield_now().await;
    }
    bail!("monitor snapshot did not reach expected state")
}

fn first_observation(snapshot: &ChannelSourceMonitorSnapshot) -> Option<&ChannelSourceObservation> {
    snapshot
        .channels
        .first()
        .and_then(|channel| channel.observations.first())
}

fn success_output(channel_id: &str, block_id: u64, hash: &str) -> ChannelSourceProbeOutput {
    ChannelSourceProbeOutput {
        health: ChannelSourceProbeFact::Observed(()),
        channel_id: ChannelSourceProbeFact::Observed(channel_id.to_owned()),
        head: ChannelSourceProbeFact::Observed(Some(block(block_id, hash))),
    }
}

fn failed_output(kind: super::super::ChannelSourceFailureKind) -> ChannelSourceProbeOutput {
    ChannelSourceProbeOutput {
        health: ChannelSourceProbeFact::Failed(probe_failure(kind)),
        channel_id: ChannelSourceProbeFact::Failed(probe_failure(kind)),
        head: ChannelSourceProbeFact::Failed(probe_failure(kind)),
    }
}

fn probe_failure(kind: super::super::ChannelSourceFailureKind) -> ChannelSourceProbeFailure {
    ChannelSourceProbeFailure {
        kind,
        diagnostic: "scripted probe failure".to_owned(),
    }
}

fn block(block_id: u64, hash: &str) -> ChannelSourceBlock {
    ChannelSourceBlock {
        block_id,
        header_hash: hash.to_owned(),
        parent_hash: Some("parent".to_owned()),
    }
}

fn config(revision: u64, source_count: u8, pending: bool) -> ChannelSourceConfig {
    let channel_id = channel_id();
    let sequencer_sources = (1..=source_count)
        .map(|number| {
            let target = ChannelSourceTarget::Rpc {
                endpoint: format!("http://localhost:{}/", 3000_u16 + u16::from(number)),
            };
            ConfiguredSequencerSource {
                source_id: source_id(number),
                label: None,
                target: target.clone(),
                channel_attestation: if pending {
                    PersistedSequencerAttestation::Pending
                } else {
                    PersistedSequencerAttestation::PersistedAttested {
                        channel_id: channel_id.clone(),
                        target_fingerprint: target.fingerprint(),
                        attested_at_unix: 1,
                    }
                },
            }
        })
        .collect::<Vec<_>>();
    ChannelSourceConfig {
        network_scope: scope(),
        channel_id,
        config_revision: revision,
        selected_sequencer_source_id: sequencer_sources
            .first()
            .map(|source| source.source_id.clone()),
        sequencer_sources,
        indexer_source: None,
    }
}

fn scope() -> NetworkScope {
    NetworkScope::GenesisId {
        genesis_id: id('a'),
    }
}

fn channel_id() -> String {
    id('8')
}

fn id(character: char) -> String {
    character.to_string().repeat(64)
}

fn source_id(number: u8) -> String {
    format!("src_{number:032x}")
}
