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
        property string directResponseJson: "\"direct\""

        function reset() {
            callCount = 0
            lastModule = ""
            lastMethod = ""
            lastArgs = []
            directResponseJson = "\"direct\""
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
            return directResponseJson
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
        id: jsonHostWithUnrelatedAsync

        property int jsonCallCount: 0
        property int unrelatedAsyncCallCount: 0

        function reset() {
            jsonCallCount = 0
            unrelatedAsyncCallCount = 0
        }

        function callModuleJson(moduleName, method, argsJson) {
            jsonCallCount += 1
            return JSON.stringify({
                ok: true,
                value: { method: method },
                text: "json",
                error: ""
            })
        }

        function callModuleAsync(moduleName, method, args, callback) {
            unrelatedAsyncCallCount += 1
        }
    }

    component BasecampAsyncHost: QtObject {
        signal moduleEventReceived(string moduleName, string eventName, var args)

        property int syncCallCount: 0
        property int asyncCallCount: 0
        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []
        property var pendingCallback: null
        property bool throwOnAsync: false
        property bool subscriptionResult: true
        property int subscriptionCallCount: 0
        property string subscribedModule: ""
        property string subscribedEvent: ""

        function reset() {
            syncCallCount = 0
            asyncCallCount = 0
            lastModule = ""
            lastMethod = ""
            lastArgs = []
            pendingCallback = null
            throwOnAsync = false
            subscriptionResult = true
            subscriptionCallCount = 0
            subscribedModule = ""
            subscribedEvent = ""
        }

        function callModule(moduleName, method, args) {
            syncCallCount += 1
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = args || []
            return JSON.stringify(JSON.stringify({
                ok: true,
                value: { transport: "sync" },
                text: "sync",
                error: ""
            }))
        }

        function callModuleAsync(moduleName, method, args, callback, timeoutMs) {
            if (throwOnAsync) {
                throw new Error("Basecamp async failure")
            }
            asyncCallCount += 1
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = args || []
            pendingCallback = callback
        }

        function complete(responseJson) {
            if (pendingCallback) {
                pendingCallback(responseJson)
            }
        }

        function onModuleEvent(moduleName, eventName) {
            subscriptionCallCount += 1
            subscribedModule = String(moduleName || "")
            subscribedEvent = String(eventName || "")
            return subscriptionResult
        }
    }

    BasecampAsyncHost {
        id: basecampAsyncHost
    }

    BasecampAsyncHost {
        id: replacementBasecampAsyncHost
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

        onCallbackFailed: function (error) {
            testRoot.callbackFailureCount += 1
            testRoot.callbackFailure = error
        }
    }

    property string receivedModule: ""
    property string receivedEvent: ""
    property var receivedArgs: null
    property var asyncResponse: null
    property int asyncCallbackCount: 0
    property bool legacyQueueDrained: false
    property int callbackFailureCount: 0
    property string callbackFailure: ""

    function init() {
        client.host = standaloneHost
        standaloneHost.reset()
        standaloneHost.defaultResponse = ({
            ok: true,
            value: { answer: 42 },
            text: "OK",
            error: ""
        })
        basecampHost.reset()
        asyncHost.reset()
        jsonHostWithUnrelatedAsync.reset()
        basecampAsyncHost.reset()
        replacementBasecampAsyncHost.reset()
        eventHost.reset()
        client.nextRequestId = 1
        client.pendingCalls = ({})
        client.moduleEventSubscriptions = ({})
        client.moduleEventRegistrations = []
        receivedModule = ""
        receivedEvent = ""
        receivedArgs = null
        asyncResponse = null
        asyncCallbackCount = 0
        legacyQueueDrained = false
        callbackFailureCount = 0
        callbackFailure = ""
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
        compare(Object.keys(client.pendingCalls).length, 0)
    }

    function test_basecamp_async_host_wraps_inspector_call_and_unwraps_nested_response() {
        client.host = basecampAsyncHost

        const requestId = client.callModuleAsync("logos_inspector", "blockchainNode", ["a"], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        compare(requestId, 1)
        verify(client.hasAsyncCalls())
        compare(basecampAsyncHost.asyncCallCount, 1)
        compare(basecampAsyncHost.syncCallCount, 0)
        compare(basecampAsyncHost.lastModule, "logos_inspector")
        compare(basecampAsyncHost.lastMethod, "call")
        compare(basecampAsyncHost.lastArgs[0], "blockchainNode")
        compare(JSON.parse(basecampAsyncHost.lastArgs[1])[0], "a")

        const responseJson = JSON.stringify(JSON.stringify({
            ok: true,
            value: { done: true },
            text: "done",
            error: ""
        }))
        basecampAsyncHost.complete(responseJson)
        basecampAsyncHost.complete(responseJson)

        compare(asyncCallbackCount, 1)
        verify(asyncResponse.ok)
        compare(asyncResponse.value.done, true)
        compare(Object.keys(client.pendingCalls).length, 0)
    }

    function test_basecamp_async_host_keeps_direct_routes_direct() {
        client.host = basecampAsyncHost

        client.callModuleAsync("logos_inspector", "moduleVersion", [], function (response) {
            asyncResponse = response
        })

        compare(basecampAsyncHost.lastModule, "logos_inspector")
        compare(basecampAsyncHost.lastMethod, "moduleVersion")
        compare(basecampAsyncHost.lastArgs.length, 0)
        basecampAsyncHost.complete(JSON.stringify("1.2.3"))
        verify(asyncResponse.ok)
        compare(asyncResponse.value, "1.2.3")

        asyncResponse = null
        client.callModuleAsync("storage_module", "space", ["rest"], function (response) {
            asyncResponse = response
        })

        compare(basecampAsyncHost.lastModule, "storage_module")
        compare(basecampAsyncHost.lastMethod, "space")
        compare(basecampAsyncHost.lastArgs[0], "rest")
        basecampAsyncHost.complete(JSON.stringify({ available: 12 }))
        verify(asyncResponse.ok)
        compare(asyncResponse.value.available, 12)
    }

    function test_basecamp_direct_values_do_not_collide_with_inspector_envelope() {
        client.host = basecampAsyncHost

        client.callModuleAsync("storage_module", "domainResult", [], function (response) {
            asyncResponse = response
        })
        basecampAsyncHost.complete(JSON.stringify({
            ok: false,
            error: "domain value",
            payload: 7
        }))

        verify(asyncResponse.ok)
        compare(asyncResponse.value.ok, false)
        compare(asyncResponse.value.error, "domain value")
        compare(asyncResponse.value.payload, 7)

        client.host = basecampHost
        basecampHost.directResponseJson = JSON.stringify({
            ok: false,
            error: "domain value",
            payload: 8
        })
        const syncResponse = client.callModule("storage_module", "domainResult", [])

        verify(syncResponse.ok)
        compare(syncResponse.value.ok, false)
        compare(syncResponse.value.error, "domain value")
        compare(syncResponse.value.payload, 8)
    }

    function test_basecamp_module_version_normalizes_reserved_host_error() {
        client.host = basecampAsyncHost

        client.callModuleAsync("logos_inspector", "moduleVersion", [], function (response) {
            asyncResponse = response
        })
        basecampAsyncHost.complete(JSON.stringify({ error: "Module not connected" }))

        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: Module not connected")
    }

    function test_basecamp_direct_call_normalizes_reserved_host_errors() {
        client.host = basecampAsyncHost

        client.callModuleAsync("storage_module", "space", [], function (response) {
            asyncResponse = response
        })
        basecampAsyncHost.complete(JSON.stringify({
            error: "timeout",
            module: "storage_module",
            method: "space"
        }))

        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: timeout")

        client.host = basecampHost
        basecampHost.directResponseJson = "not-json"
        const malformed = client.callModule("storage_module", "space", [])
        verify(!malformed.ok)
        verify(malformed.error.indexOf("invalid response JSON") >= 0)
    }

    function test_basecamp_async_host_normalizes_transport_error() {
        client.host = basecampAsyncHost

        client.callModuleAsync("logos_inspector", "blockchainNode", [], function (response) {
            asyncResponse = response
        })
        basecampAsyncHost.complete(JSON.stringify({
            error: "timeout",
            module: "logos_inspector",
            method: "call"
        }))

        verify(!asyncResponse.ok)
        compare(asyncResponse.value, null)
        compare(asyncResponse.error, "Logos bridge call failed: timeout")
    }

    function test_basecamp_async_host_converts_synchronous_throw_once() {
        client.host = basecampAsyncHost
        basecampAsyncHost.throwOnAsync = true

        client.callModuleAsync("logos_inspector", "blockchainNode", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        verify(asyncResponse.error.indexOf("Basecamp async failure") >= 0)
        compare(Object.keys(client.pendingCalls).length, 0)
    }

    function test_sync_only_basecamp_host_is_deferred_legacy_fallback() {
        client.host = basecampHost

        client.callModuleAsync("logos_inspector", "blockchainNode", ["legacy"], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        verify(!client.hasAsyncCalls())
        compare(basecampHost.callCount, 0)
        tryCompare(testRoot, "asyncCallbackCount", 1)
        compare(basecampHost.callCount, 1)
        verify(asyncResponse.ok)
    }

    function test_json_host_does_not_use_unrelated_callback_async_method() {
        client.host = jsonHostWithUnrelatedAsync

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        verify(!client.hasAsyncCalls())
        tryVerify(function () { return asyncResponse !== null })
        compare(jsonHostWithUnrelatedAsync.jsonCallCount, 1)
        compare(jsonHostWithUnrelatedAsync.unrelatedAsyncCallCount, 0)
        verify(asyncResponse.ok)
    }

    function test_host_replacement_fails_pending_and_ignores_late_callback() {
        client.host = basecampAsyncHost
        client.callModuleAsync("logos_inspector", "blockchainNode", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        compare(Object.keys(client.pendingCalls).length, 1)
        client.host = replacementBasecampAsyncHost

        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: host_changed")
        compare(Object.keys(client.pendingCalls).length, 0)

        basecampAsyncHost.complete(JSON.stringify(JSON.stringify({
            ok: true,
            value: { stale: true },
            text: "stale",
            error: ""
        })))
        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
    }

    function test_host_replacement_drains_all_callbacks_when_one_throws() {
        client.host = basecampAsyncHost
        let firstCount = 0
        let secondCount = 0
        client.callModuleAsync("logos_inspector", "first", [], function () {
            firstCount += 1
            throw new Error("first callback failed")
        })
        client.callModuleAsync("logos_inspector", "second", [], function () {
            secondCount += 1
        })

        client.host = replacementBasecampAsyncHost

        compare(firstCount, 1)
        compare(secondCount, 1)
        compare(callbackFailureCount, 1)
        compare(callbackFailure, "first callback failed")
        compare(Object.keys(client.pendingCalls).length, 0)
    }

    function test_host_replacement_cancels_queued_legacy_fallback() {
        client.host = basecampHost
        client.callModuleAsync("logos_inspector", "blockchainNode", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        client.host = replacementBasecampAsyncHost
        compare(asyncCallbackCount, 1)
        compare(asyncResponse.error, "Logos bridge call failed: host_changed")
        Qt.callLater(function () {
            legacyQueueDrained = true
        })
        tryCompare(testRoot, "legacyQueueDrained", true)
        compare(basecampHost.callCount, 0)
        compare(replacementBasecampAsyncHost.syncCallCount, 0)
    }

    function test_basecamp_module_event_uses_boolean_subscription_and_received_signal() {
        client.host = basecampAsyncHost

        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        compare(basecampAsyncHost.subscriptionCallCount, 1)
        compare(basecampAsyncHost.subscribedModule, "storage_module")
        compare(basecampAsyncHost.subscribedEvent, "storageUploadDone")

        basecampAsyncHost.moduleEventReceived("storage_module", "storageUploadDone", [{ cid: "cid-1" }])

        compare(receivedModule, "storage_module")
        compare(receivedEvent, "storageUploadDone")
        compare(receivedArgs[0].cid, "cid-1")
    }

    function test_failed_basecamp_module_event_subscription_can_retry() {
        client.host = basecampAsyncHost
        basecampAsyncHost.subscriptionResult = false

        verify(!client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        compare(basecampAsyncHost.subscriptionCallCount, 1)

        basecampAsyncHost.subscriptionResult = true
        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        compare(basecampAsyncHost.subscriptionCallCount, 2)
    }

    function test_module_event_registration_is_not_duplicated_after_host_return() {
        client.host = basecampAsyncHost
        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        compare(basecampAsyncHost.subscriptionCallCount, 1)

        client.host = replacementBasecampAsyncHost
        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        compare(replacementBasecampAsyncHost.subscriptionCallCount, 1)

        client.host = basecampAsyncHost
        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        compare(basecampAsyncHost.subscriptionCallCount, 1)
    }

    function test_compatibility_module_event_ignores_old_host_callback() {
        client.host = eventHost
        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        const staleCallback = eventHost.subscribedCallback

        client.host = basecampAsyncHost
        client.host = eventHost
        verify(client.subscribeModuleEvent("storage_module", "storageUploadDone"))
        const currentCallback = eventHost.subscribedCallback
        staleCallback({ cid: "stale" })

        compare(receivedModule, "")
        compare(receivedEvent, "")
        compare(receivedArgs, null)

        currentCallback({ cid: "current" })
        compare(receivedModule, "storage_module")
        compare(receivedEvent, "storageUploadDone")
        compare(receivedArgs.cid, "current")
    }

    function test_compatibility_module_event_subscription_normalizes_payload() {
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
