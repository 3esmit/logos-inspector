.pragma library

function identity(value) {
    return String(value || "0").repeat(64)
}

function networkScope() {
    return {
        kind: "genesis_id",
        genesis_id: identity("f")
    }
}

function zones() {
    return [sequencerZone(), dataZone(), unknownZone()]
}

function sequencerZone() {
    return {
        channel_id: identity("1"),
        display: {
            title: "Devnet Settlement",
            alias: "Devnet",
            short_channel_id: "11111111...111111",
            alias_source: "configured"
        },
        l1_channel: {
            tip_slot: 187085,
            tip_hash: identity("a"),
            lib_slot: 187085,
            balance: "1200000000",
            key_count: 3,
            withdraw_threshold: "2",
            operation_count: 1842,
            finality_state: "final"
        },
        settlement_link: {
            status: "linked",
            source: "configured",
            selected_sequencer_source_id: "src_11111111111111111111111111111111",
            indexer_source_id: "src_33333333333333333333333333333333",
            lag_blocks: 1,
            lag_slots: 2
        },
        activity_state: "active",
        activity_detail: {
            reason: "Selected Sequencer reachable",
            last_seen_unix: 1783818000,
            last_l1_slot: 187085,
            last_l2_block_id: 12844
        },
        provenance: provenance("complete"),
        kind: "sequencer_zone",
        l2_zone: {
            source_status: "reachable",
            selected_source_id: "src_11111111111111111111111111111111",
            configured_source_count: 2,
            observed_source_count: 2,
            latest_block_id: 12844,
            latest_block_hash: identity("b"),
            safe_block_id: 12843,
            finalized_block_id: 12840,
            finality_state: "safe",
            agreement_state: "converged"
        },
        sequencer_committee: {
            members: [identity("2"), identity("3"), identity("4")],
            active_member: identity("2"),
            observed_at_slot: 187085
        }
    }
}

function dataZone() {
    return {
        channel_id: identity("8"),
        display: {
            title: "Archive Payloads",
            alias: "Archive",
            short_channel_id: "88888888...888888",
            alias_source: "configured"
        },
        l1_channel: {
            tip_slot: 187079,
            tip_hash: identity("c"),
            lib_slot: 187085,
            balance: "0",
            key_count: 1,
            withdraw_threshold: "1",
            operation_count: 73,
            finality_state: "final"
        },
        settlement_link: {
            status: "raw_data",
            source: "l1_scan",
            selected_sequencer_source_id: null,
            indexer_source_id: null,
            lag_blocks: null,
            lag_slots: null
        },
        activity_state: "raw",
        activity_detail: {
            reason: "Raw inscriptions observed",
            last_seen_unix: 1783817900,
            last_l1_slot: 187079,
            last_l2_block_id: null
        },
        provenance: provenance("complete"),
        kind: "data_channel",
        raw_activity: {
            inscription_count: 72,
            latest_slot: 187079,
            latest_payload_size: 4096,
            finality_state: "final"
        }
    }
}

function unknownZone() {
    return {
        channel_id: identity("4"),
        display: {
            title: "Unclassified Channel",
            alias: null,
            short_channel_id: "44444444...444444",
            alias_source: "none"
        },
        l1_channel: {
            tip_slot: 186991,
            tip_hash: identity("d"),
            lib_slot: 187085,
            balance: null,
            key_count: 1,
            withdraw_threshold: "1",
            operation_count: 2,
            finality_state: "final"
        },
        settlement_link: {
            status: "unknown",
            source: "none",
            selected_sequencer_source_id: null,
            indexer_source_id: null,
            lag_blocks: null,
            lag_slots: null
        },
        activity_state: "unknown",
        activity_detail: {
            reason: "Coverage is incomplete",
            last_seen_unix: 1783810000,
            last_l1_slot: 186991,
            last_l2_block_id: null
        },
        provenance: staleProvenance(),
        kind: "unknown"
    }
}

function detailFor(channelId) {
    const rows = zones()
    let summary = rows[0]
    for (let i = 0; i < rows.length; ++i) {
        if (rows[i].channel_id === channelId) {
            summary = rows[i]
            break
        }
    }
    const sequencer = summary.kind === "sequencer_zone"
    return {
        summary: summary,
        l1_channel_snapshot: {
            channel_tip: summary.l1_channel.tip_hash,
            keys: sequencer ? summary.sequencer_committee.members : [identity("9")],
            observed_at_slot: summary.l1_channel.tip_slot
        },
        channel_source_config: sequencer ? sourceConfig() : {
            config_revision: 0,
            selected_sequencer_source_id: null,
            sequencer_sources: [],
            indexer_source: null
        },
        source_observations: sequencer ? observations() : [],
        source_agreement: {
            state: sequencer ? "converged" : "not_applicable"
        },
        classification_evidence: {
            recognized_l2_evidence: sequencer,
            configured_sequencer_link: sequencer,
            raw_inscription_evidence: summary.kind === "data_channel",
            l2_absence_is_covered: summary.kind === "data_channel",
            conflicting_evidence: false
        },
        activity_counts: {
            l1_operations: summary.l1_channel.operation_count,
            recognized_l2_blocks: sequencer ? 12844 : 0,
            raw_inscriptions: summary.kind === "data_channel" ? 72 : 0
        },
        detail_revision: 4
    }
}

function sourceConfig() {
    return {
        config_revision: 7,
        selected_sequencer_source_id: "src_11111111111111111111111111111111",
        sequencer_sources: [{
            source_id: "src_11111111111111111111111111111111",
            label: "Primary",
            target: { kind: "rpc", endpoint: "https://sequencer.devnet.example/" },
            binding_state: "runtime_attested"
        }, {
            source_id: "src_22222222222222222222222222222222",
            label: "Local fallback",
            target: { kind: "rpc", endpoint: "http://127.0.0.1:3040/" },
            binding_state: "persisted_attested"
        }],
        indexer_source: {
            source_id: "src_33333333333333333333333333333333",
            label: "Finalized history",
            target: { kind: "module", module_id: "lez_indexer_module" },
            binding_state: null
        }
    }
}

function observations() {
    return [{
        source_id: "src_11111111111111111111111111111111",
        role: "sequencer",
        binding_state: "runtime_attested",
        health: "reachable",
        reported_channel_id: identity("1"),
        head_block_id: 12844,
        head_block_hash: identity("b"),
        head_parent_hash: identity("a"),
        observed_at_unix: 1783818000,
        latency_millis: 18,
        last_error: null
    }, {
        source_id: "src_22222222222222222222222222222222",
        role: "sequencer",
        binding_state: "persisted_attested",
        health: "stale",
        reported_channel_id: identity("1"),
        head_block_id: 12842,
        head_block_hash: identity("e"),
        head_parent_hash: identity("d"),
        observed_at_unix: 1783817900,
        latency_millis: 9,
        last_error: "Source head is behind selected Sequencer"
    }, {
        source_id: "src_33333333333333333333333333333333",
        role: "indexer",
        binding_state: null,
        health: "reachable",
        reported_channel_id: null,
        head_block_id: 12840,
        head_block_hash: identity("f"),
        head_parent_hash: identity("e"),
        observed_at_unix: 1783817950,
        latency_millis: 3,
        last_error: null
    }]
}

function evidenceRows(channelId) {
    return [evidenceRow("evidence-config", channelId, 187070, "channel_configuration", 0),
        evidenceRow("evidence-operation", channelId, 187075, "channel_operation", 2),
        evidenceRow("evidence-raw", channelId, 187079, "raw_inscription", 0)]
}

function evidenceRow(evidenceId, channelId, slot, kind, operationIndex) {
    return {
        reference: {
            evidence_id: evidenceId,
            channel_id: channelId,
            coverage_segment_id: "segment-finalized-0",
            l1_slot: slot,
            block_id: identity("a"),
            transaction_hash: identity("c"),
            operation_index: operationIndex,
            message_id: null,
            evidence_kind: kind,
            evidence_use: kind === "channel_configuration" ? "point_snapshot"
                : (kind === "channel_operation" ? "replay_contribution" : "presence")
        },
        segment: {
            segment_id: "segment-finalized-0",
            floor_slot: 0,
            frontier_slot: 187085,
            reaches_target_lib: true
        },
        source: {
            kind: "direct_http",
            fingerprint: "sha256:" + identity("9")
        },
        finality: "final"
    }
}

function evidenceDetail(row) {
    return {
        report_kind: "zones.evidence_detail",
        schema_version: 1,
        source_revision: 3,
        network_scope: networkScope(),
        catalog_revision: 19,
        channel_id: row.reference.channel_id,
        row: row,
        operation: { opcode: row.reference.evidence_kind === "channel_configuration" ? 16 : 17 },
        payload: {
            byte_length: 4096,
            sha256: "sha256:" + identity("7"),
            encoding: row.reference.evidence_kind === "raw_inscription" ? "utf8" : "json",
            inline_text: row.reference.evidence_kind === "raw_inscription"
                ? "raw_data: archived payload\nchannel: " + row.reference.channel_id
                : "{\"channel\":\"" + row.reference.channel_id + "\",\"threshold\":2}",
            inline_base64: null,
            preview: "catalog evidence preview",
            preview_truncated: false,
            inline_truncated: false,
            session_id: null,
            warning: null
        }
    }
}

function provenance(status) {
    return {
        network_scope: networkScope(),
        verification_state: "verified",
        coverage: {
            status: status,
            coverage_floor: status === "complete" ? 0 : 180000,
            scanned_through_slot: 187085,
            observed_lib_slot: 187085,
            prefix_status: status === "complete" ? "complete" : "unavailable",
            continuity_checkpoint: null
        },
        observed_at_unix: 1783818000
    }
}

function staleProvenance() {
    const value = provenance("partial")
    value.verification_state = "cached_unverified"
    return value
}
