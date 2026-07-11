mod classification;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

use serde::{Deserialize, Serialize};

pub use classification::{ZoneClassificationEvidence, classify_zone};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneKind {
    SequencerZone,
    DataChannel,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneSummary {
    pub channel_id: String,
    pub display: ZoneDisplay,
    pub l1_channel: L1ChannelSummary,
    pub settlement_link: SettlementLinkSummary,
    pub activity_state: ZoneActivityState,
    pub activity_detail: ZoneActivityDetail,
    pub provenance: ZoneProvenance,
    #[serde(flatten)]
    pub facts: ZoneFacts,
}

impl ZoneSummary {
    #[must_use]
    pub fn kind(&self) -> ZoneKind {
        self.facts.kind()
    }

    #[must_use]
    pub fn l2_zone(&self) -> Option<&L2ZoneSummary> {
        match &self.facts {
            ZoneFacts::SequencerZone { l2_zone, .. } => Some(l2_zone),
            ZoneFacts::DataChannel { .. } | ZoneFacts::Unknown => None,
        }
    }

    #[must_use]
    pub fn sequencer_committee(&self) -> Option<&SequencerCommitteeSummary> {
        match &self.facts {
            ZoneFacts::SequencerZone {
                sequencer_committee,
                ..
            } => Some(sequencer_committee),
            ZoneFacts::DataChannel { .. } | ZoneFacts::Unknown => None,
        }
    }

    #[must_use]
    pub fn raw_activity(&self) -> Option<&RawActivitySummary> {
        match &self.facts {
            ZoneFacts::DataChannel { raw_activity } => Some(raw_activity),
            ZoneFacts::SequencerZone { .. } | ZoneFacts::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ZoneFacts {
    SequencerZone {
        l2_zone: L2ZoneSummary,
        sequencer_committee: SequencerCommitteeSummary,
    },
    DataChannel {
        raw_activity: RawActivitySummary,
    },
    Unknown,
}

impl ZoneFacts {
    #[must_use]
    pub fn kind(&self) -> ZoneKind {
        match self {
            Self::SequencerZone { .. } => ZoneKind::SequencerZone,
            Self::DataChannel { .. } => ZoneKind::DataChannel,
            Self::Unknown => ZoneKind::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneDisplay {
    pub title: String,
    pub alias: Option<String>,
    pub short_channel_id: String,
    pub alias_source: ZoneAliasSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneAliasSource {
    Configured,
    KnownStatic,
    ZonescanApi,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L1ChannelSummary {
    pub tip_slot: Option<u64>,
    pub tip_hash: Option<String>,
    pub lib_slot: Option<u64>,
    pub balance: Option<String>,
    pub key_count: Option<u64>,
    pub withdraw_threshold: Option<String>,
    pub operation_count: u64,
    pub finality_state: L1FinalityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L1FinalityState {
    Final,
    Finalizing,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SettlementLinkSummary {
    pub status: SettlementLinkStatus,
    pub source: SettlementLinkSource,
    pub selected_sequencer_source_id: Option<String>,
    pub indexer_source_id: Option<String>,
    pub lag_blocks: Option<u64>,
    pub lag_slots: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementLinkStatus {
    Linked,
    Unconfigured,
    L1Only,
    RawData,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementLinkSource {
    Configured,
    L1Scan,
    ZonescanApi,
    Inferred,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L2ZoneSummary {
    pub source_status: L2SourceStatus,
    pub selected_source_id: Option<String>,
    pub configured_source_count: u64,
    pub observed_source_count: u64,
    pub latest_block_id: Option<u64>,
    pub latest_block_hash: Option<String>,
    pub safe_block_id: Option<u64>,
    pub finalized_block_id: Option<u64>,
    pub finality_state: L2FinalityState,
    pub agreement_state: SequencerAgreementState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2SourceStatus {
    Reachable,
    Unconfigured,
    Unreachable,
    Stale,
    Degraded,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum L2FinalityState {
    Final,
    Safe,
    Pending,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SequencerAgreementState {
    NotApplicable,
    Unconfigured,
    Unobserved,
    SingleSource,
    Converged,
    Lagging,
    SkewUnverified,
    Divergent,
    FinalizedConflict,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencerCommitteeSummary {
    pub members: Vec<String>,
    pub active_member: Option<String>,
    pub observed_at_slot: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RawActivitySummary {
    pub inscription_count: u64,
    pub latest_slot: Option<u64>,
    pub latest_payload_size: Option<u64>,
    pub finality_state: L1FinalityState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneActivityState {
    Active,
    Idle,
    Finalizing,
    Raw,
    ClockOnly,
    Degraded,
    Unknown,
}

impl ZoneActivityState {
    #[must_use]
    pub fn is_valid_for(self, kind: ZoneKind) -> bool {
        match self {
            Self::Raw => kind == ZoneKind::DataChannel,
            Self::ClockOnly => kind == ZoneKind::SequencerZone,
            Self::Active | Self::Idle | Self::Finalizing | Self::Degraded | Self::Unknown => true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneActivityDetail {
    pub reason: String,
    pub last_seen_unix: Option<u64>,
    pub last_l1_slot: Option<u64>,
    pub last_l2_block_id: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneProvenance {
    pub network_scope: Option<NetworkScope>,
    pub verification_state: CatalogVerificationState,
    pub coverage: ZoneCoverageProvenance,
    pub observed_at_unix: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum NetworkScope {
    GenesisId {
        genesis_id: String,
    },
    FinalizedAnchor {
        genesis_time: String,
        block_slot: u64,
        block_id: String,
        parent_id: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogVerificationState {
    Empty,
    CachedUnverified,
    Verifying,
    SourceBehind,
    Verified,
    Mismatch,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneCoverageProvenance {
    pub status: CatalogCoverageStatus,
    pub coverage_floor: Option<u64>,
    pub scanned_through_slot: Option<u64>,
    pub observed_lib_slot: Option<u64>,
    pub prefix_status: CoveragePrefixStatus,
    pub continuity_checkpoint: Option<FinalizedBlockCheckpoint>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogCoverageStatus {
    Complete,
    Partial,
    Rebuilding,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoveragePrefixStatus {
    Complete,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalizedBlockCheckpoint {
    pub slot: u64,
    pub block_id: String,
    pub parent_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneDetail {
    pub summary: ZoneSummary,
    pub l1_channel_snapshot: L1ChannelSnapshot,
    pub channel_source_config: ChannelSourceConfigSummary,
    pub source_observations: Vec<ZoneSourceObservation>,
    pub classification_evidence: ZoneClassificationEvidence,
    pub activity_counts: ZoneActivityCounts,
    pub detail_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct L1ChannelSnapshot {
    pub channel_tip: Option<String>,
    pub keys: Vec<String>,
    pub observed_at_slot: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSourceConfigSummary {
    pub config_revision: u64,
    pub selected_sequencer_source_id: Option<String>,
    pub sequencer_sources: Vec<ConfiguredZoneSource>,
    pub indexer_source: Option<ConfiguredZoneSource>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredZoneSource {
    pub source_id: String,
    pub label: Option<String>,
    pub target: ZoneSourceTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ZoneSourceTarget {
    Rpc { endpoint: String },
    Module { module_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneSourceObservation {
    pub source_id: String,
    pub role: ZoneSourceRole,
    pub health: ZoneSourceHealth,
    pub reported_channel_id: Option<String>,
    pub head_block_id: Option<u64>,
    pub head_block_hash: Option<String>,
    pub observed_at_unix: Option<u64>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneSourceRole {
    Sequencer,
    Indexer,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneSourceHealth {
    Pending,
    Reachable,
    Unreachable,
    Stale,
    ChannelMismatch,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneActivityCounts {
    pub l1_operations: u64,
    pub recognized_l2_blocks: u64,
    pub raw_inscriptions: u64,
}
