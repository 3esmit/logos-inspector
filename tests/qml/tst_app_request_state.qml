import QtQuick
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "AppRequestState"

    QtObject {
        id: shell

        property bool busy: false
        property string statusText: ""
        property string resultTitle: ""
        property string resultText: ""
        property bool resultIsError: false
        property var resultValue: null

        function setResult(title, text, isError, value) {
            resultTitle = String(title || "")
            resultText = String(text || "")
            resultIsError = isError === true
            resultValue = value
        }
    }

    QtObject {
        id: bridge

        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []
        property var response: ({ ok: true, value: { ok: true }, text: "ok", error: "" })

        function reset() {
            lastModule = ""
            lastMethod = ""
            lastArgs = []
            response = ({ ok: true, value: { ok: true }, text: "ok", error: "" })
        }

        function callModule(moduleName, method, args) {
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = Array.isArray(args) ? args : []
            return response
        }

        function callModuleAsync(moduleName, method, args, callback) {
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = Array.isArray(args) ? args : []
            callback(response)
            return "request-1"
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

    function init() {
        shell.busy = false
        shell.statusText = ""
        shell.resultTitle = ""
        shell.resultText = ""
        shell.resultIsError = false
        shell.resultValue = null
        bridge.reset()
        cachedMethod = ""
        cachedValue = null
        networkMethod = ""
        networkOk = false
    }

    function test_external_module_calls_route_through_inspector() {
        const response = requests.requestModule("storage_module", "space", ["rest"], "Storage", true)

        verify(response.ok)
        compare(bridge.lastModule, "logos_inspector")
        compare(bridge.lastMethod, "callModule")
        compare(bridge.lastArgs[0], "storage_module")
        compare(bridge.lastArgs[1], "space")
        compare(shell.resultTitle, "Storage")
        compare(cachedMethod, "space")
        compare(networkMethod, "space")
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
    }
}
