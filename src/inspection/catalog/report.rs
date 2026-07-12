use serde::{Deserialize, Serialize};

use crate::{
    inspection::{
        CatalogCoverageStatus, CatalogVerificationState, CoveragePrefixStatus,
        FinalizedBlockCheckpoint, NetworkScope, ZoneDetail, ZoneSourceObservation, ZoneSummary,
        sources::ZoneSourceAgreement,
    },
    source_routing::channel_sources::{ChannelSourceConfig, ChannelSourceConfigApplyRequest},
};

pub const ZONE_CATALOG_REPORT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneCatalogConfigureRequest {
    pub source: ZoneCatalogSourceRequest,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ZoneCatalogSourceRequest {
    DirectHttp { endpoint: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneCatalogStatusRequest {}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZonesSummaryRequest {
    pub source_revision: u64,
    pub network_scope: Option<NetworkScope>,
    pub after_summary_revision: Option<u64>,
    pub cursor: Option<String>,
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneDetailRequest {
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub catalog_revision: u64,
    pub summary_revision: u64,
    pub observation_revision: u64,
    pub channel_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneCatalogControlRequest {
    pub source_revision: u64,
}

pub type ChannelSourceApplyRequest = ChannelSourceConfigApplyRequest;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneCatalogConfigureReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub source_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ZoneCatalogControlReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub control: ZoneCatalogControl,
    pub source_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneCatalogControl {
    Retry,
    Rebuild,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneCatalogStatusReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub source_revision: u64,
    pub network_scope: Option<NetworkScope>,
    pub catalog_revision: u64,
    pub source_config_epoch: u64,
    pub observation_revision: u64,
    pub summary_revision: u64,
    pub verification: CatalogVerificationState,
    pub coverage: ZoneCatalogCoverageReport,
    pub ingestion: ZoneCatalogIngestionReport,
    pub current_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneCatalogCoverageReport {
    pub status: CatalogCoverageStatus,
    pub coverage_floor: Option<u64>,
    pub scanned_through_slot: Option<u64>,
    pub observed_lib_slot: Option<u64>,
    pub prefix_status: CoveragePrefixStatus,
    pub continuity_checkpoint: Option<FinalizedBlockCheckpoint>,
    pub gap_count: u64,
}

impl Default for ZoneCatalogCoverageReport {
    fn default() -> Self {
        Self {
            status: CatalogCoverageStatus::Unknown,
            coverage_floor: None,
            scanned_through_slot: None,
            observed_lib_slot: None,
            prefix_status: CoveragePrefixStatus::Unknown,
            continuity_checkpoint: None,
            gap_count: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ZoneCatalogIngestionReport {
    pub worker_running: bool,
    pub target_lib_slot: Option<u64>,
    pub ingestion_cursor_slot: Option<u64>,
    pub discovered_zone_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZonesSummaryReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub source_revision: u64,
    pub network_scope: Option<NetworkScope>,
    pub catalog_revision: u64,
    pub source_config_epoch: u64,
    pub observation_revision: u64,
    pub summary_revision: u64,
    pub changes: ZoneSummaryChanges,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ZoneSummaryChanges {
    Reset {
        rows: Vec<ZoneSummary>,
    },
    Delta {
        upserts: Vec<ZoneSummary>,
        removed_zone_ids: Vec<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneDetailReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub catalog_revision: u64,
    pub source_config_epoch: u64,
    pub observation_revision: u64,
    pub summary_revision: u64,
    pub detail: ZoneDetail,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChannelSourceConfigReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub source_revision: u64,
    pub catalog_revision: u64,
    pub source_config_epoch: u64,
    pub observation_revision: u64,
    pub summary_revision: u64,
    pub config: ChannelSourceConfig,
    pub observations: Vec<ZoneSourceObservation>,
    pub agreement: ZoneSourceAgreement,
    pub attestation_warning: Option<ChannelSourceAttestationWarning>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChannelSourceAttestationWarning {
    pub code: ChannelSourceAttestationWarningCode,
    pub recovery: ChannelSourceAttestationRecovery,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSourceAttestationWarningCode {
    PendingAttestation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelSourceAttestationRecovery {
    Retry,
}
