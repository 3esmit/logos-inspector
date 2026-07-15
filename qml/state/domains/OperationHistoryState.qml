import QtQml
import "../OperationHistoryVocabulary.js" as OperationHistoryVocabulary
import "../runtime/RuntimeOperationPolicy.js" as RuntimeOperationPolicy

QtObject {
    id: root

    property var runtimeOperations: ({})
    property var runtimeOperationEventSeq: ({})
    property var runtimeOperationEventFacts: ({})
    property var runtimeOperationHistory: []
    property int runtimeOperationsRevision: 0
    property var runtimeOperationPollGenerations: ({})
    property var runtimeOperationPendingPolls: ({})
    property var runtimeOperationTerminalOrder: []
    property var runtimeOperationCursorOrder: []

    readonly property int maxProjectedActiveOperations: 64
    readonly property int maxTerminalOperations: 128
    readonly property int maxProjectedPendingPolls: 128
    readonly property int maxCursorEntries: maxProjectedActiveOperations + maxProjectedPendingPolls
    readonly property int maxHistoryRows: 100
    readonly property int maxDiagnosticPayloadBytes: 16 * 1024
    readonly property int maxDiagnosticTextBytes: 4 * 1024
    readonly property int pendingEventPollCount: Object.keys(runtimeOperationPendingPolls || {}).length

    function updateOperation(operation, historyReset) {
        const value = operation || null
        const operationId = String(value && value.operationId ? value.operationId : "")
        if (!validProjectionId(operationId)) {
            return false
        }
        const current = runtimeOperations[operationId] || null
        if (!knownOperationStatus(value.status)
                || (!current && OperationHistoryVocabulary.isRuntimeActiveStatus(value.status)
                    && activeOperationCount() >= maxProjectedActiveOperations)) {
            return false
        }
        const reset = historyReset === true
        if ((!reset && !OperationHistoryVocabulary.runtimeSnapshotIsNewer(current, value))
                || (reset && !historyResetIsCompatible(current, value))) {
            return false
        }
        const next = copyObject(runtimeOperations)
        next[operationId] = diagnosticOperation(value)
        const pruned = pruneTerminalOperations(next, operationId)
        runtimeOperations = pruned.operations
        runtimeOperationTerminalOrder = pruned.order
        forgetOperationIds(pruned.evicted)
        runtimeOperationsRevision += 1
        return true
    }

    function setEventSeq(operationId, seq) {
        const id = String(operationId || "")
        if (!validProjectionId(id)) {
            return false
        }
        const candidate = coercingSequence(seq)
        if (candidate === null) {
            return false
        }
        const currentValue = runtimeOperationEventSeq[id]
        const current = Number(currentValue)
        if (currentValue !== undefined && Number.isFinite(current) && candidate <= current) {
            return false
        }
        const next = copyObject(runtimeOperationEventSeq)
        next[id] = candidate
        runtimeOperationEventSeq = next
        rememberCursorId(id)
        runtimeOperationsRevision += 1
        return true
    }

    function beginEventPoll(operationId, afterSeq, context) {
        const id = String(operationId || "")
        const after = coercingSequence(afterSeq)
        if (!validProjectionId(id) || after === null
                || runtimeOperationPendingPolls[id] !== undefined
                || pendingEventPollCount >= maxProjectedPendingPolls) {
            return null
        }
        const previousGeneration = safeSequence(runtimeOperationPollGenerations[id])
        if (previousGeneration !== null && previousGeneration >= Number.MAX_SAFE_INTEGER) {
            return null
        }
        const generation = previousGeneration === null ? 1 : previousGeneration + 1
        const pollContext = normalizedPollContext(context)
        const ticket = {
            operationId: id,
            generation: generation,
            afterSeq: after,
            hostEpoch: pollContext.hostEpoch,
            hostIdentity: pollContext.hostIdentity,
            configurationIdentity: pollContext.configurationIdentity,
            backendIdentity: pollContext.backendIdentity,
            backendRevision: pollContext.backendRevision
        }
        const generations = copyObject(runtimeOperationPollGenerations)
        generations[id] = generation
        runtimeOperationPollGenerations = generations
        const pending = copyObject(runtimeOperationPendingPolls)
        pending[id] = ticket
        runtimeOperationPendingPolls = pending
        rememberCursorId(id)
        return ticket
    }

    function finishEventPoll(ticket, response, currentContext) {
        const value = ticket || null
        const operationId = String(value && value.operationId || "")
        const pending = operationId.length ? runtimeOperationPendingPolls[operationId] || null : null
        if (!samePollTicket(pending, value)) {
            return pollCompletion(false, true, false, "stale_poll")
        }
        releaseEventPoll(operationId, value.generation)
        if (!pollContextMatches(value, normalizedPollContext(currentContext))) {
            return pollCompletion(false, true, false, "stale_poll_context")
        }
        if (!response || response.ok !== true) {
            return pollCompletion(true, false, false, "")
        }
        const window = eventWindow(value, response.value)
        if (!window.valid) {
            return pollCompletion(false, false, true, window.error)
        }

        const operation = copyObject(window.operation)
        if (window.modern) {
            operation.oldestSeq = window.oldestSeq
            operation.nextSeq = window.nextSeq
            operation.droppedCount = window.droppedCount
            operation.coalescedCount = window.coalescedCount
            operation.retainedCount = window.retainedCount
            operation.retainedBytes = window.retainedBytes
            operation.historyTruncated = window.historyTruncated
        }
        if (window.resetRequired && !updateOperation(operation, true)) {
            return pollCompletion(false, false, true, "reset_snapshot")
        }
        if (!window.resetRequired) {
            const updated = updateOperation(operation, false)
            const current = runtimeOperations[operationId] || null
            if (!updated && !sameProjectedSnapshotIdentity(current, operation)) {
                return pollCompletion(false, false, true, "snapshot_rejected")
            }
        }
        reconcileEventSeq(operationId, window.eventCursor, window.resetRequired)
        setEventFacts(operationId, window)
        return pollCompletion(true, false, false, "")
    }

    function abandonEventPoll(ticket) {
        const value = ticket || null
        return releaseEventPoll(
            String(value && value.operationId || ""),
            value ? value.generation : null
        )
    }

    function eventPollPending(operationId) {
        const id = String(operationId || "")
        return id.length > 0 && runtimeOperationPendingPolls[id] !== undefined
    }

    function eventFacts(operationId) {
        return runtimeOperationEventFacts[String(operationId || "")] || null
    }

    function append(operation, detail) {
        const value = operation || {}
        const rows = Array.isArray(runtimeOperationHistory)
            ? runtimeOperationHistory.slice(-(maxHistoryRows - 1)) : []
        rows.push(historyRecord(value, detail))
        runtimeOperationHistory = rows
        runtimeOperationsRevision += 1
    }

    function rows(domain) {
        const revision = runtimeOperationsRevision
        const wanted = String(domain || "")
        const values = Array.isArray(runtimeOperationHistory) ? runtimeOperationHistory.slice(0) : []
        const filtered = wanted.length ? values.filter(row => String(row.domain || "") === wanted) : values
        return filtered.reverse()
    }

    function historyRecord(operation, detail) {
        const record = OperationHistoryVocabulary.historyRecord(
            operation || {},
            detail,
            new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        )
        const metadata = operationMetadata(operation || {})
        record.operationClass = metadata.operationClass
        record.affectedInputs = metadata.affectedInputs
        record.restartPolicy = metadata.restartPolicy
        record.confirmationRequired = metadata.confirmationRequired
        if (operation && operation.importId !== undefined) {
            record.importId = String(operation.importId || "")
        }
	        if (operation && operation.backupCatalogId !== undefined) {
	            record.backupCatalogId = String(operation.backupCatalogId || "")
	        }
	        if (operation && operation.previousOperationId !== undefined) {
	            record.previousOperationId = String(operation.previousOperationId || "")
	        }
	        if (operation && operation.restartOperationId !== undefined) {
	            record.restartOperationId = String(operation.restartOperationId || "")
	        }
	        if (operation && operation.reason !== undefined) {
	            record.reason = String(operation.reason || "")
	        }
        copyBoundedDiagnosticField(record, operation, "provenance")
        const conversationFields = [
            "clientRequestId",
            "bridgeCallbackId",
            "moduleSessionId",
            "moduleRequestId",
            "externalSessionId",
            "requestId",
            "eventCursor",
            "externalCorrelation",
            "terminalEventContract",
            "terminalReason"
        ]
        for (let i = 0; i < conversationFields.length; ++i) {
            copyBoundedDiagnosticField(record, operation, conversationFields[i])
        }
        if (operation && operation.result !== undefined && operation.result !== null) {
            markProjectionOmitted(record, "result", operation.result)
        }
        if (operation && operation.acknowledgement !== undefined
                && operation.acknowledgement !== null) {
            markProjectionOmitted(record, "acknowledgement", operation.acknowledgement)
        }
        const textFields = [
            "label", "detail", "domain", "method", "operationId", "operationClass",
            "restartPolicy", "importId", "backupCatalogId", "previousOperationId",
            "restartOperationId", "reason", "terminalReason"
        ]
        for (let i = 0; i < textFields.length; ++i) {
            boundDiagnosticText(record, textFields[i])
        }
        boundExistingDiagnosticValue(record, "affectedInputs")
        return record
    }

    function eventWindow(ticket, responseValue) {
        const response = responseValue && typeof responseValue === "object"
            && !Array.isArray(responseValue) ? responseValue : null
        const operation = response && response.operation
        if (!response || !operationMatchesTicket(operation, ticket)) {
            return invalidWindow("operation_identity")
        }
        const modernFields = [
            "oldestSeq", "nextSeq", "droppedCount", "coalescedCount",
            "retainedCount", "retainedBytes", "historyTruncated", "resetRequired"
        ]
        let modern = false
        for (let i = 0; i < modernFields.length; ++i) {
            if (response[modernFields[i]] !== undefined) {
                modern = true
                break
            }
        }
        if (!modern) {
            const legacyCursor = coercingSequence(response.eventCursor !== undefined
                ? response.eventCursor : response.nextSeq)
            if (legacyCursor === null || legacyCursor >= Number.MAX_SAFE_INTEGER
                    || legacyCursor < ticket.afterSeq) {
                return invalidWindow("legacy_cursor")
            }
            return {
                valid: true,
                modern: false,
                operation: operation,
                eventCursor: legacyCursor,
                resetRequired: false,
                oldestSeq: 0,
                nextSeq: legacyCursor + 1,
                droppedCount: 0,
                coalescedCount: 0,
                retainedCount: Array.isArray(response.events) ? response.events.length : 0,
                retainedBytes: 0,
                historyTruncated: false
            }
        }

        const oldestSeq = safeSequence(response.oldestSeq)
        const nextSeq = safeSequence(response.nextSeq)
        const eventCursor = safeSequence(response.eventCursor)
        const droppedCount = safeSequence(response.droppedCount)
        const coalescedCount = safeSequence(response.coalescedCount)
        const retainedCount = safeSequence(response.retainedCount)
        const retainedBytes = safeSequence(response.retainedBytes)
        if (oldestSeq === null || nextSeq === null || eventCursor === null
                || droppedCount === null || coalescedCount === null
                || retainedCount === null || retainedBytes === null
                || typeof response.historyTruncated !== "boolean"
                || typeof response.resetRequired !== "boolean"
                || !Array.isArray(response.events)
                || nextSeq < 1 || eventCursor + 1 !== nextSeq
                || oldestSeq > nextSeq
                || retainedCount !== nextSeq - oldestSeq
                || (retainedCount === 0 && oldestSeq !== nextSeq)
                || (retainedCount > 0 && (oldestSeq < 1 || oldestSeq >= nextSeq))) {
            return invalidWindow("cursor_facts")
        }
        const resetRequired = response.resetRequired === true
        const expectedReset = ticket.afterSeq < oldestSeq - 1
            || ticket.afterSeq >= nextSeq
        if (resetRequired !== expectedReset
                || !validEventWindowEntries(
                    response.events,
                    ticket,
                    oldestSeq,
                    nextSeq,
                    retainedCount,
                    resetRequired
                )) {
            return invalidWindow("cursor_window")
        }
        const operationCursor = safeSequence(operation.eventCursor)
        if (operationCursor === null || operationCursor !== eventCursor) {
            return invalidWindow("operation_cursor")
        }
        return {
            valid: true,
            modern: true,
            operation: operation,
            eventCursor: eventCursor,
            resetRequired: resetRequired,
            oldestSeq: oldestSeq,
            nextSeq: nextSeq,
            droppedCount: droppedCount,
            coalescedCount: coalescedCount,
            retainedCount: retainedCount,
            retainedBytes: retainedBytes,
            historyTruncated: response.historyTruncated === true
        }
    }

    function validEventWindowEntries(events, ticket, oldestSeq, nextSeq, retainedCount, resetRequired) {
        const expectedStart = resetRequired
            ? oldestSeq : Math.max(oldestSeq, ticket.afterSeq + 1)
        if (events.length !== nextSeq - expectedStart
                || (resetRequired && events.length !== retainedCount)) {
            return false
        }
        for (let i = 0; i < events.length; ++i) {
            const event = events[i] || null
            const seq = safeSequence(event && event.seq !== undefined
                ? event.seq : event && event.eventCursor)
            if (seq === null || seq < oldestSeq || seq >= nextSeq
                    || seq !== expectedStart + i
                    || (!resetRequired && seq <= ticket.afterSeq)
                    || (event && event.operationId !== undefined
                        && String(event.operationId || "") !== ticket.operationId)) {
                return false
            }
            if (event && event.seq !== undefined && event.eventCursor !== undefined
                    && safeSequence(event.eventCursor) !== seq) {
                return false
            }
        }
        return true
    }

    function setEventFacts(operationId, window) {
        const id = String(operationId || "")
        if (!id.length) {
            return
        }
        const next = copyObject(runtimeOperationEventFacts)
        next[id] = {
            oldestSeq: window.oldestSeq,
            nextSeq: window.nextSeq,
            eventCursor: window.eventCursor,
            droppedCount: window.droppedCount,
            coalescedCount: window.coalescedCount,
            retainedCount: window.retainedCount,
            retainedBytes: window.retainedBytes,
            historyTruncated: window.historyTruncated,
            resetRequired: window.resetRequired
        }
        runtimeOperationEventFacts = next
        rememberCursorId(id)
        runtimeOperationsRevision += 1
    }

    function reconcileEventSeq(operationId, seq, resetRequired) {
        const id = String(operationId || "")
        const candidate = safeSequence(seq)
        if (!id.length || candidate === null) {
            return false
        }
        if (resetRequired !== true) {
            const currentValue = runtimeOperationEventSeq[id]
            if (currentValue !== undefined && candidate <= Number(currentValue)) {
                return false
            }
        } else if (runtimeOperationEventSeq[id] !== undefined
                && candidate === Number(runtimeOperationEventSeq[id])) {
            return false
        }
        const next = copyObject(runtimeOperationEventSeq)
        next[id] = candidate
        runtimeOperationEventSeq = next
        rememberCursorId(id)
        runtimeOperationsRevision += 1
        return true
    }

    function releaseEventPoll(operationId, generation) {
        const id = String(operationId || "")
        const pending = id.length ? runtimeOperationPendingPolls[id] || null : null
        if (!pending || safeSequence(generation) === null
                || Number(pending.generation) !== Number(generation)) {
            return false
        }
        const next = copyObject(runtimeOperationPendingPolls)
        delete next[id]
        runtimeOperationPendingPolls = next
        return true
    }

    function samePollTicket(current, candidate) {
        return current !== null && candidate !== null
            && String(current.operationId || "") === String(candidate.operationId || "")
            && safeSequence(current.generation) !== null
            && Number(current.generation) === Number(candidate.generation)
            && Number(current.afterSeq) === Number(candidate.afterSeq)
    }

    function normalizedPollContext(context) {
        const value = context && typeof context === "object" ? context : ({})
        const hostEpoch = safeSequence(value.hostEpoch)
        return {
            hostEpoch: hostEpoch === null ? 0 : hostEpoch,
            hostIdentity: value.hostIdentity === undefined ? null : value.hostIdentity,
            configurationIdentity: String(value.configurationIdentity || ""),
            backendIdentity: String(value.backendIdentity || ""),
            backendRevision: String(value.backendRevision || "")
        }
    }

    function pollContextMatches(ticket, context) {
        return Number(ticket.hostEpoch) === Number(context.hostEpoch)
            && ticket.hostIdentity === context.hostIdentity
            && String(ticket.configurationIdentity || "") === context.configurationIdentity
            && String(ticket.backendIdentity || "") === context.backendIdentity
            && String(ticket.backendRevision || "") === context.backendRevision
    }

    function operationMatchesTicket(operation, ticket) {
        const value = operation && typeof operation === "object" ? operation : null
        if (!value || !knownOperationStatus(value.status)
                || String(value.operationId || "") !== String(ticket.operationId || "")) {
            return false
        }
        const expectedBackend = String(ticket.backendIdentity || "")
        const candidateBackend = operationBackendIdentity(value)
        if (expectedBackend.length && candidateBackend !== expectedBackend) {
            return false
        }
        const expectedRevision = String(ticket.backendRevision || "")
        const candidateRevision = operationBackendRevision(value)
        return !expectedRevision.length || candidateRevision === expectedRevision
    }

    function historyResetIsCompatible(current, candidate) {
        if (!current) {
            return true
        }
        const currentBackend = operationBackendIdentity(current)
        const candidateBackend = operationBackendIdentity(candidate)
        const sameBackend = !currentBackend.length || !candidateBackend.length
            || currentBackend === candidateBackend
        if (!sameBackend) {
            return false
        }
        if (OperationHistoryVocabulary.isRuntimeTerminalStatus(current.status)) {
            return OperationHistoryVocabulary.isRuntimeTerminalStatus(candidate.status)
                && String(current.status || "") === String(candidate.status || "")
        }
        return true
    }

    function sameProjectedSnapshotIdentity(current, candidate) {
        if (!current || !candidate) {
            return false
        }
        return String(current.operationId || "") === String(candidate.operationId || "")
            && String(current.status || "") === String(candidate.status || "")
            && safeSequence(current.eventCursor) !== null
            && Number(current.eventCursor) === Number(candidate.eventCursor)
            && operationBackendIdentity(current) === operationBackendIdentity(candidate)
    }

    function operationBackendIdentity(operation) {
        const value = operation || {}
        return JSON.stringify([
            String(value.backend || ""),
            String(value.domain || ""),
            String(value.method || ""),
            String(value.source || ""),
            String(value.endpoint || "")
        ])
    }

    function operationBackendRevision(operation) {
        const value = operation || {}
        if (value.projectionBackendRevision !== undefined) {
            return String(value.projectionBackendRevision || "")
        }
        const context = value.context && typeof value.context === "object"
            ? value.context : ({})
        return JSON.stringify([
            value.backendRevision === undefined ? null : value.backendRevision,
            value.backend_revision === undefined ? null : value.backend_revision,
            value.configurationRevision === undefined ? null : value.configurationRevision,
            context.backend_revision === undefined ? null : context.backend_revision,
            context.configuration_revision === undefined ? null : context.configuration_revision
        ])
    }

    function diagnosticOperation(operation) {
        const projected = ({})
        const allowedFields = [
            "operationId", "clientRequestId", "bridgeCallbackId", "moduleSessionId",
            "moduleRequestId", "externalCorrelation", "externalSessionId", "requestId",
            "eventCursor", "domain", "backend", "method", "status", "label",
            "policyFacts", "terminalEventContract", "acknowledgement", "progress",
            "bytesWritten", "contentLength", "result", "error", "terminalReason",
            "cancellable", "startedAt", "updatedAt", "context", "cid", "path",
            "endpoint", "source", "oldestSeq", "nextSeq", "droppedCount",
            "coalescedCount", "retainedCount", "retainedBytes", "historyTruncated",
            "terminalAt", "resultPurged", "acknowledgementPurged",
            "retainedPayloadBytes", "errorRedacted", "redactedErrorBytes",
            "importId", "backupCatalogId", "previousOperationId", "restartOperationId",
            "reason", "provenance", "operationClass", "affectedInputs",
            "restartPolicy", "confirmationRequired"
        ]
        for (let i = 0; i < allowedFields.length; ++i) {
            const field = allowedFields[i]
            if (operation && operation[field] !== undefined) {
                projected[field] = operation[field]
            }
        }
        projected.projectionBackendRevision = operationBackendRevision(operation)
        const terminal = OperationHistoryVocabulary.isRuntimeTerminalStatus(projected.status)
        const terminalPayloadFields = ["result", "acknowledgement"]
        for (let i = 0; i < terminalPayloadFields.length; ++i) {
            const field = terminalPayloadFields[i]
            if (projected[field] !== undefined && projected[field] !== null && terminal) {
                const fieldValue = projected[field]
                delete projected[field]
                markProjectionOmitted(projected, field, fieldValue)
            } else {
                boundExistingDiagnosticValue(projected, field)
            }
        }
        const valueFields = [
            "context", "policyFacts", "externalCorrelation",
            "terminalEventContract", "provenance", "affectedInputs"
        ]
        for (let i = 0; i < valueFields.length; ++i) {
            boundExistingDiagnosticValue(projected, valueFields[i])
        }
        const textFields = [
            "label", "error", "terminalReason", "cid", "path", "endpoint", "source",
            "importId", "backupCatalogId", "previousOperationId", "restartOperationId",
            "reason", "clientRequestId", "moduleSessionId", "moduleRequestId",
            "externalSessionId", "requestId"
        ]
        for (let i = 0; i < textFields.length; ++i) {
            boundDiagnosticText(projected, textFields[i])
        }
        return projected
    }

    function activeOperationCount() {
        const operations = runtimeOperations || ({})
        const ids = Object.keys(operations)
        let count = 0
        for (let i = 0; i < ids.length; ++i) {
            if (OperationHistoryVocabulary.isRuntimeActiveStatus(operations[ids[i]].status)) {
                count += 1
            }
        }
        return count
    }

    function knownOperationStatus(status) {
        return OperationHistoryVocabulary.isRuntimeActiveStatus(status)
            || OperationHistoryVocabulary.isRuntimeTerminalStatus(status)
    }

    function validProjectionId(operationId) {
        const value = String(operationId || "")
        return value.length > 0 && utf8ByteLength(value, maxDiagnosticTextBytes)
            <= maxDiagnosticTextBytes
    }

    function pruneTerminalOperations(operations, updatedOperationId) {
        const next = operations
        const known = ({})
        const order = []
        const previousOrder = Array.isArray(runtimeOperationTerminalOrder)
            ? runtimeOperationTerminalOrder : []
        for (let i = 0; i < previousOrder.length; ++i) {
            const id = String(previousOrder[i] || "")
            if (!id.length || known[id] || !OperationHistoryVocabulary.isRuntimeTerminalStatus(
                    next[id] && next[id].status)) {
                continue
            }
            known[id] = true
            order.push(id)
        }
        const updatedId = String(updatedOperationId || "")
        if (updatedId.length && OperationHistoryVocabulary.isRuntimeTerminalStatus(
                next[updatedId] && next[updatedId].status) && !known[updatedId]) {
            known[updatedId] = true
            order.push(updatedId)
        }
        const keys = Object.keys(next)
        for (let i = 0; i < keys.length; ++i) {
            const id = keys[i]
            if (!known[id] && OperationHistoryVocabulary.isRuntimeTerminalStatus(next[id].status)) {
                known[id] = true
                order.push(id)
            }
        }
        const evicted = []
        while (order.length > maxTerminalOperations) {
            let evictIndex = -1
            for (let i = 0; i < order.length; ++i) {
                if (runtimeOperationPendingPolls[order[i]] === undefined) {
                    evictIndex = i
                    break
                }
            }
            if (evictIndex < 0) {
                break
            }
            const id = order.splice(evictIndex, 1)[0]
            delete next[id]
            evicted.push(id)
        }
        return { operations: next, order: order, evicted: evicted }
    }

    function rememberCursorId(operationId) {
        const id = String(operationId || "")
        if (!id.length) {
            return
        }
        const current = Array.isArray(runtimeOperationCursorOrder)
            ? runtimeOperationCursorOrder : []
        const order = current.filter(candidate => String(candidate || "") !== id)
        order.push(id)
        const forgotten = []
        while (order.length > maxCursorEntries) {
            let evictIndex = -1
            for (let i = 0; i < order.length; ++i) {
                const candidate = order[i]
                const operation = runtimeOperations[candidate] || null
                if (!OperationHistoryVocabulary.isRuntimeActiveStatus(operation && operation.status)
                        && runtimeOperationPendingPolls[candidate] === undefined) {
                    evictIndex = i
                    break
                }
            }
            if (evictIndex < 0) {
                break
            }
            forgotten.push(order.splice(evictIndex, 1)[0])
        }
        runtimeOperationCursorOrder = order
        forgetOperationIds(forgotten)
    }

    function forgetOperationIds(operationIds) {
        const ids = Array.isArray(operationIds) ? operationIds : []
        if (!ids.length) {
            return
        }
        const cursors = copyObject(runtimeOperationEventSeq)
        const facts = copyObject(runtimeOperationEventFacts)
        const generations = copyObject(runtimeOperationPollGenerations)
        const pending = copyObject(runtimeOperationPendingPolls)
        for (let i = 0; i < ids.length; ++i) {
            const id = String(ids[i] || "")
            delete cursors[id]
            delete facts[id]
            delete generations[id]
            delete pending[id]
        }
        runtimeOperationEventSeq = cursors
        runtimeOperationEventFacts = facts
        runtimeOperationPollGenerations = generations
        runtimeOperationPendingPolls = pending
        const forgotten = ({})
        for (let i = 0; i < ids.length; ++i) {
            forgotten[String(ids[i] || "")] = true
        }
        runtimeOperationCursorOrder = (Array.isArray(runtimeOperationCursorOrder)
            ? runtimeOperationCursorOrder : []).filter(id => !forgotten[String(id || "")])
    }

    function copyBoundedDiagnosticField(target, source, field) {
        if (!source || source[field] === undefined) {
            return
        }
        target[field] = source[field]
        boundExistingDiagnosticValue(target, field)
    }

    function boundExistingDiagnosticValue(target, field) {
        if (!target || target[field] === undefined || target[field] === null) {
            return
        }
        if (typeof target[field] === "string") {
            boundDiagnosticText(target, field)
            return
        }
        if (typeof target[field] !== "object") {
            return
        }
        const bytes = serializedByteSize(target[field])
        if (bytes <= maxDiagnosticPayloadBytes) {
            return
        }
        delete target[field]
        markProjectionOmitted(target, field, null, bytes)
    }

    function markProjectionOmitted(target, field, value, measuredBytes) {
        const bytes = measuredBytes === undefined
            ? serializedByteSize(value) : Number(measuredBytes)
        target[field + "ProjectionOmitted"] = true
        target[field + "ProjectionBytes"] = Number.isSafeInteger(bytes)
            ? bytes : Number.MAX_SAFE_INTEGER
    }

    function boundDiagnosticText(target, field) {
        if (!target || typeof target[field] !== "string") {
            return
        }
        const text = target[field]
        const bytes = utf8ByteLength(text)
        if (bytes <= maxDiagnosticTextBytes) {
            return
        }
        target[field] = utf8Prefix(text, maxDiagnosticTextBytes)
        target[field + "ProjectionTruncated"] = true
        target[field + "ProjectionOriginalBytes"] = bytes
    }

    function serializedByteSize(value) {
        try {
            return utf8ByteLength(JSON.stringify(value))
        } catch (error) {
            return Number.MAX_SAFE_INTEGER
        }
    }

    function utf8ByteLength(text, stopAfter) {
        const value = String(text || "")
        const limit = Number(stopAfter)
        let bytes = 0
        for (let i = 0; i < value.length; ++i) {
            const codeUnit = value.charCodeAt(i)
            if (codeUnit <= 0x7f) {
                bytes += 1
            } else if (codeUnit <= 0x7ff) {
                bytes += 2
            } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff
                    && i + 1 < value.length) {
                const nextCodeUnit = value.charCodeAt(i + 1)
                if (nextCodeUnit >= 0xdc00 && nextCodeUnit <= 0xdfff) {
                    bytes += 4
                    i += 1
                } else {
                    bytes += 3
                }
            } else {
                bytes += 3
            }
            if (Number.isFinite(limit) && bytes > limit) {
                return bytes
            }
        }
        return bytes
    }

    function utf8Prefix(text, maxBytes) {
        const value = String(text || "")
        const limit = Math.max(0, Number(maxBytes || 0))
        let bytes = 0
        let end = 0
        for (let i = 0; i < value.length; ++i) {
            const codeUnit = value.charCodeAt(i)
            let codeUnitCount = 1
            let characterBytes = 3
            if (codeUnit <= 0x7f) {
                characterBytes = 1
            } else if (codeUnit <= 0x7ff) {
                characterBytes = 2
            } else if (codeUnit >= 0xd800 && codeUnit <= 0xdbff
                    && i + 1 < value.length) {
                const nextCodeUnit = value.charCodeAt(i + 1)
                if (nextCodeUnit >= 0xdc00 && nextCodeUnit <= 0xdfff) {
                    characterBytes = 4
                    codeUnitCount = 2
                }
            }
            if (bytes + characterBytes > limit) {
                break
            }
            bytes += characterBytes
            end = i + codeUnitCount
            i += codeUnitCount - 1
        }
        return value.slice(0, end)
    }

    function safeSequence(value) {
        return typeof value === "number" && Number.isSafeInteger(value) && value >= 0
            ? value : null
    }

    function coercingSequence(value) {
        if (value === undefined || value === null || value === "" || typeof value === "boolean") {
            return null
        }
        return safeSequence(Number(value))
    }

    function invalidWindow(error) {
        return { valid: false, error: String(error || "invalid_event_window") }
    }

    function pollCompletion(accepted, stale, invalid, error) {
        return {
            accepted: accepted === true,
            stale: stale === true,
            invalid: invalid === true,
            error: String(error || "")
        }
    }

    function operationMetadata(operation) {
        return RuntimeOperationPolicy.operationMetadata(operation || {})
    }

    function explicitAffectedInputs(operation) {
        return RuntimeOperationPolicy.explicitAffectedInputs(operation || {})
    }

    function classifyOperation(operation) {
        return RuntimeOperationPolicy.classifyOperation(operation || {})
    }

    function metadata(operationClass, affectedInputs, restartPolicy, confirmationRequired) {
        return RuntimeOperationPolicy.metadata(operationClass, affectedInputs, restartPolicy, confirmationRequired)
    }

    function affectedInputs(operation) {
        return RuntimeOperationPolicy.affectedInputs(operation || {})
    }

    function pushInput(inputs, key, value) {
        return RuntimeOperationPolicy.pushInput(inputs, key, value)
    }

    function copyObject(value) {
        const next = ({})
        const source = value && typeof value === "object" && !Array.isArray(value) ? value : ({})
        const keys = Object.keys(source)
        for (let i = 0; i < keys.length; ++i) {
            next[keys[i]] = source[keys[i]]
        }
        return next
    }
}
