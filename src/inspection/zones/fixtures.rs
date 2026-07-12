use super::*;
use crate::inspection::catalog::{
    CatalogBlockCheckpoint, CatalogBlockReference, CatalogEvidenceUse, CatalogFrontier,
    CatalogIdentityAssurance, CatalogMetadata, CatalogSnapshot, CatalogSnapshotOrigin,
    CatalogSnapshotProvenance, CatalogTraversal, CoverageGap, CoverageGapReason, CoverageGapStatus,
    CoverageSegment, ZoneCatalogRecord, ZoneClassificationCounters, ZoneEvidenceKind,
    ZoneEvidenceReference,
};
use crate::source_routing::channel_sources::{
    ChannelSourceConfig, ChannelSourceTarget, ConfiguredIndexerSource, ConfiguredSequencerSource,
    PersistedSequencerAttestation,
};

pub(super) fn linked_sequencer_zone() -> ZoneSummary {
    ZoneSummary {
        channel_id: repeated_id('8'),
        display: ZoneDisplay {
            title: "Paradox Computer".to_owned(),
            alias: Some("Paradox Computer".to_owned()),
            short_channel_id: "8888...8888".to_owned(),
            alias_source: ZoneAliasSource::KnownStatic,
        },
        l1_channel: L1ChannelSummary {
            tip_slot: Some(187_085),
            tip_hash: Some(repeated_id('9')),
            lib_slot: Some(186_706),
            balance: Some("5400000000".to_owned()),
            key_count: Some(2),
            withdraw_threshold: Some("2".to_owned()),
            operation_count: 42,
            finality_state: L1FinalityState::Finalizing,
        },
        settlement_link: SettlementLinkSummary {
            status: SettlementLinkStatus::Linked,
            source: SettlementLinkSource::Configured,
            selected_sequencer_source_id: Some("seq-primary".to_owned()),
            indexer_source_id: Some("indexer-main".to_owned()),
            lag_blocks: Some(2),
            lag_slots: Some(12),
        },
        activity_state: ZoneActivityState::Active,
        activity_detail: ZoneActivityDetail {
            reason: "L2 blocks and L1 settlement are moving.".to_owned(),
            last_seen_unix: Some(1_782_985_805),
            last_l1_slot: Some(187_085),
            last_l2_block_id: Some(1_099),
        },
        provenance: complete_provenance(),
        facts: ZoneFacts::SequencerZone {
            l2_zone: L2ZoneSummary {
                source_status: L2SourceStatus::Reachable,
                selected_source_id: Some("seq-primary".to_owned()),
                configured_source_count: 2,
                observed_source_count: 2,
                latest_block_id: Some(1_099),
                latest_block_hash: Some(repeated_id('b')),
                safe_block_id: Some(1_097),
                finalized_block_id: Some(1_088),
                finality_state: L2FinalityState::Safe,
                agreement_state: SequencerAgreementState::Converged,
            },
            sequencer_committee: SequencerCommitteeSummary {
                members: vec![repeated_id('a'), repeated_id('c')],
                active_member: Some(repeated_id('a')),
                observed_at_slot: Some(187_085),
            },
        },
    }
}

pub(super) fn l1_only_sequencer_zone() -> ZoneSummary {
    ZoneSummary {
        channel_id: repeated_id('1'),
        display: ZoneDisplay {
            title: "0101...0101".to_owned(),
            alias: None,
            short_channel_id: "0101...0101".to_owned(),
            alias_source: ZoneAliasSource::None,
        },
        l1_channel: L1ChannelSummary {
            tip_slot: Some(187_036),
            tip_hash: Some(repeated_id('5')),
            lib_slot: Some(186_706),
            balance: Some("800000".to_owned()),
            key_count: Some(1),
            withdraw_threshold: Some("1".to_owned()),
            operation_count: 8,
            finality_state: L1FinalityState::Finalizing,
        },
        settlement_link: SettlementLinkSummary {
            status: SettlementLinkStatus::L1Only,
            source: SettlementLinkSource::L1Scan,
            selected_sequencer_source_id: None,
            indexer_source_id: None,
            lag_blocks: None,
            lag_slots: Some(330),
        },
        activity_state: ZoneActivityState::Degraded,
        activity_detail: ZoneActivityDetail {
            reason: "L1 settlement exists; no Sequencer source is configured.".to_owned(),
            last_seen_unix: Some(1_782_985_500),
            last_l1_slot: Some(187_036),
            last_l2_block_id: None,
        },
        provenance: complete_provenance(),
        facts: ZoneFacts::SequencerZone {
            l2_zone: L2ZoneSummary {
                source_status: L2SourceStatus::Unconfigured,
                selected_source_id: None,
                configured_source_count: 0,
                observed_source_count: 0,
                latest_block_id: None,
                latest_block_hash: None,
                safe_block_id: None,
                finalized_block_id: None,
                finality_state: L2FinalityState::Unknown,
                agreement_state: SequencerAgreementState::Unconfigured,
            },
            sequencer_committee: SequencerCommitteeSummary {
                members: vec![repeated_id('d')],
                active_member: Some(repeated_id('d')),
                observed_at_slot: Some(187_036),
            },
        },
    }
}

pub(super) fn data_channel() -> ZoneSummary {
    ZoneSummary {
        channel_id: "d4f779ae00112233445566778899aabbccddeeff00112233445566778899bf63".to_owned(),
        display: ZoneDisplay {
            title: "Guest data drop".to_owned(),
            alias: Some("Guest data drop".to_owned()),
            short_channel_id: "d4f7...bf63".to_owned(),
            alias_source: ZoneAliasSource::Configured,
        },
        l1_channel: L1ChannelSummary {
            tip_slot: Some(177_635),
            tip_hash: Some(repeated_id('0')),
            lib_slot: Some(177_700),
            balance: Some("12000000".to_owned()),
            key_count: Some(1),
            withdraw_threshold: Some("1".to_owned()),
            operation_count: 3,
            finality_state: L1FinalityState::Final,
        },
        settlement_link: SettlementLinkSummary {
            status: SettlementLinkStatus::RawData,
            source: SettlementLinkSource::L1Scan,
            selected_sequencer_source_id: None,
            indexer_source_id: None,
            lag_blocks: None,
            lag_slots: Some(0),
        },
        activity_state: ZoneActivityState::Raw,
        activity_detail: ZoneActivityDetail {
            reason: "Raw L1 inscriptions have no recognized L2 block evidence.".to_owned(),
            last_seen_unix: Some(1_782_985_805),
            last_l1_slot: Some(177_635),
            last_l2_block_id: None,
        },
        provenance: complete_provenance(),
        facts: ZoneFacts::DataChannel {
            raw_activity: RawActivitySummary {
                inscription_count: 3,
                latest_slot: Some(177_635),
                latest_payload_size: Some(31),
                finality_state: L1FinalityState::Final,
            },
        },
    }
}

pub(super) fn unknown_l1_channel() -> ZoneSummary {
    ZoneSummary {
        channel_id: repeated_id('f'),
        display: ZoneDisplay {
            title: "f0f0...f0f0".to_owned(),
            alias: None,
            short_channel_id: "f0f0...f0f0".to_owned(),
            alias_source: ZoneAliasSource::None,
        },
        l1_channel: L1ChannelSummary {
            tip_slot: Some(176_400),
            tip_hash: Some(repeated_id('6')),
            lib_slot: Some(177_700),
            balance: Some("0".to_owned()),
            key_count: Some(0),
            withdraw_threshold: None,
            operation_count: 1,
            finality_state: L1FinalityState::Unknown,
        },
        settlement_link: SettlementLinkSummary {
            status: SettlementLinkStatus::Unknown,
            source: SettlementLinkSource::None,
            selected_sequencer_source_id: None,
            indexer_source_id: None,
            lag_blocks: None,
            lag_slots: None,
        },
        activity_state: ZoneActivityState::Unknown,
        activity_detail: ZoneActivityDetail {
            reason: "L1 evidence lacks raw or L2 classification certainty.".to_owned(),
            last_seen_unix: Some(1_782_980_000),
            last_l1_slot: Some(176_400),
            last_l2_block_id: None,
        },
        provenance: partial_provenance(),
        facts: ZoneFacts::Unknown,
    }
}

fn complete_provenance() -> ZoneProvenance {
    ZoneProvenance {
        network_scope: Some(NetworkScope::GenesisId {
            genesis_id: repeated_id('e'),
        }),
        verification_state: CatalogVerificationState::Verified,
        coverage: ZoneCoverageProvenance {
            status: CatalogCoverageStatus::Complete,
            coverage_floor: Some(0),
            scanned_through_slot: Some(186_706),
            observed_lib_slot: Some(186_706),
            prefix_status: CoveragePrefixStatus::Complete,
            continuity_checkpoint: Some(FinalizedBlockCheckpoint {
                slot: 186_706,
                block_id: repeated_id('7'),
                parent_id: repeated_id('4'),
            }),
        },
        observed_at_unix: Some(1_782_985_805),
    }
}

fn partial_provenance() -> ZoneProvenance {
    ZoneProvenance {
        network_scope: Some(NetworkScope::GenesisId {
            genesis_id: repeated_id('e'),
        }),
        verification_state: CatalogVerificationState::Verified,
        coverage: ZoneCoverageProvenance {
            status: CatalogCoverageStatus::Partial,
            coverage_floor: Some(170_000),
            scanned_through_slot: Some(177_700),
            observed_lib_slot: Some(177_700),
            prefix_status: CoveragePrefixStatus::Unavailable,
            continuity_checkpoint: Some(FinalizedBlockCheckpoint {
                slot: 177_700,
                block_id: repeated_id('3'),
                parent_id: repeated_id('2'),
            }),
        },
        observed_at_unix: Some(1_782_985_805),
    }
}

fn repeated_id(character: char) -> String {
    character.to_string().repeat(64)
}

pub(super) fn complete_replay_catalog() -> CatalogSnapshot {
    let channel_id = repeated_id('8');
    let segment_id = "segment-main".to_owned();
    let target = block_reference(10, 'a');
    CatalogSnapshot {
        metadata: catalog_metadata(),
        frontier: Some(CatalogFrontier {
            scanned_through_slot: Some(10),
            checkpoint: Some(block_checkpoint(10, 'a', '9')),
            observed_lib: Some(target.clone()),
            coverage_floor: Some(0),
            prefix_status: CoveragePrefixStatus::Complete,
            coverage_status: CatalogCoverageStatus::Complete,
        }),
        traversal: Some(CatalogTraversal {
            target_lib: Some(target.clone()),
            ingestion_cursor: Some(target),
        }),
        zones: vec![catalog_record(CatalogRecordFixture {
            channel_id: &channel_id,
            observed_label: Some("Paradox Computer"),
            segment_id: &segment_id,
            origin: CatalogSnapshotOrigin::ReplayDerived,
            first_seen_slot: 1,
            last_seen_slot: 10,
            observed_slot: 10,
            evidence_count: 2,
            recognized_l2_blocks: 1,
            raw_inscriptions: 0,
            sequencer_committee: Some(SequencerCommitteeSummary {
                members: vec![repeated_id('c')],
                active_member: Some(repeated_id('c')),
                observed_at_slot: Some(10),
            }),
        })],
        evidence: vec![
            evidence_reference(
                "evidence-created",
                &channel_id,
                &segment_id,
                1,
                '1',
                ZoneEvidenceKind::ChannelCreated,
                CatalogEvidenceUse::Presence,
            ),
            evidence_reference(
                "evidence-l2",
                &channel_id,
                &segment_id,
                10,
                'a',
                ZoneEvidenceKind::SequencerBlock,
                CatalogEvidenceUse::ReplayContribution,
            ),
        ],
        segments: vec![CoverageSegment {
            segment_id,
            floor: block_checkpoint(0, '0', 'f'),
            frontier: block_reference(10, 'a'),
            reaches_target_lib: true,
        }],
        gaps: Vec::new(),
    }
}

pub(super) fn partial_raw_catalog_with_connected_lifecycle() -> CatalogSnapshot {
    partial_raw_catalog(true)
}

pub(super) fn partial_raw_catalog_without_connected_lifecycle() -> CatalogSnapshot {
    partial_raw_catalog(false)
}

pub(super) fn point_snapshot_catalog_across_gap() -> CatalogSnapshot {
    let channel_id = repeated_id('1');
    let upper_segment_id = "segment-upper".to_owned();
    let target = block_reference(10, 'a');
    CatalogSnapshot {
        metadata: catalog_metadata(),
        frontier: Some(CatalogFrontier {
            scanned_through_slot: Some(3),
            checkpoint: Some(block_checkpoint(3, '3', '2')),
            observed_lib: Some(target.clone()),
            coverage_floor: Some(0),
            prefix_status: CoveragePrefixStatus::Complete,
            coverage_status: CatalogCoverageStatus::Partial,
        }),
        traversal: Some(CatalogTraversal {
            target_lib: Some(target.clone()),
            ingestion_cursor: Some(target),
        }),
        zones: vec![catalog_record(CatalogRecordFixture {
            channel_id: &channel_id,
            observed_label: None,
            segment_id: &upper_segment_id,
            origin: CatalogSnapshotOrigin::PointSnapshot,
            first_seen_slot: 1,
            last_seen_slot: 8,
            observed_slot: 8,
            evidence_count: 2,
            recognized_l2_blocks: 0,
            raw_inscriptions: 0,
            sequencer_committee: Some(SequencerCommitteeSummary {
                members: vec![repeated_id('c')],
                active_member: Some(repeated_id('c')),
                observed_at_slot: Some(8),
            }),
        })],
        evidence: vec![
            evidence_reference(
                "evidence-created",
                &channel_id,
                "segment-lower",
                1,
                '1',
                ZoneEvidenceKind::ChannelCreated,
                CatalogEvidenceUse::Presence,
            ),
            evidence_reference(
                "evidence-snapshot",
                &channel_id,
                &upper_segment_id,
                8,
                '8',
                ZoneEvidenceKind::ChannelConfiguration,
                CatalogEvidenceUse::PointSnapshot,
            ),
        ],
        segments: vec![
            CoverageSegment {
                segment_id: "segment-lower".to_owned(),
                floor: block_checkpoint(0, '0', 'f'),
                frontier: block_reference(3, '3'),
                reaches_target_lib: false,
            },
            CoverageSegment {
                segment_id: upper_segment_id,
                floor: block_checkpoint(5, '5', '4'),
                frontier: block_reference(10, 'a'),
                reaches_target_lib: true,
            },
        ],
        gaps: vec![CoverageGap {
            gap_id: "gap-main".to_owned(),
            lower_segment_id: "segment-lower".to_owned(),
            lower_checkpoint: block_reference(3, '3'),
            upper_segment_id: "segment-upper".to_owned(),
            upper_block: block_reference(5, '5'),
            required_parent_id: repeated_id('4'),
            reason: CoverageGapReason::MissingParent,
            status: CoverageGapStatus::Pending,
            attempts: 1,
            first_seen_at_unix: 100,
            last_attempt_at_unix: None,
        }],
    }
}

pub(super) fn sequencer_catalog_across_gap() -> CatalogSnapshot {
    let mut snapshot = point_snapshot_catalog_across_gap();
    if let Some(record) = snapshot.zones.first_mut() {
        record.snapshot_provenance.origin = CatalogSnapshotOrigin::ReplayDerived;
        record.sequencer_committee = None;
        record.classification.recognized_l2_blocks = 1;
    }
    if let Some(reference) = snapshot.evidence.last_mut() {
        reference.evidence_kind = ZoneEvidenceKind::SequencerBlock;
        reference.evidence_use = CatalogEvidenceUse::Presence;
    }
    snapshot
}

pub(super) fn sequencer_source_config(
    network_scope: &NetworkScope,
    channel_id: &str,
) -> ChannelSourceConfig {
    let sequencer_source_id = format!("src_{}", "a".repeat(32));
    ChannelSourceConfig {
        network_scope: network_scope.clone(),
        channel_id: channel_id.to_owned(),
        config_revision: 1,
        sequencer_sources: vec![ConfiguredSequencerSource {
            source_id: sequencer_source_id.clone(),
            label: Some("Primary".to_owned()),
            target: ChannelSourceTarget::Rpc {
                endpoint: "http://127.0.0.1:3040/".to_owned(),
            },
            channel_attestation: PersistedSequencerAttestation::Pending,
        }],
        selected_sequencer_source_id: Some(sequencer_source_id),
        indexer_source: Some(ConfiguredIndexerSource {
            source_id: format!("src_{}", "b".repeat(32)),
            label: Some("Indexer".to_owned()),
            target: ChannelSourceTarget::Rpc {
                endpoint: "http://127.0.0.1:3041/".to_owned(),
            },
        }),
    }
}

fn partial_raw_catalog(include_creation: bool) -> CatalogSnapshot {
    let channel_id = repeated_id('d');
    let segment_id = "segment-partial".to_owned();
    let target = block_reference(10, 'a');
    let mut evidence = Vec::new();
    if include_creation {
        evidence.push(evidence_reference(
            "evidence-created",
            &channel_id,
            &segment_id,
            5,
            '5',
            ZoneEvidenceKind::ChannelCreated,
            CatalogEvidenceUse::Presence,
        ));
    }
    evidence.push(evidence_reference(
        "evidence-raw",
        &channel_id,
        &segment_id,
        9,
        '9',
        ZoneEvidenceKind::RawInscription,
        CatalogEvidenceUse::ReplayContribution,
    ));
    let evidence_count = u64::try_from(evidence.len()).unwrap_or(u64::MAX);
    CatalogSnapshot {
        metadata: catalog_metadata(),
        frontier: Some(CatalogFrontier {
            scanned_through_slot: Some(10),
            checkpoint: Some(block_checkpoint(10, 'a', '9')),
            observed_lib: Some(target.clone()),
            coverage_floor: Some(4),
            prefix_status: CoveragePrefixStatus::Unavailable,
            coverage_status: CatalogCoverageStatus::Partial,
        }),
        traversal: Some(CatalogTraversal {
            target_lib: Some(target.clone()),
            ingestion_cursor: Some(target),
        }),
        zones: vec![catalog_record(CatalogRecordFixture {
            channel_id: &channel_id,
            observed_label: Some("Raw Channel"),
            segment_id: &segment_id,
            origin: CatalogSnapshotOrigin::ReplayDerived,
            first_seen_slot: if include_creation { 5 } else { 9 },
            last_seen_slot: 9,
            observed_slot: 10,
            evidence_count,
            recognized_l2_blocks: 0,
            raw_inscriptions: 1,
            sequencer_committee: None,
        })],
        evidence,
        segments: vec![CoverageSegment {
            segment_id,
            floor: block_checkpoint(4, '4', '3'),
            frontier: block_reference(10, 'a'),
            reaches_target_lib: true,
        }],
        gaps: Vec::new(),
    }
}

struct CatalogRecordFixture<'a> {
    channel_id: &'a str,
    observed_label: Option<&'a str>,
    segment_id: &'a str,
    origin: CatalogSnapshotOrigin,
    first_seen_slot: u64,
    last_seen_slot: u64,
    observed_slot: u64,
    evidence_count: u64,
    recognized_l2_blocks: u64,
    raw_inscriptions: u64,
    sequencer_committee: Option<SequencerCommitteeSummary>,
}

fn catalog_record(fixture: CatalogRecordFixture<'_>) -> ZoneCatalogRecord {
    ZoneCatalogRecord {
        channel_id: fixture.channel_id.to_owned(),
        observed_label: fixture.observed_label.map(str::to_owned),
        l1_channel: L1ChannelSummary {
            tip_slot: Some(fixture.last_seen_slot),
            tip_hash: Some(repeated_id('b')),
            lib_slot: Some(10),
            balance: Some("1000".to_owned()),
            key_count: Some(1),
            withdraw_threshold: Some("1".to_owned()),
            operation_count: fixture.evidence_count,
            finality_state: L1FinalityState::Final,
        },
        sequencer_committee: fixture.sequencer_committee,
        classification: ZoneClassificationCounters {
            channel_operations: fixture.evidence_count,
            recognized_l2_blocks: fixture.recognized_l2_blocks,
            raw_inscriptions: fixture.raw_inscriptions,
            conflicting_evidence: false,
        },
        first_seen_slot: fixture.first_seen_slot,
        last_seen_slot: fixture.last_seen_slot,
        latest_evidence_id: if fixture.raw_inscriptions > 0 {
            "evidence-raw".to_owned()
        } else if fixture.recognized_l2_blocks > 0 {
            "evidence-l2".to_owned()
        } else {
            "evidence-snapshot".to_owned()
        },
        evidence_count: fixture.evidence_count,
        snapshot_provenance: CatalogSnapshotProvenance {
            origin: fixture.origin,
            coverage_segment_id: fixture.segment_id.to_owned(),
            observed_slot: fixture.observed_slot,
            source_revision: 1,
        },
        updated_at_unix: 101,
    }
}

fn evidence_reference(
    evidence_id: &str,
    channel_id: &str,
    segment_id: &str,
    l1_slot: u64,
    block_character: char,
    evidence_kind: ZoneEvidenceKind,
    evidence_use: CatalogEvidenceUse,
) -> ZoneEvidenceReference {
    ZoneEvidenceReference {
        evidence_id: evidence_id.to_owned(),
        channel_id: channel_id.to_owned(),
        coverage_segment_id: segment_id.to_owned(),
        l1_slot,
        block_id: repeated_id(block_character),
        transaction_hash: Some(repeated_id('e')),
        operation_index: 0,
        message_id: None,
        evidence_kind,
        evidence_use,
    }
}

fn catalog_metadata() -> CatalogMetadata {
    CatalogMetadata {
        catalog_file_id: "catalog_fixture".to_owned(),
        network_scope: NetworkScope::GenesisId {
            genesis_id: repeated_id('e'),
        },
        identity_aliases: Vec::new(),
        identity_assurance: CatalogIdentityAssurance::SourceAttested,
        identity_transition: None,
        catalog_revision: 1,
        created_at_unix: 100,
        updated_at_unix: 101,
    }
}

fn block_reference(slot: u64, character: char) -> CatalogBlockReference {
    CatalogBlockReference {
        slot,
        block_id: repeated_id(character),
    }
}

fn block_checkpoint(slot: u64, character: char, parent: char) -> CatalogBlockCheckpoint {
    CatalogBlockCheckpoint {
        slot,
        block_id: repeated_id(character),
        parent_id: repeated_id(parent),
    }
}
