use std::{
    collections::{BTreeMap, BTreeSet, HashSet},
    fmt,
    num::NonZeroUsize,
};

use common::block::{Block as SequencerBlock, HashableBlockData};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use super::{
    model::{
        CatalogBatch, CatalogBlockCheckpoint, CatalogBlockReference, CatalogEvidenceUse,
        CatalogFrontier, CatalogSnapshot, CatalogSnapshotOrigin, CatalogTraversal, CoverageGap,
        CoverageGapReason, CoverageGapStatus, CoverageSegment, ZoneCatalogRecord,
        ZoneClassificationCounters, ZoneEvidenceKind, ZoneEvidenceReference, validate_hex_id,
    },
    source::{
        CatalogL1Block, CatalogL1BlockEvent, CatalogL1RangePage, CatalogL1Source,
        CatalogL1SourceError,
    },
};
use crate::inspection::zones::{
    CatalogCoverageStatus, CoveragePrefixStatus, L1ChannelSummary, L1FinalityState, NetworkScope,
    SequencerCommitteeSummary,
};

pub const DEFAULT_CATALOG_REPAIR_BLOCKS: usize = 64;
pub const MAX_CATALOG_REPAIR_BLOCKS: usize = 512;

pub type CatalogEngineResult<T> = Result<T, CatalogEngineError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogEngineError {
    InvalidState(String),
    InvalidBlock(String),
    SourceInconsistent(String),
    Overflow(String),
}

impl fmt::Display for CatalogEngineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidState(detail) => {
                write!(formatter, "invalid catalog engine state: {detail}")
            }
            Self::InvalidBlock(detail) => write!(formatter, "invalid finalized block: {detail}"),
            Self::SourceInconsistent(detail) => {
                write!(formatter, "catalog L1 source is inconsistent: {detail}")
            }
            Self::Overflow(detail) => write!(formatter, "catalog engine overflow: {detail}"),
        }
    }
}

impl std::error::Error for CatalogEngineError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogEngineContext {
    pub source_revision: u64,
    pub updated_at_unix: u64,
    repair_block_limit: NonZeroUsize,
}

impl CatalogEngineContext {
    pub fn new(source_revision: u64, updated_at_unix: u64) -> CatalogEngineResult<Self> {
        Self::with_repair_block_limit(
            source_revision,
            updated_at_unix,
            DEFAULT_CATALOG_REPAIR_BLOCKS,
        )
    }

    pub fn with_repair_block_limit(
        source_revision: u64,
        updated_at_unix: u64,
        repair_block_limit: usize,
    ) -> CatalogEngineResult<Self> {
        let repair_block_limit = NonZeroUsize::new(repair_block_limit).ok_or_else(|| {
            CatalogEngineError::InvalidState(
                "repair block limit must be greater than zero".to_owned(),
            )
        })?;
        if repair_block_limit.get() > MAX_CATALOG_REPAIR_BLOCKS {
            return Err(CatalogEngineError::InvalidState(format!(
                "repair block limit exceeds {MAX_CATALOG_REPAIR_BLOCKS}"
            )));
        }
        Ok(Self {
            source_revision,
            updated_at_unix,
            repair_block_limit,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogAncestryRepairRequest {
    pub lower_checkpoint: Option<CatalogBlockReference>,
    pub upper_checkpoint: CatalogBlockCheckpoint,
    pub expected_genesis_id: Option<String>,
    max_blocks: NonZeroUsize,
}

impl CatalogAncestryRepairRequest {
    #[must_use]
    pub const fn max_blocks(&self) -> NonZeroUsize {
        self.max_blocks
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogAncestryRepairOutcome {
    Connected {
        recovered_blocks: Vec<CatalogL1Block>,
    },
    Unresolved {
        recovered_blocks: Vec<CatalogL1Block>,
        missing_block_id: String,
        reason: CoverageGapReason,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum CatalogPageReduction {
    NoProgress,
    Commit {
        batch: Box<CatalogBatch>,
        remaining_events: Vec<CatalogL1BlockEvent>,
    },
    RepairRequired {
        request: CatalogAncestryRepairRequest,
        pending_events: Vec<CatalogL1BlockEvent>,
    },
    GapConfirmationRequired {
        request: CatalogAncestryRepairRequest,
        pending_events: Vec<CatalogL1BlockEvent>,
        outcome: Box<CatalogAncestryRepairOutcome>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogRepairConfirmation {
    pub target_lib: CatalogBlockReference,
    pub lower_checkpoint: Option<CatalogBlockReference>,
    pub upper_frontier_checkpoint: Option<CatalogBlockCheckpoint>,
}

impl CatalogRepairConfirmation {
    #[must_use]
    pub const fn new(
        target_lib: CatalogBlockReference,
        lower_checkpoint: Option<CatalogBlockReference>,
        upper_frontier_checkpoint: Option<CatalogBlockCheckpoint>,
    ) -> Self {
        Self {
            target_lib,
            lower_checkpoint,
            upper_frontier_checkpoint,
        }
    }
}

pub fn prepare_catalog_catch_up(
    snapshot: &CatalogSnapshot,
    target_lib: CatalogBlockReference,
    context: CatalogEngineContext,
) -> CatalogEngineResult<Option<CatalogBatch>> {
    validate_context(snapshot, context)?;
    validate_reference(&target_lib, "target LIB")?;

    let mut working = WorkingCatalog::from_snapshot(snapshot);
    if let Some(cursor) = working
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.ingestion_cursor.as_ref())
        && cursor.slot > target_lib.slot
    {
        return Err(CatalogEngineError::SourceInconsistent(format!(
            "target LIB slot {} is behind ingestion cursor slot {}",
            target_lib.slot, cursor.slot
        )));
    }
    if let Some(previous_target) = working
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.target_lib.as_ref())
    {
        if previous_target.slot > target_lib.slot {
            return Err(CatalogEngineError::SourceInconsistent(format!(
                "target LIB moved backward from slot {} to {}",
                previous_target.slot, target_lib.slot
            )));
        }
        if previous_target.slot == target_lib.slot
            && previous_target.block_id != target_lib.block_id
        {
            return Err(CatalogEngineError::SourceInconsistent(
                "target LIB id changed at the same slot".to_owned(),
            ));
        }
    }

    let cursor = working
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.ingestion_cursor.clone());
    working.traversal = Some(CatalogTraversal {
        target_lib: Some(target_lib.clone()),
        ingestion_cursor: cursor,
    });
    let frontier = working.frontier.get_or_insert(CatalogFrontier {
        scanned_through_slot: None,
        checkpoint: None,
        observed_lib: None,
        coverage_floor: None,
        prefix_status: CoveragePrefixStatus::Unknown,
        coverage_status: CatalogCoverageStatus::Rebuilding,
    });
    frontier.observed_lib = Some(target_lib.clone());

    for segment in working.segments.values_mut() {
        segment.reaches_target_lib = segment.frontier == target_lib;
    }
    for zone in working.zones.values_mut() {
        if zone.l1_channel.lib_slot != Some(target_lib.slot) {
            zone.l1_channel.lib_slot = Some(target_lib.slot);
            zone.updated_at_unix = context.updated_at_unix;
        }
    }
    recompute_coverage(&mut working)?;
    working.into_batch(snapshot, context)
}

pub fn reduce_catalog_page(
    snapshot: &CatalogSnapshot,
    page: CatalogL1RangePage,
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogPageReduction> {
    validate_context(snapshot, context)?;
    if page.events.is_empty() {
        return Ok(CatalogPageReduction::NoProgress);
    }
    validate_page_snapshots(&page.events)?;
    let target = current_target(snapshot)?.clone();
    let first = page.events.first().ok_or_else(|| {
        CatalogEngineError::InvalidState("non-empty page lost first event".into())
    })?;
    for event in &page.events {
        validate_event_against_target(event, &target)?;
    }
    validate_strictly_ascending_slots(&page.events)?;

    let cursor = snapshot
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.ingestion_cursor.as_ref());
    match cursor {
        Some(cursor) => {
            active_segment_id(snapshot, cursor)?;
            if first.block.checkpoint.slot <= cursor.slot {
                return Err(CatalogEngineError::SourceInconsistent(format!(
                    "page starts at slot {}, not after cursor slot {}",
                    first.block.checkpoint.slot, cursor.slot
                )));
            }
            if first.block.checkpoint.parent_id != cursor.block_id {
                return Ok(CatalogPageReduction::RepairRequired {
                    request: repair_request(
                        Some(cursor.clone()),
                        first.block.checkpoint.clone(),
                        snapshot,
                        context,
                    ),
                    pending_events: page.events,
                });
            }
        }
        None => {
            if !snapshot.segments.is_empty() || !snapshot.gaps.is_empty() {
                return Err(CatalogEngineError::InvalidState(
                    "catalog has coverage records without an ingestion cursor".to_owned(),
                ));
            }
            if first.block.checkpoint.slot != 0 {
                return Ok(CatalogPageReduction::RepairRequired {
                    request: repair_request(
                        None,
                        first.block.checkpoint.clone(),
                        snapshot,
                        context,
                    ),
                    pending_events: page.events,
                });
            }
            validate_genesis_block(snapshot, &first.block.checkpoint)?;
        }
    }

    let (accepted, remaining) = split_connected_events(page.events)?;
    let batch = apply_connected_events(snapshot, &accepted, context)?;
    Ok(CatalogPageReduction::Commit {
        batch: Box::new(batch),
        remaining_events: remaining,
    })
}

pub async fn repair_catalog_ancestry(
    source: &dyn CatalogL1Source,
    request: &CatalogAncestryRepairRequest,
) -> CatalogEngineResult<CatalogAncestryRepairOutcome> {
    validate_repair_request(request)?;

    let mut requested_id = request.upper_checkpoint.parent_id.clone();
    let mut child_slot = request.upper_checkpoint.slot;
    let mut recovered_descending = Vec::new();
    let mut visited = HashSet::new();

    loop {
        if request
            .lower_checkpoint
            .as_ref()
            .is_some_and(|lower| lower.block_id == requested_id)
        {
            recovered_descending.reverse();
            return Ok(CatalogAncestryRepairOutcome::Connected {
                recovered_blocks: recovered_descending,
            });
        }
        if recovered_descending.len() >= request.max_blocks().get() {
            recovered_descending.reverse();
            return Ok(CatalogAncestryRepairOutcome::Unresolved {
                recovered_blocks: recovered_descending,
                missing_block_id: requested_id,
                reason: CoverageGapReason::MissingParent,
            });
        }
        if !visited.insert(requested_id.clone()) {
            return Err(CatalogEngineError::SourceInconsistent(format!(
                "repair ancestry cycles at block {requested_id}"
            )));
        }

        let block = match source.block(requested_id.clone()).await {
            Ok(Some(block)) => block,
            Ok(None) => {
                recovered_descending.reverse();
                return Ok(CatalogAncestryRepairOutcome::Unresolved {
                    recovered_blocks: recovered_descending,
                    missing_block_id: requested_id,
                    reason: CoverageGapReason::MissingBlockBody,
                });
            }
            Err(CatalogL1SourceError::Unavailable(_)) => {
                recovered_descending.reverse();
                return Ok(CatalogAncestryRepairOutcome::Unresolved {
                    recovered_blocks: recovered_descending,
                    missing_block_id: requested_id,
                    reason: CoverageGapReason::SourceUnavailable,
                });
            }
            Err(
                error @ (CatalogL1SourceError::InvalidRequest(_)
                | CatalogL1SourceError::InvalidResponse(_)),
            ) => {
                return Err(CatalogEngineError::SourceInconsistent(error.to_string()));
            }
        };
        validate_checkpoint(&block.checkpoint, "repair block").map_err(|error| {
            CatalogEngineError::SourceInconsistent(format!(
                "repair returned an invalid checkpoint: {error}"
            ))
        })?;
        if block.checkpoint.block_id != requested_id {
            return Err(CatalogEngineError::SourceInconsistent(format!(
                "repair returned block {}, requested {requested_id}",
                block.checkpoint.block_id
            )));
        }
        if block.checkpoint.slot >= child_slot {
            return Err(CatalogEngineError::SourceInconsistent(format!(
                "repair block {} at slot {} does not precede child slot {child_slot}",
                block.checkpoint.block_id, block.checkpoint.slot
            )));
        }
        if let Some(lower) = request.lower_checkpoint.as_ref()
            && block.checkpoint.slot <= lower.slot
        {
            return Err(CatalogEngineError::SourceInconsistent(format!(
                "repair crossed lower checkpoint slot {} without reaching block {}",
                lower.slot, lower.block_id
            )));
        }

        let reached_genesis = request.lower_checkpoint.is_none() && block.checkpoint.slot == 0;
        if reached_genesis
            && request
                .expected_genesis_id
                .as_ref()
                .is_some_and(|expected| expected != &block.checkpoint.block_id)
        {
            return Err(CatalogEngineError::SourceInconsistent(
                "repair reached a conflicting genesis block".to_owned(),
            ));
        }

        requested_id = block.checkpoint.parent_id.clone();
        child_slot = block.checkpoint.slot;
        recovered_descending.push(block);
        if reached_genesis {
            recovered_descending.reverse();
            return Ok(CatalogAncestryRepairOutcome::Connected {
                recovered_blocks: recovered_descending,
            });
        }
    }
}

pub fn reduce_catalog_repair(
    snapshot: &CatalogSnapshot,
    request: &CatalogAncestryRepairRequest,
    pending_events: Vec<CatalogL1BlockEvent>,
    outcome: CatalogAncestryRepairOutcome,
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogPageReduction> {
    validate_context(snapshot, context)?;
    let pending_first = pending_events.first().ok_or_else(|| {
        CatalogEngineError::InvalidState("repair continuation has no pending upper block".into())
    })?;
    if pending_first.block.checkpoint != request.upper_checkpoint {
        return Err(CatalogEngineError::InvalidState(
            "repair continuation upper block changed".to_owned(),
        ));
    }
    let snapshot_state = pending_first.snapshot.clone();

    match outcome {
        CatalogAncestryRepairOutcome::Connected { recovered_blocks } => {
            validate_connected_repair(request, &recovered_blocks)?;
            let events = prepend_recovered_events(recovered_blocks, pending_events, snapshot_state);
            reduce_catalog_page(snapshot, CatalogL1RangePage { events }, context)
        }
        CatalogAncestryRepairOutcome::Unresolved {
            recovered_blocks,
            missing_block_id,
            reason,
        } => {
            validate_unresolved_repair(
                request,
                &recovered_blocks,
                &missing_block_id,
                &pending_first.block.checkpoint,
            )?;
            Ok(CatalogPageReduction::GapConfirmationRequired {
                request: request.clone(),
                pending_events,
                outcome: Box::new(CatalogAncestryRepairOutcome::Unresolved {
                    recovered_blocks,
                    missing_block_id,
                    reason,
                }),
            })
        }
    }
}

pub fn confirm_catalog_repair_gap(
    snapshot: &CatalogSnapshot,
    request: &CatalogAncestryRepairRequest,
    pending_events: Vec<CatalogL1BlockEvent>,
    outcome: CatalogAncestryRepairOutcome,
    confirmation: &CatalogRepairConfirmation,
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogPageReduction> {
    validate_context(snapshot, context)?;
    validate_repair_confirmation(
        snapshot,
        request.lower_checkpoint.as_ref(),
        confirmation,
        false,
    )?;
    let pending_first = pending_events.first().ok_or_else(|| {
        CatalogEngineError::InvalidState("gap confirmation has no pending upper block".into())
    })?;
    if pending_first.block.checkpoint != request.upper_checkpoint {
        return Err(CatalogEngineError::InvalidState(
            "gap confirmation upper block changed".to_owned(),
        ));
    }
    let snapshot_state = pending_first.snapshot.clone();
    let CatalogAncestryRepairOutcome::Unresolved {
        recovered_blocks,
        missing_block_id,
        reason,
    } = outcome
    else {
        return Err(CatalogEngineError::InvalidState(
            "gap confirmation requires unresolved repair".to_owned(),
        ));
    };
    validate_unresolved_repair(
        request,
        &recovered_blocks,
        &missing_block_id,
        &pending_first.block.checkpoint,
    )?;
    let events = prepend_recovered_events(recovered_blocks, pending_events, snapshot_state);
    open_disconnected_segment(snapshot, events, missing_block_id, reason, context)
}

pub fn catalog_gap_repair_request(
    snapshot: &CatalogSnapshot,
    gap_id: &str,
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogAncestryRepairRequest> {
    validate_context(snapshot, context)?;
    let gap = snapshot
        .gaps
        .iter()
        .find(|gap| gap.gap_id == gap_id)
        .ok_or_else(|| CatalogEngineError::InvalidState(format!("unknown gap {gap_id}")))?;
    let upper = snapshot
        .segments
        .iter()
        .find(|segment| segment.segment_id == gap.upper_segment_id)
        .ok_or_else(|| {
            CatalogEngineError::InvalidState(format!(
                "gap {} has no upper segment {}",
                gap.gap_id, gap.upper_segment_id
            ))
        })?;
    Ok(CatalogAncestryRepairRequest {
        lower_checkpoint: Some(gap.lower_checkpoint.clone()),
        upper_checkpoint: upper.floor.clone(),
        expected_genesis_id: None,
        max_blocks: context.repair_block_limit,
    })
}

pub fn catalog_prefix_repair_request(
    snapshot: &CatalogSnapshot,
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogAncestryRepairRequest> {
    validate_context(snapshot, context)?;
    let frontier = snapshot.frontier.as_ref().ok_or_else(|| {
        CatalogEngineError::InvalidState("catalog frontier is missing".to_owned())
    })?;
    if frontier.prefix_status != CoveragePrefixStatus::Unavailable {
        return Err(CatalogEngineError::InvalidState(
            "catalog prefix is not marked unavailable".to_owned(),
        ));
    }
    let floor = frontier.coverage_floor.ok_or_else(|| {
        CatalogEngineError::InvalidState("unavailable prefix has no coverage floor".to_owned())
    })?;
    let segment = snapshot
        .segments
        .iter()
        .find(|segment| segment.floor.slot == floor)
        .ok_or_else(|| {
            CatalogEngineError::InvalidState(
                "unavailable prefix has no matching first segment".to_owned(),
            )
        })?;
    Ok(CatalogAncestryRepairRequest {
        lower_checkpoint: None,
        upper_checkpoint: segment.floor.clone(),
        expected_genesis_id: match &snapshot.metadata.network_scope {
            NetworkScope::GenesisId { genesis_id } => Some(genesis_id.clone()),
            NetworkScope::FinalizedAnchor { .. } => None,
        },
        max_blocks: context.repair_block_limit,
    })
}

pub fn reduce_catalog_prefix_repair(
    snapshot: &CatalogSnapshot,
    outcome: CatalogAncestryRepairOutcome,
    confirmation: &CatalogRepairConfirmation,
    context: CatalogEngineContext,
) -> CatalogEngineResult<Option<CatalogBatch>> {
    validate_context(snapshot, context)?;
    validate_repair_confirmation(snapshot, None, confirmation, true)?;
    let request = catalog_prefix_repair_request(snapshot, context)?;
    let mut working = WorkingCatalog::from_snapshot(snapshot);
    let segment_id = working
        .segments
        .values()
        .find(|segment| segment.floor == request.upper_checkpoint)
        .map(|segment| segment.segment_id.clone())
        .ok_or_else(|| CatalogEngineError::InvalidState("prefix segment disappeared".to_owned()))?;

    match outcome {
        CatalogAncestryRepairOutcome::Connected { recovered_blocks } => {
            validate_connected_repair(&request, &recovered_blocks)?;
            let oldest = recovered_blocks.first().ok_or_else(|| {
                CatalogEngineError::InvalidState(
                    "connected prefix repair has no genesis block".to_owned(),
                )
            })?;
            ingest_blocks(&mut working, &recovered_blocks, &segment_id, context)?;
            let segment = working.segments.get_mut(&segment_id).ok_or_else(|| {
                CatalogEngineError::InvalidState("prefix segment is missing".to_owned())
            })?;
            segment.floor = oldest.checkpoint.clone();
            let frontier = working.frontier.as_mut().ok_or_else(|| {
                CatalogEngineError::InvalidState("catalog frontier is missing".to_owned())
            })?;
            frontier.coverage_floor = Some(0);
            frontier.prefix_status = CoveragePrefixStatus::Complete;
        }
        CatalogAncestryRepairOutcome::Unresolved {
            recovered_blocks,
            missing_block_id,
            ..
        } => {
            validate_unresolved_repair(
                &request,
                &recovered_blocks,
                &missing_block_id,
                &request.upper_checkpoint,
            )?;
            let Some(oldest) = recovered_blocks.first() else {
                return Ok(None);
            };
            ingest_blocks(&mut working, &recovered_blocks, &segment_id, context)?;
            let segment = working.segments.get_mut(&segment_id).ok_or_else(|| {
                CatalogEngineError::InvalidState("prefix segment is missing".to_owned())
            })?;
            segment.floor = oldest.checkpoint.clone();
            let frontier = working.frontier.as_mut().ok_or_else(|| {
                CatalogEngineError::InvalidState("catalog frontier is missing".to_owned())
            })?;
            frontier.coverage_floor = Some(oldest.checkpoint.slot);
        }
    }

    recompute_coverage(&mut working)?;
    working.into_batch(snapshot, context)
}

pub fn reduce_catalog_gap_repair(
    snapshot: &CatalogSnapshot,
    gap_id: &str,
    outcome: CatalogAncestryRepairOutcome,
    confirmation: &CatalogRepairConfirmation,
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogBatch> {
    validate_context(snapshot, context)?;
    let mut working = WorkingCatalog::from_snapshot(snapshot);
    let gap = working
        .gaps
        .get(gap_id)
        .cloned()
        .ok_or_else(|| CatalogEngineError::InvalidState(format!("unknown gap {gap_id}")))?;
    let lower = working
        .segments
        .get(&gap.lower_segment_id)
        .cloned()
        .ok_or_else(|| CatalogEngineError::InvalidState("gap lower segment is missing".into()))?;
    let upper = working
        .segments
        .get(&gap.upper_segment_id)
        .cloned()
        .ok_or_else(|| CatalogEngineError::InvalidState("gap upper segment is missing".into()))?;
    let request = CatalogAncestryRepairRequest {
        lower_checkpoint: Some(gap.lower_checkpoint.clone()),
        upper_checkpoint: upper.floor.clone(),
        expected_genesis_id: None,
        max_blocks: context.repair_block_limit,
    };
    validate_repair_confirmation(
        snapshot,
        request.lower_checkpoint.as_ref(),
        confirmation,
        false,
    )?;

    match outcome {
        CatalogAncestryRepairOutcome::Connected { recovered_blocks } => {
            validate_connected_repair(&request, &recovered_blocks)?;
            let confirmed = confirmation
                .upper_frontier_checkpoint
                .clone()
                .ok_or_else(|| {
                    CatalogEngineError::InvalidState(
                        "connected gap repair requires verified upper frontier checkpoint"
                            .to_owned(),
                    )
                })?;
            validate_checkpoint(&confirmed, "verified upper frontier")?;
            if confirmed.slot != upper.frontier.slot
                || confirmed.block_id != upper.frontier.block_id
            {
                return Err(CatalogEngineError::SourceInconsistent(
                    "verified upper frontier does not match persisted segment".to_owned(),
                ));
            }

            reassign_segment(&mut working, &upper.segment_id, &lower.segment_id, context)?;
            ingest_blocks(&mut working, &recovered_blocks, &lower.segment_id, context)?;
            let merged = CoverageSegment {
                segment_id: lower.segment_id.clone(),
                floor: lower.floor,
                frontier: upper.frontier,
                reaches_target_lib: upper.reaches_target_lib,
            };
            working.segments.insert(merged.segment_id.clone(), merged);
            working.segments.remove(&upper.segment_id);
            working.gaps.remove(gap_id);
            for adjacent in working.gaps.values_mut() {
                if adjacent.lower_segment_id == upper.segment_id {
                    adjacent.lower_segment_id = lower.segment_id.clone();
                }
                if adjacent.upper_segment_id == upper.segment_id {
                    adjacent.upper_segment_id = lower.segment_id.clone();
                }
            }
            if working
                .frontier
                .as_ref()
                .and_then(|frontier| frontier.checkpoint.as_ref())
                .is_some_and(|checkpoint| checkpoint.block_id == gap.lower_checkpoint.block_id)
            {
                set_connected_frontier_checkpoint(&mut working, confirmed);
            }
        }
        CatalogAncestryRepairOutcome::Unresolved {
            recovered_blocks,
            missing_block_id,
            reason,
        } => {
            validate_unresolved_repair(
                &request,
                &recovered_blocks,
                &missing_block_id,
                &upper.floor,
            )?;
            let mut updated_gap = gap;
            updated_gap.attempts = updated_gap.attempts.checked_add(1).ok_or_else(|| {
                CatalogEngineError::Overflow("coverage gap attempts exhausted".to_owned())
            })?;
            updated_gap.last_attempt_at_unix = Some(context.updated_at_unix);
            updated_gap.status = CoverageGapStatus::Backoff;
            updated_gap.reason = reason;
            if let Some(oldest) = recovered_blocks.first() {
                let mut expanded_upper = upper;
                expanded_upper.floor = oldest.checkpoint.clone();
                updated_gap.upper_block = block_reference(&oldest.checkpoint);
                updated_gap.required_parent_id = missing_block_id;
                ingest_blocks(
                    &mut working,
                    &recovered_blocks,
                    &expanded_upper.segment_id,
                    context,
                )?;
                working
                    .segments
                    .insert(expanded_upper.segment_id.clone(), expanded_upper);
            }
            working.gaps.insert(updated_gap.gap_id.clone(), updated_gap);
        }
    }

    recompute_coverage(&mut working)?;
    working
        .into_batch(snapshot, context)?
        .ok_or_else(|| CatalogEngineError::InvalidState("gap repair produced no change".into()))
}

fn apply_connected_events(
    snapshot: &CatalogSnapshot,
    events: &[CatalogL1BlockEvent],
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogBatch> {
    let first = events.first().ok_or_else(|| {
        CatalogEngineError::InvalidState("connected page has no accepted events".to_owned())
    })?;
    let last = events.last().ok_or_else(|| {
        CatalogEngineError::InvalidState("connected page has no final event".to_owned())
    })?;
    let target = current_target(snapshot)?.clone();
    let mut working = WorkingCatalog::from_snapshot(snapshot);
    let cursor = working
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.ingestion_cursor.clone());

    let segment_id = if let Some(cursor) = cursor.as_ref() {
        active_segment_id_in_working(&working, cursor)?
    } else {
        let segment_id = segment_id_for_floor(&first.block.checkpoint);
        if working.segments.contains_key(&segment_id) {
            return Err(CatalogEngineError::InvalidState(format!(
                "initial segment {segment_id} already exists"
            )));
        }
        working.segments.insert(
            segment_id.clone(),
            CoverageSegment {
                segment_id: segment_id.clone(),
                floor: first.block.checkpoint.clone(),
                frontier: block_reference(&first.block.checkpoint),
                reaches_target_lib: false,
            },
        );
        let frontier = working.frontier.as_mut().ok_or_else(|| {
            CatalogEngineError::InvalidState("catch-up frontier is missing".to_owned())
        })?;
        frontier.coverage_floor = Some(0);
        frontier.prefix_status = CoveragePrefixStatus::Complete;
        segment_id
    };

    let blocks = events
        .iter()
        .map(|event| event.block.clone())
        .collect::<Vec<_>>();
    ingest_blocks(&mut working, &blocks, &segment_id, context)?;
    let segment = working.segments.get_mut(&segment_id).ok_or_else(|| {
        CatalogEngineError::InvalidState(format!("active segment {segment_id} is missing"))
    })?;
    segment.frontier = block_reference(&last.block.checkpoint);
    segment.reaches_target_lib = segment.frontier == target;

    let traversal = working.traversal.as_mut().ok_or_else(|| {
        CatalogEngineError::InvalidState("catch-up traversal is missing".to_owned())
    })?;
    traversal.ingestion_cursor = Some(block_reference(&last.block.checkpoint));
    if working.gaps.is_empty() {
        set_connected_frontier_checkpoint(&mut working, last.block.checkpoint.clone());
    }
    recompute_coverage(&mut working)?;
    working
        .into_batch(snapshot, context)?
        .ok_or_else(|| CatalogEngineError::InvalidState("page reduction produced no change".into()))
}

fn open_disconnected_segment(
    snapshot: &CatalogSnapshot,
    events: Vec<CatalogL1BlockEvent>,
    missing_block_id: String,
    reason: CoverageGapReason,
    context: CatalogEngineContext,
) -> CatalogEngineResult<CatalogPageReduction> {
    validate_id(&missing_block_id, "missing repair block id")?;
    validate_page_snapshots(&events)?;
    let target = current_target(snapshot)?.clone();
    for event in &events {
        validate_event_against_target(event, &target)?;
    }
    validate_strictly_ascending_slots(&events)?;
    let (accepted, remaining) = split_connected_events(events)?;
    let first = accepted.first().ok_or_else(|| {
        CatalogEngineError::InvalidState("disconnected segment has no floor block".to_owned())
    })?;
    let last = accepted.last().ok_or_else(|| {
        CatalogEngineError::InvalidState("disconnected segment has no frontier block".to_owned())
    })?;
    if first.block.checkpoint.parent_id != missing_block_id {
        return Err(CatalogEngineError::InvalidState(
            "disconnected floor does not reference unresolved parent".to_owned(),
        ));
    }

    let mut working = WorkingCatalog::from_snapshot(snapshot);
    let lower = working
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.ingestion_cursor.clone());
    let segment_id = segment_id_for_floor(&first.block.checkpoint);
    if working.segments.contains_key(&segment_id) {
        return Err(CatalogEngineError::InvalidState(format!(
            "disconnected segment {segment_id} already exists"
        )));
    }
    let segment = CoverageSegment {
        segment_id: segment_id.clone(),
        floor: first.block.checkpoint.clone(),
        frontier: block_reference(&last.block.checkpoint),
        reaches_target_lib: block_reference(&last.block.checkpoint) == target,
    };
    working.segments.insert(segment_id.clone(), segment);

    if let Some(lower) = lower.as_ref() {
        let lower_segment_id = active_segment_id_in_working(&working, lower)?;
        let gap_id = gap_id_for_boundary(lower, &first.block.checkpoint);
        working.gaps.insert(
            gap_id.clone(),
            CoverageGap {
                gap_id,
                lower_segment_id,
                lower_checkpoint: lower.clone(),
                upper_segment_id: segment_id.clone(),
                upper_block: block_reference(&first.block.checkpoint),
                required_parent_id: missing_block_id,
                reason,
                status: CoverageGapStatus::Pending,
                attempts: 1,
                first_seen_at_unix: context.updated_at_unix,
                last_attempt_at_unix: Some(context.updated_at_unix),
            },
        );
    } else {
        if !snapshot.segments.is_empty() || !snapshot.gaps.is_empty() {
            return Err(CatalogEngineError::InvalidState(
                "unavailable prefix cannot open over existing coverage".to_owned(),
            ));
        }
        let frontier = working.frontier.as_mut().ok_or_else(|| {
            CatalogEngineError::InvalidState("catch-up frontier is missing".to_owned())
        })?;
        frontier.coverage_floor = Some(first.block.checkpoint.slot);
        frontier.prefix_status = CoveragePrefixStatus::Unavailable;
        set_connected_frontier_checkpoint(&mut working, last.block.checkpoint.clone());
    }

    let blocks = accepted
        .iter()
        .map(|event| event.block.clone())
        .collect::<Vec<_>>();
    ingest_blocks(&mut working, &blocks, &segment_id, context)?;
    let traversal = working.traversal.as_mut().ok_or_else(|| {
        CatalogEngineError::InvalidState("catch-up traversal is missing".to_owned())
    })?;
    traversal.ingestion_cursor = Some(block_reference(&last.block.checkpoint));
    recompute_coverage(&mut working)?;
    let batch = working.into_batch(snapshot, context)?.ok_or_else(|| {
        CatalogEngineError::InvalidState("disconnected reduction produced no change".into())
    })?;
    Ok(CatalogPageReduction::Commit {
        batch: Box::new(batch),
        remaining_events: remaining,
    })
}

fn split_connected_events(
    events: Vec<CatalogL1BlockEvent>,
) -> CatalogEngineResult<(Vec<CatalogL1BlockEvent>, Vec<CatalogL1BlockEvent>)> {
    let mut iterator = events.into_iter();
    let first = iterator.next().ok_or_else(|| {
        CatalogEngineError::InvalidState("cannot split an empty event page".to_owned())
    })?;
    let mut previous = first.block.checkpoint.clone();
    let mut accepted = vec![first];
    let mut remaining = Vec::new();
    let mut broken = false;
    for event in iterator {
        if !broken
            && event.block.checkpoint.slot > previous.slot
            && event.block.checkpoint.parent_id == previous.block_id
        {
            previous = event.block.checkpoint.clone();
            accepted.push(event);
        } else {
            if !broken && event.block.checkpoint.slot <= previous.slot {
                return Err(CatalogEngineError::SourceInconsistent(
                    "page block slots are not strictly ascending".to_owned(),
                ));
            }
            broken = true;
            remaining.push(event);
        }
    }
    Ok((accepted, remaining))
}

fn prepend_recovered_events(
    recovered_blocks: Vec<CatalogL1Block>,
    pending_events: Vec<CatalogL1BlockEvent>,
    snapshot: super::source::CatalogL1ChainSnapshot,
) -> Vec<CatalogL1BlockEvent> {
    let mut events =
        Vec::with_capacity(recovered_blocks.len().saturating_add(pending_events.len()));
    events.extend(
        recovered_blocks
            .into_iter()
            .map(|block| CatalogL1BlockEvent {
                block,
                snapshot: snapshot.clone(),
            }),
    );
    events.extend(pending_events);
    events
}

fn validate_connected_repair(
    request: &CatalogAncestryRepairRequest,
    recovered: &[CatalogL1Block],
) -> CatalogEngineResult<()> {
    validate_repair_request(request)?;
    if recovered.is_empty() {
        if request
            .lower_checkpoint
            .as_ref()
            .is_some_and(|lower| request.upper_checkpoint.parent_id == lower.block_id)
        {
            return Ok(());
        }
        return Err(CatalogEngineError::InvalidState(
            "empty repair does not connect ancestry".to_owned(),
        ));
    }
    validate_recovered_chain(recovered)?;
    let oldest = recovered.first().ok_or_else(|| {
        CatalogEngineError::InvalidState("connected repair lost oldest block".into())
    })?;
    let newest = recovered.last().ok_or_else(|| {
        CatalogEngineError::InvalidState("connected repair lost newest block".into())
    })?;
    if newest.checkpoint.block_id != request.upper_checkpoint.parent_id {
        return Err(CatalogEngineError::InvalidState(
            "repair does not reach upper block parent".to_owned(),
        ));
    }
    if newest.checkpoint.slot >= request.upper_checkpoint.slot {
        return Err(CatalogEngineError::SourceInconsistent(
            "repaired ancestry does not precede upper block".to_owned(),
        ));
    }
    if recovered
        .iter()
        .any(|block| block.checkpoint.block_id == request.upper_checkpoint.block_id)
    {
        return Err(CatalogEngineError::SourceInconsistent(
            "repaired ancestry contains upper boundary block".to_owned(),
        ));
    }
    match request.lower_checkpoint.as_ref() {
        Some(lower)
            if oldest.checkpoint.slot <= lower.slot
                || recovered
                    .iter()
                    .any(|block| block.checkpoint.block_id == lower.block_id) =>
        {
            Err(CatalogEngineError::SourceInconsistent(
                "repaired ancestry crosses a requested boundary".to_owned(),
            ))
        }
        Some(lower) if oldest.checkpoint.parent_id != lower.block_id => Err(
            CatalogEngineError::InvalidState("repair does not reach lower checkpoint".to_owned()),
        ),
        None if oldest.checkpoint.slot != 0 => Err(CatalogEngineError::InvalidState(
            "prefix repair does not reach slot-zero block".to_owned(),
        )),
        None if request
            .expected_genesis_id
            .as_ref()
            .is_some_and(|expected| expected != &oldest.checkpoint.block_id) =>
        {
            Err(CatalogEngineError::SourceInconsistent(
                "prefix repair reached conflicting genesis id".to_owned(),
            ))
        }
        Some(_) | None => Ok(()),
    }
}

fn validate_unresolved_repair(
    request: &CatalogAncestryRepairRequest,
    recovered: &[CatalogL1Block],
    missing_block_id: &str,
    upper: &CatalogBlockCheckpoint,
) -> CatalogEngineResult<()> {
    validate_repair_request(request)?;
    validate_id(missing_block_id, "unresolved repair block id")?;
    if upper != &request.upper_checkpoint {
        return Err(CatalogEngineError::InvalidState(
            "unresolved repair upper checkpoint changed".to_owned(),
        ));
    }
    if request
        .lower_checkpoint
        .as_ref()
        .is_some_and(|lower| lower.block_id == missing_block_id)
    {
        return Err(CatalogEngineError::InvalidState(
            "unresolved repair already reaches lower checkpoint".to_owned(),
        ));
    }
    if recovered.is_empty() {
        if upper.parent_id != missing_block_id {
            return Err(CatalogEngineError::InvalidState(
                "unresolved repair id does not match upper parent".to_owned(),
            ));
        }
        return Ok(());
    }
    validate_recovered_chain(recovered)?;
    let oldest = recovered.first().ok_or_else(|| {
        CatalogEngineError::InvalidState("unresolved repair lost oldest block".into())
    })?;
    let newest = recovered.last().ok_or_else(|| {
        CatalogEngineError::InvalidState("unresolved repair lost newest block".into())
    })?;
    if oldest.checkpoint.parent_id != missing_block_id
        || newest.checkpoint.block_id != upper.parent_id
    {
        return Err(CatalogEngineError::InvalidState(
            "unresolved repair chain does not match gap boundaries".to_owned(),
        ));
    }
    if newest.checkpoint.slot >= upper.slot
        || request
            .lower_checkpoint
            .as_ref()
            .is_some_and(|lower| oldest.checkpoint.slot <= lower.slot)
        || request.lower_checkpoint.is_none() && oldest.checkpoint.slot == 0
        || recovered.iter().any(|block| {
            block.checkpoint.block_id == missing_block_id
                || block.checkpoint.block_id == upper.block_id
                || request
                    .lower_checkpoint
                    .as_ref()
                    .is_some_and(|lower| block.checkpoint.block_id == lower.block_id)
        })
    {
        return Err(CatalogEngineError::SourceInconsistent(
            "unresolved ancestry crosses a requested boundary".to_owned(),
        ));
    }
    Ok(())
}

fn validate_repair_request(request: &CatalogAncestryRepairRequest) -> CatalogEngineResult<()> {
    validate_checkpoint(&request.upper_checkpoint, "repair upper checkpoint")?;
    if let Some(lower) = request.lower_checkpoint.as_ref() {
        validate_reference(lower, "repair lower checkpoint")?;
        if lower.slot >= request.upper_checkpoint.slot {
            return Err(CatalogEngineError::InvalidState(
                "repair lower checkpoint must precede upper block".to_owned(),
            ));
        }
    }
    if let Some(genesis_id) = request.expected_genesis_id.as_ref() {
        validate_id(genesis_id, "expected genesis id")?;
    }
    Ok(())
}

fn validate_recovered_chain(recovered: &[CatalogL1Block]) -> CatalogEngineResult<()> {
    let mut previous: Option<&CatalogBlockCheckpoint> = None;
    let mut ids = HashSet::new();
    for block in recovered {
        validate_checkpoint(&block.checkpoint, "recovered block")?;
        if !ids.insert(block.checkpoint.block_id.as_str()) {
            return Err(CatalogEngineError::SourceInconsistent(
                "recovered ancestry contains duplicate block ids".to_owned(),
            ));
        }
        if let Some(previous) = previous
            && (block.checkpoint.slot <= previous.slot
                || block.checkpoint.parent_id != previous.block_id)
        {
            return Err(CatalogEngineError::SourceInconsistent(
                "recovered ancestry is not strictly ascending and parent-linked".to_owned(),
            ));
        }
        previous = Some(&block.checkpoint);
    }
    Ok(())
}

fn validate_page_snapshots(events: &[CatalogL1BlockEvent]) -> CatalogEngineResult<()> {
    let Some(first) = events.first() else {
        return Ok(());
    };
    for event in events {
        if event.snapshot != first.snapshot {
            return Err(CatalogEngineError::SourceInconsistent(
                "page chain snapshot changed between events".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_strictly_ascending_slots(events: &[CatalogL1BlockEvent]) -> CatalogEngineResult<()> {
    let mut previous = None;
    for event in events {
        if previous.is_some_and(|slot| event.block.checkpoint.slot <= slot) {
            return Err(CatalogEngineError::SourceInconsistent(
                "page block slots are not strictly ascending".to_owned(),
            ));
        }
        previous = Some(event.block.checkpoint.slot);
    }
    Ok(())
}

fn validate_repair_confirmation(
    snapshot: &CatalogSnapshot,
    expected_lower: Option<&CatalogBlockReference>,
    confirmation: &CatalogRepairConfirmation,
    require_upper_frontier: bool,
) -> CatalogEngineResult<()> {
    validate_reference(&confirmation.target_lib, "confirmed target LIB")?;
    if &confirmation.target_lib != current_target(snapshot)? {
        return Err(CatalogEngineError::SourceInconsistent(
            "repair confirmation target LIB changed".to_owned(),
        ));
    }
    if confirmation.lower_checkpoint.as_ref() != expected_lower {
        return Err(CatalogEngineError::SourceInconsistent(
            "repair confirmation lower checkpoint changed".to_owned(),
        ));
    }
    if let Some(lower) = confirmation.lower_checkpoint.as_ref() {
        validate_reference(lower, "confirmed lower checkpoint")?;
    }
    if let Some(upper) = confirmation.upper_frontier_checkpoint.as_ref() {
        validate_checkpoint(upper, "confirmed upper frontier")?;
    }
    if require_upper_frontier {
        let persisted = snapshot
            .frontier
            .as_ref()
            .and_then(|frontier| frontier.checkpoint.as_ref())
            .ok_or_else(|| {
                CatalogEngineError::InvalidState(
                    "prefix repair has no persisted frontier checkpoint".to_owned(),
                )
            })?;
        if confirmation.upper_frontier_checkpoint.as_ref() != Some(persisted) {
            return Err(CatalogEngineError::SourceInconsistent(
                "repair confirmation frontier checkpoint changed".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_event_against_target(
    event: &CatalogL1BlockEvent,
    target: &CatalogBlockReference,
) -> CatalogEngineResult<()> {
    if event.block.checkpoint.slot > target.slot {
        return Err(CatalogEngineError::SourceInconsistent(format!(
            "page block slot {} is beyond target LIB slot {}",
            event.block.checkpoint.slot, target.slot
        )));
    }
    if event.block.checkpoint.slot == target.slot
        && event.block.checkpoint.block_id != target.block_id
    {
        return Err(CatalogEngineError::SourceInconsistent(
            "page block conflicts with target LIB id".to_owned(),
        ));
    }
    if event.snapshot.lib.slot < target.slot
        || event.snapshot.lib.slot == target.slot && event.snapshot.lib.block_id != target.block_id
    {
        return Err(CatalogEngineError::SourceInconsistent(
            "page source snapshot does not cover fixed target LIB".to_owned(),
        ));
    }
    Ok(())
}

fn repair_request(
    lower_checkpoint: Option<CatalogBlockReference>,
    upper_checkpoint: CatalogBlockCheckpoint,
    snapshot: &CatalogSnapshot,
    context: CatalogEngineContext,
) -> CatalogAncestryRepairRequest {
    let expected_genesis_id = match &snapshot.metadata.network_scope {
        NetworkScope::GenesisId { genesis_id } => Some(genesis_id.clone()),
        NetworkScope::FinalizedAnchor { .. } => None,
    };
    CatalogAncestryRepairRequest {
        lower_checkpoint,
        upper_checkpoint,
        expected_genesis_id,
        max_blocks: context.repair_block_limit,
    }
}

fn validate_genesis_block(
    snapshot: &CatalogSnapshot,
    checkpoint: &CatalogBlockCheckpoint,
) -> CatalogEngineResult<()> {
    if checkpoint.slot != 0 {
        return Err(CatalogEngineError::InvalidState(
            "genesis block must be at slot zero".to_owned(),
        ));
    }
    if let NetworkScope::GenesisId { genesis_id } = &snapshot.metadata.network_scope
        && checkpoint.block_id != *genesis_id
    {
        return Err(CatalogEngineError::SourceInconsistent(
            "slot-zero block conflicts with catalog genesis id".to_owned(),
        ));
    }
    Ok(())
}

fn active_segment_id(
    snapshot: &CatalogSnapshot,
    cursor: &CatalogBlockReference,
) -> CatalogEngineResult<String> {
    let matches = snapshot
        .segments
        .iter()
        .filter(|segment| segment.frontier == *cursor)
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [segment] => Ok(segment.segment_id.clone()),
        [] => Err(CatalogEngineError::InvalidState(
            "ingestion cursor has no active coverage segment".to_owned(),
        )),
        _ => Err(CatalogEngineError::InvalidState(
            "ingestion cursor matches multiple coverage segments".to_owned(),
        )),
    }
}

fn active_segment_id_in_working(
    working: &WorkingCatalog,
    cursor: &CatalogBlockReference,
) -> CatalogEngineResult<String> {
    let matches = working
        .segments
        .values()
        .filter(|segment| segment.frontier == *cursor)
        .map(|segment| segment.segment_id.clone())
        .collect::<Vec<_>>();
    match matches.as_slice() {
        [segment_id] => Ok(segment_id.clone()),
        [] => Err(CatalogEngineError::InvalidState(
            "ingestion cursor has no active coverage segment".to_owned(),
        )),
        _ => Err(CatalogEngineError::InvalidState(
            "ingestion cursor matches multiple coverage segments".to_owned(),
        )),
    }
}

fn current_target(snapshot: &CatalogSnapshot) -> CatalogEngineResult<&CatalogBlockReference> {
    snapshot
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.target_lib.as_ref())
        .ok_or_else(|| {
            CatalogEngineError::InvalidState("catch-up target LIB is missing".to_owned())
        })
}

fn current_target_in_working(
    working: &WorkingCatalog,
) -> CatalogEngineResult<&CatalogBlockReference> {
    working
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.target_lib.as_ref())
        .ok_or_else(|| {
            CatalogEngineError::InvalidState("catch-up target LIB is missing".to_owned())
        })
}

fn set_connected_frontier_checkpoint(
    working: &mut WorkingCatalog,
    checkpoint: CatalogBlockCheckpoint,
) {
    if let Some(frontier) = working.frontier.as_mut() {
        frontier.scanned_through_slot = Some(checkpoint.slot);
        frontier.checkpoint = Some(checkpoint);
    }
}

fn recompute_coverage(working: &mut WorkingCatalog) -> CatalogEngineResult<()> {
    let target = current_target_in_working(working)?.clone();
    let reaches_target = working
        .segments
        .values()
        .any(|segment| segment.reaches_target_lib && segment.frontier == target);
    let frontier = working.frontier.as_mut().ok_or_else(|| {
        CatalogEngineError::InvalidState("catalog frontier is missing".to_owned())
    })?;
    frontier.observed_lib = Some(target);
    frontier.coverage_status = if reaches_target
        && working.gaps.is_empty()
        && frontier.prefix_status == CoveragePrefixStatus::Complete
        && frontier.coverage_floor == Some(0)
    {
        CatalogCoverageStatus::Complete
    } else if reaches_target
        || !working.gaps.is_empty()
        || frontier.prefix_status == CoveragePrefixStatus::Unavailable
    {
        CatalogCoverageStatus::Partial
    } else {
        CatalogCoverageStatus::Rebuilding
    };
    Ok(())
}

fn reassign_segment(
    working: &mut WorkingCatalog,
    old_segment_id: &str,
    new_segment_id: &str,
    context: CatalogEngineContext,
) -> CatalogEngineResult<()> {
    let mut channels = BTreeSet::new();
    for evidence in working.evidence.values_mut() {
        if evidence.coverage_segment_id == old_segment_id {
            channels.insert(evidence.channel_id.clone());
            evidence.coverage_segment_id = new_segment_id.to_owned();
        }
    }
    for zone in working.zones.values_mut() {
        if zone.snapshot_provenance.coverage_segment_id == old_segment_id {
            zone.snapshot_provenance.coverage_segment_id = new_segment_id.to_owned();
            zone.snapshot_provenance.source_revision = context.source_revision;
            zone.updated_at_unix = context.updated_at_unix;
        }
    }
    let target_slot = current_target_in_working(working)?.slot;
    recompute_zone_records(working, &channels, target_slot, context)
}

fn validate_context(
    snapshot: &CatalogSnapshot,
    context: CatalogEngineContext,
) -> CatalogEngineResult<()> {
    if context.updated_at_unix < snapshot.metadata.updated_at_unix {
        return Err(CatalogEngineError::InvalidState(
            "engine update time precedes catalog update time".to_owned(),
        ));
    }
    Ok(())
}

fn validate_reference(reference: &CatalogBlockReference, label: &str) -> CatalogEngineResult<()> {
    validate_id(&reference.block_id, &format!("{label} block id"))
}

fn validate_checkpoint(
    checkpoint: &CatalogBlockCheckpoint,
    label: &str,
) -> CatalogEngineResult<()> {
    validate_id(&checkpoint.block_id, &format!("{label} id"))?;
    validate_id(&checkpoint.parent_id, &format!("{label} parent id"))
}

fn validate_id(value: &str, label: &str) -> CatalogEngineResult<()> {
    validate_hex_id(value, label)
        .map_err(|error| CatalogEngineError::InvalidState(error.to_string()))
}

fn block_reference(checkpoint: &CatalogBlockCheckpoint) -> CatalogBlockReference {
    CatalogBlockReference {
        slot: checkpoint.slot,
        block_id: checkpoint.block_id.clone(),
    }
}

fn segment_id_for_floor(floor: &CatalogBlockCheckpoint) -> String {
    format!("segment-{}", floor.block_id)
}

fn gap_id_for_boundary(lower: &CatalogBlockReference, upper: &CatalogBlockCheckpoint) -> String {
    let mut hasher = Sha256::new();
    hasher.update(lower.block_id.as_bytes());
    hasher.update(upper.block_id.as_bytes());
    format!("gap-{}", hex::encode(hasher.finalize()))
}

#[derive(Clone)]
struct WorkingCatalog {
    frontier: Option<CatalogFrontier>,
    traversal: Option<CatalogTraversal>,
    zones: BTreeMap<String, ZoneCatalogRecord>,
    evidence: BTreeMap<String, ZoneEvidenceReference>,
    segments: BTreeMap<String, CoverageSegment>,
    gaps: BTreeMap<String, CoverageGap>,
}

impl WorkingCatalog {
    fn from_snapshot(snapshot: &CatalogSnapshot) -> Self {
        Self {
            frontier: snapshot.frontier.clone(),
            traversal: snapshot.traversal.clone(),
            zones: records_by_key(&snapshot.zones, |record| &record.channel_id),
            evidence: records_by_key(&snapshot.evidence, |record| &record.evidence_id),
            segments: records_by_key(&snapshot.segments, |record| &record.segment_id),
            gaps: records_by_key(&snapshot.gaps, |record| &record.gap_id),
        }
    }

    fn into_batch(
        self,
        snapshot: &CatalogSnapshot,
        context: CatalogEngineContext,
    ) -> CatalogEngineResult<Option<CatalogBatch>> {
        let (upsert_zones, delete_zone_ids) =
            record_diff(&snapshot.zones, &self.zones, |record| &record.channel_id);
        let (upsert_evidence, delete_evidence_ids) =
            record_diff(&snapshot.evidence, &self.evidence, |record| {
                &record.evidence_id
            });
        let (upsert_segments, delete_segment_ids) =
            record_diff(&snapshot.segments, &self.segments, |record| {
                &record.segment_id
            });
        let (upsert_gaps, delete_gap_ids) =
            record_diff(&snapshot.gaps, &self.gaps, |record| &record.gap_id);
        let changed = !upsert_zones.is_empty()
            || !delete_zone_ids.is_empty()
            || !upsert_evidence.is_empty()
            || !delete_evidence_ids.is_empty()
            || !upsert_segments.is_empty()
            || !delete_segment_ids.is_empty()
            || !upsert_gaps.is_empty()
            || !delete_gap_ids.is_empty()
            || self.frontier != snapshot.frontier
            || self.traversal != snapshot.traversal;
        if !changed {
            return Ok(None);
        }
        Ok(Some(CatalogBatch {
            expected_catalog_revision: snapshot.metadata.catalog_revision,
            updated_at_unix: context.updated_at_unix,
            upsert_zones,
            delete_zone_ids,
            upsert_evidence,
            delete_evidence_ids,
            upsert_segments,
            delete_segment_ids,
            upsert_gaps,
            delete_gap_ids,
            frontier: self.frontier,
            traversal: self.traversal,
        }))
    }
}

fn records_by_key<T: Clone>(records: &[T], key: impl Fn(&T) -> &String) -> BTreeMap<String, T> {
    records
        .iter()
        .map(|record| (key(record).clone(), record.clone()))
        .collect()
}

fn record_diff<T: Clone + PartialEq>(
    before: &[T],
    after: &BTreeMap<String, T>,
    key: impl Fn(&T) -> &String,
) -> (Vec<T>, Vec<String>) {
    let before = records_by_key(before, key);
    let upserts = after
        .iter()
        .filter(|(record_key, record)| before.get(*record_key) != Some(*record))
        .map(|(_, record)| record.clone())
        .collect();
    let deletes = before
        .keys()
        .filter(|record_key| !after.contains_key(*record_key))
        .cloned()
        .collect();
    (upserts, deletes)
}

#[derive(Debug, Clone)]
struct ChannelObservation {
    channel_id: String,
    transaction_hash: String,
    operation_index: u32,
    kind: ChannelObservationKind,
}

#[derive(Debug, Clone)]
enum ChannelObservationKind {
    Configuration {
        keys: Vec<String>,
        withdraw_threshold: String,
    },
    Inscription {
        evidence_kind: ZoneEvidenceKind,
        parent_is_root: bool,
        signer: String,
        conflicting: bool,
    },
    Operation,
}

fn ingest_blocks(
    working: &mut WorkingCatalog,
    blocks: &[CatalogL1Block],
    segment_id: &str,
    context: CatalogEngineContext,
) -> CatalogEngineResult<()> {
    let target_slot = current_target_in_working(working)?.slot;
    let mut touched_channels = BTreeSet::new();
    for block in blocks {
        for observation in extract_channel_observations(block)? {
            touched_channels.insert(observation.channel_id.clone());
            ingest_observation(
                working,
                block,
                segment_id,
                target_slot,
                observation,
                context,
            )?;
        }
    }
    recompute_zone_records(working, &touched_channels, target_slot, context)
}

fn extract_channel_observations(
    block: &CatalogL1Block,
) -> CatalogEngineResult<Vec<ChannelObservation>> {
    let transactions = block
        .payload
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CatalogEngineError::InvalidBlock("block transactions are not an array".to_owned())
        })?;
    let mut observations = Vec::new();
    for transaction in transactions {
        let Some(mantle_tx) = transaction.get("mantle_tx") else {
            continue;
        };
        let Some(operations) = mantle_tx.get("ops").and_then(Value::as_array) else {
            continue;
        };
        for (index, operation) in operations.iter().enumerate() {
            let Some(opcode) = operation.get("opcode").and_then(parse_opcode) else {
                continue;
            };
            if !matches!(opcode, 0x10..=0x13) {
                continue;
            }
            let transaction_hash = mantle_tx
                .get("hash")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    CatalogEngineError::InvalidBlock(
                        "Channel operation transaction hash is missing".to_owned(),
                    )
                })
                .and_then(|value| canonical_hex(value, "transaction hash"))?;
            let operation_index = u32::try_from(index).map_err(|_| {
                CatalogEngineError::Overflow("transaction operation index exceeds u32".to_owned())
            })?;
            let payload = operation.get("payload").ok_or_else(|| {
                CatalogEngineError::InvalidBlock(format!(
                    "Channel opcode {opcode:#x} payload is missing"
                ))
            })?;
            let observation = match opcode {
                0x10 => parse_configuration(payload, transaction_hash, operation_index)?,
                0x11 => parse_inscription(payload, transaction_hash, operation_index)?,
                0x12 | 0x13 => parse_channel_operation(payload, transaction_hash, operation_index)?,
                _ => {
                    return Err(CatalogEngineError::InvalidBlock(format!(
                        "unsupported Channel opcode {opcode:#x}"
                    )));
                }
            };
            observations.push(observation);
        }
    }
    Ok(observations)
}

fn parse_configuration(
    payload: &Value,
    transaction_hash: String,
    operation_index: u32,
) -> CatalogEngineResult<ChannelObservation> {
    let channel_id = required_channel_id(payload, &["channel"])?;
    let keys = payload
        .get("keys")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            CatalogEngineError::InvalidBlock("ChannelConfig keys are missing".to_owned())
        })?
        .iter()
        .map(|key| {
            key.as_str()
                .ok_or_else(|| {
                    CatalogEngineError::InvalidBlock("ChannelConfig key is not text".to_owned())
                })
                .and_then(|key| canonical_local_text(key, "Channel key"))
        })
        .collect::<CatalogEngineResult<Vec<_>>>()?;
    if keys.is_empty() {
        return Err(CatalogEngineError::InvalidBlock(
            "ChannelConfig keys are empty".to_owned(),
        ));
    }
    let withdraw_threshold = payload
        .get("withdraw_threshold")
        .and_then(integer_text)
        .ok_or_else(|| {
            CatalogEngineError::InvalidBlock(
                "ChannelConfig withdraw_threshold is missing".to_owned(),
            )
        })?;
    Ok(ChannelObservation {
        channel_id,
        transaction_hash,
        operation_index,
        kind: ChannelObservationKind::Configuration {
            keys,
            withdraw_threshold,
        },
    })
}

fn parse_inscription(
    payload: &Value,
    transaction_hash: String,
    operation_index: u32,
) -> CatalogEngineResult<ChannelObservation> {
    let channel_id = required_channel_id(payload, &["channel_id"])?;
    let parent = payload
        .get("parent")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CatalogEngineError::InvalidBlock("ChannelInscribe parent is missing".to_owned())
        })
        .and_then(|value| canonical_hex(value, "ChannelInscribe parent"))?;
    let signer = payload
        .get("signer")
        .and_then(Value::as_str)
        .ok_or_else(|| {
            CatalogEngineError::InvalidBlock("ChannelInscribe signer is missing".to_owned())
        })
        .and_then(|value| canonical_local_text(value, "ChannelInscribe signer"))?;
    let inscription = payload.get("inscription").ok_or_else(|| {
        CatalogEngineError::InvalidBlock("ChannelInscribe inscription is missing".to_owned())
    })?;
    let (evidence_kind, conflicting) = classify_inscription(inscription)?;
    Ok(ChannelObservation {
        channel_id,
        transaction_hash,
        operation_index,
        kind: ChannelObservationKind::Inscription {
            evidence_kind,
            parent_is_root: parent.bytes().all(|byte| byte == b'0'),
            signer,
            conflicting,
        },
    })
}

fn parse_channel_operation(
    payload: &Value,
    transaction_hash: String,
    operation_index: u32,
) -> CatalogEngineResult<ChannelObservation> {
    Ok(ChannelObservation {
        channel_id: required_channel_id(payload, &["channel_id"])?,
        transaction_hash,
        operation_index,
        kind: ChannelObservationKind::Operation,
    })
}

fn classify_inscription(value: &Value) -> CatalogEngineResult<(ZoneEvidenceKind, bool)> {
    let bytes = inscription_bytes(value)?;
    let Ok(block) = borsh::from_slice::<SequencerBlock>(&bytes) else {
        return Ok((ZoneEvidenceKind::RawInscription, false));
    };
    let hashable = HashableBlockData::from(block.clone());
    let encoded = borsh::to_vec(&hashable).map_err(|error| {
        CatalogEngineError::InvalidBlock(format!("failed to re-encode Sequencer block: {error}"))
    })?;
    const PREFIX: &[u8; 32] = b"/LEE/v0.3/Message/Block/\x00\x00\x00\x00\x00\x00\x00\x00";
    let mut hasher = Sha256::new();
    hasher.update(PREFIX);
    hasher.update(encoded);
    let computed: [u8; 32] = hasher.finalize().into();
    if computed == block.header.hash.0 {
        Ok((ZoneEvidenceKind::SequencerBlock, false))
    } else {
        Ok((ZoneEvidenceKind::RawInscription, true))
    }
}

fn inscription_bytes(value: &Value) -> CatalogEngineResult<Vec<u8>> {
    match value {
        Value::String(value) => {
            let value = value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"))
                .unwrap_or(value);
            hex::decode(value).map_err(|error| {
                CatalogEngineError::InvalidBlock(format!(
                    "ChannelInscribe inscription is not hexadecimal: {error}"
                ))
            })
        }
        Value::Array(values) => values
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .and_then(|value| u8::try_from(value).ok())
                    .ok_or_else(|| {
                        CatalogEngineError::InvalidBlock(
                            "ChannelInscribe byte array contains a non-byte value".to_owned(),
                        )
                    })
            })
            .collect(),
        _ => Err(CatalogEngineError::InvalidBlock(
            "ChannelInscribe inscription is not bytes".to_owned(),
        )),
    }
}

fn ingest_observation(
    working: &mut WorkingCatalog,
    block: &CatalogL1Block,
    segment_id: &str,
    target_slot: u64,
    observation: ChannelObservation,
    context: CatalogEngineContext,
) -> CatalogEngineResult<()> {
    let (evidence_kind, evidence_use, parent_is_root, conflicting) = match &observation.kind {
        ChannelObservationKind::Configuration { .. } => (
            ZoneEvidenceKind::ChannelConfiguration,
            CatalogEvidenceUse::PointSnapshot,
            false,
            false,
        ),
        ChannelObservationKind::Inscription {
            evidence_kind,
            parent_is_root,
            conflicting,
            ..
        } => (
            *evidence_kind,
            CatalogEvidenceUse::Presence,
            *parent_is_root,
            *conflicting,
        ),
        ChannelObservationKind::Operation => (
            ZoneEvidenceKind::ChannelOperation,
            CatalogEvidenceUse::ReplayContribution,
            false,
            false,
        ),
    };
    let primary = evidence_reference(&observation, block, segment_id, evidence_kind, evidence_use);
    let primary_is_new = insert_evidence(working, primary.clone())?;
    if parent_is_root {
        let created = evidence_reference(
            &observation,
            block,
            segment_id,
            ZoneEvidenceKind::ChannelCreated,
            CatalogEvidenceUse::Presence,
        );
        insert_evidence(working, created)?;
    }
    if !primary_is_new {
        return Ok(());
    }

    let observation_is_latest = working
        .evidence
        .values()
        .filter(|reference| {
            reference.channel_id == observation.channel_id
                && reference.evidence_kind != ZoneEvidenceKind::ChannelCreated
        })
        .max_by_key(|reference| evidence_order(reference))
        .is_some_and(|reference| reference.evidence_id == primary.evidence_id);
    let configuration_is_latest = matches!(
        &observation.kind,
        ChannelObservationKind::Configuration { .. }
    ) && working
        .evidence
        .values()
        .filter(|reference| {
            reference.channel_id == observation.channel_id
                && reference.evidence_kind == ZoneEvidenceKind::ChannelConfiguration
        })
        .max_by_key(|reference| evidence_order(reference))
        .is_some_and(|reference| reference.evidence_id == primary.evidence_id);

    let record = working
        .zones
        .entry(observation.channel_id.clone())
        .or_insert_with(|| ZoneCatalogRecord {
            channel_id: observation.channel_id.clone(),
            observed_label: None,
            l1_channel: L1ChannelSummary {
                tip_slot: Some(block.checkpoint.slot),
                tip_hash: None,
                lib_slot: Some(target_slot),
                balance: None,
                key_count: None,
                withdraw_threshold: None,
                operation_count: 0,
                finality_state: L1FinalityState::Final,
            },
            sequencer_committee: None,
            classification: ZoneClassificationCounters {
                channel_operations: 0,
                recognized_l2_blocks: 0,
                raw_inscriptions: 0,
                conflicting_evidence: false,
            },
            first_seen_slot: block.checkpoint.slot,
            last_seen_slot: block.checkpoint.slot,
            latest_evidence_id: primary.evidence_id.clone(),
            evidence_count: 0,
            snapshot_provenance: super::model::CatalogSnapshotProvenance {
                origin: CatalogSnapshotOrigin::ReplayDerived,
                coverage_segment_id: segment_id.to_owned(),
                observed_slot: block.checkpoint.slot,
                source_revision: context.source_revision,
            },
            updated_at_unix: context.updated_at_unix,
        });
    record.first_seen_slot = record.first_seen_slot.min(block.checkpoint.slot);
    record.last_seen_slot = record.last_seen_slot.max(block.checkpoint.slot);
    record.l1_channel.tip_slot = Some(
        record
            .l1_channel
            .tip_slot
            .unwrap_or_default()
            .max(block.checkpoint.slot),
    );
    record.l1_channel.lib_slot = Some(target_slot);
    record.l1_channel.finality_state = L1FinalityState::Final;
    record.classification.conflicting_evidence |= conflicting;
    record.updated_at_unix = context.updated_at_unix;

    match observation.kind {
        ChannelObservationKind::Configuration {
            keys,
            withdraw_threshold,
        } => {
            if configuration_is_latest {
                let key_count = u64::try_from(keys.len()).map_err(|_| {
                    CatalogEngineError::Overflow("Channel key count exceeds u64".to_owned())
                })?;
                let active_member = record
                    .sequencer_committee
                    .as_ref()
                    .and_then(|committee| committee.active_member.clone())
                    .filter(|active| keys.contains(active));
                record.l1_channel.key_count = Some(key_count);
                record.l1_channel.withdraw_threshold = Some(withdraw_threshold);
                record.sequencer_committee = Some(SequencerCommitteeSummary {
                    members: keys,
                    active_member,
                    observed_at_slot: Some(block.checkpoint.slot),
                });
            }
        }
        ChannelObservationKind::Inscription {
            signer,
            parent_is_root,
            ..
        } => match record.sequencer_committee.as_mut() {
            Some(committee) if observation_is_latest && committee.members.contains(&signer) => {
                committee.active_member = Some(signer);
                committee.observed_at_slot = Some(block.checkpoint.slot);
            }
            None if parent_is_root && observation_is_latest => {
                record.l1_channel.key_count = Some(1);
                record.sequencer_committee = Some(SequencerCommitteeSummary {
                    members: vec![signer.clone()],
                    active_member: Some(signer),
                    observed_at_slot: Some(block.checkpoint.slot),
                });
            }
            Some(_) | None => {}
        },
        ChannelObservationKind::Operation => {}
    }
    Ok(())
}

fn evidence_reference(
    observation: &ChannelObservation,
    block: &CatalogL1Block,
    segment_id: &str,
    evidence_kind: ZoneEvidenceKind,
    evidence_use: CatalogEvidenceUse,
) -> ZoneEvidenceReference {
    ZoneEvidenceReference {
        evidence_id: format!(
            "evidence-{}-{}-{}",
            observation.transaction_hash,
            observation.operation_index,
            evidence_kind_tag(evidence_kind)
        ),
        channel_id: observation.channel_id.clone(),
        coverage_segment_id: segment_id.to_owned(),
        l1_slot: block.checkpoint.slot,
        block_id: block.checkpoint.block_id.clone(),
        transaction_hash: Some(observation.transaction_hash.clone()),
        operation_index: observation.operation_index,
        message_id: None,
        evidence_kind,
        evidence_use,
    }
}

fn insert_evidence(
    working: &mut WorkingCatalog,
    evidence: ZoneEvidenceReference,
) -> CatalogEngineResult<bool> {
    if let Some(existing) = working.evidence.get(&evidence.evidence_id) {
        if existing == &evidence {
            return Ok(false);
        }
        return Err(CatalogEngineError::SourceInconsistent(format!(
            "evidence id {} resolved to conflicting observations",
            evidence.evidence_id
        )));
    }
    working
        .evidence
        .insert(evidence.evidence_id.clone(), evidence);
    Ok(true)
}

fn recompute_zone_records(
    working: &mut WorkingCatalog,
    channels: &BTreeSet<String>,
    target_slot: u64,
    context: CatalogEngineContext,
) -> CatalogEngineResult<()> {
    for channel_id in channels {
        let references = working
            .evidence
            .values()
            .filter(|reference| reference.channel_id == *channel_id)
            .cloned()
            .collect::<Vec<_>>();
        let primary = references
            .iter()
            .filter(|reference| reference.evidence_kind != ZoneEvidenceKind::ChannelCreated)
            .collect::<Vec<_>>();
        let latest = references
            .iter()
            .max_by_key(|reference| evidence_order(reference))
            .ok_or_else(|| {
                CatalogEngineError::InvalidState(format!(
                    "Zone {channel_id} has no evidence after ingestion"
                ))
            })?;
        let latest_configuration = references
            .iter()
            .filter(|reference| reference.evidence_kind == ZoneEvidenceKind::ChannelConfiguration)
            .max_by_key(|reference| evidence_order(reference));
        let operation_count = u64::try_from(primary.len()).map_err(|_| {
            CatalogEngineError::Overflow("Channel operation count exceeds u64".to_owned())
        })?;
        let recognized_l2_blocks = count_evidence(&references, ZoneEvidenceKind::SequencerBlock)?;
        let raw_inscriptions = count_evidence(&references, ZoneEvidenceKind::RawInscription)?;
        let evidence_count = u64::try_from(references.len()).map_err(|_| {
            CatalogEngineError::Overflow("Zone evidence count exceeds u64".to_owned())
        })?;
        let first_seen = primary
            .iter()
            .map(|reference| reference.l1_slot)
            .min()
            .unwrap_or(latest.l1_slot);
        let last_seen = primary
            .iter()
            .map(|reference| reference.l1_slot)
            .max()
            .unwrap_or(latest.l1_slot);
        let record = working.zones.get_mut(channel_id).ok_or_else(|| {
            CatalogEngineError::InvalidState(format!("Zone {channel_id} is missing"))
        })?;
        record.first_seen_slot = first_seen;
        record.last_seen_slot = last_seen;
        record.latest_evidence_id = latest.evidence_id.clone();
        record.evidence_count = evidence_count;
        record.classification.channel_operations = operation_count;
        record.classification.recognized_l2_blocks = recognized_l2_blocks;
        record.classification.raw_inscriptions = raw_inscriptions;
        record.classification.conflicting_evidence |=
            recognized_l2_blocks > 0 && raw_inscriptions > 0;
        record.l1_channel.operation_count = operation_count;
        record.l1_channel.tip_slot = Some(last_seen);
        record.l1_channel.lib_slot = Some(target_slot);
        record.l1_channel.finality_state = L1FinalityState::Final;
        record.snapshot_provenance.origin = if latest_configuration.is_some_and(|configuration| {
            configuration.coverage_segment_id == latest.coverage_segment_id
        }) {
            CatalogSnapshotOrigin::FullConfiguration
        } else {
            CatalogSnapshotOrigin::ReplayDerived
        };
        record.snapshot_provenance.coverage_segment_id = latest.coverage_segment_id.clone();
        record.snapshot_provenance.observed_slot = last_seen;
        record.snapshot_provenance.source_revision = context.source_revision;
        record.updated_at_unix = context.updated_at_unix;
    }
    Ok(())
}

fn count_evidence(
    references: &[ZoneEvidenceReference],
    kind: ZoneEvidenceKind,
) -> CatalogEngineResult<u64> {
    u64::try_from(
        references
            .iter()
            .filter(|reference| reference.evidence_kind == kind)
            .count(),
    )
    .map_err(|_| CatalogEngineError::Overflow("evidence counter exceeds u64".to_owned()))
}

fn evidence_order(reference: &ZoneEvidenceReference) -> (u64, &str, u32, u8) {
    (
        reference.l1_slot,
        reference.transaction_hash.as_deref().unwrap_or_default(),
        reference.operation_index,
        evidence_kind_rank(reference.evidence_kind),
    )
}

const fn evidence_kind_rank(kind: ZoneEvidenceKind) -> u8 {
    match kind {
        ZoneEvidenceKind::ChannelCreated => 0,
        ZoneEvidenceKind::ChannelConfiguration => 1,
        ZoneEvidenceKind::ChannelOperation => 2,
        ZoneEvidenceKind::RawInscription => 3,
        ZoneEvidenceKind::SequencerBlock => 4,
    }
}

const fn evidence_kind_tag(kind: ZoneEvidenceKind) -> &'static str {
    match kind {
        ZoneEvidenceKind::ChannelCreated => "created",
        ZoneEvidenceKind::ChannelConfiguration => "config",
        ZoneEvidenceKind::ChannelOperation => "operation",
        ZoneEvidenceKind::SequencerBlock => "sequencer",
        ZoneEvidenceKind::RawInscription => "raw",
    }
}

fn required_channel_id(value: &Value, fields: &[&str]) -> CatalogEngineResult<String> {
    fields
        .iter()
        .find_map(|field| value.get(*field))
        .and_then(Value::as_str)
        .ok_or_else(|| CatalogEngineError::InvalidBlock("Channel id is missing".to_owned()))
        .and_then(|value| canonical_hex(value, "Channel id"))
}

fn canonical_hex(value: &str, label: &str) -> CatalogEngineResult<String> {
    let value = value.trim();
    let value = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
        .unwrap_or(value);
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(CatalogEngineError::InvalidBlock(format!(
            "{label} must be 32-byte hexadecimal text"
        )));
    }
    Ok(value.to_ascii_lowercase())
}

fn canonical_local_text(value: &str, label: &str) -> CatalogEngineResult<String> {
    let value = value.trim();
    if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(CatalogEngineError::InvalidBlock(format!(
            "{label} is invalid"
        )));
    }
    Ok(value.to_owned())
}

fn parse_opcode(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| {
        value.as_str().and_then(|value| {
            let value = value.trim();
            if let Some(value) = value
                .strip_prefix("0x")
                .or_else(|| value.strip_prefix("0X"))
            {
                u64::from_str_radix(value, 16).ok()
            } else {
                value.parse().ok()
            }
        })
    })
}

fn integer_text(value: &Value) -> Option<String> {
    match value {
        Value::Number(value) => Some(value.to_string()),
        Value::String(value) if !value.trim().is_empty() => Some(value.trim().to_owned()),
        Value::Null | Value::Bool(_) | Value::String(_) | Value::Array(_) | Value::Object(_) => {
            None
        }
    }
}

#[cfg(test)]
mod tests;
