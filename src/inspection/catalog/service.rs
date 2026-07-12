use std::{
    fmt,
    future::Future,
    pin::Pin,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};

use sha2::{Digest as _, Sha256};
use tokio::{
    runtime::Handle,
    sync::{mpsc, oneshot, watch},
    task::JoinHandle,
};
use tokio_util::sync::CancellationToken;

use super::{
    CatalogBlockCheckpoint, CatalogBlockReference, CatalogError, CatalogIdentityAssurance,
    CatalogIdentityTransition, CatalogIdentityTransitionStage, CatalogL1Source,
    CatalogL1SourceError, CatalogSnapshot, DirectCatalogL1Source, NetworkIdentityAlias,
    ZoneCatalog,
};
use crate::{
    inspection::zones::{CatalogVerificationState, CoveragePrefixStatus, NetworkScope},
    source_routing::channel_sources::rebind_channel_source_configs,
};

pub type ZoneCatalogServiceResult<T> = Result<T, ZoneCatalogServiceError>;
pub type ZoneCatalogWorkerFuture =
    Pin<Box<dyn Future<Output = ZoneCatalogServiceResult<()>> + Send + 'static>>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneCatalogRunMode {
    Resume,
    Rebuild,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ZoneCatalogServiceError {
    InvalidSource(String),
    InvalidState(String),
    Source(String),
    Catalog(String),
    Worker(String),
    Join(String),
    Cancelled,
    StaleRevision { expected: u64, current: u64 },
}

impl fmt::Display for ZoneCatalogServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSource(detail) => write!(formatter, "invalid catalog source: {detail}"),
            Self::InvalidState(detail) => {
                write!(formatter, "invalid catalog service state: {detail}")
            }
            Self::Source(detail) => write!(formatter, "catalog source failed: {detail}"),
            Self::Catalog(detail) => write!(formatter, "Zone Catalog failed: {detail}"),
            Self::Worker(detail) => write!(formatter, "Zone Catalog worker failed: {detail}"),
            Self::Join(detail) => write!(formatter, "Zone Catalog task failed: {detail}"),
            Self::Cancelled => write!(formatter, "Zone Catalog run was cancelled"),
            Self::StaleRevision { expected, current } => write!(
                formatter,
                "Zone Catalog source revision is stale: expected {expected}, current {current}"
            ),
        }
    }
}

impl std::error::Error for ZoneCatalogServiceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneCatalogSourceDescriptor {
    endpoint: String,
    fingerprint: String,
}

impl ZoneCatalogSourceDescriptor {
    pub fn direct_http(endpoint: impl AsRef<str>) -> ZoneCatalogServiceResult<Self> {
        let source = DirectCatalogL1Source::new(endpoint)
            .map_err(|error| ZoneCatalogServiceError::InvalidSource(error.to_string()))?;
        let endpoint = source.endpoint().to_owned();
        let mut digest = Sha256::new();
        digest.update(b"catalog-l1-direct-http\0");
        digest.update(endpoint.as_bytes());
        Ok(Self {
            endpoint,
            fingerprint: format!("sha256:{}", hex::encode(digest.finalize())),
        })
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    #[must_use]
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneCatalogServiceReport {
    pub source_revision: u64,
    pub source_fingerprint: Option<String>,
    pub verification_state: CatalogVerificationState,
    pub catalog: Option<Arc<CatalogSnapshot>>,
    pub current_error: Option<String>,
    pub worker_running: bool,
}

impl Default for ZoneCatalogServiceReport {
    fn default() -> Self {
        Self {
            source_revision: 0,
            source_fingerprint: None,
            verification_state: CatalogVerificationState::Empty,
            catalog: None,
            current_error: None,
            worker_running: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneCatalogPublication {
    pub verification_state: CatalogVerificationState,
    pub catalog: Option<Arc<CatalogSnapshot>>,
    pub current_error: Option<String>,
}

pub trait ZoneCatalogWorker: Send + Sync + 'static {
    fn run(
        self: Arc<Self>,
        source: ZoneCatalogSourceDescriptor,
        context: ZoneCatalogRunContext,
    ) -> ZoneCatalogWorkerFuture;
}

#[derive(Clone)]
pub struct ZoneCatalogRunContext {
    source_revision: u64,
    source_fingerprint: String,
    run_mode: ZoneCatalogRunMode,
    cancellation: CancellationToken,
    publisher: CatalogRunPublisher,
}

impl ZoneCatalogRunContext {
    #[must_use]
    pub const fn source_revision(&self) -> u64 {
        self.source_revision
    }

    #[must_use]
    pub fn source_fingerprint(&self) -> &str {
        &self.source_fingerprint
    }

    #[must_use]
    pub const fn run_mode(&self) -> ZoneCatalogRunMode {
        self.run_mode
    }

    #[must_use]
    pub fn cancellation(&self) -> &CancellationToken {
        &self.cancellation
    }

    #[must_use]
    pub fn is_current(&self) -> bool {
        !self.cancellation.is_cancelled()
            && self.publisher.desired_revision.load(Ordering::Acquire) == self.source_revision
    }

    pub fn ensure_current(&self) -> ZoneCatalogServiceResult<()> {
        if self.cancellation.is_cancelled() {
            return Err(ZoneCatalogServiceError::Cancelled);
        }
        let current = self.publisher.desired_revision.load(Ordering::Acquire);
        if current != self.source_revision {
            return Err(ZoneCatalogServiceError::StaleRevision {
                expected: self.source_revision,
                current,
            });
        }
        Ok(())
    }

    pub async fn run_blocking_catalog<T, F>(&self, task: F) -> ZoneCatalogServiceResult<T>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, CatalogError> + Send + 'static,
    {
        self.ensure_current()?;
        let value = tokio::task::spawn_blocking(task)
            .await
            .map_err(map_join_error)?
            .map_err(map_catalog_error)?;
        self.ensure_current()?;
        Ok(value)
    }

    #[must_use]
    pub fn publish(&self, publication: ZoneCatalogPublication) -> bool {
        let mut accepted = false;
        self.publisher.reports.send_if_modified(|report| {
            if self.cancellation.is_cancelled()
                || self.publisher.desired_revision.load(Ordering::Acquire) != self.source_revision
                || report.source_revision != self.source_revision
            {
                return false;
            }
            report.verification_state = publication.verification_state;
            report.catalog = publication.catalog;
            report.current_error = publication.current_error;
            accepted = true;
            true
        });
        accepted
    }

    fn finish(&self, error: Option<String>) {
        self.publisher.reports.send_if_modified(|report| {
            if self.publisher.desired_revision.load(Ordering::Acquire) != self.source_revision
                || report.source_revision != self.source_revision
            {
                return false;
            }
            report.worker_running = false;
            if let Some(error) = error {
                report.current_error = Some(error);
                if report.verification_state == CatalogVerificationState::Verifying {
                    report.verification_state = if report.catalog.is_some() {
                        CatalogVerificationState::CachedUnverified
                    } else {
                        CatalogVerificationState::Empty
                    };
                }
            }
            true
        });
    }
}

#[derive(Clone)]
struct CatalogRunPublisher {
    desired_revision: Arc<AtomicU64>,
    reports: watch::Sender<ZoneCatalogServiceReport>,
}

impl CatalogRunPublisher {
    fn begin_revision(&self, source_revision: u64, source: &ZoneCatalogSourceDescriptor) {
        self.desired_revision
            .store(source_revision, Ordering::Release);
        self.reports.send_modify(|report| {
            report.source_revision = source_revision;
            report.source_fingerprint = Some(source.fingerprint().to_owned());
            report.verification_state = CatalogVerificationState::Verifying;
            report.catalog = None;
            report.current_error = None;
            report.worker_running = false;
        });
    }

    fn mark_running(&self, source_revision: u64) {
        self.reports.send_if_modified(|report| {
            if self.desired_revision.load(Ordering::Acquire) != source_revision
                || report.source_revision != source_revision
            {
                return false;
            }
            report.worker_running = true;
            true
        });
    }

    fn clear(&self, source_revision: u64) {
        self.desired_revision
            .store(source_revision, Ordering::Release);
        self.reports.send_modify(|report| {
            *report = ZoneCatalogServiceReport {
                source_revision,
                ..ZoneCatalogServiceReport::default()
            };
        });
    }
}

pub struct ZoneCatalogService {
    commands: mpsc::Sender<CatalogServiceCommand>,
    reports: watch::Receiver<ZoneCatalogServiceReport>,
    shutdown: CancellationToken,
    controller: Mutex<Option<JoinHandle<()>>>,
}

impl ZoneCatalogService {
    #[must_use]
    pub fn new(runtime: &Handle, worker: Arc<dyn ZoneCatalogWorker>) -> Self {
        let (commands, receiver) = mpsc::channel(8);
        let (report_sender, reports) = watch::channel(ZoneCatalogServiceReport::default());
        let shutdown = CancellationToken::new();
        let publisher = CatalogRunPublisher {
            desired_revision: Arc::new(AtomicU64::new(0)),
            reports: report_sender,
        };
        let controller = runtime.spawn(run_catalog_controller(
            runtime.clone(),
            worker,
            receiver,
            publisher,
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
    pub fn report(&self) -> ZoneCatalogServiceReport {
        self.reports.borrow().clone()
    }

    pub async fn configure(
        &self,
        source: ZoneCatalogSourceDescriptor,
    ) -> ZoneCatalogServiceResult<u64> {
        let (response, result) = oneshot::channel();
        self.commands
            .send(CatalogServiceCommand::Configure { source, response })
            .await
            .map_err(|_| {
                ZoneCatalogServiceError::InvalidState(
                    "catalog service controller is stopped".to_owned(),
                )
            })?;
        result.await.map_err(|_| {
            ZoneCatalogServiceError::InvalidState(
                "catalog service controller dropped its response".to_owned(),
            )
        })?
    }

    pub async fn retry(&self) -> ZoneCatalogServiceResult<u64> {
        self.control(CatalogControl::Retry).await
    }

    pub async fn rebuild(&self) -> ZoneCatalogServiceResult<u64> {
        self.control(CatalogControl::Rebuild).await
    }

    async fn control(&self, control: CatalogControl) -> ZoneCatalogServiceResult<u64> {
        let (response, result) = oneshot::channel();
        self.commands
            .send(CatalogServiceCommand::Control { control, response })
            .await
            .map_err(|_| {
                ZoneCatalogServiceError::InvalidState(
                    "catalog service controller is stopped".to_owned(),
                )
            })?;
        result.await.map_err(|_| {
            ZoneCatalogServiceError::InvalidState(
                "catalog service controller dropped its response".to_owned(),
            )
        })?
    }

    pub async fn shutdown(&self) -> ZoneCatalogServiceResult<()> {
        let controller = {
            let mut controller = self.controller.lock().map_err(|_| {
                ZoneCatalogServiceError::InvalidState(
                    "catalog service controller lock is poisoned".to_owned(),
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
            .send(CatalogServiceCommand::Shutdown { response })
            .await
            .is_ok()
        {
            result.await.map_err(|_| {
                ZoneCatalogServiceError::InvalidState(
                    "catalog service shutdown response was dropped".to_owned(),
                )
            })?
        } else {
            Ok(())
        };
        controller
            .await
            .map_err(|error| ZoneCatalogServiceError::Join(error.to_string()))?;
        response_result
    }
}

impl Drop for ZoneCatalogService {
    fn drop(&mut self) {
        self.shutdown.cancel();
    }
}

enum CatalogServiceCommand {
    Configure {
        source: ZoneCatalogSourceDescriptor,
        response: oneshot::Sender<ZoneCatalogServiceResult<u64>>,
    },
    Control {
        control: CatalogControl,
        response: oneshot::Sender<ZoneCatalogServiceResult<u64>>,
    },
    Shutdown {
        response: oneshot::Sender<ZoneCatalogServiceResult<()>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CatalogControl {
    Retry,
    Rebuild,
}

struct ActiveCatalogRun {
    source: ZoneCatalogSourceDescriptor,
    cancellation: CancellationToken,
    join: JoinHandle<()>,
}

async fn run_catalog_controller(
    runtime: Handle,
    worker: Arc<dyn ZoneCatalogWorker>,
    mut commands: mpsc::Receiver<CatalogServiceCommand>,
    publisher: CatalogRunPublisher,
    shutdown: CancellationToken,
) {
    let mut active = None;
    loop {
        tokio::select! {
            () = shutdown.cancelled() => {
                invalidate_catalog_run(&publisher);
                drop(stop_catalog_run(&mut active).await);
                break;
            }
            command = commands.recv() => {
                match command {
                    Some(CatalogServiceCommand::Configure { source, response }) => {
                        let result = configure_catalog_run(
                            &runtime,
                            worker.clone(),
                            &publisher,
                            &shutdown,
                            &mut active,
                            CatalogRunRequest {
                                source,
                                force_restart: false,
                                run_mode: ZoneCatalogRunMode::Resume,
                            },
                        ).await;
                        drop(response.send(result));
                    }
                    Some(CatalogServiceCommand::Control { control, response }) => {
                        let result = restart_catalog_run(
                            &runtime,
                            worker.clone(),
                            &publisher,
                            &shutdown,
                            &mut active,
                            control,
                        ).await;
                        drop(response.send(result));
                    }
                    Some(CatalogServiceCommand::Shutdown { response }) => {
                        invalidate_catalog_run(&publisher);
                        shutdown.cancel();
                        let result = stop_catalog_run(&mut active).await;
                        drop(response.send(result));
                        break;
                    }
                    None => {
                        invalidate_catalog_run(&publisher);
                        drop(stop_catalog_run(&mut active).await);
                        break;
                    }
                }
            }
        }
    }
}

async fn configure_catalog_run(
    runtime: &Handle,
    worker: Arc<dyn ZoneCatalogWorker>,
    publisher: &CatalogRunPublisher,
    shutdown: &CancellationToken,
    active: &mut Option<ActiveCatalogRun>,
    request: CatalogRunRequest,
) -> ZoneCatalogServiceResult<u64> {
    let CatalogRunRequest {
        source,
        force_restart,
        run_mode,
    } = request;
    if let Some(current) = active.as_ref()
        && current.source == source
        && !current.join.is_finished()
        && !force_restart
    {
        return Ok(publisher.desired_revision.load(Ordering::Acquire));
    }

    let source_revision = next_source_revision(publisher)?;
    publisher.begin_revision(source_revision, &source);
    stop_catalog_run(active).await?;
    if shutdown.is_cancelled() {
        return Err(ZoneCatalogServiceError::Cancelled);
    }

    let cancellation = shutdown.child_token();
    let context = ZoneCatalogRunContext {
        source_revision,
        source_fingerprint: source.fingerprint().to_owned(),
        run_mode,
        cancellation: cancellation.clone(),
        publisher: publisher.clone(),
    };
    let task_source = source.clone();
    let task_context = context.clone();
    publisher.mark_running(source_revision);
    let join = runtime.spawn(async move {
        let result = worker.run(task_source, task_context.clone()).await;
        let error = match result {
            Ok(()) if task_context.is_current() => {
                Some("Zone Catalog worker stopped unexpectedly".to_owned())
            }
            Ok(()) => None,
            Err(
                ZoneCatalogServiceError::Cancelled | ZoneCatalogServiceError::StaleRevision { .. },
            ) if !task_context.is_current() => None,
            Err(error) => Some(error.to_string()),
        };
        task_context.finish(error);
    });
    *active = Some(ActiveCatalogRun {
        source,
        cancellation,
        join,
    });
    Ok(source_revision)
}

async fn restart_catalog_run(
    runtime: &Handle,
    worker: Arc<dyn ZoneCatalogWorker>,
    publisher: &CatalogRunPublisher,
    shutdown: &CancellationToken,
    active: &mut Option<ActiveCatalogRun>,
    control: CatalogControl,
) -> ZoneCatalogServiceResult<u64> {
    let source = active
        .as_ref()
        .map(|run| run.source.clone())
        .ok_or_else(|| {
            ZoneCatalogServiceError::InvalidState(
                "catalog service has no configured source".to_owned(),
            )
        })?;
    let run_mode = match control {
        CatalogControl::Retry => ZoneCatalogRunMode::Resume,
        CatalogControl::Rebuild => ZoneCatalogRunMode::Rebuild,
    };
    configure_catalog_run(
        runtime,
        worker,
        publisher,
        shutdown,
        active,
        CatalogRunRequest {
            source,
            force_restart: true,
            run_mode,
        },
    )
    .await
}

struct CatalogRunRequest {
    source: ZoneCatalogSourceDescriptor,
    force_restart: bool,
    run_mode: ZoneCatalogRunMode,
}

fn next_source_revision(publisher: &CatalogRunPublisher) -> ZoneCatalogServiceResult<u64> {
    publisher
        .desired_revision
        .load(Ordering::Acquire)
        .checked_add(1)
        .ok_or_else(|| {
            ZoneCatalogServiceError::InvalidState("catalog source revision is exhausted".to_owned())
        })
}

fn invalidate_catalog_run(publisher: &CatalogRunPublisher) {
    let next = publisher
        .desired_revision
        .load(Ordering::Acquire)
        .saturating_add(1);
    publisher.clear(next);
}

async fn stop_catalog_run(active: &mut Option<ActiveCatalogRun>) -> ZoneCatalogServiceResult<()> {
    let Some(active) = active.take() else {
        return Ok(());
    };
    active.cancellation.cancel();
    active
        .join
        .await
        .map_err(|error| ZoneCatalogServiceError::Join(error.to_string()))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogCandidateActivation {
    Verified {
        promotion: Option<Box<CatalogIdentityPromotion>>,
    },
    SourceBehind {
        source_lib_slot: u64,
        required_slot: u64,
    },
    CachedUnverified {
        detail: String,
    },
    Mismatch {
        detail: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogIdentityPromotion {
    pub old_scope: NetworkScope,
    pub new_scope: NetworkScope,
    pub anchor: CatalogBlockCheckpoint,
    pub checkpoint: CatalogBlockCheckpoint,
    pub assurance: CatalogIdentityAssurance,
}

pub type CatalogIdentityRebindFuture<'a> =
    Pin<Box<dyn Future<Output = ZoneCatalogServiceResult<()>> + Send + 'a>>;

pub trait CatalogIdentityRebinder: Send + Sync {
    fn rebind<'a>(
        &'a self,
        old_scope: &'a NetworkScope,
        new_scope: &'a NetworkScope,
    ) -> CatalogIdentityRebindFuture<'a>;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ChannelSourceCatalogIdentityRebinder;

impl CatalogIdentityRebinder for ChannelSourceCatalogIdentityRebinder {
    fn rebind<'a>(
        &'a self,
        old_scope: &'a NetworkScope,
        new_scope: &'a NetworkScope,
    ) -> CatalogIdentityRebindFuture<'a> {
        let old_scope = old_scope.clone();
        let new_scope = new_scope.clone();
        Box::pin(async move {
            tokio::task::spawn_blocking(move || rebind_channel_source_configs(old_scope, new_scope))
                .await
                .map_err(map_join_error)?
                .map_err(|_error| {
                    ZoneCatalogServiceError::Worker(
                        "failed to rebind Channel source settings".to_owned(),
                    )
                })
        })
    }
}

pub async fn verify_catalog_candidate(
    source: &dyn CatalogL1Source,
    snapshot: &CatalogSnapshot,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogCandidateActivation> {
    context.ensure_current()?;
    let status = source.chain_status().await.map_err(map_source_error)?;
    context.ensure_current()?;

    let reported_genesis = status.genesis_id.clone();
    let pending_transition = snapshot
        .metadata
        .identity_transition
        .as_ref()
        .filter(|transition| transition.stage != CatalogIdentityTransitionStage::Committed);
    if let Some(transition) = pending_transition {
        match reported_genesis.as_ref() {
            Some(reported) if Some(reported) != genesis_id(&transition.new_scope) => {
                return Ok(CatalogCandidateActivation::Mismatch {
                    detail: "source genesis does not match pending identity transition".to_owned(),
                });
            }
            None => {
                return Ok(CatalogCandidateActivation::CachedUnverified {
                    detail: "source omitted genesis id required by pending promotion".to_owned(),
                });
            }
            Some(_) => {}
        }
    }

    let current_scope = &snapshot.metadata.network_scope;
    match (current_scope, reported_genesis.as_deref()) {
        (NetworkScope::GenesisId { genesis_id }, Some(reported)) if genesis_id != reported => {
            return Ok(CatalogCandidateActivation::Mismatch {
                detail: "source reported a conflicting genesis id".to_owned(),
            });
        }
        (NetworkScope::GenesisId { .. }, None) if finalized_anchor_alias(snapshot).is_none() => {
            return Ok(CatalogCandidateActivation::CachedUnverified {
                detail: "source omitted genesis id and catalog has no retained anchor".to_owned(),
            });
        }
        (NetworkScope::FinalizedAnchor { .. }, Some(_))
        | (NetworkScope::FinalizedAnchor { .. }, None)
        | (NetworkScope::GenesisId { .. }, Some(_))
        | (NetworkScope::GenesisId { .. }, None) => {}
    }

    let anchor_scope = match current_scope {
        NetworkScope::FinalizedAnchor { .. } => Some(current_scope),
        NetworkScope::GenesisId { .. } if reported_genesis.is_none() => {
            finalized_anchor_alias(snapshot)
        }
        NetworkScope::GenesisId { .. } => None,
    };
    let anchor = pending_transition
        .map(|transition| transition.anchor.clone())
        .or_else(|| anchor_scope.and_then(finalized_anchor_checkpoint));
    let checkpoint = pending_transition
        .map(|transition| transition.checkpoint.clone())
        .or_else(|| latest_catalog_checkpoint(snapshot))
        .or_else(|| anchor.clone());
    let mut checks = Vec::new();
    if let Some(anchor) = anchor.as_ref() {
        checks.push(anchor.clone());
    }
    if let Some(checkpoint) = checkpoint.as_ref()
        && checks.iter().all(|existing| existing != checkpoint)
    {
        checks.push(checkpoint.clone());
    }

    if let Some(required_slot) = checks.iter().map(|checkpoint| checkpoint.slot).max()
        && status.snapshot.lib.slot < required_slot
    {
        return Ok(CatalogCandidateActivation::SourceBehind {
            source_lib_slot: status.snapshot.lib.slot,
            required_slot,
        });
    }
    if let Some(anchor_scope) = anchor_scope {
        context.ensure_current()?;
        let time = source.time_status().await.map_err(map_source_error)?;
        context.ensure_current()?;
        let reported_time = time.genesis_time_unix_ms.to_string();
        if finalized_anchor_genesis_time(anchor_scope) != Some(reported_time.as_str()) {
            return Ok(CatalogCandidateActivation::Mismatch {
                detail: "source genesis time does not match catalog anchor".to_owned(),
            });
        }
    }
    for expected in &checks {
        if status.snapshot.lib.slot == expected.slot
            && status.snapshot.lib.block_id != expected.block_id
        {
            return Ok(CatalogCandidateActivation::Mismatch {
                detail: format!(
                    "source LIB conflicts with catalog checkpoint at slot {}",
                    expected.slot
                ),
            });
        }
        context.ensure_current()?;
        let actual = source
            .block(expected.block_id.clone())
            .await
            .map_err(map_source_error)?;
        context.ensure_current()?;
        let Some(actual) = actual else {
            return Ok(CatalogCandidateActivation::CachedUnverified {
                detail: format!(
                    "source cannot return catalog checkpoint at slot {}",
                    expected.slot
                ),
            });
        };
        if actual.checkpoint != *expected {
            return Ok(CatalogCandidateActivation::Mismatch {
                detail: format!(
                    "source block conflicts with catalog checkpoint at slot {}",
                    expected.slot
                ),
            });
        }
    }

    let promotion = match (current_scope, reported_genesis) {
        (old_scope @ NetworkScope::FinalizedAnchor { .. }, Some(genesis_id)) => {
            let anchor = anchor.ok_or_else(|| {
                ZoneCatalogServiceError::InvalidState(
                    "provisional catalog has no finalized anchor".to_owned(),
                )
            })?;
            let checkpoint = checkpoint.clone().unwrap_or_else(|| anchor.clone());
            let new_scope = NetworkScope::GenesisId { genesis_id };
            let assurance = promotion_assurance(snapshot, &new_scope, &checkpoint);
            let promotion = CatalogIdentityPromotion {
                old_scope: old_scope.clone(),
                new_scope,
                anchor,
                checkpoint,
                assurance,
            };
            if let Some(transition) = pending_transition
                && !transition_matches_promotion(transition, &promotion)
            {
                return Ok(CatalogCandidateActivation::Mismatch {
                    detail: "pending identity transition conflicts with current source".to_owned(),
                });
            }
            Some(Box::new(promotion))
        }
        (NetworkScope::FinalizedAnchor { .. }, None)
        | (NetworkScope::GenesisId { .. }, Some(_))
        | (NetworkScope::GenesisId { .. }, None) => None,
    };
    Ok(CatalogCandidateActivation::Verified { promotion })
}

pub async fn apply_catalog_identity_promotion(
    catalog: Arc<ZoneCatalog>,
    promotion: CatalogIdentityPromotion,
    rebinder: Arc<dyn CatalogIdentityRebinder>,
    context: &ZoneCatalogRunContext,
    updated_at_unix: u64,
) -> ZoneCatalogServiceResult<CatalogSnapshot> {
    let mut snapshot = catalog_snapshot(catalog.clone(), context).await?;
    loop {
        let stage = snapshot
            .metadata
            .identity_transition
            .as_ref()
            .map(|transition| transition.stage);
        match stage {
            None => {
                let expected_revision = snapshot.metadata.catalog_revision;
                let catalog = catalog.clone();
                let promotion = promotion.clone();
                let source_revision = context.source_revision;
                snapshot = context
                    .run_blocking_catalog(move || {
                        catalog.prepare_identity_promotion(
                            expected_revision,
                            updated_at_unix,
                            source_revision,
                            &promotion,
                        )
                    })
                    .await?;
            }
            Some(CatalogIdentityTransitionStage::Prepared) => {
                context.ensure_current()?;
                rebinder
                    .rebind(&promotion.old_scope, &promotion.new_scope)
                    .await?;
                context.ensure_current()?;
                let expected_revision = snapshot.metadata.catalog_revision;
                let catalog = catalog.clone();
                let promotion = promotion.clone();
                snapshot = context
                    .run_blocking_catalog(move || {
                        catalog.mark_identity_settings_rebound(
                            expected_revision,
                            updated_at_unix,
                            &promotion,
                        )
                    })
                    .await?;
            }
            Some(CatalogIdentityTransitionStage::SettingsRebound) => {
                context.ensure_current()?;
                let expected_revision = snapshot.metadata.catalog_revision;
                let catalog = catalog.clone();
                let promotion = promotion.clone();
                snapshot = context
                    .run_blocking_catalog(move || {
                        catalog.commit_identity_promotion(
                            expected_revision,
                            updated_at_unix,
                            &promotion,
                        )
                    })
                    .await?;
            }
            Some(CatalogIdentityTransitionStage::Committed) => {
                if snapshot.metadata.network_scope != promotion.new_scope
                    || !snapshot
                        .metadata
                        .identity_transition
                        .as_ref()
                        .is_some_and(|transition| {
                            transition_matches_promotion(transition, &promotion)
                        })
                {
                    return Err(ZoneCatalogServiceError::InvalidState(
                        "committed identity transition has wrong scope or boundaries".to_owned(),
                    ));
                }
                return Ok(snapshot);
            }
        }
    }
}

impl ZoneCatalog {
    fn prepare_identity_promotion(
        &self,
        expected_catalog_revision: u64,
        updated_at_unix: u64,
        source_revision: u64,
        promotion: &CatalogIdentityPromotion,
    ) -> Result<CatalogSnapshot, CatalogError> {
        let transition = CatalogIdentityTransition {
            old_scope: promotion.old_scope.clone(),
            new_scope: promotion.new_scope.clone(),
            anchor: promotion.anchor.clone(),
            checkpoint: promotion.checkpoint.clone(),
            source_revision,
            stage: CatalogIdentityTransitionStage::Prepared,
            prepared_at_unix: updated_at_unix,
        };
        self.store.update_metadata(
            expected_catalog_revision,
            updated_at_unix,
            move |metadata| {
                if let Some(existing) = metadata.identity_transition.as_ref() {
                    if transition_matches_promotion(existing, promotion) {
                        return Ok(());
                    }
                    return Err(CatalogError::invalid_input(
                        "catalog has another identity transition",
                    ));
                }
                if metadata.network_scope != promotion.old_scope {
                    return Err(CatalogError::invalid_input(
                        "catalog canonical scope changed before promotion",
                    ));
                }
                metadata.identity_transition = Some(transition);
                Ok(())
            },
        )
    }

    fn mark_identity_settings_rebound(
        &self,
        expected_catalog_revision: u64,
        updated_at_unix: u64,
        promotion: &CatalogIdentityPromotion,
    ) -> Result<CatalogSnapshot, CatalogError> {
        self.store
            .update_metadata(expected_catalog_revision, updated_at_unix, |metadata| {
                let transition = metadata.identity_transition.as_mut().ok_or_else(|| {
                    CatalogError::invalid_input("catalog identity transition is missing")
                })?;
                if !transition_matches_promotion(transition, promotion) {
                    return Err(CatalogError::invalid_input(
                        "catalog identity transition boundaries changed",
                    ));
                }
                match transition.stage {
                    CatalogIdentityTransitionStage::Prepared => {
                        transition.stage = CatalogIdentityTransitionStage::SettingsRebound;
                    }
                    CatalogIdentityTransitionStage::SettingsRebound
                    | CatalogIdentityTransitionStage::Committed => {}
                }
                Ok(())
            })
    }

    fn commit_identity_promotion(
        &self,
        expected_catalog_revision: u64,
        updated_at_unix: u64,
        promotion: &CatalogIdentityPromotion,
    ) -> Result<CatalogSnapshot, CatalogError> {
        self.store
            .update_metadata(expected_catalog_revision, updated_at_unix, |metadata| {
                let transition = metadata.identity_transition.as_mut().ok_or_else(|| {
                    CatalogError::invalid_input("catalog identity transition is missing")
                })?;
                if !transition_matches_promotion(transition, promotion) {
                    return Err(CatalogError::invalid_input(
                        "catalog identity transition boundaries changed",
                    ));
                }
                match transition.stage {
                    CatalogIdentityTransitionStage::Prepared => {
                        return Err(CatalogError::invalid_input(
                            "catalog source settings are not rebound",
                        ));
                    }
                    CatalogIdentityTransitionStage::SettingsRebound => {
                        transition.stage = CatalogIdentityTransitionStage::Committed;
                        metadata.network_scope = promotion.new_scope.clone();
                        metadata.identity_assurance = promotion.assurance;
                        if !metadata
                            .identity_aliases
                            .iter()
                            .any(|alias| alias.network_scope == promotion.old_scope)
                        {
                            metadata.identity_aliases.push(NetworkIdentityAlias {
                                network_scope: promotion.old_scope.clone(),
                                accepted_at_unix: updated_at_unix,
                            });
                        }
                    }
                    CatalogIdentityTransitionStage::Committed => {}
                }
                Ok(())
            })
    }
}

async fn catalog_snapshot(
    catalog: Arc<ZoneCatalog>,
    context: &ZoneCatalogRunContext,
) -> ZoneCatalogServiceResult<CatalogSnapshot> {
    context
        .run_blocking_catalog(move || catalog.snapshot())
        .await
}

fn latest_catalog_checkpoint(snapshot: &CatalogSnapshot) -> Option<CatalogBlockCheckpoint> {
    snapshot
        .frontier
        .as_ref()
        .and_then(|frontier| frontier.checkpoint.clone())
}

fn finalized_anchor_alias(snapshot: &CatalogSnapshot) -> Option<&NetworkScope> {
    snapshot
        .metadata
        .identity_aliases
        .iter()
        .rev()
        .map(|alias| &alias.network_scope)
        .find(|scope| matches!(scope, NetworkScope::FinalizedAnchor { .. }))
}

fn finalized_anchor_checkpoint(scope: &NetworkScope) -> Option<CatalogBlockCheckpoint> {
    match scope {
        NetworkScope::FinalizedAnchor {
            block_slot,
            block_id,
            parent_id,
            ..
        } => Some(CatalogBlockCheckpoint {
            slot: *block_slot,
            block_id: block_id.clone(),
            parent_id: parent_id.clone(),
        }),
        NetworkScope::GenesisId { .. } => None,
    }
}

fn finalized_anchor_genesis_time(scope: &NetworkScope) -> Option<&str> {
    match scope {
        NetworkScope::FinalizedAnchor { genesis_time, .. } => Some(genesis_time),
        NetworkScope::GenesisId { .. } => None,
    }
}

fn genesis_id(scope: &NetworkScope) -> Option<&String> {
    match scope {
        NetworkScope::GenesisId { genesis_id } => Some(genesis_id),
        NetworkScope::FinalizedAnchor { .. } => None,
    }
}

fn promotion_assurance(
    snapshot: &CatalogSnapshot,
    new_scope: &NetworkScope,
    checkpoint: &CatalogBlockCheckpoint,
) -> CatalogIdentityAssurance {
    let Some(genesis_id) = genesis_id(new_scope) else {
        return CatalogIdentityAssurance::SourceAttested;
    };
    let complete_prefix = snapshot.frontier.as_ref().is_some_and(|frontier| {
        frontier.coverage_floor == Some(0)
            && frontier.prefix_status == CoveragePrefixStatus::Complete
            && frontier.checkpoint.as_ref() == Some(checkpoint)
    });
    let checkpoint_reference = CatalogBlockReference {
        slot: checkpoint.slot,
        block_id: checkpoint.block_id.clone(),
    };
    let connected = snapshot.gaps.is_empty()
        && snapshot.segments.len() == 1
        && snapshot.segments.iter().any(|segment| {
            segment.floor.slot == 0
                && segment.floor.block_id == *genesis_id
                && segment.frontier == checkpoint_reference
        });
    if complete_prefix && connected {
        CatalogIdentityAssurance::AncestryVerified
    } else {
        CatalogIdentityAssurance::SourceAttested
    }
}

fn transition_matches_promotion(
    transition: &CatalogIdentityTransition,
    promotion: &CatalogIdentityPromotion,
) -> bool {
    transition.old_scope == promotion.old_scope
        && transition.new_scope == promotion.new_scope
        && transition.anchor == promotion.anchor
        && transition.checkpoint == promotion.checkpoint
}

fn map_source_error(error: CatalogL1SourceError) -> ZoneCatalogServiceError {
    let detail = match error {
        CatalogL1SourceError::InvalidRequest(_) => "catalog L1 request was invalid",
        CatalogL1SourceError::Unavailable(_) => "catalog L1 source is unavailable",
        CatalogL1SourceError::InvalidResponse(_) => "catalog L1 source returned invalid data",
    };
    ZoneCatalogServiceError::Source(detail.to_owned())
}

fn map_catalog_error(_error: CatalogError) -> ZoneCatalogServiceError {
    ZoneCatalogServiceError::Catalog("catalog storage or validation failed".to_owned())
}

fn map_join_error(error: tokio::task::JoinError) -> ZoneCatalogServiceError {
    ZoneCatalogServiceError::Join(error.to_string())
}

#[cfg(test)]
mod tests;
