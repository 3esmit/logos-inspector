import QtQuick
import "BridgeEnvelope.js" as BridgeEnvelope

QtObject {
    id: root

    property var host: null
    property int hostEpoch: 0
    property int nextRequestId: 1
    property var pendingCalls: ({})
    property var moduleEventSubscriptions: ({})
    property var moduleEventRegistrations: []
    readonly property string basecampAsyncBridgeSchema: "logos-inspector-async-bridge/v1"
    property int basecampAsyncPollIntervalMs: 50
    property int basecampAsyncTimeoutMs: 30000
    property int basecampAsyncStartAttemptTimeoutMs: 2000
    property int basecampAsyncMaxPollAttempts: 600
    property int basecampAsyncMaxPollsInFlight: 8
    property int basecampAsyncMaxPendingCalls: 128

    onHostChanged: {
        hostEpoch += 1
        moduleEventSubscriptions = ({})
        failPendingCalls({
            ok: false,
            value: null,
            text: "",
            error: "Logos bridge call failed: host_changed"
        })
    }

    signal moduleEventReceived(string moduleName, string eventName, var args)
    signal callbackFailed(string error)

    function prefersBasecampModules() {
        return BridgeEnvelope.prefersBasecampModules(root.host)
    }

    function backendOwnsRuntimeModuleEvents() {
        return BridgeEnvelope.basecampInspectorOwnsRuntimeModuleEvents(root.host)
    }

    function callModule(moduleName, method, args) {
        return BridgeEnvelope.callModule(root.host, moduleName, method, args || [])
    }

    function hasAsyncCalls() {
        return BridgeEnvelope.hasAsyncCalls(root.host)
    }

    function callModuleAsync(moduleName, method, args, callback) {
        const requestId = root.nextRequestId
        const requestHost = root.host
        const requestHostEpoch = root.hostEpoch
        root.nextRequestId += 1
        if (Object.keys(root.pendingCalls || {}).length >= root.basecampAsyncMaxPendingCalls) {
            Qt.callLater(function () {
                if (typeof callback === "function") {
                    callback({
                        ok: false,
                        value: null,
                        text: "",
                        error: "Logos bridge call failed: async_client_capacity"
                    })
                }
            })
            return requestId
        }
        const inspectorAsyncRequest = root.prefersBasecampModules()
            && moduleName === "logos_inspector"
            && method !== "moduleVersion"
        const basecampSchemaProbe = inspectorAsyncRequest
            ? BridgeEnvelope.probeBasecampInspectorAsyncBridge(requestHost)
            : {
                status: "absent",
                schema: "",
                error: ""
            }
        const reportedBasecampSchema = String(basecampSchemaProbe.schema || "")
        const basecampPolling = BridgeEnvelope.usesBasecampInspectorPolling(
            requestHost,
            moduleName,
            method,
            root.basecampAsyncBridgeSchema,
            reportedBasecampSchema
        )
        let startArgsJson = ""
        if (basecampPolling) {
            try {
                startArgsJson = JSON.stringify(Array.isArray(args) ? args : [])
            } catch (error) {
                Qt.callLater(function () {
                    if (typeof callback === "function") {
                        callback({
                            ok: false,
                            value: null,
                            text: "",
                            error: "Logos bridge call failed: async_arguments_invalid"
                        })
                    }
                })
                return requestId
            }
        }
        const pending = root.copyPendingCalls()
        pending[requestId] = {
            callback: typeof callback === "function" ? callback : null,
            host: requestHost,
            hostEpoch: requestHostEpoch,
            route: basecampPolling ? "basecamp_poll" : "direct",
            phase: basecampPolling ? "starting" : "dispatched",
            correlationId: basecampPolling ? root.newBasecampCorrelationId(requestId) : "",
            startMethod: basecampPolling ? String(method || "") : "",
            startArgsJson: startArgsJson,
            startInFlight: false,
            startAttempts: 0,
            nextStartAtMs: 0,
            backendToken: "",
            pollInFlight: false,
            pollAttempts: 0,
            nextPollAtMs: 0,
            deadlineMs: basecampPolling ? Date.now() + root.basecampAsyncTimeoutMs : 0
        }
        root.pendingCalls = pending

        if (basecampPolling) {
            root.beginBasecampCall(requestId)
            return requestId
        }

        if (inspectorAsyncRequest && basecampSchemaProbe.status === "probe_failed") {
            Qt.callLater(function () {
                root.finishAsyncCall(requestId, {
                    ok: false,
                    value: null,
                    text: "",
                    error: "Logos bridge call failed: Basecamp inspector async bridge probe failed: "
                        + String(basecampSchemaProbe.error || "unknown capability error")
                }, requestHostEpoch)
            })
            return requestId
        }

        if (inspectorAsyncRequest && !basecampPolling) {
            Qt.callLater(function () {
                root.finishAsyncCall(requestId, {
                    ok: false,
                    value: null,
                    text: "",
                    error: "Logos bridge call failed: Basecamp inspector async bridge v1 required"
                }, requestHostEpoch)
            })
            return requestId
        }

        if (BridgeEnvelope.dispatchAsync(requestHost, requestId, moduleName, method, args || [], function (response) {
                root.finishAsyncCall(requestId, response, requestHostEpoch)
            })) {
            return requestId
        }

        Qt.callLater(function () {
            if (requestHostEpoch !== root.hostEpoch) {
                return
            }
            root.finishAsyncCall(
                requestId,
                BridgeEnvelope.callModule(requestHost, moduleName, method, args || []),
                requestHostEpoch
            )
        })
        return requestId
    }

    function newBasecampCorrelationId(requestId) {
        function hex32(value) {
            const text = (Number(value) >>> 0).toString(16)
            return "00000000".slice(text.length) + text
        }
        return "qml_"
            + hex32(root.hostEpoch)
            + hex32(requestId)
            + hex32(Date.now())
            + hex32(Math.floor(Math.random() * 0x100000000))
    }

    function beginBasecampCall(requestId) {
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || entry.route !== "basecamp_poll"
                || entry.phase !== "starting"
                || entry.startInFlight
                || entry.host !== root.host
                || entry.hostEpoch !== root.hostEpoch) {
            return false
        }
        const remainingMs = entry.deadlineMs - Date.now()
        if (remainingMs <= 0) {
            root.timeoutBasecampCall(requestId, entry)
            return false
        }
        entry.startInFlight = true
        entry.startAttempts += 1
        const requestHost = entry.host
        const requestHostEpoch = entry.hostEpoch
        const attemptTimeoutMs = Math.max(
            1,
            Math.min(root.basecampAsyncStartAttemptTimeoutMs, remainingMs)
        )
        root.pendingCalls = pending
        const dispatched = BridgeEnvelope.beginBasecampInspectorCall(
            requestHost,
            entry.correlationId,
            entry.startMethod,
            entry.startArgsJson,
            attemptTimeoutMs,
            function (response) {
                root.handleBasecampStart(
                    requestId,
                    requestHost,
                    requestHostEpoch,
                    entry.correlationId,
                    response
                )
            }
        )
        if (!dispatched) {
            root.finishAsyncCall(requestId, {
                ok: false,
                value: null,
                text: "",
                error: "Logos bridge call failed: Basecamp inspector async bridge unavailable"
            }, requestHostEpoch)
        }
        return dispatched
    }

    function handleBasecampStart(
            requestId,
            requestHost,
            requestHostEpoch,
            expectedCorrelationId,
            response) {
        const value = response && response.ok === true ? response.value : null
        const token = value && typeof value.token === "string" ? value.token : ""
        const schema = value ? String(value.schema || "") : ""
        const responseCorrelationId = value ? String(value.correlationId || "") : ""
        const correlationMatches = responseCorrelationId === expectedCorrelationId
        const current = root.pendingCalls || {}
        const entry = current[requestId]
        if (!entry
                || entry.host !== requestHost
                || entry.hostEpoch !== requestHostEpoch
                || root.host !== requestHost
                || root.hostEpoch !== requestHostEpoch) {
            if (token.length && correlationMatches) {
                root.abandonBasecampToken(requestHost, token)
            }
            return
        }
        if (entry.phase !== "starting") {
            return
        }
        if (!response || response.ok !== true) {
            if (BridgeEnvelope.retryableBasecampPollError(response)
                    && Date.now() < entry.deadlineMs) {
                const pending = root.copyPendingCalls()
                const pendingEntry = pending[requestId]
                if (pendingEntry && pendingEntry.phase === "starting") {
                    pendingEntry.startInFlight = false
                    pendingEntry.nextStartAtMs = Date.now()
                        + root.basecampPollBackoffMs(pendingEntry.startAttempts)
                    root.pendingCalls = pending
                }
                return
            }
            const error = BridgeEnvelope.missingBasecampAsyncBridge(response)
                ? {
                    ok: false,
                    value: null,
                    text: "",
                    error: "Logos bridge call failed: Basecamp inspector async bridge v1 required"
                }
                : response
            root.finishAsyncCall(requestId, error, requestHostEpoch)
            return
        }
        if (schema !== root.basecampAsyncBridgeSchema
                || !token.length
                || !correlationMatches
                || entry.correlationId !== expectedCorrelationId) {
            if (token.length && correlationMatches) {
                root.abandonBasecampToken(requestHost, token)
            }
            root.finishAsyncCall(requestId, {
                ok: false,
                value: null,
                text: "",
                error: "Logos bridge call failed: async_acceptance_unknown"
            }, requestHostEpoch)
            return
        }
        const pending = root.copyPendingCalls()
        const pendingEntry = pending[requestId]
        if (!pendingEntry || pendingEntry.hostEpoch !== requestHostEpoch) {
            root.abandonBasecampToken(requestHost, token)
            return
        }
        pendingEntry.phase = "polling"
        pendingEntry.backendToken = token
        pendingEntry.pollInFlight = false
        pendingEntry.nextPollAtMs = 0
        root.pendingCalls = pending
        root.pollBasecampCalls()
    }

    function pollBasecampCalls() {
        const pending = root.pendingCalls || {}
        const now = Date.now()
        let inFlight = root.basecampPollsInFlight()
        const requestIds = Object.keys(pending)
        for (let i = 0; i < requestIds.length; ++i) {
            const requestId = requestIds[i]
            const entry = (root.pendingCalls || {})[requestId]
            if (!entry || entry.route !== "basecamp_poll") {
                continue
            }
            if (now >= entry.deadlineMs) {
                root.timeoutBasecampCall(requestId, entry)
                continue
            }
            if (entry.phase === "starting") {
                if (!entry.startInFlight && now >= Number(entry.nextStartAtMs || 0)) {
                    root.beginBasecampCall(requestId)
                }
                continue
            }
            if (entry.phase !== "polling") {
                continue
            }
            if (entry.pollInFlight) {
                continue
            }
            if (entry.pollAttempts >= root.basecampAsyncMaxPollAttempts) {
                root.timeoutBasecampCall(requestId, entry)
                continue
            }
            if (now < Number(entry.nextPollAtMs || 0)
                    || inFlight >= root.basecampAsyncMaxPollsInFlight) {
                continue
            }
            root.pollBasecampCall(requestId)
            inFlight += 1
        }
    }

    function pollBasecampCall(requestId) {
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || entry.route !== "basecamp_poll"
                || entry.phase !== "polling"
                || entry.pollInFlight
                || !entry.backendToken.length
                || entry.host !== root.host
                || entry.hostEpoch !== root.hostEpoch) {
            return false
        }
        if (Date.now() >= entry.deadlineMs
                || entry.pollAttempts >= root.basecampAsyncMaxPollAttempts) {
            root.timeoutBasecampCall(requestId, entry)
            return false
        }
        entry.pollInFlight = true
        entry.pollAttempts += 1
        const token = entry.backendToken
        const requestHost = entry.host
        const requestHostEpoch = entry.hostEpoch
        root.pendingCalls = pending
        BridgeEnvelope.pollBasecampInspectorCall(
            requestHost,
            token,
            root.basecampAsyncTimeoutMs,
            function (response) {
                root.handleBasecampPoll(
                    requestId,
                    requestHost,
                    requestHostEpoch,
                    token,
                    response
                )
            }
        )
        return true
    }

    function handleBasecampPoll(requestId, requestHost, requestHostEpoch, token, response) {
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || entry.host !== requestHost
                || entry.hostEpoch !== requestHostEpoch
                || entry.backendToken !== token
                || root.host !== requestHost
                || root.hostEpoch !== requestHostEpoch) {
            return
        }
        if (!response || response.ok !== true) {
            if (BridgeEnvelope.retryableBasecampPollError(response)
                    && Date.now() < entry.deadlineMs) {
                entry.pollInFlight = false
                entry.nextPollAtMs = Date.now() + root.basecampPollBackoffMs(entry.pollAttempts)
                root.pendingCalls = pending
                return
            }
            root.abandonBasecampToken(requestHost, token)
            root.finishAsyncCall(requestId, response || {
                ok: false,
                value: null,
                text: "",
                error: "Logos bridge call failed: malformed async poll response"
            }, requestHostEpoch)
            return
        }
        const value = response.value
        const schema = value ? String(value.schema || "") : ""
        const responseToken = value ? String(value.token || "") : ""
        const status = value ? String(value.status || "") : ""
        if (schema !== root.basecampAsyncBridgeSchema || responseToken !== token) {
            root.abandonBasecampToken(requestHost, token)
            root.finishAsyncCall(requestId, {
                ok: false,
                value: null,
                text: "",
                error: "Logos bridge call failed: incompatible async poll schema"
            }, requestHostEpoch)
            return
        }
        if (status === "pending") {
            entry.pollInFlight = false
            entry.nextPollAtMs = Date.now() + root.basecampPollBackoffMs(entry.pollAttempts)
            root.pendingCalls = pending
            return
        }
        if (status === "ready" && typeof value.responseJson === "string") {
            root.releaseBasecampToken(requestHost, token)
            root.finishAsyncCall(
                requestId,
                BridgeEnvelope.parseResponseJson(value.responseJson),
                requestHostEpoch
            )
            return
        }
        root.abandonBasecampToken(requestHost, token)
        root.finishAsyncCall(requestId, {
            ok: false,
            value: null,
            text: "",
            error: "Logos bridge call failed: malformed async poll state"
        }, requestHostEpoch)
    }

    function basecampPollBackoffMs(attempts) {
        const exponent = Math.min(Math.max(Number(attempts || 1) - 1, 0), 3)
        return root.basecampAsyncPollIntervalMs * Math.pow(2, exponent)
    }

    function basecampPollsInFlight() {
        let count = 0
        const pending = root.pendingCalls || {}
        for (const requestId in pending) {
            const entry = pending[requestId]
            if (entry && entry.route === "basecamp_poll" && entry.pollInFlight) {
                count += 1
            }
        }
        return count
    }

    function hasBasecampPollingCalls() {
        const pending = root.pendingCalls || {}
        for (const requestId in pending) {
            if (pending[requestId] && pending[requestId].route === "basecamp_poll") {
                return true
            }
        }
        return false
    }

    function timeoutBasecampCall(requestId, entry) {
        if (!entry || !(root.pendingCalls || {})[requestId]) {
            return
        }
        if (entry.backendToken && entry.backendToken.length) {
            root.abandonBasecampToken(entry.host, entry.backendToken)
        }
        root.finishAsyncCall(requestId, {
            ok: false,
            value: null,
            text: "",
            error: entry.phase === "starting"
                ? "Logos bridge call failed: async_acceptance_unknown"
                : "Logos bridge call failed: async_response_timeout"
        }, entry.hostEpoch)
    }

    function abandonBasecampToken(host, token) {
        if (!host || !String(token || "").length) {
            return
        }
        BridgeEnvelope.cancelBasecampInspectorCall(
            host,
            token,
            root.basecampAsyncTimeoutMs,
            function () {}
        )
        root.releaseBasecampToken(host, token)
    }

    function releaseBasecampToken(host, token) {
        if (!host || !String(token || "").length) {
            return
        }
        BridgeEnvelope.releaseBasecampInspectorCall(
            host,
            token,
            root.basecampAsyncTimeoutMs,
            function () {}
        )
    }

    function subscribeModuleEvents(moduleName, events) {
        let count = 0
        const rows = Array.isArray(events) ? events : []
        for (let i = 0; i < rows.length; ++i) {
            if (root.subscribeModuleEvent(moduleName, rows[i])) {
                count += 1
            }
        }
        return count
    }

    function subscribeModuleEvent(moduleName, eventName) {
        const moduleText = String(moduleName || "")
        const eventText = String(eventName || "")
        if (!root.host || !moduleText.length || !eventText.length) {
            return false
        }
        const key = moduleText + "::" + eventText
        const current = root.moduleEventSubscriptions || {}
        if (current[key] && current[key].active === true) {
            return true
        }
        const subscriptionHost = root.host
        const subscriptionHostEpoch = root.hostEpoch
        const basecampSubscription = root.prefersBasecampModules()
        const registered = basecampSubscription
            ? root.moduleEventRegistration(subscriptionHost, key)
            : null
        if (registered) {
            root.rememberActiveModuleEventSubscription(key, registered.callback)
            return true
        }
        const callback = function () {
            if (root.host !== subscriptionHost
                    || root.hostEpoch !== subscriptionHostEpoch) {
                return
            }
            const values = []
            for (let i = 0; i < arguments.length; ++i) {
                values.push(arguments[i])
            }
            root.moduleEventReceived(moduleText, eventText, values.length === 1 ? values[0] : values)
        }
        let active = false
        try {
            if (subscriptionHost && subscriptionHost["onModuleEvent"]) {
                if (basecampSubscription) {
                    active = subscriptionHost["onModuleEvent"](moduleText, eventText) === true
                } else {
                    active = subscriptionHost["onModuleEvent"](moduleText, eventText, callback) !== false
                }
            }
        } catch (error) {
            active = false
        }
        try {
            if (!active && subscriptionHost && subscriptionHost["module"]) {
                const moduleObject = subscriptionHost["module"](moduleText)
                if (moduleObject && moduleObject["on"]) {
                    moduleObject["on"](eventText, callback)
                    active = true
                }
            }
        } catch (error) {
            active = false
        }
        if (!active) {
            return false
        }
        if (basecampSubscription) {
            const registrations = Array.isArray(root.moduleEventRegistrations)
                ? root.moduleEventRegistrations.slice()
                : []
            registrations.push({
                host: subscriptionHost,
                key: key,
                callback: callback
            })
            root.moduleEventRegistrations = registrations
        }
        if (root.host !== subscriptionHost) {
            return false
        }
        root.rememberActiveModuleEventSubscription(key, callback)
        return true
    }

    function moduleEventRegistration(host, key) {
        const registrations = Array.isArray(root.moduleEventRegistrations)
            ? root.moduleEventRegistrations
            : []
        for (let i = 0; i < registrations.length; ++i) {
            const registration = registrations[i]
            if (registration && registration.host === host && registration.key === key) {
                return registration
            }
        }
        return null
    }

    function rememberActiveModuleEventSubscription(key, callback) {
        const next = {}
        const current = root.moduleEventSubscriptions || {}
        for (const name in current) {
            next[name] = current[name]
        }
        next[key] = {
            active: true,
            callback: callback
        }
        root.moduleEventSubscriptions = next
    }

    function finishAsyncCall(requestId, response, expectedHostEpoch) {
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || (expectedHostEpoch !== undefined
                    && entry.hostEpoch !== expectedHostEpoch)) {
            return
        }
        delete pending[requestId]
        root.pendingCalls = pending
        if (typeof entry.callback === "function") {
            entry.callback(response)
        }
    }

    function failPendingCalls(response) {
        const pending = root.pendingCalls || {}
        root.pendingCalls = ({})
        let firstError = ""
        for (const requestId in pending) {
            const entry = pending[requestId]
            if (entry
                    && entry.route === "basecamp_poll"
                    && entry.backendToken
                    && entry.backendToken.length) {
                root.abandonBasecampToken(entry.host, entry.backendToken)
            }
            if (entry && typeof entry.callback === "function") {
                try {
                    entry.callback(response)
                } catch (error) {
                    if (!firstError.length) {
                        firstError = error && error.message ? error.message : String(error)
                    }
                }
            }
        }
        if (firstError.length) {
            root.callbackFailed(firstError)
        }
    }

    function copyPendingCalls() {
        const copy = {}
        const current = root.pendingCalls || {}
        for (const key in current) {
            copy[key] = current[key]
        }
        return copy
    }

    property Connections hostConnections: Connections {
        target: root.host
        ignoreUnknownSignals: true

        function onModuleCallFinished(requestId, responseJson) {
            root.finishAsyncCall(requestId, BridgeEnvelope.parseResponseJson(responseJson))
        }

        function onModuleEvent(moduleName, eventName, args) {
            root.moduleEventReceived(String(moduleName || ""), String(eventName || ""), args)
        }

        function onModuleEventReceived(moduleName, eventName, args) {
            root.moduleEventReceived(String(moduleName || ""), String(eventName || ""), args)
        }
    }

    property Timer basecampPollTimer: Timer {
        interval: Math.max(1, root.basecampAsyncPollIntervalMs)
        repeat: true
        running: root.hasBasecampPollingCalls()
        onTriggered: root.pollBasecampCalls()
    }
}
