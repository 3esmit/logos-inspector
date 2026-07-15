import QtQuick
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "AppRequestState"

    QtObject {
        id: shell

        property bool busy: false
        property string currentView: "modules"
        property string statusText: ""
        property string resultTitle: ""
        property string resultText: ""
        property bool resultIsError: false
        property var resultValue: null
        property string resultOwner: ""
        property int resultGeneration: 0

        function setResult(title, text, isError, value, owner) {
            resultGeneration += 1
            resultTitle = String(title || "")
            resultText = String(text || "")
            resultIsError = isError === true
            resultValue = value
            resultOwner = String(owner || "")
        }
    }

    QtObject {
        id: bridge

        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []
        property int syncCallCount: 0
        property int asyncCallCount: 0
        property bool deferAsync: false
        property var pendingAsyncCalls: []
        property var response: ({ ok: true, value: { ok: true }, text: "ok", error: "" })

        function reset() {
            lastModule = ""
            lastMethod = ""
            lastArgs = []
            syncCallCount = 0
            asyncCallCount = 0
            deferAsync = false
            pendingAsyncCalls = []
            response = ({ ok: true, value: { ok: true }, text: "ok", error: "" })
        }

        function callModule(moduleName, method, args) {
            syncCallCount += 1
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = Array.isArray(args) ? args : []
            return response
        }

        function callModuleAsync(moduleName, method, args, callback) {
            asyncCallCount += 1
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = Array.isArray(args) ? args : []
            if (deferAsync) {
                const calls = pendingAsyncCalls.slice()
                calls.push({ callback: callback })
                pendingAsyncCalls = calls
                return "request-" + asyncCallCount
            }
            callback(response)
            return "request-" + asyncCallCount
        }

        function completeAsync(index, value) {
            const calls = pendingAsyncCalls.slice()
            const call = calls.splice(index, 1)[0]
            pendingAsyncCalls = calls
            call.callback(value)
        }
    }

    AppRequestState {
        id: requests

        bridge: bridge
        shell: shell
        inspectorModule: "logos_inspector"
        updateDashboardCache: function (method, value) {
            testRoot.cachedMethod = String(method || "")
            testRoot.cachedValue = value
        }
        updateNetworkConnectionStatus: function (method, response) {
            testRoot.networkMethod = String(method || "")
            testRoot.networkOk = response && response.ok === true
        }
    }

    property string cachedMethod: ""
    property var cachedValue: null
    property string networkMethod: ""
    property bool networkOk: false
    property int projectedResponseCount: 0

    function init() {
        shell.busy = false
        shell.currentView = "modules"
        shell.statusText = ""
        shell.resultTitle = ""
        shell.resultText = ""
        shell.resultIsError = false
        shell.resultValue = null
        shell.resultOwner = ""
        shell.resultGeneration = 0
        bridge.reset()
        requests.nextAsyncGeneration = 1
        requests.latestAsyncGenerationByMethod = ({})
        requests.activePresentationGeneration = 0
        cachedMethod = ""
        cachedValue = null
        networkMethod = ""
        networkOk = false
        projectedResponseCount = 0
        requests.projectObservationResponse = null
    }

    function test_external_module_calls_route_through_inspector() {
        const response = requests.requestModule("storage_module", "space", ["rest"], "Storage", true)

        verify(response.ok)
        compare(bridge.syncCallCount, 1)
        compare(bridge.asyncCallCount, 0)
        compare(bridge.lastModule, "logos_inspector")
        compare(bridge.lastMethod, "callModule")
        compare(bridge.lastArgs[0], "storage_module")
        compare(bridge.lastArgs[1], "space")
        compare(shell.resultTitle, "Storage")
        compare(cachedMethod, "space")
        compare(networkMethod, "space")
    }

    function test_async_module_report_uses_inspector_route() {
        let callbackCount = 0

        const requestId = requests.callInspectorAsync("modules", [], "Modules", function (response) {
            callbackCount += 1
            verify(response.ok)
        })

        compare(requestId, "request-1")
        compare(bridge.syncCallCount, 0)
        compare(bridge.asyncCallCount, 1)
        compare(bridge.lastModule, "logos_inspector")
        compare(bridge.lastMethod, "modules")
        compare(bridge.lastArgs.length, 0)
        compare(callbackCount, 1)
    }

    function test_async_external_module_call_wraps_through_inspector() {
        let callbackCount = 0

        requests.requestModuleAsync("storage_module", "space", ["rest"], "Storage", false, function (response) {
            callbackCount += 1
            verify(response.ok)
        })

        compare(bridge.syncCallCount, 0)
        compare(bridge.asyncCallCount, 1)
        compare(bridge.lastModule, "logos_inspector")
        compare(bridge.lastMethod, "callModule")
        compare(bridge.lastArgs[0], "storage_module")
        compare(bridge.lastArgs[1], "space")
        compare(bridge.lastArgs[2][0], "rest")
        compare(callbackCount, 1)
    }

    function test_unobserved_async_request_skips_domain_projection() {
        let callbackCount = 0

        requests.requestModuleAsyncUnobserved(
            "logos_inspector",
            "storageSourceReport",
            [],
            "Storage",
            true,
            function () { callbackCount += 1 }
        )

        compare(callbackCount, 1)
        compare(shell.resultTitle, "Storage")
        compare(cachedMethod, "")
        compare(networkMethod, "")
    }

    function test_combined_projection_replaces_legacy_double_hooks() {
        requests.projectObservationResponse = function (method, response, cacheResult) {
            projectedResponseCount += 1
            compare(method, "storageSourceReport")
            verify(response.ok)
            verify(cacheResult)
        }

        requests.requestModuleAsync(
            "logos_inspector",
            "storageSourceReport",
            [],
            "Storage",
            false
        )

        compare(projectedResponseCount, 1)
        compare(cachedMethod, "")
        compare(networkMethod, "")
    }

    function test_async_zone_status_uses_inspector_route() {
        requests.requestModuleAsync("logos_inspector", "zoneCatalogStatus", [{}], "", false)

        compare(bridge.syncCallCount, 0)
        compare(bridge.asyncCallCount, 1)
        compare(bridge.lastModule, "logos_inspector")
        compare(bridge.lastMethod, "zoneCatalogStatus")
    }

    function test_async_runtime_status_uses_inspector_route() {
        requests.requestModuleAsync("logos_inspector", "runtimeOperationStatus", ["op-7"], "", false)

        compare(bridge.syncCallCount, 0)
        compare(bridge.asyncCallCount, 1)
        compare(bridge.lastModule, "logos_inspector")
        compare(bridge.lastMethod, "runtimeOperationStatus")
        compare(bridge.lastArgs[0], "op-7")
    }

    function test_async_same_method_reverse_completion_invokes_both_callbacks() {
        bridge.deferAsync = true
        const completed = []
        requests.requestModuleAsync(
            "logos_inspector",
            "runtimeOperationStatus",
            ["op-a"],
            "",
            false,
            function (response) { completed.push(response.value.operationId) }
        )
        requests.requestModuleAsync(
            "logos_inspector",
            "runtimeOperationStatus",
            ["op-b"],
            "",
            false,
            function (response) { completed.push(response.value.operationId) }
        )

        bridge.completeAsync(1, {
            ok: true,
            value: { operationId: "op-b" },
            text: "",
            error: ""
        })
        bridge.completeAsync(0, {
            ok: true,
            value: { operationId: "op-a" },
            text: "",
            error: ""
        })

        compare(completed.join(","), "op-b,op-a")
        compare(cachedValue.operationId, "op-b")
    }

    function test_busy_request_rejects_without_bridge_call() {
        shell.busy = true

        const response = requests.requestModule("logos_inspector", "blockchainNode", [], "Node", true)

        verify(!response.ok)
        compare(bridge.lastMethod, "")
        compare(response.error, "Another inspection is already running.")
    }

    function test_async_accept_response_can_ignore_result() {
        let callbackCount = 0

        requests.requestModuleAsync("logos_inspector", "blockchainNode", [], "Node", true, function () {
            callbackCount += 1
        }, function () {
            return false
        })

        compare(callbackCount, 0)
        compare(shell.resultTitle, "")
        verify(!requests.presentationBusy)
    }

    function test_async_presentation_busy_clears_only_for_latest_request() {
        bridge.deferAsync = true
        requests.callInspectorAsync("first", [], "First")
        requests.callInspectorAsync("second", [], "Second")

        verify(requests.presentationBusy)
        compare(shell.statusText, "Second")
        bridge.completeAsync(0, {
            ok: true,
            value: { order: 1 },
            text: "first",
            error: ""
        })
        verify(requests.presentationBusy)
        compare(shell.resultTitle, "")

        bridge.completeAsync(0, {
            ok: true,
            value: { order: 2 },
            text: "second",
            error: ""
        })
        verify(!requests.presentationBusy)
        compare(shell.resultTitle, "Second")
        compare(shell.resultValue.order, 2)
    }

    function test_async_reverse_completion_keeps_latest_presentation() {
        bridge.deferAsync = true
        requests.callInspectorAsync("first", [], "First")
        requests.callInspectorAsync("second", [], "Second")

        bridge.completeAsync(1, {
            ok: true,
            value: { order: 2 },
            text: "second",
            error: ""
        })
        verify(!requests.presentationBusy)
        compare(shell.resultTitle, "Second")
        compare(shell.resultValue.order, 2)

        bridge.completeAsync(0, {
            ok: true,
            value: { order: 1 },
            text: "first",
            error: ""
        })
        compare(shell.resultTitle, "Second")
        compare(shell.resultValue.order, 2)
    }

    function test_async_presentation_keeps_originating_view_owner() {
        bridge.deferAsync = true
        requests.callInspectorAsync("modules", [], "Modules")

        shell.currentView = "settings"
        bridge.completeAsync(0, {
            ok: true,
            value: { count: 3 },
            text: "modules",
            error: ""
        })

        compare(shell.currentView, "settings")
        compare(shell.resultOwner, "modules")
        compare(shell.resultTitle, "Modules")
    }

    function test_external_presentation_cannot_overwrite_later_async_presentation() {
        bridge.deferAsync = true
        const external = requests.beginPresentation("Blockchain", "blockchain")
        verify(requests.presentationCurrent(external))

        shell.currentView = "storage"
        requests.callInspectorAsync("storageSourceReport", [], "Storage")
        verify(!requests.presentationCurrent(external))
        bridge.completeAsync(0, {
            ok: true,
            value: { source: "storage" },
            text: "storage",
            error: ""
        })

        compare(shell.resultTitle, "Storage")
        compare(shell.resultOwner, "storage")
        compare(shell.resultValue.source, "storage")
        verify(!requests.completePresentation(
            external,
            "Blockchain",
            "blockchain",
            false,
            { source: "blockchain" }
        ))
        compare(shell.resultTitle, "Storage")
        compare(shell.resultOwner, "storage")
        compare(shell.resultValue.source, "storage")
    }

    function test_direct_result_invalidates_pending_external_presentation() {
        const external = requests.beginPresentation("Remote block", "blockDetail")
        verify(requests.presentationCurrent(external))

        shell.setResult("Cached block", "cached", false, { id: "block-b" }, "blockDetail")

        verify(!requests.presentationCurrent(external))
        verify(!requests.completePresentation(
            external,
            "Remote block",
            "remote",
            false,
            { id: "block-a" }
        ))
        verify(!requests.presentationBusy)
        compare(shell.resultTitle, "Cached block")
        compare(shell.resultValue.id, "block-b")
    }
}
