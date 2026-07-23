import QtQuick
import "BridgeEnvelope.js" as BridgeEnvelope
import "BridgeHelpers.js" as BridgeHelpers

QtObject {
    id: root

    property var host: null
    property bool closed: false
    property int hostEpoch: 0
    property int nextRequestId: 1
    property var pendingCalls: ({})
    property var moduleEventSubscriptions: ({})
    property var moduleEventRegistrations: []
    property var shutdownBasecampTokenSettlements: []
    property string basecampRuntimeModuleEventOwnership: "unknown"
    property int basecampRuntimeModuleEventOwnershipEpoch: -1
    property var basecampRuntimeModuleEventOwnershipCallbacks: []
    property double basecampRuntimeModuleEventOwnershipDeadlineMs: 0
    property bool basecampRuntimeModuleEventOwnershipProbeInFlight: false
    readonly property string basecampAsyncBridgeSchema: "logos-inspector-async-bridge/v1"
    property int basecampAsyncPollIntervalMs: 50
    property int basecampAsyncTimeoutMs: 30000
    property int basecampAsyncStartAttemptTimeoutMs: 2000
    property int basecampAsyncMaxPollAttempts: 600
    property int basecampAsyncMaxPollsInFlight: 8
    property int basecampAsyncMaxPendingCalls: 128

    onHostChanged: {
        hostEpoch += 1
        basecampRuntimeModuleEventOwnershipRetryTimer.stop()
        basecampRuntimeModuleEventOwnership = "unknown"
        basecampRuntimeModuleEventOwnershipEpoch = -1
        basecampRuntimeModuleEventOwnershipCallbacks = []
        basecampRuntimeModuleEventOwnershipDeadlineMs = 0
        basecampRuntimeModuleEventOwnershipProbeInFlight = false
        root.deactivateModuleEventCallbacks()
        moduleEventSubscriptions = ({})
        moduleEventRegistrations = []
        if (root.closed) {
            return
        }
        failPendingCalls({
            ok: false,
            value: null,
            text: "",
            error: "Logos bridge call failed: host_changed"
        }, true)
    }

    signal moduleEventReceived(string moduleName, string eventName, var args)
    signal callbackFailed(string error)
    signal runtimeModuleEventOwnershipChanged(bool owned)

    function prefersBasecampModules() {
        return !root.closed && BridgeEnvelope.prefersBasecampModules(root.host)
    }

    function backendOwnsRuntimeModuleEvents() {
        if (root.closed || !root.host) {
            return false
        }
        if (root.prefersBasecampModules()) {
            return root.basecampRuntimeModuleEventOwnershipEpoch === root.hostEpoch
                && root.basecampRuntimeModuleEventOwnership === "owned"
        }
        try {
            if (typeof root.host["backendOwnsRuntimeModuleEvents"] === "function"
                    && root.host["backendOwnsRuntimeModuleEvents"]() === true) {
                return true
            }
        } catch (error) {
        }
        return false
    }

    function ensureRuntimeModuleEventOwnership(callback) {
        if (root.closed || !root.host) {
            root.notifyCallback(callback, false)
            return false
        }
        if (!root.prefersBasecampModules() || root.backendOwnsRuntimeModuleEvents()) {
            const owned = root.backendOwnsRuntimeModuleEvents()
            root.notifyCallback(callback, owned)
            return owned
        }
        const callbacks = Array.isArray(root.basecampRuntimeModuleEventOwnershipCallbacks)
            ? root.basecampRuntimeModuleEventOwnershipCallbacks.slice()
            : []
        if (typeof callback === "function") {
            callbacks.push(callback)
        }
        root.basecampRuntimeModuleEventOwnershipCallbacks = callbacks
        if (root.basecampRuntimeModuleEventOwnershipEpoch === root.hostEpoch
                && root.basecampRuntimeModuleEventOwnership === "unavailable") {
            root.finishBasecampRuntimeModuleEventOwnership(false)
            return false
        }
        if (root.basecampRuntimeModuleEventOwnershipEpoch === root.hostEpoch
                && root.basecampRuntimeModuleEventOwnership === "probing") {
            return false
        }
        root.basecampRuntimeModuleEventOwnership = "probing"
        root.basecampRuntimeModuleEventOwnershipEpoch = root.hostEpoch
        root.basecampRuntimeModuleEventOwnershipDeadlineMs = Date.now()
            + Math.max(1, root.basecampAsyncTimeoutMs)
        root.probeBasecampRuntimeModuleEventOwnership()
        return false
    }

    function probeBasecampRuntimeModuleEventOwnership() {
        if (root.closed || !root.prefersBasecampModules()
                || root.basecampRuntimeModuleEventOwnership !== "probing"
                || root.basecampRuntimeModuleEventOwnershipEpoch !== root.hostEpoch
                || root.basecampRuntimeModuleEventOwnershipProbeInFlight) {
            return
        }
        const probeHost = root.host
        const probeEpoch = root.hostEpoch
        const remainingMs = Math.max(
            0,
            root.basecampRuntimeModuleEventOwnershipDeadlineMs - Date.now()
        )
        if (remainingMs <= 0) {
            root.finishBasecampRuntimeModuleEventOwnership(false)
            return
        }
        root.basecampRuntimeModuleEventOwnershipProbeInFlight = true
        const dispatched = BridgeEnvelope.probeBasecampInspectorRuntimeModuleEventOwnership(
            probeHost,
            Math.min(root.basecampAsyncStartAttemptTimeoutMs, remainingMs),
            function (response) {
                if (root.closed
                        || root.host !== probeHost
                        || root.hostEpoch !== probeEpoch
                        || root.basecampRuntimeModuleEventOwnershipEpoch !== probeEpoch) {
                    return
                }
                root.basecampRuntimeModuleEventOwnershipProbeInFlight = false
                if (response && response.ok === true && response.value === true) {
                    root.finishBasecampRuntimeModuleEventOwnership(true)
                    return
                }
                if (BridgeEnvelope.retryableBasecampStartError(response)
                        && Date.now() < root.basecampRuntimeModuleEventOwnershipDeadlineMs) {
                    root.basecampRuntimeModuleEventOwnershipRetryTimer.restart()
                    return
                }
                root.finishBasecampRuntimeModuleEventOwnership(false)
            }
        )
        if (!dispatched) {
            root.basecampRuntimeModuleEventOwnershipProbeInFlight = false
            root.finishBasecampRuntimeModuleEventOwnership(false)
        }
    }

    function finishBasecampRuntimeModuleEventOwnership(owned) {
        if (root.closed) {
            return
        }
        root.basecampRuntimeModuleEventOwnershipRetryTimer.stop()
        root.basecampRuntimeModuleEventOwnershipProbeInFlight = false
        root.basecampRuntimeModuleEventOwnership = owned === true ? "owned" : "unavailable"
        root.basecampRuntimeModuleEventOwnershipEpoch = root.hostEpoch
        const callbacks = Array.isArray(root.basecampRuntimeModuleEventOwnershipCallbacks)
            ? root.basecampRuntimeModuleEventOwnershipCallbacks.slice()
            : []
        root.basecampRuntimeModuleEventOwnershipCallbacks = []
        root.runtimeModuleEventOwnershipChanged(owned === true)
        for (let index = 0; index < callbacks.length; ++index) {
            root.notifyCallback(callbacks[index], owned === true)
        }
    }

    function startModuleWatcher() {
        if (root.closed || !root.host
                || typeof root.host["startModuleWatcher"] !== "function") {
            return null
        }
        try {
            return root.host["startModuleWatcher"]() === true
        } catch (error) {
            return false
        }
    }

    function callModule(moduleName, method, args) {
        if (root.closed) {
            return root.closedResponse()
        }
        return BridgeEnvelope.callModule(root.host, moduleName, method, args || [])
    }

    function hasAsyncCalls() {
        return !root.closed && BridgeEnvelope.hasAsyncCalls(root.host)
    }

    function callModuleAsync(moduleName, method, args, callback) {
        const requestId = root.nextRequestId
        if (root.closed) {
            root.nextRequestId += 1
            root.notifyCallback(callback, root.closedResponse())
            return requestId
        }
        const requestHost = root.host
        const requestHostEpoch = root.hostEpoch
        root.nextRequestId += 1
        if (Object.keys(root.pendingCalls || {}).length >= root.basecampAsyncMaxPendingCalls) {
            root.notifyCallback(callback, {
                ok: false,
                value: null,
                text: "",
                error: "Logos bridge call failed: async_client_capacity"
            })
            return requestId
        }
        const inspectorAsyncRequest = root.prefersBasecampModules()
            && moduleName === "logos_inspector"
            && method !== "moduleVersion"
        const basecampPolling = BridgeEnvelope.usesBasecampInspectorPolling(
            requestHost,
            moduleName,
            method
        )
        const authoritativeCompletion = basecampPolling
            && moduleName === "logos_inspector"
            && method === "settingsBackupImportApply"
        let startArgsJson = ""
        if (basecampPolling) {
            try {
                startArgsJson = JSON.stringify(Array.isArray(args) ? args : [])
            } catch (error) {
                root.notifyCallback(callback, {
                    ok: false,
                    value: null,
                    text: "",
                    error: "Logos bridge call failed: async_arguments_invalid"
                })
                return requestId
            }
        }
        const requestLifecycle = {
            owner: root,
            settledTokens: []
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
            startAttemptId: 0,
            nextStartAtMs: 0,
            requestLifecycle: requestLifecycle,
            backendToken: "",
            pollInFlight: false,
            pollAttempts: 0,
            pollAttemptId: 0,
            nextPollAtMs: 0,
            authoritativeCompletion: authoritativeCompletion,
            deadlineMs: basecampPolling && !authoritativeCompletion
                ? Date.now() + root.basecampAsyncTimeoutMs
                : 0
        }
        root.pendingCalls = pending

        if (basecampPolling) {
            root.beginBasecampCall(requestId)
            return requestId
        }

        if (inspectorAsyncRequest && !basecampPolling) {
            Qt.callLater(function () {
                const owner = requestLifecycle.owner
                if (!owner) {
                    return
                }
                owner.finishAsyncCall(requestId, {
                    ok: false,
                    value: null,
                    text: "",
                    error: "Logos bridge call failed: Basecamp inspector async bridge v1 required"
                }, requestHostEpoch)
            })
            return requestId
        }

        if (BridgeEnvelope.dispatchAsync(requestHost, requestId, moduleName, method, args || [], function (response) {
                const owner = requestLifecycle.owner
                if (owner) {
                    owner.finishAsyncCall(requestId, response, requestHostEpoch)
                }
            })) {
            return requestId
        }

        Qt.callLater(function () {
            const owner = requestLifecycle.owner
            if (!owner || requestHostEpoch !== owner.hostEpoch) {
                return
            }
            owner.finishAsyncCall(
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
        if (root.closed) {
            return false
        }
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || entry.route !== "basecamp_poll"
                || entry.phase !== "starting"
                || entry.startInFlight
                || (!entry.authoritativeCompletion
                    && (entry.host !== root.host
                        || entry.hostEpoch !== root.hostEpoch))) {
            return false
        }
        const remainingMs = entry.authoritativeCompletion
            ? root.basecampAsyncStartAttemptTimeoutMs
            : entry.deadlineMs - Date.now()
        if (!entry.authoritativeCompletion && remainingMs <= 0) {
            root.timeoutBasecampCall(requestId, entry)
            return false
        }
        entry.startInFlight = true
        entry.startAttempts += 1
        entry.startAttemptId = Number(entry.startAttemptId || 0) + 1
        const startAttemptId = entry.startAttemptId
        const requestHost = entry.host
        const requestHostEpoch = entry.hostEpoch
        const expectedCorrelationId = entry.correlationId
        const requestLifecycle = entry.requestLifecycle
        const settlementTimeoutMs = root.basecampAsyncTimeoutMs
        const attemptTimeoutMs = Math.max(
            1,
            Math.min(root.basecampAsyncStartAttemptTimeoutMs, remainingMs)
        )
        root.pendingCalls = pending
        const dispatched = BridgeEnvelope.beginBasecampInspectorCall(
            requestHost,
            expectedCorrelationId,
            entry.startMethod,
            entry.startArgsJson,
            attemptTimeoutMs,
            function (response) {
                const owner = requestLifecycle ? requestLifecycle.owner : null
                if (owner) {
                    owner.handleBasecampStart(
                        requestId,
                        requestHost,
                        requestHostEpoch,
                        expectedCorrelationId,
                        requestLifecycle,
                        startAttemptId,
                        response
                    )
                    return
                }
                const value = response && response.ok === true ? response.value : null
                const token = value && typeof value.token === "string" ? value.token : ""
                const responseCorrelationId = value ? String(value.correlationId || "") : ""
                if (!token.length || responseCorrelationId !== expectedCorrelationId) {
                    return
                }
                const settledTokens = requestLifecycle
                        && Array.isArray(requestLifecycle.settledTokens)
                    ? requestLifecycle.settledTokens.slice()
                    : []
                if (settledTokens.indexOf(token) >= 0) {
                    return
                }
                settledTokens.push(token)
                requestLifecycle.settledTokens = settledTokens
                BridgeEnvelope.cancelBasecampInspectorCall(
                    requestHost,
                    token,
                    settlementTimeoutMs,
                    function () {}
                )
                BridgeEnvelope.releaseBasecampInspectorCall(
                    requestHost,
                    token,
                    settlementTimeoutMs,
                    function () {}
                )
            }
        )
        if (!dispatched) {
            const current = root.copyPendingCalls()
            const currentEntry = current[requestId]
            if (!currentEntry
                    || currentEntry.phase !== "starting"
                    || currentEntry.startAttemptId !== startAttemptId) {
                return false
            }
            if (currentEntry.authoritativeCompletion) {
                currentEntry.startInFlight = false
                currentEntry.nextStartAtMs = Date.now()
                    + root.basecampPollBackoffMs(currentEntry.startAttempts)
                root.pendingCalls = current
                return false
            }
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
            requestLifecycle,
            startAttemptId,
            response) {
        const value = response && response.ok === true ? response.value : null
        const token = value && typeof value.token === "string" ? value.token : ""
        const schema = value ? String(value.schema || "") : ""
        const responseCorrelationId = value ? String(value.correlationId || "") : ""
        const correlationMatches = responseCorrelationId === expectedCorrelationId
        if (root.closed) {
            if (token.length && correlationMatches) {
                root.markBasecampRequestTokenSettled(requestLifecycle, token)
                root.abandonBasecampTokenAfterShutdown(requestHost, token)
            }
            return
        }
        const current = root.pendingCalls || {}
        const entry = current[requestId]
        if (!entry
                || entry.host !== requestHost
                || entry.hostEpoch !== requestHostEpoch
                || (!entry.authoritativeCompletion
                    && (root.host !== requestHost
                        || root.hostEpoch !== requestHostEpoch))) {
            if (token.length && correlationMatches) {
                root.markBasecampRequestTokenSettled(requestLifecycle, token)
                root.abandonBasecampToken(requestHost, token)
            }
            return
        }
        if (entry.phase !== "starting") {
            if (token.length
                    && correlationMatches
                    && token !== String(entry.backendToken || "")) {
                root.markBasecampRequestTokenSettled(requestLifecycle, token)
                root.abandonBasecampToken(requestHost, token)
            }
            return
        }
        const accepted = response
            && response.ok === true
            && schema === root.basecampAsyncBridgeSchema
            && token.length
            && correlationMatches
            && entry.correlationId === expectedCorrelationId
        if (entry.startAttemptId !== startAttemptId && !accepted) {
            return
        }
        if (!response || response.ok !== true) {
            if (entry.authoritativeCompletion
                    || (BridgeEnvelope.retryableBasecampStartError(response)
                        && Date.now() < entry.deadlineMs)) {
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
            if (entry.authoritativeCompletion) {
                const retry = root.copyPendingCalls()
                const retryEntry = retry[requestId]
                if (retryEntry && retryEntry.phase === "starting") {
                    retryEntry.startInFlight = false
                    retryEntry.nextStartAtMs = Date.now()
                        + root.basecampPollBackoffMs(retryEntry.startAttempts)
                    root.pendingCalls = retry
                }
                return
            }
            if (token.length && correlationMatches) {
                root.markBasecampRequestTokenSettled(requestLifecycle, token)
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
            root.markBasecampRequestTokenSettled(requestLifecycle, token)
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
        if (root.closed) {
            return
        }
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
            if (!entry.authoritativeCompletion && now >= entry.deadlineMs) {
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
            if (!entry.authoritativeCompletion
                    && entry.pollAttempts >= root.basecampAsyncMaxPollAttempts) {
                root.timeoutBasecampCall(requestId, entry)
                continue
            }
            if (now < Number(entry.nextPollAtMs || 0)
                    || inFlight >= root.basecampAsyncMaxPollsInFlight) {
                continue
            }
            if (root.pollBasecampCall(requestId)) {
                inFlight += 1
            }
        }
    }

    function pollBasecampCall(requestId) {
        if (root.closed) {
            return false
        }
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || entry.route !== "basecamp_poll"
                || entry.phase !== "polling"
                || entry.pollInFlight
                || !entry.backendToken.length
                || (!entry.authoritativeCompletion
                    && (entry.host !== root.host
                        || entry.hostEpoch !== root.hostEpoch))) {
            return false
        }
        if ((!entry.authoritativeCompletion && Date.now() >= entry.deadlineMs)
                || (!entry.authoritativeCompletion
                    && entry.pollAttempts >= root.basecampAsyncMaxPollAttempts)) {
            root.timeoutBasecampCall(requestId, entry)
            return false
        }
        entry.pollInFlight = true
        entry.pollAttempts += 1
        entry.pollAttemptId = Number(entry.pollAttemptId || 0) + 1
        const pollAttemptId = entry.pollAttemptId
        const token = entry.backendToken
        const requestHost = entry.host
        const requestHostEpoch = entry.hostEpoch
        const requestLifecycle = entry.requestLifecycle
        root.pendingCalls = pending
        const dispatched = BridgeEnvelope.pollBasecampInspectorCall(
            requestHost,
            token,
            root.basecampAsyncTimeoutMs,
            function (response) {
                const owner = requestLifecycle ? requestLifecycle.owner : null
                if (owner) {
                    owner.handleBasecampPoll(
                        requestId,
                        requestHost,
                        requestHostEpoch,
                        token,
                        pollAttemptId,
                        response
                    )
                }
            }
        )
        if (dispatched) {
            return true
        }
        const current = root.copyPendingCalls()
        const currentEntry = current[requestId]
        if (!currentEntry
                || currentEntry.phase !== "polling"
                || currentEntry.pollAttemptId !== pollAttemptId) {
            return false
        }
        if (currentEntry.authoritativeCompletion) {
            currentEntry.pollInFlight = false
            currentEntry.nextPollAtMs = Date.now()
                + root.basecampPollBackoffMs(currentEntry.pollAttempts)
            root.pendingCalls = current
            return false
        }
        root.markBasecampRequestTokenSettled(currentEntry.requestLifecycle, token)
        root.abandonBasecampToken(requestHost, token)
        root.finishAsyncCall(requestId, {
            ok: false,
            value: null,
            text: "",
            error: "Logos bridge call failed: Basecamp inspector async bridge unavailable"
        }, requestHostEpoch)
        return false
    }

    function handleBasecampPoll(
            requestId,
            requestHost,
            requestHostEpoch,
            token,
            pollAttemptId,
            response) {
        if (root.closed) {
            return
        }
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || entry.host !== requestHost
                || entry.hostEpoch !== requestHostEpoch
                || entry.backendToken !== token
                || (!entry.authoritativeCompletion
                    && (root.host !== requestHost
                        || root.hostEpoch !== requestHostEpoch))) {
            return
        }
        const value = response && response.ok === true ? response.value : null
        const schema = value ? String(value.schema || "") : ""
        const responseToken = value ? String(value.token || "") : ""
        const status = value ? String(value.status || "") : ""
        if (entry.pollAttemptId !== pollAttemptId
                && !(entry.authoritativeCompletion
                    && schema === root.basecampAsyncBridgeSchema
                    && responseToken === token
                    && status === "ready"
                    && typeof value.responseJson === "string")) {
            return
        }
        if (!response || response.ok !== true) {
            if (entry.authoritativeCompletion
                    || (BridgeEnvelope.retryableBasecampPollError(response)
                        && Date.now() < entry.deadlineMs)) {
                entry.pollInFlight = false
                entry.nextPollAtMs = Date.now() + root.basecampPollBackoffMs(entry.pollAttempts)
                root.pendingCalls = pending
                return
            }
            root.markBasecampRequestTokenSettled(entry.requestLifecycle, token)
            root.abandonBasecampToken(requestHost, token)
            root.finishAsyncCall(requestId, response || {
                ok: false,
                value: null,
                text: "",
                error: "Logos bridge call failed: malformed async poll response"
            }, requestHostEpoch)
            return
        }
        if (schema !== root.basecampAsyncBridgeSchema || responseToken !== token) {
            if (entry.authoritativeCompletion) {
                entry.pollInFlight = false
                entry.nextPollAtMs = Date.now()
                    + root.basecampPollBackoffMs(entry.pollAttempts)
                root.pendingCalls = pending
                return
            }
            root.markBasecampRequestTokenSettled(entry.requestLifecycle, token)
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
            root.markBasecampRequestTokenSettled(entry.requestLifecycle, token)
            root.releaseBasecampToken(requestHost, token)
            root.finishAsyncCall(
                requestId,
                BridgeEnvelope.parseResponseJson(value.responseJson),
                requestHostEpoch
            )
            return
        }
        if (entry.authoritativeCompletion) {
            entry.pollInFlight = false
            entry.nextPollAtMs = Date.now() + root.basecampPollBackoffMs(entry.pollAttempts)
            root.pendingCalls = pending
            return
        }
        root.markBasecampRequestTokenSettled(entry.requestLifecycle, token)
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
        if (root.closed) {
            return false
        }
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
            root.markBasecampRequestTokenSettled(
                entry.requestLifecycle,
                entry.backendToken
            )
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

    function markBasecampRequestTokenSettled(lifecycle, token) {
        const tokenText = String(token || "")
        if (!lifecycle || !tokenText.length) {
            return
        }
        const settledTokens = Array.isArray(lifecycle.settledTokens)
            ? lifecycle.settledTokens.slice()
            : []
        if (settledTokens.indexOf(tokenText) >= 0) {
            return
        }
        settledTokens.push(tokenText)
        lifecycle.settledTokens = settledTokens
    }

    function detachRequestLifecycle(entry) {
        if (entry && entry.requestLifecycle) {
            entry.requestLifecycle.owner = null
        }
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

    function abandonBasecampTokenAfterShutdown(host, token) {
        const tokenText = String(token || "")
        if (!host || !tokenText.length) {
            return
        }
        const settlements = Array.isArray(root.shutdownBasecampTokenSettlements)
            ? root.shutdownBasecampTokenSettlements.slice()
            : []
        for (let i = 0; i < settlements.length; ++i) {
            const settlement = settlements[i]
            if (settlement && settlement.host === host && settlement.token === tokenText) {
                return
            }
        }
        settlements.push({ host: host, token: tokenText })
        root.shutdownBasecampTokenSettlements = settlements
        root.abandonBasecampToken(host, tokenText)
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
        if (root.closed) {
            return 0
        }
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
        if (root.closed || !root.host || !moduleText.length || !eventText.length) {
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
            root.rememberActiveModuleEventSubscription(key, null, null)
            return true
        }
        if (basecampSubscription) {
            if (root.backendOwnsRuntimeModuleEvents()) {
                root.rememberActiveModuleEventSubscription(key, null, null)
                return true
            }
            let globallyRegistered = false
            try {
                if (subscriptionHost && subscriptionHost["onModuleEvent"]) {
                    globallyRegistered = subscriptionHost["onModuleEvent"](
                        moduleText,
                        eventText
                    ) === true
                }
            } catch (error) {
                globallyRegistered = false
            }
            if (globallyRegistered) {
                if (root.host !== subscriptionHost) {
                    return false
                }
                const registrations = Array.isArray(root.moduleEventRegistrations)
                    ? root.moduleEventRegistrations.slice()
                    : []
                registrations.push({
                    host: subscriptionHost,
                    key: key
                })
                root.moduleEventRegistrations = registrations
                root.rememberActiveModuleEventSubscription(key, null, null)
                return true
            }
        }
        const callbackLifecycle = {
            owner: root,
            host: subscriptionHost,
            hostEpoch: subscriptionHostEpoch
        }
        const callback = function () {
            const owner = callbackLifecycle.owner
            if (!owner
                    || owner.closed
                    || owner.host !== callbackLifecycle.host
                    || owner.hostEpoch !== callbackLifecycle.hostEpoch) {
                return
            }
            const values = []
            for (let i = 0; i < arguments.length; ++i) {
                values.push(arguments[i])
            }
            owner.moduleEventReceived(
                moduleText,
                eventText,
                values.length === 1 ? values[0] : values
            )
        }
        let active = false
        try {
            if (!basecampSubscription
                    && subscriptionHost
                    && subscriptionHost["onModuleEvent"]) {
                active = subscriptionHost["onModuleEvent"](
                    moduleText,
                    eventText,
                    callback
                ) !== false
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
            callbackLifecycle.owner = null
            return false
        }
        if (root.host !== subscriptionHost) {
            callbackLifecycle.owner = null
            return false
        }
        root.rememberActiveModuleEventSubscription(key, callback, callbackLifecycle)
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

    function rememberActiveModuleEventSubscription(key, callback, lifecycle) {
        const next = {}
        const current = root.moduleEventSubscriptions || {}
        for (const name in current) {
            next[name] = current[name]
        }
        next[key] = {
            active: true,
            callback: callback,
            lifecycle: lifecycle
        }
        root.moduleEventSubscriptions = next
    }

    function deactivateModuleEventCallbacks() {
        const subscriptions = root.moduleEventSubscriptions || {}
        for (const key in subscriptions) {
            const subscription = subscriptions[key]
            if (subscription && subscription.lifecycle) {
                subscription.lifecycle.owner = null
            }
        }
    }

    function finishAsyncCall(requestId, response, expectedHostEpoch) {
        if (root.closed) {
            return
        }
        const pending = root.copyPendingCalls()
        const entry = pending[requestId]
        if (!entry
                || (expectedHostEpoch !== undefined
                    && entry.hostEpoch !== expectedHostEpoch)) {
            return
        }
        delete pending[requestId]
        root.pendingCalls = pending
        root.detachRequestLifecycle(entry)
        if (typeof entry.callback === "function") {
            entry.callback(response)
        }
    }

    function failPendingCalls(response, retainAuthoritative) {
        const pending = root.pendingCalls || {}
        const retained = {}
        const failed = {}
        let firstError = ""
        for (const requestId in pending) {
            const entry = pending[requestId]
            if (retainAuthoritative === true
                    && entry
                    && entry.route === "basecamp_poll"
                    && entry.authoritativeCompletion === true) {
                retained[requestId] = entry
            } else {
                failed[requestId] = entry
            }
        }
        root.pendingCalls = retained
        for (const requestId in failed) {
            const entry = failed[requestId]
            if (entry
                    && entry.route === "basecamp_poll"
                    && entry.backendToken
                    && entry.backendToken.length) {
                root.markBasecampRequestTokenSettled(
                    entry.requestLifecycle,
                    entry.backendToken
                )
                if (root.closed) {
                    root.abandonBasecampTokenAfterShutdown(entry.host, entry.backendToken)
                } else {
                    root.abandonBasecampToken(entry.host, entry.backendToken)
                }
            }
            root.detachRequestLifecycle(entry)
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

    function closedResponse() {
        return {
            ok: false,
            value: null,
            text: "",
            error: "Logos bridge call failed: client_shutdown"
        }
    }

    function notifyCallback(callback, response) {
        if (typeof callback !== "function") {
            return
        }
        try {
            callback(response)
        } catch (error) {
            root.callbackFailed(error && error.message ? error.message : String(error))
        }
    }

    function shutdown() {
        if (root.closed) {
            return false
        }
        root.closed = true
        root.hostEpoch += 1
        root.basecampPollTimer.stop()
        root.basecampRuntimeModuleEventOwnershipRetryTimer.stop()
        root.basecampRuntimeModuleEventOwnershipCallbacks = []
        root.basecampRuntimeModuleEventOwnershipProbeInFlight = false
        root.deactivateModuleEventCallbacks()
        root.moduleEventSubscriptions = ({})
        root.moduleEventRegistrations = []
        root.failPendingCalls(root.closedResponse())
        return true
    }

    property Connections hostConnections: Connections {
        target: root.closed ? null : root.host
        ignoreUnknownSignals: true

        function onModuleCallFinished(requestId, responseJson) {
            if (!root.closed) {
                root.finishAsyncCall(requestId, BridgeEnvelope.parseResponseJson(responseJson))
            }
        }

        function onModuleEvent(moduleName, eventName, args) {
            if (!root.closed) {
                root.moduleEventReceived(String(moduleName || ""), String(eventName || ""), args)
            }
        }

        function onModuleEventReceived(moduleName, eventName, args) {
            if (!root.closed) {
                root.moduleEventReceived(String(moduleName || ""), String(eventName || ""), args)
            }
        }

        function onModuleEventJson(moduleName, eventName, argsJson) {
            if (root.closed) {
                return
            }
            const parsed = BridgeHelpers.parseJson(String(argsJson || "[]"))
            if (!parsed.ok || !Array.isArray(parsed.value)) {
                return
            }
            root.moduleEventReceived(
                String(moduleName || ""),
                String(eventName || ""),
                parsed.value
            )
        }
    }

    property Timer basecampPollTimer: Timer {
        interval: Math.max(1, root.basecampAsyncPollIntervalMs)
        repeat: true
        running: !root.closed && root.hasBasecampPollingCalls()
        onTriggered: root.pollBasecampCalls()
    }

    property Timer basecampRuntimeModuleEventOwnershipRetryTimer: Timer {
        interval: Math.max(1, root.basecampAsyncPollIntervalMs)
        repeat: false
        onTriggered: root.probeBasecampRuntimeModuleEventOwnership()
    }

    Component.onDestruction: root.shutdown()
}
