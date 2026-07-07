function networkConnectionSummary(root, kind, value) {
    if (kind === "blockchain") {
        const probe = value && value.cryptarchia_info ? value.cryptarchia_info : null
        const payload = probe && probe.value ? probe.value : probe
        const info = payload && payload.cryptarchia_info ? payload.cryptarchia_info : payload
        return info && info.slot !== undefined ? qsTr("slot %1").arg(info.slot) : qsTr("node reachable")
    }
    if (kind === "indexer") {
        if (value && typeof value === "object") {
            const status = value.status && typeof value.status === "object" ? value.status : value
            const state = String(status.state || "")
            const indexedBlockId = status.indexedBlockId !== undefined ? status.indexedBlockId : null
            if (state.length && indexedBlockId !== null) {
                return qsTr("%1, head %2").arg(state).arg(root.valueText(indexedBlockId))
            }
            if (state.length) {
                return state
            }
        }
        const scalar = root.scalarValue(value)
        return scalar !== null ? qsTr("head %1").arg(root.valueText(scalar)) : qsTr("reachable")
    }
    if (kind === "execution") {
        const scalar = root.scalarValue(value)
        return scalar !== null ? qsTr("head %1").arg(root.valueText(scalar)) : qsTr("reachable")
    }
    if (kind === "messaging") {
        const health = sourceHealth(value)
        if (health && health.summary) {
            return String(health.ready === true ? health.summary : (health.detail || health.summary))
        }
        if (!moduleReportReachable(root, value)) {
            return moduleReportError(value) || qsTr("source unavailable")
        }
        const version = root.moduleProbeValue("messaging", "version")
        return version !== null ? qsTr("version %1").arg(root.valueText(version)) : qsTr("%1 reachable").arg(root.deliverySourceLabel())
    }
    if (kind === "storage") {
        const health = sourceHealth(value)
        if (health && health.summary) {
            return String(health.ready === true ? health.summary : (health.detail || health.summary))
        }
        if (!moduleReportReachable(root, value)) {
            return moduleReportError(value) || qsTr("source unavailable")
        }
        if (String(value && value.module ? value.module : "") === "storage_metrics") {
            return qsTr("metrics available")
        }
        const version = root.moduleProbeValue("storage", "version") || root.moduleProbeValue("storage", "moduleVersion")
        return version !== null ? qsTr("version %1").arg(root.valueText(version)) : qsTr("%1 reachable").arg(root.storageSourceLabel())
    }
    return qsTr("reachable")
}

function connectionValueOk(root, kind, value) {
    if (kind === "messaging") {
        const ready = sourceHealthReady(value)
        return ready !== null ? ready : moduleReportReachable(root, value)
    }
    if (kind === "storage") {
        return storageReportReady(root, value)
    }
    return true
}

function storageReportReady(root, report) {
    const ready = sourceHealthReady(report)
    if (ready !== null) {
        return ready
    }
    return moduleReportReachable(root, report)
}

function moduleReportReachable(root, report) {
    if (!report || typeof report !== "object") {
        return false
    }
    const health = sourceHealth(report)
    if (health && health.reachable !== undefined) {
        return health.reachable === true
    }
    if (report.module_info && report.module_info.ok === true) {
        return true
    }
    const probes = Array.isArray(report.probes) ? report.probes : []
    for (let i = 0; i < probes.length; ++i) {
        if (probes[i] && probes[i].ok === true) {
            return true
        }
    }
    return false
}

function sourceHealth(report) {
    const health = report && report.health && typeof report.health === "object" && !Array.isArray(report.health)
        ? report.health
        : null
    return health
}

function sourceHealthReady(report) {
    const health = sourceHealth(report)
    if (health && health.ready !== undefined) {
        return health.ready === true
    }
    return null
}

function sourceCapability(report, key) {
    const wanted = String(key || "")
    const facts = report && Array.isArray(report.capability_facts) ? report.capability_facts : []
    for (let i = 0; i < facts.length; ++i) {
        const fact = facts[i] || {}
        if (String(fact.key || "") === wanted) {
            return fact
        }
    }
    return null
}

function sourceCapabilityAvailable(report, key) {
    const fact = sourceCapability(report, key)
    return fact !== null ? fact.available === true : null
}

function sourceCapabilityEvidence(report, key) {
    const fact = sourceCapability(report, key)
    return fact && fact.evidence !== undefined ? String(fact.evidence) : ""
}

function sourceCapabilityValue(report, key) {
    const fact = sourceCapability(report, key)
    return fact && fact.value !== undefined ? fact.value : null
}

function sourceProbeFact(report, key) {
    const wanted = String(key || "")
    const facts = report && Array.isArray(report.probe_facts) ? report.probe_facts : []
    for (let i = 0; i < facts.length; ++i) {
        const fact = facts[i] || {}
        if (String(fact.key || "") === wanted) {
            return fact
        }
    }
    return null
}

function sourceProbeValue(report, key) {
    const fact = sourceProbeFact(report, key)
    return fact && fact.ok === true && fact.value !== undefined && fact.value !== null ? fact.value : null
}

function reportProbeValue(report, method) {
    const probe = reportProbe(report, method)
    if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
        return null
    }
    return probe.value
}

function reportProbeOk(report, method) {
    const probe = reportProbe(report, method)
    return probe !== null && probe.ok === true
}

function reportProbe(report, method) {
    if (!report || typeof report !== "object") {
        return null
    }
    const wanted = String(method || "")
    const fact = sourceProbeFact(report, wanted)
    if (fact) {
        return fact
    }
    const moduleInfo = report.module_info || null
    if (moduleInfo) {
        if (String(moduleInfo.probe_key || "") === wanted) {
            return moduleInfo
        }
        const label = String(moduleInfo.label || "")
        const source = String(moduleInfo.source || "")
        if (label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0) {
            return moduleInfo
        }
    }
    const probes = Array.isArray(report.probes) ? report.probes : []
    for (let i = 0; i < probes.length; ++i) {
        const probe = probes[i] || {}
        if (String(probe.probe_key || "") === wanted) {
            return probe
        }
        const label = String(probe.label || "")
        const source = String(probe.source || "")
        if (label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0) {
            return probe
        }
    }
    return null
}

function deliveryReportHealthy(root, report) {
    const ready = sourceHealthReady(report)
    if (ready !== null) {
        return ready
    }
    return moduleReportReachable(root, report)
}

function deliveryHealthValueOk(root, value, unknownOk) {
    if (value === undefined || value === null) {
        return unknownOk === true
    }
    const scalar = root.scalarValue(value)
    if (typeof scalar === "boolean") {
        return scalar
    }
    const text = String(scalar === null ? value : scalar).trim().toLowerCase()
    if (!text.length) {
        return unknownOk === true
    }
    const normalized = text.replace(/[^a-z0-9]+/g, "")
    if (normalized === "ready" || normalized === "healthy" || normalized === "ok"
            || normalized === "connected" || normalized === "true") {
        return true
    }
    if (normalized === "initializing" || normalized === "synchronizing" || normalized === "notready"
            || normalized === "notmounted" || normalized === "shuttingdown" || normalized === "eventlooplagging"
            || normalized === "disconnected" || normalized === "partiallyconnected" || normalized === "false"
            || text.indexOf("not") >= 0 || text.indexOf("unhealthy") >= 0 || text.indexOf("error") >= 0
            || text.indexOf("fail") >= 0 || text.indexOf("down") >= 0 || text.indexOf("disconnect") >= 0) {
        return false
    }
    return unknownOk === true
}

function moduleReportError(report) {
    if (!report || typeof report !== "object") {
        return ""
    }
    if (report.module_info && report.module_info.ok === false && report.module_info.error) {
        return String(report.module_info.error)
    }
    const probes = Array.isArray(report.probes) ? report.probes : []
    for (let i = 0; i < probes.length; ++i) {
        if (probes[i] && probes[i].ok === false && probes[i].error) {
            return String(probes[i].error)
        }
    }
    return ""
}
