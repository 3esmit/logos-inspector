function isRuntimeTerminalStatus(status) {
    const value = String(status || "")
    return value === "completed" || value === "failed" || value === "canceled"
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
