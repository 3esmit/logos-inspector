import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "ZoneInspectionState"

    property var zoneState: null
    property var l2State: null
    property var l2BlockState: null
    property var l2AccountState: null
    property var l2ToolState: null
    property var evidenceState: null
    property var sourceEditorState: null
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

    QtObject {
        id: decodeRegistry

        property int count: 0
    }

    QtObject {
        id: decodeSocial

        property int sharedIdlRevision: 0
    }

    QtObject {
        id: decodeAppModel

        property var registeredIdls: decodeRegistry
        property var candidates: []
        property var transactionIdlEntries: []
        property int accountIdlSelectionRevision: 0
        property var social: decodeSocial

        function idlEntriesForProgram(programId) {
            const expected = String(programId || "")
            return transactionIdlEntries.filter(function (entry) {
                return String(entry && entry.programIdHex || "") === expected
            })
        }

        function accountDecodeCandidates(accountId, ownerProgramId) {
            return candidates.slice()
        }

        function programDecodeCandidatePayload(values) {
            return Array.isArray(values) ? values.slice() : []
        }

        function selectAccountDecodeSessionAsync(dataHex, accountId, ownerProgramId,
                candidatesValue, callback) {
            return gateway.request("selectAccountDecodeSession", [
                String(dataHex || ""),
                String(accountId || ""),
                Array.isArray(candidatesValue) ? candidatesValue : [],
                String(ownerProgramId || "")
            ], callback)
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
        decodeRegistry.count = 0
        decodeAppModel.candidates = []
        decodeAppModel.transactionIdlEntries = []
        decodeAppModel.accountIdlSelectionRevision = 0
        decodeSocial.sharedIdlRevision = 0
        zoneState = stateComponent.createObject(testRoot, {
            gateway: gateway
        })
        verify(zoneState !== null)
        l2State = zoneState.l2
        l2BlockState = l2State.blocks
        l2AccountState = l2State.accounts
        l2ToolState = l2State.tools
        evidenceState = zoneState.evidence
        sourceEditorState = zoneState.sourceEditor
        statusRefreshSpy.target = zoneState
        statusRefreshSpy.clear()
        zoneState.zoneSummariesChanged.connect(function () {
            activeZoneSeenOnSummaryChange = zoneState.activeZoneId
        })
    }

    function cleanup() {
        statusRefreshSpy.target = null
        l2State = null
        l2BlockState = null
        l2AccountState = null
        l2ToolState = null
        evidenceState = null
        sourceEditorState = null
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

    function test_focused_l2_states_project_active_context_changes() {
        let blockChanges = 0
        let toolChanges = 0
        l2BlockState.activeZoneContextChanged.connect(function () {
            blockChanges += 1
        })
        l2ToolState.activeZoneContextChanged.connect(function () {
            toolChanges += 1
        })
        const context = {
            network_scope: scope("network-a"),
            channel_id: "a".repeat(64),
            zone_kind: "sequencer_zone",
            context_revision: 1
        }

        zoneState.activeZoneContext = context

        compare(l2BlockState.activeZoneContext, context)
        compare(l2ToolState.activeZoneContext, context)
        compare(blockChanges, 1)
        compare(toolChanges, 1)
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

    function zoneRow(channelId, kind, selectedSourceId, indexerSourceId, configRevision) {
        const zoneKind = String(kind || "sequencer_zone")
        const row = {
            channel_id: String(channelId),
            active_zone_context_fields: {
                network_scope: scope("network-a"),
                channel_id: String(channelId),
                zone_kind: zoneKind,
                selected_sequencer_source_id: selectedSourceId || null,
                indexer_source_id: indexerSourceId || null,
                source_config_revision: configRevision === undefined
                    ? (selectedSourceId || indexerSourceId ? 1 : 0)
                    : Number(configRevision)
            },
            kind: zoneKind,
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
        const row = zoneRow("zone-a", "sequencer_zone", "seq-a", "idx-a", 7)
        loadOneZone(row)
        verify(zoneState.activateZone("zone-a"))
        gateway.respondNext("zoneDetail", ok(detailReport(row, configuredSourceConfig())))
        verify(l2State.l2ReadEnabled)
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

    function accountDecodeRequest(dataHex) {
        for (let i = gateway.requests.length - 1; i >= 0; --i) {
            const entry = gateway.requests[i]
            const args = entry.args || []
            if (entry.method === "selectAccountDecodeSession" && !entry.completed
                    && args.length > 0 && String(args[0] || "") === String(dataHex || "")) {
                return entry
            }
        }
        return null
    }

    function accountDecodeSession(accountType, value) {
        return {
            selected: {
                evidence: {
                    key: "token-idl",
                    name: "Token Fixture",
                    programIdHex: "cd".repeat(32),
                    accountType: String(accountType),
                    source: "local"
                },
                report: {
                    account_id: "account-a",
                    account_type: String(accountType),
                    consumed_bytes: 2,
                    total_bytes: 2,
                    remaining_bytes: 0,
                    decoded: { value: String(value) },
                    rows: [{ path: "value", value: String(value) }]
                }
            },
            partial: null,
            firstError: null
        }
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

    function test_startup_selects_the_only_configured_sequencer_zone() {
        configure("https://l1.example", 1)
        const row = zoneRow("zone-a", "sequencer_zone", "seq-a", "idx-a", 7)

        loadOneZone(row)

        compare(zoneState.activeZoneId, "zone-a")
        compare(zoneState.activeZoneContext.selected_sequencer_source_id, "seq-a")
        compare(zoneState.activeZoneContext.indexer_source_id, "idx-a")
        verify(zoneState.detailInFlight)
        compare(gateway.requestCount("zoneDetail"), 1)
    }

    function test_startup_selection_does_not_fallback_after_the_zone_is_removed() {
        configure("https://l1.example", 1)
        const first = zoneRow("zone-a", "sequencer_zone", "seq-a", "idx-a", 7)
        const replacement = zoneRow("zone-b", "sequencer_zone", "seq-b", "idx-b", 8)
        loadOneZone(first)
        compare(zoneState.activeZoneId, "zone-a")

        verify(zoneState.pollStatus())
        gateway.respondNext("zoneCatalogStatus", ok(statusReport({
            verification: "verified",
            coverage: { status: "complete", gap_count: 0 },
            ingestion: { worker_running: false, discovered_zone_count: 1 },
            summary_revision: 2
        })))
        gateway.respondNext("zonesSummary", ok(summaryReport(2, {
            kind: "delta",
            upserts: [replacement],
            removed_zone_ids: ["zone-a"]
        }, null)))

        compare(zoneState.activeZoneId, "")
        compare(gateway.requestCount("zoneDetail"), 1)
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

    function test_testnet_default_topology_is_explicit_in_catalog_configuration() {
        zoneState.sourceDescriptor = {
            kind: "direct_http",
            endpoint: "http://127.0.0.1:8080/",
            default_topology: "logos_testnet"
        }
        zoneState.start()

        const request = gateway.pendingRequest("zoneCatalogConfigure")
        verify(request !== null)
        compare(request.args[0].source.kind, "direct_http")
        compare(request.args[0].source.endpoint, "http://127.0.0.1:8080/")
        compare(request.args[0].source.default_topology, "logos_testnet")
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

        sourceEditorState.applyChannelSourceConfig({
            expected_config_revision: 1,
            mutation: {
                kind: "select_sequencer",
                source_id: null
            }
        }, function (response) {
            mutationCallbackResponse = response
        })
        verify(sourceEditorState.sourceMutationInFlight)
        compare(zoneState.activeZoneContext.selected_sequencer_source_id, "src-a")

        gateway.respondNext("channelSourceConfigApply", ok({
            report_kind: "zones.channel_source_config",
            schema_version: 1,
            source_revision: 1,
            catalog_revision: 1,
            source_config_epoch: 2,
            observation_revision: 1,
            summary_revision: 2,
            active_zone_context_fields: {
                network_scope: scope("network-a"),
                channel_id: "zone-a",
                zone_kind: "sequencer_zone",
                selected_sequencer_source_id: null,
                indexer_source_id: "idx-a",
                source_config_revision: 2
            },
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
        sourceEditorState.applyChannelSourceConfig({
            expected_config_revision: 2,
            mutation: { kind: "remove_indexer" }
        })
        gateway.respondNext("channelSourceConfigApply", failed("revision conflict"))
        compare(zoneState.contextRevision, contextAfterSuccess)
        compare(zoneState.activeZoneContext.indexer_source_id, "idx-a")
        compare(sourceEditorState.sourceMutationError, "revision conflict")
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

        verify(evidenceState.loadEvidence("all"))
        const firstRequest = gateway.lastRequest("zoneEvidencePage")
        compare(firstRequest.args[0].channel_id, "zone-a")
        compare(firstRequest.args[0].catalog_revision, 1)
        compare(firstRequest.args[0].filter, "all")
        const evidenceA = evidenceRow("evidence-a", 10, "channel_configuration")
        const evidenceB = evidenceRow("evidence-b", 12, "raw_inscription")
        gateway.respondNext("zoneEvidencePage", ok(evidencePageReport([evidenceA], "cursor-2", "all")))
        compare(evidenceState.evidenceRows.length, 1)
        compare(evidenceState.evidenceNextCursor, "cursor-2")

        verify(evidenceState.loadMoreEvidence())
        compare(gateway.lastRequest("zoneEvidencePage").args[0].cursor, "cursor-2")
        gateway.respondNext("zoneEvidencePage", ok(evidencePageReport([evidenceB], null, "all")))
        compare(evidenceState.evidenceRows.length, 2)
        compare(evidenceState.evidenceNextCursor, "")

        verify(evidenceState.openEvidence(evidenceB))
        compare(gateway.lastRequest("zoneEvidenceDetail").args[0].reference.evidence_id, "evidence-b")
        gateway.respondNext("zoneEvidenceDetail", ok(evidenceDetailReport(evidenceB, "session-b")))
        compare(evidenceState.evidenceDetail.row.reference.evidence_id, "evidence-b")
        verify(!evidenceState.evidencePayloadDone)

        verify(evidenceState.loadNextEvidencePayloadChunk())
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
        compare(evidenceState.evidencePayloadChunks.length, 1)
        compare(evidenceState.evidencePayloadChunks[0].text, "hello")
        compare(evidenceState.evidencePayloadOffset, 5)
        verify(evidenceState.evidencePayloadDone)

        evidenceState.closeEvidenceDetail()
        compare(evidenceState.evidenceDetail, null)
        compare(gateway.requestCount("zoneEvidencePayloadRelease"), 1)
        compare(gateway.lastRequest("zoneEvidencePayloadRelease").args[0].session_id, "session-b")
    }

    function test_l2_block_pages_carry_context_and_preserve_conflicts() {
        loadConfiguredL2Zone()

        verify(l2BlockState.refreshL2Blocks() !== null)
        const request = gateway.lastRequest("zoneL2Blocks")
        const payload = request.args[0]
        compare(payload.context.channel_id, "zone-a")
        compare(payload.context.selected_sequencer_source_id, "seq-a")
        compare(payload.context.indexer_source_id, "idx-a")
        compare(payload.context.source_config_revision, 7)
        compare(payload.context.context_revision, zoneState.activeZoneContext.context_revision)
        compare(payload.request_revision, l2BlockState.l2BlocksRequestRevision)
        compare(payload.query.cursor, null)
        compare(payload.query.limit, 25)
        compare(payload.query.exact_source_id, null)
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

        compare(l2BlockState.l2BlockRows.length, 2)
        compare(l2BlockState.l2BlockRows[0].summary.block_id, 12)
        verify(l2BlockState.l2BlockRows[0].summary.block_hash !== l2BlockState.l2BlockRows[1].summary.block_hash)
        compare(l2BlockState.l2BlockRows[0].observations[0].source_id, "idx-a")
        compare(l2BlockState.l2BlockRows[1].observations[0].finality, "provisional")
        compare(l2BlockState.l2BlocksDistinctCount, 1)
        verify(l2BlockState.l2BlocksHasMore)

        verify(l2BlockState.loadMoreL2Blocks() !== null)
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
        compare(l2BlockState.l2BlockRows.length, 3)
        compare(l2BlockState.l2BlocksDistinctCount, 2)
        verify(!l2BlockState.l2BlocksHasMore)
    }

    function test_l2_sequencer_block_pages_keep_exact_source_across_cursor() {
        loadConfiguredL2Zone()

        verify(l2BlockState.refreshL2BlocksForSource("seq-a") !== null)
        const request = gateway.lastRequest("zoneL2Blocks")
        compare(request.args[0].query.exact_source_id, "seq-a")
        compare(l2BlockState.l2BlocksExactSourceId, "seq-a")

        const provisional = l2Source(
            "seq-a", "sequencer", "provisional", "live")
        gateway.respond(request, ok(l2Report(request, "lez.blocks", {
            outcome: "found",
            value: {
                rows: [l2Block(15, "a".repeat(64), [provisional])],
                next_cursor: "sequencer-next",
                has_more: true,
                distinct_block_ids: 1,
                source_heads: [{
                    source_id: "seq-a",
                    source_role: "sequencer",
                    block_id: 15,
                    block_hash: "a".repeat(64)
                }]
            }
        }, {
            route: {
                policy: "exact_source",
                attempts: [{
                    source_id: "seq-a",
                    source_role: "sequencer"
                }]
            }
        })))
        compare(l2BlockState.l2BlockRows.length, 1)
        compare(l2BlockState.l2BlockRows[0].observations[0].source_role,
            "sequencer")

        verify(l2BlockState.loadMoreL2Blocks() !== null)
        const nextRequest = gateway.lastRequest("zoneL2Blocks")
        compare(nextRequest.args[0].query.cursor, "sequencer-next")
        compare(nextRequest.args[0].query.exact_source_id, "seq-a")
    }

    function test_l2_sequencer_empty_block_page_accepts_exact_route_without_head() {
        loadConfiguredL2Zone()

        verify(l2BlockState.refreshL2BlocksForSource("seq-a") !== null)
        const request = gateway.lastRequest("zoneL2Blocks")
        gateway.respond(request, ok(l2Report(request, "lez.blocks", {
            outcome: "found",
            value: {
                rows: [],
                next_cursor: null,
                has_more: false,
                distinct_block_ids: 0,
                source_heads: []
            }
        }, {
            route: {
                policy: "exact_source",
                attempts: [{
                    source_id: "seq-a",
                    source_role: "sequencer"
                }]
            }
        })))

        compare(l2BlockState.l2BlocksError, "")
        verify(l2BlockState.l2BlocksLoaded)
        compare(l2BlockState.l2BlockRows.length, 0)
    }

    function test_l2_block_detail_rejects_superseded_reply_and_resolves_exact_source() {
        loadConfiguredL2Zone()
        const firstSummary = l2Block(12, "a".repeat(64), []).summary
        const secondSummary = l2Block(12, "b".repeat(64), []).summary

        verify(l2BlockState.openL2Block(firstSummary, "idx-a") !== null)
        const firstRequest = gateway.lastRequest("zoneL2BlockDetail")
        verify(l2BlockState.openL2Block(secondSummary, "seq-a") !== null)
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
        compare(l2BlockState.l2BlockDetail, null)

        gateway.respondNext("zoneL2BlockDetail", ok(l2Report(secondRequest, "lez.block_detail", {
            outcome: "ambiguous",
            candidates: [{
                source_id: "seq-a",
                source_role: "sequencer",
                canonical_key: "block:12:" + secondSummary.block_hash
            }]
        })))
        compare(l2BlockState.l2BlockCandidates.length, 1)
        verify(l2BlockState.resolveL2BlockCandidate(l2BlockState.l2BlockCandidates[0]) !== null)
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
        compare(l2BlockState.l2BlockDetail.source.source_id, "seq-a")
        compare(l2BlockState.l2BlockDetail.source.retrieval, "memory_cache")
        compare(l2BlockState.l2BlockDetail.transactions.length, 1)
    }

    function test_l2_transaction_detail_auto_traces_same_source_and_fences_trace_race() {
        zoneState.appModel = decodeAppModel
        decodeAppModel.transactionIdlEntries = [{
            key: "first-token-idl",
            name: "First Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"token\"}",
            source: "local"
        }, {
            key: "replacement-token-idl",
            name: "Replacement Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"replacement\"}",
            source: "local"
        }]
        decodeRegistry.count = 2
        loadConfiguredL2Zone()
        const transaction = l2Transaction("e".repeat(64))
        transaction.program_id_hex = "cd".repeat(32)

        verify(l2BlockState.openL2Transaction(transaction.hash, "seq-a") !== null)
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

        compare(l2BlockState.l2TransactionDetail.source.source_id, "seq-a")
        const firstTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        verify(firstTraceRequest !== null)
        compare(firstTraceRequest.args[0].query.transaction_id, transaction.hash)
        compare(firstTraceRequest.args[0].query.exact_source_id, "seq-a")
        compare(firstTraceRequest.args[0].query.idl_program_id, "cd".repeat(32))

        decodeAppModel.transactionIdlEntries = [{
            key: "replacement-token-idl",
            name: "Replacement Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"replacement\"}",
            source: "local"
        }]
        decodeRegistry.count = 1
        tryVerify(function () {
            return gateway.requestCount("zoneL2TransactionTrace") === 2
        })
        const secondTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        verify(firstTraceRequest.args[0].request_revision < secondTraceRequest.args[0].request_revision)
        compare(secondTraceRequest.args[0].query.idl_program_id, "cd".repeat(32))
        const staleTrace = {
            transaction: transaction,
            trace: { hash: "stale", kind: "public", source: "local", capabilities: [], limitations: [], steps: [], inspection: {}, decoded_instruction: null },
            source: l2Source("seq-a", "sequencer", "provisional")
        }
        gateway.respondNext("zoneL2TransactionTrace", ok(l2Report(firstTraceRequest, "lez.transaction_trace", {
            outcome: "found",
            value: staleTrace
        })))
        compare(l2BlockState.l2TransactionTrace, null)

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
                decoded_instruction: {
                    program_id: "cd".repeat(32),
                    idl_name: "replacement",
                    instruction: "transfer",
                    variant_index: 0,
                    accounts: [{ path: "sender", value: "account-a" }],
                    args: [{ path: "amount_to_transfer: u128", value: "1234567" }],
                    remaining_words: []
                }
            },
            source: l2Source("seq-a", "sequencer", "provisional", "memory_cache")
        }
        gateway.respondNext("zoneL2TransactionTrace", ok(l2Report(secondTraceRequest, "lez.transaction_trace", {
            outcome: "found",
            value: currentTrace
        })))
        compare(l2BlockState.l2TransactionTrace.trace.hash, transaction.hash)
        compare(l2BlockState.l2TransactionTrace.source.source_id, "seq-a")
        compare(l2BlockState.l2TransactionTrace.source.retrieval, "memory_cache")
        compare(l2BlockState.l2TransactionTrace.trace.steps.length, 1)
        compare(l2BlockState.l2TransactionTrace.trace.decoded_instruction.idl_name,
            "replacement")
    }

    function test_l2_transaction_redecodes_loaded_trace_when_matching_idl_is_registered() {
        zoneState.appModel = decodeAppModel
        loadConfiguredL2Zone()
        const transaction = l2Transaction("f".repeat(64))
        transaction.program_id_hex = "cd".repeat(32)

        verify(l2BlockState.openL2Transaction(transaction.hash, "seq-a") !== null)
        const detailRequest = gateway.lastRequest("zoneL2Transaction")
        gateway.respond(detailRequest, ok(l2Report(detailRequest, "lez.transaction", {
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

        const genericTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        compare(genericTraceRequest.args[0].query.idl_program_id, null)
        decodeAppModel.transactionIdlEntries = [{
            key: "token-idl",
            name: "Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"token\"}",
            source: "local"
        }]
        decodeRegistry.count = 1
        compare(gateway.requestCount("zoneL2TransactionTrace"), 1)
        tryVerify(function () {
            return gateway.requestCount("zoneL2TransactionTrace") === 2
        })
        const decodedTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        compare(decodedTraceRequest.args[0].query.idl_program_id, "cd".repeat(32))

        gateway.respond(genericTraceRequest, ok(l2Report(genericTraceRequest,
            "lez.transaction_trace", {
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
                    source: l2Source("seq-a", "sequencer", "provisional")
                }
            })))
        compare(l2BlockState.l2TransactionTrace, null)

        gateway.respond(decodedTraceRequest, ok(l2Report(decodedTraceRequest,
            "lez.transaction_trace", {
                outcome: "found",
                value: {
                    transaction: transaction,
                    trace: {
                        hash: transaction.hash,
                        kind: transaction.kind,
                        source: "local_derivation",
                        capabilities: ["IDL decode"],
                        limitations: [],
                        steps: [],
                        inspection: {},
                        decoded_instruction: {
                            program_id: "cd".repeat(32),
                            idl_name: "token",
                            instruction: "transfer",
                            variant_index: 0,
                            accounts: [{ path: "sender", value: "account-a" }, {
                                path: "recipient", value: "account-b"
                            }],
                            args: [{ path: "amount_to_transfer: u128", value: "1234567" }],
                            remaining_words: []
                        }
                    },
                    source: l2Source("seq-a", "sequencer", "provisional")
                }
            })))
        compare(l2BlockState.l2TransactionTrace.trace.decoded_instruction.instruction,
            "transfer")
        compare(l2BlockState.l2TransactionTrace.trace.decoded_instruction.args[0].value,
            "1234567")

        decodeAppModel.transactionIdlEntries = []
        decodeRegistry.count = 0
        tryVerify(function () {
            return gateway.requestCount("zoneL2TransactionTrace") === 3
        })
        const postRemovalTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        compare(postRemovalTraceRequest.args[0].query.idl_program_id, null)
    }

    function test_l2_transaction_redecodes_when_registry_reload_keeps_same_count() {
        zoneState.appModel = decodeAppModel
        decodeAppModel.transactionIdlEntries = [{
            key: "first-token-idl",
            name: "First Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"first\"}",
            source: "local"
        }]
        decodeRegistry.count = 1
        loadConfiguredL2Zone()
        const transaction = l2Transaction("b".repeat(64))
        transaction.program_id_hex = "cd".repeat(32)

        verify(l2BlockState.openL2Transaction(transaction.hash, "seq-a") !== null)
        const detailRequest = gateway.lastRequest("zoneL2Transaction")
        gateway.respond(detailRequest, ok(l2Report(detailRequest, "lez.transaction", {
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
        const firstTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        gateway.respond(firstTraceRequest, ok(l2Report(firstTraceRequest,
            "lez.transaction_trace", {
                outcome: "found",
                value: {
                    transaction: transaction,
                    trace: {
                        hash: transaction.hash,
                        kind: transaction.kind,
                        source: "local_derivation",
                        capabilities: ["IDL decode"],
                        limitations: [],
                        steps: [],
                        inspection: {},
                        decoded_instruction: {
                            program_id: "cd".repeat(32),
                            idl_name: "first",
                            instruction: "transfer",
                            variant_index: 0,
                            accounts: [],
                            args: [],
                            remaining_words: []
                        }
                    },
                    source: l2Source("seq-a", "sequencer", "provisional")
                }
            })))
        compare(l2BlockState.l2TransactionTrace.trace.decoded_instruction.idl_name, "first")

        decodeAppModel.transactionIdlEntries = [{
            key: "replacement-token-idl",
            name: "Replacement Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"replacement\"}",
            source: "local"
        }]
        decodeRegistry.count = 0
        decodeRegistry.count = 1
        compare(gateway.requestCount("zoneL2TransactionTrace"), 1)
        tryVerify(function () {
            return gateway.requestCount("zoneL2TransactionTrace") === 2
        })
        const replacementTraceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        compare(replacementTraceRequest.args[0].query.idl_program_id, "cd".repeat(32))
        gateway.respond(replacementTraceRequest, ok(l2Report(replacementTraceRequest,
            "lez.transaction_trace", {
                outcome: "found",
                value: {
                    transaction: transaction,
                    trace: {
                        hash: transaction.hash,
                        kind: transaction.kind,
                        source: "local_derivation",
                        capabilities: ["IDL decode"],
                        limitations: [],
                        steps: [],
                        inspection: {},
                        decoded_instruction: {
                            program_id: "cd".repeat(32),
                            idl_name: "replacement",
                            instruction: "transfer",
                            variant_index: 0,
                            accounts: [],
                            args: [],
                            remaining_words: []
                        }
                    },
                    source: l2Source("seq-a", "sequencer", "provisional")
                }
            })))
        compare(l2BlockState.l2TransactionTrace.trace.decoded_instruction.idl_name,
            "replacement")
    }

    function test_l2_transaction_trace_skips_unmatched_registered_idl() {
        zoneState.appModel = decodeAppModel
        decodeAppModel.transactionIdlEntries = [{
            key: "other-token-idl",
            name: "Other Token Fixture",
            programIdHex: "ef".repeat(32),
            json: "{\"name\":\"other\"}",
            source: "local"
        }]
        decodeRegistry.count = 1
        loadConfiguredL2Zone()
        const transaction = l2Transaction("a".repeat(64))
        transaction.program_id_hex = "cd".repeat(32)

        verify(l2BlockState.openL2Transaction(transaction.hash, "seq-a") !== null)
        const detailRequest = gateway.lastRequest("zoneL2Transaction")
        gateway.respond(detailRequest, ok(l2Report(detailRequest, "lez.transaction", {
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

        const traceRequest = gateway.lastRequest("zoneL2TransactionTrace")
        compare(traceRequest.args[0].query.idl_program_id, null)
    }

    function test_l2_trace_rejects_different_source_provenance() {
        loadConfiguredL2Zone()
        const transaction = l2Transaction("9".repeat(64))
        verify(l2BlockState.requestL2TransactionTrace(transaction.hash, "seq-a") !== null)
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
        compare(l2BlockState.l2TransactionTrace, null)
        compare(l2BlockState.l2TransactionTraceError,
            "Transaction trace returned different source provenance.")
    }

    function test_l2_success_with_mismatched_context_never_replaces_visible_rows() {
        loadConfiguredL2Zone()
        verify(l2BlockState.refreshL2Blocks() !== null)
        const request = gateway.lastRequest("zoneL2Blocks")
        const wrongContext = l2State.l2RequestContext()
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
        compare(l2BlockState.l2BlockRows.length, 0)
        verify(!l2BlockState.l2BlocksLoaded)
    }

    function test_l2_account_snapshots_are_independent_and_historical_is_exact() {
        loadConfiguredL2Zone()
        verify(l2AccountState.inspectL2Account("account-a"))
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
        compare(l2AccountState.l2AccountProvisional.account.balance, "19")
        compare(l2AccountState.l2AccountProvisional.anchor_state, "moving")
        compare(l2AccountState.l2AccountFinalized, null)

        gateway.respondNext("zoneL2AccountActivity", failed("activity unavailable"))
        compare(l2AccountState.l2AccountActivityError, "activity unavailable")
        compare(l2AccountState.l2AccountProvisional.account.balance, "19")

        const finalized = l2AccountSnapshot("account-a", "17",
            l2Source("idx-a", "indexer", "finalized"), "exact", 12)
        gateway.respond(finalizedRequest, ok(l2Report(finalizedRequest, "lez.account", {
            outcome: "found",
            value: finalized
        })))
        compare(l2AccountState.l2AccountFinalized.account.balance, "17")
        compare(l2AccountState.l2AccountProvisional.account.balance, "19")
        compare(l2AccountState.l2AccountFinalized.source.source_role, "indexer")
        compare(l2AccountState.l2AccountProvisional.source.source_role, "sequencer")

        verify(l2AccountState.requestL2HistoricalAccount(9, "9".repeat(64)) !== null)
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
        compare(l2AccountState.l2AccountHistorical.account.balance, "11")
        compare(l2AccountState.l2AccountHistorical.source.retrieval, "memory_cache")
        compare(l2AccountState.l2AccountFinalized.account.balance, "17")
        compare(l2AccountState.l2AccountProvisional.account.balance, "19")
    }

    function test_l2_sequencer_account_requests_only_provisional_source() {
        loadConfiguredL2Zone()

        verify(l2AccountState.inspectL2SequencerAccount("account-a"))
        compare(gateway.requestCount("zoneL2Account"), 1)
        compare(gateway.requestCount("zoneL2AccountActivity"), 0)
        const request = gateway.lastRequest("zoneL2Account")
        compare(request.args[0].query.snapshot.kind, "provisional")
        compare(request.args[0].query.exact_source_id, "seq-a")
        compare(l2AccountState.l2AccountFinalized, null)

        const provisional = l2AccountSnapshot("account-a", "19",
            l2Source("seq-a", "sequencer", "provisional"), "exact", 14)
        gateway.respond(request, ok(l2Report(request, "lez.account", {
            outcome: "found",
            value: provisional
        })))
        compare(l2AccountState.l2AccountProvisional.source.source_role,
            "sequencer")

        verify(l2AccountState.refreshL2SequencerAccount())
        compare(gateway.requestCount("zoneL2Account"), 2)
        compare(gateway.lastRequest("zoneL2Account")
            .args[0].query.exact_source_id, "seq-a")
        compare(gateway.requestCount("zoneL2AccountActivity"), 0)
    }

    function test_l2_account_matching_idl_decodes_snapshots_without_cross_cancellation() {
        zoneState.appModel = decodeAppModel
        verify(l2State.appModel === decodeAppModel)
        verify(l2AccountState.appModel === decodeAppModel)
        decodeRegistry.count = 1
        decodeAppModel.candidates = [{
            key: "token-idl",
            name: "Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"token\"}",
            source: "local"
        }]
        loadConfiguredL2Zone()
        verify(l2AccountState.inspectL2Account("account-a"))

        const finalized = l2AccountSnapshot("account-a", "17",
            l2Source("idx-a", "indexer", "finalized"), "exact", 12)
        finalized.account.data_hex = "0103"
        const provisional = l2AccountSnapshot("account-a", "19",
            l2Source("seq-a", "sequencer", "provisional"), "moving", 14)
        provisional.account.data_hex = "0102"
        provisional.after_anchor = {
            block_id: 15,
            block_hash: "f".repeat(64)
        }
        const finalizedRequest = l2AccountRequest("finalized")
        const provisionalRequest = l2AccountRequest("provisional")
        gateway.respond(provisionalRequest, ok(l2Report(provisionalRequest, "lez.account", {
            outcome: "found",
            value: provisional
        })))
        gateway.respond(finalizedRequest, ok(l2Report(finalizedRequest, "lez.account", {
            outcome: "found",
            value: finalized
        })))

        const provisionalDecode = accountDecodeRequest("0102")
        const finalizedDecode = accountDecodeRequest("0103")
        verify(provisionalDecode !== null)
        verify(finalizedDecode !== null)
        compare(provisionalDecode.args[1], "account-a")
        compare(provisionalDecode.args[3], "cd".repeat(32))

        gateway.respond(finalizedDecode, ok(accountDecodeSession("FinalizedToken", "17")))
        gateway.respond(provisionalDecode, ok(accountDecodeSession("TokenDefinition", "Pebble")))
        compare(l2AccountState.l2AccountFinalizedDecode.report.account_type, "FinalizedToken")
        compare(l2AccountState.l2AccountProvisionalDecode.report.account_type, "TokenDefinition")
        compare(l2AccountState.l2AccountProvisionalDecode.report.rows[0].value, "Pebble")
    }

    function test_l2_account_redecodes_loaded_snapshot_when_idl_is_registered() {
        zoneState.appModel = decodeAppModel
        loadConfiguredL2Zone()
        verify(l2AccountState.inspectL2Account("account-a"))
        const provisionalRequest = l2AccountRequest("provisional")
        const provisional = l2AccountSnapshot("account-a", "19",
            l2Source("seq-a", "sequencer", "provisional"), "exact", 14)
        provisional.account.data_hex = "0102"
        gateway.respond(provisionalRequest, ok(l2Report(provisionalRequest, "lez.account", {
            outcome: "found",
            value: provisional
        })))
        compare(gateway.requestCount("selectAccountDecodeSession"), 0)

        decodeAppModel.candidates = [{
            key: "token-idl",
            name: "Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"token\"}",
            source: "local"
        }]
        decodeRegistry.count = 1
        tryVerify(function () {
            return accountDecodeRequest("0102") !== null
        })
        const decodeRequest = accountDecodeRequest("0102")
        gateway.respond(decodeRequest, ok(accountDecodeSession("TokenDefinition", "Pebble")))
        compare(l2AccountState.l2AccountProvisionalDecode.report.account_type, "TokenDefinition")
    }

    function test_l2_account_redecodes_loaded_snapshot_when_decode_candidates_change() {
        zoneState.appModel = decodeAppModel
        decodeRegistry.count = 1
        loadConfiguredL2Zone()
        verify(l2AccountState.inspectL2Account("account-a"))
        const provisionalRequest = l2AccountRequest("provisional")
        const provisional = l2AccountSnapshot("account-a", "19",
            l2Source("seq-a", "sequencer", "provisional"), "exact", 14)
        provisional.account.data_hex = "0102"
        gateway.respond(provisionalRequest, ok(l2Report(provisionalRequest, "lez.account", {
            outcome: "found",
            value: provisional
        })))
        compare(gateway.requestCount("selectAccountDecodeSession"), 0)

        decodeAppModel.candidates = [{
            key: "token-idl",
            name: "Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"token\"}",
            source: "local"
        }]
        decodeAppModel.accountIdlSelectionRevision += 1
        tryVerify(function () {
            return accountDecodeRequest("0102") !== null
        })
        const selectionDecode = accountDecodeRequest("0102")
        gateway.respond(selectionDecode, ok(accountDecodeSession("TokenDefinition", "Pebble")))
        compare(l2AccountState.l2AccountProvisionalDecode.report.account_type, "TokenDefinition")

        decodeAppModel.candidates = []
        decodeAppModel.accountIdlSelectionRevision += 1
        compare(l2AccountState.l2AccountProvisionalDecode, null)
        decodeAppModel.candidates = [{
            key: "shared-token-idl",
            name: "Shared Token Fixture",
            programIdHex: "cd".repeat(32),
            json: "{\"name\":\"token\"}",
            source: "shared"
        }]
        decodeSocial.sharedIdlRevision += 1
        tryVerify(function () {
            return accountDecodeRequest("0102") !== null
                && gateway.requestCount("selectAccountDecodeSession") === 2
        })
        const sharedDecode = accountDecodeRequest("0102")
        gateway.respond(sharedDecode, ok(accountDecodeSession("TokenDefinition", "Pebble")))
        compare(l2AccountState.l2AccountProvisionalDecode.report.rows[0].value, "Pebble")
    }

    function test_l2_account_activity_appends_oldest_first_without_touching_snapshots() {
        loadConfiguredL2Zone()
        verify(l2AccountState.inspectL2Account("account-a"))
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
        compare(l2AccountState.l2AccountActivityRows.length, 2)
        compare(l2AccountState.l2AccountActivityRows[0].transaction_id, "tx-oldest")
        compare(l2AccountState.l2AccountActivityRows[1].transaction_id, "tx-next")
        verify(l2AccountState.l2AccountActivityHasMore)

        verify(l2AccountState.loadMoreL2AccountActivity())
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
        compare(l2AccountState.l2AccountActivityRows.length, 3)
        compare(l2AccountState.l2AccountActivityRows[2].transaction_id, "tx-newest")
        verify(!l2AccountState.l2AccountActivityHasMore)
        compare(l2AccountState.l2AccountFinalized, null)
        compare(l2AccountState.l2AccountProvisional, null)
    }

    function test_l2_sequencer_tools_use_selected_exact_source_and_isolated_slots() {
        loadConfiguredL2Zone()
        verify(l2ToolState.refreshL2Programs() !== null)
        const programsRequest = gateway.lastRequest("zoneL2Programs")
        compare(programsRequest.args[0].query.exact_source_id, "seq-a")
        gateway.respond(programsRequest, ok(l2Report(programsRequest, "lez.programs", {
            outcome: "found",
            value: {
                programs: [{ label: "System", base58: "program-58", hex: "ab".repeat(32) }],
                source: l2Source("seq-a", "sequencer", "provisional")
            }
        })))
        compare(l2ToolState.l2Programs.length, 1)

        verify(l2ToolState.requestL2CommitmentProof("cd".repeat(32)) !== null)
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
        compare(l2ToolState.l2CommitmentProof.leaf_index, 4)
        compare(l2ToolState.l2Programs.length, 1)

        verify(l2ToolState.requestL2AccountNonces(["account-a", "account-b"]) !== null)
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
        compare(l2ToolState.l2AccountNonces.length, 2)
        compare(l2ToolState.l2CommitmentProof.leaf_index, 4)
        compare(l2ToolState.l2Programs.length, 1)
    }

    function test_l2_transfer_pages_replace_window_and_restore_newer_page() {
        loadConfiguredL2Zone()
        verify(l2ToolState.refreshL2Transfers() !== null)
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
        compare(l2ToolState.l2TransferRecipients[0].received, "10")
        compare(l2ToolState.l2TransferRecipients[0].source,
            "transfer_outputs_and_account_refs")
        compare(l2ToolState.l2TransfersNewestBlock, 20)
        compare(l2ToolState.l2TransfersOldestBlock, 16)

        verify(l2ToolState.loadOlderL2Transfers() !== null)
        compare(l2ToolState.l2TransferRecipients[0].received, "10")
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
        compare(l2ToolState.l2TransferRecipients.length, 1)
        compare(l2ToolState.l2TransferRecipients[0].received, "3")
        compare(l2ToolState.l2TransfersHistory.length, 1)
        compare(l2ToolState.l2TransfersNewestBlock, 15)
        compare(l2ToolState.l2TransfersOldestBlock, 11)

        verify(l2ToolState.loadNewerL2Transfers())
        compare(l2ToolState.l2TransferRecipients[0].received, "10")
        compare(l2ToolState.l2TransfersNewestBlock, 20)
        compare(l2ToolState.l2TransfersOldestBlock, 16)
        compare(l2ToolState.l2TransfersHistory.length, 0)
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
        verify(l2State.l2Capability("").enabled)
        verify(l2State.collaborationCapability().enabled)

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

        verify(l2State.l2Capability("").enabled)
        verify(!l2State.collaborationCapability().enabled)
        verify(l2State.collaborationCapability().reason.indexOf("genesis") >= 0)
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
