.import "../../services/BridgeHelpers.js" as BridgeHelpers

function applyModuleEvent(root, eventName, args) {
    const name = String(eventName || "")
    const payload = eventPayload(args)
    if (!payload || typeof payload !== "object") {
        return false
    }
    const sessionId = String(payload.sessionId || payload.session_id || payload.id || "")
    const success = payload.success !== false && !String(payload.error || "").length
    const terminal = name === "storageUploadDone"
        || name === "storageDownloadDone"
        || name === "storageDownloadManifestDone"
        || name === "storageRemoveDone"
    const progress = name === "storageUploadProgress" || name === "storageDownloadProgress"
    if (!terminal && !progress) {
        root.appendOperation(labelForEvent(name), {
            ok: success,
            value: payload,
            error: success ? "" : String(payload.error || qsTr("Storage module event failed."))
        })
        root.lastOperation = labelForEvent(name)
        return true
    }
    const operation = copyOperation(root.activeOperation || {})
    operation.operationId = String(operation.operationId || (sessionId.length ? "storage-module-" + sessionId : "storage-module-" + name))
    operation.domain = "storage"
    operation.backend = "module"
    operation.method = operation.method || methodForEvent(name)
    operation.label = operation.label || labelForEvent(name)
    operation.externalSessionId = sessionId || operation.externalSessionId || ""
    operation.status = terminal ? (success ? "completed" : "failed") : "running"
    operation.cancellable = false
    if (payload.cid !== undefined && payload.cid !== null) {
        operation.cid = String(payload.cid)
    }
    if (payload.path !== undefined && payload.path !== null) {
        operation.path = String(payload.path)
    }
    const bytes = Number(payload.bytes || payload.byteCount || payload.byte_count || 0)
    const total = Number(payload.totalBytes || payload.total_bytes || payload.contentLength || payload.content_length || 0)
    const prior = Number(operation.bytesWritten || 0)
    operation.bytesWritten = progress ? prior + (Number.isFinite(bytes) ? Math.max(0, bytes) : 0) : prior
    if (terminal && Number.isFinite(bytes) && bytes > operation.bytesWritten) {
        operation.bytesWritten = bytes
    }
    if (Number.isFinite(total) && total > 0) {
        operation.contentLength = total
        operation.progress = Math.max(0, Math.min(1, Number(operation.bytesWritten || 0) / total))
    } else if (terminal && success) {
        operation.progress = 1
    }
    operation.result = terminal ? payload : (operation.result || null)
    operation.error = success ? "" : String(payload.error || qsTr("Storage module event failed."))
    root.updateActiveOperation(operation)
    root.appendOperation(labelForEvent(name), {
        ok: success,
        value: payload,
        error: operation.error
    })
    if (terminal) {
        root.appendTerminalStorageOperation(operation)
        root.lastOperation = success ? qsTr("Complete") : qsTr("Stopped")
    } else {
        root.lastOperation = qsTr("Running")
    }
    return true
}

function spaceSummary(root) {
    const value = spaceValue(root)
    if (!value) {
        return qsTr("n/a")
    }
    const used = Number(value.quotaUsedBytes || value.quota_used_bytes || value.usedBytes || value.used_bytes || 0)
    const max = Number(value.quotaMaxBytes || value.quota_max_bytes || value.maxBytes || value.max_bytes || 0)
    if (Number.isFinite(max) && max > 0) {
        const percent = Math.floor(Math.max(0, Math.min(100, (used / max) * 100)))
        return qsTr("%1% used").arg(percent)
    }
    if (Number.isFinite(used) && used > 0) {
        return qsTr("%1 bytes").arg(root.valueText(used))
    }
    return qsTr("available")
}

function spaceTone(root) {
    const value = spaceValue(root)
    if (!value) {
        return "neutral"
    }
    const used = Number(value.quotaUsedBytes || value.quota_used_bytes || 0)
    const max = Number(value.quotaMaxBytes || value.quota_max_bytes || 0)
    if (!Number.isFinite(max) || max <= 0) {
        return "success"
    }
    const ratio = used / max
    if (ratio >= 0.9) {
        return "error"
    }
    if (ratio >= 0.75) {
        return "warning"
    }
    return "success"
}

function spaceValue(root) {
    const report = root.sourceReport || {}
    const probes = Array.isArray(report.probe_facts) ? report.probe_facts : []
    for (let i = 0; i < probes.length; ++i) {
        const probe = probes[i] || {}
        if (String(probe.key || probe.probe_key || "") === "space" && probe.ok === true) {
            return operationPayload(probe.value)
        }
    }
    const rawProbes = Array.isArray(report.probes) ? report.probes : []
    for (let j = 0; j < rawProbes.length; ++j) {
        const raw = rawProbes[j] || {}
        const key = String(raw.probe_key || "")
        const label = String(raw.label || "")
        if ((key === "space" || label.indexOf(".space") >= 0) && raw.ok === true) {
            return operationPayload(raw.value)
        }
    }
    return null
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
    const values = Array.isArray(args) ? args : (args === undefined || args === null ? [] : [args])
    const raw = values.length > 0 ? values[0] : args
    if (raw && typeof raw === "object") {
        return raw
    }
    const text = String(raw || "").trim()
    if (!text.length) {
        return null
    }
    const parsed = BridgeHelpers.parseJson(text)
    if (parsed.ok && parsed.value && typeof parsed.value === "object") {
        return parsed.value
    }
    return {
        value: text
    }
}

function methodForEvent(eventName) {
    switch (String(eventName || "")) {
    case "storageUploadProgress":
    case "storageUploadDone":
        return "storageUploadUrl"
    case "storageDownloadProgress":
    case "storageDownloadDone":
        return "storageDownloadToUrl"
    case "storageDownloadManifestDone":
        return "storageDownloadManifest"
    case "storageRemoveDone":
        return "storageRemove"
    default:
        return String(eventName || "storageModuleEvent")
    }
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
