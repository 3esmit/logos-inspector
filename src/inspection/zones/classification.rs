use serde::{Deserialize, Serialize};

use super::ZoneKind;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ZoneClassificationEvidence {
    pub recognized_l2_evidence: bool,
    pub configured_sequencer_link: bool,
    pub raw_inscription_evidence: bool,
    pub l2_absence_is_covered: bool,
    pub conflicting_evidence: bool,
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
