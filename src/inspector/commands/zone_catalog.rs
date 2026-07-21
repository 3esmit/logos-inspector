use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, MutexGuard},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::runtime::Runtime;

mod projection;

use projection::ZoneProjectionLedger;

#[cfg(test)]
use super::zone_evidence::DirectEvidenceBlockReader;
use super::{
    decode_object_request,
    zone_evidence::{EvidenceBlockReader, RoutedEvidenceBlockReader, ZoneEvidenceCommandInterface},
};
use crate::{
    inspection::{
        CatalogVerificationState, NetworkScope,
        catalog::{
            ChannelSourceApplyRequest, ChannelSourceAttestationRecovery,
            ChannelSourceAttestationWarning, ChannelSourceAttestationWarningCode,
            ChannelSourceConfigCurrentReport, ChannelSourceConfigCurrentRequest,
            ChannelSourceConfigReport, ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            ZoneCatalogConfigureReport, ZoneCatalogConfigureRequest, ZoneCatalogControl,
            ZoneCatalogControlReport, ZoneCatalogControlRequest, ZoneCatalogCoverageReport,
            ZoneCatalogDefaultTopology, ZoneCatalogIngestionReport, ZoneCatalogService,
            ZoneCatalogSourceDescriptor, ZoneCatalogSourceRequest, ZoneCatalogStatusReport,
            ZoneCatalogStatusRequest, ZoneCatalogWorker, ZoneDetailReport, ZoneDetailRequest,
            ZoneEvidenceDetailRequest, ZoneEvidencePageRequest, ZoneEvidencePayloadChunkRequest,
            ZoneEvidencePayloadReleaseRequest, ZonesSummaryReport, ZonesSummaryRequest,
        },
    },
    inspector::value::to_value,
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
    source_routing::channel_sources::{
        ChannelSourceAttestationOutcome, ChannelSourceConfig, ChannelSourceConfigMutation,
        ChannelSourceConfigMutationInterface, ChannelSourceMonitor, ChannelSourceMonitorSnapshot,
        SequencerLegacyAnchorState, SettingsChannelSourceConfigMutation, indexer,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ZoneCatalogCommand {
    Configure,
    Status,
    Summaries,
    Detail,
    Retry,
    Rebuild,
    CurrentSourceConfig,
    ApplySourceConfig,
    RefreshIndexerSource,
    EvidencePage,
    EvidenceDetail,
    EvidencePayloadChunk,
    EvidencePayloadRelease,
}

const COMMANDS: [(&str, ZoneCatalogCommand); 13] = [
    ("zoneCatalogConfigure", ZoneCatalogCommand::Configure),
    ("zoneCatalogStatus", ZoneCatalogCommand::Status),
    ("zonesSummary", ZoneCatalogCommand::Summaries),
    ("zoneDetail", ZoneCatalogCommand::Detail),
    ("zoneCatalogRetry", ZoneCatalogCommand::Retry),
    ("zoneCatalogRebuild", ZoneCatalogCommand::Rebuild),
    (
        "channelSourceConfigCurrent",
        ZoneCatalogCommand::CurrentSourceConfig,
    ),
    (
        "channelSourceConfigApply",
        ZoneCatalogCommand::ApplySourceConfig,
    ),
    (
        "channelIndexerSourceRefresh",
        ZoneCatalogCommand::RefreshIndexerSource,
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChannelIndexerSourceRefreshRequest {
    source_revision: u64,
    network_scope: NetworkScope,
    channel_id: String,
    source_config_revision: u64,
    source_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ChannelIndexerSourceRefreshReport {
    report_kind: &'static str,
    schema_version: u32,
    source_revision: u64,
    network_scope: NetworkScope,
    channel_id: String,
    source_config_revision: u64,
    source_id: String,
    observation_revision: u64,
}

type SourceMonitorFuture<'a> = Pin<Box<dyn Future<Output = Result<u64>> + Send + 'a>>;
type SourceMonitorShutdownFuture<'a> = Pin<Box<dyn Future<Output = Result<()>> + Send + 'a>>;

trait ZoneSourceMonitor: Send + Sync {
    fn snapshot(&self) -> ChannelSourceMonitorSnapshot;

    fn configure<'a>(
        &'a self,
        network_scope: NetworkScope,
        catalog_verified: bool,
        configs: Vec<ChannelSourceConfig>,
    ) -> SourceMonitorFuture<'a>;

    fn refresh_source<'a>(
        &'a self,
        network_scope: NetworkScope,
        channel_id: String,
        source_config_revision: u64,
        source_id: String,
    ) -> SourceMonitorFuture<'a>;

    fn shutdown(&self) -> SourceMonitorShutdownFuture<'_>;
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

    fn refresh_source<'a>(
        &'a self,
        network_scope: NetworkScope,
        channel_id: String,
        source_config_revision: u64,
        source_id: String,
    ) -> SourceMonitorFuture<'a> {
        Box::pin(async move {
            ChannelSourceMonitor::refresh_source(
                self,
                network_scope,
                channel_id,
                source_config_revision,
                source_id,
            )
            .await
            .map_err(anyhow::Error::from)
        })
    }

    fn shutdown(&self) -> SourceMonitorShutdownFuture<'_> {
        Box::pin(async move { self.shutdown().await.map_err(anyhow::Error::from) })
    }
}

pub(crate) struct ZoneCatalogCommandInterface {
    service: ZoneCatalogService,
    monitor: Arc<dyn ZoneSourceMonitor>,
    source_store: Arc<dyn ChannelSourceConfigMutationInterface>,
    evidence: ZoneEvidenceCommandInterface,
    state: Mutex<ZoneProjectionLedger>,
    default_topology: Mutex<Option<(u64, ZoneCatalogDefaultTopology)>>,
}

impl ZoneCatalogCommandInterface {
    #[must_use]
    pub(crate) fn with_worker_and_module_transport(
        runtime: &Runtime,
        worker: Arc<dyn ZoneCatalogWorker>,
        module_transport: SharedModuleTransport,
        module_transport_kind: ModuleTransportKind,
    ) -> Self {
        let source_store = Arc::new(SettingsChannelSourceConfigMutation::with_module_transport(
            module_transport.clone(),
            module_transport_kind,
        ));
        Self::with_all_dependencies(
            runtime,
            worker,
            source_store,
            Arc::new(ChannelSourceMonitor::with_module_transport(
                runtime.handle(),
                Arc::clone(&module_transport),
                module_transport_kind,
            )),
            Arc::new(RoutedEvidenceBlockReader::new(module_transport)),
        )
    }

    #[cfg(test)]
    fn with_dependencies(
        runtime: &Runtime,
        worker: Arc<dyn ZoneCatalogWorker>,
        source_store: Arc<dyn ChannelSourceConfigMutationInterface>,
        monitor: Arc<dyn ZoneSourceMonitor>,
    ) -> Self {
        Self::with_all_dependencies(
            runtime,
            worker,
            source_store,
            monitor,
            Arc::new(DirectEvidenceBlockReader),
        )
    }

    fn with_all_dependencies(
        runtime: &Runtime,
        worker: Arc<dyn ZoneCatalogWorker>,
        source_store: Arc<dyn ChannelSourceConfigMutationInterface>,
        monitor: Arc<dyn ZoneSourceMonitor>,
        evidence_reader: Arc<dyn EvidenceBlockReader>,
    ) -> Self {
        Self {
            service: ZoneCatalogService::new(runtime.handle(), worker),
            monitor,
            source_store,
            evidence: ZoneEvidenceCommandInterface::new(evidence_reader),
            state: Mutex::new(ZoneProjectionLedger::default()),
            default_topology: Mutex::new(None),
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
            ZoneCatalogCommand::CurrentSourceConfig => {
                let request = decode_object_request(args, "channelSourceConfigCurrent")?;
                to_value(self.current_source_config(request)?)
            }
            ZoneCatalogCommand::ApplySourceConfig => {
                let request = decode_object_request(args, "channelSourceConfigApply")?;
                to_value(self.apply_source_config(runtime, request)?)
            }
            ZoneCatalogCommand::RefreshIndexerSource => {
                let request = decode_object_request(args, "channelIndexerSourceRefresh")?;
                to_value(self.refresh_indexer_source(runtime, request)?)
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

    pub(crate) async fn shutdown(&self) -> Result<()> {
        let (service_result, monitor_result) =
            tokio::join!(self.service.shutdown(), self.monitor.shutdown());
        service_result.context("failed to shut down zone catalog service")?;
        monitor_result.context("failed to shut down channel source monitor")?;
        Ok(())
    }

    fn configure(
        &self,
        runtime: &Runtime,
        request: ZoneCatalogConfigureRequest,
    ) -> Result<ZoneCatalogConfigureReport> {
        let (source, default_topology) = match request.source {
            ZoneCatalogSourceRequest::DirectHttp {
                endpoint,
                default_topology,
            } => (
                ZoneCatalogSourceDescriptor::direct_http(endpoint)?,
                default_topology,
            ),
            ZoneCatalogSourceRequest::LogoscoreCli { default_topology } => (
                ZoneCatalogSourceDescriptor::logoscore_cli(),
                default_topology,
            ),
        };
        let source_revision = runtime.block_on(self.service.configure(source.clone()))?;
        *self.lock_default_topology()? =
            default_topology.map(|topology| (source_revision, topology));
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
            source_config_epoch: state.source_config_epoch(),
            observation_revision: state.observation_revision(),
            summary_revision: state.summary_revision(),
            verification: service.verification_state,
            coverage,
            ingestion,
            readiness: service.readiness,
            current_error: service.current_error,
        })
    }

    fn summaries(
        &self,
        runtime: &Runtime,
        request: ZonesSummaryRequest,
    ) -> Result<ZonesSummaryReport> {
        self.refresh(runtime)?;
        let mut state = self.lock_state()?;
        state.summaries(request)
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
        state.detail_report(request, &service, &catalog)
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
        self.rebind_default_topology(current, source_revision)?;
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
        {
            let state = self.lock_state()?;
            if !state.contains_channel(&request.channel_id) {
                bail!("Channel is not present in current Zone Catalog projection");
            }
        }
        let legacy_anchor = if matches!(
            &request.mutation,
            ChannelSourceConfigMutation::AddSequencer { .. }
                | ChannelSourceConfigMutation::UpdateSequencer { .. }
                | ChannelSourceConfigMutation::RetryAttestation { .. }
        ) {
            self.evidence
                .sequencer_attestation_anchor(&self.service, &request.channel_id)
        } else {
            SequencerLegacyAnchorState::Missing
        };
        let outcome = runtime.block_on(
            self.source_store
                .clone()
                .apply_with_legacy_anchor(request, legacy_anchor),
        )?;
        let attestation_warning = match outcome.attestation {
            ChannelSourceAttestationOutcome::Pending => Some(ChannelSourceAttestationWarning {
                code: ChannelSourceAttestationWarningCode::PendingAttestation,
                recovery: ChannelSourceAttestationRecovery::Retry,
                message:
                    "Sequencer Channel verification is pending; retry when the source is available."
                        .to_owned(),
            }),
            ChannelSourceAttestationOutcome::EvidenceMatched => {
                Some(ChannelSourceAttestationWarning {
                    code: ChannelSourceAttestationWarningCode::LegacyEvidenceMatched,
                    recovery: ChannelSourceAttestationRecovery::None,
                    message: "Legacy Sequencer does not expose Channel identity. This user-selected mapping is enabled because its live block matches finalized L1 evidence for this Channel."
                        .to_owned(),
                })
            }
            ChannelSourceAttestationOutcome::NotRequired
            | ChannelSourceAttestationOutcome::Persisted => None,
        };
        let config = outcome.config;
        self.refresh(runtime)?;
        let service = self.service.report();
        let state = self.lock_state()?;
        state.source_config_report(&service, config, attestation_warning)
    }

    fn current_source_config(
        &self,
        request: ChannelSourceConfigCurrentRequest,
    ) -> Result<ChannelSourceConfigCurrentReport> {
        let service = self.service.report();
        require_verified(&service.verification_state)?;
        let catalog = service
            .catalog
            .context("verified Zone Catalog is unavailable")?;
        if request.network_scope != catalog.metadata.network_scope {
            bail!("Channel source configuration belongs to a stale network scope");
        }
        {
            let state = self.lock_state()?;
            if !state.contains_channel(&request.channel_id) {
                bail!("Channel is not present in current Zone Catalog projection");
            }
        }
        let config = self
            .source_store
            .load()?
            .into_iter()
            .find(|config| {
                config.network_scope == request.network_scope
                    && config.channel_id == request.channel_id
            })
            .unwrap_or_else(|| ChannelSourceConfig {
                network_scope: request.network_scope.clone(),
                channel_id: request.channel_id.clone(),
                config_revision: 0,
                sequencer_sources: Vec::new(),
                selected_sequencer_source_id: None,
                indexer_source: None,
            });
        Ok(ChannelSourceConfigCurrentReport {
            report_kind: "zones.channel_source_config_current",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: service.source_revision,
            network_scope: request.network_scope,
            channel_id: request.channel_id,
            config,
        })
    }

    fn refresh_indexer_source(
        &self,
        runtime: &Runtime,
        request: ChannelIndexerSourceRefreshRequest,
    ) -> Result<ChannelIndexerSourceRefreshReport> {
        self.refresh(runtime)?;
        let service = self.service.report();
        require_verified(&service.verification_state)?;
        if request.source_revision != service.source_revision {
            bail!(
                "Zone Catalog source revision conflict: expected {}, current {}",
                request.source_revision,
                service.source_revision
            );
        }
        let catalog = service
            .catalog
            .context("verified Zone Catalog is unavailable")?;
        if request.network_scope != catalog.metadata.network_scope {
            bail!("Channel source refresh belongs to a stale network scope");
        }
        {
            let state = self.lock_state()?;
            if !state.contains_channel(&request.channel_id) {
                bail!("Channel is not present in current Zone Catalog projection");
            }
        }
        let config = self
            .source_store
            .load()?
            .into_iter()
            .find(|config| {
                config.network_scope == request.network_scope
                    && config.channel_id == request.channel_id
            })
            .context("Channel source configuration is unavailable for this Channel")?;
        if config.config_revision != request.source_config_revision {
            bail!("Channel source configuration changed before the refresh could run");
        }
        let source = config
            .indexer_source
            .as_ref()
            .filter(|source| source.source_id == request.source_id)
            .context("Channel Indexer source is unavailable for this Channel")?;
        if !matches!(
            &source.target,
            crate::source_routing::channel_sources::ChannelSourceTarget::Module { module_id }
                if module_id == indexer::MODULE_ID
        ) {
            bail!("Channel Indexer source is not the managed Indexer module");
        }
        let observation_revision = runtime.block_on(self.monitor.refresh_source(
            request.network_scope.clone(),
            request.channel_id.clone(),
            request.source_config_revision,
            request.source_id.clone(),
        ))?;
        Ok(ChannelIndexerSourceRefreshReport {
            report_kind: "zones.channel_indexer_source_refresh",
            schema_version: ZONE_CATALOG_REPORT_SCHEMA_VERSION,
            source_revision: service.source_revision,
            network_scope: request.network_scope,
            channel_id: request.channel_id,
            source_config_revision: request.source_config_revision,
            source_id: request.source_id,
            observation_revision,
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

    pub(crate) fn context_snapshot(
        &self,
        runtime: &Runtime,
    ) -> Result<crate::inspection::l2::ZoneL2RuntimeFacts> {
        self.refresh(runtime)?;
        let state = self.lock_state()?;
        Ok(state.runtime_facts())
    }

    fn refresh(&self, runtime: &Runtime) -> Result<()> {
        let service = self.service.report();
        self.evidence.reconcile(&service)?;
        let default_topology = self
            .lock_default_topology()?
            .filter(|(source_revision, _)| *source_revision == service.source_revision)
            .map(|(_, topology)| topology);
        let configs = match service.catalog.as_deref() {
            Some(catalog)
                if default_topology == Some(ZoneCatalogDefaultTopology::LogosTestnet)
                    && service.verification_state == CatalogVerificationState::Verified
                    && catalog.zones.iter().any(|zone| {
                        zone.channel_id == crate::testnet::LOGOS_TESTNET_CHANNEL_ID
                    }) =>
            {
                self.source_store
                    .ensure_testnet_defaults(catalog.metadata.network_scope.clone())?
            }
            _ => self.source_store.load()?,
        };
        synchronize_monitor(runtime, self.monitor.as_ref(), &service, &configs)?;
        let observations = self.monitor.snapshot();
        let mut state = self.lock_state()?;
        state.refresh(&service, configs, observations)
    }

    fn lock_default_topology(
        &self,
    ) -> Result<MutexGuard<'_, Option<(u64, ZoneCatalogDefaultTopology)>>> {
        self.default_topology
            .lock()
            .map_err(|_| anyhow::anyhow!("Zone Catalog default topology lock is poisoned"))
    }

    fn rebind_default_topology(
        &self,
        previous_source_revision: u64,
        source_revision: u64,
    ) -> Result<()> {
        let mut default_topology = self.lock_default_topology()?;
        if let Some((bound_revision, _)) = default_topology.as_mut()
            && *bound_revision == previous_source_revision
        {
            *bound_revision = source_revision;
        }
        Ok(())
    }

    fn lock_state(&self) -> Result<MutexGuard<'_, ZoneProjectionLedger>> {
        self.state
            .lock()
            .map_err(|_| anyhow::anyhow!("Zone Catalog projection lock is poisoned"))
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
    use std::collections::VecDeque;

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
                ZoneCatalogPublication, ZoneCatalogReadinessPhase, ZoneCatalogReadinessReport,
                ZoneCatalogRecord, ZoneCatalogRunContext, ZoneCatalogRunMode,
                ZoneCatalogSourceDescriptor, ZoneCatalogWorkerFuture, ZoneClassificationCounters,
                ZoneEvidenceKind, ZoneEvidenceReference,
            },
        },
        inspector::commands::zone_evidence::EvidenceBlockFuture,
        source_routing::channel_sources::{
            ChannelSourceTarget, ConfiguredIndexerSource, ConfiguredSequencerSource,
            PersistedSequencerAttestation,
        },
    };

    #[test]
    fn verified_catalog_admits_testnet_defaults_only_for_public_lez_channel() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('9');
        let block = evidence_block(crate::testnet::LOGOS_TESTNET_CHANNEL_ID, b"testnet");
        let store = Arc::new(FakeSourceStore::new(Vec::new()));
        let monitor = Arc::new(FakeMonitor::default());
        let (worker, mut started) = PublishingWorker::new(evidence_snapshot(
            network_scope.clone(),
            crate::testnet::LOGOS_TESTNET_CHANNEL_ID,
            &block,
        ));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store.clone(),
            monitor,
        );

        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{
                "source": {
                    "kind": "direct_http",
                    "endpoint": "http://127.0.0.1:8080",
                    "default_topology": "logos_testnet"
                }
            }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;
        interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;

        let scopes = store
            .testnet_default_scopes
            .lock()
            .map_err(|_| anyhow::anyhow!("fake Testnet defaults lock poisoned"))?;
        if scopes.is_empty() || scopes.iter().any(|scope| scope != &network_scope) {
            bail!("verified Testnet Channel did not admit scoped defaults: {scopes:?}");
        }
        Ok(())
    }

    #[test]
    fn logoscore_cli_catalog_configuration_accepts_an_endpoint_free_source() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('9');
        let channel_id = identity('a');
        let block = evidence_block(&channel_id, b"CLI catalog evidence");
        let (worker, mut started) =
            PublishingWorker::new(evidence_snapshot(network_scope, &channel_id, &block));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            Arc::new(FakeSourceStore::default()),
            Arc::new(FakeMonitor::default()),
        );

        let report = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{
                "source": {
                    "kind": "logoscore_cli"
                }
            }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("CLI catalog worker did not start")?;
        if report.get("source_revision").and_then(Value::as_u64) != Some(1) {
            bail!("endpoint-free CLI catalog configure was rejected: {report}");
        }
        Ok(())
    }

    #[test]
    fn retry_rebinds_testnet_default_topology_to_new_source_revision() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('9');
        let block = evidence_block(crate::testnet::LOGOS_TESTNET_CHANNEL_ID, b"testnet");
        let store = Arc::new(FakeSourceStore::new(Vec::new()));
        let monitor = Arc::new(FakeMonitor::default());
        let (worker, mut started) = PublishingWorker::new(evidence_snapshot(
            network_scope.clone(),
            crate::testnet::LOGOS_TESTNET_CHANNEL_ID,
            &block,
        ));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store.clone(),
            monitor,
        );

        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{
                "source": {
                    "kind": "direct_http",
                    "endpoint": "http://127.0.0.1:8080",
                    "default_topology": "logos_testnet"
                }
            }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;
        store
            .testnet_default_scopes
            .lock()
            .map_err(|_| anyhow::anyhow!("fake Testnet defaults lock poisoned"))?
            .clear();
        let retry = interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Retry,
            &json!([{ "source_revision": 1 }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("retried catalog worker did not publish")?;
        interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;

        if retry.get("source_revision").and_then(Value::as_u64) != Some(2) {
            bail!("retry did not advance the source revision: {retry}");
        }
        let scopes = store
            .testnet_default_scopes
            .lock()
            .map_err(|_| anyhow::anyhow!("fake Testnet defaults lock poisoned"))?;
        if scopes.is_empty() || scopes.iter().any(|scope| scope != &network_scope) {
            bail!("retry lost scoped Testnet defaults: {scopes:?}");
        }
        Ok(())
    }

    #[test]
    fn verified_foreign_catalog_with_public_channel_id_does_not_admit_testnet_defaults()
    -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('8');
        let block = evidence_block(crate::testnet::LOGOS_TESTNET_CHANNEL_ID, b"foreign");
        let store = Arc::new(FakeSourceStore::new(Vec::new()));
        let monitor = Arc::new(FakeMonitor::default());
        let (worker, mut started) = PublishingWorker::new(evidence_snapshot(
            network_scope,
            crate::testnet::LOGOS_TESTNET_CHANNEL_ID,
            &block,
        ));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store.clone(),
            monitor,
        );

        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{
                "source": {
                    "kind": "direct_http",
                    "endpoint": "https://custom.example"
                }
            }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("catalog worker did not publish")?;
        interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;

        let scopes = store
            .testnet_default_scopes
            .lock()
            .map_err(|_| anyhow::anyhow!("fake Testnet defaults lock poisoned"))?;
        if !scopes.is_empty() {
            bail!("foreign catalog admitted Testnet defaults: {scopes:?}");
        }
        Ok(())
    }

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
            || status.get("readiness").is_some()
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
        let current_source_command = zone_catalog_command("channelSourceConfigCurrent")
            .context("current Channel source command is missing")?;
        let current_source = interface.bridge_call(
            &runtime,
            current_source_command,
            &json!([{
                "network_scope": scope,
                "channel_id": channel_a
            }]),
        )?;
        if current_source.get("report_kind").and_then(Value::as_str)
            != Some("zones.channel_source_config_current")
            || current_source
                .pointer("/config/config_revision")
                .and_then(Value::as_u64)
                != Some(2)
            || current_source.get("network_scope") != Some(&network_scope)
            || current_source.get("channel_id").and_then(Value::as_str) != Some(channel_a.as_str())
        {
            bail!(
                "current Channel source read did not return persisted revision 2: {current_source}"
            );
        }
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

        store.queue_apply(Err(anyhow::anyhow!(
            "Channel source configuration revision conflict: expected 1, current 2"
        )))?;
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
    fn catalog_status_exposes_live_bedrock_readiness() -> Result<()> {
        let runtime = Runtime::new()?;
        let store = Arc::new(FakeSourceStore::new(Vec::new()));
        let monitor = Arc::new(FakeMonitor::default());
        let (worker, mut started) = PublishingWorker::source_behind(snapshot(scope('1')));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store,
            monitor,
        );

        interface.bridge_call(
            &runtime,
            ZoneCatalogCommand::Configure,
            &json!([{
                "source": {
                    "kind": "direct_http",
                    "endpoint": "https://l1.example"
                }
            }]),
        )?;
        runtime
            .block_on(started.recv())
            .context("source-behind catalog worker did not publish")?;
        let status = interface.bridge_call(&runtime, ZoneCatalogCommand::Status, &json!([{}]))?;

        if status.get("verification").and_then(Value::as_str) != Some("source_behind")
            || status.pointer("/readiness/phase").and_then(Value::as_str)
                != Some("waiting_for_bedrock")
            || status
                .pointer("/readiness/finalized_lib_slot")
                .and_then(Value::as_u64)
                != Some(0)
            || status
                .pointer("/readiness/required_checkpoint_slot")
                .and_then(Value::as_u64)
                != Some(691_337)
        {
            bail!("source-behind status omitted live Bedrock readiness: {status}");
        }
        Ok(())
    }

    #[test]
    fn managed_indexer_source_refresh_is_fenced_to_the_exact_configured_source() -> Result<()> {
        let runtime = Runtime::new()?;
        let network_scope = scope('4');
        let channel_id = identity('a');
        let indexer_source_id = source_id('i');
        let mut channel_config = config(&network_scope, &channel_id, 'c', 1, 1);
        channel_config.indexer_source = Some(ConfiguredIndexerSource {
            source_id: indexer_source_id.clone(),
            label: Some("Managed Indexer".to_owned()),
            target: ChannelSourceTarget::Module {
                module_id: indexer::MODULE_ID.to_owned(),
            },
        });
        let store = Arc::new(FakeSourceStore::new(vec![channel_config]));
        let monitor = Arc::new(FakeMonitor::default());
        let block = evidence_block(&channel_id, b"managed-indexer");
        let (worker, mut started) = PublishingWorker::new(evidence_snapshot(
            network_scope.clone(),
            &channel_id,
            &block,
        ));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store,
            monitor.clone(),
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
        let source_revision = required_u64(&status, "source_revision")?;
        let command = zone_catalog_command("channelIndexerSourceRefresh")
            .context("Indexer source refresh command is missing")?;
        let refreshed = interface.bridge_call(
            &runtime,
            command,
            &json!([{
                "source_revision": source_revision,
                "network_scope": network_scope,
                "channel_id": channel_id,
                "source_config_revision": 1,
                "source_id": indexer_source_id
            }]),
        )?;
        if refreshed.get("report_kind").and_then(Value::as_str)
            != Some("zones.channel_indexer_source_refresh")
            || refreshed
                .get("source_config_revision")
                .and_then(Value::as_u64)
                != Some(1)
        {
            bail!("unexpected Indexer source refresh report: {refreshed}");
        }
        let refreshes = monitor
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("fake monitor lock poisoned"))?
            .source_refreshes
            .clone();
        if refreshes != vec![(scope('4'), identity('a'), 1, source_id('i'))] {
            bail!("Indexer source refresh lost exact binding: {refreshes:?}");
        }
        let stale = interface.bridge_call(
            &runtime,
            command,
            &json!([{
                "source_revision": source_revision,
                "network_scope": scope('4'),
                "channel_id": identity('a'),
                "source_config_revision": 2,
                "source_id": source_id('i')
            }]),
        );
        let Err(error) = stale else {
            bail!("stale Indexer source refresh unexpectedly succeeded");
        };
        if !error.to_string().contains("configuration changed") {
            bail!("unexpected stale Indexer source refresh error: {error:#}");
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
        store.queue_apply(Err(anyhow::anyhow!(
            "Sequencer source verification resolved to another Channel"
        )))?;
        let (worker, mut started) = PublishingWorker::new(snapshot(network_scope.clone()));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store.clone(),
            Arc::new(FakeMonitor::default()),
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
            .contains("Sequencer source verification resolved to another Channel")
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
        let initial = config(&network_scope, &channel_id, 'c', 1, 1);
        let mut applied = initial.clone();
        let target = ChannelSourceTarget::Rpc {
            endpoint: "https://backup.example/".to_owned(),
        };
        applied.sequencer_sources.push(ConfiguredSequencerSource {
            source_id: source_id('e'),
            label: Some("Backup".to_owned()),
            target: target.clone(),
            channel_attestation: PersistedSequencerAttestation::PersistedAttested {
                channel_id: channel_id.clone(),
                target_fingerprint: target.fingerprint(),
                attested_at_unix: 2,
            },
        });
        applied.config_revision = 2;
        let store = Arc::new(FakeSourceStore::new(vec![initial]));
        store.queue_apply(Ok(
            crate::source_routing::channel_sources::ChannelSourceConfigApplyOutcome {
                config: applied,
                attestation: ChannelSourceAttestationOutcome::Persisted,
            },
        ))?;
        let (worker, mut started) = PublishingWorker::new(snapshot(network_scope.clone()));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store,
            Arc::new(FakeMonitor::default()),
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
        let initial = config(&network_scope, &channel_id, 'c', 1, 1);
        let mut applied = initial.clone();
        applied.sequencer_sources.push(ConfiguredSequencerSource {
            source_id: source_id('e'),
            label: None,
            target: ChannelSourceTarget::Rpc {
                endpoint: "https://offline.example/".to_owned(),
            },
            channel_attestation: PersistedSequencerAttestation::Pending,
        });
        applied.config_revision = 2;
        let store = Arc::new(FakeSourceStore::new(vec![initial]));
        store.queue_apply(Ok(
            crate::source_routing::channel_sources::ChannelSourceConfigApplyOutcome {
                config: applied,
                attestation: ChannelSourceAttestationOutcome::Pending,
            },
        ))?;
        let (worker, mut started) = PublishingWorker::new(snapshot(network_scope.clone()));
        let interface = ZoneCatalogCommandInterface::with_dependencies(
            &runtime,
            Arc::new(worker),
            store,
            Arc::new(FakeMonitor::default()),
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
        verification_state: CatalogVerificationState,
        readiness: Option<ZoneCatalogReadinessReport>,
        current_error: Option<String>,
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
                    verification_state: CatalogVerificationState::Verified,
                    readiness: None,
                    current_error: None,
                    started,
                },
                receiver,
            )
        }

        fn source_behind(
            snapshot: CatalogSnapshot,
        ) -> (Self, mpsc::UnboundedReceiver<ZoneCatalogRunMode>) {
            let (mut worker, receiver) = Self::new(snapshot);
            worker.verification_state = CatalogVerificationState::SourceBehind;
            worker.readiness = Some(ZoneCatalogReadinessReport {
                phase: ZoneCatalogReadinessPhase::WaitingForBedrock,
                finalized_lib_slot: 0,
                required_checkpoint_slot: 691_337,
            });
            worker.current_error = Some("Bedrock is still syncing".to_owned());
            (worker, receiver)
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
                    verification_state: self.verification_state,
                    catalog: Some(self.snapshot.clone()),
                    readiness: self.readiness,
                    current_error: self.current_error.clone(),
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
        testnet_default_scopes: Mutex<Vec<NetworkScope>>,
        apply_results: Mutex<
            VecDeque<
                Result<crate::source_routing::channel_sources::ChannelSourceConfigApplyOutcome>,
            >,
        >,
    }

    impl FakeSourceStore {
        fn new(configs: Vec<ChannelSourceConfig>) -> Self {
            Self {
                configs: Mutex::new(configs),
                testnet_default_scopes: Mutex::new(Vec::new()),
                apply_results: Mutex::new(VecDeque::new()),
            }
        }

        fn replace(&self, configs: Vec<ChannelSourceConfig>) -> Result<()> {
            *self
                .configs
                .lock()
                .map_err(|_| anyhow::anyhow!("fake source store lock poisoned"))? = configs;
            Ok(())
        }

        fn queue_apply(
            &self,
            result: Result<crate::source_routing::channel_sources::ChannelSourceConfigApplyOutcome>,
        ) -> Result<()> {
            self.apply_results
                .lock()
                .map_err(|_| anyhow::anyhow!("fake source result lock poisoned"))?
                .push_back(result);
            Ok(())
        }
    }

    impl ChannelSourceConfigMutationInterface for FakeSourceStore {
        fn load(&self) -> Result<Vec<ChannelSourceConfig>> {
            Ok(self
                .configs
                .lock()
                .map_err(|_| anyhow::anyhow!("fake source store lock poisoned"))?
                .clone())
        }

        fn ensure_testnet_defaults(
            &self,
            network_scope: NetworkScope,
        ) -> Result<Vec<ChannelSourceConfig>> {
            self.testnet_default_scopes
                .lock()
                .map_err(|_| anyhow::anyhow!("fake Testnet defaults lock poisoned"))?
                .push(network_scope);
            self.load()
        }

        fn apply(
            self: Arc<Self>,
            _request: ChannelSourceApplyRequest,
        ) -> crate::source_routing::channel_sources::ChannelSourceConfigMutationFuture {
            let result = self
                .apply_results
                .lock()
                .map_err(|_| anyhow::anyhow!("fake source result lock poisoned"))
                .and_then(|mut results| {
                    results
                        .pop_front()
                        .context("fake source mutation result was not configured")?
                });
            if let Ok(outcome) = &result {
                let update = self.configs.lock().map(|mut configs| {
                    if let Some(current) = configs.iter_mut().find(|current| {
                        current.network_scope == outcome.config.network_scope
                            && current.channel_id == outcome.config.channel_id
                    }) {
                        *current = outcome.config.clone();
                    } else {
                        configs.push(outcome.config.clone());
                    }
                });
                if update.is_err() {
                    return Box::pin(async {
                        Err(anyhow::anyhow!("fake source store lock poisoned"))
                    });
                }
            }
            Box::pin(async move { result })
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
        source_refreshes: Vec<(NetworkScope, String, u64, String)>,
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

        fn refresh_source<'a>(
            &'a self,
            network_scope: NetworkScope,
            channel_id: String,
            source_config_revision: u64,
            source_id: String,
        ) -> SourceMonitorFuture<'a> {
            Box::pin(async move {
                let mut state = self
                    .state
                    .lock()
                    .map_err(|_| anyhow::anyhow!("fake monitor lock poisoned"))?;
                let config = state
                    .configs
                    .iter()
                    .find(|config| {
                        config.network_scope == network_scope && config.channel_id == channel_id
                    })
                    .context("fake monitor Channel source configuration is missing")?;
                if config.config_revision != source_config_revision {
                    bail!("fake monitor Channel source configuration changed");
                }
                if config
                    .indexer_source
                    .as_ref()
                    .is_none_or(|source| source.source_id != source_id)
                {
                    bail!("fake monitor Channel Indexer source is missing");
                }
                state.source_refreshes.push((
                    network_scope,
                    channel_id,
                    source_config_revision,
                    source_id,
                ));
                Ok(state.snapshot.observation_revision)
            })
        }

        fn shutdown(&self) -> SourceMonitorShutdownFuture<'_> {
            Box::pin(async { Ok(()) })
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
