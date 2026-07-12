import QtQuick
import QtTest
import "../../qml/services"
import "fixtures"

TestCase {
    id: testRoot

    name: "BridgeClient"

    BridgeHostFixture {
        id: standaloneHost
    }

    QtObject {
        id: basecampHost

        property int callCount: 0
        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []

        function reset() {
            callCount = 0
            lastModule = ""
            lastMethod = ""
            lastArgs = []
        }

        function callModule(moduleName, method, args) {
            callCount += 1
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = args || []
            if (lastModule === "logos_inspector" && lastMethod === "call") {
                return JSON.stringify({
                    ok: true,
                    value: {
                        method: lastArgs[0],
                        args: JSON.parse(String(lastArgs[1] || "[]"))
                    },
                    text: "OK",
                    error: ""
                })
            }
            return "direct"
        }
    }

    QtObject {
        id: asyncHost

        signal moduleCallFinished(int requestId, string responseJson)

        property int lastRequestId: 0
        property string lastModule: ""
        property string lastMethod: ""
        property string lastArgsJson: ""

        function reset() {
            lastRequestId = 0
            lastModule = ""
            lastMethod = ""
            lastArgsJson = ""
        }

        function callModuleJsonAsync(requestId, moduleName, method, argsJson) {
            lastRequestId = requestId
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgsJson = String(argsJson || "")
        }
    }

    QtObject {
        id: eventHost

        signal moduleEvent(string moduleName, string eventName, var args)

        property string subscribedModule: ""
        property string subscribedEvent: ""
        property var subscribedCallback: null

        function reset() {
            subscribedModule = ""
            subscribedEvent = ""
            subscribedCallback = null
        }

        function onModuleEvent(moduleName, eventName, callback) {
            subscribedModule = String(moduleName || "")
            subscribedEvent = String(eventName || "")
            subscribedCallback = callback
        }
    }

    BridgeClient {
        id: client

        host: standaloneHost

        onModuleEventReceived: function (moduleName, eventName, args) {
            testRoot.receivedModule = moduleName
            testRoot.receivedEvent = eventName
            testRoot.receivedArgs = args
        }
    }

    property string receivedModule: ""
    property string receivedEvent: ""
    property var receivedArgs: null
    property var asyncResponse: null

    function init() {
        standaloneHost.reset()
        standaloneHost.defaultResponse = ({
            ok: true,
            value: { answer: 42 },
            text: "OK",
            error: ""
        })
        basecampHost.reset()
        asyncHost.reset()
        eventHost.reset()
        client.host = standaloneHost
        client.nextRequestId = 1
        client.pendingCalls = ({})
        client.moduleEventSubscriptions = ({})
        receivedModule = ""
        receivedEvent = ""
        receivedArgs = null
        asyncResponse = null
    }

    function test_standalone_json_host_round_trips_module_call() {
        const response = client.callModule("logos_inspector", "sourcePolicy", [])

        verify(response.ok)
        compare(response.value.answer, 42)
        compare(standaloneHost.lastModule, "logos_inspector")
        compare(standaloneHost.lastMethod, "sourcePolicy")
        compare(standaloneHost.lastArgs.length, 0)
    }

    function test_basecamp_host_wraps_inspector_calls() {
        client.host = basecampHost

        const response = client.callModule("logos_inspector", "blockchainNode", ["endpoint"])

        verify(response.ok)
        compare(basecampHost.lastModule, "logos_inspector")
        compare(basecampHost.lastMethod, "call")
        compare(basecampHost.lastArgs[0], "blockchainNode")
        compare(response.value.args[0], "endpoint")
    }

    function test_missing_host_returns_bridge_error() {
        client.host = null

        const response = client.callModule("logos_inspector", "blockchainNode", [])

        verify(!response.ok)
        verify(response.error.indexOf("Logos bridge not available") >= 0)
    }

    function test_async_host_uses_request_id_and_finishes_callback() {
        client.host = asyncHost

        const requestId = client.callModuleAsync("logos_inspector", "blockchainNode", ["a"], function (response) {
            asyncResponse = response
        })

        compare(requestId, 1)
        compare(asyncHost.lastRequestId, 1)
        compare(asyncHost.lastModule, "logos_inspector")
        compare(asyncHost.lastMethod, "blockchainNode")
        compare(asyncHost.lastArgsJson, "[\"a\"]")

        asyncHost.moduleCallFinished(1, JSON.stringify({
            ok: true,
            value: { done: true },
            text: "done",
            error: ""
        }))

        verify(asyncResponse.ok)
        compare(asyncResponse.value.done, true)
    }

    function test_module_event_subscription_normalizes_payload() {
        client.host = eventHost

        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        compare(eventHost.subscribedModule, "storage_module")
        compare(eventHost.subscribedEvent, "storageUploadDone")

        eventHost.subscribedCallback({ cid: "cid-1" })

        compare(receivedModule, "storage_module")
        compare(receivedEvent, "storageUploadDone")
        compare(receivedArgs.cid, "cid-1")
    }
}
