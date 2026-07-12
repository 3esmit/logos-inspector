use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use tokio::runtime::Runtime;

use super::{
    decode_object_request,
    zone_evidence::{DirectEvidenceBlockReader, EvidenceBlockReader, ZoneEvidenceCommandInterface},
};
use crate::{
    inspection::{
        CatalogVerificationState, NetworkScope, ZoneSummary,
        catalog::{
            ChannelSourceApplyRequest, ChannelSourceAttestationRecovery,
            ChannelSourceAttestationWarning, ChannelSourceAttestationWarningCode,
            ChannelSourceConfigReport, ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            ZoneCatalogConfigureReport, ZoneCatalogConfigureRequest, ZoneCatalogControl,
            ZoneCatalogControlReport, ZoneCatalogControlRequest, ZoneCatalogCoverageReport,
            ZoneCatalogIngestionReport, ZoneCatalogService, ZoneCatalogSourceDescriptor,
            ZoneCatalogSourceRequest, ZoneCatalogStatusReport, ZoneCatalogStatusRequest,
            ZoneCatalogWorker, ZoneDetailReport, ZoneDetailRequest, ZoneEvidenceDetailRequest,
            ZoneEvidencePageRequest, ZoneEvidencePayloadChunkRequest,
            ZoneEvidencePayloadReleaseRequest, ZoneSummaryChanges, ZonesSummaryReport,
            ZonesSummaryRequest,
        },
        project_catalog_zone_detail, project_catalog_zones_with_sources,
        sources::project_zone_sources,
    },
    inspector::value::to_value,
    source_routing::channel_sources::{
        ChannelSourceConfig, ChannelSourceConfigMutation, ChannelSourceMonitor,
        ChannelSourceMonitorSnapshot, ChannelSourceRole, ChannelSourceTarget,
        SequencerAttestationReceipt, apply_channel_source_config_with_attestation,
        attest_sequencer_target, load_channel_source_configs, normalize_channel_id,
    },
    support::time::now_millis,
};

const DEFAULT_SUMMARY_PAGE_SIZE: usize = 200;
const MAX_SUMMARY_PAGE_SIZE: usize = 500;
const MAX_SUMMARY_JOURNAL_REVISIONS: usize = 64;
const MAX_SUMMARY_JOURNAL_CHANGES: usize = 4_096;
const MAX_SUMMARY_CURSORS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZoneCatalogCommand {
    Configure,
    Status,
    Summaries,
    Detail,
    Retry,
    Rebuild,
    ApplySourceConfig,
    EvidencePage,
    EvidenceDetail,
    EvidencePayloadChunk,
    EvidencePayloadRelease,
}

const COMMANDS: [(&str, ZoneCatalogCommand); 11] = [
    ("zoneCatalogConfigure", ZoneCatalogCommand::Configure),
    ("zoneCatalogStatus", ZoneCatalogCommand::Status),
    ("zonesSummary", ZoneCatalogCommand::Summaries),
    ("zoneDetail", ZoneCatalogCommand::Detail),
    ("zoneCatalogRetry", ZoneCatalogCommand::Retry),
    ("zoneCatalogRebuild", ZoneCatalogCommand::Rebuild),
    (
        "channelSourceConfigApply",
        ZoneCatalogCommand::ApplySourceConfig,
    ),
    ("zoneEvidencePage", ZoneCatalogCommand::EvidencePage),
    ("zoneEvidenceDetail", ZoneCatalogCommand::EvidenceDetail),
    (
        "zoneEvidencePayloadChunk",
        ZoneCatalogCommand::EvidencePayloadChunk,
    ),
    (
        "zoneEvidencePayloadRelease",
        ZoneCatalogCommand::EvidencePayloadRelease,
    ),
];

pub(crate) fn zone_catalog_command(method: &str) -> Option<ZoneCatalogCommand> {
    COMMANDS
        .iter()
        .find_map(|(name, command)| (*name == method).then_some(*command))
}

#[cfg(test)]
pub(crate) fn zone_catalog_command_names() -> impl Iterator<Item = &'static str> {
    COMMANDS.iter().map(|(name, _)| *name)
}

trait ChannelSourceConfigStore: Send + Sync {
    fn load(&self) -> Result<Vec<ChannelSourceConfig>>;
    fn apply(
        &self,
        request: ChannelSourceApplyRequest,
        attestation: Option<SequencerAttestationReceipt>,
    ) -> Result<ChannelSourceConfig>;
}

type SourceMonitorFuture<'a> = Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;
type SequencerAttestorFuture<'a> = Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

trait SequencerTargetAttestor: Send + Sync {
    fn attest<'a>(&'a self, target: ChannelSourceTarget) -> SequencerAttestorFuture<'a>;
}

#[derive(Debug, Clone, Copy, Default)]
struct DirectSequencerTargetAttestor;

impl SequencerTargetAttestor for DirectSequencerTargetAttestor {
    fn attest<'a>(&'a self, target: ChannelSourceTarget) -> SequencerAttestorFuture<'a> {
        Box::pin(async move { attest_sequencer_target(target).await })
    }
}

trait ZoneSourceMonitor: Send + Sync {
    fn snapshot(&self) -> ChannelSourceMonitorSnapshot;

    fn configure<'a>(
        &'a self,
        network_scope: NetworkScope,
        catalog_verified: bool,
        configs: Vec<ChannelSourceConfig>,
    ) -> SourceMonitorFuture<'a>;
}

impl ZoneSourceMonitor for ChannelSourceMonitor {
    fn snapshot(&self) -> ChannelSourceMonitorSnapshot {
        self.snapshot()
    }

    fn configure<'a>(
        &'a self,
        network_scope: NetworkScope,
        catalog_verified: bool,
        configs: Vec<ChannelSourceConfig>,
    ) -> SourceMonitorFuture<'a> {
        Box::pin(async move {
            self.configure(network_scope, catalog_verified, configs)
                .await
                .map_err(anyhow::Error::from)
        })
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct SettingsChannelSourceConfigStore;

impl ChannelSourceConfigStore for SettingsChannelSourceConfigStore {
    fn load(&self) -> Result<Vec<ChannelSourceConfig>> {
        load_channel_source_configs()
    }

    fn apply(
        &self,
        request: ChannelSourceApplyRequest,
        attestation: Option<SequencerAttestationReceipt>,
    ) -> Result<ChannelSourceConfig> {
        apply_channel_source_config_with_attestation(request, attestation)
    }
}

pub(crate) struct ZoneCatalogCommandInterface {
    service: ZoneCatalogService,
    monitor: Arc<dyn ZoneSourceMonitor>,
    sequencer_attestor: Arc<dyn SequencerTargetAttestor>,
    source_store: Arc<dyn ChannelSourceConfigStore>,
    evidence: ZoneEvidenceCommandInterface,
    state: Mutex<ProjectionState>,
}

impl ZoneCatalogCommandInterface {
    #[must_use]
    pub(crate) fn with_worker(runtime: &Runtime, worker: Arc<dyn ZoneCatalogWorker>) -> Self {
        Self::with_dependencies(
            runtime,
            worker,
            Arc::new(SettingsChannelSourceConfigStore),
            Arc::new(ChannelSourceMonitor::new(runtime.handle())),
            Arc::new(DirectSequencerTargetAttestor),
        )
    }

    fn with_dependencies(
        runtime: &Runtime,
        worker: Arc<dyn ZoneCatalogWorker>,
        source_store: Arc<dyn ChannelSourceConfigStore>,
        monitor: Arc<dyn ZoneSourceMonitor>,
        sequencer_attestor: Arc<dyn SequencerTargetAttestor>,
    ) -> Self {
        Self::with_all_dependencies(
            runtime,
            worker,
            source_store,
            monitor,
            sequencer_attestor,
            Arc::new(DirectEvidenceBlockReader),
        )
    }

    fn with_all_dependencies(
        runtime: &Runtime,
        worker: Arc<dyn ZoneCatalogWorker>,
        source_store: Arc<dyn ChannelSourceConfigStore>,
        monitor: Arc<dyn ZoneSourceMonitor>,
        sequencer_attestor: Arc<dyn SequencerTargetAttestor>,
        evidence_reader: Arc<dyn EvidenceBlockReader>,
    ) -> Self {
        Self {
            service: ZoneCatalogService::new(runtime.handle(), worker),
            monitor,
            sequencer_attestor,
            source_store,
            evidence: ZoneEvidenceCommandInterface::new(evidence_reader),
            state: Mutex::new(ProjectionState::default()),
        }
    }

    pub(crate) fn bridge_call(
        &self,
        runtime: &Runtime,
        command: ZoneCatalogCommand,
        args: &Value,
    ) -> Result<Value> {
        match command {
            ZoneCatalogCommand::Configure => {
                let request = decode_object_request(args, "zoneCatalogConfigure")?;
                to_value(self.configure(runtime, request)?)
            }
            ZoneCatalogCommand::Status => {
                let request = decode_object_request(args, "zoneCatalogStatus")?;
                to_value(self.status(runtime, request)?)
            }
            ZoneCatalogCommand::Summaries => {
                let request = decode_object_request(args, "zonesSummary")?;
                to_value(self.summaries(runtime, request)?)
            }
            ZoneCatalogCommand::Detail => {
                let request = decode_object_request(args, "zoneDetail")?;
                to_value(self.detail(runtime, request)?)
            }
            ZoneCatalogCommand::Retry => {
                let request = decode_object_request(args, "zoneCatalogRetry")?;
                to_value(self.control(runtime, request, ZoneCatalogControl::Retry)?)
            }
            ZoneCatalogCommand::Rebuild => {
                let request = decode_object_request(args, "zoneCatalogRebuild")?;
                to_value(self.control(runtime, request, ZoneCatalogControl::Rebuild)?)
            }
            ZoneCatalogCommand::ApplySourceConfig => {
                let request = decode_object_request(args, "channelSourceConfigApply")?;
                to_value(self.apply_source_config(runtime, request)?)
            }
            ZoneCatalogCommand::EvidencePage => {
                let request = decode_object_request(args, "zoneEvidencePage")?;
                to_value(self.evidence_page(runtime, request)?)
            }
            ZoneCatalogCommand::EvidenceDetail => {
                let request = decode_object_request(args, "zoneEvidenceDetail")?;
                to_value(self.evidence_detail(runtime, request)?)
            }
            ZoneCatalogCommand::EvidencePayloadChunk => {
                let request = decode_object_request(args, "zoneEvidencePayloadChunk")?;
                to_value(self.evidence_payload_chunk(runtime, request)?)
            }
            ZoneCatalogCommand::EvidencePayloadRelease => {
                let request = decode_object_request(args, "zoneEvidencePayloadRelease")?;
                to_value(self.evidence_payload_release(runtime, request)?)
            }
        }
    }

    fn configure(
        &self,
        runtime: &Runtime,
        request: ZoneCatalogConfigureRequest,
    ) -> Result<ZoneCatalogConfigureReport> {
        let source = match request.source {
            ZoneCatalogSourceRequest::DirectHttp { endpoint } => {
                ZoneCatalogSourceDescriptor::direct_http(endpoint)?
            }
        };
        let source_revision = runtime.block_on(self.service.configure(source.clone()))?;
        self.evidence.configure_source(source_revision, source)?;
        self.refresh(runtime)?;
        Ok(ZoneCatalogConfigureReport {
            report_kind: "zones.catalog_configured",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision,
        })
    }

    fn status(
        &self,
        runtime: &Runtime,
        _request: ZoneCatalogStatusRequest,
    ) -> Result<ZoneCatalogStatusReport> {
        self.refresh(runtime)?;
        let service = self.service.report();
        let state = self.lock_state()?;
        let snapshot = service.catalog.as_deref();
        let network_scope = snapshot.map(|catalog| catalog.metadata.network_scope.clone());
        let catalog_revision = snapshot.map_or(0, |catalog| catalog.metadata.catalog_revision);
        let coverage = snapshot.map(project_coverage_report).unwrap_or_default();
        let ingestion = ZoneCatalogIngestionReport {
            worker_running: service.worker_running,
            target_lib_slot: snapshot
                .and_then(|catalog| catalog.traversal.as_ref())
                .and_then(|traversal| traversal.target_lib.as_ref())
                .map(|target| target.slot),
            ingestion_cursor_slot: snapshot
                .and_then(|catalog| catalog.traversal.as_ref())
                .and_then(|traversal| traversal.ingestion_cursor.as_ref())
                .map(|cursor| cursor.slot),
            discovered_zone_count: snapshot.map_or(0, |catalog| usize_to_u64(catalog.zones.len())),
        };
        Ok(ZoneCatalogStatusReport {
            report_kind: "zones.catalog_status",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: service.source_revision,
            network_scope,
            catalog_revision,
            source_config_epoch: state.source_config_epoch,
            observation_revision: state.observations.observation_revision,
            summary_revision: state.summary_revision,
            verification: service.verification_state,
            coverage,
            ingestion,
            current_error: service.current_error,
        })
    }

    fn summaries(
        &self,
        runtime: &Runtime,
        request: ZonesSummaryRequest,
    ) -> Result<ZonesSummaryReport> {
        self.refresh(runtime)?;
        let limit = summary_page_limit(request.limit)?;
        let mut state = self.lock_state()?;
        let snapshot = if let Some(cursor) = request.cursor.as_deref() {
            let cursor = state
                .cursors
                .remove(cursor)
                .context("Zone summary cursor is invalid or expired")?;
            validate_summary_request_fences(
                request.source_revision,
                request.network_scope.as_ref(),
                &cursor.snapshot,
            )?;
            CursorPage {
                snapshot: cursor.snapshot,
                offset: cursor.offset,
            }
        } else {
            validate_current_summary_fences(&request, &state)?;
            CursorPage {
                snapshot: state.summary_snapshot(request.after_summary_revision),
                offset: 0,
            }
        };
        let (changes, next_offset) = snapshot.snapshot.page(snapshot.offset, limit);
        let next_cursor = next_offset
            .map(|offset| state.insert_cursor(snapshot.snapshot.clone(), offset))
            .transpose()?;
        Ok(snapshot.snapshot.report(changes, next_cursor))
    }

    fn detail(&self, runtime: &Runtime, request: ZoneDetailRequest) -> Result<ZoneDetailReport> {
        self.refresh(runtime)?;
        let service = self.service.report();
        require_verified(&service.verification_state)?;
        let catalog = service
            .catalog
            .clone()
            .context("verified Zone Catalog is unavailable")?;
        let state = self.lock_state()?;
        validate_detail_fences(&request, &service, &catalog, &state)?;
        let detail_revision = state
            .detail_revisions
            .get(&request.channel_id)
            .copied()
            .context("Zone does not exist in current catalog projection")?;
        let detail = project_catalog_zone_detail(
            &catalog,
            &state.configs,
            &state.observations,
            service.verification_state,
            &request.channel_id,
            detail_revision,
        )
        .context("Zone does not exist in current catalog projection")?;
        Ok(ZoneDetailReport {
            report_kind: "zones.zone_detail",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: service.source_revision,
            network_scope: catalog.metadata.network_scope.clone(),
            catalog_revision: catalog.metadata.catalog_revision,
            source_config_epoch: state.source_config_epoch,
            observation_revision: state.observations.observation_revision,
            summary_revision: state.summary_revision,
            detail,
        })
    }

    fn control(
        &self,
        runtime: &Runtime,
        request: ZoneCatalogControlRequest,
        control: ZoneCatalogControl,
    ) -> Result<ZoneCatalogControlReport> {
        let current = self.service.report().source_revision;
        if request.source_revision != current {
            bail!(
                "Zone Catalog source revision conflict: expected {}, current {current}",
                request.source_revision
            );
        }
        let source_revision = match control {
            ZoneCatalogControl::Retry => runtime.block_on(self.service.retry())?,
            ZoneCatalogControl::Rebuild => runtime.block_on(self.service.rebuild())?,
        };
        self.evidence.rebind_source_revision(source_revision)?;
        self.refresh(runtime)?;
        Ok(ZoneCatalogControlReport {
            report_kind: "zones.catalog_control",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            control,
            source_revision,
        })
    }

    fn apply_source_config(
        &self,
        runtime: &Runtime,
        request: ChannelSourceApplyRequest,
    ) -> Result<ChannelSourceConfigReport> {
        self.refresh(runtime)?;
        let service = self.service.report();
        require_verified(&service.verification_state)?;
        let catalog = service
            .catalog
            .context("verified Zone Catalog is unavailable")?;
        if request.network_scope != catalog.metadata.network_scope {
            bail!("Channel source configuration belongs to a stale network scope");
        }
        let configs = {
            let state = self.lock_state()?;
            if !state.summaries.contains_key(&request.channel_id) {
                bail!("Channel is not present in current Zone Catalog projection");
            }
            state.configs.clone()
        };
        validate_source_mutation_revision(&request, &configs)?;
        let (attestation, attestation_warning) = if let Some(plan) =
            sequencer_attestation_plan(&request, &configs)?
        {
            match runtime.block_on(self.sequencer_attestor.attest(plan.target.clone())) {
                    Ok(reported_channel_id) => {
                        let reported_channel_id = normalize_channel_id(&reported_channel_id)?;
                        if reported_channel_id != request.channel_id {
                            bail!("Sequencer attestation reported another Channel");
                        }
                        (
                            Some(SequencerAttestationReceipt {
                                reported_channel_id,
                                target_fingerprint: plan.target.fingerprint(),
                                attested_at_unix: now_millis() / 1_000,
                            }),
                            None,
                        )
                    }
                    Err(_error) if plan.allow_pending => (
                        None,
                        Some(ChannelSourceAttestationWarning {
                            code: ChannelSourceAttestationWarningCode::PendingAttestation,
                            recovery: ChannelSourceAttestationRecovery::Retry,
                            message: "Sequencer Channel attestation is pending; retry when the source is available."
                                .to_owned(),
                        }),
                    ),
                    Err(error) => {
                        return Err(error.context("Sequencer Channel attestation retry failed"));
                    }
                }
        } else {
            (None, None)
        };
        let config = self.source_store.apply(request, attestation)?;
        self.refresh(runtime)?;
        let service = self.service.report();
        let state = self.lock_state()?;
        let summary = state
            .summaries
            .get(&config.channel_id)
            .context("updated Channel disappeared from Zone projection")?;
        let projection = project_zone_sources(
            summary.kind(),
            &config.channel_id,
            Some(&config),
            &state.observations,
        );
        Ok(ChannelSourceConfigReport {
            report_kind: "zones.channel_source_config",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: service.source_revision,
            catalog_revision: service
                .catalog
                .as_deref()
                .map_or(0, |catalog| catalog.metadata.catalog_revision),
            source_config_epoch: state.source_config_epoch,
            observation_revision: state.observations.observation_revision,
            summary_revision: state.summary_revision,
            config,
            observations: projection.observations,
            agreement: projection.agreement,
            attestation_warning,
        })
    }

    fn evidence_page(
        &self,
        runtime: &Runtime,
        request: ZoneEvidencePageRequest,
    ) -> Result<crate::inspection::catalog::ZoneEvidencePageReport> {
        self.refresh(runtime)?;
        self.evidence.page(&self.service.report(), request)
    }

    fn evidence_detail(
        &self,
        runtime: &Runtime,
        request: ZoneEvidenceDetailRequest,
    ) -> Result<crate::inspection::catalog::ZoneEvidenceDetailReport> {
        self.refresh(runtime)?;
        self.evidence.detail(runtime, &self.service, request)
    }

    fn evidence_payload_chunk(
        &self,
        runtime: &Runtime,
        request: ZoneEvidencePayloadChunkRequest,
    ) -> Result<crate::inspection::catalog::ZoneEvidencePayloadChunkReport> {
        self.refresh(runtime)?;
        self.evidence.payload_chunk(&self.service.report(), request)
    }

    fn evidence_payload_release(
        &self,
        runtime: &Runtime,
        request: ZoneEvidencePayloadReleaseRequest,
    ) -> Result<crate::inspection::catalog::ZoneEvidencePayloadReleaseReport> {
        self.refresh(runtime)?;
        self.evidence
            .release_payload(&self.service.report(), request)
    }

    pub(crate) fn context_snapshot(&self, runtime: &Runtime) -> Result<ZoneContextSnapshot> {
        self.refresh(runtime)?;
        let service = self.service.report();
        let state = self.lock_state()?;
        Ok(ZoneContextSnapshot {
            network_scope: service
                .catalog
                .as_deref()
                .map(|catalog| catalog.metadata.network_scope.clone()),
            verification: service.verification_state,
            summaries: state.summaries.clone(),
            configs: state.configs.clone(),
            observations: state.observations.clone(),
        })
    }

    fn refresh(&self, runtime: &Runtime) -> Result<()> {
        let service = self.service.report();
        self.evidence.reconcile(&service)?;
        let configs = self.source_store.load()?;
        synchronize_monitor(runtime, self.monitor.as_ref(), &service, &configs)?;
        let observations = self.monitor.snapshot();
        let mut state = self.lock_state()?;
        state.refresh(&service, configs, observations)
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, ProjectionState>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Zone Catalog projection lock is poisoned"))
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ZoneContextSnapshot {
    pub network_scope: Option<NetworkScope>,
    pub verification: CatalogVerificationState,
    pub summaries: BTreeMap<String, ZoneSummary>,
    pub configs: Vec<ChannelSourceConfig>,
    pub observations: ChannelSourceMonitorSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProjectionKey {
    source_revision: u64,
    network_scope: Option<NetworkScope>,
    catalog_revision: u64,
    source_config_epoch: u64,
    observation_revision: u64,
    verification: CatalogVerificationState,
}

#[derive(Default)]
struct ProjectionState {
    projection_key: Option<ProjectionKey>,
    source_config_epoch: u64,
    configs: Vec<ChannelSourceConfig>,
    observations: ChannelSourceMonitorSnapshot,
    summary_revision: u64,
    delta_floor_revision: u64,
    summaries: BTreeMap<String, ZoneSummary>,
    detail_revisions: BTreeMap<String, u64>,
    journal: VecDeque<SummaryJournalEntry>,
    journal_change_count: usize,
    cursors: BTreeMap<String, SummaryCursor>,
    cursor_order: VecDeque<String>,
}

impl ProjectionState {
    fn refresh(
        &mut self,
        service: &crate::inspection::catalog::ZoneCatalogServiceReport,
        configs: Vec<ChannelSourceConfig>,
        observations: ChannelSourceMonitorSnapshot,
    ) -> Result<()> {
        if self.configs != configs {
            self.source_config_epoch = self
                .source_config_epoch
                .checked_add(1)
                .context("Channel source configuration epoch overflow")?;
        }
        let network_scope = service
            .catalog
            .as_deref()
            .map(|catalog| catalog.metadata.network_scope.clone());
        let catalog_revision = service
            .catalog
            .as_deref()
            .map_or(0, |catalog| catalog.metadata.catalog_revision);
        let key = ProjectionKey {
            source_revision: service.source_revision,
            network_scope,
            catalog_revision,
            source_config_epoch: self.source_config_epoch,
            observation_revision: observations.observation_revision,
            verification: service.verification_state,
        };
        let rows = service.catalog.as_deref().map_or_else(Vec::new, |catalog| {
            project_catalog_zones_with_sources(
                catalog,
                &configs,
                &observations,
                service.verification_state,
            )
        });
        self.configs = configs;
        self.observations = observations;
        if self.projection_key.as_ref() == Some(&key) {
            return Ok(());
        }
        let initial_empty = self.projection_key.is_none()
            && key.source_revision == 0
            && key.network_scope.is_none()
            && key.catalog_revision == 0
            && key.source_config_epoch == 0
            && key.observation_revision == 0
            && rows.is_empty();
        let identity_changed = self.projection_key.as_ref().map_or(
            key.source_revision != 0 || key.network_scope.is_some(),
            |previous| {
                previous.source_revision != key.source_revision
                    || previous.network_scope != key.network_scope
            },
        );
        self.projection_key = Some(key.clone());
        if initial_empty {
            return Ok(());
        }

        let next_revision = self
            .summary_revision
            .checked_add(1)
            .context("Zone summary revision overflow")?;
        if identity_changed {
            self.delta_floor_revision = next_revision;
        }
        let next_rows = rows
            .into_iter()
            .map(|row| (row.channel_id.clone(), row))
            .collect::<BTreeMap<_, _>>();
        let upserts = next_rows
            .iter()
            .filter(|(channel_id, row)| self.summaries.get(*channel_id) != Some(*row))
            .map(|(_, row)| row.clone())
            .collect::<Vec<_>>();
        let removed_zone_ids = self
            .summaries
            .keys()
            .filter(|channel_id| !next_rows.contains_key(*channel_id))
            .cloned()
            .collect::<Vec<_>>();
        for channel_id in next_rows.keys() {
            let revision = self.detail_revisions.entry(channel_id.clone()).or_insert(0);
            *revision = revision
                .checked_add(1)
                .context("Zone detail revision overflow")?;
        }
        for channel_id in &removed_zone_ids {
            self.detail_revisions.remove(channel_id);
        }
        let change_count = upserts.len().saturating_add(removed_zone_ids.len());
        self.journal.push_back(SummaryJournalEntry {
            revision: next_revision,
            source_revision: key.source_revision,
            network_scope: key.network_scope,
            upserts,
            removed_zone_ids,
        });
        self.journal_change_count = self.journal_change_count.saturating_add(change_count);
        self.trim_journal();
        self.summary_revision = next_revision;
        self.summaries = next_rows;
        Ok(())
    }

    fn trim_journal(&mut self) {
        while self.journal.len() > MAX_SUMMARY_JOURNAL_REVISIONS
            || self.journal_change_count > MAX_SUMMARY_JOURNAL_CHANGES
        {
            let Some(removed) = self.journal.pop_front() else {
                break;
            };
            self.journal_change_count = self
                .journal_change_count
                .saturating_sub(removed.change_count());
        }
    }

    fn summary_snapshot(&self, after_revision: Option<u64>) -> SummarySnapshot {
        let key = self.projection_key.clone().unwrap_or(ProjectionKey {
            source_revision: 0,
            network_scope: None,
            catalog_revision: 0,
            source_config_epoch: 0,
            observation_revision: 0,
            verification: CatalogVerificationState::Empty,
        });
        let changes = after_revision
            .and_then(|revision| self.delta_since(revision, &key))
            .unwrap_or_else(|| SummarySnapshotChanges::Reset {
                rows: self.summaries.values().cloned().collect(),
            });
        SummarySnapshot {
            source_revision: key.source_revision,
            network_scope: key.network_scope,
            catalog_revision: key.catalog_revision,
            source_config_epoch: key.source_config_epoch,
            observation_revision: key.observation_revision,
            summary_revision: self.summary_revision,
            changes,
        }
    }

    fn delta_since(
        &self,
        after_revision: u64,
        key: &ProjectionKey,
    ) -> Option<SummarySnapshotChanges> {
        if after_revision > self.summary_revision {
            return None;
        }
        if after_revision < self.delta_floor_revision {
            return None;
        }
        if after_revision == self.summary_revision {
            return Some(SummarySnapshotChanges::Delta {
                upserts: Vec::new(),
                removed_zone_ids: Vec::new(),
            });
        }
        let first_required = after_revision.checked_add(1)?;
        let first = self.journal.iter().find(|entry| {
            entry.revision == first_required
                && entry.source_revision == key.source_revision
                && entry.network_scope == key.network_scope
        })?;
        let mut expected = first.revision;
        let mut upserts = BTreeMap::new();
        let mut removed = BTreeSet::new();
        for entry in self.journal.iter().filter(|entry| {
            entry.revision >= first_required && entry.revision <= self.summary_revision
        }) {
            if entry.revision != expected
                || entry.source_revision != key.source_revision
                || entry.network_scope != key.network_scope
            {
                return None;
            }
            for channel_id in &entry.removed_zone_ids {
                upserts.remove(channel_id);
                removed.insert(channel_id.clone());
            }
            for row in &entry.upserts {
                removed.remove(&row.channel_id);
                upserts.insert(row.channel_id.clone(), row.clone());
            }
            expected = expected.checked_add(1)?;
        }
        if expected != self.summary_revision.checked_add(1)? {
            return None;
        }
        Some(SummarySnapshotChanges::Delta {
            upserts: upserts.into_values().collect(),
            removed_zone_ids: removed.into_iter().collect(),
        })
    }

    fn insert_cursor(&mut self, snapshot: SummarySnapshot, offset: usize) -> Result<String> {
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).context("failed to create Zone summary cursor")?;
        let token = format!("zsc1_{}", hex::encode(random));
        self.cursors
            .insert(token.clone(), SummaryCursor { snapshot, offset });
        self.cursor_order.push_back(token.clone());
        while self.cursor_order.len() > MAX_SUMMARY_CURSORS {
            if let Some(expired) = self.cursor_order.pop_front() {
                self.cursors.remove(&expired);
            }
        }
        Ok(token)
    }
}

#[derive(Debug, Clone)]
struct SummaryJournalEntry {
    revision: u64,
    source_revision: u64,
    network_scope: Option<NetworkScope>,
    upserts: Vec<ZoneSummary>,
    removed_zone_ids: Vec<String>,
}

impl SummaryJournalEntry {
    fn change_count(&self) -> usize {
        self.upserts
            .len()
            .saturating_add(self.removed_zone_ids.len())
    }
}

#[derive(Debug, Clone)]
struct SummarySnapshot {
    source_revision: u64,
    network_scope: Option<NetworkScope>,
    catalog_revision: u64,
    source_config_epoch: u64,
    observation_revision: u64,
    summary_revision: u64,
    changes: SummarySnapshotChanges,
}

impl SummarySnapshot {
    fn page(&self, offset: usize, limit: usize) -> (ZoneSummaryChanges, Option<usize>) {
        match &self.changes {
            SummarySnapshotChanges::Reset { rows } => {
                let end = offset.saturating_add(limit).min(rows.len());
                let page = rows.get(offset..end).unwrap_or_default().to_vec();
                let next = (end < rows.len()).then_some(end);
                (ZoneSummaryChanges::Reset { rows: page }, next)
            }
            SummarySnapshotChanges::Delta {
                upserts,
                removed_zone_ids,
            } => {
                let total = upserts.len().saturating_add(removed_zone_ids.len());
                let end = offset.saturating_add(limit).min(total);
                let upsert_start = offset.min(upserts.len());
                let upsert_end = end.min(upserts.len());
                let removed_start = offset.saturating_sub(upserts.len());
                let removed_end = end.saturating_sub(upserts.len());
                let page_upserts = upserts
                    .get(upsert_start..upsert_end)
                    .unwrap_or_default()
                    .to_vec();
                let page_removed = removed_zone_ids
                    .get(removed_start..removed_end)
                    .unwrap_or_default()
                    .to_vec();
                let next = (end < total).then_some(end);
                (
                    ZoneSummaryChanges::Delta {
                        upserts: page_upserts,
                        removed_zone_ids: page_removed,
                    },
                    next,
                )
            }
        }
    }

    fn report(
        &self,
        changes: ZoneSummaryChanges,
        next_cursor: Option<String>,
    ) -> ZonesSummaryReport {
        ZonesSummaryReport {
            report_kind: "zones.summary",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: self.source_revision,
            network_scope: self.network_scope.clone(),
            catalog_revision: self.catalog_revision,
            source_config_epoch: self.source_config_epoch,
            observation_revision: self.observation_revision,
            summary_revision: self.summary_revision,
            changes,
            next_cursor,
        }
    }
}

#[derive(Debug, Clone)]
enum SummarySnapshotChanges {
    Reset {
        rows: Vec<ZoneSummary>,
    },
    Delta {
        upserts: Vec<ZoneSummary>,
        removed_zone_ids: Vec<String>,
    },
}

#[derive(Debug, Clone)]
struct SummaryCursor {
    snapshot: SummarySnapshot,
    offset: usize,
}

struct CursorPage {
    snapshot: SummarySnapshot,
    offset: usize,
}

fn validate_source_mutation_revision(
    request: &ChannelSourceApplyRequest,
    configs: &[ChannelSourceConfig],
) -> Result<()> {
    let current = configs
        .iter()
        .find(|config| {
            config.network_scope == request.network_scope && config.channel_id == request.channel_id
        })
        .map_or(0, |config| config.config_revision);
    if current != request.expected_config_revision {
        bail!(
            "Channel source configuration revision conflict: expected {}, current {current}",
            request.expected_config_revision
        );
    }
    Ok(())
}

struct SequencerAttestationPlan {
    target: ChannelSourceTarget,
    allow_pending: bool,
}

fn sequencer_attestation_plan(
    request: &ChannelSourceApplyRequest,
    configs: &[ChannelSourceConfig],
) -> Result<Option<SequencerAttestationPlan>> {
    match &request.mutation {
        ChannelSourceConfigMutation::AddSequencer {
            target,
            allow_insecure_http,
            ..
        } => Ok(Some(SequencerAttestationPlan {
            target: target
                .clone()
                .normalized(ChannelSourceRole::Sequencer, *allow_insecure_http)?,
            allow_pending: true,
        })),
        ChannelSourceConfigMutation::UpdateSequencer {
            source_id,
            target,
            allow_insecure_http,
            ..
        } => {
            let target = target
                .clone()
                .normalized(ChannelSourceRole::Sequencer, *allow_insecure_http)?;
            let config = configs
                .iter()
                .find(|config| {
                    config.network_scope == request.network_scope
                        && config.channel_id == request.channel_id
                })
                .context("Channel source configuration does not exist")?;
            let source = config
                .sequencer_sources
                .iter()
                .find(|source| source.source_id == *source_id)
                .with_context(|| format!("Sequencer source `{source_id}` does not exist"))?;
            Ok(
                (source.target != target).then_some(SequencerAttestationPlan {
                    target,
                    allow_pending: true,
                }),
            )
        }
        ChannelSourceConfigMutation::RetryAttestation { source_id } => {
            let config = configs
                .iter()
                .find(|config| {
                    config.network_scope == request.network_scope
                        && config.channel_id == request.channel_id
                })
                .context("Channel source configuration does not exist")?;
            let source = config
                .sequencer_sources
                .iter()
                .find(|source| source.source_id == *source_id)
                .with_context(|| format!("Sequencer source `{source_id}` does not exist"))?;
            Ok(Some(SequencerAttestationPlan {
                target: source.target.clone(),
                allow_pending: false,
            }))
        }
        ChannelSourceConfigMutation::RemoveSequencer { .. }
        | ChannelSourceConfigMutation::SelectSequencer { .. }
        | ChannelSourceConfigMutation::SetIndexer { .. }
        | ChannelSourceConfigMutation::RemoveIndexer => Ok(None),
    }
}

fn synchronize_monitor(
    runtime: &Runtime,
    monitor: &dyn ZoneSourceMonitor,
    service: &crate::inspection::catalog::ZoneCatalogServiceReport,
    configs: &[ChannelSourceConfig],
) -> Result<()> {
    let current = monitor.snapshot();
    let network_scope = service
        .catalog
        .as_deref()
        .map(|catalog| catalog.metadata.network_scope.clone())
        .or(current.network_scope);
    let Some(network_scope) = network_scope else {
        return Ok(());
    };
    let verified = service.verification_state == CatalogVerificationState::Verified
        && service.catalog.is_some();
    let matching = configs
        .iter()
        .filter(|config| config.network_scope == network_scope)
        .cloned()
        .collect();
    runtime.block_on(monitor.configure(network_scope, verified, matching))?;
    Ok(())
}

fn project_coverage_report(
    snapshot: &crate::inspection::catalog::CatalogSnapshot,
) -> ZoneCatalogCoverageReport {
    let Some(frontier) = snapshot.frontier.as_ref() else {
        return ZoneCatalogCoverageReport {
            gap_count: usize_to_u64(snapshot.gaps.len()),
            ..ZoneCatalogCoverageReport::default()
        };
    };
    ZoneCatalogCoverageReport {
        status: frontier.coverage_status,
        coverage_floor: frontier.coverage_floor,
        scanned_through_slot: frontier.scanned_through_slot,
        observed_lib_slot: frontier.observed_lib.as_ref().map(|block| block.slot),
        prefix_status: frontier.prefix_status,
        continuity_checkpoint: frontier.checkpoint.as_ref().map(|checkpoint| {
            crate::inspection::FinalizedBlockCheckpoint {
                slot: checkpoint.slot,
                block_id: checkpoint.block_id.clone(),
                parent_id: checkpoint.parent_id.clone(),
            }
        }),
        gap_count: usize_to_u64(snapshot.gaps.len()),
    }
}

fn summary_page_limit(limit: Option<u16>) -> Result<usize> {
    let limit = limit.map_or(DEFAULT_SUMMARY_PAGE_SIZE, usize::from);
    if limit == 0 || limit > MAX_SUMMARY_PAGE_SIZE {
        bail!("Zone summary page limit must be between 1 and {MAX_SUMMARY_PAGE_SIZE}");
    }
    Ok(limit)
}

fn validate_current_summary_fences(
    request: &ZonesSummaryRequest,
    state: &ProjectionState,
) -> Result<()> {
    let key = state
        .projection_key
        .as_ref()
        .context("Zone summary projection is unavailable")?;
    if request.source_revision != key.source_revision {
        bail!("Zone summary source revision is stale");
    }
    if request.network_scope.as_ref() != key.network_scope.as_ref() {
        bail!("Zone summary network scope is stale");
    }
    if request
        .after_summary_revision
        .is_some_and(|revision| revision > state.summary_revision)
    {
        bail!("Zone summary revision is newer than current state");
    }
    Ok(())
}

fn validate_summary_request_fences(
    source_revision: u64,
    network_scope: Option<&NetworkScope>,
    snapshot: &SummarySnapshot,
) -> Result<()> {
    if source_revision != snapshot.source_revision
        || network_scope != snapshot.network_scope.as_ref()
    {
        bail!("Zone summary cursor belongs to another source or network");
    }
    Ok(())
}

fn validate_detail_fences(
    request: &ZoneDetailRequest,
    service: &crate::inspection::catalog::ZoneCatalogServiceReport,
    catalog: &crate::inspection::catalog::CatalogSnapshot,
    state: &ProjectionState,
) -> Result<()> {
    if request.source_revision != service.source_revision
        || request.network_scope != catalog.metadata.network_scope
        || request.catalog_revision != catalog.metadata.catalog_revision
        || request.summary_revision != state.summary_revision
        || request.observation_revision != state.observations.observation_revision
    {
        bail!("Zone detail request fences are stale");
    }
    Ok(())
}

fn require_verified(verification: &CatalogVerificationState) -> Result<()> {
    if *verification != CatalogVerificationState::Verified {
        bail!("Zone Catalog is not verified");
    }
    Ok(())
}

fn usize_to_u64(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::json;
    use tokio::sync::mpsc;

    use super::*;
    use crate::{
        inspection::{
            CatalogCoverageStatus, CoveragePrefixStatus, L1ChannelSummary, L1FinalityState,
            catalog::{
                CatalogBlockCheckpoint, CatalogBlockReference, CatalogEvidenceUse, CatalogFrontier,
                CatalogMetadata, CatalogSnapshot, CatalogSnapshotOrigin, CoverageSegment,
                ZoneCatalogPublication, ZoneCatalogRecord, ZoneCatalogRunContext,
                ZoneCatalogRunMode, ZoneCatalogSourceDescriptor, ZoneCatalogWorkerFuture,
                ZoneClassificationCounters, ZoneEvidenceKind, ZoneEvidenceReference,
            },
        },
        inspector::commands::zone_evidence::EvidenceBlockFuture,
        source_routing::channel_sources::{
            ChannelSourceConfigMutation, ChannelSourceTarget, ConfiguredSequencerSource,
            PersistedSequencerAttestation,
        },
    };

    #[test]
    fn catalog_reports_are_fenced_paged_and_cursor_immutable() -> Result<()> {
        let runtime = Runtime::new()?;
        let scope = scope('1');
        let channel_a = identity('a');
        let channel_b = identity('b');
        let store = Arc::new(FakeSourceStore::new(vec![
            config(&scope, &channel_a, 'c', 1, 1),
            config(&scope, &channel_b, 'd', 1, 1),
        ]));
        let monitor = Arc::new(FakeMonitor::default());
        let (worker, mut started) = PublishingWorker::new(snapshot(scope.clone()));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store.clone(),
            monitor,
            Arc::new(UnusedAttestor),
        );

        let configured = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{ "source": { "kind": "direct_http", "endpoint": "https://l1.example" } }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;
        if configured.get("source_revision").and_then(Value::as_u64) != Some(1) {
            bail!("unexpected configure report: {configured}");
        }

        let status = interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;
        if status.get("report_kind").and_then(Value::as_str) != Some("zones.catalog_status")
            || status.get("verification").and_then(Value::as_str) != Some("verified")
            || status.get("catalog_revision").and_then(Value::as_u64) != Some(7)
            || status.get("rows").is_some()
            || status.get("zones").is_some()
        {
            bail!("status is not compact or correctly fenced: {status}");
        }
        let old_summary_revision = required_u64(&status, "summary_revision")?;
        let source_revision = required_u64(&status, "source_revision")?;
        let network_scope = status
            .get("network_scope")
            .cloned()
            .context("status has no network scope")?;

        let first = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Summaries,
            &json!([{
                "source_revision": source_revision,
                "network_scope": network_scope,
                "after_summary_revision": null,
                "cursor": null,
                "limit": 1
            }]),
        )?;
        let old_cursor = first
            .get("next_cursor")
            .and_then(Value::as_str)
            .context("first summary page has no cursor")?
            .to_owned();
        if first.pointer("/changes/kind").and_then(Value::as_str) != Some("reset")
            || first
                .pointer("/changes/rows")
                .and_then(Value::as_array)
                .map(Vec::len)
                != Some(1)
        {
            bail!("unexpected first summary page: {first}");
        }

        store.replace(vec![
            config(&scope, &channel_a, 'c', 2, 2),
            config(&scope, &channel_b, 'd', 1, 1),
        ])?;
        let current_status =
            interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;
        let current_summary_revision = required_u64(&current_status, "summary_revision")?;
        if current_summary_revision <= old_summary_revision {
            bail!("summary revision did not advance after source config change");
        }

        let old_second = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Summaries,
            &json!([{
                "source_revision": source_revision,
                "network_scope": current_status.get("network_scope"),
                "after_summary_revision": null,
                "cursor": old_cursor,
                "limit": 1
            }]),
        )?;
        if required_u64(&old_second, "summary_revision")? != old_summary_revision
            || old_second
                .pointer("/changes/rows")
                .and_then(Value::as_array)
                .map(Vec::len)
                != Some(1)
        {
            bail!("new publication mutated old summary cursor: {old_second}");
        }

        let delta = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Summaries,
            &json!([{
                "source_revision": source_revision,
                "network_scope": current_status.get("network_scope"),
                "after_summary_revision": old_summary_revision,
                "cursor": null,
                "limit": 200
            }]),
        )?;
        if delta.pointer("/changes/kind").and_then(Value::as_str) != Some("delta")
            || delta
                .pointer("/changes/upserts")
                .and_then(Value::as_array)
                .is_none_or(Vec::is_empty)
        {
            bail!("source change did not produce summary delta: {delta}");
        }

        let detail = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Detail,
            &json!([{
                "source_revision": source_revision,
                "network_scope": current_status.get("network_scope"),
                "catalog_revision": current_status.get("catalog_revision"),
                "summary_revision": current_status.get("summary_revision"),
                "observation_revision": current_status.get("observation_revision"),
                "channel_id": channel_a
            }]),
        )?;
        if detail
            .pointer("/detail/summary/channel_id")
            .and_then(Value::as_str)
            != Some(channel_a.as_str())
            || detail
                .pointer("/detail/channel_source_config/sequencer_sources/0/target/endpoint")
                .and_then(Value::as_str)
                != Some("https://sequencer.example/")
            || detail
                .pointer("/detail/channel_source_config/sequencer_sources/0/binding_state")
                .and_then(Value::as_str)
                != Some("persisted_attested")
            || detail
                .pointer("/detail/detail_revision")
                .and_then(Value::as_u64)
                != Some(2)
        {
            bail!("unexpected Zone detail report: {detail}");
        }

        let conflict = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::ApplySourceConfig,
            &json!([{
                "network_scope": scope,
                "channel_id": channel_a,
                "expected_config_revision": 1,
                "mutation": { "kind": "select_sequencer", "source_id": null }
            }]),
        );
        let Err(error) = conflict else {
            bail!("stale source mutation unexpectedly succeeded");
        };
        if !error.to_string().contains("revision conflict") {
            bail!("unexpected source conflict error: {error:#}");
        }

        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Retry,
            &json!([{ "source_revision": source_revision }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("retried catalog worker did not publish")?;
        let retried_status =
            interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;
        let reset_after_source_change = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Summaries,
            &json!([{
                "source_revision": retried_status.get("source_revision"),
                "network_scope": retried_status.get("network_scope"),
                "after_summary_revision": current_summary_revision,
                "cursor": null,
                "limit": 200
            }]),
        )?;
        if reset_after_source_change
            .pointer("/changes/kind")
            .and_then(Value::as_str)
            != Some("reset")
        {
            bail!("summary delta crossed a source revision: {reset_after_source_change}");
        }
        Ok(())
    }

    #[test]
    fn controls_restart_worker_with_explicit_run_mode() -> Result<()> {
        let runtime = Runtime::new()?;
        let (worker, mut started) = PublishingWorker::new(snapshot(scope('2')));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            Arc::new(FakeSourceStore::default()),
            Arc::new(FakeMonitor::default()),
            Arc::new(UnusedAttestor),
        );
        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{ "source": { "kind": "direct_http", "endpoint": "https://l1.example" } }]),
        )?;
        if runtime.block_on(started.recv()) != Some(ZoneCatalogRunMode::Resume) {
            bail!("configure did not start resume mode");
        }
        let retry = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Retry,
            &json!([{ "source_revision": 1 }]),
        )?;
        if runtime.block_on(started.recv()) != Some(ZoneCatalogRunMode::Resume)
            || retry.get("source_revision").and_then(Value::as_u64) != Some(2)
        {
            bail!("retry did not restart resume mode: {retry}");
        }
        let rebuild = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Rebuild,
            &json!([{ "source_revision": 2 }]),
        )?;
        if runtime.block_on(started.recv()) != Some(ZoneCatalogRunMode::Rebuild)
            || rebuild.get("source_revision").and_then(Value::as_u64) != Some(3)
        {
            bail!("rebuild did not restart rebuild mode: {rebuild}");
        }
        Ok(())
    }

    #[test]
    fn sequencer_channel_mismatch_blocks_source_mutation_before_write() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('3');
        let channel_id = identity('a');
        let store = Arc::new(FakeSourceStore::new(vec![config(
            &network_scope,
            &channel_id,
            'c',
            1,
            1,
        )]));
        let (worker, mut started) = PublishingWorker::new(snapshot(network_scope.clone()));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store.clone(),
            Arc::new(FakeMonitor::default()),
            Arc::new(FixedAttestor {
                reported_channel_id: identity('b'),
            }),
        );
        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{ "source": { "kind": "direct_http", "endpoint": "https://l1.example" } }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;

        let result = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::ApplySourceConfig,
            &json!([{
                "network_scope": network_scope,
                "channel_id": channel_id,
                "expected_config_revision": 1,
                "mutation": {
                    "kind": "add_sequencer",
                    "label": "wrong channel",
                    "target": { "kind": "rpc", "endpoint": "https://new-sequencer.example" }
                }
            }]),
        );
        let Err(error) = result else {
            bail!("mismatched Sequencer source mutation succeeded");
        };
        if !error
            .to_string()
            .contains("Sequencer attestation reported another Channel")
        {
            bail!("unexpected mismatch error: {error:#}");
        }
        let configs = store.load()?;
        let Some(config) = configs.first() else {
            bail!("source config disappeared after rejected mutation");
        };
        if config.config_revision != 1 || config.sequencer_sources.len() != 1 {
            bail!("rejected source mutation changed persisted config: {config:?}");
        }
        Ok(())
    }

    #[test]
    fn successful_source_mutation_returns_complete_attested_report() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('4');
        let channel_id = identity('a');
        let store = Arc::new(FakeSourceStore::new(vec![config(
            &network_scope,
            &channel_id,
            'c',
            1,
            1,
        )]));
        let (worker, mut started) = PublishingWorker::new(snapshot(network_scope.clone()));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store,
            Arc::new(FakeMonitor::default()),
            Arc::new(FixedAttestor {
                reported_channel_id: channel_id.clone(),
            }),
        );
        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{ "source": { "kind": "direct_http", "endpoint": "https://l1.example" } }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;

        let report = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::ApplySourceConfig,
            &json!([{
                "network_scope": network_scope,
                "channel_id": channel_id,
                "expected_config_revision": 1,
                "mutation": {
                    "kind": "add_sequencer",
                    "label": "Backup",
                    "target": { "kind": "rpc", "endpoint": "https://backup.example" }
                }
            }]),
        )?;
        if report.get("report_kind").and_then(Value::as_str) != Some("zones.channel_source_config")
            || report
                .pointer("/config/config_revision")
                .and_then(Value::as_u64)
                != Some(2)
            || report
                .pointer("/config/sequencer_sources")
                .and_then(Value::as_array)
                .map(Vec::len)
                != Some(2)
            || report
                .pointer("/config/sequencer_sources/1/channel_attestation/state")
                .and_then(Value::as_str)
                != Some("persisted_attested")
            || !report.get("observations").is_some_and(Value::is_array)
            || !report.get("agreement").is_some_and(Value::is_object)
        {
            bail!("source mutation report is incomplete: {report}");
        }
        Ok(())
    }

    #[test]
    fn unreachable_attestation_saves_pending_with_sanitized_warning() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('5');
        let channel_id = identity('a');
        let store = Arc::new(FakeSourceStore::new(vec![config(
            &network_scope,
            &channel_id,
            'c',
            1,
            1,
        )]));
        let (worker, mut started) = PublishingWorker::new(snapshot(network_scope.clone()));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store,
            Arc::new(FakeMonitor::default()),
            Arc::new(UnusedAttestor),
        );
        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{ "source": { "kind": "direct_http", "endpoint": "https://l1.example" } }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;

        let report = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::ApplySourceConfig,
            &json!([{
                "network_scope": network_scope,
                "channel_id": channel_id,
                "expected_config_revision": 1,
                "mutation": {
                    "kind": "add_sequencer",
                    "label": null,
                    "target": { "kind": "rpc", "endpoint": "https://offline.example" }
                }
            }]),
        )?;
        let warning = report
            .get("attestation_warning")
            .context("pending report has no attestation warning")?;
        let serialized_warning = warning.to_string();
        if report
            .pointer("/config/sequencer_sources/1/channel_attestation/state")
            .and_then(Value::as_str)
            != Some("pending")
            || report
                .pointer("/attestation_warning/code")
                .and_then(Value::as_str)
                != Some("pending_attestation")
            || report
                .pointer("/attestation_warning/recovery")
                .and_then(Value::as_str)
                != Some("retry")
            || serialized_warning.contains("offline.example")
            || serialized_warning.contains("endpoint")
        {
            bail!("pending attestation report is malformed or unsafe: {report}");
        }
        Ok(())
    }

    #[test]
    fn evidence_commands_page_and_refetch_exact_l1_payload() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('6');
        let channel_id = identity('a');
        let block = evidence_block(&channel_id, b"plain evidence");
        let snapshot = evidence_snapshot(network_scope.clone(), &channel_id, &block);
        let (worker, mut started) = PublishingWorker::new(snapshot);
        let interface = ZoneCatalogCommandInterface::with_all_dependencies(
            &runtime,
            Arc::new(worker),
            Arc::new(FakeSourceStore::default()),
            Arc::new(FakeMonitor::default()),
            Arc::new(UnusedAttestor),
            Arc::new(FakeEvidenceReader {
                block: Mutex::new(Some(block)),
            }),
        );
        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{ "source": { "kind": "direct_http", "endpoint": "https://l1.example" } }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;
        let status = interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;

        let page = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::EvidencePage,
            &json!([{
                "source_revision": status.get("source_revision"),
                "network_scope": status.get("network_scope"),
                "catalog_revision": status.get("catalog_revision"),
                "channel_id": channel_id,
                "filter": "raw_inscription",
                "cursor": null,
                "limit": 25
            }]),
        )?;
        let reference = page
            .pointer("/rows/0/reference")
            .cloned()
            .context("evidence page has no reference")?;
        if page.get("report_kind").and_then(Value::as_str) != Some("zones.evidence_page")
            || page
                .pointer("/rows/0/segment/segment_id")
                .and_then(Value::as_str)
                != Some("segment-main")
            || page.to_string().contains("plain evidence")
        {
            bail!("evidence page is incomplete or embeds payload data: {page}");
        }

        let detail = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::EvidenceDetail,
            &json!([{
                "source_revision": status.get("source_revision"),
                "network_scope": status.get("network_scope"),
                "catalog_revision": status.get("catalog_revision"),
                "channel_id": identity('a'),
                "reference": reference
            }]),
        )?;
        if detail.get("report_kind").and_then(Value::as_str) != Some("zones.evidence_detail")
            || detail.pointer("/operation/opcode").and_then(Value::as_u64) != Some(0x11)
            || detail
                .pointer("/payload/inline_text")
                .and_then(Value::as_str)
                != Some("plain evidence")
            || detail.pointer("/payload/encoding").and_then(Value::as_str) != Some("utf8")
        {
            bail!("unexpected exact evidence detail: {detail}");
        }
        Ok(())
    }

    struct PublishingWorker {
        snapshot: Arc<CatalogSnapshot>,
        started: mpsc::UnboundedSender<ZoneCatalogRunMode>,
    }

    struct FakeEvidenceReader {
        block: Mutex<Option<crate::inspection::catalog::CatalogL1Block>>,
    }

    impl EvidenceBlockReader for FakeEvidenceReader {
        fn block<'a>(
            &'a self,
            _source: ZoneCatalogSourceDescriptor,
            _block_id: String,
        ) -> EvidenceBlockFuture<'a> {
            Box::pin(async move {
                Ok(self
                    .block
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake evidence block lock poisoned"))?
                    .clone())
            })
        }
    }

    impl PublishingWorker {
        fn new(snapshot: CatalogSnapshot) -> (Self, mpsc::UnboundedReceiver<ZoneCatalogRunMode>) {
            let (started, receiver) = mpsc::unbounded_channel();
            (
                Self {
                    snapshot: Arc::new(snapshot),
                    started,
                },
                receiver,
            )
        }
    }

    impl ZoneCatalogWorker for PublishingWorker {
        fn run(
            self: Arc<Self>,
            _source: ZoneCatalogSourceDescriptor,
            context: ZoneCatalogRunContext,
        ) -> ZoneCatalogWorkerFuture {
            Box::pin(async move {
                let _published = context.publish(ZoneCatalogPublication {
                    verification_state: CatalogVerificationState::Verified,
                    catalog: Some(self.snapshot.clone()),
                    current_error: None,
                });
                self.started.send(context.run_mode()).map_err(|_| {
                    crate::inspection::catalog::ZoneCatalogServiceError::Worker(
                        "test start receiver closed".to_owned(),
                    )
                })?;
                context.cancellation().cancelled().await;
                Ok(())
            })
        }
    }

    #[derive(Default)]
    struct FakeSourceStore {
        configs: Mutex<Vec<ChannelSourceConfig>>,
    }

    impl FakeSourceStore {
        fn new(configs: Vec<ChannelSourceConfig>) -> Self {
            Self {
                configs: Mutex::new(configs),
            }
        }

        fn replace(&self, configs: Vec<ChannelSourceConfig>) -> Result<()> {
            *self
                .configs
                .lock()
                .map_err(|_| anyhow::anyhow!("fake source store lock poisoned"))? = configs;
            Ok(())
        }
    }

    impl ChannelSourceConfigStore for FakeSourceStore {
        fn load(&self) -> Result<Vec<ChannelSourceConfig>> {
            Ok(self
                .configs
                .lock()
                .map_err(|_| anyhow::anyhow!("fake source store lock poisoned"))?
                .clone())
        }

        fn apply(
            &self,
            request: ChannelSourceApplyRequest,
            attestation: Option<SequencerAttestationReceipt>,
        ) -> Result<ChannelSourceConfig> {
            let mut configs = self
                .configs
                .lock()
                .map_err(|_| anyhow::anyhow!("fake source store lock poisoned"))?;
            let config = configs
                .iter_mut()
                .find(|config| {
                    config.network_scope == request.network_scope
                        && config.channel_id == request.channel_id
                })
                .context("fake Channel source configuration does not exist")?;
            if config.config_revision != request.expected_config_revision {
                bail!(
                    "Channel source configuration revision conflict: expected {}, current {}",
                    request.expected_config_revision,
                    config.config_revision
                );
            }
            match request.mutation {
                ChannelSourceConfigMutation::SelectSequencer { source_id } => {
                    if attestation.is_some() {
                        bail!("selection received an unexpected attestation");
                    }
                    config.selected_sequencer_source_id = source_id;
                }
                ChannelSourceConfigMutation::AddSequencer { label, target, .. } => {
                    let target = target.normalized(ChannelSourceRole::Sequencer, false)?;
                    let channel_attestation = if let Some(receipt) = attestation {
                        if receipt.target_fingerprint != target.fingerprint()
                            || receipt.reported_channel_id != config.channel_id
                        {
                            bail!("fake attestation does not match source mutation");
                        }
                        PersistedSequencerAttestation::PersistedAttested {
                            channel_id: receipt.reported_channel_id,
                            target_fingerprint: receipt.target_fingerprint,
                            attested_at_unix: receipt.attested_at_unix,
                        }
                    } else {
                        PersistedSequencerAttestation::Pending
                    };
                    config.sequencer_sources.push(ConfiguredSequencerSource {
                        source_id: source_id('e'),
                        label,
                        target,
                        channel_attestation,
                    });
                }
                _ => bail!("fake source store only supports selection"),
            }
            config.config_revision = config.config_revision.saturating_add(1);
            Ok(config.clone())
        }
    }

    struct UnusedAttestor;

    impl SequencerTargetAttestor for UnusedAttestor {
        fn attest<'a>(&'a self, _target: ChannelSourceTarget) -> SequencerAttestorFuture<'a> {
            Box::pin(async { bail!("unexpected Sequencer attestation") })
        }
    }

    struct FixedAttestor {
        reported_channel_id: String,
    }

    impl SequencerTargetAttestor for FixedAttestor {
        fn attest<'a>(&'a self, _target: ChannelSourceTarget) -> SequencerAttestorFuture<'a> {
            let channel_id = self.reported_channel_id.clone();
            Box::pin(async move { Ok(channel_id) })
        }
    }

    #[derive(Default)]
    struct FakeMonitor {
        state: Mutex<FakeMonitorState>,
    }

    #[derive(Default)]
    struct FakeMonitorState {
        snapshot: ChannelSourceMonitorSnapshot,
        configs: Vec<ChannelSourceConfig>,
    }

    impl ZoneSourceMonitor for FakeMonitor {
        fn snapshot(&self) -> ChannelSourceMonitorSnapshot {
            self.state.lock().map_or_else(
                |_| ChannelSourceMonitorSnapshot::default(),
                |state| state.snapshot.clone(),
            )
        }

        fn configure<'a>(
            &'a self,
            network_scope: NetworkScope,
            catalog_verified: bool,
            configs: Vec<ChannelSourceConfig>,
        ) -> SourceMonitorFuture<'a> {
            Box::pin(async move {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake monitor lock poisoned"))?;
                let changed = state.snapshot.network_scope.as_ref() != Some(&network_scope)
                    || state.snapshot.catalog_verified != catalog_verified
                    || state.configs != configs;
                if changed {
                    state.snapshot.observation_revision = state
                        .snapshot
                        .observation_revision
                        .checked_add(1)
                        .context("fake observation revision overflow")?;
                    state.snapshot.network_scope = Some(network_scope);
                    state.snapshot.catalog_verified = catalog_verified;
                    state.snapshot.channels.clear();
                    state.configs = configs;
                }
                Ok(state.snapshot.observation_revision)
            })
        }
    }

    fn snapshot(scope: NetworkScope) -> CatalogSnapshot {
        let metadata = CatalogMetadata {
            catalog_file_id: "catalog_test".to_owned(),
            network_scope: scope,
            identity_aliases: Vec::new(),
            identity_assurance:
                crate::inspection::catalog::CatalogIdentityAssurance::SourceAttested,
            identity_transition: None,
            catalog_revision: 7,
            created_at_unix: 1,
            updated_at_unix: 1,
        };
        CatalogSnapshot {
            metadata,
            frontier: Some(CatalogFrontier {
                scanned_through_slot: None,
                checkpoint: None,
                observed_lib: None,
                coverage_floor: None,
                prefix_status: CoveragePrefixStatus::Unknown,
                coverage_status: CatalogCoverageStatus::Rebuilding,
            }),
            traversal: None,
            zones: Vec::new(),
            evidence: Vec::new(),
            segments: Vec::new(),
            gaps: Vec::new(),
        }
    }

    fn evidence_snapshot(
        scope: NetworkScope,
        channel_id: &str,
        block: &crate::inspection::catalog::CatalogL1Block,
    ) -> CatalogSnapshot {
        let mut snapshot = snapshot(scope);
        snapshot.frontier = Some(CatalogFrontier {
            scanned_through_slot: Some(block.checkpoint.slot),
            checkpoint: Some(block.checkpoint.clone()),
            observed_lib: Some(CatalogBlockReference {
                slot: block.checkpoint.slot,
                block_id: block.checkpoint.block_id.clone(),
            }),
            coverage_floor: Some(0),
            prefix_status: CoveragePrefixStatus::Complete,
            coverage_status: CatalogCoverageStatus::Complete,
        });
        snapshot.segments = vec![CoverageSegment {
            segment_id: "segment-main".to_owned(),
            floor: CatalogBlockCheckpoint {
                slot: 0,
                block_id: identity('0'),
                parent_id: identity('f'),
            },
            frontier: CatalogBlockReference {
                slot: block.checkpoint.slot,
                block_id: block.checkpoint.block_id.clone(),
            },
            reaches_target_lib: true,
        }];
        let evidence = ZoneEvidenceReference {
            evidence_id: format!("evidence-{}-0-raw_inscription", identity('e')),
            channel_id: channel_id.to_owned(),
            coverage_segment_id: "segment-main".to_owned(),
            l1_slot: block.checkpoint.slot,
            block_id: block.checkpoint.block_id.clone(),
            transaction_hash: Some(identity('e')),
            operation_index: 0,
            message_id: None,
            evidence_kind: ZoneEvidenceKind::RawInscription,
            evidence_use: CatalogEvidenceUse::Presence,
        };
        snapshot.evidence = vec![evidence.clone()];
        snapshot.zones = vec![ZoneCatalogRecord {
            channel_id: channel_id.to_owned(),
            observed_label: Some("Evidence Zone".to_owned()),
            l1_channel: L1ChannelSummary {
                tip_slot: Some(block.checkpoint.slot),
                tip_hash: Some(block.checkpoint.block_id.clone()),
                lib_slot: Some(block.checkpoint.slot),
                balance: None,
                key_count: Some(1),
                withdraw_threshold: Some("1".to_owned()),
                operation_count: 1,
                finality_state: L1FinalityState::Final,
            },
            sequencer_committee: None,
            classification: ZoneClassificationCounters {
                channel_operations: 1,
                recognized_l2_blocks: 0,
                raw_inscriptions: 1,
                conflicting_evidence: false,
            },
            first_seen_slot: block.checkpoint.slot,
            last_seen_slot: block.checkpoint.slot,
            latest_evidence_id: evidence.evidence_id,
            evidence_count: 1,
            snapshot_provenance: crate::inspection::catalog::CatalogSnapshotProvenance {
                origin: CatalogSnapshotOrigin::ReplayDerived,
                coverage_segment_id: "segment-main".to_owned(),
                observed_slot: block.checkpoint.slot,
                source_revision: 1,
            },
            updated_at_unix: 1,
        }];
        snapshot
    }

    fn evidence_block(
        channel_id: &str,
        payload: &[u8],
    ) -> crate::inspection::catalog::CatalogL1Block {
        crate::inspection::catalog::CatalogL1Block {
            checkpoint: CatalogBlockCheckpoint {
                slot: 7,
                block_id: identity('7'),
                parent_id: identity('6'),
            },
            payload: json!({
                "header": {
                    "slot": 7,
                    "id": identity('7'),
                    "parent_block": identity('6')
                },
                "transactions": [{
                    "mantle_tx": {
                        "hash": identity('e'),
                        "ops": [{
                            "opcode": 17,
                            "payload": {
                                "channel_id": channel_id,
                                "parent": identity('0'),
                                "signer": identity('1'),
                                "inscription": hex::encode(payload)
                            }
                        }]
                    },
                    "ops_proofs": []
                }]
            }),
        }
    }

    fn config(
        scope: &NetworkScope,
        channel_id: &str,
        source_character: char,
        revision: u64,
        source_count: usize,
    ) -> ChannelSourceConfig {
        let sources = (0..source_count)
            .map(|index| {
                let source_id = source_id(
                    char::from_u32(
                        u32::from(source_character)
                            .saturating_add(u32::try_from(index).unwrap_or(0)),
                    )
                    .unwrap_or(source_character),
                );
                let target = ChannelSourceTarget::Rpc {
                    endpoint: "https://sequencer.example/".to_owned(),
                };
                ConfiguredSequencerSource {
                    source_id,
                    label: None,
                    channel_attestation: PersistedSequencerAttestation::PersistedAttested {
                        channel_id: channel_id.to_owned(),
                        target_fingerprint: target.fingerprint(),
                        attested_at_unix: 1,
                    },
                    target,
                }
            })
            .collect::<Vec<_>>();
        ChannelSourceConfig {
            network_scope: scope.clone(),
            channel_id: channel_id.to_owned(),
            config_revision: revision,
            selected_sequencer_source_id: sources.first().map(|source| source.source_id.clone()),
            sequencer_sources: sources,
            indexer_source: None,
        }
    }

    fn scope(character: char) -> NetworkScope {
        NetworkScope::GenesisId {
            genesis_id: identity(character),
        }
    }

    fn identity(character: char) -> String {
        character.to_string().repeat(64)
    }

    fn source_id(character: char) -> String {
        format!("src_{}", character.to_string().repeat(32))
    }

    fn required_u64(value: &Value, field: &str) -> Result<u64> {
        value
            .get(field)
            .and_then(Value::as_u64)
            .with_context(|| format!("report field `{field}` is missing"))
    }
}
