import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "ZoneInspectionState"

    property var zoneState: null
    property var mutationCallbackResponse: null
    property string activeZoneSeenOnSummaryChange: ""

    QtObject {
        id: gateway

        property int nextRequestId: 1
        property var requests: []

        function reset() {
            nextRequestId = 1
            requests = []
        }

        function request(method, args, callback) {
            const requestId = nextRequestId
            nextRequestId += 1
            requests = requests.concat([{
                requestId: requestId,
                method: String(method || ""),
                args: args || [],
                callback: callback,
                completed: false
            }])
            return requestId
        }

        function requestCount(method) {
            let count = 0
            for (let i = 0; i < requests.length; ++i) {
                if (requests[i].method === method) {
                    count += 1
                }
            }
            return count
        }

        function pendingRequest(method) {
            for (let i = 0; i < requests.length; ++i) {
                if (!requests[i].completed && requests[i].method === method) {
                    return requests[i]
                }
            }
            return null
        }

        function lastRequest(method) {
            for (let i = requests.length - 1; i >= 0; --i) {
                if (requests[i].method === method) {
                    return requests[i]
                }
            }
            return null
        }

        function respondNext(method, response) {
            const entry = pendingRequest(method)
            testRoot.verify(entry !== null, "Missing pending request for " + method)
            respond(entry, response)
        }

        function respond(entry, response) {
            testRoot.verify(entry !== null, "Missing request entry")
            testRoot.verify(!entry.completed, "Request already completed")
            entry.completed = true
            entry.callback(response)
        }
    }

    Component {
        id: stateComponent

        Domains.ZoneInspectionState {}
    }

    SignalSpy {
        id: statusRefreshSpy

        signalName: "statusRefreshRequested"
    }

    function init() {
        gateway.reset()
        mutationCallbackResponse = null
        activeZoneSeenOnSummaryChange = ""
        zoneState = stateComponent.createObject(testRoot, {
            gateway: gateway
        })
        verify(zoneState !== null)
        statusRefreshSpy.target = zoneState
        statusRefreshSpy.clear()
        zoneState.zoneSummariesChanged.connect(function () {
            activeZoneSeenOnSummaryChange = zoneState.activeZoneId
        })
    }

    function cleanup() {
        statusRefreshSpy.target = null
        if (zoneState) {
            zoneState.destroy()
            zoneState = null
        }
    }

    function ok(value) {
        return {
            ok: true,
            value: value,
            text: "OK",
            error: ""
        }
    }

    function failed(error) {
        return {
            ok: false,
            value: null,
            text: "",
            error: String(error || "failed")
        }
    }

    function scope(value) {
        return {
            kind: "genesis_id",
            genesis_id: String(value || "network-a")
        }
    }

    function configure(endpoint, revision) {
        zoneState.sourceDescriptor = {
            kind: "direct_http",
            endpoint: String(endpoint || "https://l1.example")
        }
        zoneState.start()
        gateway.respondNext("zoneCatalogConfigure", ok({
            report_kind: "zones.catalog_configured",
            schema_version: 1,
            source_revision: Number(revision || 1)
        }))
    }

    function statusReport(overrides) {
        const report = {
            report_kind: "zones.catalog_status",
            schema_version: 1,
            source_revision: 1,
            network_scope: scope("network-a"),
            catalog_revision: 1,
            source_config_epoch: 1,
            observation_revision: 1,
            summary_revision: 0,
            verification: "verifying",
            coverage: {
                status: "rebuilding",
                gap_count: 0
            },
            ingestion: {
                worker_running: true,
                discovered_zone_count: 0
            },
            current_error: null
        }
        const values = overrides || {}
        for (const key in values) {
            report[key] = values[key]
        }
        return report
    }

    function summaryReport(revision, changes, nextCursor, overrides) {
        const report = {
            report_kind: "zones.summary",
            schema_version: 1,
            source_revision: 1,
            network_scope: scope("network-a"),
            catalog_revision: 1,
            source_config_epoch: 1,
            observation_revision: 1,
            summary_revision: Number(revision || 0),
            changes: changes,
            next_cursor: nextCursor || null
        }
        const values = overrides || {}
        for (const key in values) {
            report[key] = values[key]
        }
        return report
    }

    function zoneRow(channelId, kind, selectedSourceId, indexerSourceId) {
        const row = {
            channel_id: String(channelId),
            kind: String(kind || "sequencer_zone"),
            display: {
                title: String(channelId),
                short_channel_id: String(channelId)
            },
            settlement_link: {
                status: "linked",
                selected_sequencer_source_id: selectedSourceId || null,
                indexer_source_id: indexerSourceId || null
            },
            activity_state: "active"
        }
        if (row.kind === "sequencer_zone") {
            row.l2_zone = {
                selected_source_id: selectedSourceId || null
            }
        }
        return row
    }

    function detailReport(row, config, overrides) {
        const report = {
            report_kind: "zones.zone_detail",
            schema_version: 1,
            source_revision: 1,
            network_scope: scope("network-a"),
            catalog_revision: 1,
            source_config_epoch: 1,
            observation_revision: 1,
            summary_revision: 1,
            detail: {
                summary: row,
                l1_channel_snapshot: {},
                channel_source_config: config || {
                    config_revision: 0,
                    selected_sequencer_source_id: null,
                    sequencer_sources: [],
                    indexer_source: null
                },
                source_observations: [],
                source_agreement: {},
                classification_evidence: {},
                activity_counts: {},
                detail_revision: 1
            }
        }
        const values = overrides || {}
        for (const key in values) {
            report[key] = values[key]
        }
        return report
    }

    function evidenceRow(evidenceId, slot, kind) {
        return {
            reference: {
                evidence_id: String(evidenceId),
                channel_id: "zone-a",
                coverage_segment_id: "segment-main",
                l1_slot: Number(slot),
                block_id: "b".repeat(64),
                transaction_hash: "c".repeat(64),
                operation_index: 0,
                message_id: null,
                evidence_kind: String(kind || "raw_inscription"),
                evidence_use: "presence"
            },
            segment: {
                segment_id: "segment-main",
                floor_slot: 0,
                frontier_slot: Number(slot),
                reaches_target_lib: true
            },
            source: {
                kind: "direct_http",
                fingerprint: "sha256:test"
            },
            finality: "final"
        }
    }

    function evidencePageReport(rows, nextCursor, filter) {
        return {
            report_kind: "zones.evidence_page",
            schema_version: 1,
            source_revision: 1,
            network_scope: scope("network-a"),
            catalog_revision: 1,
            channel_id: "zone-a",
            filter: String(filter || "all"),
            rows: rows,
            next_cursor: nextCursor || null
        }
    }

    function evidenceDetailReport(row, sessionId) {
        return {
            report_kind: "zones.evidence_detail",
            schema_version: 1,
            source_revision: 1,
            network_scope: scope("network-a"),
            catalog_revision: 1,
            channel_id: "zone-a",
            row: row,
            operation: { opcode: 17 },
            payload: {
                byte_length: 300000,
                sha256: "sha256:test",
                encoding: "utf8",
                inline_text: null,
                inline_base64: null,
                preview: "payload preview",
                preview_truncated: true,
                inline_truncated: true,
                session_id: sessionId || null,
                warning: null
            }
        }
    }

    function configuredSourceConfig() {
        return {
            config_revision: 7,
            selected_sequencer_source_id: "seq-a",
            sequencer_sources: [{
                source_id: "seq-a",
                label: "Primary",
                target: { kind: "rpc", endpoint: "https://sequencer.example" },
                binding_state: "runtime_attested"
            }],
            indexer_source: {
                source_id: "idx-a",
                label: "Finalized",
                target: { kind: "module", module_id: "lez_indexer_module" }
            }
        }
    }

    function loadConfiguredL2Zone() {
        configure("https://l1.example", 1)
        const row = zoneRow("zone-a", "sequencer_zone", "seq-a", "idx-a")
        loadOneZone(row)
        verify(zoneState.activateZone("zone-a"))
        gateway.respondNext("zoneDetail", ok(detailReport(row, configuredSourceConfig())))
        verify(zoneState.l2ReadEnabled)
        compare(zoneState.activeZoneContext.source_config_revision, 7)
        return row
    }

    function l2Report(requestEntry, reportKind, data, overrides) {
        const request = requestEntry.args[0]
        const report = {
            report_kind: String(reportKind),
            schema_version: 1,
            context: request.context,
            request_revision: request.request_revision,
            route: {
                policy: "composite",
                attempts: []
            },
            route_completeness: "all_configured",
            warnings: [],
            data: data
        }
        const values = overrides || {}
        for (const key in values) {
            report[key] = values[key]
        }
        return report
    }

    function l2Source(sourceId, role, finality, retrieval) {
        return {
            source_id: String(sourceId),
            source_role: String(role),
            source_config_revision: 7,
            finality: String(finality),
            retrieval: String(retrieval || "live")
        }
    }

    function l2Block(blockId, hashValue, observations) {
        return {
            summary: {
                block_id: Number(blockId),
                block_hash: String(hashValue),
                parent_hash: "p".repeat(64),
                timestamp: 1000 + Number(blockId),
                bedrock_status: "accepted",
                transaction_count: 1
            },
            observations: observations
        }
    }

    function l2Transaction(hashValue) {
        return {
            hash: String(hashValue),
            kind: "public",
            program_id_hex: "ab".repeat(32),
            account_ids: ["account-a"],
            nonces: ["1"],
            instruction_data: [1, 2],
            bytecode_len: null,
            raw_signature_valid: true,
            message_prehash: "cd".repeat(32),
            prehash_signature_valid: true
        }
    }

    function l2AccountSnapshot(accountId, balance, source, anchorState, blockId) {
        return {
            account: {
                account_id: String(accountId),
                account_id_base58: "account-base58",
                account_id_hex: "ab".repeat(32),
                balance: String(balance),
                nonce: "4",
                owner_program_base58: "program-base58",
                owner_program_hex: "cd".repeat(32),
                data_hex: "0102",
                existence: "unknown"
            },
            anchor: blockId === null || blockId === undefined ? null : {
                block_id: Number(blockId),
                block_hash: String(blockId).repeat(64).slice(0, 64)
            },
            after_anchor: null,
            anchor_state: String(anchorState || "exact"),
            source: source
        }
    }

    function l2ActivityRow(index, transactionId) {
        return {
            index: Number(index),
            transaction_id: String(transactionId),
            kind: "public",
            direction: "outgoing",
            program_id_hex: "ab".repeat(32),
            account_ids: ["account-a"],
            signer_account_ids: ["account-a"],
            nonces: [String(index)],
            instruction_data: [1, 2],
            transfer_outputs: [],
            bytecode_len: null
        }
    }

    function l2TransferRecipient(recipient, received, outputs, references) {
        return {
            recipient: String(recipient),
            account_ref: String(recipient),
            received: received === null ? null : String(received),
            txs: 1,
            outputs: Number(outputs),
            references: Number(references),
            last_slot: 12,
            source: outputs > 0 && references > 0
                ? "transfer_outputs_and_account_refs"
                : (outputs > 0 ? "transfer_outputs" : "account_refs"),
            transfers: [{
                slot: 12,
                tx_hash: "ef".repeat(32),
                block_hash: "12".repeat(32),
                value: received === null ? null : String(received)
            }]
        }
    }

    function l2AccountRequest(kind) {
        for (let i = gateway.requests.length - 1; i >= 0; --i) {
            const entry = gateway.requests[i]
            const query = entry.args && entry.args[0] && entry.args[0].query
            if (entry.method === "zoneL2Account" && query && query.snapshot
                    && query.snapshot.kind === kind && !entry.completed) {
                return entry
            }
        }
        return null
    }

    function loadOneZone(row) {
        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "verified",
            coverage: { status: "complete", gap_count: 0 },
            ingestion: { worker_running: false, discovered_zone_count: 1 },
            summary_revision: 1
        })))
        gateway.respondNext("zonesSummary", ok(summaryReport(1, {
            kind: "reset",
            rows: [row]
        }, null)))
        compare(zoneState.zoneSummaries.length, 1)
    }

    function test_status_single_flight_backoff_and_steady_cadence() {
        configure("https://l1.example", 1)

        verify(zoneState.statusPollingEnabled)
        compare(zoneState.statusPollInterval, 1000)
        verify(zoneState.pollStatus())
        verify(!zoneState.pollStatus())
        compare(gateway.requestCount("zoneCatalogStatus"), 1)

        const expectedBackoffs = [2000, 5000, 15000, 30000, 30000]
        for (let i = 0; i < expectedBackoffs.length; ++i) {
            gateway.respondNext("zoneCatalogStatus", failed("bridge down"))
            compare(zoneState.statusPollInterval, expectedBackoffs[i])
            verify(zoneState.pollStatus())
        }

        gateway.respondNext("zoneCatalogStatus", ok(statusReport()))
        compare(zoneState.statusFailureCount, 0)
        compare(zoneState.statusPollInterval, 1000)

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "verified",
            coverage: { status: "complete", gap_count: 0 },
            ingestion: { worker_running: false, discovered_zone_count: 0 }
        })))
        compare(zoneState.statusPollInterval, 5000)
        verify(zoneState.summaryInFlight)
    }

    function test_missing_or_unsupported_l1_source_never_polls() {
        zoneState.start()
        compare(gateway.requests.length, 0)
        verify(!zoneState.statusPollingEnabled)
        verify(!zoneState.pollStatus())

        zoneState.sourceDescriptor = {
            kind: "module",
            module_id: "blockchain_module"
        }
        compare(gateway.requests.length, 0)
        verify(!zoneState.statusPollingEnabled)
    }

    function test_configure_race_accepts_only_latest_source() {
        zoneState.sourceDescriptor = {
            kind: "direct_http",
            endpoint: "https://l1-a.example"
        }
        zoneState.start()
        compare(gateway.requestCount("zoneCatalogConfigure"), 1)

        zoneState.sourceDescriptor = {
            kind: "direct_http",
            endpoint: "https://l1-b.example"
        }
        gateway.respondNext("zoneCatalogConfigure", ok({
            report_kind: "zones.catalog_configured",
            schema_version: 1,
            source_revision: 1
        }))

        tryCompare(gateway, "nextRequestId", 3)
        verify(!zoneState.catalogConfigured)
        gateway.respondNext("zoneCatalogConfigure", ok({
            report_kind: "zones.catalog_configured",
            schema_version: 1,
            source_revision: 2
        }))

        verify(zoneState.catalogConfigured)
        compare(zoneState.sourceRevision, 2)
        compare(zoneState.desiredSource.endpoint, "https://l1-b.example")
        compare(statusRefreshSpy.count, 1)
    }

    function test_summary_pages_commit_atomically_then_catch_up_delta() {
        configure("https://l1.example", 1)
        const rowA = zoneRow("a", "sequencer_zone", "src-a", null)
        const rowB = zoneRow("b", "data_channel", null, null)
        const rowC = zoneRow("c", "sequencer_zone", "src-c", "idx-c")

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "verified",
            coverage: { status: "complete" },
            ingestion: { worker_running: false },
            summary_revision: 5
        })))
        compare(gateway.requestCount("zonesSummary"), 1)

        gateway.respondNext("zonesSummary", ok(summaryReport(5, {
            kind: "reset",
            rows: [rowA]
        }, "cursor-2")))
        compare(zoneState.zoneSummaries.length, 0)
        verify(zoneState.summaryInFlight)
        compare(gateway.requestCount("zonesSummary"), 2)

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "verified",
            coverage: { status: "complete" },
            ingestion: { worker_running: false },
            catalog_revision: 2,
            observation_revision: 2,
            summary_revision: 6
        })))

        gateway.respondNext("zonesSummary", ok(summaryReport(5, {
            kind: "reset",
            rows: [rowB]
        }, null)))
        compare(zoneState.zoneSummaries.length, 2)
        compare(zoneState.zoneSummaries[0].channel_id, "a")
        compare(zoneState.zoneSummaries[1].channel_id, "b")
        compare(zoneState.summaryRevision, 5)
        verify(zoneState.summaryStale)
        compare(gateway.requestCount("zonesSummary"), 3)

        const deltaRequest = gateway.lastRequest("zonesSummary")
        compare(deltaRequest.args[0].after_summary_revision, 5)
        compare(deltaRequest.args[0].cursor, null)
        gateway.respondNext("zonesSummary", ok(summaryReport(6, {
            kind: "delta",
            upserts: [rowC],
            removed_zone_ids: ["a"]
        }, null, {
            catalog_revision: 2,
            observation_revision: 2
        })))

        compare(zoneState.zoneSummaries.length, 2)
        compare(zoneState.zoneSummaries[0].channel_id, "b")
        compare(zoneState.zoneSummaries[1].channel_id, "c")
        compare(zoneState.summaryRevision, 6)
        verify(!zoneState.summaryStale)
    }

    function test_network_change_clears_rows_context_and_stale_detail() {
        configure("https://l1.example", 1)
        const row = zoneRow("zone-a", "sequencer_zone", "src-a", null)
        loadOneZone(row)

        verify(zoneState.activateZone("zone-a"))
        verify(zoneState.detailInFlight)
        const oldContextRevision = zoneState.contextRevision

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            network_scope: scope("network-b"),
            verification: "verified",
            coverage: { status: "complete" },
            ingestion: { worker_running: false },
            catalog_revision: 2,
            summary_revision: 2
        })))

        compare(zoneState.zoneSummaries.length, 0)
        compare(zoneState.activeZoneContext, null)
        verify(zoneState.contextRevision > oldContextRevision)

        gateway.respondNext("zoneDetail", ok(detailReport(row, null)))
        compare(zoneState.zoneDetail, null)
        compare(zoneState.activeZoneContext, null)
    }

    function test_verification_loss_and_zone_removal_clear_active_context() {
        configure("https://l1.example", 1)
        const row = zoneRow("zone-a", "sequencer_zone", null, null)
        loadOneZone(row)
        verify(zoneState.activateZone("zone-a"))

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "mismatch",
            coverage: { status: "unknown" },
            ingestion: { worker_running: false },
            summary_revision: 1
        })))
        compare(zoneState.activeZoneContext, null)

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "verified",
            coverage: { status: "complete" },
            ingestion: { worker_running: false },
            summary_revision: 1
        })))
        verify(zoneState.activateZone("zone-a"))

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "verified",
            coverage: { status: "complete" },
            ingestion: { worker_running: false },
            summary_revision: 2
        })))
        gateway.respondNext("zonesSummary", ok(summaryReport(2, {
            kind: "delta",
            upserts: [],
            removed_zone_ids: ["zone-a"]
        }, null)))
        compare(zoneState.zoneSummaries.length, 0)
        compare(activeZoneSeenOnSummaryChange, "")
        verify(!zoneState.activateZone("zone-a"))
    }

    function test_source_mutation_updates_active_context_only_after_success() {
        configure("https://l1.example", 1)
        const row = zoneRow("zone-a", "sequencer_zone", "src-a", "idx-a")
        loadOneZone(row)
        verify(zoneState.activateZone("zone-a"))

        const initialConfig = {
            config_revision: 1,
            selected_sequencer_source_id: "src-a",
            sequencer_sources: [{
                source_id: "src-a",
                target: { kind: "rpc", endpoint: "https://seq-a" },
                channel_attestation: { state: "persisted_attested" }
            }],
            indexer_source: { source_id: "idx-a", target: { kind: "rpc", endpoint: "https://idx-a" } }
        }
        gateway.respondNext("zoneDetail", ok(detailReport(row, initialConfig)))
        compare(zoneState.activeZoneContext.source_config_revision, 1)
        const contextBeforeMutation = zoneState.contextRevision

        zoneState.applyChannelSourceConfig({
            expected_config_revision: 1,
            mutation: {
                kind: "select_sequencer",
                source_id: null
            }
        }, function (response) {
            mutationCallbackResponse = response
        })
        verify(zoneState.sourceMutationInFlight)
        compare(zoneState.activeZoneContext.selected_sequencer_source_id, "src-a")

        gateway.respondNext("channelSourceConfigApply", ok({
            report_kind: "zones.channel_source_config",
            schema_version: 1,
            source_revision: 1,
            catalog_revision: 1,
            source_config_epoch: 2,
            observation_revision: 1,
            summary_revision: 2,
            config: {
                network_scope: scope("network-a"),
                channel_id: "zone-a",
                config_revision: 2,
                selected_sequencer_source_id: null,
                sequencer_sources: initialConfig.sequencer_sources,
                indexer_source: initialConfig.indexer_source
            },
            observations: [],
            agreement: { state: "unconfigured" },
            attestation_warning: null
        }))

        verify(mutationCallbackResponse.ok)
        compare(zoneState.activeZoneContext.selected_sequencer_source_id, null)
        compare(zoneState.activeZoneContext.source_config_revision, 2)
        verify(zoneState.contextRevision > contextBeforeMutation)
        compare(zoneState.zoneDetail.channel_source_config.config_revision, 2)
        compare(zoneState.zoneDetail.channel_source_config.sequencer_sources[0].binding_state, "persisted_attested")
        verify(zoneState.summaryStale)

        const contextAfterSuccess = zoneState.contextRevision
        zoneState.applyChannelSourceConfig({
            expected_config_revision: 2,
            mutation: { kind: "remove_indexer" }
        })
        gateway.respondNext("channelSourceConfigApply", failed("revision conflict"))
        compare(zoneState.contextRevision, contextAfterSuccess)
        compare(zoneState.activeZoneContext.indexer_source_id, "idx-a")
        compare(zoneState.sourceMutationError, "revision conflict")
    }

    function test_control_and_resume_request_immediate_status_refresh() {
        configure("https://l1.example", 1)
        statusRefreshSpy.clear()

        zoneState.appResumed()
        compare(statusRefreshSpy.count, 1)

        zoneState.retryCatalog()
        verify(zoneState.controlInFlight)
        compare(zoneState.statusPollInterval, 1000)
        gateway.respondNext("zoneCatalogRetry", ok({
            report_kind: "zones.catalog_control",
            schema_version: 1,
            control: "retry",
            source_revision: 1
        }))
        compare(statusRefreshSpy.count, 2)
    }

    function test_evidence_pages_detail_chunks_and_release_are_context_fenced() {
        configure("https://l1.example", 1)
        const row = zoneRow("zone-a", "data_channel", null, null)
        loadOneZone(row)
        verify(zoneState.activateZone("zone-a"))
        gateway.respondNext("zoneDetail", ok(detailReport(row, null)))

        verify(zoneState.loadEvidence("all"))
        const firstRequest = gateway.lastRequest("zoneEvidencePage")
        compare(firstRequest.args[0].channel_id, "zone-a")
        compare(firstRequest.args[0].catalog_revision, 1)
        compare(firstRequest.args[0].filter, "all")
        const evidenceA = evidenceRow("evidence-a", 10, "channel_configuration")
        const evidenceB = evidenceRow("evidence-b", 12, "raw_inscription")
        gateway.respondNext("zoneEvidencePage", ok(evidencePageReport([evidenceA], "cursor-2", "all")))
        compare(zoneState.evidenceRows.length, 1)
        compare(zoneState.evidenceNextCursor, "cursor-2")

        verify(zoneState.loadMoreEvidence())
        compare(gateway.lastRequest("zoneEvidencePage").args[0].cursor, "cursor-2")
        gateway.respondNext("zoneEvidencePage", ok(evidencePageReport([evidenceB], null, "all")))
        compare(zoneState.evidenceRows.length, 2)
        compare(zoneState.evidenceNextCursor, "")

        verify(zoneState.openEvidence(evidenceB))
        compare(gateway.lastRequest("zoneEvidenceDetail").args[0].reference.evidence_id, "evidence-b")
        gateway.respondNext("zoneEvidenceDetail", ok(evidenceDetailReport(evidenceB, "session-b")))
        compare(zoneState.evidenceDetail.row.reference.evidence_id, "evidence-b")
        verify(!zoneState.evidencePayloadDone)

        verify(zoneState.loadNextEvidencePayloadChunk())
        compare(gateway.lastRequest("zoneEvidencePayloadChunk").args[0].offset, 0)
        gateway.respondNext("zoneEvidencePayloadChunk", ok({
            report_kind: "zones.evidence_payload_chunk",
            schema_version: 1,
            session_id: "session-b",
            evidence_id: "evidence-b",
            encoding: "utf8",
            offset: 0,
            next_offset: 5,
            done: true,
            text: "hello",
            base64: null
        }))
        compare(zoneState.evidencePayloadChunks.length, 1)
        compare(zoneState.evidencePayloadChunks[0].text, "hello")
        compare(zoneState.evidencePayloadOffset, 5)
        verify(zoneState.evidencePayloadDone)

        zoneState.closeEvidenceDetail()
        compare(zoneState.evidenceDetail, null)
        compare(gateway.requestCount("zoneEvidencePayloadRelease"), 1)
        compare(gateway.lastRequest("zoneEvidencePayloadRelease").args[0].session_id, "session-b")
    }

    function test_l2_block_pages_carry_context_and_preserve_conflicts() {
        loadConfiguredL2Zone()

        verify(zoneState.refreshL2Blocks() !== null)
        const request = gateway.lastRequest("zoneL2Blocks")
        const payload = request.args[0]
        compare(payload.context.channel_id, "zone-a")
        compare(payload.context.selected_sequencer_source_id, "seq-a")
        compare(payload.context.indexer_source_id, "idx-a")
        compare(payload.context.source_config_revision, 7)
        compare(payload.context.context_revision, zoneState.activeZoneContext.context_revision)
        compare(payload.request_revision, zoneState.l2BlocksRequestRevision)
        compare(payload.query.cursor, null)
        compare(payload.query.limit, 25)
        verify(JSON.stringify(payload).indexOf("endpoint") < 0)

        const finalized = l2Source("idx-a", "indexer", "finalized", "live")
        const provisional = l2Source("seq-a", "sequencer", "provisional", "live")
        const rows = [
            l2Block(12, "a".repeat(64), [finalized]),
            l2Block(12, "b".repeat(64), [provisional])
        ]
        gateway.respondNext("zoneL2Blocks", ok(l2Report(request, "lez.blocks", {
            outcome: "found",
            value: {
                rows: rows,
                next_cursor: "opaque-next",
                has_more: true,
                distinct_block_ids: 1,
                source_heads: [{ source_id: "idx-a", source_role: "indexer", block_id: 12, block_hash: "a".repeat(64) }]
            }
        })))

        compare(zoneState.l2BlockRows.length, 2)
        compare(zoneState.l2BlockRows[0].summary.block_id, 12)
        verify(zoneState.l2BlockRows[0].summary.block_hash !== zoneState.l2BlockRows[1].summary.block_hash)
        compare(zoneState.l2BlockRows[0].observations[0].source_id, "idx-a")
        compare(zoneState.l2BlockRows[1].observations[0].finality, "provisional")
        compare(zoneState.l2BlocksDistinctCount, 1)
        verify(zoneState.l2BlocksHasMore)

        verify(zoneState.loadMoreL2Blocks() !== null)
        const nextRequest = gateway.lastRequest("zoneL2Blocks")
        compare(nextRequest.args[0].query.cursor, "opaque-next")
        gateway.respondNext("zoneL2Blocks", ok(l2Report(nextRequest, "lez.blocks", {
            outcome: "found",
            value: {
                rows: [l2Block(11, "c".repeat(64), [finalized])],
                next_cursor: null,
                has_more: false,
                distinct_block_ids: 1,
                source_heads: []
            }
        })))
        compare(zoneState.l2BlockRows.length, 3)
        compare(zoneState.l2BlocksDistinctCount, 2)
        verify(!zoneState.l2BlocksHasMore)
    }

    function test_l2_block_detail_rejects_superseded_reply_and_resolves_exact_source() {
        loadConfiguredL2Zone()
        const firstSummary = l2Block(12, "a".repeat(64), []).summary
        const secondSummary = l2Block(12, "b".repeat(64), []).summary

        verify(zoneState.openL2Block(firstSummary, "idx-a") !== null)
        const firstRequest = gateway.lastRequest("zoneL2BlockDetail")
        verify(zoneState.openL2Block(secondSummary, "seq-a") !== null)
        const secondRequest = gateway.lastRequest("zoneL2BlockDetail")
        verify(firstRequest.args[0].request_revision < secondRequest.args[0].request_revision)

        gateway.respondNext("zoneL2BlockDetail", ok(l2Report(firstRequest, "lez.block_detail", {
            outcome: "found",
            value: {
                summary: firstSummary,
                transactions: [],
                source: l2Source("idx-a", "indexer", "finalized")
            }
        })))
        compare(zoneState.l2BlockDetail, null)

        gateway.respondNext("zoneL2BlockDetail", ok(l2Report(secondRequest, "lez.block_detail", {
            outcome: "ambiguous",
            candidates: [{
                source_id: "seq-a",
                source_role: "sequencer",
                canonical_key: "block:12:" + secondSummary.block_hash
            }]
        })))
        compare(zoneState.l2BlockCandidates.length, 1)
        verify(zoneState.resolveL2BlockCandidate(zoneState.l2BlockCandidates[0]) !== null)
        const exactRequest = gateway.lastRequest("zoneL2BlockDetail")
        compare(exactRequest.args[0].query.exact_source_id, "seq-a")
        compare(exactRequest.args[0].query.target.kind, "identity")
        compare(exactRequest.args[0].query.target.block_hash, secondSummary.block_hash)

        gateway.respondNext("zoneL2BlockDetail", ok(l2Report(exactRequest, "lez.block_detail", {
            outcome: "found",
            value: {
                summary: secondSummary,
                transactions: [l2Transaction("d".repeat(64))],
                source: l2Source("seq-a", "sequencer", "provisional", "memory_cache")
            }
        })))
        compare(zoneState.l2BlockDetail.source.source_id, "seq-a")
        compare(zoneState.l2BlockDetail.source.retrieval, "memory_cache")
        compare(zoneState.l2BlockDetail.transactions.length, 1)
    }

    function test_l2_transaction_detail_auto_traces_same_source_and_fences_trace_race() {
        loadConfiguredL2Zone()
        const transaction = l2Transaction("e".repeat(64))

        verify(zoneState.openL2Transaction(transaction.hash, "seq-a") !== null)
        const detailRequest = gateway.lastRequest("zoneL2Transaction")
        compare(detailRequest.args[0].query.exact_source_id, "seq-a")
        gateway.respondNext("zoneL2Transaction", ok(l2Report(detailRequest, "lez.transaction", {
            outcome: "found",
            value: {
                transaction: transaction,
                inspection: {
                    hash: transaction.hash,
                    kind: transaction.kind,
                    sections: [{ title: "Message", rows: [] }],
                    raw_summary: transaction
                },
                source: l2Source("seq-a", "sequencer", "provisional")
            }
        })))

        compare(zoneState.l2TransactionDetail.source.source_id, "seq-a")
        const firstTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        verify(firstTraceRequest !== null)
        compare(firstTraceRequest.args[0].query.transaction_id, transaction.hash)
        compare(firstTraceRequest.args[0].query.exact_source_id, "seq-a")
        compare(firstTraceRequest.args[0].query.idl_program_id, null)

        verify(zoneState.requestL2TransactionTrace(transaction.hash, "seq-a", "") !== null)
        const secondTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        verify(firstTraceRequest.args[0].request_revision < secondTraceRequest.args[0].request_revision)
        const staleTrace = {
            transaction: transaction,
            trace: { hash: "stale", kind: "public", source: "local", capabilities: [], limitations: [], steps: [], inspection: {}, decoded_instruction: null },
            source: l2Source("seq-a", "sequencer", "provisional")
        }
        gateway.respondNext("zoneL2TransactionTrace", ok(l2Report(firstTraceRequest, "lez.transaction_trace", {
            outcome: "found",
            value: staleTrace
        })))
        compare(zoneState.l2TransactionTrace, null)

        const currentTrace = {
            transaction: transaction,
            trace: {
                hash: transaction.hash,
                kind: "public",
                source: "local_derivation",
                capabilities: ["Signature checks"],
                limitations: [],
                steps: [{ index: 0, phase: "parse", label: "Parse", status: "ok", severity: "success", details: [], refs: null }],
                inspection: {},
                decoded_instruction: null
            },
            source: l2Source("seq-a", "sequencer", "provisional", "memory_cache")
        }
        gateway.respondNext("zoneL2TransactionTrace", ok(l2Report(secondTraceRequest, "lez.transaction_trace", {
            outcome: "found",
            value: currentTrace
        })))
        compare(zoneState.l2TransactionTrace.trace.hash, transaction.hash)
        compare(zoneState.l2TransactionTrace.source.source_id, "seq-a")
        compare(zoneState.l2TransactionTrace.source.retrieval, "memory_cache")
        compare(zoneState.l2TransactionTrace.trace.steps.length, 1)
    }

    function test_l2_trace_rejects_different_source_provenance() {
        loadConfiguredL2Zone()
        const transaction = l2Transaction("9".repeat(64))
        verify(zoneState.requestL2TransactionTrace(transaction.hash, "seq-a", "") !== null)
        const request = gateway.lastRequest("zoneL2TransactionTrace")
        gateway.respondNext("zoneL2TransactionTrace", ok(l2Report(request, "lez.transaction_trace", {
            outcome: "found",
            value: {
                transaction: transaction,
                trace: {
                    hash: transaction.hash,
                    kind: transaction.kind,
                    source: "local_derivation",
                    capabilities: [],
                    limitations: [],
                    steps: [],
                    inspection: {},
                    decoded_instruction: null
                },
                source: l2Source("idx-a", "indexer", "finalized")
            }
        })))
        compare(zoneState.l2TransactionTrace, null)
        compare(zoneState.l2TransactionTraceError,
            "Transaction trace returned different source provenance.")
    }

    function test_l2_success_with_mismatched_context_never_replaces_visible_rows() {
        loadConfiguredL2Zone()
        verify(zoneState.refreshL2Blocks() !== null)
        const request = gateway.lastRequest("zoneL2Blocks")
        const wrongContext = zoneState.l2RequestContext()
        wrongContext.context_revision += 1
        gateway.respondNext("zoneL2Blocks", ok(l2Report(request, "lez.blocks", {
            outcome: "found",
            value: {
                rows: [l2Block(12, "f".repeat(64), [l2Source("idx-a", "indexer", "finalized")])],
                next_cursor: null,
                has_more: false,
                distinct_block_ids: 1,
                source_heads: []
            }
        }, {
            context: wrongContext
        })))
        compare(zoneState.l2BlockRows.length, 0)
        verify(!zoneState.l2BlocksLoaded)
    }

    function test_l2_account_snapshots_are_independent_and_historical_is_exact() {
        loadConfiguredL2Zone()
        verify(zoneState.inspectL2Account("account-a"))
        compare(gateway.requestCount("zoneL2Account"), 2)
        compare(gateway.requestCount("zoneL2AccountActivity"), 1)

        const finalizedRequest = l2AccountRequest("finalized")
        const provisionalRequest = l2AccountRequest("provisional")
        compare(finalizedRequest.args[0].query.exact_source_id, "idx-a")
        compare(provisionalRequest.args[0].query.exact_source_id, "seq-a")

        const provisional = l2AccountSnapshot("account-a", "19",
            l2Source("seq-a", "sequencer", "provisional"), "moving", 14)
        provisional.after_anchor = {
            block_id: 15,
            block_hash: "f".repeat(64)
        }
        gateway.respond(provisionalRequest, ok(l2Report(provisionalRequest, "lez.account", {
            outcome: "found",
            value: provisional
        })))
        compare(zoneState.l2AccountProvisional.account.balance, "19")
        compare(zoneState.l2AccountProvisional.anchor_state, "moving")
        compare(zoneState.l2AccountFinalized, null)

        gateway.respondNext("zoneL2AccountActivity", failed("activity unavailable"))
        compare(zoneState.l2AccountActivityError, "activity unavailable")
        compare(zoneState.l2AccountProvisional.account.balance, "19")

        const finalized = l2AccountSnapshot("account-a", "17",
            l2Source("idx-a", "indexer", "finalized"), "exact", 12)
        gateway.respond(finalizedRequest, ok(l2Report(finalizedRequest, "lez.account", {
            outcome: "found",
            value: finalized
        })))
        compare(zoneState.l2AccountFinalized.account.balance, "17")
        compare(zoneState.l2AccountProvisional.account.balance, "19")
        compare(zoneState.l2AccountFinalized.source.source_role, "indexer")
        compare(zoneState.l2AccountProvisional.source.source_role, "sequencer")

        verify(zoneState.requestL2HistoricalAccount(9, "9".repeat(64)) !== null)
        const historicalRequest = l2AccountRequest("historical")
        compare(historicalRequest.args[0].query.snapshot.block_id, 9)
        compare(historicalRequest.args[0].query.snapshot.block_hash, "9".repeat(64))
        compare(historicalRequest.args[0].query.exact_source_id, "idx-a")
        const historical = l2AccountSnapshot("account-a", "11",
            l2Source("idx-a", "indexer", "finalized", "memory_cache"), "exact", 9)
        gateway.respond(historicalRequest, ok(l2Report(historicalRequest, "lez.account", {
            outcome: "found",
            value: historical
        })))
        compare(zoneState.l2AccountHistorical.account.balance, "11")
        compare(zoneState.l2AccountHistorical.source.retrieval, "memory_cache")
        compare(zoneState.l2AccountFinalized.account.balance, "17")
        compare(zoneState.l2AccountProvisional.account.balance, "19")
    }

    function test_l2_account_activity_appends_oldest_first_without_touching_snapshots() {
        loadConfiguredL2Zone()
        verify(zoneState.inspectL2Account("account-a"))
        const activityRequest = gateway.lastRequest("zoneL2AccountActivity")
        compare(activityRequest.args[0].query.order, "oldest_first")
        compare(activityRequest.args[0].query.limit, 25)
        gateway.respond(activityRequest, ok(l2Report(activityRequest, "lez.account_activity", {
            outcome: "found",
            value: {
                account_id: "account-a",
                order: "oldest_first",
                rows: [l2ActivityRow(0, "tx-oldest"), l2ActivityRow(1, "tx-next")],
                next_cursor: "activity-next",
                has_more: true
            }
        })))
        compare(zoneState.l2AccountActivityRows.length, 2)
        compare(zoneState.l2AccountActivityRows[0].transaction_id, "tx-oldest")
        compare(zoneState.l2AccountActivityRows[1].transaction_id, "tx-next")
        verify(zoneState.l2AccountActivityHasMore)

        verify(zoneState.loadMoreL2AccountActivity())
        const nextRequest = gateway.lastRequest("zoneL2AccountActivity")
        compare(nextRequest.args[0].query.cursor, "activity-next")
        gateway.respond(nextRequest, ok(l2Report(nextRequest, "lez.account_activity", {
            outcome: "found",
            value: {
                account_id: "account-a",
                order: "oldest_first",
                rows: [l2ActivityRow(2, "tx-newest")],
                next_cursor: null,
                has_more: false
            }
        })))
        compare(zoneState.l2AccountActivityRows.length, 3)
        compare(zoneState.l2AccountActivityRows[2].transaction_id, "tx-newest")
        verify(!zoneState.l2AccountActivityHasMore)
        compare(zoneState.l2AccountFinalized, null)
        compare(zoneState.l2AccountProvisional, null)
    }

    function test_l2_sequencer_tools_use_selected_exact_source_and_isolated_slots() {
        loadConfiguredL2Zone()
        verify(zoneState.refreshL2Programs() !== null)
        const programsRequest = gateway.lastRequest("zoneL2Programs")
        compare(programsRequest.args[0].query.exact_source_id, "seq-a")
        gateway.respond(programsRequest, ok(l2Report(programsRequest, "lez.programs", {
            outcome: "found",
            value: {
                programs: [{ label: "System", base58: "program-58", hex: "ab".repeat(32) }],
                source: l2Source("seq-a", "sequencer", "provisional")
            }
        })))
        compare(zoneState.l2Programs.length, 1)

        verify(zoneState.requestL2CommitmentProof("cd".repeat(32)) !== null)
        const proofRequest = gateway.lastRequest("zoneL2CommitmentProof")
        compare(proofRequest.args[0].query.exact_source_id, "seq-a")
        gateway.respond(proofRequest, ok(l2Report(proofRequest, "lez.commitment_proof", {
            outcome: "found",
            value: {
                commitment_hex: "cd".repeat(32),
                leaf_index: 4,
                sibling_hashes: ["ef".repeat(32)],
                source: l2Source("seq-a", "sequencer", "provisional")
            }
        })))
        compare(zoneState.l2CommitmentProof.leaf_index, 4)
        compare(zoneState.l2Programs.length, 1)

        verify(zoneState.requestL2AccountNonces(["account-a", "account-b"]) !== null)
        const nonceRequest = gateway.lastRequest("zoneL2AccountNonces")
        compare(nonceRequest.args[0].query.exact_source_id, "seq-a")
        gateway.respond(nonceRequest, ok(l2Report(nonceRequest, "lez.account_nonces", {
            outcome: "found",
            value: {
                rows: [{ account_id: "account-a", nonce: "7" },
                    { account_id: "account-b", nonce: "9" }],
                source: l2Source("seq-a", "sequencer", "provisional")
            }
        })))
        compare(zoneState.l2AccountNonces.length, 2)
        compare(zoneState.l2CommitmentProof.leaf_index, 4)
        compare(zoneState.l2Programs.length, 1)
    }

    function test_l2_transfer_pages_replace_window_and_restore_newer_page() {
        loadConfiguredL2Zone()
        verify(zoneState.refreshL2Transfers() !== null)
        const firstRequest = gateway.lastRequest("zoneL2Transfers")
        compare(firstRequest.args[0].query.cursor, null)
        compare(firstRequest.args[0].query.block_limit, 25)
        const recipient = "aa".repeat(32)
        gateway.respond(firstRequest, ok(l2Report(firstRequest, "lez.transfers", {
            outcome: "found",
            value: {
                recipients: [l2TransferRecipient(recipient, "10", 1, 2)],
                next_cursor: "transfers-older",
                has_more: true,
                newest_block: 20,
                oldest_block: 16,
                scanned_blocks: 5,
                finalized: true
            }
        })))
        compare(zoneState.l2TransferRecipients[0].received, "10")
        compare(zoneState.l2TransferRecipients[0].source,
            "transfer_outputs_and_account_refs")
        compare(zoneState.l2TransfersNewestBlock, 20)
        compare(zoneState.l2TransfersOldestBlock, 16)

        verify(zoneState.loadOlderL2Transfers() !== null)
        compare(zoneState.l2TransferRecipients[0].received, "10")
        const olderRequest = gateway.lastRequest("zoneL2Transfers")
        compare(olderRequest.args[0].query.cursor, "transfers-older")
        gateway.respond(olderRequest, ok(l2Report(olderRequest, "lez.transfers", {
            outcome: "found",
            value: {
                recipients: [l2TransferRecipient(recipient, "3", 1, 0)],
                next_cursor: null,
                has_more: false,
                newest_block: 15,
                oldest_block: 11,
                scanned_blocks: 5,
                finalized: true
            }
        })))
        compare(zoneState.l2TransferRecipients.length, 1)
        compare(zoneState.l2TransferRecipients[0].received, "3")
        compare(zoneState.l2TransfersHistory.length, 1)
        compare(zoneState.l2TransfersNewestBlock, 15)
        compare(zoneState.l2TransfersOldestBlock, 11)

        verify(zoneState.loadNewerL2Transfers())
        compare(zoneState.l2TransferRecipients[0].received, "10")
        compare(zoneState.l2TransfersNewestBlock, 20)
        compare(zoneState.l2TransfersOldestBlock, 16)
        compare(zoneState.l2TransfersHistory.length, 0)
    }

    function test_target_resolution_is_request_and_context_fenced() {
        loadConfiguredL2Zone()
        let accepted = null
        verify(zoneState.resolveTarget("42", function (report) {
            accepted = report
        }) !== null)
        const first = gateway.lastRequest("inspectionResolveTarget")
        compare(first.args[0].query, "42")
        compare(first.args[0].active_zone_context.channel_id, "zone-a")

        verify(zoneState.resolveTarget("43", function (report) {
            accepted = report
        }) !== null)
        const second = gateway.lastRequest("inspectionResolveTarget")
        verify(first.args[0].request_revision < second.args[0].request_revision)
        gateway.respond(first, ok(targetResolutionReport(first, "resolved", [{
            entity_ref: {
                layer: "l1",
                network_scope: scope("network-a"),
                entity_kind: "block",
                canonical_key: "block:42",
                block_id: 42,
                block_hash: null
            }
        }])))
        compare(accepted, null)
        compare(zoneState.targetResolutionCandidates.length, 0)

        gateway.respond(second, ok(targetResolutionReport(second, "ambiguous", [{
            entity_ref: {
                layer: "l1",
                network_scope: scope("network-a"),
                entity_kind: "block",
                canonical_key: "block:43",
                block_id: 43,
                block_hash: null
            }
        }, {
            entity_ref: {
                layer: "l2",
                network_scope: scope("network-a"),
                channel_id: "zone-a",
                zone_kind: "sequencer_zone",
                entity_kind: "block",
                canonical_key: "block:43:" + "a".repeat(64),
                source: { kind: "exact", source_id: "idx-a", source_role: "indexer" }
            }
        }])))
        verify(accepted !== null)
        compare(zoneState.targetResolutionStatus, "ambiguous")
        compare(zoneState.targetResolutionCandidates.length, 2)

        const oldRevision = zoneState.targetResolutionRequestRevision
        zoneState.activeZoneContext = Object.assign({}, zoneState.activeZoneContext, {
            context_revision: zoneState.activeZoneContext.context_revision + 1
        })
        verify(zoneState.targetResolutionRequestRevision > oldRevision)
        compare(zoneState.targetResolutionCandidates.length, 0)
    }

    function test_zone_capabilities_gate_provisional_collaboration_only() {
        loadConfiguredL2Zone()
        verify(zoneState.l2Capability("").enabled)
        verify(zoneState.collaborationCapability().enabled)

        zoneState.activeZoneContext = Object.assign({}, zoneState.activeZoneContext, {
            network_scope: {
                kind: "finalized_anchor",
                genesis_time: "2026-01-01T00:00:00Z",
                block_slot: 1,
                block_id: "a".repeat(64),
                parent_id: "b".repeat(64)
            },
            context_revision: zoneState.activeZoneContext.context_revision + 1
        })

        verify(zoneState.l2Capability("").enabled)
        verify(!zoneState.collaborationCapability().enabled)
        verify(zoneState.collaborationCapability().reason.indexOf("genesis") >= 0)
    }

    function targetResolutionReport(request, status, candidates) {
        const payload = request.args[0]
        return {
            report_kind: "inspection.target_resolution",
            schema_version: 1,
            query: payload.query,
            request_revision: payload.request_revision,
            context_revision: payload.active_zone_context.context_revision,
            status: status,
            candidates: candidates,
            recovery: null,
            warnings: []
        }
    }
}
