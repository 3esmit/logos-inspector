use std::{
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use tokio::time::sleep;

use super::{
    CatalogCandidateActivation, CatalogEngineContext, CatalogL1RangePage, CatalogL1RangeRequest,
    CatalogL1Source, CatalogMetadata, CatalogPageReduction, CatalogRepairConfirmation,
    CatalogSnapshot, ChannelSourceCatalogIdentityRebinder, DirectCatalogL1Source, ZoneCatalog,
    ZoneCatalogPublication, ZoneCatalogRunContext, ZoneCatalogRunMode, ZoneCatalogServiceError,
    ZoneCatalogServiceResult, ZoneCatalogSourceDescriptor, ZoneCatalogWorker,
    ZoneCatalogWorkerFuture, apply_catalog_identity_promotion, catalog_gap_repair_request,
    catalog_prefix_repair_request, confirm_catalog_repair_gap, prepare_catalog_catch_up,
    reduce_catalog_gap_repair, reduce_catalog_page, reduce_catalog_prefix_repair,
    reduce_catalog_repair, repair_catalog_ancestry, verify_catalog_candidate,
};
use crate::{
    inspection::{CatalogVerificationState, NetworkScope},
    support::{config_path::config_dir, time::now_millis},
};

const CATALOG_DIRECTORY: &str = "zone-catalogs";
const CATALOG_FILE_EXTENSION: &str = "redb";
const CATALOG_RANGE_BLOCK_LIMIT: usize = 256;
const CATALOG_POLL_INTERVAL: Duration = Duration::from_secs(5);
const CATALOG_REPAIR_INTERVAL_SECONDS: u64 = 30;

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
                            current_error: Some(format!(
                                "L1 source LIB {source_lib_slot} is behind catalog checkpoint {required_slot}"
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
    context.ensure_current()?;
    let status = source.chain_status().await.map_err(map_source_error)?;
    context.ensure_current()?;
    let network_scope = if let Some(genesis_id) = status.genesis_id {
        NetworkScope::GenesisId { genesis_id }
    } else {
        let time = source.time_status().await.map_err(map_source_error)?;
        context.ensure_current()?;
        let block = source
            .block(status.snapshot.lib.block_id)
            .await
            .map_err(map_source_error)?
            .ok_or_else(|| {
                ZoneCatalogServiceError::Source(
                    "L1 source cannot return its finalized anchor block".to_owned(),
                )
            })?;
        NetworkScope::FinalizedAnchor {
            genesis_time: time.genesis_time_unix_ms.to_string(),
            block_slot: block.checkpoint.slot,
            block_id: block.checkpoint.block_id,
            parent_id: block.checkpoint.parent_id,
        }
    };
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
    use anyhow::{Result, bail};

    use super::*;

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
}
