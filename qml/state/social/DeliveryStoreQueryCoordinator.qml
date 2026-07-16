import QtQml
import "../OperationHistoryVocabulary.js" as OperationHistoryVocabulary
import "../source_operations/NodeOperationRequest.js" as NodeOperationRequest

QtObject {
    id: root

    required property var gateway
    property var adapterInitialization: ({ source_mode: "", inputs: ({}) })
    property bool mutatingDiagnosticsEnabled: true
    property int maxPendingQueries: 16
    property int maxPollsPerTick: 4

    property int sourceEpoch: 0
    property int nextGeneration: 0
    property int nextPollToken: 0
    property int pollCursor: 0
    property var currentTickets: ({})
    property var pendingQueries: ({})

    readonly property int activeTicketCount: Object.keys(currentTickets || {}).length
    readonly property int pendingCount: Object.keys(pendingQueries || {}).length
    readonly property bool running: pendingCount > 0

    function start(scope, cursor, pageSize, label, callback) {
        const normalizedScope = normalizeScope(scope)
        if (!normalizedScope.callerKey.length || !normalizedScope.topic.length
                || typeof callback !== "function") {
            return null
        }

        if (callerPending(normalizedScope.callerKey)) {
            callback({
                ok: false,
                value: null,
                error: qsTr("A Delivery Store query is already pending for this caller.")
            }, null)
            return null
        }

        const existing = currentTickets[normalizedScope.callerKey] || null
        if (!existing && activeTicketCount >= Math.max(1, Number(maxPendingQueries || 1))) {
            callback({
                ok: false,
                value: null,
                error: qsTr("Too many Delivery Store queries are pending.")
            }, null)
            return null
        }

        nextGeneration += 1
        const ticket = {
            callerKey: normalizedScope.callerKey,
            family: normalizedScope.family,
            accountId: normalizedScope.accountId,
            topic: normalizedScope.topic,
            sourceEpoch: sourceEpoch,
            generation: nextGeneration
        }
        setCurrentTicket(ticket)

        const request = operationRequest(
            normalizedScope.topic,
            cursor,
            pageSize,
            label
        )
        setPending(normalizedScope.callerKey, {
            ticket: ticket,
            request: request,
            callback: callback,
            operation: null,
            operationId: "",
            pollPending: false,
            pollToken: 0
        })

        if (!gateway || typeof gateway.startRuntimeOperation !== "function") {
            failStart(ticket, callback, qsTr("Runtime operation bridge is unavailable."))
            return null
        }

        gateway.startRuntimeOperation(request, false, function (response) {
            acceptStartResponse(ticket, response)
        })
        return ticket
    }

    function poll() {
        const keys = Object.keys(pendingQueries || {})
        if (!keys.length || !gateway || typeof gateway.runtimeOperationStatus !== "function") {
            return 0
        }

        const limit = Math.max(1, Number(maxPollsPerTick || 1))
        const startIndex = pollCursor % keys.length
        let inspected = 0
        let started = 0
        while (inspected < keys.length && started < limit) {
            const key = keys[(startIndex + inspected) % keys.length]
            const entry = pendingQueries[key] || null
            inspected += 1
            if (!entry || entry.pollPending === true || !String(entry.operationId || "").length) {
                continue
            }
            startPoll(entry)
            started += 1
        }
        pollCursor = keys.length ? (startIndex + inspected) % keys.length : 0
        return started
    }

    function invalidateSource() {
        sourceEpoch += 1
        pendingQueries = ({})
        currentTickets = ({})
        pollCursor = 0
    }

    function invalidateFamily(family) {
        const wanted = String(family || "")
        const tickets = currentTickets || {}
        const keys = Object.keys(tickets)
        for (let i = 0; i < keys.length; ++i) {
            const ticket = tickets[keys[i]] || null
            if (ticket && String(ticket.family || "") === wanted) {
                invalidateCaller(keys[i])
            }
        }
    }

    function invalidateCaller(callerKey) {
        const key = String(callerKey || "")
        if (!key.length) {
            return
        }
        removePending(key)
        const next = copyMap(currentTickets)
        delete next[key]
        currentTickets = next
    }

    function callerPending(callerKey) {
        const key = String(callerKey || "")
        return key.length > 0 && pendingQueries[key] !== undefined
    }

    function isCurrent(ticket) {
        const value = ticket || null
        const key = String(value && value.callerKey || "")
        const current = key.length ? currentTickets[key] || null : null
        return current !== null
            && Number(value.sourceEpoch) === sourceEpoch
            && Number(current.sourceEpoch) === Number(value.sourceEpoch)
            && Number(current.generation) === Number(value.generation)
            && String(current.family || "") === String(value.family || "")
            && String(current.accountId || "") === String(value.accountId || "")
            && String(current.topic || "") === String(value.topic || "")
    }

    function release(ticket) {
        if (!isCurrent(ticket)) {
            return false
        }
        const key = String(ticket.callerKey || "")
        const next = copyMap(currentTickets)
        delete next[key]
        currentTickets = next
        return true
    }

    function normalizeScope(scope) {
        const value = scope && typeof scope === "object" ? scope : ({})
        return {
            callerKey: String(value.callerKey || "").trim(),
            family: String(value.family || "").trim(),
            accountId: String(value.accountId || "").trim(),
            topic: String(value.topic || "").trim()
        }
    }

    function operationRequest(topic, cursor, pageSize, label) {
        const requestedSize = Number(pageSize || 20)
        const size = Number.isFinite(requestedSize)
            ? Math.max(1, Math.min(100, Math.floor(requestedSize))) : 20
        const request = NodeOperationRequest.envelope(
            adapterInitialization,
            NodeOperationRequest.deliveryPayload("deliveryStoreQuery", [
                "",
                String(topic || ""),
                "",
                String(cursor || ""),
                size,
                true,
                true
            ])
        )
        request.domain = "delivery"
        request.method = "deliveryStoreQuery"
        request.label = String(label || qsTr("Delivery Store"))
        return request
    }

    function acceptStartResponse(ticket, response) {
        const entry = pendingEntry(ticket)
        if (!entry) {
            return
        }
        if (!response || response.ok !== true) {
            finishFailure(ticket, response || {
                ok: false,
                value: null,
                error: qsTr("Delivery Store query failed to start.")
            })
            return
        }

        const operation = response.value || null
        if (!matchesRequest(operation, entry.request)) {
            finishFailure(ticket, {
                ok: false,
                value: null,
                error: qsTr("Delivery Store returned an invalid operation identity.")
            })
            return
        }
        if (isTerminal(operation)) {
            finishTerminal(ticket, operation)
            return
        }

        const next = copyEntry(entry)
        next.operation = operation
        next.operationId = String(operation.operationId || "")
        setPending(ticket.callerKey, next)
    }

    function startPoll(entry) {
        const ticket = entry.ticket
        const operationId = String(entry.operationId || "")
        nextPollToken += 1
        const pollToken = nextPollToken
        const next = copyEntry(entry)
        next.pollPending = true
        next.pollToken = pollToken
        setPending(ticket.callerKey, next)
        gateway.runtimeOperationStatus(operationId, false, function (response) {
            acceptPollResponse(ticket, operationId, pollToken, response)
        })
    }

    function acceptPollResponse(ticket, operationId, pollToken, response) {
        const entry = pendingEntry(ticket)
        if (!entry || entry.pollPending !== true
                || Number(entry.pollToken) !== Number(pollToken)
                || String(entry.operationId || "") !== String(operationId || "")) {
            return
        }

        const released = copyEntry(entry)
        released.pollPending = false
        released.pollToken = 0
        setPending(ticket.callerKey, released)
        if (!response || response.ok !== true) {
            return
        }

        const operation = response.value || null
        if (!matchesOperationId(operation, operationId)
                || !matchesRequest(operation, released.request)
                || !OperationHistoryVocabulary.runtimeSnapshotIsNewer(released.operation, operation)) {
            return
        }
        if (isTerminal(operation)) {
            finishTerminal(ticket, operation)
            return
        }

        released.operation = operation
        setPending(ticket.callerKey, released)
    }

    function finishTerminal(ticket, operation) {
        const entry = pendingEntry(ticket)
        if (!entry) {
            return
        }
        removePending(ticket.callerKey)
        if (gateway && typeof gateway.appendOperationHistory === "function") {
            gateway.appendOperationHistory(operation, "")
        }
        const completed = String(operation.status || "") === "completed"
        const hasResult = operation.result !== undefined
            && operation.result !== null
            && typeof operation.result === "object"
        const ok = completed && hasResult
        const response = {
            ok: ok,
            value: ok ? operation.result : null,
            operation: operation,
            error: ok ? "" : String(operation.error
                || qsTr("Delivery Store query did not return a completed result."))
        }
        deliver(entry, response)
    }

    function finishFailure(ticket, response) {
        const entry = pendingEntry(ticket)
        if (!entry) {
            return
        }
        removePending(ticket.callerKey)
        deliver(entry, response || {
            ok: false,
            value: null,
            error: qsTr("Delivery Store query failed.")
        })
    }

    function failStart(ticket, callback, error) {
        removePending(ticket.callerKey)
        let retain = false
        try {
            retain = callback({ ok: false, value: null, error: String(error || "") }, ticket) === true
        } finally {
            if (!retain) {
                release(ticket)
            }
        }
    }

    function deliver(entry, response) {
        if (!entry || !isCurrent(entry.ticket)) {
            return
        }
        let retain = false
        try {
            retain = entry.callback(response, entry.ticket) === true
        } finally {
            if (!retain) {
                release(entry.ticket)
            }
        }
    }

    function pendingEntry(ticket) {
        if (!isCurrent(ticket)) {
            return null
        }
        const entry = pendingQueries[String(ticket.callerKey || "")] || null
        return entry && Number(entry.ticket.generation) === Number(ticket.generation)
            ? entry : null
    }

    function matchesRequest(operation, request) {
        if (!operation || !String(operation.operationId || "").length) {
            return false
        }
        const expectedDomain = String(request && request.domain || "")
        const expectedMethod = String(request && request.method || "")
        const actualDomain = String(operation.domain || "")
        const actualMethod = String(operation.method || "")
        return actualDomain === expectedDomain && actualMethod === expectedMethod
    }

    function matchesOperationId(operation, operationId) {
        const actual = String(operation && operation.operationId || "")
        return actual.length > 0 && actual === String(operationId || "")
    }

    function isTerminal(operation) {
        return OperationHistoryVocabulary.isRuntimeTerminalStatus(operation && operation.status)
    }

    function setCurrentTicket(ticket) {
        const next = copyMap(currentTickets)
        next[ticket.callerKey] = ticket
        currentTickets = next
    }

    function setPending(callerKey, entry) {
        const next = copyMap(pendingQueries)
        next[String(callerKey || "")] = entry
        pendingQueries = next
    }

    function removePending(callerKey) {
        const key = String(callerKey || "")
        if (!pendingQueries[key]) {
            return
        }
        const next = copyMap(pendingQueries)
        delete next[key]
        pendingQueries = next
    }

    function copyEntry(entry) {
        const next = ({})
        const value = entry || {}
        const keys = Object.keys(value)
        for (let i = 0; i < keys.length; ++i) {
            next[keys[i]] = value[keys[i]]
        }
        return next
    }

    function copyMap(value) {
        const next = ({})
        const source = value && typeof value === "object" ? value : ({})
        const keys = Object.keys(source)
        for (let i = 0; i < keys.length; ++i) {
            next[keys[i]] = source[keys[i]]
        }
        return next
    }
}
