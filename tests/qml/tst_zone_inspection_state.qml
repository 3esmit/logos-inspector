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
}
