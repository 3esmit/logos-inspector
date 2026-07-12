use serde::{Deserialize, Serialize};

use super::{CoveragePrefixStatus, ZoneKind};
use crate::inspection::catalog::{
    CatalogEvidenceUse, CatalogSnapshot, CatalogSnapshotOrigin, ZoneCatalogRecord,
    ZoneEvidenceKind, ZoneEvidenceReference,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneClassificationEvidence {
    pub recognized_l2_evidence: bool,
    pub configured_sequencer_link: bool,
    pub raw_inscription_evidence: bool,
    pub l2_absence_is_covered: bool,
    pub conflicting_evidence: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneFactGates {
    pub presence_facts: bool,
    pub point_snapshot_facts: bool,
    pub replay_facts: bool,
    pub absence_facts: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CatalogZoneClassification {
    pub kind: ZoneKind,
    pub evidence: ZoneClassificationEvidence,
    pub fact_gates: ZoneFactGates,
}

#[must_use]
pub fn classify_zone(evidence: ZoneClassificationEvidence) -> ZoneKind {
    if evidence.conflicting_evidence {
        return ZoneKind::Unknown;
    }
    if evidence.recognized_l2_evidence || evidence.configured_sequencer_link {
        return ZoneKind::SequencerZone;
    }
    if evidence.raw_inscription_evidence && evidence.l2_absence_is_covered {
        return ZoneKind::DataChannel;
    }
    ZoneKind::Unknown
}

#[must_use]
pub fn classify_catalog_zone(
    snapshot: &CatalogSnapshot,
    record: &ZoneCatalogRecord,
    configured_sequencer_link: bool,
) -> CatalogZoneClassification {
    let fact_gates = catalog_fact_gates(snapshot, record);
    let recognized_l2_reference = channel_evidence(snapshot, record).any(|reference| {
        reference.evidence_kind == ZoneEvidenceKind::SequencerBlock
            && evidence_has_valid_segment(snapshot, reference)
    });
    let authoritative_committee = record
        .sequencer_committee
        .as_ref()
        .is_some_and(|committee| !committee.members.is_empty())
        && (fact_gates.point_snapshot_facts || fact_gates.replay_facts);
    let raw_inscription_reference = channel_evidence(snapshot, record).any(|reference| {
        reference.evidence_kind == ZoneEvidenceKind::RawInscription
            && evidence_has_valid_segment(snapshot, reference)
    });
    let evidence = ZoneClassificationEvidence {
        recognized_l2_evidence: (record.classification.recognized_l2_blocks > 0
            && recognized_l2_reference)
            || authoritative_committee,
        configured_sequencer_link,
        raw_inscription_evidence: record.classification.raw_inscriptions > 0
            && raw_inscription_reference,
        l2_absence_is_covered: fact_gates.absence_facts,
        conflicting_evidence: record.classification.conflicting_evidence,
    };
    CatalogZoneClassification {
        kind: classify_zone(evidence),
        evidence,
        fact_gates,
    }
}

#[must_use]
pub fn catalog_fact_gates(snapshot: &CatalogSnapshot, record: &ZoneCatalogRecord) -> ZoneFactGates {
    let presence_facts = channel_evidence(snapshot, record)
        .any(|reference| evidence_has_valid_segment(snapshot, reference));
    let Some(snapshot_segment) = snapshot
        .segments
        .iter()
        .find(|segment| segment.segment_id == record.snapshot_provenance.coverage_segment_id)
    else {
        return ZoneFactGates {
            presence_facts,
            point_snapshot_facts: false,
            replay_facts: false,
            absence_facts: false,
        };
    };

    let point_snapshot_facts = matches!(
        record.snapshot_provenance.origin,
        CatalogSnapshotOrigin::PointSnapshot | CatalogSnapshotOrigin::FullConfiguration
    ) && channel_evidence(snapshot, record).any(|reference| {
        reference.coverage_segment_id == snapshot_segment.segment_id
            && reference.evidence_use == CatalogEvidenceUse::PointSnapshot
            && evidence_has_valid_segment(snapshot, reference)
    });
    let all_lifecycle_evidence_is_connected = channel_evidence(snapshot, record).all(|reference| {
        reference.coverage_segment_id == snapshot_segment.segment_id
            && reference.l1_slot <= record.snapshot_provenance.observed_slot
            && evidence_has_valid_segment(snapshot, reference)
    });
    let creation_is_covered = catalog_has_connected_genesis_prefix(snapshot, snapshot_segment)
        || channel_evidence(snapshot, record).any(|reference| {
            reference.coverage_segment_id == snapshot_segment.segment_id
                && reference.evidence_kind == ZoneEvidenceKind::ChannelCreated
                && reference.l1_slot <= record.first_seen_slot
                && evidence_has_valid_segment(snapshot, reference)
        });
    let snapshot_is_in_segment = record.first_seen_slot >= snapshot_segment.floor.slot
        && record.first_seen_slot <= record.snapshot_provenance.observed_slot
        && record.last_seen_slot <= record.snapshot_provenance.observed_slot
        && record.snapshot_provenance.observed_slot <= snapshot_segment.frontier.slot;
    let lifecycle_reaches_target = current_catalog_target(snapshot).is_some_and(|target| {
        snapshot_segment.reaches_target_lib && &snapshot_segment.frontier == target
    });
    let lifecycle_history_is_connected =
        snapshot_is_in_segment && all_lifecycle_evidence_is_connected && creation_is_covered;

    ZoneFactGates {
        presence_facts,
        point_snapshot_facts,
        replay_facts: record.snapshot_provenance.origin == CatalogSnapshotOrigin::ReplayDerived
            && lifecycle_history_is_connected,
        absence_facts: lifecycle_history_is_connected && lifecycle_reaches_target,
    }
}

fn channel_evidence<'a>(
    snapshot: &'a CatalogSnapshot,
    record: &'a ZoneCatalogRecord,
) -> impl Iterator<Item = &'a ZoneEvidenceReference> {
    snapshot
        .evidence
        .iter()
        .filter(|reference| reference.channel_id == record.channel_id)
}

fn evidence_has_valid_segment(
    snapshot: &CatalogSnapshot,
    reference: &ZoneEvidenceReference,
) -> bool {
    snapshot.segments.iter().any(|segment| {
        segment.segment_id == reference.coverage_segment_id
            && reference.l1_slot >= segment.floor.slot
            && reference.l1_slot <= segment.frontier.slot
    })
}

fn catalog_has_connected_genesis_prefix(
    snapshot: &CatalogSnapshot,
    snapshot_segment: &crate::inspection::catalog::CoverageSegment,
) -> bool {
    snapshot.frontier.as_ref().is_some_and(|frontier| {
        frontier.prefix_status == CoveragePrefixStatus::Complete
            && frontier.coverage_floor == Some(0)
            && snapshot.gaps.is_empty()
            && snapshot.segments.len() == 1
            && snapshot_segment.floor.slot == 0
    })
}

fn current_catalog_target(
    snapshot: &CatalogSnapshot,
) -> Option<&crate::inspection::catalog::CatalogBlockReference> {
    snapshot
        .traversal
        .as_ref()
        .and_then(|traversal| traversal.target_lib.as_ref())
        .or_else(|| {
            snapshot
                .frontier
                .as_ref()
                .and_then(|frontier| frontier.observed_lib.as_ref())
        })
}

#[cfg(test)]
mod tests {
    use anyhow::{Context as _, Result, ensure};

    use super::*;
    use crate::inspection::zones::fixtures::{
        complete_replay_catalog, partial_raw_catalog_with_connected_lifecycle,
        partial_raw_catalog_without_connected_lifecycle, point_snapshot_catalog_across_gap,
        sequencer_catalog_across_gap,
    };

    fn evidence() -> ZoneClassificationEvidence {
        ZoneClassificationEvidence {
            recognized_l2_evidence: false,
            configured_sequencer_link: false,
            raw_inscription_evidence: false,
            l2_absence_is_covered: false,
            conflicting_evidence: false,
        }
    }

    #[test]
    fn positive_l2_evidence_classifies_sequencer_zone_without_complete_coverage() {
        let evidence = ZoneClassificationEvidence {
            recognized_l2_evidence: true,
            ..evidence()
        };

        assert_eq!(classify_zone(evidence), ZoneKind::SequencerZone);
    }

    #[test]
    fn configured_sequencer_link_preserves_zone_kind_during_source_failure() {
        let evidence = ZoneClassificationEvidence {
            configured_sequencer_link: true,
            ..evidence()
        };

        assert_eq!(classify_zone(evidence), ZoneKind::SequencerZone);
    }

    #[test]
    fn raw_evidence_requires_covered_l2_absence() {
        let incomplete = ZoneClassificationEvidence {
            raw_inscription_evidence: true,
            ..evidence()
        };
        let covered = ZoneClassificationEvidence {
            l2_absence_is_covered: true,
            ..incomplete
        };

        assert_eq!(classify_zone(incomplete), ZoneKind::Unknown);
        assert_eq!(classify_zone(covered), ZoneKind::DataChannel);
    }

    #[test]
    fn missing_sources_and_activity_are_not_classification_evidence() {
        assert_eq!(classify_zone(evidence()), ZoneKind::Unknown);
    }

    #[test]
    fn conflicting_evidence_remains_unknown() {
        let evidence = ZoneClassificationEvidence {
            recognized_l2_evidence: true,
            configured_sequencer_link: true,
            raw_inscription_evidence: true,
            l2_absence_is_covered: true,
            conflicting_evidence: true,
        };

        assert_eq!(classify_zone(evidence), ZoneKind::Unknown);
    }

    #[test]
    fn positive_l2_presence_survives_an_unresolved_catalog_gap() -> Result<()> {
        let snapshot = sequencer_catalog_across_gap();
        let record = snapshot
            .zones
            .first()
            .context("Zone fixture should exist")?;

        let classification = classify_catalog_zone(&snapshot, record, false);

        ensure!(classification.kind == ZoneKind::SequencerZone);
        ensure!(classification.fact_gates.presence_facts);
        ensure!(!classification.fact_gates.point_snapshot_facts);
        ensure!(!classification.fact_gates.replay_facts);
        ensure!(!classification.fact_gates.absence_facts);
        Ok(())
    }

    #[test]
    fn raw_presence_without_covered_lifecycle_remains_unknown() -> Result<()> {
        let snapshot = partial_raw_catalog_without_connected_lifecycle();
        let record = snapshot
            .zones
            .first()
            .context("Zone fixture should exist")?;

        let classification = classify_catalog_zone(&snapshot, record, false);

        ensure!(classification.kind == ZoneKind::Unknown);
        ensure!(classification.evidence.raw_inscription_evidence);
        ensure!(!classification.evidence.l2_absence_is_covered);
        ensure!(!classification.fact_gates.replay_facts);
        Ok(())
    }

    #[test]
    fn connected_zone_lifecycle_allows_data_channel_classification_under_partial_prefix()
    -> Result<()> {
        let snapshot = partial_raw_catalog_with_connected_lifecycle();
        let record = snapshot
            .zones
            .first()
            .context("Zone fixture should exist")?;

        let classification = classify_catalog_zone(&snapshot, record, false);

        ensure!(classification.kind == ZoneKind::DataChannel);
        ensure!(classification.fact_gates.replay_facts);
        ensure!(classification.fact_gates.absence_facts);
        Ok(())
    }

    #[test]
    fn explicit_point_snapshot_survives_gap_without_enabling_replay_or_absence() -> Result<()> {
        let snapshot = point_snapshot_catalog_across_gap();
        let record = snapshot
            .zones
            .first()
            .context("Zone fixture should exist")?;

        let classification = classify_catalog_zone(&snapshot, record, false);

        ensure!(classification.kind == ZoneKind::SequencerZone);
        ensure!(classification.fact_gates.point_snapshot_facts);
        ensure!(!classification.fact_gates.replay_facts);
        ensure!(!classification.fact_gates.absence_facts);
        Ok(())
    }

    #[test]
    fn complete_prefix_enables_connected_replay_without_explicit_creation_reference() -> Result<()>
    {
        let mut snapshot = complete_replay_catalog();
        snapshot
            .evidence
            .retain(|reference| reference.evidence_kind != ZoneEvidenceKind::ChannelCreated);
        if let Some(record) = snapshot.zones.first_mut() {
            record.evidence_count = 1;
            record.classification.channel_operations = 1;
        }
        let record = snapshot
            .zones
            .first()
            .context("Zone fixture should exist")?;

        let classification = classify_catalog_zone(&snapshot, record, false);

        ensure!(classification.kind == ZoneKind::SequencerZone);
        ensure!(classification.fact_gates.replay_facts);
        ensure!(classification.fact_gates.absence_facts);
        Ok(())
    }

    #[test]
    fn advancing_target_keeps_prior_replay_but_disables_current_absence() -> Result<()> {
        let mut snapshot = complete_replay_catalog();
        if let Some(traversal) = snapshot.traversal.as_mut() {
            traversal.target_lib = Some(crate::inspection::catalog::CatalogBlockReference {
                slot: 12,
                block_id: "c".repeat(64),
            });
        }
        let record = snapshot
            .zones
            .first()
            .context("Zone fixture should exist")?;

        let classification = classify_catalog_zone(&snapshot, record, false);

        ensure!(classification.fact_gates.replay_facts);
        ensure!(!classification.fact_gates.absence_facts);
        Ok(())
    }
}
