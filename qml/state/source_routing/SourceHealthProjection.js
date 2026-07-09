.import "InspectionFacts.js" as InspectionFacts

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
    return InspectionFacts.reachable(report)
}

function sourceHealth(report) {
    return InspectionFacts.health(report)
}

function sourceHealthReady(report) {
    return InspectionFacts.healthReady(report)
}

function sourceCapability(report, key) {
    return InspectionFacts.capability(report, key)
}

function sourceCapabilityAvailable(report, key) {
    return InspectionFacts.capabilityAvailable(report, key)
}

function sourceCapabilityEvidence(report, key) {
    return InspectionFacts.capabilityEvidence(report, key)
}

function sourceCapabilityValue(report, key) {
    return InspectionFacts.capabilityValue(report, key)
}

function sourceProbeFact(report, key) {
    return InspectionFacts.probeFact(report, key)
}

function sourceFact(report, fieldName, key) {
    return InspectionFacts.fact(report, fieldName, key)
}

function sourceProbeValue(report, key) {
    return InspectionFacts.probeValue(report, key)
}

function reportProbeValue(report, method) {
    return InspectionFacts.reportProbeValue(report, method)
}

function reportProbeOk(report, method) {
    return InspectionFacts.reportProbeOk(report, method)
}

function reportProbe(report, method) {
    return InspectionFacts.reportProbe(report, method)
}

function probeMatchesKey(probe, wanted) {
    return InspectionFacts.probeMatchesKey(probe, wanted)
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
    return InspectionFacts.error(report)
}
