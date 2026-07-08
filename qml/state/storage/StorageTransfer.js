.import "StorageOperationLifecycle.js" as StorageOperationLifecycle

function applyStatusUpdate(root, operation) {
    return StorageOperationLifecycle.applyStatusUpdate(root, operation)
}

function applyModuleEvent(root, eventName, args) {
    return StorageOperationLifecycle.applyModuleEvent(root, eventName, args)
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
