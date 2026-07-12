use std::fmt;

use serde::{Deserialize, Serialize};

use crate::inspection::zones::{
    CatalogCoverageStatus, CoveragePrefixStatus, L1ChannelSummary, NetworkScope,
    SequencerCommitteeSummary,
};

pub const CATALOG_SCHEMA_VERSION: u32 = 1;
pub const CATALOG_RECORD_VERSION: u32 = 1;

pub type CatalogResult<T> = Result<T, CatalogError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    Invalidated(CatalogInvalidation),
    RevisionConflict { expected: u64, current: u64 },
    InvalidInput(String),
    Storage(String),
}

impl CatalogError {
    #[must_use]
    pub fn invalidation(&self) -> Option<&CatalogInvalidation> {
        match self {
            Self::Invalidated(invalidation) => Some(invalidation),
            Self::RevisionConflict { .. } | Self::InvalidInput(_) | Self::Storage(_) => None,
        }
    }

    pub(crate) fn invalidated(
        reason: CatalogInvalidationReason,
        detail: impl Into<String>,
    ) -> Self {
        Self::Invalidated(CatalogInvalidation {
            reason,
            detail: detail.into(),
        })
    }

    pub(crate) fn invalid_input(detail: impl Into<String>) -> Self {
        Self::InvalidInput(detail.into())
    }

    pub(crate) fn storage(error: impl fmt::Display) -> Self {
        Self::Storage(error.to_string())
    }
}

impl fmt::Display for CatalogError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalidated(invalidation) => write!(
                formatter,
                "Zone Catalog invalidated ({:?}): {}",
                invalidation.reason, invalidation.detail
            ),
            Self::RevisionConflict { expected, current } => write!(
                formatter,
                "Zone Catalog revision conflict: expected {expected}, current {current}"
            ),
            Self::InvalidInput(detail) => write!(formatter, "invalid Zone Catalog input: {detail}"),
            Self::Storage(detail) => write!(formatter, "Zone Catalog storage error: {detail}"),
        }
    }
}

impl std::error::Error for CatalogError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogInvalidation {
    pub reason: CatalogInvalidationReason,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogInvalidationReason {
    DatabaseUnreadable,
    SchemaMissing,
    SchemaVersion,
    TableSchema,
    RecordVersion,
    RecordDecode,
    RecordInvariant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSchemaMetadata {
    pub schema_version: u32,
    pub record_version: u32,
}

impl CatalogSchemaMetadata {
    #[must_use]
    pub const fn current() -> Self {
        Self {
            schema_version: CATALOG_SCHEMA_VERSION,
            record_version: CATALOG_RECORD_VERSION,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogMetadata {
    pub catalog_file_id: String,
    pub network_scope: NetworkScope,
    pub identity_aliases: Vec<NetworkIdentityAlias>,
    pub identity_assurance: CatalogIdentityAssurance,
    pub identity_transition: Option<CatalogIdentityTransition>,
    pub catalog_revision: u64,
    pub created_at_unix: u64,
    pub updated_at_unix: u64,
}

impl CatalogMetadata {
    pub fn new(network_scope: NetworkScope, created_at_unix: u64) -> CatalogResult<Self> {
        let network_scope = canonical_network_scope(network_scope)?;
        let mut random = [0_u8; 16];
        getrandom::fill(&mut random).map_err(CatalogError::storage)?;
        let identity_assurance = match &network_scope {
            NetworkScope::GenesisId { .. } => CatalogIdentityAssurance::SourceAttested,
            NetworkScope::FinalizedAnchor { .. } => CatalogIdentityAssurance::Provisional,
        };
        Ok(Self {
            catalog_file_id: format!("catalog_{}", hex::encode(random)),
            network_scope,
            identity_aliases: Vec::new(),
            identity_assurance,
            identity_transition: None,
            catalog_revision: 0,
            created_at_unix,
            updated_at_unix: created_at_unix,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogIdentityAssurance {
    Provisional,
    SourceAttested,
    AncestryVerified,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkIdentityAlias {
    pub network_scope: NetworkScope,
    pub accepted_at_unix: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogIdentityTransition {
    pub old_scope: NetworkScope,
    pub new_scope: NetworkScope,
    pub anchor: CatalogBlockCheckpoint,
    pub checkpoint: CatalogBlockCheckpoint,
    pub source_revision: u64,
    pub stage: CatalogIdentityTransitionStage,
    pub prepared_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogIdentityTransitionStage {
    Prepared,
    SettingsRebound,
    Committed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogBlockReference {
    pub slot: u64,
    pub block_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogBlockCheckpoint {
    pub slot: u64,
    pub block_id: String,
    pub parent_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogFrontier {
    pub scanned_through_slot: Option<u64>,
    pub checkpoint: Option<CatalogBlockCheckpoint>,
    pub observed_lib: Option<CatalogBlockReference>,
    pub coverage_floor: Option<u64>,
    pub prefix_status: CoveragePrefixStatus,
    pub coverage_status: CatalogCoverageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogTraversal {
    pub target_lib: Option<CatalogBlockReference>,
    pub ingestion_cursor: Option<CatalogBlockReference>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageSegment {
    pub segment_id: String,
    pub floor: CatalogBlockCheckpoint,
    pub frontier: CatalogBlockReference,
    pub reaches_target_lib: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageGap {
    pub gap_id: String,
    pub lower_segment_id: String,
    pub lower_checkpoint: CatalogBlockReference,
    pub upper_segment_id: String,
    pub upper_block: CatalogBlockReference,
    pub required_parent_id: String,
    pub reason: CoverageGapReason,
    pub status: CoverageGapStatus,
    pub attempts: u32,
    pub first_seen_at_unix: u64,
    pub last_attempt_at_unix: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageGapReason {
    MissingParent,
    MissingBlockBody,
    ParentLinkMismatch,
    SourceUnavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoverageGapStatus {
    Pending,
    Repairing,
    Backoff,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneCatalogRecord {
    pub channel_id: String,
    pub observed_label: Option<String>,
    pub l1_channel: L1ChannelSummary,
    pub sequencer_committee: Option<SequencerCommitteeSummary>,
    pub classification: ZoneClassificationCounters,
    pub first_seen_slot: u64,
    pub last_seen_slot: u64,
    pub latest_evidence_id: String,
    pub evidence_count: u64,
    pub snapshot_provenance: CatalogSnapshotProvenance,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneClassificationCounters {
    pub channel_operations: u64,
    pub recognized_l2_blocks: u64,
    pub raw_inscriptions: u64,
    pub conflicting_evidence: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogSnapshotProvenance {
    pub origin: CatalogSnapshotOrigin,
    pub coverage_segment_id: String,
    pub observed_slot: u64,
    pub source_revision: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogSnapshotOrigin {
    ReplayDerived,
    PointSnapshot,
    FullConfiguration,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneEvidenceReference {
    pub evidence_id: String,
    pub channel_id: String,
    pub coverage_segment_id: String,
    pub l1_slot: u64,
    pub block_id: String,
    pub transaction_hash: Option<String>,
    pub operation_index: u32,
    pub message_id: Option<String>,
    pub evidence_kind: ZoneEvidenceKind,
    pub evidence_use: CatalogEvidenceUse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZoneEvidenceKind {
    ChannelCreated,
    ChannelConfiguration,
    ChannelOperation,
    SequencerBlock,
    RawInscription,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CatalogEvidenceUse {
    Presence,
    PointSnapshot,
    ReplayContribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogBatch {
    pub expected_catalog_revision: u64,
    pub updated_at_unix: u64,
    pub upsert_zones: Vec<ZoneCatalogRecord>,
    pub delete_zone_ids: Vec<String>,
    pub upsert_evidence: Vec<ZoneEvidenceReference>,
    pub delete_evidence_ids: Vec<String>,
    pub upsert_segments: Vec<CoverageSegment>,
    pub delete_segment_ids: Vec<String>,
    pub upsert_gaps: Vec<CoverageGap>,
    pub delete_gap_ids: Vec<String>,
    pub frontier: Option<CatalogFrontier>,
    pub traversal: Option<CatalogTraversal>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogSnapshot {
    pub metadata: CatalogMetadata,
    pub frontier: Option<CatalogFrontier>,
    pub traversal: Option<CatalogTraversal>,
    pub zones: Vec<ZoneCatalogRecord>,
    pub evidence: Vec<ZoneEvidenceReference>,
    pub segments: Vec<CoverageSegment>,
    pub gaps: Vec<CoverageGap>,
}

pub(crate) fn validate_metadata(metadata: &CatalogMetadata) -> CatalogResult<()> {
    validate_local_id(&metadata.catalog_file_id, "catalog file id")?;
    validate_network_scope(&metadata.network_scope)?;
    for alias in &metadata.identity_aliases {
        validate_network_scope(&alias.network_scope)?;
    }
    if let Some(transition) = metadata.identity_transition.as_ref() {
        validate_network_scope(&transition.old_scope)?;
        validate_network_scope(&transition.new_scope)?;
        validate_checkpoint(&transition.anchor)?;
        validate_checkpoint(&transition.checkpoint)?;
    }
    Ok(())
}

pub(crate) fn validate_frontier(frontier: &CatalogFrontier) -> CatalogResult<()> {
    if let Some(checkpoint) = frontier.checkpoint.as_ref() {
        validate_checkpoint(checkpoint)?;
        if frontier.scanned_through_slot != Some(checkpoint.slot) {
            return Err(CatalogError::invalid_input(
                "catalog checkpoint must match scanned-through slot",
            ));
        }
    } else if frontier.scanned_through_slot.is_some() {
        return Err(CatalogError::invalid_input(
            "scanned catalog frontier requires an actual block checkpoint",
        ));
    }
    if let Some(observed_lib) = frontier.observed_lib.as_ref() {
        validate_block_reference(observed_lib)?;
        if frontier
            .scanned_through_slot
            .is_some_and(|slot| slot > observed_lib.slot)
        {
            return Err(CatalogError::invalid_input(
                "catalog frontier is beyond observed LIB",
            ));
        }
    }
    if frontier.coverage_floor.is_some_and(|floor| {
        frontier
            .scanned_through_slot
            .is_none_or(|scanned| floor > scanned)
    }) {
        return Err(CatalogError::invalid_input(
            "catalog coverage floor is beyond its frontier",
        ));
    }
    Ok(())
}

pub(crate) fn validate_traversal(traversal: &CatalogTraversal) -> CatalogResult<()> {
    if let Some(target) = traversal.target_lib.as_ref() {
        validate_block_reference(target)?;
    }
    if let Some(cursor) = traversal.ingestion_cursor.as_ref() {
        validate_block_reference(cursor)?;
        if traversal
            .target_lib
            .as_ref()
            .is_some_and(|target| cursor.slot > target.slot)
        {
            return Err(CatalogError::invalid_input(
                "catalog ingestion cursor is beyond target LIB",
            ));
        }
    }
    Ok(())
}

pub(crate) fn validate_segment(segment: &CoverageSegment) -> CatalogResult<()> {
    validate_local_id(&segment.segment_id, "coverage segment id")?;
    validate_checkpoint(&segment.floor)?;
    validate_block_reference(&segment.frontier)?;
    if segment.frontier.slot < segment.floor.slot {
        return Err(CatalogError::invalid_input(
            "coverage segment frontier precedes its floor",
        ));
    }
    if segment.frontier.slot == segment.floor.slot
        && segment.frontier.block_id != segment.floor.block_id
    {
        return Err(CatalogError::invalid_input(
            "single-block coverage segment has conflicting block ids",
        ));
    }
    Ok(())
}

pub(crate) fn validate_gap(gap: &CoverageGap) -> CatalogResult<()> {
    validate_local_id(&gap.gap_id, "coverage gap id")?;
    validate_local_id(&gap.lower_segment_id, "lower coverage segment id")?;
    validate_local_id(&gap.upper_segment_id, "upper coverage segment id")?;
    validate_block_reference(&gap.lower_checkpoint)?;
    validate_block_reference(&gap.upper_block)?;
    validate_hex_id(&gap.required_parent_id, "required parent id")?;
    if gap.lower_segment_id == gap.upper_segment_id {
        return Err(CatalogError::invalid_input(
            "coverage gap must join distinct segments",
        ));
    }
    if gap.upper_block.slot <= gap.lower_checkpoint.slot {
        return Err(CatalogError::invalid_input(
            "coverage gap upper block must follow its lower checkpoint",
        ));
    }
    Ok(())
}

pub(crate) fn validate_zone(record: &ZoneCatalogRecord) -> CatalogResult<()> {
    validate_hex_id(&record.channel_id, "Channel id")?;
    if record.observed_label.as_ref().is_some_and(|label| {
        label.is_empty() || label.len() > 256 || label.chars().any(char::is_control)
    }) {
        return Err(CatalogError::invalid_input("Zone label is invalid"));
    }
    if let Some(tip_hash) = record.l1_channel.tip_hash.as_ref() {
        validate_hex_id(tip_hash, "Channel tip hash")?;
    }
    if let Some(committee) = record.sequencer_committee.as_ref() {
        let mut members = std::collections::HashSet::new();
        for member in &committee.members {
            validate_local_id(member, "Sequencer committee member")?;
            if !members.insert(member) {
                return Err(CatalogError::invalid_input(
                    "Sequencer committee contains duplicate members",
                ));
            }
        }
        if committee
            .active_member
            .as_ref()
            .is_some_and(|active| !members.contains(active))
        {
            return Err(CatalogError::invalid_input(
                "active Sequencer committee member is not accredited",
            ));
        }
    }
    validate_local_id(&record.latest_evidence_id, "latest evidence id")?;
    validate_local_id(
        &record.snapshot_provenance.coverage_segment_id,
        "snapshot coverage segment id",
    )?;
    if record.last_seen_slot < record.first_seen_slot {
        return Err(CatalogError::invalid_input(
            "Zone last-seen slot precedes first-seen slot",
        ));
    }
    if record.evidence_count == 0 {
        return Err(CatalogError::invalid_input(
            "Zone record must reference evidence",
        ));
    }
    Ok(())
}

pub(crate) fn validate_evidence(reference: &ZoneEvidenceReference) -> CatalogResult<()> {
    validate_local_id(&reference.evidence_id, "evidence id")?;
    validate_hex_id(&reference.channel_id, "evidence Channel id")?;
    validate_local_id(
        &reference.coverage_segment_id,
        "evidence coverage segment id",
    )?;
    validate_hex_id(&reference.block_id, "evidence block id")?;
    if let Some(transaction_hash) = reference.transaction_hash.as_ref() {
        validate_hex_id(transaction_hash, "evidence transaction hash")?;
    }
    if reference
        .message_id
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.chars().any(char::is_control))
    {
        return Err(CatalogError::invalid_input(
            "evidence message id is invalid",
        ));
    }
    Ok(())
}

pub(crate) fn validate_network_scope(scope: &NetworkScope) -> CatalogResult<()> {
    match scope {
        NetworkScope::GenesisId { genesis_id } => validate_hex_id(genesis_id, "genesis id"),
        NetworkScope::FinalizedAnchor {
            genesis_time,
            block_id,
            parent_id,
            ..
        } => {
            if genesis_time.is_empty() || genesis_time.chars().any(char::is_control) {
                return Err(CatalogError::invalid_input(
                    "finalized-anchor genesis time is invalid",
                ));
            }
            validate_hex_id(block_id, "finalized-anchor block id")?;
            validate_hex_id(parent_id, "finalized-anchor parent id")
        }
    }
}

fn canonical_network_scope(scope: NetworkScope) -> CatalogResult<NetworkScope> {
    let scope = match scope {
        NetworkScope::GenesisId { genesis_id } => NetworkScope::GenesisId {
            genesis_id: canonical_hex_id(&genesis_id, "genesis id")?,
        },
        NetworkScope::FinalizedAnchor {
            genesis_time,
            block_slot,
            block_id,
            parent_id,
        } => {
            let genesis_time = genesis_time.trim();
            if genesis_time.is_empty() || genesis_time.chars().any(char::is_control) {
                return Err(CatalogError::invalid_input(
                    "finalized-anchor genesis time is invalid",
                ));
            }
            NetworkScope::FinalizedAnchor {
                genesis_time: genesis_time.to_owned(),
                block_slot,
                block_id: canonical_hex_id(&block_id, "finalized-anchor block id")?,
                parent_id: canonical_hex_id(&parent_id, "finalized-anchor parent id")?,
            }
        }
    };
    Ok(scope)
}

fn validate_checkpoint(checkpoint: &CatalogBlockCheckpoint) -> CatalogResult<()> {
    validate_hex_id(&checkpoint.block_id, "checkpoint block id")?;
    validate_hex_id(&checkpoint.parent_id, "checkpoint parent id")
}

fn validate_block_reference(reference: &CatalogBlockReference) -> CatalogResult<()> {
    validate_hex_id(&reference.block_id, "block id")
}

pub(crate) fn validate_hex_id(value: &str, label: &str) -> CatalogResult<()> {
    if value.len() != 64
        || !value
            .chars()
            .all(|character| character.is_ascii_digit() || ('a'..='f').contains(&character))
    {
        return Err(CatalogError::invalid_input(format!(
            "{label} must be canonical 32-byte hexadecimal text"
        )));
    }
    Ok(())
}

fn canonical_hex_id(value: &str, label: &str) -> CatalogResult<String> {
    let value = value.trim();
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.len() != 64 || !value.chars().all(|character| character.is_ascii_hexdigit()) {
        return Err(CatalogError::invalid_input(format!(
            "{label} must be 32-byte hexadecimal text"
        )));
    }
    Ok(value.to_ascii_lowercase())
}

pub(crate) fn validate_local_id(value: &str, label: &str) -> CatalogResult<()> {
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(CatalogError::invalid_input(format!("{label} is invalid")));
    }
    Ok(())
}
