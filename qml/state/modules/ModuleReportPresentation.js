function moduleLabel(root, kind) {
    switch (kind) {
    case "storage":
        return qsTr("Storage")
    case "messaging":
        return qsTr("Messaging")
    case "capabilities":
        return qsTr("Capabilities")
    default:
        return qsTr("L1 Node")
    }
}

function moduleLayer(root) {
    if (root.moduleKind === "blockchain") {
        return qsTr("L1 Bedrock")
    }
    return qsTr("Diagnostics")
}

function moduleName(root, kind) {
    switch (kind) {
    case "storage":
        return root.model.storageModule
    case "messaging":
        return root.model.deliveryModule
    case "capabilities":
        return root.model.inspectorModule
    default:
        return root.model.nodeUrl
    }
}

function modulePanelTitle(root) {
    return qsTr("%1 tools").arg(moduleLabel(root, root.moduleKind))
}

function moduleMessageTitle(root) {
    if (root.moduleKind === "blockchain") {
        return qsTr("Node and LogosCore")
    }
    return qsTr("LogosCore module")
}

function moduleMessage(root) {
    switch (root.moduleKind) {
    case "storage":
        return qsTr("Run storage REST metadata probes, then check a specific CID through the configured source.")
    case "messaging":
        return qsTr("Inspect delivery REST metadata without leaving the Messaging surface.")
    case "capabilities":
        return qsTr("Check LogosCore status and source configuration from one place.")
    default:
        return qsTr("Probe the configured blockchain node and block windows from this screen.")
    }
}

function moduleTargetText(root) {
    if (root.moduleKind === "blockchain") {
        return root.endpointLabel(root.model.nodeUrl)
    }
    return qsTr("Local")
}

function moduleTargetDetail(root) {
    if (root.moduleKind === "blockchain") {
        return root.shortEndpoint(root.model.nodeUrl)
    }
    return qsTr("LogosCore bridge")
}

function moduleStatusText(root) {
    if (!root.hasResponse) {
        return qsTr("Idle")
    }
    if (root.model.resultIsError) {
        return qsTr("Error")
    }
    return responseStatusText(root)
}

function moduleStatusDelta(root) {
    if (!root.hasResponse) {
        return qsTr("Awaiting call")
    }
    if (root.model.resultIsError) {
        return root.model.resultText
    }
    return responseSourceText(root)
}

function moduleStatusColor(root) {
    if (!root.hasResponse) {
        return root.theme.textMuted
    }
    if (root.model.resultIsError) {
        return root.theme.warning
    }
    return responseStatusColor(root)
}

function moduleProbeText(root) {
    if (root.responseProbeModel.length > 0) {
        return root.numberText(root.responseProbeModel.length)
    }
    return expectedProbeText(root)
}

function moduleProbeDelta(root) {
    if (root.responseProbeModel.length > 0) {
        return responseProbeDelta(root)
    }
    return qsTr("Default probe plan")
}

function expectedProbeText(root) {
    switch (root.moduleKind) {
    case "storage":
        return "10"
    case "messaging":
        return "12"
    case "capabilities":
        return "1"
    default:
        return "5"
    }
}

function responseStatusText(root) {
    if (root.model.resultIsError) {
        return qsTr("Error")
    }
    const rows = root.responseProbeModel
    if (!rows.length) {
        return qsTr("OK")
    }
    const ok = responseProbeOkCount(root)
    if (ok === rows.length) {
        return qsTr("OK")
    }
    if (ok === 0) {
        return qsTr("Error")
    }
    return qsTr("Partial")
}

function responseStatusColor(root) {
    const status = responseStatusText(root)
    if (status === qsTr("OK")) {
        return root.theme.success
    }
    if (status === qsTr("Partial")) {
        return root.theme.warning
    }
    return root.theme.error
}

function responseSourceText(root) {
    return root.model.resultTitle.length ? root.model.resultTitle : moduleLabel(root, root.moduleKind)
}

function responseProbeOkCount(root) {
    const rows = root.responseProbeModel
    let ok = 0
    for (let i = 0; i < rows.length; ++i) {
        if (rows[i].ok) {
            ok += 1
        }
    }
    return ok
}

function responseProbeOkText(root) {
    const rows = root.responseProbeModel
    if (!rows.length) {
        return root.hasResponse && !root.model.resultIsError ? qsTr("Yes") : "-"
    }
    return qsTr("%1/%2").arg(responseProbeOkCount(root)).arg(rows.length)
}

function responseProbeDelta(root) {
    const rows = root.responseProbeModel
    if (!rows.length) {
        return qsTr("No probe breakdown")
    }
    return qsTr("%1 probe(s)").arg(rows.length)
}

function responsePayloadText(root) {
    const value = root.responseValue
    if (value === null || value === undefined) {
        return "-"
    }
    if (Array.isArray(value)) {
        return root.numberText(value.length)
    }
    if (typeof value === "object") {
        return root.numberText(Object.keys(value).length)
    }
    return root.valueText(value)
}

function responseKindText(root) {
    const value = root.responseValue
    if (Array.isArray(value)) {
        return qsTr("Array items")
    }
    if (value && typeof value === "object") {
        return qsTr("Object fields")
    }
    return qsTr("Scalar value")
}

function responseTargetText(root) {
    const value = root.responseValue
    if (value && typeof value === "object" && !Array.isArray(value) && value.endpoint !== undefined) {
        return root.endpointLabel(value.endpoint)
    }
    if (root.moduleKind === "blockchain") {
        return root.endpointLabel(root.model.nodeUrl)
    }
    return qsTr("Local")
}

function responseTargetDetail(root) {
    const value = root.responseValue
    if (value && typeof value === "object" && !Array.isArray(value) && value.endpoint !== undefined) {
        return root.shortEndpoint(value.endpoint)
    }
    return moduleTargetDetail(root)
}

function responseProbeRows(root) {
    const rows = []
    const value = root.responseValue
    if (!value || typeof value !== "object" || Array.isArray(value)) {
        return rows
    }

    if (isProbe(value)) {
        pushProbe(root, rows, value, responseSourceText(root), "")
        return rows
    }

    appendModuleReport(root, rows, value, "")

    pushNamedProbe(root, rows, value, "cryptarchia_info", qsTr("Cryptarchia info"), "")
    pushNamedProbe(root, rows, value, "headers", qsTr("Headers"), "")
    pushNamedProbe(root, rows, value, "network_info", qsTr("Network info"), "")
    pushNamedProbe(root, rows, value, "mantle_metrics", qsTr("Mantle metrics"), "")
    pushNamedProbe(root, rows, value, "status", qsTr("LogosCore status"), "")

    appendModuleReport(root, rows, value.blockchain, qsTr("Blockchain"))
    appendModuleReport(root, rows, value.storage, qsTr("Storage"))
    appendModuleReport(root, rows, value.delivery, qsTr("Messaging"))
    appendModuleReport(root, rows, value.capabilities, qsTr("Capabilities"))

    return rows
}

function blockchainPeerIdProbe(root) {
    const value = root.responseValue
    if (!value || typeof value !== "object" || Array.isArray(value)) {
        return root.model.moduleProbe("blockchain", "get_peer_id")
    }
    if (isBlockchainModuleReport(root, value)) {
        return findModuleProbe(value, "get_peer_id")
    }
    if (isBlockchainModuleReport(root, value.blockchain)) {
        return findModuleProbe(value.blockchain, "get_peer_id")
    }
    return root.model.moduleProbe("blockchain", "get_peer_id")
}

function blockchainPeerIdText(root) {
    const probe = blockchainPeerIdProbe(root)
    if (!probe) {
        return qsTr("Unavailable")
    }
    if (probe.ok !== true) {
        return probe.error ? qsTr("Unavailable: %1").arg(root.valueText(probe.error)) : qsTr("Unavailable")
    }
    const value = probeScalarText(root, probe.value)
    return value.length > 0 ? value : qsTr("Unavailable")
}

function blockchainPeerIdCopyText(root) {
    const probe = blockchainPeerIdProbe(root)
    if (!probe || probe.ok !== true) {
        return ""
    }
    return probeScalarText(root, probe.value)
}

function probeScalarText(root, value) {
    if (value === undefined || value === null || value === "") {
        return ""
    }
    const scalar = root.model.scalarValue(value)
    if (scalar === null || scalar === undefined || scalar === "") {
        return root.valueText(value)
    }
    return String(scalar)
}

function isBlockchainModuleReport(root, value) {
    return value && typeof value === "object" && !Array.isArray(value) && String(value.module || "") === root.model.blockchainModule
}

function findModuleProbe(report, method) {
    if (!report || typeof report !== "object" || Array.isArray(report)) {
        return null
    }
    const wanted = String(method || "")
    const probes = Array.isArray(report.probes) ? report.probes : []
    for (let i = 0; i < probes.length; ++i) {
        const probe = probes[i] || {}
        if (String(probe.probe_key || probe.key || "") === wanted) {
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

function appendModuleReport(root, rows, report, prefix) {
    if (!report || typeof report !== "object" || Array.isArray(report)) {
        return
    }
    const labelPrefix = prefix.length ? prefix : moduleDisplayName(root, report.module)
    const facts = Array.isArray(report.probe_facts) ? report.probe_facts : []
    if (facts.length > 0) {
        for (let i = 0; i < facts.length; ++i) {
            pushProbe(root, rows, facts[i], qsTr("Probe fact"), labelPrefix)
        }
        return
    }
    if (isProbe(report.module_info)) {
        pushProbe(root, rows, report.module_info, qsTr("Module info"), labelPrefix)
    }
    const probes = Array.isArray(report.probes) ? report.probes : []
    for (let i = 0; i < probes.length; ++i) {
        pushProbe(root, rows, probes[i], qsTr("Probe"), labelPrefix)
    }
}

function pushNamedProbe(root, rows, value, key, label, prefix) {
    if (value && isProbe(value[key])) {
        pushProbe(root, rows, value[key], label, prefix)
    }
}

function pushProbe(root, rows, probe, fallbackLabel, prefix) {
    if (!isProbe(probe)) {
        return
    }
    const baseLabel = String(probe.label || probe.key || fallbackLabel || "-")
    const label = prefix && prefix.length ? qsTr("%1 / %2").arg(prefix).arg(baseLabel) : baseLabel
    rows.push({
        label: label,
        source: String(probe.source || ""),
        ok: !!probe.ok,
        detail: probe.ok ? root.valueSummary(probe.value) : root.valueText(probe.error)
    })
}

function isProbe(value) {
    return value && typeof value === "object" && !Array.isArray(value) && value.ok !== undefined
}

function moduleDisplayName(root, name) {
    switch (String(name || "")) {
    case root.model.storageModule:
        return qsTr("Storage")
    case root.model.deliveryModule:
        return qsTr("Messaging")
    case root.model.capabilityModule:
        return qsTr("Capabilities")
    case root.model.blockchainModule:
        return qsTr("Blockchain")
    default:
        return ""
    }
}
