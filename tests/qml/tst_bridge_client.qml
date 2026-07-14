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
        property string asyncBridgeSchemaResponse: ""
        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []
        property string directResponseJson: "\"direct\""

        function reset() {
            callCount = 0
            asyncBridgeSchemaResponse = ""
            lastModule = ""
            lastMethod = ""
            lastArgs = []
            directResponseJson = "\"direct\""
        }

        function callModule(moduleName, method, args) {
            if (String(moduleName || "") === "logos_inspector"
                    && String(method || "") === "asyncBridgeSchema") {
                return JSON.stringify(asyncBridgeSchemaResponse)
            }
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

        property string asyncBridgeSchemaResponse: "logos-inspector-async-bridge/v1"
        property string schemaProbeMode: "normal"
        property int syncCallCount: 0
        property int asyncCallCount: 0
        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []
        property var pendingCallback: null
        property var pendingAsyncCalls: []
        property var lastCompletedCallback: null
        property bool throwOnAsync: false
        property bool subscriptionResult: true
        property int subscriptionCallCount: 0
        property string subscribedModule: ""
        property string subscribedEvent: ""

        function reset() {
            asyncBridgeSchemaResponse = "logos-inspector-async-bridge/v1"
            schemaProbeMode = "normal"
            syncCallCount = 0
            asyncCallCount = 0
            lastModule = ""
            lastMethod = ""
            lastArgs = []
            pendingCallback = null
            pendingAsyncCalls = []
            lastCompletedCallback = null
            throwOnAsync = false
            subscriptionResult = true
            subscriptionCallCount = 0
            subscribedModule = ""
            subscribedEvent = ""
        }

        function callModule(moduleName, method, args) {
            if (String(moduleName || "") === "logos_inspector"
                    && String(method || "") === "asyncBridgeSchema") {
                if (schemaProbeMode === "throw") {
                    throw new Error("schema probe transport failed")
                }
                if (schemaProbeMode === "malformed") {
                    return "{"
                }
                if (schemaProbeMode === "missing") {
                    return JSON.stringify({ error: "Invalid response" })
                }
                if (schemaProbeMode === "unavailable") {
                    return JSON.stringify({ error: "Module not connected" })
                }
                return JSON.stringify(asyncBridgeSchemaResponse)
            }
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
            const calls = pendingAsyncCalls.slice()
            calls.push({
                moduleName: lastModule,
                method: lastMethod,
                args: lastArgs,
                callback: callback
            })
            pendingAsyncCalls = calls
        }

        function complete(responseJson) {
            return completeNext("", responseJson)
        }

        function completeNext(method, responseJson) {
            const calls = pendingAsyncCalls.slice()
            for (let i = 0; i < calls.length; ++i) {
                if (!method.length || calls[i].method === method) {
                    const call = calls.splice(i, 1)[0]
                    pendingAsyncCalls = calls
                    lastCompletedCallback = call.callback
                    pendingCallback = calls.length ? calls[calls.length - 1].callback : null
                    call.callback(responseJson)
                    return true
                }
            }
            return false
        }

        function completeForToken(method, token, responseJson) {
            const calls = pendingAsyncCalls.slice()
            for (let i = 0; i < calls.length; ++i) {
                if (calls[i].method === method
                        && String(calls[i].args[0] || "") === String(token || "")) {
                    const call = calls.splice(i, 1)[0]
                    pendingAsyncCalls = calls
                    lastCompletedCallback = call.callback
                    pendingCallback = calls.length ? calls[calls.length - 1].callback : null
                    call.callback(responseJson)
                    return true
                }
            }
            return false
        }

        function replayLast(responseJson) {
            if (!lastCompletedCallback) {
                return false
            }
            lastCompletedCallback(responseJson)
            return true
        }

        function pendingCount(method) {
            let count = 0
            for (let i = 0; i < pendingAsyncCalls.length; ++i) {
                if (!method.length || pendingAsyncCalls[i].method === method) {
                    count += 1
                }
            }
            return count
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
        client.basecampAsyncPollIntervalMs = 100000
        client.basecampAsyncTimeoutMs = 30000
        client.basecampAsyncStartAttemptTimeoutMs = 2000
        client.basecampAsyncMaxPollAttempts = 600
        client.basecampAsyncMaxPollsInFlight = 8
        client.basecampAsyncMaxPendingCalls = 128
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

    function inspectorControlResponse(value) {
        return JSON.stringify(JSON.stringify({
            ok: true,
            value: value,
            text: "",
            error: ""
        }))
    }

    function inspectorControlError(error) {
        return JSON.stringify(JSON.stringify({
            ok: false,
            value: null,
            text: "",
            error: error
        }))
    }

    function finalBridgeResponse(value) {
        return JSON.stringify({
            ok: true,
            value: value,
            text: "done",
            error: ""
        })
    }

    function pendingControlArgument(host, method, argumentIndex) {
        const calls = host && Array.isArray(host.pendingAsyncCalls)
            ? host.pendingAsyncCalls
            : []
        for (let i = 0; i < calls.length; ++i) {
            if (calls[i].method === method) {
                return String(calls[i].args[argumentIndex] || "")
            }
        }
        return ""
    }

    function completeBasecampStart(host, token) {
        return host.completeNext("callAsync", inspectorControlResponse({
            schema: client.basecampAsyncBridgeSchema,
            correlationId: pendingControlArgument(host, "callAsync", 0),
            token: token
        }))
    }

    function completeBasecampPoll(host, status, responseJson) {
        const value = {
            schema: client.basecampAsyncBridgeSchema,
            token: pendingControlArgument(host, "pollAsync", 0),
            status: status
        }
        if (responseJson !== undefined) {
            value.responseJson = responseJson
        }
        return host.completeNext("pollAsync", inspectorControlResponse(value))
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

    function test_basecamp_async_host_polls_inspector_call_and_unwraps_nested_response() {
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
        compare(basecampAsyncHost.lastMethod, "callAsync")
        verify(String(basecampAsyncHost.lastArgs[0]).length >= 32)
        compare(basecampAsyncHost.lastArgs[1], "blockchainNode")
        compare(JSON.parse(basecampAsyncHost.lastArgs[2])[0], "a")

        verify(completeBasecampStart(basecampAsyncHost, "lai_00112233445566778899aabbccddeeff"))
        compare(basecampAsyncHost.lastMethod, "pollAsync")
        compare(basecampAsyncHost.lastArgs[0], "lai_00112233445566778899aabbccddeeff")
        verify(completeBasecampPoll(
            basecampAsyncHost,
            "ready",
            finalBridgeResponse({ done: true })
        ))
        basecampAsyncHost.replayLast(inspectorControlResponse({
            schema: client.basecampAsyncBridgeSchema,
            token: "lai_00112233445566778899aabbccddeeff",
            status: "ready",
            responseJson: finalBridgeResponse({ stale: true })
        }))

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

    function test_unadvertised_basecamp_host_keeps_legacy_async_inspector_route() {
        client.host = basecampAsyncHost
        basecampAsyncHost.asyncBridgeSchemaResponse = ""

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        compare(basecampAsyncHost.lastModule, "logos_inspector")
        compare(basecampAsyncHost.lastMethod, "call")
        compare(basecampAsyncHost.lastArgs[0], "sourcePolicy")
        basecampAsyncHost.complete(inspectorControlResponse({ legacy: true }))
        verify(asyncResponse.ok)
        compare(asyncResponse.value.legacy, true)
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
        basecampAsyncHost.completeNext("callAsync", JSON.stringify({
            error: "disconnected",
            module: "logos_inspector",
            method: "callAsync"
        }))

        verify(!asyncResponse.ok)
        compare(asyncResponse.value, null)
        compare(asyncResponse.error, "Logos bridge call failed: disconnected")
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

    function test_basecamp_poll_keeps_one_request_in_flight_and_retries_pending() {
        client.host = basecampAsyncHost
        const requestId = client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        verify(completeBasecampStart(basecampAsyncHost, "lai_11112222333344445555666677778888"))
        compare(basecampAsyncHost.pendingCount("pollAsync"), 1)
        verify(completeBasecampPoll(basecampAsyncHost, "pending"))
        compare(client.pendingCalls[requestId].pollInFlight, false)

        client.pendingCalls[requestId].nextPollAtMs = 0
        client.pollBasecampCalls()
        client.pollBasecampCalls()
        compare(basecampAsyncHost.pendingCount("pollAsync"), 1)

        verify(completeBasecampPoll(
            basecampAsyncHost,
            "ready",
            finalBridgeResponse({ revision: 2 })
        ))
        compare(asyncCallbackCount, 1)
        verify(asyncResponse.ok)
        compare(asyncResponse.value.revision, 2)
    }

    function test_basecamp_poll_caps_immediate_start_fan_out() {
        client.host = basecampAsyncHost
        client.basecampAsyncMaxPollsInFlight = 3
        const requestCount = 8

        for (let i = 0; i < requestCount; ++i) {
            client.callModuleAsync("logos_inspector", "sourcePolicy", [i], function () {})
        }
        for (let i = 0; i < requestCount; ++i) {
            verify(completeBasecampStart(
                basecampAsyncHost,
                "lai_" + String(i).padStart(32, "0")
            ))
            verify(basecampAsyncHost.pendingCount("pollAsync") <= 3)
        }

        compare(basecampAsyncHost.pendingCount("pollAsync"), 3)
        compare(client.basecampPollsInFlight(), 3)
    }

    function test_basecamp_poll_waits_for_final_allowed_attempt() {
        client.host = basecampAsyncHost
        client.basecampAsyncMaxPollAttempts = 1
        const requestId = client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        verify(completeBasecampStart(basecampAsyncHost, "lai_12121212121212121212121212121212"))
        compare(basecampAsyncHost.pendingCount("pollAsync"), 1)
        compare(client.pendingCalls[requestId].pollAttempts, 1)

        client.pollBasecampCalls()

        compare(asyncCallbackCount, 0)
        verify(client.pendingCalls[requestId].pollInFlight)
        verify(completeBasecampPoll(
            basecampAsyncHost,
            "ready",
            finalBridgeResponse({ finalAttempt: true })
        ))
        compare(asyncCallbackCount, 1)
        verify(asyncResponse.ok)
        compare(asyncResponse.value.finalAttempt, true)
    }

    function test_basecamp_poll_preserves_successful_null() {
        client.host = basecampAsyncHost
        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        verify(completeBasecampStart(basecampAsyncHost, "lai_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"))
        verify(completeBasecampPoll(basecampAsyncHost, "ready", finalBridgeResponse(null)))

        verify(asyncResponse.ok)
        compare(asyncResponse.value, null)
    }

    function test_basecamp_poll_preserves_terminal_bridge_error() {
        client.host = basecampAsyncHost
        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        verify(completeBasecampStart(basecampAsyncHost, "lai_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"))
        verify(completeBasecampPoll(basecampAsyncHost, "ready", JSON.stringify({
            ok: false,
            value: null,
            text: "",
            error: "backend failed"
        })))

        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "backend failed")
    }

    function test_basecamp_start_schema_mismatch_reports_acceptance_unknown() {
        client.host = basecampAsyncHost
        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        basecampAsyncHost.completeNext("callAsync", inspectorControlResponse({
            schema: "unknown/v9"
        }))

        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: async_acceptance_unknown")
    }

    function test_basecamp_start_rejects_crossed_correlation_identity() {
        client.host = basecampAsyncHost
        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        basecampAsyncHost.completeNext("callAsync", inspectorControlResponse({
            schema: client.basecampAsyncBridgeSchema,
            correlationId: "wrong-correlation",
            token: "lai_99999999999999999999999999999999"
        }))

        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: async_acceptance_unknown")
        compare(basecampAsyncHost.pendingCount("cancelAsync"), 0)
        compare(basecampAsyncHost.pendingCount("releaseAsync"), 0)
    }

    function test_basecamp_poll_rejects_crossed_token_identity() {
        client.host = basecampAsyncHost
        const token = "lai_88888888888888888888888888888888"
        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })
        verify(completeBasecampStart(basecampAsyncHost, token))

        verify(basecampAsyncHost.completeNext("pollAsync", inspectorControlResponse({
            schema: client.basecampAsyncBridgeSchema,
            token: "lai_77777777777777777777777777777777",
            status: "ready",
            responseJson: finalBridgeResponse({ crossed: true })
        })))

        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: incompatible async poll schema")
        compare(basecampAsyncHost.pendingCount("cancelAsync"), 1)
        compare(basecampAsyncHost.pendingCount("releaseAsync"), 1)
    }

    function test_old_basecamp_core_reports_required_async_schema() {
        client.host = basecampAsyncHost
        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        basecampAsyncHost.completeNext("callAsync", JSON.stringify({ error: "Invalid response" }))

        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: Basecamp inspector async bridge v1 required")
    }

    function test_basecamp_poll_timeout_cancels_and_releases_once() {
        client.host = basecampAsyncHost
        const requestId = client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        verify(completeBasecampStart(basecampAsyncHost, "lai_cccccccccccccccccccccccccccccccc"))
        client.pendingCalls[requestId].deadlineMs = 0
        client.pollBasecampCalls()

        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: async_response_timeout")
        compare(basecampAsyncHost.pendingCount("cancelAsync"), 1)
        compare(basecampAsyncHost.pendingCount("releaseAsync"), 1)
    }

    function test_basecamp_poll_transport_timeout_retries_same_token() {
        client.host = basecampAsyncHost
        const token = "lai_eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
        const requestId = client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        verify(completeBasecampStart(basecampAsyncHost, token))
        verify(basecampAsyncHost.completeNext("pollAsync", JSON.stringify({
            error: "timeout",
            module: "logos_inspector",
            method: "pollAsync"
        })))
        compare(asyncCallbackCount, 0)
        client.pendingCalls[requestId].nextPollAtMs = 0
        client.pollBasecampCalls()
        compare(basecampAsyncHost.lastArgs[0], token)
        verify(completeBasecampPoll(
            basecampAsyncHost,
            "ready",
            finalBridgeResponse({ retried: true })
        ))
        compare(asyncCallbackCount, 1)
        compare(asyncResponse.value.retried, true)
    }

    function test_basecamp_poll_completes_independent_calls_in_reverse_order() {
        client.host = basecampAsyncHost
        const completed = []
        client.callModuleAsync("logos_inspector", "first", [], function (response) {
            completed.push("first:" + String(response.value.order))
        })
        client.callModuleAsync("logos_inspector", "second", [], function (response) {
            completed.push("second:" + String(response.value.order))
        })

        const firstToken = "lai_10101010101010101010101010101010"
        const secondToken = "lai_20202020202020202020202020202020"
        verify(completeBasecampStart(basecampAsyncHost, firstToken))
        verify(completeBasecampStart(basecampAsyncHost, secondToken))
        verify(basecampAsyncHost.completeForToken(
            "pollAsync",
            secondToken,
            inspectorControlResponse({
                schema: client.basecampAsyncBridgeSchema,
                token: secondToken,
                status: "ready",
                responseJson: finalBridgeResponse({ order: 2 })
            })
        ))
        verify(basecampAsyncHost.completeForToken(
            "pollAsync",
            firstToken,
            inspectorControlResponse({
                schema: client.basecampAsyncBridgeSchema,
                token: firstToken,
                status: "ready",
                responseJson: finalBridgeResponse({ order: 1 })
            })
        ))

        compare(completed.length, 2)
        compare(completed[0], "second:2")
        compare(completed[1], "first:1")
    }

    function test_basecamp_start_timeout_retries_same_correlation_and_recovers_token() {
        client.host = basecampAsyncHost
        const args = [{ nested: { revision: 1 } }]
        const requestId = client.callModuleAsync("logos_inspector", "sourcePolicy", args, function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })
        const correlationId = String(basecampAsyncHost.lastArgs[0])
        const frozenArgsJson = String(basecampAsyncHost.lastArgs[2])
        args[0].nested.revision = 2

        verify(basecampAsyncHost.completeNext("callAsync", JSON.stringify({
            error: "timeout",
            module: "logos_inspector",
            method: "callAsync"
        })))
        compare(asyncCallbackCount, 0)
        client.pendingCalls[requestId].nextStartAtMs = 0
        client.pollBasecampCalls()
        compare(basecampAsyncHost.pendingCount("callAsync"), 1)
        compare(String(basecampAsyncHost.lastArgs[0]), correlationId)
        compare(String(basecampAsyncHost.lastArgs[2]), frozenArgsJson)
        compare(JSON.parse(String(basecampAsyncHost.lastArgs[2]))[0].nested.revision, 1)

        verify(completeBasecampStart(basecampAsyncHost, "lai_ffffffffffffffffffffffffffffffff"))
        verify(completeBasecampPoll(
            basecampAsyncHost,
            "ready",
            finalBridgeResponse({ recovered: true })
        ))
        compare(asyncCallbackCount, 1)
        verify(asyncResponse.ok)
        compare(asyncResponse.value.recovered, true)
    }

    function test_basecamp_start_deadline_reports_unknown_acceptance() {
        client.host = basecampAsyncHost
        const requestId = client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncCallbackCount += 1
            asyncResponse = response
        })

        client.pendingCalls[requestId].deadlineMs = 0
        client.pollBasecampCalls()

        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        compare(asyncResponse.error, "Logos bridge call failed: async_acceptance_unknown")
    }

    function test_sync_only_basecamp_host_keeps_deferred_legacy_fallback() {
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
        compare(asyncResponse.value.method, "blockchainNode")
        compare(asyncResponse.value.args[0], "legacy")
    }

    function test_advertised_schema_without_async_transport_fails_closed() {
        client.host = basecampHost
        basecampHost.asyncBridgeSchemaResponse = client.basecampAsyncBridgeSchema

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        tryVerify(function () { return asyncResponse !== null })
        compare(basecampHost.callCount, 0)
        verify(!asyncResponse.ok)
        verify(asyncResponse.error.indexOf("async bridge v1 required") >= 0)
    }

    function test_incompatible_reported_schema_does_not_enter_legacy_route() {
        client.host = basecampAsyncHost
        basecampAsyncHost.asyncBridgeSchemaResponse = "logos-inspector-async-bridge/v2"

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        tryVerify(function () { return asyncResponse !== null })
        compare(basecampAsyncHost.pendingCount("call"), 0)
        compare(basecampAsyncHost.pendingCount("callAsync"), 0)
        verify(!asyncResponse.ok)
        verify(asyncResponse.error.indexOf("async bridge v1 required") >= 0)
    }

    function test_schema_probe_throw_fails_closed() {
        client.host = basecampAsyncHost
        basecampAsyncHost.schemaProbeMode = "throw"

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        tryVerify(function () { return asyncResponse !== null })
        compare(basecampAsyncHost.asyncCallCount, 0)
        verify(!asyncResponse.ok)
        verify(asyncResponse.error.indexOf("async bridge probe failed") >= 0)
    }

    function test_malformed_schema_probe_fails_closed() {
        client.host = basecampAsyncHost
        basecampAsyncHost.schemaProbeMode = "malformed"

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        tryVerify(function () { return asyncResponse !== null })
        compare(basecampAsyncHost.asyncCallCount, 0)
        verify(!asyncResponse.ok)
        verify(asyncResponse.error.indexOf("async bridge probe failed") >= 0)
    }

    function test_unavailable_schema_probe_fails_closed() {
        client.host = basecampAsyncHost
        basecampAsyncHost.schemaProbeMode = "unavailable"

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        tryVerify(function () { return asyncResponse !== null })
        compare(basecampAsyncHost.asyncCallCount, 0)
        verify(!asyncResponse.ok)
        verify(asyncResponse.error.indexOf("async bridge probe failed") >= 0)
    }

    function test_known_missing_schema_method_keeps_legacy_route() {
        client.host = basecampAsyncHost
        basecampAsyncHost.schemaProbeMode = "missing"

        client.callModuleAsync("logos_inspector", "sourcePolicy", [], function (response) {
            asyncResponse = response
        })

        compare(basecampAsyncHost.pendingCount("call"), 1)
        verify(basecampAsyncHost.completeNext("call", finalBridgeResponse({ legacy: true })))
        verify(asyncResponse.ok)
        compare(asyncResponse.value.legacy, true)
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

        verify(completeBasecampStart(basecampAsyncHost, "lai_dddddddddddddddddddddddddddddddd"))
        compare(asyncCallbackCount, 1)
        verify(!asyncResponse.ok)
        compare(basecampAsyncHost.pendingCount("cancelAsync"), 1)
        compare(basecampAsyncHost.pendingCount("releaseAsync"), 1)
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

    function test_host_replacement_cancels_queued_async_unavailable_result() {
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
