use std::{
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::time::sleep;

use super::{
    CatalogCandidateActivation, CatalogEngineContext, CatalogL1RangePage, CatalogL1RangeRequest,
    CatalogL1Source, CatalogMetadata, CatalogPageReduction, CatalogRepairConfirmation,
    CatalogSnapshot, ChannelSourceCatalogIdentityRebinder, DirectCatalogL1Source,
    MAX_CATALOG_L1_RANGE_BLOCKS, ZoneCatalog, ZoneCatalogPublication, ZoneCatalogRunContext,
    ZoneCatalogRunMode, ZoneCatalogServiceError, ZoneCatalogServiceResult,
    ZoneCatalogSourceDescriptor, ZoneCatalogWorker, ZoneCatalogWorkerFuture,
    apply_catalog_identity_promotion, catalog_gap_repair_request, catalog_prefix_repair_request,
    confirm_catalog_repair_gap, prepare_catalog_catch_up, reduce_catalog_gap_repair,
    reduce_catalog_page, reduce_catalog_prefix_repair, reduce_catalog_repair,
    repair_catalog_ancestry, verify_catalog_candidate,
};
use crate::{
    inspection::{CatalogVerificationState, NetworkScope},
    support::{config_path::config_dir, time::now_millis},
};

const CATALOG_DIRECTORY: &str = "zone-catalogs";
const CATALOG_FILE_EXTENSION: &str = "redb";
const CATALOG_RANGE_BLOCK_LIMIT: usize = MAX_CATALOG_L1_RANGE_BLOCKS;
const CATALOG_CATCH_UP_PAGE_INTERVAL: Duration = Duration::from_millis(500);
const CATALOG_POLL_INTERVAL: Duration = Duration::from_secs(5);
const CATALOG_REPAIR_INTERVAL_SECONDS: u64 = 30;

trait CatalogPagePacer {
    fn wait<'a>(
        &'a self,
        context: &'a ZoneCatalogRunContext,
    ) -> impl Future<Output = ZoneCatalogServiceResult<()>> + Send + 'a;
}

struct IntervalCatalogPagePacer {
    interval: Duration,
}

impl CatalogPagePacer for IntervalCatalogPagePacer {
    fn wait<'a>(
        &'a self,
        context: &'a ZoneCatalogRunContext,
    ) -> impl Future<Output = ZoneCatalogServiceResult<()>> + Send + 'a {
        wait_for_retry(context, self.interval)
    }
}

#[derive(Debug, Clone)]
pub struct DirectZoneCatalogWorker {
    catalog_directory: PathBuf,
}

impl DirectZoneCatalogWorker {
    pub fn for_config_dir() -> ZoneCatalogServiceResult<Self> {
        let config = config_dir().map_err(|error| {
            ZoneCatalogServiceError::Worker(format!(
                "failed to locate Zone Catalog directory: {error}"
            ))
        })?;
        Ok(Self::new(config.join(CATALOG_DIRECTORY)))
    }

    #[must_use]
    pub fn new(catalog_directory: PathBuf) -> Self {
        Self { catalog_directory }
    }
}

impl ZoneCatalogWorker for DirectZoneCatalogWorker {
    fn run(
        self: Arc<Self>,
        descriptor: ZoneCatalogSourceDescriptor,
        context: ZoneCatalogRunContext,
    ) -> ZoneCatalogWorkerFuture {
        Box::pin(async move { self.run_direct(descriptor, context).await })
    }
}

impl DirectZoneCatalogWorker {
    async fn run_direct(
        &self,
        descriptor: ZoneCatalogSourceDescriptor,
        context: ZoneCatalogRunContext,
    ) -> ZoneCatalogServiceResult<()> {
        let source =
            Arc::new(DirectCatalogL1Source::new(descriptor.endpoint()).map_err(map_source_error)?);
        let catalog_directory =
            source_catalog_directory(&self.catalog_directory, descriptor.fingerprint())?;
        tokio::fs::create_dir_all(&catalog_directory)
            .await
            .map_err(|error| {
                ZoneCatalogServiceError::Worker(format!(
                    "failed to create Zone Catalog directory: {error}"
                ))
            })?;
        let catalog = self
            .open_or_create_catalog(&catalog_directory, source.as_ref(), &context)
            .await?;
        context.ensure_current()?;
        run_catalog_scan(catalog, source, &context).await
    }

    async fn open_or_create_catalog(
        &self,
        catalog_directory: &Path,
        source: &dyn CatalogL1Source,
        context: &ZoneCatalogRunContext,
    ) -> ZoneCatalogServiceResult<Arc<ZoneCatalog>> {
        let paths = catalog_paths(catalog_directory).await?;
        for path in paths {
            context.ensure_current()?;
            let snapshot = match open_catalog_snapshot(path.clone(), context).await {
                Ok(snapshot) => snapshot,
                Err(_) => {
                    quarantine_catalog(path, "invalid", context).await?;
                    continue;
                }
            };
            let _published = context.publish(ZoneCatalogPublication {
                verification_state: CatalogVerificationState::CachedUnverified,
                catalog: Some(Arc::new(snapshot.clone())),
                current_error: None,
            });
            loop {
                match verify_catalog_candidate(source, &snapshot, context).await? {
                    CatalogCandidateActivation::Verified { promotion } => {
                        if context.run_mode() == ZoneCatalogRunMode::Rebuild {
                            quarantine_catalog(path, "rebuild", context).await?;
                            break;
                        }
                        let catalog = open_catalog(path, context).await?;
                        let snapshot = if let Some(promotion) = promotion {
                            apply_catalog_identity_promotion(
                                catalog.clone(),
                                *promotion,
                                Arc::new(ChannelSourceCatalogIdentityRebinder),
                                context,
                                now_unix(),
                            )
                            .await?
                        } else {
                            catalog_snapshot(catalog.clone(), context).await?
                        };
                        publish_verified(context, snapshot);
                        return Ok(catalog);
                    }
                    CatalogCandidateActivation::SourceBehind {
                        source_lib_slot,
                        required_slot,
                    } => {
                        let _published = context.publish(ZoneCatalogPublication {
                            verification_state: CatalogVerificationState::SourceBehind,
                            catalog: Some(Arc::new(snapshot.clone())),
                            current_error: Some(source_behind_message(
                                source_lib_slot,
                                required_slot,
                            )),
                        });
                        wait_for_retry(context, CATALOG_POLL_INTERVAL).await?;
                    }
                    CatalogCandidateActivation::CachedUnverified { detail } => {
                        let _published = context.publish(ZoneCatalogPublication {
                            verification_state: CatalogVerificationState::CachedUnverified,
                            catalog: Some(Arc::new(snapshot.clone())),
                            current_error: Some(detail),
                        });
                        wait_for_retry(context, CATALOG_POLL_INTERVAL).await?;
                    }
                    CatalogCandidateActivation::Mismatch { detail } => {
                        let _published = context.publish(ZoneCatalogPublication {
                            verification_state: CatalogVerificationState::Mismatch,
                            catalog: None,
                            current_error: Some(detail),
                        });
                        quarantine_catalog(path, "mismatch", context).await?;
                        break;
                    }
                }
            }
        }
        let catalog = create_catalog(catalog_directory, source, context).await?;
        publish_verified(context, catalog_snapshot(catalog.clone(), context).await?);
        Ok(catalog)
    }
}

fn source_catalog_directory(
    root: &Path,
    source_fingerprint: &str,
) -> ZoneCatalogServiceResult<PathBuf> {
    let digest = source_fingerprint.strip_prefix("sha256:").ok_or_else(|| {
        ZoneCatalogServiceError::InvalidSource(
            "catalog source fingerprint has an unknown format".to_owned(),
        )
    })?;
    if digest.len() != 64
        || !digest
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ZoneCatalogServiceError::InvalidSource(
            "catalog source fingerprint is invalid".to_owned(),
        ));
    }
    Ok(root.join(digest.to_ascii_lowercase()))
}

async fn run_catalog_scan(
    catalog: Arc<ZoneCatalog>,
    source: Arc<dyn CatalogL1Source>,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<()> {
    let pacer = IntervalCatalogPagePacer {
        interval: CATALOG_CATCH_UP_PAGE_INTERVAL,
    };
    run_catalog_scan_with_pacer(catalog, source, context, &pacer).await
}

async fn run_catalog_scan_with_pacer(
    catalog: Arc<ZoneCatalog>,
    source: Arc<dyn CatalogL1Source>,
    context: &ZoneCatalogRunContext,
    pacer: &impl CatalogPagePacer,
) -> ZoneCatalogServiceResult<()> {
    let mut next_repair_at_unix = 0;
    let mut no_progress_round = 0_u32;
    loop {
        context.ensure_current()?;
        let status = source.chain_status().await.map_err(map_source_error)?;
        context.ensure_current()?;
        let target = status.snapshot.lib;
        let mut snapshot = catalog_snapshot(catalog.clone(), context).await?;
        let mut made_progress = false;
        let engine_context = engine_context(context, &snapshot)?;
        if let Some(batch) = prepare_catalog_catch_up(&snapshot, target.clone(), engine_context)
            .map_err(map_engine_error)?
        {
            snapshot = commit_catalog_batch(catalog.clone(), batch, context).await?;
            made_progress = true;
            publish_verified(context, snapshot.clone());
        }

        loop {
            context.ensure_current()?;
            let cursor = snapshot
                .traversal
                .as_ref()
                .and_then(|traversal| traversal.ingestion_cursor.as_ref());
            if cursor.is_some_and(|cursor| cursor == &target) {
                break;
            }
            let slot_from = cursor.map_or(0, |cursor| cursor.slot.saturating_add(1));
            let request =
                CatalogL1RangeRequest::new(slot_from, target.clone(), CATALOG_RANGE_BLOCK_LIMIT)
                    .map_err(map_source_error)?;
            let page = source
                .finalized_range(request)
                .await
                .map_err(map_source_error)?;
            context.ensure_current()?;
            if page.events.is_empty() {
                break;
            }
            let previous_revision = snapshot.metadata.catalog_revision;
            snapshot =
                apply_catalog_page(catalog.clone(), source.as_ref(), snapshot, page, context)
                    .await?;
            made_progress |= snapshot.metadata.catalog_revision != previous_revision;
            publish_verified(context, snapshot.clone());
            let reached_target = snapshot
                .traversal
                .as_ref()
                .and_then(|traversal| traversal.ingestion_cursor.as_ref())
                .is_some_and(|cursor| cursor == &target);
            if !reached_target {
                pacer.wait(context).await?;
            }
        }

        let now = now_unix();
        if now >= next_repair_at_unix {
            let previous_revision = snapshot.metadata.catalog_revision;
            snapshot =
                repair_one_catalog_gap(catalog.clone(), source.as_ref(), snapshot, context).await?;
            next_repair_at_unix = now.saturating_add(CATALOG_REPAIR_INTERVAL_SECONDS);
            if snapshot.metadata.catalog_revision != previous_revision {
                made_progress = true;
                publish_verified(context, snapshot);
            }
        }

        let delay = if made_progress {
            no_progress_round = 0;
            CATALOG_POLL_INTERVAL
        } else {
            no_progress_round = no_progress_round.saturating_add(1);
            no_progress_delay(no_progress_round)
        };
        wait_for_retry(context, delay).await?;
    }
}

fn no_progress_delay(round: u32) -> Duration {
    let seconds = match round {
        0 | 1 => 2,
        2 => 5,
        3 => 15,
        _ => 30,
    };
    Duration::from_secs(seconds)
}

async fn apply_catalog_page(
    catalog: Arc<ZoneCatalog>,
    source: &dyn CatalogL1Source,
    mut snapshot: CatalogSnapshot,
    page: CatalogL1RangePage,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogSnapshot> {
    let mut reduction = reduce_catalog_page(&snapshot, page, engine_context(context, &snapshot)?)
        .map_err(map_engine_error)?;
    loop {
        context.ensure_current()?;
        reduction = match reduction {
            CatalogPageReduction::NoProgress => return Ok(snapshot),
            CatalogPageReduction::Commit {
                batch,
                remaining_events,
            } => {
                snapshot = commit_catalog_batch(catalog.clone(), *batch, context).await?;
                publish_verified(context, snapshot.clone());
                if remaining_events.is_empty() {
                    return Ok(snapshot);
                }
                reduce_catalog_page(
                    &snapshot,
                    CatalogL1RangePage {
                        events: remaining_events,
                    },
                    engine_context(context, &snapshot)?,
                )
                .map_err(map_engine_error)?
            }
            CatalogPageReduction::RepairRequired {
                request,
                pending_events,
            } => {
                let outcome = repair_catalog_ancestry(source, &request)
                    .await
                    .map_err(map_engine_error)?;
                context.ensure_current()?;
                reduce_catalog_repair(
                    &snapshot,
                    &request,
                    pending_events,
                    outcome,
                    engine_context(context, &snapshot)?,
                )
                .map_err(map_engine_error)?
            }
            CatalogPageReduction::GapConfirmationRequired {
                request,
                pending_events,
                outcome,
            } => {
                let confirmation = confirm_current_catalog(
                    source,
                    &snapshot,
                    request.lower_checkpoint.clone(),
                    context,
                )
                .await?;
                confirm_catalog_repair_gap(
                    &snapshot,
                    &request,
                    pending_events,
                    *outcome,
                    &confirmation,
                    engine_context(context, &snapshot)?,
                )
                .map_err(map_engine_error)?
            }
        };
    }
}

async fn confirm_current_catalog(
    source: &dyn CatalogL1Source,
    snapshot: &CatalogSnapshot,
    lower_checkpoint: Option<super::CatalogBlockReference>,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogRepairConfirmation> {
    match verify_catalog_candidate(source, snapshot, context).await? {
        CatalogCandidateActivation::Verified { promotion: None } => {}
        CatalogCandidateActivation::Verified { promotion: Some(_) } => {
            return Err(ZoneCatalogServiceError::Worker(
                "catalog identity strengthened during ancestry repair; retry required".to_owned(),
            ));
        }
        CatalogCandidateActivation::SourceBehind { .. }
        | CatalogCandidateActivation::CachedUnverified { .. }
        | CatalogCandidateActivation::Mismatch { .. } => {
            return Err(ZoneCatalogServiceError::Worker(
                "catalog identity or checkpoint changed during ancestry repair".to_owned(),
            ));
        }
    }
    let target_lib = snapshot
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.target_lib.clone())
        .ok_or_else(|| {
            ZoneCatalogServiceError::Worker("catalog repair has no fixed target LIB".to_owned())
        })?;
    let upper_frontier_checkpoint = snapshot
        .frontier
        .as_ref()
        .and_then(|frontier| frontier.checkpoint.clone());
    Ok(CatalogRepairConfirmation::new(
        target_lib,
        lower_checkpoint,
        upper_frontier_checkpoint,
    ))
}

async fn repair_one_catalog_gap(
    catalog: Arc<ZoneCatalog>,
    source: &dyn CatalogL1Source,
    snapshot: CatalogSnapshot,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogSnapshot> {
    let initial_context = engine_context(context, &snapshot)?;
    if snapshot.frontier.as_ref().is_some_and(|frontier| {
        frontier.prefix_status == crate::inspection::CoveragePrefixStatus::Unavailable
    }) {
        let request =
            catalog_prefix_repair_request(&snapshot, initial_context).map_err(map_engine_error)?;
        let outcome = repair_catalog_ancestry(source, &request)
            .await
            .map_err(map_engine_error)?;
        context.ensure_current()?;
        let confirmation = confirm_current_catalog(source, &snapshot, None, context).await?;
        let Some(batch) = reduce_catalog_prefix_repair(
            &snapshot,
            outcome,
            &confirmation,
            engine_context(context, &snapshot)?,
        )
        .map_err(map_engine_error)?
        else {
            return Ok(snapshot);
        };
        return commit_catalog_batch(catalog, batch, context).await;
    }

    let Some(gap) = snapshot.gaps.first() else {
        return Ok(snapshot);
    };
    let gap_id = gap.gap_id.clone();
    let lower_checkpoint = Some(gap.lower_checkpoint.clone());
    let request = catalog_gap_repair_request(&snapshot, &gap_id, initial_context)
        .map_err(map_engine_error)?;
    let outcome = repair_catalog_ancestry(source, &request)
        .await
        .map_err(map_engine_error)?;
    context.ensure_current()?;
    let confirmation =
        confirm_current_catalog(source, &snapshot, lower_checkpoint, context).await?;
    let batch = reduce_catalog_gap_repair(
        &snapshot,
        &gap_id,
        outcome,
        &confirmation,
        engine_context(context, &snapshot)?,
    )
    .map_err(map_engine_error)?;
    commit_catalog_batch(catalog, batch, context).await
}

async fn create_catalog(
    directory: &Path,
    source: &dyn CatalogL1Source,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<Arc<ZoneCatalog>> {
    let network_scope = resolve_catalog_network_scope(source, || context.ensure_current()).await?;
    let metadata = CatalogMetadata::new(network_scope, now_unix()).map_err(map_catalog_error)?;
    let path = directory.join(format!(
        "{}.{}",
        metadata.catalog_file_id, CATALOG_FILE_EXTENSION
    ));
    let catalog = context
        .run_blocking_catalog(move || ZoneCatalog::create(path, metadata))
        .await?;
    Ok(Arc::new(catalog))
}

async fn resolve_catalog_network_scope<F>(
    source: &dyn CatalogL1Source,
    mut ensure_current: F,
) -> ZoneCatalogServiceResult<NetworkScope>
where
    F: FnMut() -> ZoneCatalogServiceResult<()>,
{
    ensure_current()?;
    let status = source.chain_status().await.map_err(map_source_error)?;
    ensure_current()?;
    let network_scope = if let Some(genesis_id) = status.genesis_id {
        NetworkScope::GenesisId { genesis_id }
    } else {
        let time = source.time_status().await.map_err(map_source_error)?;
        ensure_current()?;
        let request =
            CatalogL1RangeRequest::new(0, status.snapshot.lib, 1).map_err(map_source_error)?;
        let page = source
            .finalized_range(request)
            .await
            .map_err(map_source_error)?;
        ensure_current()?;
        let block = page
            .events
            .into_iter()
            .next()
            .map(|event| event.block)
            .ok_or_else(|| {
                ZoneCatalogServiceError::Source(
                    "L1 source cannot return its earliest finalized anchor block".to_owned(),
                )
            })?;
        NetworkScope::FinalizedAnchor {
            genesis_time: time.genesis_time_unix_ms.to_string(),
            block_slot: block.checkpoint.slot,
            block_id: block.checkpoint.block_id,
            parent_id: block.checkpoint.parent_id,
        }
    };
    Ok(network_scope)
}

async fn catalog_paths(directory: &Path) -> ZoneCatalogServiceResult<Vec<PathBuf>> {
    let mut entries = tokio::fs::read_dir(directory).await.map_err(|error| {
        ZoneCatalogServiceError::Worker(format!("failed to read Zone Catalog directory: {error}"))
    })?;
    let mut paths = Vec::new();
    while let Some(entry) = entries.next_entry().await.map_err(|error| {
        ZoneCatalogServiceError::Worker(format!("failed to read Zone Catalog entry: {error}"))
    })? {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) == Some(CATALOG_FILE_EXTENSION)
        {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

async fn open_catalog_snapshot(
    path: PathBuf,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogSnapshot> {
    context
        .run_blocking_catalog(move || ZoneCatalog::open_read_only(path)?.snapshot())
        .await
}

async fn open_catalog(
    path: PathBuf,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<Arc<ZoneCatalog>> {
    let catalog = context
        .run_blocking_catalog(move || ZoneCatalog::open(path))
        .await?;
    Ok(Arc::new(catalog))
}

async fn catalog_snapshot(
    catalog: Arc<ZoneCatalog>,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogSnapshot> {
    context
        .run_blocking_catalog(move || catalog.snapshot())
        .await
}

async fn commit_catalog_batch(
    catalog: Arc<ZoneCatalog>,
    batch: super::CatalogBatch,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogSnapshot> {
    context
        .run_blocking_catalog(move || catalog.commit_batch(batch))
        .await
}

async fn quarantine_catalog(
    path: PathBuf,
    reason: &str,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<()> {
    let quarantined = path.with_extension(format!("{reason}-{}.quarantine", now_millis()));
    context.ensure_current()?;
    tokio::fs::rename(path, quarantined)
        .await
        .map_err(|error| {
            ZoneCatalogServiceError::Worker(format!("failed to quarantine Zone Catalog: {error}"))
        })?;
    context.ensure_current()
}

fn publish_verified(context: &ZoneCatalogRunContext, snapshot: CatalogSnapshot) {
    let _published = context.publish(ZoneCatalogPublication {
        verification_state: CatalogVerificationState::Verified,
        catalog: Some(Arc::new(snapshot)),
        current_error: None,
    });
}

fn source_behind_message(source_lib_slot: u64, required_slot: u64) -> String {
    format!(
        "Bedrock is still syncing: finalized LIB {source_lib_slot} has not reached Zone Catalog checkpoint {required_slot}. Zones resume automatically when it catches up."
    )
}

async fn wait_for_retry(
    context: &ZoneCatalogRunContext,
    duration: Duration,
) -> ZoneCatalogServiceResult<()> {
    tokio::select! {
        () = context.cancellation().cancelled() => Err(ZoneCatalogServiceError::Cancelled),
        () = sleep(duration) => context.ensure_current(),
    }
}

fn engine_context(
    context: &ZoneCatalogRunContext,
    snapshot: &CatalogSnapshot,
) -> ZoneCatalogServiceResult<CatalogEngineContext> {
    let updated_at = now_unix().max(snapshot.metadata.updated_at_unix);
    CatalogEngineContext::new(context.source_revision(), updated_at).map_err(map_engine_error)
}

fn now_unix() -> u64 {
    now_millis() / 1_000
}

fn map_source_error(error: super::CatalogL1SourceError) -> ZoneCatalogServiceError {
    let detail = match error {
        super::CatalogL1SourceError::InvalidRequest(_) => "catalog L1 request was invalid",
        super::CatalogL1SourceError::Unavailable(_) => "catalog L1 source is unavailable",
        super::CatalogL1SourceError::InvalidResponse(_) => {
            "catalog L1 source returned invalid data"
        }
    };
    ZoneCatalogServiceError::Source(detail.to_owned())
}

fn map_catalog_error(_error: super::CatalogError) -> ZoneCatalogServiceError {
    ZoneCatalogServiceError::Catalog("catalog storage or validation failed".to_owned())
}

fn map_engine_error(_error: super::CatalogEngineError) -> ZoneCatalogServiceError {
    ZoneCatalogServiceError::Worker("catalog ingestion validation failed".to_owned())
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::{Result, bail, ensure};
    use tokio::sync::mpsc;

    use super::*;

    struct FallbackIdentitySource {
        status: super::super::CatalogL1ChainStatus,
        time: super::super::CatalogL1TimeStatus,
        anchor: super::super::CatalogL1Block,
    }

    struct PagingSource {
        status: super::super::CatalogL1ChainStatus,
        calls: AtomicUsize,
        call_events: mpsc::UnboundedSender<usize>,
    }

    struct BlockingPacer {
        wait_events: mpsc::UnboundedSender<()>,
    }

    impl CatalogPagePacer for BlockingPacer {
        fn wait<'a>(
            &'a self,
            context: &'a ZoneCatalogRunContext,
        ) -> impl Future<Output = ZoneCatalogServiceResult<()>> + Send + 'a {
            wait_for_blocking_pacer(&self.wait_events, context)
        }
    }

    async fn wait_for_blocking_pacer(
        wait_events: &mpsc::UnboundedSender<()>,
        context: &ZoneCatalogRunContext,
    ) -> ZoneCatalogServiceResult<()> {
        let _sent = wait_events.send(());
        context.cancellation().cancelled().await;
        Err(ZoneCatalogServiceError::Cancelled)
    }

    impl CatalogL1Source for PagingSource {
        fn chain_status(
            &self,
        ) -> super::super::CatalogL1SourceFuture<'_, super::super::CatalogL1ChainStatus> {
            Box::pin(async { Ok(self.status.clone()) })
        }

        fn time_status(
            &self,
        ) -> super::super::CatalogL1SourceFuture<'_, super::super::CatalogL1TimeStatus> {
            Box::pin(async {
                Err(super::super::CatalogL1SourceError::InvalidRequest(
                    "paging test does not use time status".to_owned(),
                ))
            })
        }

        fn finalized_range(
            &self,
            request: CatalogL1RangeRequest,
        ) -> super::super::CatalogL1SourceFuture<'_, CatalogL1RangePage> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            let _sent = self.call_events.send(call);
            let snapshot = self.status.snapshot.clone();
            Box::pin(async move {
                let block = match request.slot_from() {
                    0 => test_block(0, '0', '0'),
                    1 => test_block(1, 'a', '0'),
                    2 => test_block(2, 'b', 'a'),
                    _ => {
                        return Ok(CatalogL1RangePage { events: Vec::new() });
                    }
                };
                Ok(CatalogL1RangePage {
                    events: vec![super::super::CatalogL1BlockEvent { block, snapshot }],
                })
            })
        }

        fn block(
            &self,
            _block_id: String,
        ) -> super::super::CatalogL1SourceFuture<'_, Option<super::super::CatalogL1Block>> {
            Box::pin(async {
                Err(super::super::CatalogL1SourceError::InvalidRequest(
                    "paging test does not repair ancestry".to_owned(),
                ))
            })
        }
    }

    impl CatalogL1Source for FallbackIdentitySource {
        fn chain_status(
            &self,
        ) -> super::super::CatalogL1SourceFuture<'_, super::super::CatalogL1ChainStatus> {
            Box::pin(async { Ok(self.status.clone()) })
        }

        fn time_status(
            &self,
        ) -> super::super::CatalogL1SourceFuture<'_, super::super::CatalogL1TimeStatus> {
            Box::pin(async { Ok(self.time.clone()) })
        }

        fn finalized_range(
            &self,
            request: CatalogL1RangeRequest,
        ) -> super::super::CatalogL1SourceFuture<'_, CatalogL1RangePage> {
            Box::pin(async move {
                if request.slot_from() != 0
                    || request.blocks_limit().get() != 1
                    || request.target_lib() != &self.status.snapshot.lib
                {
                    return Err(super::super::CatalogL1SourceError::InvalidRequest(
                        "fallback identity did not request the earliest finalized block".to_owned(),
                    ));
                }
                Ok(CatalogL1RangePage {
                    events: vec![super::super::CatalogL1BlockEvent {
                        block: self.anchor.clone(),
                        snapshot: self.status.snapshot.clone(),
                    }],
                })
            })
        }

        fn block(
            &self,
            _block_id: String,
        ) -> super::super::CatalogL1SourceFuture<'_, Option<super::super::CatalogL1Block>> {
            Box::pin(async {
                Err(super::super::CatalogL1SourceError::InvalidRequest(
                    "fallback identity must not use the moving LIB block".to_owned(),
                ))
            })
        }
    }

    fn fallback_identity_source(lib_slot: u64, lib_id: char) -> FallbackIdentitySource {
        FallbackIdentitySource {
            status: super::super::CatalogL1ChainStatus {
                snapshot: super::super::CatalogL1ChainSnapshot {
                    tip: super::super::CatalogBlockReference {
                        slot: lib_slot.saturating_add(5),
                        block_id: id('f'),
                    },
                    lib: super::super::CatalogBlockReference {
                        slot: lib_slot,
                        block_id: id(lib_id),
                    },
                },
                genesis_id: None,
            },
            time: super::super::CatalogL1TimeStatus {
                genesis_time_unix_ms: 1_000,
                slot_duration_ms: 1_000,
                current_slot: lib_slot.saturating_add(10),
                current_epoch: 1,
            },
            anchor: super::super::CatalogL1Block {
                checkpoint: super::super::CatalogBlockCheckpoint {
                    slot: 1_008,
                    block_id: id('9'),
                    parent_id: id('8'),
                },
                payload: serde_json::json!({}),
            },
        }
    }

    fn id(value: char) -> String {
        value.to_string().repeat(64)
    }

    fn test_block(slot: u64, block_id: char, parent_id: char) -> super::super::CatalogL1Block {
        super::super::CatalogL1Block {
            checkpoint: super::super::CatalogBlockCheckpoint {
                slot,
                block_id: id(block_id),
                parent_id: id(parent_id),
            },
            payload: serde_json::json!({
                "header": {
                    "slot": slot,
                    "id": id(block_id),
                    "parent_block": id(parent_id),
                },
                "transactions": []
            }),
        }
    }

    #[test]
    fn source_namespace_and_worker_errors_do_not_expose_endpoint_text() -> Result<()> {
        let descriptor =
            ZoneCatalogSourceDescriptor::direct_http("https://catalog-user-visible.example/path")?;
        let directory = source_catalog_directory(Path::new("catalogs"), descriptor.fingerprint())?;
        let unavailable = map_source_error(super::super::CatalogL1SourceError::Unavailable(
            "request failed for https://catalog-user-visible.example/path".to_owned(),
        ));
        let combined = format!("{}{unavailable}", directory.display());
        if combined.contains("catalog-user-visible.example")
            || directory
                .file_name()
                .and_then(|name| name.to_str())
                .is_none_or(|name| name.len() != 64)
        {
            bail!("catalog source namespace or error leaked endpoint: {combined}");
        }
        Ok(())
    }

    #[test]
    fn worker_range_limit_satisfies_source_request_contract() -> Result<()> {
        let target = super::super::CatalogBlockReference {
            slot: 1,
            block_id: "a".repeat(64),
        };

        CatalogL1RangeRequest::new(0, target, CATALOG_RANGE_BLOCK_LIMIT)?;
        Ok(())
    }

    #[test]
    fn catch_up_pages_use_a_bounded_nonzero_pacing_interval() -> Result<()> {
        if CATALOG_CATCH_UP_PAGE_INTERVAL.is_zero()
            || CATALOG_CATCH_UP_PAGE_INTERVAL >= CATALOG_POLL_INTERVAL
        {
            bail!("catalog catch-up page pacing is disabled or exceeds foreground polling");
        }
        Ok(())
    }

    #[test]
    fn source_behind_message_explains_sync_and_automatic_recovery() -> Result<()> {
        let message = source_behind_message(0, 691_337);

        ensure!(
            message
                == "Bedrock is still syncing: finalized LIB 0 has not reached Zone Catalog checkpoint 691337. Zones resume automatically when it catches up.",
            "source-behind status did not explain recovery: {message}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn catch_up_waits_between_range_pages_and_remains_cancellable() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let catalog = Arc::new(ZoneCatalog::create(
            directory.path().join("catalog.redb"),
            CatalogMetadata::new(
                NetworkScope::GenesisId {
                    genesis_id: id('0'),
                },
                100,
            )?,
        )?);
        let target = super::super::CatalogBlockReference {
            slot: 2,
            block_id: id('b'),
        };
        let status = super::super::CatalogL1ChainStatus {
            snapshot: super::super::CatalogL1ChainSnapshot {
                tip: target.clone(),
                lib: target,
            },
            genesis_id: Some(id('0')),
        };
        let (call_events, mut calls) = mpsc::unbounded_channel();
        let source = Arc::new(PagingSource {
            status,
            calls: AtomicUsize::new(0),
            call_events,
        });
        let context = ZoneCatalogRunContext::test_context(1);
        let (wait_events, mut waits) = mpsc::unbounded_channel();
        let pacer = BlockingPacer { wait_events };
        let task_context = context.clone();
        let task = tokio::spawn(async move {
            run_catalog_scan_with_pacer(catalog, source, &task_context, &pacer).await
        });

        ensure!(
            calls.recv().await == Some(1),
            "first range page was not requested"
        );
        if waits.recv().await.is_none() {
            bail!("catch-up pacer was not entered: {:?}", task.await?);
        }
        ensure!(
            calls.try_recv().is_err(),
            "second range page bypassed catch-up pacing"
        );
        context.cancellation().cancel();
        ensure!(
            matches!(task.await?, Err(ZoneCatalogServiceError::Cancelled)),
            "catalog scan did not stop through the paced cancellation boundary"
        );
        Ok(())
    }

    #[tokio::test]
    async fn provisional_scope_uses_stable_earliest_finalized_anchor() -> Result<()> {
        let first =
            resolve_catalog_network_scope(&fallback_identity_source(10_000, 'a'), || Ok(()))
                .await?;
        let later =
            resolve_catalog_network_scope(&fallback_identity_source(20_000, 'b'), || Ok(()))
                .await?;

        if first != later {
            bail!("provisional network scope changed when only the current LIB advanced");
        }
        if first
            != (NetworkScope::FinalizedAnchor {
                genesis_time: "1000".to_owned(),
                block_slot: 1_008,
                block_id: id('9'),
                parent_id: id('8'),
            })
        {
            bail!("provisional network scope did not use the earliest finalized block");
        }
        Ok(())
    }
}
