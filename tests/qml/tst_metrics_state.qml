import QtQml
import QtTest
import "../../qml/state/domains"

TestCase {
    id: testRoot

    name: "MetricsState"

    property var dashboardOverview: null
    property var dashboardNode: null
    property var dashboardL1Blocks: []
    property var dashboardBlocks: []
    property var dashboardProvisionalBlocks: []
    property int joinedCompletionCount: 0

    QtObject {
        id: sourceRouting

        property string storageEndpoint: "http://storage.invalid"
        property string storageCid: "z-test-cid"

        function deliverySourceReportArgs() {
            return [{ source_mode: "rest", inputs: { rest_endpoint: "http://delivery.invalid" } }]
        }

        function storageSourceReportArgs(includeSensitiveProbe) {
            return [{
                source_mode: "rest",
                inputs: {
                    rest_endpoint: storageEndpoint
                },
                options: {
                    cid: includeSensitiveProbe === true ? storageCid : "",
                    privileged_debug_enabled: false
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

        function reset() {
            requests = []
            capabilityRefreshCount = 0
            dashboardResultCount = 0
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

        function beginObservationPresentation(label) {
            presentationSequence += 1
            activePresentationGeneration = presentationSequence
            presentationBeginCount += 1
            return { generation: presentationSequence, label: String(label || "") }
        }

        function completeObservationPresentation(lease, title, text, isError, value) {
            if (!lease || Number(lease.generation || 0) !== activePresentationGeneration) {
                return false
            }
            activePresentationGeneration = 0
            presentationCompleteCount += 1
            lastPresentationError = isError === true
            lastPresentationText = String(text || "")
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

        function cacheBlockchainResult(method, value) {
            if (method === "blockchainNode") {
                testRoot.dashboardNode = value || null
            } else if (method === "blockchainLiveBlocks") {
                testRoot.dashboardL1Blocks = value && Array.isArray(value.blocks)
                    ? value.blocks : []
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
        testRoot.dashboardOverview = null
        testRoot.dashboardNode = null
        testRoot.dashboardL1Blocks = []
        testRoot.dashboardBlocks = []
        testRoot.dashboardProvisionalBlocks = []
        joinedCompletionCount = 0
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

    function test_second_observer_joins_active_family_lease() {
        metrics.queryNetworkConnection("messaging", false)
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

    function test_interactive_observer_joins_background_lease_and_presents() {
        metrics.queryNetworkConnection("messaging", false, false, "scheduler")
        const joined = metrics.queryNetworkConnection(
            "messaging", true, false, "source-inspection")

        verify(joined.joined)
        compare(gateway.requests.length, 1)
        compare(gateway.presentationBeginCount, 1)

        gateway.completeRequest(0, success(sourceReport(true, "presented")))

        compare(gateway.presentationCompleteCount, 1)
        verify(!gateway.lastPresentationError)
        verify(gateway.lastPresentationText.length > 0)
        compare(metrics.sourceReport("messaging").marker, "presented")
    }

    function test_interactive_join_prevents_incompatible_background_supersede() {
        metrics.queryNetworkConnection("storage", false, false, "scheduler")
        const joined = metrics.queryNetworkConnection(
            "storage", true, false, "source-inspection")
        verify(joined.joined)
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

    function test_dashboard_uses_shared_family_pending_lifecycle() {
        verify(metrics.refreshDashboard())

        verify(metrics.dashboardRefreshing)
        verify(metrics.networkConnectionIsPending("blockchain"))
        verify(metrics.networkConnectionIsPending("storage"))
        verify(metrics.networkConnectionIsPending("messaging"))
        compare(gateway.requests.length, 4)

        gateway.completeRequest(0, success({ cryptarchia_info: null }))
        gateway.completeRequest(0, success(sourceReport(true, "storage-dashboard")))
        gateway.completeRequest(0, success(sourceReport(false, "messaging-dashboard")))
        gateway.completeRequest(0, success({ blocks: [{ id: "l1" }] }))

        verify(!metrics.dashboardRefreshing)
        verify(!metrics.networkConnectionIsPending("blockchain"))
        verify(!metrics.networkConnectionIsPending("storage"))
        verify(!metrics.networkConnectionIsPending("messaging"))
        compare(metrics.sourceReport("storage").marker, "storage-dashboard")
        compare(metrics.sourceReport("messaging").marker, "messaging-dashboard")
        compare(gateway.dashboardResultCount, 1)
        verify(gateway.dashboardResultOk)
    }

    function test_synchronous_dashboard_rejection_settles_once() {
        gateway.synchronouslyRejectDashboard = true
        verify(metrics.refreshDashboard())

        verify(metrics.dashboardRefreshing)
        compare(gateway.requests.length, 3)
        compare(gateway.dashboardResultCount, 0)

        gateway.completeRequest(0, success({ cryptarchia_info: null }))
        gateway.completeRequest(0, success(sourceReport(true, "storage")))
        verify(metrics.dashboardRefreshing)
        compare(gateway.dashboardResultCount, 0)
        gateway.completeRequest(0, success(sourceReport(true, "messaging")))

        verify(!metrics.dashboardRefreshing)
        compare(gateway.dashboardResultCount, 1)
        verify(gateway.dashboardResultOk)
    }
}
