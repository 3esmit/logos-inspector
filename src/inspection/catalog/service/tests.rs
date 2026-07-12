use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicUsize, Ordering as AtomicOrdering},
        mpsc as std_mpsc,
    },
};

use anyhow::{Context as _, Result, bail, ensure};
use serde_json::json;
use tokio::{runtime::Runtime, sync::mpsc};

use super::*;
use crate::inspection::catalog::{
    CatalogEngineContext, CatalogL1Block, CatalogL1BlockEvent, CatalogL1ChainSnapshot,
    CatalogL1ChainStatus, CatalogL1RangePage, CatalogL1RangeRequest, CatalogL1SourceFuture,
    CatalogL1TimeStatus, CatalogMetadata, CatalogPageReduction, prepare_catalog_catch_up,
    reduce_catalog_page,
};

#[test]
fn promotion_resumes_prepared_transition_and_keeps_catalog_file() -> Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let old_scope = anchor_scope(5, '5', '4');
        let catalog = Arc::new(ZoneCatalog::create(
            &path,
            CatalogMetadata::new(old_scope.clone(), 100)?,
        )?);
        let original_file_id = catalog.snapshot()?.metadata.catalog_file_id;
        let source =
            IdentitySource::new(reference(10, 'a'), Some(id('0'))).with_block(block(5, '5', '4'));
        let context = test_run_context(1);
        let activation = verify_catalog_candidate(&source, &catalog.snapshot()?, &context).await?;
        let promotion = verified_promotion(activation)?;
        ensure!(
            promotion.assurance == CatalogIdentityAssurance::SourceAttested,
            "empty provisional catalog overstated ancestry"
        );
        let rebinder = Arc::new(CountingRebinder::with_failures(1));

        let first = apply_catalog_identity_promotion(
            catalog.clone(),
            promotion.clone(),
            rebinder.clone(),
            &context,
            101,
        )
        .await;
        ensure!(first.is_err(), "injected rebind failure should surface");
        let prepared = catalog.snapshot()?;
        ensure!(
            prepared
                .metadata
                .identity_transition
                .as_ref()
                .is_some_and(|transition| {
                    transition.stage == CatalogIdentityTransitionStage::Prepared
                }),
            "prepared transition was not durable"
        );
        ensure!(
            prepared.metadata.network_scope == old_scope,
            "prepared transition published new scope early"
        );

        let committed = apply_catalog_identity_promotion(
            catalog.clone(),
            promotion,
            rebinder.clone(),
            &context,
            102,
        )
        .await?;
        ensure!(
            committed.metadata.network_scope
                == NetworkScope::GenesisId {
                    genesis_id: id('0'),
                },
            "promotion did not publish genesis scope"
        );
        ensure!(
            committed.metadata.identity_assurance == CatalogIdentityAssurance::SourceAttested,
            "wrong promotion assurance"
        );
        ensure!(
            committed
                .metadata
                .identity_aliases
                .iter()
                .any(|alias| alias.network_scope == old_scope),
            "promotion discarded finalized anchor alias"
        );
        ensure!(
            committed
                .metadata
                .identity_transition
                .as_ref()
                .is_some_and(|transition| {
                    transition.stage == CatalogIdentityTransitionStage::Committed
                }),
            "identity transition did not commit"
        );
        ensure!(
            rebinder.calls() == 2,
            "prepared transition did not retry rebind"
        );
        ensure!(
            committed.metadata.catalog_file_id == original_file_id,
            "promotion replaced catalog file identity"
        );
        drop(catalog);
        let reopened = ZoneCatalog::open(&path)?;
        ensure!(
            reopened.snapshot()?.metadata.network_scope == committed.metadata.network_scope,
            "promoted catalog did not reopen at original path"
        );
        Ok(())
    })
}

#[test]
fn connected_genesis_coverage_strengthens_promotion_assurance() -> Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("catalog.redb");
        let target = reference(10, 'a');
        let old_scope = anchor_scope(10, 'a', '0');
        let catalog = Arc::new(ZoneCatalog::create(
            path,
            CatalogMetadata::new(old_scope, 100)?,
        )?);
        complete_catalog(&catalog, &target)?;
        let source = IdentitySource::new(target, Some(id('0'))).with_block(block(10, 'a', '0'));
        let context = test_run_context(7);

        let activation = verify_catalog_candidate(&source, &catalog.snapshot()?, &context).await?;
        let promotion = verified_promotion(activation)?;
        ensure!(
            promotion.assurance == CatalogIdentityAssurance::AncestryVerified,
            "connected genesis ancestry was not retained"
        );
        let committed = apply_catalog_identity_promotion(
            catalog,
            promotion,
            Arc::new(CountingRebinder::default()),
            &context,
            103,
        )
        .await?;
        ensure!(
            committed.metadata.identity_assurance == CatalogIdentityAssurance::AncestryVerified,
            "committed promotion lost ancestry assurance"
        );
        Ok(())
    })
}

#[test]
fn verification_distinguishes_source_behind_and_identity_mismatch() -> Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let directory = tempfile::tempdir()?;
        let catalog = ZoneCatalog::create(
            directory.path().join("catalog.redb"),
            CatalogMetadata::new(anchor_scope(5, '5', '4'), 100)?,
        )?;
        let snapshot = catalog.snapshot()?;
        let context = test_run_context(1);
        let behind = IdentitySource::new(reference(4, '4'), Some(id('0')));
        let activation = verify_catalog_candidate(&behind, &snapshot, &context).await?;
        ensure!(
            matches!(
                activation,
                CatalogCandidateActivation::SourceBehind {
                    source_lib_slot: 4,
                    required_slot: 5
                }
            ),
            "behind source was not held inactive: {activation:?}"
        );

        let conflict =
            IdentitySource::new(reference(10, 'a'), Some(id('1'))).with_block(block(5, '5', '4'));
        let activation = verify_catalog_candidate(&conflict, &snapshot, &context).await?;
        ensure!(
            matches!(activation, CatalogCandidateActivation::Verified { .. }),
            "provisional catalog should permit promotion to reported genesis"
        );
        let pending = verified_promotion(activation)?;
        catalog.prepare_identity_promotion(0, 101, 1, &pending)?;
        let other_source =
            IdentitySource::new(reference(10, 'a'), Some(id('2'))).with_block(block(5, '5', '4'));
        let activation =
            verify_catalog_candidate(&other_source, &catalog.snapshot()?, &context).await?;
        ensure!(
            matches!(activation, CatalogCandidateActivation::Mismatch { .. }),
            "pending promotion accepted conflicting genesis: {activation:?}"
        );
        Ok(())
    })
}

#[test]
fn replacement_waits_for_commit_fences_stale_publish_and_shutdown_joins() -> Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let (worker, mut signals, release_commit) = LifecycleWorker::new();
        let service = Arc::new(ZoneCatalogService::new(runtime.handle(), Arc::new(worker)));
        let source_one = ZoneCatalogSourceDescriptor::direct_http("https://one.example")?;
        let source_two = ZoneCatalogSourceDescriptor::direct_http("https://two.example")?;
        let first_fingerprint = source_one.fingerprint().to_owned();
        ensure!(
            service.report().verification_state == CatalogVerificationState::Empty
                && !service.report().worker_running
                && signals.started.try_recv().is_err(),
            "unconfigured service started scan work"
        );

        let first_revision = service.configure(source_one.clone()).await?;
        ensure!(first_revision == 1, "first source revision was not one");
        ensure!(
            signals.started.recv().await.as_deref() == Some(first_fingerprint.as_str()),
            "first worker did not start"
        );
        signals
            .commit_started
            .recv()
            .await
            .context("first commit did not start")?;

        let repeated_revision = service.configure(source_one).await?;
        ensure!(
            repeated_revision == first_revision,
            "equivalent source restarted worker"
        );
        ensure!(
            signals.started.try_recv().is_err(),
            "equivalent source spawned duplicate worker"
        );

        let replacement_service = service.clone();
        let replacement =
            runtime.spawn(async move { replacement_service.configure(source_two.clone()).await });
        signals
            .cancel_seen
            .recv()
            .await
            .context("old worker did not observe cancellation")?;
        ensure!(
            signals.started.try_recv().is_err(),
            "replacement started before old commit completed"
        );
        release_commit
            .send(())
            .map_err(|_| anyhow::anyhow!("failed to release simulated commit"))?;
        let replacement_revision = replacement
            .await
            .context("replacement configure task failed")??;
        ensure!(
            replacement_revision == 2,
            "replacement revision did not advance"
        );
        let second_fingerprint = signals
            .started
            .recv()
            .await
            .context("replacement worker did not start")?;
        ensure!(
            second_fingerprint
                == ZoneCatalogSourceDescriptor::direct_http("https://two.example")?.fingerprint(),
            "wrong replacement worker started"
        );
        ensure!(
            signals
                .stale_publish_accepted
                .recv()
                .await
                .is_some_and(|accepted| !accepted),
            "cancelled worker published stale catalog state"
        );
        ensure!(
            signals
                .exited
                .recv()
                .await
                .is_some_and(|fingerprint| fingerprint == first_fingerprint),
            "replacement started before old worker joined"
        );
        ensure!(
            service.report().source_revision == 2 && service.report().worker_running,
            "replacement report was not current"
        );

        service.shutdown().await?;
        ensure!(
            signals
                .exited
                .recv()
                .await
                .is_some_and(|fingerprint| fingerprint == second_fingerprint),
            "shutdown returned before replacement worker exited"
        );
        ensure!(
            !service.report().worker_running,
            "shutdown report still marks worker running"
        );
        Ok(())
    })
}

#[test]
fn drop_fallback_signals_active_worker_cancellation() -> Result<()> {
    let runtime = Runtime::new()?;
    runtime.block_on(async {
        let (worker, mut signals, release_commit) = LifecycleWorker::new();
        let service = ZoneCatalogService::new(runtime.handle(), Arc::new(worker));
        let source = ZoneCatalogSourceDescriptor::direct_http("https://drop.example")?;
        let fingerprint = source.fingerprint().to_owned();
        service.configure(source).await?;
        signals
            .started
            .recv()
            .await
            .context("drop worker did not start")?;
        signals
            .commit_started
            .recv()
            .await
            .context("drop worker commit did not start")?;

        drop(service);
        signals
            .cancel_seen
            .recv()
            .await
            .context("service drop did not signal cancellation")?;
        release_commit
            .send(())
            .map_err(|_| anyhow::anyhow!("failed to release drop commit"))?;
        ensure!(
            signals
                .exited
                .recv()
                .await
                .is_some_and(|exited| exited == fingerprint),
            "drop-cancelled worker did not exit"
        );
        Ok(())
    })
}

fn complete_catalog(catalog: &ZoneCatalog, target: &CatalogBlockReference) -> Result<()> {
    let snapshot = catalog.snapshot()?;
    let batch = prepare_catalog_catch_up(
        &snapshot,
        target.clone(),
        CatalogEngineContext::new(1, 101)?,
    )?
    .context("catch-up preparation did not produce a batch")?;
    let snapshot = catalog.commit_batch(batch)?;
    let source_snapshot = CatalogL1ChainSnapshot {
        tip: reference(15, 'f'),
        lib: target.clone(),
    };
    let page = CatalogL1RangePage {
        events: vec![
            CatalogL1BlockEvent {
                block: block(0, '0', 'f'),
                snapshot: source_snapshot.clone(),
            },
            CatalogL1BlockEvent {
                block: block(10, 'a', '0'),
                snapshot: source_snapshot,
            },
        ],
    };
    let reduction = reduce_catalog_page(&snapshot, page, CatalogEngineContext::new(1, 102)?)?;
    let CatalogPageReduction::Commit {
        batch,
        remaining_events,
    } = reduction
    else {
        bail!("complete fixture did not commit")
    };
    ensure!(remaining_events.is_empty(), "complete fixture left events");
    catalog.commit_batch(*batch)?;
    Ok(())
}

fn verified_promotion(activation: CatalogCandidateActivation) -> Result<CatalogIdentityPromotion> {
    match activation {
        CatalogCandidateActivation::Verified {
            promotion: Some(promotion),
        } => Ok(*promotion),
        other => bail!("expected verified promotion, got {other:?}"),
    }
}

fn test_run_context(source_revision: u64) -> ZoneCatalogRunContext {
    let desired_revision = Arc::new(AtomicU64::new(source_revision));
    let (reports, _receiver) = watch::channel(ZoneCatalogServiceReport {
        source_revision,
        source_fingerprint: Some("test-source".to_owned()),
        verification_state: CatalogVerificationState::Verifying,
        catalog: None,
        current_error: None,
        worker_running: true,
    });
    ZoneCatalogRunContext {
        source_revision,
        source_fingerprint: "test-source".to_owned(),
        run_mode: ZoneCatalogRunMode::Resume,
        cancellation: CancellationToken::new(),
        publisher: CatalogRunPublisher {
            desired_revision,
            reports,
        },
    }
}

fn anchor_scope(slot: u64, block_id: char, parent_id: char) -> NetworkScope {
    NetworkScope::FinalizedAnchor {
        genesis_time: "1000".to_owned(),
        block_slot: slot,
        block_id: id(block_id),
        parent_id: id(parent_id),
    }
}

fn block(slot: u64, block_id: char, parent_id: char) -> CatalogL1Block {
    CatalogL1Block {
        checkpoint: CatalogBlockCheckpoint {
            slot,
            block_id: id(block_id),
            parent_id: id(parent_id),
        },
        payload: json!({
            "header": {
                "slot": slot,
                "id": id(block_id),
                "parent_block": id(parent_id)
            },
            "transactions": []
        }),
    }
}

fn reference(slot: u64, block_id: char) -> CatalogBlockReference {
    CatalogBlockReference {
        slot,
        block_id: id(block_id),
    }
}

fn id(value: char) -> String {
    value.to_string().repeat(64)
}

#[derive(Default)]
struct CountingRebinder {
    calls: AtomicUsize,
    failures: AtomicUsize,
}

impl CountingRebinder {
    fn with_failures(failures: usize) -> Self {
        Self {
            calls: AtomicUsize::new(0),
            failures: AtomicUsize::new(failures),
        }
    }

    fn calls(&self) -> usize {
        self.calls.load(AtomicOrdering::Acquire)
    }
}

impl CatalogIdentityRebinder for CountingRebinder {
    fn rebind<'a>(
        &'a self,
        _old_scope: &'a NetworkScope,
        _new_scope: &'a NetworkScope,
    ) -> CatalogIdentityRebindFuture<'a> {
        Box::pin(async move {
            let _previous_calls = self.calls.fetch_add(1, AtomicOrdering::AcqRel);
            if self
                .failures
                .fetch_update(
                    AtomicOrdering::AcqRel,
                    AtomicOrdering::Acquire,
                    |remaining| remaining.checked_sub(1),
                )
                .is_ok()
            {
                return Err(ZoneCatalogServiceError::Worker(
                    "injected settings rebind failure".to_owned(),
                ));
            }
            Ok(())
        })
    }
}

struct IdentitySource {
    status: CatalogL1ChainStatus,
    time: CatalogL1TimeStatus,
    blocks: BTreeMap<String, CatalogL1Block>,
}

impl IdentitySource {
    fn new(lib: CatalogBlockReference, genesis_id: Option<String>) -> Self {
        Self {
            status: CatalogL1ChainStatus {
                snapshot: CatalogL1ChainSnapshot {
                    tip: reference(lib.slot.saturating_add(5), 'f'),
                    lib,
                },
                genesis_id,
            },
            time: CatalogL1TimeStatus {
                genesis_time_unix_ms: 1000,
                slot_duration_ms: 1000,
                current_slot: 20,
                current_epoch: 1,
            },
            blocks: BTreeMap::new(),
        }
    }

    fn with_block(mut self, block: CatalogL1Block) -> Self {
        self.blocks.insert(block.checkpoint.block_id.clone(), block);
        self
    }
}

impl CatalogL1Source for IdentitySource {
    fn chain_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1ChainStatus> {
        Box::pin(async { Ok(self.status.clone()) })
    }

    fn time_status(&self) -> CatalogL1SourceFuture<'_, CatalogL1TimeStatus> {
        Box::pin(async { Ok(self.time.clone()) })
    }

    fn finalized_range(
        &self,
        _request: CatalogL1RangeRequest,
    ) -> CatalogL1SourceFuture<'_, CatalogL1RangePage> {
        Box::pin(async {
            Err(CatalogL1SourceError::InvalidRequest(
                "identity test does not scan ranges".to_owned(),
            ))
        })
    }

    fn block(&self, block_id: String) -> CatalogL1SourceFuture<'_, Option<CatalogL1Block>> {
        Box::pin(async move { Ok(self.blocks.get(&block_id).cloned()) })
    }
}

struct LifecycleSignals {
    started: mpsc::UnboundedReceiver<String>,
    commit_started: mpsc::UnboundedReceiver<()>,
    cancel_seen: mpsc::UnboundedReceiver<()>,
    stale_publish_accepted: mpsc::UnboundedReceiver<bool>,
    exited: mpsc::UnboundedReceiver<String>,
}

struct LifecycleWorker {
    started: mpsc::UnboundedSender<String>,
    commit_started: mpsc::UnboundedSender<()>,
    cancel_seen: mpsc::UnboundedSender<()>,
    stale_publish_accepted: mpsc::UnboundedSender<bool>,
    exited: mpsc::UnboundedSender<String>,
    release_commit: Mutex<Option<std_mpsc::Receiver<()>>>,
    run_count: AtomicUsize,
}

impl LifecycleWorker {
    fn new() -> (Self, LifecycleSignals, std_mpsc::Sender<()>) {
        let (started, started_rx) = mpsc::unbounded_channel();
        let (commit_started, commit_started_rx) = mpsc::unbounded_channel();
        let (cancel_seen, cancel_seen_rx) = mpsc::unbounded_channel();
        let (stale_publish_accepted, stale_publish_accepted_rx) = mpsc::unbounded_channel();
        let (exited, exited_rx) = mpsc::unbounded_channel();
        let (release_commit, release_commit_rx) = std_mpsc::channel();
        (
            Self {
                started,
                commit_started,
                cancel_seen,
                stale_publish_accepted,
                exited,
                release_commit: Mutex::new(Some(release_commit_rx)),
                run_count: AtomicUsize::new(0),
            },
            LifecycleSignals {
                started: started_rx,
                commit_started: commit_started_rx,
                cancel_seen: cancel_seen_rx,
                stale_publish_accepted: stale_publish_accepted_rx,
                exited: exited_rx,
            },
            release_commit,
        )
    }
}

impl ZoneCatalogWorker for LifecycleWorker {
    fn run(
        self: Arc<Self>,
        source: ZoneCatalogSourceDescriptor,
        context: ZoneCatalogRunContext,
    ) -> ZoneCatalogWorkerFuture {
        Box::pin(async move {
            let run_index = self.run_count.fetch_add(1, AtomicOrdering::AcqRel);
            self.started
                .send(source.fingerprint().to_owned())
                .map_err(|_| ZoneCatalogServiceError::Worker("start signal closed".to_owned()))?;
            if run_index == 0 {
                let release = self
                    .release_commit
                    .lock()
                    .map_err(|_| {
                        ZoneCatalogServiceError::Worker(
                            "release signal lock is poisoned".to_owned(),
                        )
                    })?
                    .take()
                    .ok_or_else(|| {
                        ZoneCatalogServiceError::Worker(
                            "release signal was already consumed".to_owned(),
                        )
                    })?;
                let cancellation = context.cancellation().clone();
                let cancel_seen = self.cancel_seen.clone();
                let cancellation_observer = tokio::spawn(async move {
                    cancellation.cancelled().await;
                    cancel_seen.send(()).map_err(|_| {
                        ZoneCatalogServiceError::Worker("cancel signal closed".to_owned())
                    })
                });
                let commit_started = self.commit_started.clone();
                let commit = context
                    .run_blocking_catalog(move || {
                        commit_started.send(()).map_err(|_| {
                            CatalogError::InvalidInput("commit signal closed".to_owned())
                        })?;
                        release.recv().map_err(|_| {
                            CatalogError::InvalidInput("commit release closed".to_owned())
                        })?;
                        Ok(())
                    })
                    .await;
                cancellation_observer.await.map_err(map_join_error)??;
                match commit {
                    Err(
                        ZoneCatalogServiceError::Cancelled
                        | ZoneCatalogServiceError::StaleRevision { .. },
                    ) => {}
                    Err(error) => return Err(error),
                    Ok(()) => {
                        return Err(ZoneCatalogServiceError::Worker(
                            "cancelled commit remained current".to_owned(),
                        ));
                    }
                }
                let accepted = context.publish(ZoneCatalogPublication {
                    verification_state: CatalogVerificationState::Verified,
                    catalog: None,
                    current_error: None,
                });
                self.stale_publish_accepted.send(accepted).map_err(|_| {
                    ZoneCatalogServiceError::Worker("stale signal closed".to_owned())
                })?;
            } else {
                context.cancellation().cancelled().await;
            }
            self.exited
                .send(source.fingerprint().to_owned())
                .map_err(|_| ZoneCatalogServiceError::Worker("exit signal closed".to_owned()))?;
            Ok(())
        })
    }
}
