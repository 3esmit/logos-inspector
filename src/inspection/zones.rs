mod classification;

#[cfg(test)]
mod fixtures;
#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

pub use classification::{
    CatalogZoneClassification, ZoneClassificationEvidence, ZoneFactGates, catalog_fact_gates,
    classify_catalog_zone, classify_zone,
};

use crate::inspection::catalog::{
    CatalogSnapshot, CatalogSnapshotOrigin, ZoneCatalogRecord, ZoneEvidenceKind,
};
use crate::{
    inspection::sources::{ZoneSourceAgreement, project_zone_sources},
    source_routing::channel_sources::{
        ChannelSourceConfig, ChannelSourceMonitorSnapshot, ChannelSourceTarget,
        PersistedSequencerAttestation,
    },
};

#[must_use]
pub fn project_catalog_zones(
    snapshot: &CatalogSnapshot,
    source_configs: &[ChannelSourceConfig],
    verification_state: CatalogVerificationState,
) -> Vec<ZoneSummary> {
    let configs_by_channel: BTreeMap<&str, &ChannelSourceConfig> = source_configs
        .iter()
        .filter(|config| config.network_scope == snapshot.metadata.network_scope)
        .map(|config| (config.channel_id.as_str(), config))
        .collect();
    let catalog_channel_ids: BTreeSet<&str> = snapshot
        .zones
        .iter()
        .map(|record| record.channel_id.as_str())
        .collect();
    let mut zones = Vec::with_capacity(
        snapshot
            .zones
            .len()
            .saturating_add(configs_by_channel.len()),
    );

    for record in &snapshot.zones {
        let config = configs_by_channel.get(record.channel_id.as_str()).copied();
        zones.push(project_catalog_record(
            snapshot,
            record,
            config,
            verification_state,
        ));
    }
    for (channel_id, config) in configs_by_channel {
        if !catalog_channel_ids.contains(channel_id) {
            zones.push(project_configured_channel(
                snapshot,
                config,
                verification_state,
            ));
        }
    }
    zones.sort_by(|left, right| left.channel_id.cmp(&right.channel_id));
    zones
}

#[must_use]
pub fn project_catalog_zones_with_sources(
    snapshot: &CatalogSnapshot,
    source_configs: &[ChannelSourceConfig],
    source_observations: &ChannelSourceMonitorSnapshot,
    verification_state: CatalogVerificationState,
) -> Vec<ZoneSummary> {
    let mut zones = project_catalog_zones(snapshot, source_configs, verification_state);
    for zone in &mut zones {
        let config = source_configs.iter().find(|config| {
            config.network_scope == snapshot.metadata.network_scope
                && config.channel_id == zone.channel_id
        });
        let projection =
            project_zone_sources(zone.kind(), &zone.channel_id, config, source_observations);
        if let ZoneFacts::SequencerZone { l2_zone, .. } = &mut zone.facts {
            projection.apply_to_l2_zone(l2_zone);
            zone.activity_detail.last_l2_block_id = l2_zone.latest_block_id;
        }
    }
    zones
}

#[must_use]
pub fn project_catalog_zone_detail(
    snapshot: &CatalogSnapshot,
    source_configs: &[ChannelSourceConfig],
    source_observations: &ChannelSourceMonitorSnapshot,
    verification_state: CatalogVerificationState,
    channel_id: &str,
    detail_revision: u64,
) -> Option<ZoneDetail> {
    let summary = project_catalog_zones_with_sources(
        snapshot,
        source_configs,
        source_observations,
        verification_state,
    )
    .into_iter()
    .find(|summary| summary.channel_id == channel_id)?;
    let record = snapshot
        .zones
        .iter()
        .find(|record| record.channel_id == channel_id);
    let config = source_configs.iter().find(|config| {
        config.network_scope == snapshot.metadata.network_scope && config.channel_id == channel_id
    });
    let source_projection =
        project_zone_sources(summary.kind(), channel_id, config, source_observations);
    let l1_keys = summary
        .sequencer_committee()
        .map_or_else(Vec::new, |committee| committee.members.clone());
    let classification_evidence = record.map_or(
        ZoneClassificationEvidence {
            recognized_l2_evidence: false,
            configured_sequencer_link: config
                .is_some_and(|config| !config.sequencer_sources.is_empty()),
            raw_inscription_evidence: false,
            l2_absence_is_covered: false,
            conflicting_evidence: false,
        },
        |record| {
            classify_catalog_zone(
                snapshot,
                record,
                config.is_some_and(|config| !config.sequencer_sources.is_empty()),
            )
            .evidence
        },
    );
    Some(ZoneDetail {
        l1_channel_snapshot: L1ChannelSnapshot {
            channel_tip: record.and_then(|record| record.l1_channel.tip_hash.clone()),
            keys: l1_keys,
            observed_at_slot: record.map(|record| record.snapshot_provenance.observed_slot),
        },
        channel_source_config: project_source_config(config),
        source_observations: source_projection.observations,
        source_agreement: source_projection.agreement,
        classification_evidence,
        activity_counts: ZoneActivityCounts {
            l1_operations: record.map_or(0, |record| record.classification.channel_operations),
            recognized_l2_blocks: record
                .map_or(0, |record| record.classification.recognized_l2_blocks),
            raw_inscriptions: record.map_or(0, |record| record.classification.raw_inscriptions),
        },
        summary,
        detail_revision,
    })
}

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
    pub source_agreement: ZoneSourceAgreement,
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

fn project_source_config(config: Option<&ChannelSourceConfig>) -> ChannelSourceConfigSummary {
    let Some(config) = config else {
        return ChannelSourceConfigSummary {
            config_revision: 0,
            selected_sequencer_source_id: None,
            sequencer_sources: Vec::new(),
            indexer_source: None,
        };
    };
    ChannelSourceConfigSummary {
        config_revision: config.config_revision,
        selected_sequencer_source_id: config.selected_sequencer_source_id.clone(),
        sequencer_sources: config
            .sequencer_sources
            .iter()
            .map(|source| ConfiguredZoneSource {
                source_id: source.source_id.clone(),
                label: source.label.clone(),
                target: project_source_target(&source.target),
                binding_state: Some(match &source.channel_attestation {
                    PersistedSequencerAttestation::Pending => ZoneSourceBindingState::Pending,
                    PersistedSequencerAttestation::PersistedAttested { .. } => {
                        ZoneSourceBindingState::PersistedAttested
                    }
                }),
            })
            .collect(),
        indexer_source: config
            .indexer_source
            .as_ref()
            .map(|source| ConfiguredZoneSource {
                source_id: source.source_id.clone(),
                label: source.label.clone(),
                target: project_source_target(&source.target),
                binding_state: None,
            }),
    }
}

fn project_source_target(target: &ChannelSourceTarget) -> ZoneSourceTarget {
    match target {
        ChannelSourceTarget::Rpc { endpoint } => ZoneSourceTarget::Rpc {
            endpoint: endpoint.clone(),
        },
        ChannelSourceTarget::Module { module_id } => ZoneSourceTarget::Module {
            module_id: module_id.clone(),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfiguredZoneSource {
    pub source_id: String,
    pub label: Option<String>,
    pub target: ZoneSourceTarget,
    pub binding_state: Option<ZoneSourceBindingState>,
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
    pub binding_state: Option<ZoneSourceBindingState>,
    pub health: ZoneSourceHealth,
    pub reported_channel_id: Option<String>,
    pub head_block_id: Option<u64>,
    pub head_block_hash: Option<String>,
    pub head_parent_hash: Option<String>,
    pub observed_at_unix: Option<u64>,
    pub latency_millis: Option<u64>,
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
pub enum ZoneSourceBindingState {
    PersistedAttested,
    Pending,
    RuntimeAttested,
    ChannelMismatch,
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

fn project_catalog_record(
    snapshot: &CatalogSnapshot,
    record: &ZoneCatalogRecord,
    config: Option<&ChannelSourceConfig>,
    verification_state: CatalogVerificationState,
) -> ZoneSummary {
    let classification = classify_catalog_zone(
        snapshot,
        record,
        config.is_some_and(|config| !config.sequencer_sources.is_empty()),
    );
    let l1_channel = project_l1_channel(snapshot, record, classification.fact_gates);
    let facts = match classification.kind {
        ZoneKind::SequencerZone => ZoneFacts::SequencerZone {
            l2_zone: project_l2_zone(config),
            sequencer_committee: authoritative_committee(record, classification.fact_gates),
        },
        ZoneKind::DataChannel => ZoneFacts::DataChannel {
            raw_activity: project_raw_activity(snapshot, record),
        },
        ZoneKind::Unknown => ZoneFacts::Unknown,
    };
    ZoneSummary {
        channel_id: record.channel_id.clone(),
        display: project_display(&record.channel_id, record.observed_label.as_deref()),
        settlement_link: project_settlement_link(classification.kind, config),
        activity_state: project_activity_state(classification.kind, config, &l1_channel),
        activity_detail: project_activity_detail(
            classification.kind,
            config,
            Some(record),
            &l1_channel,
        ),
        provenance: project_provenance(snapshot, verification_state, Some(record.updated_at_unix)),
        l1_channel,
        facts,
    }
}

fn project_configured_channel(
    snapshot: &CatalogSnapshot,
    config: &ChannelSourceConfig,
    verification_state: CatalogVerificationState,
) -> ZoneSummary {
    let kind = if config.sequencer_sources.is_empty() {
        ZoneKind::Unknown
    } else {
        ZoneKind::SequencerZone
    };
    let l1_channel = L1ChannelSummary {
        tip_slot: None,
        tip_hash: None,
        lib_slot: observed_lib_slot(snapshot),
        balance: None,
        key_count: None,
        withdraw_threshold: None,
        operation_count: 0,
        finality_state: L1FinalityState::Unknown,
    };
    let facts = if kind == ZoneKind::SequencerZone {
        ZoneFacts::SequencerZone {
            l2_zone: project_l2_zone(Some(config)),
            sequencer_committee: empty_committee(),
        }
    } else {
        ZoneFacts::Unknown
    };
    ZoneSummary {
        channel_id: config.channel_id.clone(),
        display: project_display(&config.channel_id, None),
        settlement_link: project_settlement_link(kind, Some(config)),
        activity_state: project_activity_state(kind, Some(config), &l1_channel),
        activity_detail: project_activity_detail(kind, Some(config), None, &l1_channel),
        provenance: project_provenance(snapshot, verification_state, None),
        l1_channel,
        facts,
    }
}

fn project_l1_channel(
    snapshot: &CatalogSnapshot,
    record: &ZoneCatalogRecord,
    fact_gates: ZoneFactGates,
) -> L1ChannelSummary {
    let snapshot_is_authoritative = match record.snapshot_provenance.origin {
        CatalogSnapshotOrigin::ReplayDerived => fact_gates.replay_facts,
        CatalogSnapshotOrigin::PointSnapshot | CatalogSnapshotOrigin::FullConfiguration => {
            fact_gates.point_snapshot_facts
        }
    };
    if snapshot_is_authoritative {
        return record.l1_channel.clone();
    }
    L1ChannelSummary {
        tip_slot: None,
        tip_hash: None,
        lib_slot: observed_lib_slot(snapshot),
        balance: None,
        key_count: None,
        withdraw_threshold: None,
        operation_count: record.classification.channel_operations,
        finality_state: if fact_gates.presence_facts {
            L1FinalityState::Final
        } else {
            L1FinalityState::Unknown
        },
    }
}

fn authoritative_committee(
    record: &ZoneCatalogRecord,
    fact_gates: ZoneFactGates,
) -> SequencerCommitteeSummary {
    if fact_gates.point_snapshot_facts || fact_gates.replay_facts {
        record
            .sequencer_committee
            .clone()
            .unwrap_or_else(empty_committee)
    } else {
        empty_committee()
    }
}

fn empty_committee() -> SequencerCommitteeSummary {
    SequencerCommitteeSummary {
        members: Vec::new(),
        active_member: None,
        observed_at_slot: None,
    }
}

fn project_l2_zone(config: Option<&ChannelSourceConfig>) -> L2ZoneSummary {
    let configured_source_count = config.map_or(0, |config| {
        u64::try_from(config.sequencer_sources.len()).unwrap_or(u64::MAX)
    });
    let selected_source_id = config.and_then(|config| config.selected_sequencer_source_id.clone());
    L2ZoneSummary {
        source_status: if configured_source_count == 0 {
            L2SourceStatus::Unconfigured
        } else {
            L2SourceStatus::Unknown
        },
        selected_source_id,
        configured_source_count,
        observed_source_count: 0,
        latest_block_id: None,
        latest_block_hash: None,
        safe_block_id: None,
        finalized_block_id: None,
        finality_state: L2FinalityState::Unknown,
        agreement_state: if configured_source_count == 0 {
            SequencerAgreementState::Unconfigured
        } else {
            SequencerAgreementState::Unobserved
        },
    }
}

fn project_raw_activity(
    snapshot: &CatalogSnapshot,
    record: &ZoneCatalogRecord,
) -> RawActivitySummary {
    let latest_slot = snapshot
        .evidence
        .iter()
        .filter(|reference| {
            reference.channel_id == record.channel_id
                && reference.evidence_kind == ZoneEvidenceKind::RawInscription
        })
        .map(|reference| reference.l1_slot)
        .max();
    RawActivitySummary {
        inscription_count: record.classification.raw_inscriptions,
        latest_slot,
        latest_payload_size: None,
        finality_state: L1FinalityState::Final,
    }
}

fn project_settlement_link(
    kind: ZoneKind,
    config: Option<&ChannelSourceConfig>,
) -> SettlementLinkSummary {
    if kind == ZoneKind::DataChannel {
        return SettlementLinkSummary {
            status: SettlementLinkStatus::RawData,
            source: SettlementLinkSource::L1Scan,
            selected_sequencer_source_id: None,
            indexer_source_id: None,
            lag_blocks: None,
            lag_slots: None,
        };
    }
    let has_sequencer_sources = config.is_some_and(|config| !config.sequencer_sources.is_empty());
    let selected_sequencer_source_id =
        config.and_then(|config| config.selected_sequencer_source_id.clone());
    let indexer_source_id = config.and_then(|config| {
        config
            .indexer_source
            .as_ref()
            .map(|source| source.source_id.clone())
    });
    let (status, source) = match kind {
        ZoneKind::SequencerZone if !has_sequencer_sources => {
            (SettlementLinkStatus::L1Only, SettlementLinkSource::L1Scan)
        }
        ZoneKind::SequencerZone if selected_sequencer_source_id.is_some() => (
            SettlementLinkStatus::Linked,
            SettlementLinkSource::Configured,
        ),
        ZoneKind::SequencerZone => (
            SettlementLinkStatus::Unconfigured,
            SettlementLinkSource::Configured,
        ),
        ZoneKind::Unknown => (
            SettlementLinkStatus::Unknown,
            if config.is_some() {
                SettlementLinkSource::Configured
            } else {
                SettlementLinkSource::None
            },
        ),
        ZoneKind::DataChannel => (SettlementLinkStatus::RawData, SettlementLinkSource::L1Scan),
    };
    SettlementLinkSummary {
        status,
        source,
        selected_sequencer_source_id,
        indexer_source_id,
        lag_blocks: None,
        lag_slots: None,
    }
}

fn project_activity_state(
    kind: ZoneKind,
    config: Option<&ChannelSourceConfig>,
    l1_channel: &L1ChannelSummary,
) -> ZoneActivityState {
    if l1_channel.finality_state == L1FinalityState::Finalizing {
        return ZoneActivityState::Finalizing;
    }
    match kind {
        ZoneKind::DataChannel => ZoneActivityState::Raw,
        ZoneKind::SequencerZone
            if config.is_none_or(|config| config.sequencer_sources.is_empty()) =>
        {
            ZoneActivityState::Degraded
        }
        ZoneKind::SequencerZone | ZoneKind::Unknown => ZoneActivityState::Unknown,
    }
}

fn project_activity_detail(
    kind: ZoneKind,
    config: Option<&ChannelSourceConfig>,
    record: Option<&ZoneCatalogRecord>,
    l1_channel: &L1ChannelSummary,
) -> ZoneActivityDetail {
    let reason = if l1_channel.finality_state == L1FinalityState::Finalizing {
        "L1 Channel evidence is still finalizing."
    } else {
        match kind {
            ZoneKind::DataChannel => {
                "Finalized raw L1 inscriptions have no recognized L2 block evidence."
            }
            ZoneKind::SequencerZone
                if config.is_none_or(|config| config.sequencer_sources.is_empty()) =>
            {
                "L1 settlement evidence exists; no Sequencer source is configured."
            }
            ZoneKind::SequencerZone if record.is_none() => {
                "Configured Channel has not been discovered in finalized L1 evidence."
            }
            ZoneKind::SequencerZone => "Sequencer source observations are pending.",
            ZoneKind::Unknown if record.is_none() => {
                "Configured Channel has not been discovered in finalized L1 evidence."
            }
            ZoneKind::Unknown => "Available L1 evidence does not establish a Zone kind.",
        }
    };
    ZoneActivityDetail {
        reason: reason.to_owned(),
        last_seen_unix: record.map(|record| record.updated_at_unix),
        last_l1_slot: record.map(|record| record.last_seen_slot),
        last_l2_block_id: None,
    }
}

fn project_display(channel_id: &str, alias: Option<&str>) -> ZoneDisplay {
    let short_channel_id = short_channel_id(channel_id);
    ZoneDisplay {
        title: alias.unwrap_or(&short_channel_id).to_owned(),
        alias: alias.map(str::to_owned),
        short_channel_id,
        alias_source: if alias.is_some() {
            ZoneAliasSource::Configured
        } else {
            ZoneAliasSource::None
        },
    }
}

fn short_channel_id(channel_id: &str) -> String {
    if channel_id.len() <= 12 {
        return channel_id.to_owned();
    }
    let suffix_start = channel_id.len().saturating_sub(4);
    match (channel_id.get(..4), channel_id.get(suffix_start..)) {
        (Some(prefix), Some(suffix)) => format!("{prefix}...{suffix}"),
        _ => channel_id.to_owned(),
    }
}

fn project_provenance(
    snapshot: &CatalogSnapshot,
    verification_state: CatalogVerificationState,
    observed_at_unix: Option<u64>,
) -> ZoneProvenance {
    ZoneProvenance {
        network_scope: Some(snapshot.metadata.network_scope.clone()),
        verification_state,
        coverage: project_coverage(snapshot),
        observed_at_unix,
    }
}

fn project_coverage(snapshot: &CatalogSnapshot) -> ZoneCoverageProvenance {
    let Some(frontier) = snapshot.frontier.as_ref() else {
        return ZoneCoverageProvenance {
            status: CatalogCoverageStatus::Unknown,
            coverage_floor: None,
            scanned_through_slot: None,
            observed_lib_slot: None,
            prefix_status: CoveragePrefixStatus::Unknown,
            continuity_checkpoint: None,
        };
    };
    ZoneCoverageProvenance {
        status: frontier.coverage_status,
        coverage_floor: frontier.coverage_floor,
        scanned_through_slot: frontier.scanned_through_slot,
        observed_lib_slot: frontier.observed_lib.as_ref().map(|block| block.slot),
        prefix_status: frontier.prefix_status,
        continuity_checkpoint: frontier.checkpoint.as_ref().map(|checkpoint| {
            FinalizedBlockCheckpoint {
                slot: checkpoint.slot,
                block_id: checkpoint.block_id.clone(),
                parent_id: checkpoint.parent_id.clone(),
            }
        }),
    }
}

fn observed_lib_slot(snapshot: &CatalogSnapshot) -> Option<u64> {
    snapshot
        .frontier
        .as_ref()
        .and_then(|frontier| frontier.observed_lib.as_ref())
        .map(|block| block.slot)
}
