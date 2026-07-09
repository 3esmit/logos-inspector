.import "StorageOperationContracts.js" as StorageOperationContracts

function sameOperationId(left, right) {
    const leftId = textValue(left && left.operationId)
    const rightId = textValue(right && right.operationId)
    return leftId.length > 0 && rightId.length > 0 && leftId === rightId
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

function matchesEventIdentity(operation, event) {
    const operationCid = textValue(operation.cid || (operation.context && operation.context.cid))
    switch (String(event.contract.match || "")) {
    case "session":
        return matchesEventSession(operation, event)
    case "sessionOrCid":
        if (hasEventSessionIdentity(operation, event)) {
            return matchesEventSession(operation, event)
        }
        return operationCid.length > 0 && operationCid === event.cid
    case "cid":
        return operationCid.length > 0 && operationCid === event.cid
    default:
        return false
    }
}

function matchesEventSession(operation, event) {
    const operationSession = textValue(operation.externalSessionId || operation.sessionId)
    const operationRequest = textValue(operation.requestId || operation.request_id)
    if (operationSession.length && event.sessionId.length) {
        return operationSession === event.sessionId
    }
    if (operationRequest.length && event.requestId.length) {
        return operationRequest === event.requestId
    }
    return !operationSession.length && !operationRequest.length && (event.sessionId.length > 0 || event.requestId.length > 0)
}

function hasEventSessionIdentity(operation, event) {
    return textValue(operation.externalSessionId || operation.sessionId).length > 0
            || event.sessionId.length > 0
            || event.requestId.length > 0
}

function textValue(value) {
    if (value === undefined || value === null) {
        return ""
    }
    return String(value)
}
