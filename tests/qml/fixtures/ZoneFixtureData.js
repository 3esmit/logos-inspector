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
            indexer_source_status: "reachable",
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

function activeZoneContext(channelId) {
    const sequencer = channelId === identity("1")
    return {
        network_scope: networkScope(),
        channel_id: channelId,
        zone_kind: sequencer ? "sequencer_zone" : "data_channel",
        selected_sequencer_source_id: sequencer
            ? "src_11111111111111111111111111111111" : null,
        indexer_source_id: sequencer
            ? "src_33333333333333333333333333333333" : null,
        source_config_revision: sequencer ? 7 : 0,
        context_revision: sequencer ? 3 : 4
    }
}

function l2Source(sourceId, role, finality, retrieval) {
    return {
        source_id: sourceId,
        source_role: role,
        source_config_revision: 7,
        finality: finality,
        retrieval: retrieval || "live"
    }
}

function l2BlockRows() {
    const indexer = l2Source(
        "src_33333333333333333333333333333333",
        "indexer",
        "finalized",
        "live"
    )
    const sequencer = l2Source(
        "src_11111111111111111111111111111111",
        "sequencer",
        "provisional",
        "live"
    )
    return [{
        summary: l2BlockSummary(12844, identity("b"), identity("a"), "pending", 3),
        observations: [sequencer]
    }, {
        summary: l2BlockSummary(12842, identity("d"), identity("c"), "accepted", 2),
        observations: [indexer]
    }, {
        summary: l2BlockSummary(12842, identity("e"), identity("c"), "accepted", 2),
        observations: [sequencer]
    }, {
        summary: l2BlockSummary(12840, identity("f"), identity("e"), "finalized", 1),
        observations: [indexer, sequencer]
    }]
}

function l2BlockSummary(blockId, hashValue, parentHash, status, transactionCount) {
    return {
        block_id: blockId,
        block_hash: hashValue,
        parent_hash: parentHash,
        timestamp: 1783818000 - (12844 - blockId) * 6,
        bedrock_status: status,
        transaction_count: transactionCount
    }
}

function l2RouteReport(reportKind, source) {
    const attempts = source ? [{
        source_id: source.source_id,
        source_role: source.source_role,
        outcome: "returned",
        contribution: "payload",
        finality: source.finality,
        source_config_revision: source.source_config_revision,
        retrieval: source.retrieval
    }] : [{
        source_id: "src_33333333333333333333333333333333",
        source_role: "indexer",
        outcome: "returned",
        contribution: "finalized_prefix",
        finality: "finalized",
        source_config_revision: 7,
        retrieval: "live"
    }, {
        source_id: "src_11111111111111111111111111111111",
        source_role: "sequencer",
        outcome: "returned",
        contribution: "provisional_tail",
        finality: "provisional",
        source_config_revision: 7,
        retrieval: "live"
    }]
    return {
        report_kind: reportKind,
        schema_version: 1,
        context: activeZoneContext(identity("1")),
        request_revision: 1,
        route: {
            policy: source ? "exact_source" : "composite",
            attempts: attempts
        },
        route_completeness: source ? "single_configured" : "all_configured",
        warnings: []
    }
}

function l2Transaction(hashValue) {
    return {
        hash: hashValue,
        kind: "public",
        program_id_hex: identity("6"),
        account_ids: [identity("7"), identity("8")],
        nonces: ["18", "4"],
        instruction_data: [16, 32, 64, 128],
        bytecode_len: null,
        raw_signature_valid: true,
        message_prehash: identity("9"),
        prehash_signature_valid: true
    }
}

function l2BlockDetail(summary, sourceId) {
    const source = l2Source(
        sourceId || "src_11111111111111111111111111111111",
        sourceId && sourceId.indexOf("3333") >= 0 ? "indexer" : "sequencer",
        sourceId && sourceId.indexOf("3333") >= 0 ? "finalized" : "provisional",
        "memory_cache"
    )
    return {
        summary: summary || l2BlockRows()[0].summary,
        transactions: [l2Transaction(identity("2")), l2Transaction(identity("3"))],
        source: source
    }
}

function l2TransactionDetail(hashValue, sourceId) {
    const transaction = l2Transaction(hashValue || identity("2"))
    const source = l2Source(
        sourceId || "src_11111111111111111111111111111111",
        sourceId && sourceId.indexOf("3333") >= 0 ? "indexer" : "sequencer",
        sourceId && sourceId.indexOf("3333") >= 0 ? "finalized" : "provisional",
        "memory_cache"
    )
    return {
        transaction: transaction,
        inspection: {
            hash: transaction.hash,
            kind: transaction.kind,
            sections: [{
                title: "Public Transaction",
                rows: [{ label: "Program ID", index: null, value: transaction.program_id_hex, decimal: null, hex: transaction.program_id_hex, base58: null },
                    { label: "Instruction word", index: 0, value: "16", decimal: "16", hex: "0x10", base58: null }]
            }],
            raw_summary: transaction
        },
        source: source
    }
}

function l2TransactionTrace(hashValue, sourceId) {
    const detail = l2TransactionDetail(hashValue, sourceId)
    return {
        transaction: detail.transaction,
        trace: {
            hash: detail.transaction.hash,
            kind: detail.transaction.kind,
            source: "local_derivation",
            capabilities: ["Content hash and signature checks"],
            limitations: ["Runtime execution is not replayed"],
            steps: [{
                index: 0,
                phase: "parse",
                label: "Parse transaction",
                status: "ok",
                severity: "success",
                details: ["Public transaction envelope normalized"],
                refs: null
            }, {
                index: 1,
                phase: "verify",
                label: "Verify signature",
                status: "valid",
                severity: "success",
                details: ["Prehash signature matches returned payload"],
                refs: { program_id_hex: detail.transaction.program_id_hex }
            }],
            inspection: detail.inspection,
            decoded_instruction: l2DecodedInstruction()
        },
        source: detail.source
    }
}

function l2DecodedInstruction() {
    return {
        program_id: identity("6"),
        idl_name: "token",
        instruction: "transfer",
        variant_index: 0,
        accounts: [{
            path: "sender",
            value: identity("7")
        }, {
            path: "recipient",
            value: identity("8")
        }],
        args: [{
            path: "amount_to_transfer: u128",
            value: "1234567"
        }],
        remaining_words: []
    }
}

function l2AccountId() {
    return identity("7")
}

function l2AccountSnapshot(kind) {
    const provisional = kind === "provisional"
    const historical = kind === "historical"
    const source = l2Source(
        provisional
            ? "src_11111111111111111111111111111111"
            : "src_33333333333333333333333333333333",
        provisional ? "sequencer" : "indexer",
        provisional ? "provisional" : "finalized",
        historical ? "memory_cache" : "live"
    )
    const blockId = historical ? 12790 : (provisional ? 12844 : 12840)
    return {
        account: {
            account_id: l2AccountId(),
            account_id_base58: "9xQeWvG816bUx9EPjHmaT23yvVMf8bYh3F4",
            account_id_hex: identity("7"),
            balance: historical ? "1184000" : (provisional ? "1242750" : "1240000"),
            nonce: historical ? "36" : (provisional ? "42" : "41"),
            owner_program_base58: "LezSystem1111111111111111111111111111",
            owner_program_hex: identity("6"),
            data_hex: "01000000000000002a00000000000000",
            existence: "unknown"
        },
        anchor: {
            block_id: blockId,
            block_hash: historical ? identity("0")
                : (provisional ? identity("b") : identity("f"))
        },
        after_anchor: provisional ? {
            block_id: 12845,
            block_hash: identity("5")
        } : null,
        anchor_state: provisional ? "moving" : "exact",
        source: source
    }
}

function l2AccountDecode() {
    return {
        evidence: {
            name: "Token Fixture",
            account_type: "TokenDefinition"
        },
        report: {
            account_type: "TokenDefinition",
            consumed_bytes: 16,
            total_bytes: 16,
            rows: [{ path: "name", value: "Pebble" },
                { path: "total_supply", value: "7654321" }]
        }
    }
}

function l2AccountActivityRows() {
    return [l2AccountActivityRow(0, identity("2"), "incoming"),
        l2AccountActivityRow(1, identity("3"), "outgoing"),
        l2AccountActivityRow(2, identity("5"), "program")]
}

function l2AccountActivityRow(index, transactionId, direction) {
    return {
        index: index,
        transaction_id: transactionId,
        kind: "public",
        direction: direction,
        program_id_hex: identity("6"),
        account_ids: [l2AccountId(), identity("8")],
        signer_account_ids: [l2AccountId()],
        nonces: [String(39 + index)],
        instruction_data: [16, 32],
        transfer_outputs: [],
        bytecode_len: null
    }
}

function l2Programs() {
    return [{
        label: "System Program",
        base58: "LezSystem1111111111111111111111111111",
        hex: identity("6")
    }, {
        label: "Token Program",
        base58: "LezToken11111111111111111111111111111",
        hex: identity("9")
    }]
}

function l2CommitmentProof() {
    return {
        commitment_hex: identity("c"),
        leaf_index: 42,
        sibling_hashes: [identity("d"), identity("e"), identity("f")],
        source: l2Source(
            "src_11111111111111111111111111111111",
            "sequencer",
            "provisional",
            "live"
        )
    }
}

function l2AccountNonces() {
    return [{ account_id: l2AccountId(), nonce: "42" },
        { account_id: identity("8"), nonce: "17" }]
}

function l2TransferRecipients() {
    return [{
        recipient: l2AccountId(),
        account_ref: l2AccountId(),
        received: "2750",
        txs: 2,
        outputs: 1,
        references: 2,
        last_slot: 12840,
        source: "transfer_outputs_and_account_refs",
        transfers: [{
            slot: 12840,
            tx_hash: identity("2"),
            block_hash: identity("f"),
            value: "2750"
        }, {
            slot: 12839,
            tx_hash: identity("3"),
            block_hash: identity("e"),
            value: null
        }]
    }, {
        recipient: identity("8"),
        account_ref: identity("8"),
        received: "500",
        txs: 1,
        outputs: 1,
        references: 0,
        last_slot: 12838,
        source: "transfer_outputs",
        transfers: [{
            slot: 12838,
            tx_hash: identity("5"),
            block_hash: identity("d"),
            value: "500"
        }]
    }]
}

function l2FoundReport(reportKind, value, source) {
    const report = l2RouteReport(reportKind, source)
    report.data = {
        outcome: "found",
        value: value
    }
    return report
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
