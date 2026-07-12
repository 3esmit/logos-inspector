use super::{CoverageSegment, ZoneEvidenceReference};
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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneEvidencePageRequest {
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub catalog_revision: u64,
    pub channel_id: String,
    #[serde(default)]
    pub filter: ZoneEvidenceFilter,
    pub cursor: Option<String>,
    pub limit: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneEvidenceDetailRequest {
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub catalog_revision: u64,
    pub channel_id: String,
    pub reference: ZoneEvidenceReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneEvidencePayloadChunkRequest {
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub evidence_id: String,
    pub session_id: String,
    pub offset: u64,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ZoneEvidencePayloadReleaseRequest {
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub channel_id: String,
    pub evidence_id: String,
    pub session_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneEvidenceFilter {
    #[default]
    All,
    ChannelConfiguration,
    ChannelOperation,
    RawInscription,
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
pub struct ZoneEvidencePageReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub catalog_revision: u64,
    pub channel_id: String,
    pub filter: ZoneEvidenceFilter,
    pub rows: Vec<ZoneEvidenceRow>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidenceRow {
    pub reference: ZoneEvidenceReference,
    pub segment: ZoneEvidenceSegmentProvenance,
    pub source: ZoneEvidenceSourceProvenance,
    pub finality: ZoneEvidenceFinality,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidenceSegmentProvenance {
    pub segment_id: String,
    pub floor_slot: u64,
    pub frontier_slot: u64,
    pub reaches_target_lib: bool,
}

impl From<&CoverageSegment> for ZoneEvidenceSegmentProvenance {
    fn from(segment: &CoverageSegment) -> Self {
        Self {
            segment_id: segment.segment_id.clone(),
            floor_slot: segment.floor.slot,
            frontier_slot: segment.frontier.slot,
            reaches_target_lib: segment.reaches_target_lib,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidenceSourceProvenance {
    pub kind: ZoneEvidenceSourceKind,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneEvidenceSourceKind {
    DirectHttp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneEvidenceFinality {
    Final,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidenceDetailReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub source_revision: u64,
    pub network_scope: NetworkScope,
    pub catalog_revision: u64,
    pub channel_id: String,
    pub row: ZoneEvidenceRow,
    pub operation: ZoneEvidenceOperation,
    pub payload: ZoneEvidencePayloadReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ZoneEvidenceOperation {
    pub opcode: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidencePayloadReport {
    pub byte_length: u64,
    pub sha256: String,
    pub encoding: ZoneEvidencePayloadEncoding,
    pub inline_text: Option<String>,
    pub inline_base64: Option<String>,
    pub preview: String,
    pub preview_truncated: bool,
    pub inline_truncated: bool,
    pub session_id: Option<String>,
    pub warning: Option<ZoneEvidencePayloadWarning>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneEvidencePayloadEncoding {
    Json,
    Utf8,
    Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidencePayloadWarning {
    pub code: ZoneEvidencePayloadWarningCode,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneEvidencePayloadWarningCode {
    EvidenceTooLarge,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidencePayloadChunkReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub session_id: String,
    pub evidence_id: String,
    pub encoding: ZoneEvidencePayloadEncoding,
    pub offset: u64,
    pub next_offset: u64,
    pub done: bool,
    pub text: Option<String>,
    pub base64: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ZoneEvidencePayloadReleaseReport {
    pub report_kind: &'static str,
    pub schema_version: u32,
    pub session_id: String,
    pub released: bool,
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
