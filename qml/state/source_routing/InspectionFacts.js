function health(report) {
    return report && report.health && typeof report.health === "object" && !Array.isArray(report.health)
        ? report.health
        : null
}

function healthReady(report) {
    const value = health(report)
    if (value && value.ready !== undefined) {
        return value.ready === true
    }
    return null
}

function reachable(report) {
    if (!report || typeof report !== "object") {
        return false
    }
    const value = health(report)
    if (value && value.reachable !== undefined) {
        return value.reachable === true
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

function capability(report, key) {
    return fact(report, "capability_facts", key)
}

function capabilityAvailable(report, key) {
    const value = capability(report, key)
    return value !== null ? value.available === true : null
}

function capabilityEvidence(report, key) {
    const value = capability(report, key)
    return value && value.evidence !== undefined ? String(value.evidence) : ""
}

function capabilityValue(report, key) {
    const value = capability(report, key)
    return value && value.value !== undefined ? value.value : null
}

function probeFact(report, key) {
    return fact(report, "probe_facts", key)
}

function fact(report, fieldName, key) {
    const wanted = String(key || "")
    const facts = report && Array.isArray(report[fieldName]) ? report[fieldName] : []
    for (let i = 0; i < facts.length; ++i) {
        const value = facts[i] || {}
        if (String(value.key || "") === wanted) {
            return value
        }
    }
    return null
}

function probeValue(report, key) {
    const value = probeFact(report, key)
    return value && value.ok === true && value.value !== undefined && value.value !== null ? value.value : null
}

function reportProbeValue(report, method) {
    const value = reportProbe(report, method)
    if (!value || value.ok !== true || value.value === undefined || value.value === null) {
        return null
    }
    return value.value
}

function reportProbeOk(report, method) {
    const value = reportProbe(report, method)
    return value !== null && value.ok === true
}

function reportProbe(report, method) {
    if (!report || typeof report !== "object") {
        return null
    }
    const wanted = String(method || "")
    const sourceFact = probeFact(report, wanted)
    if (sourceFact) {
        return sourceFact
    }
    const moduleInfo = report.module_info || null
    if (moduleInfo) {
        if (probeMatchesKey(moduleInfo, wanted)) {
            return moduleInfo
        }
    }
    const probes = Array.isArray(report.probes) ? report.probes : []
    for (let i = 0; i < probes.length; ++i) {
        const value = probes[i] || {}
        if (probeMatchesKey(value, wanted)) {
            return value
        }
    }
    return null
}

function probeMatchesKey(probe, wanted) {
    if (String(probe.probe_key || "") === wanted) {
        return true
    }
    const label = String(probe.label || "")
    const source = String(probe.source || "")
    return label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0
}

function failedProbeCount(report) {
    let failed = 0
    if (!report) {
        return failed
    }
    const facts = Array.isArray(report.probe_facts) ? report.probe_facts : []
    if (facts.length > 0) {
        for (let i = 0; i < facts.length; ++i) {
            if (facts[i] && facts[i].ok === false) {
                failed += 1
            }
        }
        return failed
    }
    if (report.module_info && report.module_info.ok === false) {
        failed += 1
    }
    const probes = Array.isArray(report.probes) ? report.probes : []
    for (let i = 0; i < probes.length; ++i) {
        if (probes[i] && probes[i].ok === false) {
            failed += 1
        }
    }
    return failed
}

function error(report) {
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
