.import "../modules/ModuleEventEnvelope.js" as ModuleEventEnvelope
.import "StorageOperationContracts.js" as StorageOperationContracts

function applyStatusUpdate(root, operation) {
    const payload = operationPayload(operation && operation.result)
    if (!isModuleDispatchAck(operation, payload)) {
        return false
    }
    if (terminalizedOperation(root, operation)) {
        return true
    }
    const active = root.activeOperation || {}
    if (textValue(active.operationId).length && !sameOperationId(active, operation)) {
        return false
    }
    const next = mergeDispatchAck(storageModuleOperation(active, operation), payload)
    root.updateActiveOperation(next)
    root.lastOperation = qsTr("Running")
    return true
}

function storageModuleOperation(active, operation) {
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
    next.backend = next.backend || prior.backend || "module"
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

function sameOperationId(left, right) {
    const leftId = textValue(left && left.operationId)
    const rightId = textValue(right && right.operationId)
    return leftId.length > 0 && rightId.length > 0 && leftId === rightId
}

function terminalizedOperation(root, operation) {
    const operationId = textValue(operation && operation.operationId)
    return operationId.length > 0 && operationId === textValue(root.terminalOperationId)
}

function applyModuleEvent(root, eventName, args) {
    const name = String(eventName || "")
    const event = normalizeModuleEvent(name, args)
    if (!event.payload) {
        return false
    }
    if (!event.contract) {
        const success = event.success
        root.appendOperation(labelForEvent(name), {
            ok: success,
            value: event.payload,
            error: success ? "" : event.error
        })
        root.lastOperation = labelForEvent(name)
        return true
    }
    const active = root.activeOperation || {}
    if (!correlates(active, event)) {
        return false
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
    root.updateActiveOperation(operation)
    if (event.terminal) {
        root.appendOperation(labelForEvent(name), {
            ok: event.success,
            value: event.payload,
            error: operation.error
        })
        root.appendTerminalStorageOperation(operation)
        root.lastOperation = event.success ? qsTr("Complete") : qsTr("Stopped")
    } else {
        root.lastOperation = qsTr("Running")
    }
    return true
}

function correlates(operation, event) {
    if (!operation || !event || !event.contract) {
        return false
    }
    if (String(operation.domain || "") !== "storage") {
        return false
    }
    if (String(operation.backend || "").indexOf("module") < 0) {
        return false
    }
    const status = String(operation.status || "")
    if (status !== "running" && status !== "canceling") {
        return false
    }
    if (String(operation.method || "") !== event.contract.method) {
        return false
    }
    return matchesEventIdentity(operation, event)
}

function isModuleDispatchAck(operation, payload) {
    if (!operation || !payload || typeof payload !== "object") {
        return false
    }
    if (String(operation.domain || "") !== "storage") {
        return false
    }
    if (String(operation.status || "") !== "completed") {
        return false
    }
    if (String(operation.backend || "").indexOf("module") < 0) {
        return false
    }
    if (!StorageOperationContracts.eventContract(operation.method)) {
        return false
    }
    return payload.dispatched === true
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
        cid: textValue(payload && payload.cid),
        path: textValue(payload && payload.path),
        bytes: Number.isFinite(bytes) ? Math.max(0, bytes) : 0,
        total: Number.isFinite(total) && total > 0 ? total : 0
    }
}

function matchesEventIdentity(operation, event) {
    const operationSession = textValue(operation.externalSessionId || operation.sessionId)
    const operationCid = textValue(operation.cid || (operation.context && operation.context.cid))
    switch (String(event.contract.match || "")) {
    case "session":
        return operationSession.length ? operationSession === event.sessionId : event.sessionId.length > 0
    case "sessionOrCid":
        if (operationSession.length || event.sessionId.length) {
            return operationSession.length ? operationSession === event.sessionId : event.sessionId.length > 0
        }
        return operationCid.length > 0 && operationCid === event.cid
    case "cid":
        return operationCid.length > 0 && operationCid === event.cid
    default:
        return false
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
