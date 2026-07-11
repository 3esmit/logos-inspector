use super::*;

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
