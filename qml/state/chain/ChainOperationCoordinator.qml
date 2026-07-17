import QtQml
import "../OperationHistoryVocabulary.js" as OperationHistoryVocabulary
import "BlockchainRangeValidation.js" as BlockchainRangeValidation

QtObject {
    id: root

    required property var gateway
    required property var sourceArgs
    required property int configurationGeneration
    property int maxActive: 16
    property int maxPollsPerTick: 4

    property int sourceEpoch: 0
    property int nextGeneration: 0
    property int nextPollToken: 0
    property int pollCursor: 0
    property var currentTickets: ({})
    property var pendingOperations: ({})
    readonly property string sourceSignature: JSON.stringify(sourceArgs || [])
    readonly property int activeTicketCount: Object.keys(currentTickets || {}).length
    readonly property int pendingCount: Object.keys(pendingOperations || {}).length
    readonly property bool running: pendingCount > 0

    onSourceSignatureChanged: invalidateSource(qsTr("Blockchain source changed."))
    onConfigurationGenerationChanged: invalidateSource(qsTr("Blockchain configuration changed."))

    function start(callerKey, command, callback) {
        const key = String(callerKey || "").trim()
        const normalized = normalizeCommand(command)
        if (!key.length || !normalized.method.length || typeof callback !== "function") {
            return null
        }
        if (!knownMethod(normalized.method)) {
            callback(failure(qsTr("Unknown Blockchain operation.")), null)
            return null
        }
        const rangeValidation = blockRangeValidation(normalized)
        if (rangeValidation && !rangeValidation.valid) {
            callback(failure(rangeValidation.message.length > 0
                ? rangeValidation.message
                : qsTr("Blockchain query arguments are invalid.")), null)
            return null
        }
        if (callerPending(key)) {
            const response = failure(qsTr("A Blockchain query is already pending for this caller."))
            response.busy = true
            callback(response, null)
            return null
        }

        const existing = currentTickets[key] || null
        if (!existing && activeTicketCount >= Math.max(1, Number(maxActive || 1))) {
            callback(failure(qsTr("Too many Blockchain queries are pending.")), null)
            return null
        }

        nextGeneration += 1
        const clientRequestId = "chain-" + String(sourceEpoch) + "-" + String(nextGeneration)
        const request = operationRequest(normalized, clientRequestId)
        const expected = expectedIdentity(normalized, clientRequestId)
        if (!expected) {
            callback(failure(qsTr("Blockchain query arguments are invalid.")), null)
            return null
        }
        const ticket = {
            callerKey: key,
            sourceEpoch: sourceEpoch,
            generation: nextGeneration,
            clientRequestId: clientRequestId,
            method: normalized.method
        }
        setCurrentTicket(ticket)
        setPending(key, {
            ticket: ticket,
            request: request,
            expected: expected,
            callback: callback,
            operation: null,
            operationId: "",
            pollPending: false,
            pollToken: 0
        })

        if (!gateway || typeof gateway.startRuntimeOperation !== "function") {
            finishFailure(ticket, failure(qsTr("Runtime operation bridge is unavailable.")))
            return null
        }
        gateway.startRuntimeOperation(request, false, function (response) {
            acceptStartResponse(ticket, response)
        })
        return ticket
    }

    function poll() {
        const keys = Object.keys(pendingOperations || {})
        if (!keys.length || !gateway || typeof gateway.runtimeOperationStatus !== "function") {
            return 0
        }
        const limit = Math.max(1, Number(maxPollsPerTick || 1))
        const startIndex = pollCursor % keys.length
        let inspected = 0
        let started = 0
        while (inspected < keys.length && started < limit) {
            const key = keys[(startIndex + inspected) % keys.length]
            const entry = pendingOperations[key] || null
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

    function invalidateSource(reason) {
        const entries = pendingOperations || ({})
        const keys = Object.keys(entries)
        sourceEpoch += 1
        pendingOperations = ({})
        currentTickets = ({})
        pollCursor = 0
        for (let i = 0; i < keys.length; ++i) {
            const entry = entries[keys[i]] || null
            cancelEntry(entry)
            notifyInvalidated(entry, reason || qsTr("Blockchain source changed."))
        }
    }

    function invalidateCaller(callerKey, reason) {
        const key = String(callerKey || "")
        if (!key.length) {
            return false
        }
        const entry = pendingOperations[key] || null
        removePending(key)
        const tickets = copyMap(currentTickets)
        delete tickets[key]
        currentTickets = tickets
        cancelEntry(entry)
        notifyInvalidated(entry, reason || qsTr("Blockchain query was replaced."))
        return entry !== null
    }

    function callerPending(callerKey) {
        const key = String(callerKey || "")
        return key.length > 0 && pendingOperations[key] !== undefined
    }

    function isCurrent(ticket) {
        const value = ticket || null
        const key = String(value && value.callerKey || "")
        const current = key.length ? currentTickets[key] || null : null
        return current !== null
            && Number(value.sourceEpoch) === sourceEpoch
            && Number(current.sourceEpoch) === Number(value.sourceEpoch)
            && Number(current.generation) === Number(value.generation)
            && String(current.clientRequestId || "") === String(value.clientRequestId || "")
            && String(current.method || "") === String(value.method || "")
    }

    function release(ticket) {
        if (!isCurrent(ticket)) {
            return false
        }
        const key = String(ticket.callerKey || "")
        const tickets = copyMap(currentTickets)
        delete tickets[key]
        currentTickets = tickets
        return true
    }

    function normalizeCommand(command) {
        const value = command && typeof command === "object" ? command : ({})
        return {
            method: String(value.method || "").trim(),
            args: Array.isArray(value.args) ? value.args.slice(0) : [],
            label: String(value.label || value.method || qsTr("Blockchain query"))
        }
    }

    function operationRequest(command, clientRequestId) {
        return {
            domain: "blockchain",
            method: command.method,
            args: (Array.isArray(sourceArgs) ? sourceArgs.slice(0) : []).concat(command.args),
            label: command.label,
            configurationGeneration: Number(configurationGeneration),
            clientRequestId: clientRequestId
        }
    }

    function expectedIdentity(command, clientRequestId) {
        const source = normalizedSource(sourceArgs)
        if (!source) {
            return null
        }
        const context = {
            source: source.mode,
            configurationGeneration: Number(configurationGeneration)
        }
        if (source.endpoint.length) {
            context.endpoint = source.endpoint
        }
        switch (command.method) {
        case "blockchainNode":
            break
        case "blockchainBlocks":
            if (command.args.length < 2) {
                return null
            }
            const blockRange = BlockchainRangeValidation.validate(
                command.args[0], command.args[1])
            if (!blockRange.valid) {
                return null
            }
            context.slotFrom = blockRange.slotFrom
            context.slotTo = blockRange.slotTo
            context.slotRange = String(context.slotFrom) + ":" + String(context.slotTo)
            if (command.args.length > 2 && typeof command.args[2] === "number") {
                context.limit = normalizedInteger(command.args[2])
                if (context.limit === null) {
                    return null
                }
            }
            break
        case "blockchainLiveBlocks":
            if (command.args.length < 2) {
                return null
            }
            const liveRange = BlockchainRangeValidation.validate(
                command.args[0], command.args[1])
            if (!liveRange.valid) {
                return null
            }
            context.slotFrom = liveRange.slotFrom
            context.slotTo = liveRange.slotTo
            context.limit = command.args.length > 2 && typeof command.args[2] === "number"
                ? normalizedInteger(command.args[2]) : 50
            if (context.limit === null) {
                return null
            }
            context.slotRange = String(context.slotFrom) + ":" + String(context.slotTo)
            break
        case "blockchainBlock":
            context.blockId = String(command.args[0] || "").trim()
            if (!context.blockId.length) {
                return null
            }
            break
        case "blockchainTransaction":
            context.transactionId = String(command.args[0] || "").trim()
            if (!context.transactionId.length) {
                return null
            }
            break
        default:
            return null
        }
        return {
            domain: "blockchain",
            method: command.method,
            clientRequestId: clientRequestId,
            backend: source.mode,
            context: context
        }
    }

    function blockRangeValidation(command) {
        if (command.method !== "blockchainBlocks"
                && command.method !== "blockchainLiveBlocks") {
            return null
        }
        return BlockchainRangeValidation.validate(command.args[0], command.args[1])
    }

    function normalizedSource(values) {
        const args = Array.isArray(values) ? values : []
        const first = String(args[0] || "").trim()
        if (!first.length) {
            return null
        }
        if (first === "module" || first === "basecamp" || first === "basecamp-module"
                || first === "basecamp module") {
            return { mode: "module", endpoint: "" }
        }
        if (first === "logoscore_cli" || first === "logoscore-cli" || first === "logoscore cli") {
            return { mode: "logoscore_cli", endpoint: "" }
        }
        if (first === "rpc" || first === "direct-rpc" || first === "direct rpc"
                || first === "standalone" || first === "standalone-rpc"
                || first === "standalone rpc") {
            const endpoint = String(args[1] || "").trim()
            return endpoint.length ? { mode: "rpc", endpoint: endpoint } : null
        }
        return { mode: "rpc", endpoint: first }
    }

    function normalizedInteger(value) {
        const number = Number(value)
        return Number.isSafeInteger(number) && number >= 0 ? number : null
    }

    function acceptStartResponse(ticket, response) {
        const entry = pendingEntry(ticket)
        if (!entry) {
            cancelLateStart(ticket, response)
            return
        }
        if (!response || response.ok !== true) {
            finishFailure(ticket, response || failure(qsTr("Blockchain query failed to start.")))
            return
        }
        const operation = response.value || null
        if (!matchesIdentity(operation, entry.expected) || !knownStatus(operation.status)) {
            finishFailure(ticket, failure(qsTr("Blockchain query returned an invalid operation identity.")))
            return
        }
        if (isTerminal(operation)) {
            finishTerminal(ticket, operation)
            return
        }
        if (!isActive(operation)) {
            finishFailure(ticket, failure(qsTr("Blockchain query returned an invalid operation status.")))
            return
        }
        const next = copyEntry(entry)
        next.operation = operation
        next.operationId = String(operation.operationId || "")
        setPending(ticket.callerKey, next)
    }

    function cancelLateStart(ticket, response) {
        const operation = response && response.ok === true ? response.value || null : null
        if (!operation || !isActive(operation)
                || String(operation.domain || "") !== "blockchain"
                || String(operation.method || "") !== String(ticket && ticket.method || "")
                || String(operation.clientRequestId || "")
                    !== String(ticket && ticket.clientRequestId || "")) {
            return false
        }
        return cancelOperationId(operation.operationId)
    }

    function cancelEntry(entry) {
        return entry ? cancelOperationId(entry.operationId) : false
    }

    function cancelOperationId(operationId) {
        const id = String(operationId || "").trim()
        if (!id.length || !gateway || typeof gateway.runtimeOperationCancel !== "function") {
            return false
        }
        try {
            gateway.runtimeOperationCancel(id, false, function () {})
            return true
        } catch (error) {
            return false
        }
    }

    function notifyInvalidated(entry, reason) {
        if (!entry || typeof entry.callback !== "function") {
            return false
        }
        const response = failure(String(reason || qsTr("Blockchain source changed.")))
        response.invalidated = true
        try {
            entry.callback(response, entry.ticket)
            return true
        } catch (error) {
            return false
        }
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
        if (!matchesIdentity(operation, released.expected)
                || String(operation.operationId || "") !== String(operationId || "")
                || !OperationHistoryVocabulary.runtimeSnapshotIsNewer(released.operation, operation)) {
            return
        }
        if (!knownStatus(operation.status)) {
            finishFailure(ticket, failure(qsTr("Blockchain query returned an invalid operation status.")))
            return
        }
        if (isTerminal(operation)) {
            finishTerminal(ticket, operation)
            return
        }
        if (!isActive(operation)) {
            finishFailure(ticket, failure(qsTr("Blockchain query returned an invalid operation status.")))
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
        const validResult = completed && resultMatchesMethod(entry.expected.method, operation.result)
        const response = {
            ok: validResult,
            value: validResult ? operation.result : null,
            operation: operation,
            terminalStatus: String(operation.status || ""),
            error: validResult ? "" : String(operation.error
                || qsTr("Blockchain query did not return a valid completed result."))
        }
        deliver(entry, response)
    }

    function finishFailure(ticket, response) {
        const entry = pendingEntry(ticket)
        if (!entry) {
            return
        }
        removePending(ticket.callerKey)
        deliver(entry, response || failure(qsTr("Blockchain query failed.")))
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
        const entry = pendingOperations[String(ticket.callerKey || "")] || null
        return entry && Number(entry.ticket.generation) === Number(ticket.generation)
            ? entry : null
    }

    function matchesIdentity(operation, expected) {
        if (!operation || !String(operation.operationId || "").length) {
            return false
        }
        if (String(operation.domain || "") !== String(expected.domain || "")
                || String(operation.method || "") !== String(expected.method || "")
                || String(operation.clientRequestId || "") !== String(expected.clientRequestId || "")
                || String(operation.backend || "") !== String(expected.backend || "")) {
            return false
        }
        const actualContext = operation.context && typeof operation.context === "object"
            ? operation.context : null
        if (!actualContext) {
            return false
        }
        const keys = Object.keys(expected.context || {})
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            if (actualContext[key] !== expected.context[key]) {
                return false
            }
        }
        return true
    }

    function knownMethod(method) {
        const value = String(method || "")
        return value === "blockchainNode" || value === "blockchainBlocks"
            || value === "blockchainLiveBlocks" || value === "blockchainBlock"
            || value === "blockchainTransaction"
    }

    function knownStatus(status) {
        return isActive({ status: status }) || isTerminal({ status: status })
    }

    function isActive(operation) {
        const status = String(operation && operation.status || "")
        return status === "running" || status === "awaiting_external" || status === "canceling"
    }

    function isTerminal(operation) {
        return OperationHistoryVocabulary.isRuntimeTerminalStatus(operation && operation.status)
    }

    function resultMatchesMethod(method, result) {
        if (String(method || "") === "blockchainBlocks") {
            return Array.isArray(result)
        }
        return result !== null && typeof result === "object" && !Array.isArray(result)
    }

    function failure(error) {
        return { ok: false, value: null, error: String(error || "") }
    }

    function setCurrentTicket(ticket) {
        const tickets = copyMap(currentTickets)
        tickets[ticket.callerKey] = ticket
        currentTickets = tickets
    }

    function setPending(callerKey, entry) {
        const pending = copyMap(pendingOperations)
        pending[String(callerKey || "")] = entry
        pendingOperations = pending
    }

    function removePending(callerKey) {
        const key = String(callerKey || "")
        if (pendingOperations[key] === undefined) {
            return
        }
        const pending = copyMap(pendingOperations)
        delete pending[key]
        pendingOperations = pending
    }

    function copyEntry(entry) {
        const next = ({})
        const value = entry || ({})
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
