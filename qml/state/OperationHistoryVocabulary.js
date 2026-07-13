function isRuntimeTerminalStatus(status) {
    const value = String(status || "")
    return value === "completed"
        || value === "dispatched"
        || value === "failed"
        || value === "canceled"
        || value === "timed_out"
}

function isRuntimeActiveStatus(status) {
    const value = String(status || "")
    return value === "running" || value === "awaiting_external" || value === "canceling"
}

function isRuntimeSuccessfulTerminalStatus(status) {
    const value = String(status || "")
    return value === "completed" || value === "dispatched"
}

function runtimeSnapshotIsNewer(current, candidate) {
    const next = candidate && typeof candidate === "object" ? candidate : null
    if (!next) {
        return false
    }
    const previous = current && typeof current === "object" ? current : null
    if (!previous) {
        return true
    }
    const previousId = String(previous.operationId || "")
    const nextId = String(next.operationId || "")
    if (previousId.length && nextId.length && previousId !== nextId) {
        return true
    }
    if (isRuntimeTerminalStatus(previous.status)) {
        return false
    }

    const previousCursor = runtimeEventCursor(previous)
    const nextCursor = runtimeEventCursor(next)
    if (previousCursor !== null && nextCursor === null) {
        return false
    }
    if (previousCursor === null || nextCursor === null) {
        return true
    }
    return nextCursor > previousCursor
}

function runtimeEventCursor(operation) {
    const value = operation && operation.eventCursor
    if (value === undefined || value === null || value === "" || typeof value === "boolean") {
        return null
    }
    const cursor = Number(value)
    return Number.isFinite(cursor) && cursor >= 0 ? cursor : null
}

function runtimeStatusText(operation, defaultLabel) {
    const value = operation || {}
    const status = String(value.status || "")
    switch (status) {
    case "running":
        return String(value.label || defaultLabel || qsTr("Running"))
    case "awaiting_external":
        return qsTr("Waiting for completion")
    case "canceling":
        return qsTr("Canceling")
    case "completed":
        return qsTr("Complete")
    case "dispatched":
        return qsTr("Dispatched")
    case "failed":
        return qsTr("Failed")
    case "canceled":
        return qsTr("Canceled")
    case "timed_out":
        return qsTr("Timed out")
    default:
        return qsTr("Idle")
    }
}

function runtimeTone(operation) {
    const status = String(operation && operation.status ? operation.status : "")
    if (status === "completed") {
        return "success"
    }
    if (status === "failed" || status === "timed_out") {
        return "error"
    }
    if (status === "running" || status === "awaiting_external" || status === "canceling" || status === "dispatched") {
        return "warning"
    }
    return "neutral"
}

function syntheticHistoryStatus(status) {
    const value = String(status || "").toLowerCase()
    if (value === "down" || value === "failed" || value === "error") {
        return "failed"
    }
    if (value === "canceled" || value === "cancelled") {
        return "canceled"
    }
    return "completed"
}

function historyDetail(operation) {
    const value = operation || {}
    const result = value.result
    if (result && typeof result === "object") {
        if (result.cid) {
            return String(result.cid)
        }
        if (result.contentTopic) {
            return String(result.contentTopic)
        }
        if (result.status) {
            return String(result.status)
        }
    }
    if (value.error) {
        return String(value.error)
    }
    if (value.progress !== undefined && value.progress !== null) {
        return String(Math.floor(Number(value.progress || 0) * 100)) + "%"
    }
    return String(value.method || "")
}

function historyRecord(operation, detail, timeText) {
    const value = operation || {}
    return {
        time: String(timeText || ""),
        label: String(value.label || value.method || qsTr("Operation")),
        status: String(value.status || ""),
        detail: String(detail || historyDetail(value)),
        domain: String(value.domain || ""),
        method: String(value.method || ""),
        operationId: String(value.operationId || "")
    }
}
