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
            sequencer_sources: [{ source_id: "src-a", target: { kind: "rpc", endpoint: "https://seq-a" } }],
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
}
