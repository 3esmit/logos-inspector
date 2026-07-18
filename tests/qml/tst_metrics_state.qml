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

    function success(value) {
        return { ok: true, value: value, text: "ok", error: "" }
    }

    function failure(message) {
        return { ok: false, value: null, text: "", error: String(message || "failed") }
    }

    function resetMetrics() {
        metrics.blockchainRefreshRate = 30
        metrics.messagingRefreshRate = 30
        metrics.storageRefreshRate = 30
        metrics.networkConnectionStatus = ({})
        metrics.networkConnectionStatusRevision = 0
        metrics.networkConnectionPending = ({})
        metrics.networkConnectionPendingRevision = 0
        metrics.dashboardMetricHistory = ({})
        metrics.dashboardMetricLastSeen = ({})
        metrics.dashboardMetricHistoryRevision = 0
        metrics.dashboardSnapshotRevision = 0
        metrics.dashboardRefreshing = false
        metrics.dashboardRefreshSerial = 0
        metrics.dashboardError = ""
        metrics.blockchainSourceReport = null
        metrics.blockchainModuleReport = null
        metrics.storageModuleReport = null
        metrics.messagingModuleReport = null
        metrics.storageSourceReport = null
        metrics.messagingSourceReport = null
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
        metrics.queryNetworkConnection("messaging", false, false, "scheduler")
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
        compare(gateway.requests.length, 2)
        verify(metrics.networkConnectionIsPending("storage"))
        verify(!gateway.completeRequest(
            0, success(sourceReport(true, "passive-stale"))))
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
        metrics.queryNetworkConnection("messaging", false, false, "scheduler")
        gateway.completeRequest(0, success(sourceReport(true, "scheduled")))

        const observation = metrics.sourceObservation("messaging")
        compare(observation.provenance.origin, "scheduler")
        compare(observation.status.origin, "scheduler")
    }

    function test_passive_observations_skip_runtime_module_calls() {
        const passiveOrigins = [
            "scheduler",
            "dashboard",
            "module-event",
            "storage-refresh"
        ]
        for (let i = 0; i < passiveOrigins.length; ++i) {
            const origin = passiveOrigins[i]
            const storage = metrics.networkConnectionRequest(
                "storage", false, origin)
            const messaging = metrics.networkConnectionRequest(
                "messaging", false, origin)
            compare(
                storage.args[0].options.runtime_diagnostics_enabled,
                false
            )
            compare(
                messaging.args[0].options.runtime_diagnostics_enabled,
                false
            )
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
        }
    }

    function test_passive_completion_preserves_explicit_full_report() {
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
            false
        )
        gateway.completeRequest(
            0, success(sourceReport(false, "passive-static")))

        const observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "explicit-full")
        compare(observation.provenance.origin, "source-inspection")
        verify(observation.status.ok)
        compare(observation.latestAttempt.origin, "scheduler")
        verify(observation.latestAttempt.transportOk)
        verify(observation.latestAttempt.runtimeDiagnosticsReduced)
        compare(metrics.networkConnectionStatusRevision, statusRevision)
        compare(metrics.observationReportRevisions.storage, reportRevision)
        compare(gateway.capabilityRefreshCount, capabilityRefreshCount)
    }

    function test_first_reduced_passive_report_stays_unqueried() {
        metrics.queryNetworkConnection(
            "storage", false, false, "scheduler")
        verify(metrics.activeObservationLeases.storage
            .runtimeDiagnosticsReduced)

        const reducedReport = sourceReport(false, "passive-no-evidence")
        reducedReport.health.reachable = true
        reducedReport.module_info = {
            ok: true,
            value: { supported: false }
        }
        gateway.completeRequest(0, success(reducedReport))

        let observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport, null)
        verify(!observation.status.known)
        verify(observation.latestAttempt.transportOk)
        verify(observation.latestAttempt.runtimeDiagnosticsReduced)

        metrics.queryNetworkConnection(
            "storage", false, false, "source-inspection")
        verify(metrics.activeObservationLeases.storage
            .runtimeDiagnosticsEnabled)
        gateway.completeRequest(
            0, success(sourceReport(true, "explicit-full")))

        observation = metrics.sourceObservation("storage")
        compare(observation.sourceReport.marker, "explicit-full")
        verify(observation.status.known)
        verify(observation.status.ok)
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

    function test_reduced_module_background_keeps_cid_out_of_request() {
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
        verify(!gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
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

    function test_storage_mutation_omits_edited_module_cid() {
        sourceRouting.storageSourceMode = "logoscore_cli"
        sourceRouting.storageCid = "cid-b"

        metrics.queryStorageAfterMutation("cid-a")

        compare(gateway.requests.length, 1)
        compare(gateway.requests[0].args[0].options.cid, "")
        verify(!gateway.requests[0].args[0].options
            .runtime_diagnostics_enabled)
        verify(metrics.activeObservationLeases.storage
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
