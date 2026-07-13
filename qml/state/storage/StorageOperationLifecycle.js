.import "../modules/ModuleEventEnvelope.js" as ModuleEventEnvelope
.import "StorageOperationContracts.js" as StorageOperationContracts
.import "StorageOperationCorrelation.js" as StorageOperationCorrelation

function applyStatusUpdate(root, operation) {
    return commitReduction(root, reduceRuntimeStatus(root, operation))
}

function reduceRuntimeStatus(root, operation) {
    if (!operation || typeof operation !== "object") {
        return ignored()
    }
    if (String(operation.domain || "") !== "storage") {
        return ignored()
    }
    const payload = operationPayload(operation && operation.result)
    if (StorageOperationCorrelation.isModuleDispatchAck(operation, payload)) {
        return reduceDispatchAck(root, operation, payload)
    }
    return reduceRuntimeOperation(root, operation)
}

function reduceDispatchAck(root, operation, payload) {
    if (terminalizedOperation(root, operation)) {
        return handled()
    }
    const active = root.activeOperation || {}
    if (textValue(active.operationId).length && !StorageOperationCorrelation.sameOperationId(active, operation)) {
        return ignored()
    }
    const next = mergeDispatchAck(storageModuleOperation(active, operation), payload)
    return activeReduction(next, qsTr("Running"))
}

function reduceRuntimeOperation(root, operation) {
    const active = root.activeOperation || {}
    if (textValue(active.operationId).length && !StorageOperationCorrelation.sameOperationId(active, operation)) {
        return ignored()
    }
    const next = runtimeOperation(active, operation)
    if (runtimeTerminal(next)) {
        return terminalReduction(next)
    }
    return activeReduction(next, runningStatusText(next))
}

function storageModuleOperation(active, operation) {
    const prior = active || {}
    const next = operationWithCarriedFields(active, operation)
    next.backend = next.backend || prior.backend || "module"
    return next
}

function runtimeOperation(active, operation) {
    return operationWithCarriedFields(active, operation)
}

function operationWithCarriedFields(active, operation) {
    const next = copyOperation(operation)
    const prior = active || {}
    const carry = [
        "label",
        "cid",
        "path",
        "contentLength",
        "bytesWritten",
        "externalSessionId",
        "requestId"
    ]
    for (let i = 0; i < carry.length; ++i) {
        const key = carry[i]
        if ((next[key] === undefined || next[key] === null || String(next[key] || "").length === 0) && prior[key] !== undefined) {
            next[key] = prior[key]
        }
    }
    return next
}

function mergeDispatchAck(operation, payload) {
    const next = copyOperation(operation)
    next.status = "running"
    next.cancellable = false
    next.progress = 0
    next.error = ""
    next.result = payload
    next.externalSessionId = textValue(payload.sessionId || payload.session_id || payload.id || next.externalSessionId)
    const requestId = textValue(payload.requestId || payload.request_id)
    if (requestId.length) {
        next.requestId = requestId
    }
    const cid = textValue(payload.cid || next.cid)
    if (cid.length) {
        next.cid = cid
    }
    const path = textValue(payload.path || next.path)
    if (path.length) {
        next.path = path
    }
    return next
}

function terminalizedOperation(root, operation) {
    const operationId = textValue(operation && operation.operationId)
    if (!operationId.length) {
        return false
    }
    if (operationId === textValue(root.terminalOperationId)) {
        return true
    }
    const active = root.activeOperation || {}
    if (!StorageOperationCorrelation.sameOperationId(active, operation) || !runtimeTerminal(active)) {
        return false
    }
    return !StorageOperationCorrelation.isModuleDispatchAck(active, operationPayload(active.result))
}

function applyModuleEvent(root, eventName, args) {
    return commitReduction(root, reduceModuleEvent(root, eventName, args))
}

function reduceModuleEvent(root, eventName, args) {
    const name = String(eventName || "")
    const event = normalizeModuleEvent(name, args)
    if (!event.payload) {
        return ignored()
    }
    if (!event.contract) {
        const success = event.success
        return logReduction(labelForEvent(name), {
            ok: success,
            value: event.payload,
            error: success ? "" : event.error
        }, labelForEvent(name))
    }
    const active = root.activeOperation || {}
    if (!StorageOperationCorrelation.correlates(active, event)) {
        return ignored()
    }

    const operation = copyOperation(active)
    operation.operationId = String(operation.operationId || (event.sessionId.length ? "storage-module-" + event.sessionId : "storage-module-" + name))
    operation.domain = "storage"
    operation.backend = operation.backend || "module"
    operation.method = operation.method || event.contract.method
    operation.label = operation.label || labelForEvent(name)
    operation.cancellable = false
    if (!String(operation.externalSessionId || "").length && event.sessionId.length) {
        operation.externalSessionId = event.sessionId
    }
    if (!String(operation.requestId || "").length && event.requestId.length) {
        operation.requestId = event.requestId
    }
    if (event.cid.length) {
        operation.cid = event.cid
    }
    if (event.path.length) {
        operation.path = event.path
    }
    applyEventProgress(operation, event)
    operation.status = event.terminal ? (event.success ? "completed" : "failed") : "running"
    operation.result = event.terminal ? event.payload : (operation.result || null)
    operation.error = event.success ? "" : event.error
    if (event.terminal) {
        return terminalReduction(operation)
    }
    return activeReduction(operation, qsTr("Running"))
}

function normalizeModuleEvent(eventName, args) {
    const payload = eventPayload(args)
    const contract = StorageOperationContracts.contractForEvent(eventName)
    const error = textValue(payload && payload.error ? payload.error : "")
    const success = payload && payload.success !== false && error.length === 0
    const bytes = Number(payload && (payload.bytes || payload.byteCount || payload.byte_count || 0))
    const total = Number(payload && (payload.totalBytes || payload.total_bytes || payload.contentLength || payload.content_length || 0))
    return {
        name: String(eventName || ""),
        payload: payload,
        contract: contract,
        progress: contract && contract.progress === eventName,
        terminal: contract && contract.terminal === eventName,
        success: success,
        error: success ? "" : (error.length ? error : qsTr("Storage module event failed.")),
        sessionId: textValue(payload && (payload.sessionId || payload.session_id || payload.id)),
        requestId: textValue(payload && (payload.requestId || payload.request_id)),
        cid: textValue(payload && payload.cid),
        path: textValue(payload && payload.path),
        bytes: Number.isFinite(bytes) ? Math.max(0, bytes) : 0,
        total: Number.isFinite(total) && total > 0 ? total : 0
    }
}

function applyEventProgress(operation, event) {
    const prior = Number(operation.bytesWritten || 0)
    if (event.progress) {
        operation.bytesWritten = prior + event.bytes
    }
    if (event.terminal && event.bytes > Number(operation.bytesWritten || 0)) {
        operation.bytesWritten = event.bytes
    }
    if (event.total > 0) {
        operation.contentLength = event.total
        operation.progress = Math.max(0, Math.min(1, Number(operation.bytesWritten || 0) / event.total))
    } else if (event.terminal && event.success) {
        operation.progress = 1
    }
}

function commitReduction(root, reduction) {
    if (!reduction || reduction.handled !== true) {
        return false
    }
    if (reduction.operation) {
        root.acceptUpdate(reduction.operation)
    }
    if (reduction.logLabel) {
        root.appendResult(reduction.logLabel, reduction.logResponse)
    }
    if (reduction.terminal === true && reduction.operation) {
        root.acceptTerminal(reduction.operation)
    }
    if (reduction.lastOperation !== undefined) {
        root.lastOperation = reduction.lastOperation
    } else if (reduction.terminal === true && reduction.operation) {
        root.lastOperation = terminalStatusText(reduction.operation)
    }
    return true
}

function ignored() {
    return { handled: false }
}

function handled() {
    return { handled: true }
}

function activeReduction(operation, lastOperation) {
    return {
        handled: true,
        operation: operation,
        terminal: false,
        lastOperation: lastOperation
    }
}

function terminalReduction(operation) {
    return {
        handled: true,
        operation: operation,
        terminal: true
    }
}

function logReduction(label, response, lastOperation) {
    return {
        handled: true,
        logLabel: label,
        logResponse: response,
        lastOperation: lastOperation
    }
}

function runtimeTerminal(operation) {
    const status = String(operation && operation.status ? operation.status : "")
    return status === "completed" || status === "failed" || status === "canceled"
}

function runningStatusText(operation) {
    const status = String(operation && operation.status ? operation.status : "")
    return status === "canceling" ? qsTr("Canceling") : qsTr("Running")
}

function terminalStatusText(operation) {
    return String(operation && operation.status ? operation.status : "") === "completed" ? qsTr("Complete") : qsTr("Stopped")
}

function textValue(value) {
    if (value === undefined || value === null) {
        return ""
    }
    return String(value)
}

function operationPayload(value) {
    if (value && value.value && value.value.result && value.value.result.value !== undefined) {
        return value.value.result.value
    }
    if (value && value.result && value.result.value !== undefined) {
        return value.result.value
    }
    if (value && value.result !== undefined && value.result !== null) {
        return value.result
    }
    if (value && value.value !== undefined) {
        return value.value
    }
    return value
}

function eventPayload(args) {
    return ModuleEventEnvelope.storagePayload(args)
}

function labelForEvent(eventName) {
    switch (String(eventName || "")) {
    case "storageUploadProgress":
        return qsTr("Upload progress")
    case "storageUploadDone":
        return qsTr("Upload done")
    case "storageDownloadProgress":
        return qsTr("Download progress")
    case "storageDownloadDone":
        return qsTr("Download done")
    case "storageDownloadManifestDone":
        return qsTr("Manifest done")
    case "storageRemoveDone":
        return qsTr("Remove done")
    case "storageStart":
        return qsTr("Storage start")
    case "storageStop":
        return qsTr("Storage stop")
    case "storageConnect":
        return qsTr("Storage connect")
    default:
        return qsTr("Storage event")
    }
}

function copyOperation(value) {
    const next = {}
    const source = value || {}
    for (const key in source) {
        next[key] = source[key]
    }
    return next
}
