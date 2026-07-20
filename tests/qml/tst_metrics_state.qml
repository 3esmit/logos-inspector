import QtQml
import QtTest
import "../../qml/state/domains"

TestCase {
    id: testRoot

    name: "MetricsState"

    property var dashboardOverview: null
    property var dashboardNode: null
    property var dashboardL1Blocks: []
    property int dashboardL1BlocksSlotTo: 0
    property var dashboardBlocks: []
    property var dashboardProvisionalBlocks: []
    property int joinedCompletionCount: 0
    property int invalidObservationCallbackCount: 0
    property string invalidObservationCallbackError: ""

    QtObject {
        id: sourceRouting

        property string storageEndpoint: "http://storage.invalid"
        property string storageCid: "z-test-cid"
        property string deliverySourceMode: "logoscore_cli"
        property string storageSourceMode: "logoscore_cli"
        property bool supportsLiveBlocks: true

        function blockchainSupportsCapability(capability) {
            return capability === "l1.live_blocks.observe" && supportsLiveBlocks
        }

        function deliverySourceReportArgs() {
            return [{
                source_mode: deliverySourceMode,
                inputs: { rest_endpoint: "http://delivery.invalid" },
                options: { runtime_diagnostics_enabled: true }
            }]
        }

        function deliverySourceView() {
            const mode = String(deliverySourceMode || "")
            return {
                mode: mode,
                resolvedMode: mode,
                connectionType: mode === "logoscore_cli"
                    ? "logoscore_cli" : (mode === "module" ? "module" : "rest")
            }
        }

        function storageSourceReportArgs(includeSensitiveProbe) {
            return [{
                source_mode: storageSourceMode,
                inputs: {
                    rest_endpoint: storageEndpoint
                },
                options: {
                    cid: includeSensitiveProbe === true ? storageCid : "",
                    privileged_debug_enabled: false,
                    runtime_diagnostics_enabled: true
                }
            }]
        }

        function deliverySourceLabel() { return "Delivery REST" }
        function storageSourceLabel() { return "Storage REST" }
    }

    QtObject {
        id: gateway

        property var requests: []
        property int capabilityRefreshCount: 0
        property int dashboardResultCount: 0
        property int dashboardStartCount: 0
        property bool dashboardResultOk: false
        property int projectDashboardCount: 0
        property int invalidatedDashboardCount: 0
        property bool synchronouslyRejectDashboard: false
        property int presentationSequence: 0
        property int activePresentationGeneration: 0
        property int presentationBeginCount: 0
        property int presentationCompleteCount: 0
        property bool lastPresentationError: false
        property string lastPresentationText: ""
        property string lastPresentationOwner: ""
        property var lastPresentationValue: null

        function reset() {
            requests = []
            capabilityRefreshCount = 0
            dashboardResultCount = 0
            dashboardStartCount = 0
            dashboardResultOk = false
            projectDashboardCount = 0
            invalidatedDashboardCount = 0
            synchronouslyRejectDashboard = false
            presentationSequence = 0
            activePresentationGeneration = 0
            presentationBeginCount = 0
            presentationCompleteCount = 0
            lastPresentationError = false
            lastPresentationText = ""
            lastPresentationOwner = ""
            lastPresentationValue = null
        }

        function appendRequest(value) {
            const next = requests.slice(0)
            next.push(value)
            requests = next
            return "request-" + next.length
        }

        function requestModuleAsyncUnobserved(moduleName, method, args, label,
                showResult, callback, acceptResponse) {
            return appendRequest({
                kind: "module",
                moduleName: moduleName,
                method: method,
                args: args,
                label: label,
                showResult: showResult,
                callback: callback,
                acceptResponse: acceptResponse
            })
        }

        function startBlockchainObservation(showResult, request, callback) {
            return appendRequest({
                kind: "blockchain",
                method: request.method,
                args: request.args,
                label: request.label,
                showResult: showResult,
                callback: callback,
                acceptResponse: null
            })
        }

        function startDashboardBlockchainOperation(request, callback) {
            dashboardStartCount += 1
            if (synchronouslyRejectDashboard) {
                callback({
                    ok: false,
                    value: null,
                    text: "",
                    error: "dashboard operation rejected"
                })
                return null
            }
            return appendRequest({
                kind: "dashboard",
                method: request.method,
                args: request.args,
                label: request.label,
                showResult: false,
                callback: callback,
                acceptResponse: null
            })
        }

        function beginObservationPresentation(label, owner) {
            presentationSequence += 1
            activePresentationGeneration = presentationSequence
            presentationBeginCount += 1
            lastPresentationOwner = String(owner || "")
            return {
                generation: presentationSequence,
                label: String(label || ""),
                owner: lastPresentationOwner
            }
        }

        function completeObservationPresentation(lease, title, text, isError, value) {
            if (!lease || Number(lease.generation || 0) !== activePresentationGeneration) {
                return false
            }
            activePresentationGeneration = 0
            presentationCompleteCount += 1
            lastPresentationError = isError === true
            lastPresentationText = String(text || "")
            lastPresentationValue = value === undefined ? null : value
            return true
        }

        function completeRequest(index, response) {
            const next = requests.slice(0)
            const request = next.splice(index, 1)[0]
            requests = next
            if (request.acceptResponse && !request.acceptResponse(response)) {
                return false
            }
            request.callback(response)
            return true
        }

        function cacheBlockchainResult(method, value, slotTo) {
            if (method === "blockchainNode") {
                testRoot.dashboardNode = value || null
            } else if (method === "blockchainLiveBlocks") {
                testRoot.dashboardL1Blocks = value && Array.isArray(value.blocks)
                    ? value.blocks : []
                const anchor = Number(slotTo || 0)
                testRoot.dashboardL1BlocksSlotTo =
                    Number.isSafeInteger(anchor) && anchor > 0 ? anchor : 0
            }
        }

        function clearBlockchainObservation() {
            testRoot.dashboardNode = null
            testRoot.dashboardOverview = null
        }

        function projectZoneDashboard() { projectDashboardCount += 1 }
        function resetDashboardProjection() {
            testRoot.dashboardOverview = null
            testRoot.dashboardNode = null
            testRoot.dashboardL1Blocks = []
            testRoot.dashboardL1BlocksSlotTo = 0
            testRoot.dashboardBlocks = []
        }
        function invalidateDashboardOperations(reason) { invalidatedDashboardCount += 1 }
        function setDashboardResult(ok, text, value) {
            dashboardResultCount += 1
            dashboardResultOk = ok === true
        }
        function refreshCapabilityRegistryIfLoaded() { capabilityRefreshCount += 1 }
        function dashboardGate(key) {
            return {
                enabled: true,
                status: "enabled",
                missing: [],
                warnings: [],
                provenance: ["test"]
            }
        }
        function scalarValue(value) {
            if (value === undefined || value === null || value === "") {
                return null
            }
            if (typeof value === "number" || typeof value === "string"
                    || typeof value === "boolean") {
                return value
            }
            if (value.value !== undefined) {
                return scalarValue(value.value)
            }
            if (value.result !== undefined) {
                return scalarValue(value.result)
            }
            return null
        }
    }

    MetricsState {
        id: metrics

        gateway: gateway
        sourceRouting: sourceRouting
        inspectorModule: "logos_inspector"
        nodeUrl: "http://node.invalid"
        storageRollingWindow: 60
        messagingRollingWindow: 60
        dashboardOverview: testRoot.dashboardOverview
        dashboardNode: testRoot.dashboardNode
        dashboardL1Blocks: testRoot.dashboardL1Blocks
        dashboardL1BlocksSlotTo: testRoot.dashboardL1BlocksSlotTo
        dashboardBlocks: testRoot.dashboardBlocks
        dashboardProvisionalBlocks: testRoot.dashboardProvisionalBlocks
    }

    function sourceReport(ready, marker) {
        return {
            marker: marker,
            health: {
                ready: ready === true,
                status: ready === true ? "healthy" : "degraded",
                summary: ready === true ? "source ready" : "source degraded",
                detail: ready === true ? "ready" : "not ready"
            },
            probes: []
        }
    }

    function blockchainProbe(ok, source, value, error) {
        return {
            ok: ok === true,
            source: source,
            value: ok === true ? value : null,
            error: ok === true ? "" : String(error || "probe failed")
        }
    }

    function blockchainReport(cryptarchiaOk, headersOk, networkOk, mantleOk) {
        return {
            endpoint: "http://bedrock.invalid",
            cryptarchia_info: blockchainProbe(cryptarchiaOk,
                "/cryptarchia/info", {
                    cryptarchia_info: { slot: 42, lib_slot: 40 }
                }, "cryptarchia down"),
            headers: blockchainProbe(headersOk, "/cryptarchia/headers", [],
                "headers down"),
            network_info: blockchainProbe(networkOk, "/network/info", {
                n_peers: 3
            }, "network down"),
            mantle_metrics: blockchainProbe(mantleOk, "/mantle/metrics", {
                transactions: 1
            }, "mantle down")
        }
    }

    function deliveryMetricsReport(ready, marker, peerCount) {
        const report = sourceReport(ready, marker)
        report.probes = [{
            probe_key: "collectOpenMetricsText",
            label: "delivery.collectOpenMetricsText",
            source: "delivery collectOpenMetricsText",
            ok: true,
            value: "libp2p_peers " + String(peerCount) + "\n"
        }]
        return report
    }

    function reportWithMetrics(kind, marker, value) {
        const report = sourceReport(true, marker)
        report.probes = [{
            probe_key: kind === "storage"
                ? "collectMetrics" : "collectOpenMetricsText",
            label: kind + ".metrics",
            source: kind + " metrics",
            ok: true,
            value: value
        }]
        return report
    }

    function success(value) {
        return { ok: true, value: value, text: "ok", error: "" }
    }

    function failure(message) {
        return { ok: false, value: null, text: "", error: String(message || "failed") }
    }

    function resetMetrics() {
        metrics.messagingRollingWindow = 60
        metrics.blockchainRefreshRate = 30
        metrics.messagingRefreshRate = 30
        metrics.storageRefreshRate = 30
        metrics.networkConnectionStatus = ({})
        metrics.networkConnectionStatusRevision = 0
        metrics.networkConnectionPending = ({})
        metrics.networkConnectionPendingRevision = 0
        metrics.dashboardMetricHistory = ({})
        metrics.dashboardMetricLastSeen = ({})
        metrics.dashboardMetricSeriesHistory = ({})
        metrics.dashboardMetricSeriesLastSeen = ({})
        metrics.dashboardMetricHistoryRevision = 0
        metrics.dashboardSnapshotRevision = 0
        metrics.deliveryModuleEventStreamStatus = "unknown"
        metrics.deliveryModuleEventStreamReason = ""
        metrics.deliveryModuleEventTimestamps =
            metrics.emptyDeliveryModuleEventTimestamps()
        metrics.deliveryModuleEventCoverageStartedAtMs =
            metrics.emptyDeliveryModuleEventCoverage(0)
        metrics.deliveryModuleEventNowMs = Date.now()
        metrics.deliveryModuleEventRevision = 0
        metrics.dashboardRefreshing = false
        metrics.dashboardRefreshSerial = 0
        metrics.dashboardError = ""
        metrics.blockchainSourceReport = null
        metrics.blockchainModuleReport = null
        metrics.storageModuleReport = null
        metrics.messagingModuleReport = null
        metrics.storageSourceReport = null
        metrics.messagingSourceReport = null
        metrics.messagingMetricsReport = null
        metrics.messagingMetricsCheckedAtMs = 0
        metrics.messagingMetricsRequestGeneration = 0
        metrics.messagingMetricsRevision = 0
        metrics.activeMessagingMetricsLease = null
        metrics.messagingMetricsAttempt = null
        metrics.observationConfigurationGenerations = ({
            blockchain: 0,
            storage: 0,
            messaging: 0
        })
        metrics.observationRequestSequences = ({
            blockchain: 0,
            storage: 0,
            messaging: 0
        })
        metrics.activeObservationLeases = ({})
        metrics.observationWaiters = ({})
        metrics.observationAttempts = ({})
        metrics.observationReportProvenance = ({})
        metrics.observationReportRequestIdentities = ({})
        metrics.observationReportRevisions = ({
            blockchain: 0,
            storage: 0,
            messaging: 0
        })
        metrics.moduleReportRevisions = ({
            blockchain: 0,
            storage: 0,
            messaging: 0
        })
        metrics.observationStatusRevisions = ({
            blockchain: 0,
            storage: 0,
            messaging: 0
        })
        metrics.observationRevision = 0
    }

    function completeDeliveryEventCoverage(now) {
        const timestamp = Number(now)
        metrics.deliveryModuleEventCoverageStartedAtMs =
            metrics.emptyDeliveryModuleEventCoverage(
                timestamp - metrics.dashboardMetricWindowMs(
                    "messaging.message_sent_events_recent") - 1)
        metrics.deliveryModuleEventNowMs = timestamp
        metrics.deliveryModuleEventRevision += 1
    }

    function init() {
        gateway.reset()
        sourceRouting.storageEndpoint = "http://storage.invalid"
        sourceRouting.storageCid = "z-test-cid"
        sourceRouting.deliverySourceMode = "logoscore_cli"
        sourceRouting.storageSourceMode = "logoscore_cli"
        sourceRouting.supportsLiveBlocks = true
        testRoot.dashboardOverview = null
        testRoot.dashboardNode = null
        testRoot.dashboardL1Blocks = []
        testRoot.dashboardL1BlocksSlotTo = 0
        testRoot.dashboardBlocks = []
        testRoot.dashboardProvisionalBlocks = []
        joinedCompletionCount = 0
        invalidObservationCallbackCount = 0
        invalidObservationCallbackError = ""
        resetMetrics()
    }

    function test_healthy_completion_commits_once() {
        metrics.queryNetworkConnection("storage", false)

        verify(metrics.networkConnectionIsPending("storage"))
        compare(gateway.requests.length, 1)
        const pendingRevision = metrics.networkConnectionPendingRevision
        const statusRevision = metrics.networkConnectionStatusRevision
        const reportRevision = metrics.observationReportRevisions.storage
        const snapshotRevision = metrics.dashboardSnapshotRevision

        verify(gateway.completeRequest(0, success(sourceReport(true, "healthy"))))

        verify(!metrics.networkConnectionIsPending("storage"))
        verify(metrics.networkConnectionState("storage").ok)
        compare(metrics.sourceReport("storage").marker, "healthy")
        compare(metrics.networkConnectionPendingRevision, pendingRevision + 1)
        compare(metrics.networkConnectionStatusRevision, statusRevision + 1)
        compare(metrics.observationReportRevisions.storage, reportRevision + 1)
        compare(metrics.dashboardSnapshotRevision, snapshotRevision + 1)
        compare(gateway.capabilityRefreshCount, 1)
    }

    function test_delivery_event_metrics_require_contiguous_watcher_coverage() {
        const sentKey = "messaging.message_sent_events_recent"
        const propagatedKey = "messaging.message_propagated_events_recent"

        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)
        verify(metrics.recordDeliveryModuleEvent("eventStreamReady", {
            object: { status: "ready" }
        }))
        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)
        verify(metrics.deliveryModuleEventMetricUnavailableReason(sentKey)
            .indexOf("continuous") >= 0)
        completeDeliveryEventCoverage(Date.now())
        compare(metrics.deliveryModuleEventMetricValue(sentKey), 0)
        compare(metrics.deliveryModuleEventMetricValue(propagatedKey), 0)

        verify(metrics.recordDeliveryModuleEvent("messageSent", {}))
        verify(metrics.recordDeliveryModuleEvent("messagePropagated", {}))
        compare(metrics.deliveryModuleEventMetricValue(sentKey), 1)
        compare(metrics.deliveryModuleEventMetricValue(propagatedKey), 1)
        compare(metrics.deliveryModuleEventMetricSamples(sentKey).slice(-1)[0].value, 1)

        verify(metrics.recordDeliveryModuleEvent("eventStreamUnavailable", {
            object: { reason: "watch exited" }
        }))
        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)
        compare(metrics.deliveryModuleEventMetricSamples(sentKey).length, 0)
        verify(!metrics.recordDeliveryModuleEvent("messageSent", {}))
        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)

        verify(metrics.recordDeliveryModuleEvent("eventStreamReady", {
            object: { status: "ready" }
        }))
        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)
        compare(metrics.deliveryModuleEventMetricValue(propagatedKey), null)
        const resumedAt = metrics.deliveryModuleEventNowMs
        completeDeliveryEventCoverage(resumedAt + 60001)
        compare(metrics.deliveryModuleEventMetricValue(sentKey), 0)
        compare(metrics.deliveryModuleEventMetricValue(propagatedKey), 0)
    }

    function test_delivery_event_history_survives_window_widening_and_prunes_only_retention_horizon() {
        const sentKey = "messaging.message_sent_events_recent"
        const now = 10000000
        metrics.deliveryModuleEventStreamStatus = "ready"
        metrics.deliveryModuleEventTimestamps = ({
            "messaging.message_sent_events_recent": [
                now - metrics.deliveryModuleEventRetentionMs - 1,
                now - 90000,
                now - 100
            ],
            "messaging.message_propagated_events_recent": []
        })
        metrics.deliveryModuleEventCoverageStartedAtMs =
            metrics.emptyDeliveryModuleEventCoverage(now - 300000)
        metrics.deliveryModuleEventNowMs = now
        metrics.deliveryModuleEventRevision += 1

        compare(metrics.deliveryModuleEventRowsInWindow(sentKey, now).length, 1)
        verify(metrics.pruneDeliveryModuleEventTelemetry(now))
        compare(metrics.deliveryModuleEventTimestamps[sentKey].length, 2)
        metrics.messagingRollingWindow = 120
        compare(metrics.deliveryModuleEventMetricValue(sentKey), 2)
    }

    function test_delivery_event_capacity_reset_recovers_without_watcher_reconnect() {
        const sentKey = "messaging.message_sent_events_recent"
        const now = Date.now()
        const rows = []
        for (let i = 0; i < metrics.deliveryModuleEventCapacity; ++i) {
            rows.push(now - 100)
        }
        metrics.deliveryModuleEventStreamStatus = "ready"
        metrics.deliveryModuleEventTimestamps = ({
            "messaging.message_sent_events_recent": rows,
            "messaging.message_propagated_events_recent": []
        })
        metrics.deliveryModuleEventCoverageStartedAtMs =
            metrics.emptyDeliveryModuleEventCoverage(now - 120000)
        metrics.deliveryModuleEventNowMs = now

        verify(metrics.recordDeliveryModuleEvent("messageSent", {}))
        compare(metrics.deliveryModuleEventStreamStatus, "ready")
        compare(metrics.deliveryModuleEventTimestamps[sentKey].length, 1)
        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)

        const restartedAt = metrics.deliveryModuleEventCoverageStartedAtMs[sentKey]
        const recoveredCoverage = metrics.emptyDeliveryModuleEventCoverage(
            restartedAt - 60001)
        const recoveredRows = metrics.emptyDeliveryModuleEventTimestamps()
        recoveredRows[sentKey] = [restartedAt + 30000]
        metrics.deliveryModuleEventCoverageStartedAtMs = recoveredCoverage
        metrics.deliveryModuleEventTimestamps = recoveredRows
        metrics.deliveryModuleEventNowMs = restartedAt + 60001
        metrics.deliveryModuleEventRevision += 1
        compare(metrics.deliveryModuleEventMetricValue(sentKey), 1)
    }

    function test_delivery_event_counts_are_source_scoped_and_reset_on_source_change() {
        const sentKey = "messaging.message_sent_events_recent"
        const now = 10000000
        metrics.deliveryModuleEventStreamStatus = "ready"
        metrics.deliveryModuleEventTimestamps = ({
            "messaging.message_sent_events_recent": [now - 1000],
            "messaging.message_propagated_events_recent": []
        })
        metrics.deliveryModuleEventCoverageStartedAtMs =
            metrics.emptyDeliveryModuleEventCoverage(now - 120000)
        metrics.deliveryModuleEventNowMs = now
        metrics.deliveryModuleEventRevision += 1
        compare(metrics.deliveryModuleEventMetricValue(sentKey), 1)

        sourceRouting.deliverySourceMode = "rest"
        verify(metrics.invalidateConfiguration("messaging", "source changed"))
        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)
        compare(metrics.deliveryModuleEventTimestamps[sentKey].length, 0)

        sourceRouting.deliverySourceMode = "logoscore_cli"
        verify(metrics.invalidateConfiguration("messaging", "source changed"))
        compare(metrics.deliveryModuleEventMetricValue(sentKey), null)
    }

    function test_delivery_event_graph_keeps_historical_window_counts() {
        const sentKey = "messaging.message_sent_events_recent"
        const now = 1000000
        metrics.messagingRollingWindow = 100
        metrics.deliveryModuleEventStreamStatus = "ready"
        metrics.deliveryModuleEventTimestamps = ({
            "messaging.message_sent_events_recent": [now - 110000, now - 60000],
            "messaging.message_propagated_events_recent": []
        })
        metrics.deliveryModuleEventCoverageStartedAtMs =
            metrics.emptyDeliveryModuleEventCoverage(now - 300000)
        metrics.deliveryModuleEventNowMs = now
        metrics.deliveryModuleEventRevision += 1

        compare(metrics.deliveryModuleEventMetricValue(sentKey), 1)
        const samples = metrics.deliveryModuleEventMetricSamples(sentKey)
        const historical = samples.filter(function (sample) {
            return sample.timestamp === now - 60000
        })
        compare(historical.length, 1)
        compare(historical[0].value, 2)
    }

    function test_invalid_storage_cid_is_rejected_before_bridge_dispatch() {
        sourceRouting.storageCid = "cid/child"

        let response = metrics.observeNetworkConnection(
            "storage", true, true, function (callbackResponse, snapshot) {
                invalidObservationCallbackCount += 1
                invalidObservationCallbackError = String(
                    callbackResponse && callbackResponse.error || "")
                verify(snapshot !== null)
            }, "source-inspection")

        verify(!response.ok)
        compare(
            response.error,
            "Storage CID must contain only ASCII letters, digits, `-`, or `_`.")
        compare(gateway.requests.length, 0)
        verify(!metrics.networkConnectionIsPending("storage"))
        compare(gateway.presentationBeginCount, 1)
        compare(gateway.presentationCompleteCount, 1)
        verify(gateway.lastPresentationError)
        compare(gateway.lastPresentationText, response.error)
        compare(invalidObservationCallbackCount, 1)
        compare(invalidObservationCallbackError, response.error)

        gateway.reset()
        sourceRouting.storageCid = "valid-CID_123"
        response = metrics.queryNetworkConnection(
            "storage", false, true, "source-inspection")

        verify(response)
        compare(gateway.requests.length, 1)
        compare(
            gateway.requests[0].args[0].options.cid,
            "valid-CID_123")
    }

    function test_completed_degraded_report_remains_readable() {
        metrics.queryNetworkConnection("messaging", false)
        gateway.completeRequest(0, success(sourceReport(false, "degraded")))

        const observation = metrics.sourceObservation("messaging")
        compare(observation.sourceReport.marker, "degraded")
        verify(observation.status.known)
        verify(!observation.status.ok)
        verify(observation.latestAttempt.transportOk)
        verify(!observation.stale)
    }

    function test_hard_failure_retains_explicit_last_known_report() {
        metrics.queryNetworkConnection("storage", false)
        gateway.completeRequest(0, success(sourceReport(true, "last-known")))
        const reportRevision = metrics.observationReportRevisions.storage

        metrics.queryNetworkConnection("storage", false)
        gateway.completeRequest(0, failure("transport down"))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "last-known")
        compare(observation.reportRevision, reportRevision)
        verify(!observation.status.ok)
        verify(observation.stale)
        compare(observation.status.detail, "transport down")
    }

    function test_first_hard_failure_has_no_stale_report() {
        metrics.queryNetworkConnection("storage", false)
        gateway.completeRequest(0, failure("source unavailable"))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport, null)
        verify(observation.status.known)
        verify(!observation.status.ok)
        verify(!observation.stale)
        verify(!observation.latestAttempt.transportOk)
        compare(observation.status.detail, "source unavailable")
    }

    function test_unrelated_configuration_change_does_not_strand_pending_request() {
        metrics.queryNetworkConnection("storage", false)
        const generation = metrics.familyConfigurationGeneration("storage")

        metrics.invalidateConfiguration("messaging", "messaging changed")

        verify(metrics.networkConnectionIsPending("storage"))
        compare(metrics.familyConfigurationGeneration("storage"), generation)
        verify(gateway.completeRequest(0, success(sourceReport(true, "accepted"))))
        verify(!metrics.networkConnectionIsPending("storage"))
        compare(metrics.sourceReport("storage").marker, "accepted")
    }

    function test_same_family_invalidation_rejects_stale_completion() {
        metrics.queryNetworkConnection("storage", false)
        const statusRevision = metrics.networkConnectionStatusRevision

        metrics.invalidateConfiguration("storage", "storage changed")

        verify(!metrics.networkConnectionIsPending("storage"))
        verify(!gateway.completeRequest(0, success(sourceReport(true, "stale"))))
        compare(metrics.networkConnectionStatusRevision, statusRevision)
        compare(metrics.sourceReport("storage"), null)
        verify(!metrics.networkConnectionState("storage").known)
    }

    function test_blockchain_invalidation_clears_source_scoped_metric_history() {
        metrics.dashboardMetricHistory = {
            "bedrock.peer_count": [{ timestamp: 1, value: 4 }],
            "lez.blocks_produced_recent": [{ timestamp: 1, value: 3 }],
            "indexer.indexer_lag_vs_sequencer_head": [
                { timestamp: 1, value: 2 }
            ],
            "storage.peer_count": [{ timestamp: 1, value: 1 }]
        }
        metrics.dashboardMetricLastSeen = {
            "bedrock.peer_count": { timestamp: 2, value: 4 },
            "lez.blocks_produced_recent": { timestamp: 2, value: 3 },
            "indexer.indexer_lag_vs_sequencer_head": {
                timestamp: 2,
                value: 2
            },
            "storage.peer_count": { timestamp: 2, value: 1 }
        }

        metrics.invalidateConfiguration("blockchain", "blockchain changed")

        compare(metrics.dashboardMetricHistory["bedrock.peer_count"], undefined)
        compare(metrics.dashboardMetricHistory["lez.blocks_produced_recent"], undefined)
        compare(metrics.dashboardMetricHistory[
            "indexer.indexer_lag_vs_sequencer_head"], undefined)
        compare(metrics.dashboardMetricLastSeen["bedrock.peer_count"], undefined)
        compare(metrics.dashboardMetricLastSeen[
            "lez.blocks_produced_recent"], undefined)
        compare(metrics.dashboardMetricLastSeen[
            "indexer.indexer_lag_vs_sequencer_head"], undefined)
        verify(metrics.dashboardMetricHistory["storage.peer_count"] !== undefined)
        verify(metrics.dashboardMetricLastSeen["storage.peer_count"] !== undefined)
        compare(metrics.dashboardMetricHistoryRevision, 1)
    }

    function test_second_observer_joins_active_family_lease() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "dashboard")
        const joined = metrics.observeNetworkConnection(
            "messaging",
            false,
            false,
            function () { testRoot.joinedCompletionCount += 1 },
            "dashboard"
        )

        verify(joined.joined)
        compare(gateway.requests.length, 1)
        gateway.completeRequest(0, success(sourceReport(true, "joined")))
        compare(joinedCompletionCount, 1)
        compare(metrics.sourceReport("messaging").marker, "joined")
    }

    function test_interactive_observer_supersedes_passive_lease_and_presents() {
        metrics.queryNetworkConnection("messaging", false, false, "dashboard")
        metrics.queryNetworkConnection(
            "messaging", true, false, "source-inspection")

        compare(gateway.requests.length, 2)
        compare(gateway.presentationBeginCount, 1)
        compare(gateway.lastPresentationOwner, "messaging")
        verify(metrics.activeObservationLeases.messaging.interactive)
        verify(!gateway.completeRequest(
            0, success(sourceReport(true, "passive-stale"))))

        gateway.completeRequest(0, success(sourceReport(true, "presented")))

        compare(gateway.presentationCompleteCount, 1)
        verify(!gateway.lastPresentationError)
        verify(gateway.lastPresentationText.length > 0)
        compare(metrics.sourceReport("messaging").marker, "presented")
    }

    function test_interactive_full_request_blocks_incompatible_background_supersede() {
        metrics.queryNetworkConnection("storage", false, false, "scheduler")
        metrics.queryNetworkConnection(
            "storage", true, false, "source-inspection")
        verify(metrics.activeObservationLeases.storage.interactive)

        sourceRouting.storageEndpoint = "http://new-storage.invalid"
        const skipped = metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")

        verify(skipped.skipped)
        compare(gateway.requests.length, 1)
        verify(metrics.networkConnectionIsPending("storage"))
        gateway.completeRequest(0, success(sourceReport(true, "interactive")))
        compare(gateway.presentationCompleteCount, 1)
        compare(metrics.sourceReport("storage").marker, "interactive")
    }

    function test_sensitive_storage_lease_satisfies_weaker_background_observer() {
        metrics.queryNetworkConnection("storage", false, true, "entity-open")
        const joined = metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")

        verify(joined.joined)
        compare(gateway.requests.length, 1)
        gateway.completeRequest(0, success(sourceReport(true, "sensitive")))
        compare(metrics.sourceReport("storage").marker, "sensitive")
    }

    function test_full_noninteractive_lease_satisfies_passive_observer() {
        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        const joined = metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")

        verify(joined.joined)
        compare(gateway.requests.length, 1)
        gateway.completeRequest(
            0, success(sourceReport(true, "full-noninteractive")))
        compare(metrics.sourceReport("storage").marker, "full-noninteractive")
        compare(
            metrics.sourceObservation("storage").provenance.origin,
            "source-inspection"
        )
    }

    function test_passive_sensitive_lease_cannot_satisfy_full_observer() {
        metrics.queryNetworkConnection(
            "storage", false, true, "scheduler")
        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")

        compare(gateway.requests.length, 2)
        verify(!gateway.completeRequest(
            0, success(sourceReport(true, "passive-sensitive"))))
        verify(gateway.completeRequest(
            0, success(sourceReport(true, "full-current"))))
        compare(metrics.sourceReport("storage").marker, "full-current")
        compare(
            metrics.sourceObservation("storage").provenance.origin,
            "source-inspection"
        )
    }

    function test_passive_sensitive_observer_cannot_cancel_active_full_lease() {
        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        const skipped = metrics.queryNetworkConnection(
            "storage", false, true, "scheduler")

        verify(skipped.skipped)
        compare(gateway.requests.length, 1)
        verify(gateway.completeRequest(
            0, success(sourceReport(true, "full-current"))))
        compare(metrics.sourceReport("storage").marker, "full-current")
    }

    function test_sensitive_upgrade_transfers_waiters_and_presentation() {
        metrics.observeNetworkConnection(
            "storage",
            false,
            false,
            function () { testRoot.joinedCompletionCount += 1 },
            "scheduler"
        )
        metrics.queryNetworkConnection("storage", true, true, "source-inspection")

        compare(gateway.requests.length, 2)
        compare(gateway.presentationBeginCount, 1)
        verify(!gateway.completeRequest(0, success(sourceReport(true, "weaker"))))
        compare(gateway.presentationCompleteCount, 0)
        verify(gateway.completeRequest(0, success(sourceReport(true, "stronger"))))

        compare(joinedCompletionCount, 1)
        compare(gateway.presentationCompleteCount, 1)
        verify(!gateway.lastPresentationError)
        compare(metrics.sourceReport("storage").marker, "stronger")
        compare(metrics.sourceObservation("storage").provenance.origin, "source-inspection")
    }

    function test_storage_interactive_presentation_uses_path_free_summary() {
        const report = sourceReport(true, "storage-summary")
        report.module = "storage_module"
        report.probes = [{
            probe_key: "dataDir",
            label: "Data directory",
            ok: true,
            value: "/var/lib/logos/private-storage"
        }, {
            probe_key: "version",
            label: "Version",
            ok: true,
            value: "0.1.0-test"
        }]

        metrics.queryNetworkConnection("storage", true, false, "manual")
        verify(gateway.completeRequest(0, success(report)))

        compare(gateway.presentationCompleteCount, 1)
        verify(gateway.lastPresentationText.indexOf(
            "/var/lib/logos/private-storage") < 0)
        compare(gateway.lastPresentationValue.module, "storage_module")
        compare(gateway.lastPresentationValue.status, "healthy")
        compare(gateway.lastPresentationValue.probes, 2)
        compare(gateway.lastPresentationValue.successful_probes, 2)
        compare(gateway.lastPresentationValue.failed_probes, 0)
        verify(gateway.lastPresentationValue.probe_facts === undefined)
    }

    function test_configuration_invalidation_completes_interactive_presentation() {
        metrics.queryNetworkConnection("storage", true, false, "source-inspection")
        compare(gateway.presentationBeginCount, 1)

        metrics.invalidateConfiguration("storage", "source changed")

        compare(gateway.presentationCompleteCount, 1)
        verify(gateway.lastPresentationError)
        verify(gateway.lastPresentationText.indexOf("source changed") >= 0)
        verify(!metrics.networkConnectionIsPending("storage"))
        verify(!gateway.completeRequest(0, success(sourceReport(true, "stale"))))
    }

    function test_sensitive_upgrade_replaces_interactive_presentation_cleanly() {
        metrics.queryNetworkConnection("storage", true, false, "manual")
        metrics.queryNetworkConnection("storage", true, true, "source-inspection")

        compare(gateway.presentationBeginCount, 2)
        verify(!gateway.completeRequest(0, success(sourceReport(true, "weaker"))))
        compare(gateway.presentationCompleteCount, 0)
        verify(gateway.completeRequest(0, success(sourceReport(true, "stronger"))))

        compare(gateway.presentationCompleteCount, 1)
        compare(gateway.activePresentationGeneration, 0)
        compare(metrics.sourceReport("storage").marker, "stronger")
    }

    function test_observation_origin_is_preserved() {
        metrics.queryNetworkConnection("messaging", false, false, "dashboard")
        gateway.completeRequest(0, success(sourceReport(true, "scheduled")))

        const observation = metrics.sourceObservation("messaging")
        compare(observation.provenance.origin, "dashboard")
        compare(observation.status.origin, "dashboard")
    }

    function test_passive_observations_keep_storage_capability_facts_and_bound_scheduled_delivery_to_metrics() {
        const passiveOrigins = [
            "scheduler",
            "dashboard",
            "module-event",
            "storage-refresh",
            "storage-mutation"
        ]
        for (let i = 0; i < passiveOrigins.length; ++i) {
            const origin = passiveOrigins[i]
            const storage = metrics.networkConnectionRequest(
                "storage", false, origin)
            const messaging = metrics.networkConnectionRequest(
                "messaging", false, origin)
            compare(
                storage.args[0].options.runtime_diagnostics_enabled,
                true
            )
            verify(!storage.runtimeDiagnosticsReduced)
            compare(
                messaging.args[0].options.runtime_diagnostics_enabled,
                false
            )
            compare(
                messaging.args[0].options.runtime_metrics_enabled === true,
                origin === "scheduler"
            )
            compare(messaging.runtimeMetricsOnly, origin === "scheduler")
        }

        const fullOrigins = ["manual", "source-inspection", "entity-open"]
        for (let i = 0; i < fullOrigins.length; ++i) {
            const origin = fullOrigins[i]
            const storage = metrics.networkConnectionRequest(
                "storage", origin === "entity-open", origin)
            const messaging = metrics.networkConnectionRequest(
                "messaging", false, origin)
            compare(
                storage.args[0].options.runtime_diagnostics_enabled,
                true
            )
            compare(
                messaging.args[0].options.runtime_diagnostics_enabled,
                true
            )
            verify(messaging.args[0].options.runtime_metrics_enabled !== true)
            verify(!messaging.runtimeMetricsOnly)
        }
    }

    function test_scheduled_delivery_metrics_use_independent_lease_and_cache() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")

        compare(gateway.requests.length, 1)
        verify(gateway.requests[0].args[0].options
            .runtime_metrics_enabled)
        verify(!gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(metrics.activeMessagingMetricsLease !== null)
        compare(metrics.activeObservationLeases.messaging, undefined)
        verify(!metrics.networkConnectionIsPending("messaging"))

        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "metrics-only", 7)))

        const observation = metrics.sourceObservation("messaging")
        compare(observation.sourceReport, null)
        compare(observation.metricsReport.marker, "metrics-only")
        verify(observation.metricsAttempt.ok)
        compare(observation.metricsAttempt.origin, "scheduler")
        verify(observation.metricsCheckedAtMs > 0)
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 7)
        compare(metrics.dashboardMetricLastSeen[
            "messaging.peer_count"].value, 7)
        verify(!observation.status.known)
        verify(!observation.status.ok)
        verify(observation.status.transportOk)
        compare(observation.status.origin, "scheduler")
        verify(observation.latestAttempt.runtimeMetricsOnly)
    }

    function test_storage_failure_window_ignores_retried_peer_timeouts() {
        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        gateway.completeRequest(0, success(reportWithMetrics(
            "storage", "storage-before", [
                {
                    name: "storage_block_exchange_requests_failed_total",
                    value: 100
                },
                {
                    name: "storage_block_exchange_peer_timeouts_total",
                    value: 50
                }
            ])))

        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        gateway.completeRequest(0, success(reportWithMetrics(
            "storage", "storage-after", [
                {
                    name: "storage_block_exchange_requests_failed_total",
                    value: 3
                },
                {
                    name: "storage_block_exchange_peer_timeouts_total",
                    value: 55
                }
            ])))

        compare(metrics.dashboardMetricRawValue(
            "storage.failed_transfers_total"), 3)
        compare(metrics.dashboardMetricValue(
            "storage.failed_transfers_recent"), 3)
        const graph = metrics.dashboardMetricSamples(
            "storage.failed_transfers_recent")
        compare(graph.length, 1)
        compare(graph[0].value, 3)
    }

    function test_delivery_aggregate_window_tracks_text_constituents_across_partial_reset() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-before", [
                "waku_node_errors_total 100",
                "waku_store_errors_total 50"
            ].join("\n"))))

        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-after", [
                "waku_node_errors_total 3",
                "waku_store_errors_total 55"
            ].join("\n"))))

        compare(metrics.dashboardMetricRawValue(
            "messaging.message_error_events_recent"), 58)
        compare(metrics.dashboardMetricValue(
            "messaging.message_error_events_recent"), 8)
        const graph = metrics.dashboardMetricSamples(
            "messaging.message_error_events_recent")
        compare(graph.length, 1)
        compare(graph[0].value, 8)
    }

    function test_delivery_openmetrics_cannot_impersonate_native_message_events() {
        const before = [
            "waku_service_requests_total{service=\"/vac/waku/lightpush/3.0.0\",state=\"served\"} 100",
            "waku_node_messages_total{type=\"relay\"} 500"
        ].join("\n")
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-events-before", before)))

        compare(metrics.dashboardMetricRawValue(
            "messaging.message_sent_events_recent"), null)
        compare(metrics.dashboardMetricRawValue(
            "messaging.message_propagated_events_recent"), null)
        compare(metrics.dashboardMetricValue(
            "messaging.message_sent_events_recent"), null)
        compare(metrics.dashboardMetricValue(
            "messaging.message_propagated_events_recent"), null)

        const after = [
            "waku_service_requests_total{service=\"/vac/waku/lightpush/3.0.0\",state=\"served\"} 104",
            "waku_node_messages_total{type=\"relay\"} 507"
        ].join("\n")
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-events-after", after)))

        compare(metrics.dashboardMetricValue(
            "messaging.message_sent_events_recent"), null)
        compare(metrics.dashboardMetricValue(
            "messaging.message_propagated_events_recent"), null)
        const sentGraph = metrics.dashboardMetricSamples(
            "messaging.message_sent_events_recent")
        const propagatedGraph = metrics.dashboardMetricSamples(
            "messaging.message_propagated_events_recent")
        compare(sentGraph.length, 0)
        compare(propagatedGraph.length, 0)
    }

    function test_delivery_json_aggregate_taxonomy_avoids_alias_double_counts() {
        const serviceRows = [
            {
                name: "waku_store_queries_total",
                value: 10
            },
            {
                name: "waku_service_requests_total",
                labels: { service: "/vac/waku/store-query/3.0.0" },
                value: 10
            },
            {
                name: "waku_filter_requests_total",
                value: 20
            },
            {
                name: "waku_service_requests_total",
                labels: {
                    service: "/vac/waku/filter-subscribe/2.0.0-beta1"
                },
                value: 20
            },
            {
                name: "waku_lightpush_messages_total",
                value: 30
            },
            {
                name: "waku_lightpush_v3_messages_total",
                value: 40
            },
            {
                name: "waku_service_requests_total",
                labels: { service: "/vac/waku/lightpush/2.0.0-beta1" },
                value: 30
            },
            {
                name: "waku_service_requests_total",
                labels: { service: "/vac/waku/lightpush/3.0.0" },
                value: 40
            },
            {
                name: "waku_px_peers_sent_total",
                value: 50
            },
            {
                name: "waku_service_requests_total",
                labels: {
                    service: "/vac/waku/peer-exchange/2.0.0-alpha1"
                },
                value: 5
            }
        ]
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-taxonomy", serviceRows)))

        compare(metrics.dashboardMetricRawValue(
            "messaging.store_query_requests_recent"), 10)
        compare(metrics.dashboardMetricRawValue(
            "messaging.filter_requests_recent"), 20)
        compare(metrics.dashboardMetricRawValue(
            "messaging.lightpush_requests_recent"), 70)
        compare(metrics.dashboardMetricRawValue(
            "messaging.message_sent_events_recent"), null)
        compare(metrics.dashboardMetricRawValue(
            "messaging.peer_exchange_requests_recent"), 5)
    }

    function test_delivery_json_labeled_series_preserve_topic_identity() {
        const rows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: 100
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "beta" },
                value: 200
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "gross", direction: "in", topic: "alpha" },
                value: 900
            }
        ]
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-labeled-json", rows)))

        compare(metrics.dashboardMetricRawValue(
            "messaging.relay_ingress_recent"), 300)
        const series = metrics.moduleMetricSeries("messaging", {
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in" }
        })
        compare(series.length, 2)
        compare(series[0].labels.topic, "alpha")
        compare(series[1].labels.topic, "beta")
    }

    function test_delivery_text_labeled_series_sum_all_matching_states() {
        const value = [
            "waku_service_requests_total{service=\"/vac/waku/store-query/3.0.0\",state=\"served\"} 10",
            "waku_service_requests_total{service=\"/vac/waku/store-query/3.0.0\",state=\"rejected\"} 2"
        ].join("\n")
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-labeled-text", value)))

        compare(metrics.dashboardMetricRawValue(
            "messaging.store_query_requests_recent"), 12)
        const series = metrics.moduleMetricSeries("messaging", {
            name: "waku_service_requests_total",
            labels: { service: "/vac/waku/store-query/3.0.0" }
        })
        compare(series.length, 2)
    }

    function test_duplicate_labeled_series_fail_closed() {
        const value = [
            "waku_relay_network_bytes_total{type=\"net\",direction=\"in\",topic=\"alpha\"} 10",
            "waku_relay_network_bytes_total{type=\"net\",direction=\"in\",topic=\"alpha\"} 20"
        ].join("\n")
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-duplicate-series", value)))

        const series = metrics.moduleMetricSeries("messaging", {
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in" }
        })
        compare(series, null)
        compare(metrics.dashboardMetricRawValue(
            "messaging.relay_ingress_recent"), null)
    }

    function test_invalid_canonical_json_series_cannot_fall_through_to_alias() {
        const rows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: null
            },
            {
                name: "waku_relay_network_bytes_in_total",
                value: 99
            }
        ]
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-invalid-series", rows)))

        compare(metrics.moduleMetricSeries("messaging", {
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in" }
        }), null)
        compare(metrics.dashboardMetricRawValue(
            "messaging.relay_ingress_recent"), null)
    }

    function test_duplicate_canonical_series_cannot_fall_through_to_alias() {
        const value = [
            "waku_relay_network_bytes_total{type=\"net\",direction=\"in\",topic=\"alpha\"} 10",
            "waku_relay_network_bytes_total{type=\"net\",direction=\"in\",topic=\"alpha\"} 20",
            "waku_relay_network_bytes_in_total 99"
        ].join("\n")
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-duplicate-with-alias", value)))

        compare(metrics.dashboardMetricRawValue(
            "messaging.relay_ingress_recent"), null)
    }

    function test_malformed_canonical_text_cannot_fall_through_to_alias() {
        const invalidValues = ["not-a-number", "10garbage"]
        for (let i = 0; i < invalidValues.length; ++i) {
            const value = [
                "waku_relay_network_bytes_total{type=\"net\",direction=\"in\",topic=\"alpha\"} "
                    + invalidValues[i],
                "waku_relay_network_bytes_in_total 99"
            ].join("\n")
            metrics.queryNetworkConnection(
                "messaging", false, false, "scheduler")
            gateway.completeRequest(0, success(reportWithMetrics(
                "messaging", "delivery-malformed-series-" + String(i), value)))

            compare(metrics.dashboardMetricRawValue(
                "messaging.relay_ingress_recent"), null)
        }
    }

    function test_positive_signed_text_series_are_accepted() {
        const value = [
            "waku_relay_network_bytes_total{type=\"net\",direction=\"in\",topic=\"alpha\"} +1",
            "waku_relay_network_bytes_total{type=\"net\",direction=\"in\",topic=\"beta\"} +2e1",
            "libp2p_peers +3"
        ].join("\n")
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(reportWithMetrics(
            "messaging", "delivery-positive-series", value)))

        compare(metrics.dashboardMetricRawValue(
            "messaging.relay_ingress_recent"), 21)
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 3)
    }

    function test_delivery_aggregate_graph_matches_headline_across_reset_sequence() {
        const observations = [
            [100, 50],
            [110, 55],
            [3, 60],
            [8, 65]
        ]
        for (let i = 0; i < observations.length; ++i) {
            metrics.queryNetworkConnection(
                "messaging", false, false, "scheduler")
            gateway.completeRequest(0, success(reportWithMetrics(
                "messaging", "delivery-" + String(i), [
                    "waku_node_errors_total " + String(observations[i][0]),
                    "waku_store_errors_total " + String(observations[i][1])
                ].join("\n"))))
        }

        const graph = metrics.dashboardMetricSamples(
            "messaging.message_error_events_recent")
        compare(JSON.stringify(graph.map(function (sample) {
            return sample.value
        })), JSON.stringify([15, 23, 33]))
        compare(metrics.dashboardMetricValue(
            "messaging.message_error_events_recent"), 33)
        compare(graph[graph.length - 1].value, 33)
    }

    function test_source_invalidation_clears_aggregate_series_baseline() {
        function acceptStorage(first, second, marker) {
            metrics.queryNetworkConnection(
                "storage", false, false, "source-inspection")
            gateway.completeRequest(0, success(reportWithMetrics(
                "storage", marker, [
                    {
                        name: "storage_block_exchange_requests_failed_total",
                        value: first
                    },
                    {
                        name: "storage_block_exchange_peer_timeouts_total",
                        value: second
                    }
                ])))
        }

        acceptStorage(100, 50, "before-1")
        acceptStorage(103, 55, "before-2")
        compare(metrics.dashboardMetricValue(
            "storage.failed_transfers_recent"), 3)

        metrics.invalidateConfiguration("storage", "source changed")

        compare(metrics.dashboardMetricSeriesHistory[
            "storage.failed_transfers_recent"], undefined)
        compare(metrics.dashboardMetricSeriesLastSeen[
            "storage.failed_transfers_recent"], undefined)
        acceptStorage(200, 100, "after-1")
        compare(metrics.dashboardMetricValue(
            "storage.failed_transfers_recent"), null)
        compare(metrics.dashboardMetricSamples(
            "storage.failed_transfers_recent").length, 0)

        acceptStorage(202, 103, "after-2")
        compare(metrics.dashboardMetricValue(
            "storage.failed_transfers_recent"), 2)
    }

    function test_scheduled_metrics_preserve_full_delivery_observation() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "source-inspection")
        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "explicit-full", 4)))
        const reportRevision = metrics.observationReportRevisions.messaging
        const statusRevision = metrics.networkConnectionStatusRevision
        const metricsRevision = metrics.messagingMetricsRevision

        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "metrics-only", 9)))

        const observation = metrics.sourceObservation("messaging")
        compare(observation.sourceReport.marker, "explicit-full")
        compare(observation.provenance.origin, "source-inspection")
        compare(observation.reportRevision, reportRevision)
        compare(metrics.networkConnectionStatusRevision, statusRevision + 1)
        compare(metrics.messagingMetricsRevision, metricsRevision + 1)
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 9)
    }

    function test_scheduled_metrics_never_upgrade_degraded_delivery_health() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "source-inspection")
        gateway.completeRequest(
            0, success(deliveryMetricsReport(false, "degraded-full", 2)))
        verify(!metrics.networkConnectionState("messaging").ok)

        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "metrics-only", 15)))

        const status = metrics.networkConnectionState("messaging")
        verify(status.known)
        verify(!status.ok)
        verify(status.transportOk)
        compare(metrics.sourceReport("messaging").marker, "degraded-full")
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 15)
    }

    function test_newer_full_delivery_metrics_reject_older_scheduler_sample() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        const schedulerSequence = metrics.activeMessagingMetricsLease.sequence

        metrics.queryNetworkConnection(
            "messaging", false, false, "source-inspection")
        verify(metrics.activeMessagingMetricsLease !== null)
        verify(metrics.activeObservationLeases.messaging !== undefined)
        verify(metrics.activeObservationLeases.messaging.sequence
            > schedulerSequence)
        gateway.completeRequest(
            1, success(deliveryMetricsReport(true, "explicit-full", 10)))
        const metricsGeneration = metrics.messagingMetricsRequestGeneration

        const statusGeneration = metrics.networkConnectionState(
            "messaging").requestGeneration
        gateway.completeRequest(0, failure("older scheduler failure"))

        compare(metrics.sourceReport("messaging").marker, "explicit-full")
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 10)
        compare(metrics.messagingMetricsRequestGeneration, metricsGeneration)
        verify(metrics.messagingMetricsAttempt.ok)
        compare(metrics.messagingMetricsAttempt.origin, "source-inspection")
        verify(metrics.networkConnectionState("messaging").known)
        verify(metrics.networkConnectionState("messaging").ok)
        verify(metrics.networkConnectionState("messaging").transportOk)
        compare(metrics.networkConnectionState(
            "messaging").requestGeneration, statusGeneration)
        compare(metrics.sourceObservation(
            "messaging").latestAttempt.origin, "source-inspection")
    }

    function test_full_delivery_observation_satisfies_scheduler_tick() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "source-inspection")
        const result = metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")

        verify(result.skipped)
        verify(result.joined)
        compare(gateway.requests.length, 1)
        compare(metrics.activeMessagingMetricsLease, null)

        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "explicit-full", 5)))
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 5)
    }

    function test_messaging_invalidation_cancels_metrics_and_rejects_late_reply() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "metrics-first", 6)))
        verify(metrics.messagingMetricsReport !== null)
        verify(metrics.dashboardMetricHistory[
            "messaging.peer_count"] !== undefined)

        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        verify(metrics.activeMessagingMetricsLease !== null)
        metrics.invalidateConfiguration("messaging", "source changed")

        compare(metrics.activeMessagingMetricsLease, null)
        compare(metrics.messagingMetricsReport, null)
        compare(metrics.messagingMetricsCheckedAtMs, 0)
        compare(metrics.dashboardMetricHistory[
            "messaging.peer_count"], undefined)
        verify(!gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "late", 99))))
        compare(metrics.messagingMetricsReport, null)
    }

    function test_blockchain_completion_does_not_refresh_delivery_sample_time() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "metrics", 8)))
        const lastSeen = metrics.dashboardMetricLastSeen[
            "messaging.peer_count"]

        metrics.queryNetworkConnection(
            "blockchain", false, false, "dashboard")
        gateway.completeRequest(
            0, success(sourceReport(true, "blockchain")))

        compare(metrics.dashboardMetricLastSeen[
            "messaging.peer_count"].timestamp, lastSeen.timestamp)
        compare(metrics.dashboardMetricLastSeen[
            "messaging.peer_count"].value, lastSeen.value)
    }

    function test_older_dashboard_reply_cannot_rollback_delivery_metrics_status() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "dashboard")
        const dashboardSequence = metrics.activeObservationLeases
            .messaging.sequence
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        verify(metrics.activeMessagingMetricsLease.sequence
            > dashboardSequence)

        gateway.completeRequest(
            1, success(deliveryMetricsReport(true, "metrics", 12)))
        const statusGeneration = metrics.networkConnectionState(
            "messaging").requestGeneration
        gateway.completeRequest(
            0, success(sourceReport(false, "older-dashboard")))

        compare(metrics.sourceReport("messaging"), null)
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 12)
        verify(!metrics.networkConnectionState("messaging").known)
        verify(!metrics.networkConnectionState("messaging").ok)
        verify(metrics.networkConnectionState("messaging").transportOk)
        compare(metrics.networkConnectionState(
            "messaging").requestGeneration, statusGeneration)
        verify(metrics.sourceObservation(
            "messaging").latestAttempt.runtimeMetricsOnly)
    }

    function test_delivery_reports_without_new_metrics_do_not_refresh_sample_time() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "metrics", 13)))
        const lastSeen = metrics.dashboardMetricLastSeen[
            "messaging.peer_count"]

        metrics.queryNetworkConnection(
            "messaging", false, false, "dashboard")
        gateway.completeRequest(
            0, success(sourceReport(true, "no-metrics")))
        compare(metrics.dashboardMetricLastSeen[
            "messaging.peer_count"].timestamp, lastSeen.timestamp)

        metrics.queryNetworkConnection(
            "messaging", false, false, "dashboard")
        gateway.completeRequest(0, failure("dashboard failed"))
        compare(metrics.dashboardMetricLastSeen[
            "messaging.peer_count"].timestamp, lastSeen.timestamp)
        compare(metrics.dashboardMetricLastSeen[
            "messaging.peer_count"].value, lastSeen.value)
    }

    function test_newer_health_status_does_not_discard_older_valid_metrics() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        const metricsSequence = metrics.activeMessagingMetricsLease.sequence
        metrics.queryNetworkConnection(
            "messaging", false, false, "dashboard")
        const healthSequence = metrics.activeObservationLeases
            .messaging.sequence
        verify(healthSequence > metricsSequence)

        gateway.completeRequest(
            1, success(sourceReport(true, "newer-health")))
        gateway.completeRequest(
            0, success(deliveryMetricsReport(true, "older-metrics", 14)))

        compare(metrics.sourceReport("messaging").marker, "newer-health")
        compare(metrics.openMetricValue("messaging", "libp2p_peers"), 14)
        compare(metrics.messagingMetricsRequestGeneration, metricsSequence)
        compare(metrics.messagingMetricsAttempt.requestGeneration,
            metricsSequence)
        compare(metrics.networkConnectionState(
            "messaging").requestGeneration, healthSequence)
        compare(metrics.sourceObservation(
            "messaging").latestAttempt.requestGeneration, healthSequence)
    }

    function test_passive_storage_completion_refreshes_capability_report() {
        metrics.queryNetworkConnection(
            "storage", true, false, "source-inspection")
        gateway.completeRequest(
            0, success(sourceReport(true, "explicit-full")))
        const statusRevision = metrics.networkConnectionStatusRevision
        const reportRevision = metrics.observationReportRevisions.storage
        const capabilityRefreshCount = gateway.capabilityRefreshCount

        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        compare(
            gateway.requests[0].args[0].options.runtime_diagnostics_enabled,
            true
        )
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
        gateway.completeRequest(
            0, success(sourceReport(true, "passive-full")))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "passive-full")
        compare(observation.provenance.origin, "scheduler")
        verify(observation.status.ok)
        compare(observation.latestAttempt.origin, "scheduler")
        verify(observation.latestAttempt.transportOk)
        verify(!observation.latestAttempt.runtimeDiagnosticsReduced)
        compare(metrics.networkConnectionStatusRevision, statusRevision + 1)
        compare(metrics.observationReportRevisions.storage, reportRevision + 1)
        compare(gateway.capabilityRefreshCount, capabilityRefreshCount + 1)
    }

    function test_first_passive_storage_report_commits_capability_evidence() {
        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        verify(metrics.activeObservationLeases.storage
            .runtimeDiagnosticsEnabled)
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)

        gateway.completeRequest(
            0, success(sourceReport(true, "passive-capability-evidence")))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "passive-capability-evidence")
        verify(observation.status.known)
        verify(observation.status.ok)
        verify(observation.latestAttempt.transportOk)
        verify(!observation.latestAttempt.runtimeDiagnosticsReduced)
        compare(gateway.capabilityRefreshCount, 1)
    }

    function test_passive_failure_marks_preserved_full_report_stale() {
        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        gateway.completeRequest(
            0, success(sourceReport(true, "explicit-full")))
        const reportRevision = metrics.observationReportRevisions.storage

        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        gateway.completeRequest(0, failure("passive transport down"))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "explicit-full")
        compare(observation.provenance.origin, "source-inspection")
        verify(!observation.status.ok)
        verify(observation.status.stale)
        compare(observation.status.detail, "passive transport down")
        compare(metrics.observationReportRevisions.storage, reportRevision)
    }

    function test_stale_module_source_keeps_passive_storage_poll_full() {
        metrics.queryNetworkConnection(
            "storage", false, true, "source-inspection")
        const fullReport = sourceReport(true, "explicit-full")
        fullReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: false
        }]
        gateway.completeRequest(0, success(fullReport))
        const reportRevision = metrics.observationReportRevisions.storage

        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
        gateway.completeRequest(0, failure("scheduler refresh failed"))

        let observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "explicit-full")
        verify(observation.status.stale)
        compare(observation.status.detail, "scheduler refresh failed")

        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        compare(gateway.requests[0].args[0].options.cid, "z-test-cid")
        verify(gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
        const recoveredReport = sourceReport(true, "recovered-full")
        recoveredReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: false
        }]
        gateway.completeRequest(0, success(recoveredReport))

        observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "recovered-full")
        compare(observation.provenance.origin, "scheduler")
        compare(observation.reportRevision, reportRevision + 1)
        compare(metrics.observationReportStorageCid("storage"), "z-test-cid")
        compare(metrics.reportProbeValue(
            observation.sourceReport, "exists"), false)
        verify(observation.status.known)
        verify(observation.status.ok)
        verify(!observation.status.stale)
        compare(observation.status.origin, "scheduler")
        compare(observation.latestAttempt.origin, "scheduler")
        verify(observation.latestAttempt.transportOk)
        verify(!observation.latestAttempt.runtimeDiagnosticsReduced)
    }

    function test_stale_storage_recovery_does_not_probe_edited_cid() {
        sourceRouting.storageCid = "cid-a"
        metrics.queryNetworkConnection(
            "storage", false, true, "source-inspection")
        const fullReport = sourceReport(true, "explicit-cid-a")
        fullReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: false
        }]
        gateway.completeRequest(0, success(fullReport))

        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        gateway.completeRequest(0, failure("scheduler refresh failed"))
        verify(metrics.sourceObservation("storage").status.stale)

        sourceRouting.storageCid = "cid-b"
        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        compare(gateway.requests[0].args[0].options.cid, "")
        verify(gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
        gateway.completeRequest(
            0, success(sourceReport(true, "recovered-without-cid")))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "recovered-without-cid")
        compare(metrics.observationReportStorageCid("storage"), "")
        compare(metrics.reportProbe(
            observation.sourceReport, "exists"), null)
        verify(observation.status.ok)
        verify(!observation.status.stale)
    }

    function test_stale_messaging_source_escalates_passive_poll_to_full() {
        metrics.queryNetworkConnection(
            "messaging", false, false, "source-inspection")
        gateway.completeRequest(
            0, success(sourceReport(true, "explicit-delivery")))
        const reportRevision = metrics.observationReportRevisions.messaging

        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        verify(metrics.activeMessagingMetricsLease !== null)
        gateway.completeRequest(0, failure("delivery refresh failed"))
        verify(metrics.sourceObservation("messaging").status.stale)

        metrics.queryNetworkConnection(
            "messaging", false, false, "scheduler")
        verify(gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(!gateway.requests[0].args[0].options
            .runtime_metrics_enabled)
        verify(!gateway.requests[0].runtimeMetricsOnly)
        verify(!metrics.activeObservationLeases.messaging
            .runtimeDiagnosticsReduced)
        compare(metrics.activeMessagingMetricsLease, null)
        gateway.completeRequest(
            0, success(sourceReport(true, "recovered-delivery")))

        const observation = metrics.sourceObservation("messaging")
        compare(observation.sourceReport.marker, "recovered-delivery")
        compare(observation.provenance.origin, "scheduler")
        compare(observation.reportRevision, reportRevision + 1)
        verify(observation.status.ok)
        verify(!observation.status.stale)
        verify(!observation.latestAttempt.runtimeDiagnosticsReduced)
    }

    function test_passive_blockchain_completion_replaces_explicit_report() {
        metrics.queryNetworkConnection(
            "blockchain", false, false, "manual")
        gateway.completeRequest(
            0, success(sourceReport(true, "explicit-blockchain")))
        const reportRevision = metrics.observationReportRevisions.blockchain

        metrics.queryNetworkConnection(
            "blockchain", false, false, "dashboard")
        gateway.completeRequest(
            0, success(sourceReport(true, "dashboard-blockchain")))

        const observation = metrics.sourceObservation("blockchain")
        compare(observation.sourceReport.marker, "dashboard-blockchain")
        compare(observation.provenance.origin, "dashboard")
        compare(
            metrics.observationReportRevisions.blockchain,
            reportRevision + 1
        )
    }

    function test_passive_endpoint_observation_refreshes_live_report() {
        sourceRouting.storageSourceMode = "rest"
        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        gateway.completeRequest(
            0, success(sourceReport(true, "endpoint-explicit")))
        const reportRevision = metrics.observationReportRevisions.storage

        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        verify(
            gateway.requests[0].args[0].options.runtime_diagnostics_enabled
        )
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
        gateway.completeRequest(
            0, success(sourceReport(true, "endpoint-scheduled")))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "endpoint-scheduled")
        compare(observation.provenance.origin, "scheduler")
        compare(
            metrics.observationReportRevisions.storage,
            reportRevision + 1
        )
    }

    function test_endpoint_background_refresh_repeats_current_cid_data() {
        return [
            { tag: "scheduler", origin: "scheduler" },
            { tag: "dashboard", origin: "dashboard" },
            { tag: "module event", origin: "module-event" },
            { tag: "storage refresh", origin: "storage-refresh" }
        ]
    }

    function test_endpoint_background_refresh_repeats_current_cid(data) {
        sourceRouting.storageSourceMode = "rest"
        sourceRouting.storageCid = "cid-a"
        const explicitReport = sourceReport(true, "explicit-cid")
        explicitReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: false
        }]

        metrics.queryNetworkConnection(
            "storage", false, true, "source-inspection")
        gateway.completeRequest(0, success(explicitReport))
        compare(metrics.observationReportStorageCid("storage"), "cid-a")

        metrics.queryNetworkConnection(
            "storage", false, false, data.origin)
        compare(gateway.requests[0].args[0].options.cid, "cid-a")
        const refreshedReport = sourceReport(true, "background-cid")
        refreshedReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: true
        }]
        gateway.completeRequest(0, success(refreshedReport))

        compare(metrics.sourceReport("storage").marker, "background-cid")
        compare(metrics.observationReportStorageCid("storage"), "cid-a")
        const exists = metrics.reportProbe(
            metrics.sourceReport("storage"), "exists")
        verify(exists !== null)
        compare(exists.value, true)
    }

    function test_endpoint_background_refresh_omits_edited_cid() {
        sourceRouting.storageSourceMode = "rest"
        sourceRouting.storageCid = "cid-a"
        const explicitReport = sourceReport(true, "explicit-cid-a")
        explicitReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: false
        }]
        metrics.queryNetworkConnection(
            "storage", false, true, "source-inspection")
        gateway.completeRequest(0, success(explicitReport))

        sourceRouting.storageCid = "cid-b"
        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        compare(gateway.requests[0].args[0].options.cid, "")
        gateway.completeRequest(
            0, success(sourceReport(true, "background-without-cid")))

        compare(metrics.sourceReport("storage").marker,
                "background-without-cid")
        compare(metrics.observationReportStorageCid("storage"), "")
        compare(metrics.reportProbe(
            metrics.sourceReport("storage"), "exists"), null)
    }

    function test_module_background_without_cid_preserves_full_report_on_failure() {
        sourceRouting.storageSourceMode = "logoscore_cli"
        sourceRouting.storageCid = "cid-a"
        const explicitReport = sourceReport(true, "explicit-module-cid")
        explicitReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: false
        }]
        metrics.queryNetworkConnection(
            "storage", false, true, "source-inspection")
        gateway.completeRequest(0, success(explicitReport))

        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        compare(gateway.requests[0].args[0].options.cid, "")
        verify(gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
        gateway.completeRequest(0, failure("module status unavailable"))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "explicit-module-cid")
        compare(metrics.observationReportStorageCid("storage"), "cid-a")
        verify(observation.status.stale)
        compare(observation.status.detail, "module status unavailable")
    }

    function test_module_event_refreshes_current_cid_after_mutation() {
        sourceRouting.storageSourceMode = "logoscore_cli"
        sourceRouting.storageCid = "cid-a"
        const explicitReport = sourceReport(true, "before-remove")
        explicitReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: true
        }]
        metrics.queryNetworkConnection(
            "storage", false, true, "source-inspection")
        gateway.completeRequest(0, success(explicitReport))

        metrics.queryNetworkConnection(
            "storage", false, false, "module-event")
        compare(gateway.requests[0].args[0].options.cid, "cid-a")
        verify(gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
        const refreshedReport = sourceReport(true, "after-remove")
        refreshedReport.probes = [{
            probe_key: "exists",
            ok: true,
            value: false
        }]
        gateway.completeRequest(0, success(refreshedReport))

        compare(metrics.sourceReport("storage").marker, "after-remove")
        compare(metrics.observationReportStorageCid("storage"), "cid-a")
        compare(metrics.reportProbeValue(
            metrics.sourceReport("storage"), "exists"), false)
    }

    function test_storage_mutation_refreshes_matching_current_module_cid() {
        sourceRouting.storageSourceMode = "logoscore_cli"
        sourceRouting.storageCid = "cid-a"

        metrics.queryStorageAfterMutation("cid-a")

        compare(gateway.requests.length, 1)
        compare(gateway.requests[0].args[0].options.cid, "cid-a")
        verify(gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
    }

    function test_storage_mutation_omits_edited_module_cid_with_full_capability_report() {
        sourceRouting.storageSourceMode = "logoscore_cli"
        sourceRouting.storageCid = "cid-b"

        metrics.queryStorageAfterMutation("cid-a")

        compare(gateway.requests.length, 1)
        compare(gateway.requests[0].args[0].options.cid, "")
        verify(gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(!metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)
    }

    function test_compatibility_projection_replaces_full_report() {
        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        gateway.completeRequest(
            0, success(sourceReport(true, "explicit-full")))
        const reportRevision = metrics.observationReportRevisions.storage

        verify(metrics.updateNetworkConnectionStatus(
            "storage",
            success(sourceReport(true, "compatibility-full"))
        ))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "compatibility-full")
        compare(observation.provenance.origin, "compatibility")
        compare(
            metrics.observationReportRevisions.storage,
            reportRevision + 1
        )
    }

    function test_different_request_identity_supersedes_active_lease() {
        metrics.queryNetworkConnection("storage", false, false)
        metrics.queryNetworkConnection("storage", false, true)

        compare(gateway.requests.length, 2)
        verify(metrics.networkConnectionIsPending("storage"))
        verify(!gateway.completeRequest(0, success(sourceReport(true, "stale"))))
        verify(metrics.networkConnectionIsPending("storage"))
        verify(gateway.completeRequest(0, success(sourceReport(true, "current"))))
        compare(metrics.sourceReport("storage").marker, "current")
        verify(!metrics.networkConnectionIsPending("storage"))
    }

    function test_source_and_module_reports_keep_distinct_provenance() {
        metrics.setModuleReport("storage", { marker: "module" })
        metrics.setSourceReport("storage", { marker: "source" }, {
            origin: "source-test",
            configurationGeneration: 2,
            requestGeneration: 4,
            checkedAtMs: 10
        })

        const observation = metrics.sourceObservation("storage")
        compare(observation.moduleReport.marker, "module")
        compare(observation.sourceReport.marker, "source")
        compare(observation.provenance.origin, "source-test")
    }

    function test_stale_observation_keeps_report_and_attempt_times_distinct() {
        metrics.queryNetworkConnection("storage", false)
        gateway.completeRequest(0, success(sourceReport(true, "timed")))
        const completed = metrics.sourceObservation("storage")
        verify(completed.reportCheckedAtMs > 0)

        wait(2)
        metrics.queryNetworkConnection("storage", false)
        gateway.completeRequest(0, failure("later failure"))

        const stale = metrics.sourceObservation("storage")
        compare(stale.reportCheckedAtMs, completed.reportCheckedAtMs)
        verify(stale.checkedAtMs > stale.reportCheckedAtMs)
        verify(stale.reportCheckedAt.length > 0)
        verify(stale.checkedAt.length > 0)
    }

    function test_cid_specific_report_is_not_retained_for_different_cid() {
        sourceRouting.storageCid = "cid-a"
        metrics.queryNetworkConnection("storage", false, true, "source-inspection")
        gateway.completeRequest(0, success({
            marker: "cid-a-report",
            health: { ready: true, status: "healthy", detail: "ready" },
            probes: [{ method: "exists", ok: true, value: true }]
        }))
        compare(metrics.sourceReport("storage").marker, "cid-a-report")

        sourceRouting.storageCid = "cid-b"
        metrics.queryNetworkConnection("storage", false, true, "source-inspection")
        gateway.completeRequest(0, failure("cid-b unavailable"))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport, null)
        verify(!observation.stale)
        compare(observation.status.detail, "cid-b unavailable")
    }

    function test_blockchain_connection_report_has_independent_lifecycle() {
        metrics.setModuleReport("blockchain", { marker: "module-report" })
        const moduleRevision = metrics.moduleReportRevisions.blockchain
        metrics.queryNetworkConnection("blockchain", false, false, "scheduler")
        gateway.completeRequest(0, success({
            marker: "connection-report",
            cryptarchia_info: null
        }))

        let observation = metrics.sourceObservation("blockchain")
        compare(observation.sourceReport.marker, "connection-report")
        compare(observation.moduleReport.marker, "module-report")
        compare(metrics.observationReport("blockchain").marker, "connection-report")
        compare(testRoot.dashboardNode.marker, "connection-report")

        metrics.invalidateConfiguration("blockchain", "node changed")

        observation = metrics.sourceObservation("blockchain")
        compare(observation.sourceReport, null)
        compare(observation.moduleReport.marker, "module-report")
        compare(metrics.moduleReportRevisions.blockchain, moduleRevision)
        compare(testRoot.dashboardNode, null)
        verify(!observation.status.known)
    }

    function test_blockchain_all_nested_probe_failures_are_unhealthy() {
        metrics.queryNetworkConnection("blockchain", false, false, "scheduler")
        gateway.completeRequest(0, success(blockchainReport(false, false, false, false)))

        const observation = metrics.sourceObservation("blockchain")
        verify(observation.latestAttempt.transportOk)
        verify(!observation.status.ok)
        compare(observation.status.detail, "cryptarchia down")
        compare(metrics.networkConnectionSummary("blockchain",
            observation.sourceReport), "cryptarchia down")
    }

    function test_blockchain_partial_nested_probe_success_is_degraded() {
        metrics.queryNetworkConnection("blockchain", false, false, "scheduler")
        gateway.completeRequest(0, success(blockchainReport(false, true, false, false)))

        const observation = metrics.sourceObservation("blockchain")
        verify(observation.latestAttempt.transportOk)
        verify(!observation.status.ok)
        compare(observation.status.detail,
                "Cryptarchia unavailable; other Bedrock APIs responded")
    }

    function test_blockchain_cryptarchia_success_is_healthy_and_stale_failure_keeps_report() {
        metrics.queryNetworkConnection("blockchain", false, false, "scheduler")
        gateway.completeRequest(0, success(blockchainReport(true, true, true, true)))

        const completed = metrics.sourceObservation("blockchain")
        verify(completed.status.ok)
        compare(completed.status.detail, "slot 42")

        metrics.queryNetworkConnection("blockchain", false, false, "scheduler")
        gateway.completeRequest(0, failure("connection refused"))

        const stale = metrics.sourceObservation("blockchain")
        verify(!stale.status.ok)
        verify(stale.status.stale)
        compare(stale.sourceReport.cryptarchia_info.value.cryptarchia_info.slot, 42)
        compare(stale.status.detail, "connection refused")
    }

    function test_missing_lib_slot_keeps_finality_gap_unknown() {
        testRoot.dashboardNode = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 42, lib_slot: null }
                }
            }
        }
        compare(metrics.tipMinusLib(), null)
        compare(metrics.finalityLagSeconds(), null)

        testRoot.dashboardNode = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 42 }
                }
            }
        }
        compare(metrics.tipMinusLib(), null)
        compare(metrics.finalityLagSeconds(), null)
    }

    function test_dashboard_uses_shared_family_pending_lifecycle() {
        verify(metrics.refreshDashboard())

        verify(metrics.dashboardRefreshing)
        verify(metrics.networkConnectionIsPending("blockchain"))
        verify(metrics.networkConnectionIsPending("storage"))
        verify(metrics.networkConnectionIsPending("messaging"))
        compare(gateway.requests.length, 3)

        gateway.completeRequest(0, success({
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 30, lib_slot: 20 }
                }
            }
        }))
        compare(gateway.requests.length, 3)
        compare(gateway.requests[2].method, "blockchainLiveBlocks")
        compare(gateway.requests[2].args[0], 0)
        compare(gateway.requests[2].args[1], 30)
        gateway.completeRequest(0, success(sourceReport(true, "storage-dashboard")))
        gateway.completeRequest(0, success(sourceReport(false, "messaging-dashboard")))
        gateway.completeRequest(0, success({ blocks: [{ id: "l1" }] }))

        verify(!metrics.dashboardRefreshing)
        verify(!metrics.networkConnectionIsPending("blockchain"))
        verify(!metrics.networkConnectionIsPending("storage"))
        verify(!metrics.networkConnectionIsPending("messaging"))
        compare(metrics.sourceReport("storage").marker, "storage-dashboard")
        compare(metrics.sourceReport("messaging").marker, "messaging-dashboard")
        compare(testRoot.dashboardL1BlocksSlotTo, 30)
        compare(gateway.dashboardResultCount, 1)
        verify(gateway.dashboardResultOk)
    }

    function test_failed_live_refresh_preserves_old_anchor_for_freshness_rejection() {
        testRoot.dashboardL1Blocks = [{ id: "old-live-block" }]
        testRoot.dashboardL1BlocksSlotTo = 29

        verify(metrics.refreshDashboard())
        gateway.completeRequest(0, success({
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 30, lib_slot: 20 }
                }
            }
        }))
        gateway.completeRequest(0, success(sourceReport(true, "storage")))
        gateway.completeRequest(0, success(sourceReport(true, "messaging")))
        gateway.completeRequest(0, failure("live blocks unavailable"))

        verify(!metrics.dashboardRefreshing)
        compare(testRoot.dashboardL1Blocks.length, 1)
        compare(testRoot.dashboardL1Blocks[0].id, "old-live-block")
        compare(testRoot.dashboardL1BlocksSlotTo, 29)
    }

    function test_dashboard_omits_live_block_request_when_connector_does_not_support_it() {
        sourceRouting.supportsLiveBlocks = false

        verify(metrics.refreshDashboard())

        compare(gateway.requests.length, 3)
        verify(!gateway.requests.some(function (request) {
            return request.method === "blockchainLiveBlocks"
        }))
        gateway.completeRequest(0, success({ cryptarchia_info: null }))
        gateway.completeRequest(0, success(sourceReport(true, "storage")))
        gateway.completeRequest(0, success(sourceReport(true, "messaging")))
        verify(!metrics.dashboardRefreshing)
        verify(gateway.dashboardResultOk)
    }

    function test_invalidated_dashboard_does_not_start_live_blocks_from_stale_node_reply() {
        testRoot.dashboardL1Blocks = [{ id: "old-live-block" }]
        testRoot.dashboardL1BlocksSlotTo = 29
        verify(metrics.refreshDashboard())
        compare(gateway.requests.length, 3)

        metrics.invalidateDashboard("test invalidation")
        compare(testRoot.dashboardL1Blocks.length, 0)
        compare(testRoot.dashboardL1BlocksSlotTo, 0)
        gateway.completeRequest(0, success({
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 30, lib_slot: 20 }
                }
            }
        }))

        verify(!metrics.dashboardRefreshing)
        verify(!gateway.requests.some(function (request) {
            return request.method === "blockchainLiveBlocks"
        }))
        compare(gateway.invalidatedDashboardCount, 1)
    }

    function test_synchronous_dashboard_rejection_settles_once() {
        gateway.synchronouslyRejectDashboard = true
        verify(metrics.refreshDashboard())

        verify(metrics.dashboardRefreshing)
        compare(gateway.requests.length, 3)
        compare(gateway.dashboardResultCount, 0)

        gateway.completeRequest(0, success({
            cryptarchia_info: {
                value: {
                    cryptarchia_info: { slot: 30, lib_slot: 20 }
                }
            }
        }))
        compare(gateway.dashboardStartCount, 1)
        gateway.completeRequest(0, success(sourceReport(true, "storage")))
        verify(metrics.dashboardRefreshing)
        compare(gateway.dashboardResultCount, 0)
        gateway.completeRequest(0, success(sourceReport(true, "messaging")))

        verify(!metrics.dashboardRefreshing)
        compare(gateway.dashboardResultCount, 1)
        verify(gateway.dashboardResultOk)
    }
}
