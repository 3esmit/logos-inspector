.import "SourcePolicyCatalog.js" as SourcePolicyCatalog

function sourcePolicyDefault(root, key, fallback) {
    const policy = root.sourcePolicy || {}
    const defaults = policy.defaults && typeof policy.defaults === "object"
        ? policy.defaults
        : null
    const value = defaults && defaults[key] !== undefined ? String(defaults[key] || "").trim() : ""
    return value.length > 0 ? value : SourcePolicyCatalog.defaultValue(key, fallback)
}

function sourceModePolicy(root, family, value) {
    const token = String(value || "auto").trim().toLowerCase()
    const modes = sourceModePolicies(root, family)
    for (let i = 0; i < modes.length; ++i) {
        const mode = modes[i] || {}
        if (String(mode.key || "") === token) {
            return mode
        }
        const aliases = Array.isArray(mode.aliases) ? mode.aliases : []
        for (let j = 0; j < aliases.length; ++j) {
            if (String(aliases[j] || "").toLowerCase() === token) {
                return mode
            }
        }
    }
    const fallbackKey = family === "core" ? "auto" : "unsupported"
    for (let k = 0; k < modes.length; ++k) {
        const mode = modes[k] || {}
        if (String(mode.key || "") === fallbackKey) {
            return mode
        }
    }
    return modes.length > 0 ? modes[0] : ({ key: "auto", effective: family === "core" ? "rpc" : "rest" })
}

function sourceModePolicies(root, family) {
    const policy = root.sourcePolicy || {}
    const sourceModes = policy.source_modes || {}
    const modes = Array.isArray(sourceModes[family]) ? sourceModes[family] : []
    return modes.length > 0 ? modes : SourcePolicyCatalog.sourceModes(family)
}

function sourceModeOptions(root, family) {
    const modes = sourceModePolicies(root, family)
    const options = []
    for (let i = 0; i < modes.length; ++i) {
        const mode = modes[i] || {}
        const key = String(mode.key || "")
        if (!key.length) {
            continue
        }
        options.push({
            key: key,
            label: String(mode.label || key),
            summary: String(mode.summary || "")
        })
    }
    return options
}

function sourceModeIndexFor(root, family, value, options) {
    const source = String(sourceModePolicy(root, family, value).key || "auto")
    const count = sourceModeOptionCount(options)
    for (let i = 0; i < count; ++i) {
        const option = sourceModeOptionAt(options, i)
        if (option && String(option.key || "") === source) {
            return i
        }
    }
    return 0
}

function sourceModeAt(index, options) {
    const option = sourceModeOptionAt(options, index)
    return option && option.key !== undefined ? String(option.key || "auto") : "auto"
}

function sourceModeAdapter(root, family, value) {
    const adapter = sourceModePolicy(root, family, resolvedSourceModeKey(root, family, value)).adapter
    return adapter && typeof adapter === "object" ? adapter : ({})
}

function resolvedSourceModeKey(root, family, value) {
    const policy = sourceModePolicy(root, family, value)
    const key = String(policy.key || "auto")
    if (key === "auto" && root.prefersBasecampModules()
            && (family === "core" || family === "delivery" || family === "storage")) {
        return "module"
    }
    return key
}

function sourceModeTargetKind(root, family, value) {
    return String(sourceModeAdapter(root, family, value).target || "none")
}

function sourceModeUsesEndpoint(root, family, value, endpointKind) {
    const adapter = sourceModeAdapter(root, family, value)
    switch (String(endpointKind || "")) {
    case "rest":
        return adapter.uses_rest_endpoint === true
    case "metrics":
        return adapter.uses_metrics_endpoint === true
    default:
        return false
    }
}

function sourceModeSupportsCidProbe(root, family, value) {
    return sourceModeAdapter(root, family, value).supports_cid_probe === true
}

function sourceModeSupportsMutatingDiagnostics(root, family, value) {
    return sourceModeAdapter(root, family, value).supports_mutating_diagnostics === true
}

function coreSourceArgs(root, sourceMode, endpoint, extra) {
    const rest = Array.isArray(extra) ? extra : []
    if (String(sourceModePolicy(root, "core", resolvedSourceModeKey(root, "core", sourceMode)).effective || "rpc") === "module") {
        return ["module", String(endpoint || "")].concat(rest)
    }
    return [String(endpoint || "")].concat(rest)
}

function accountLookupArgs(root, executionSourceMode, sequencerEndpoint, indexerSourceMode, indexerEndpoint, account, idlJson, accountType) {
    const suffix = [String(account || "")]
    const idl = String(idlJson || "").trim()
    if (idl.length > 0) {
        suffix.push(idl)
        if (accountType !== undefined && accountType !== null && String(accountType).trim().length > 0) {
            suffix.push(String(accountType).trim())
        }
    }
    const executionMode = String(sourceModePolicy(root, "core", resolvedSourceModeKey(root, "core", executionSourceMode)).effective || "rpc")
    const indexerMode = String(sourceModePolicy(root, "core", resolvedSourceModeKey(root, "core", indexerSourceMode)).effective || "rpc")
    if (executionMode === "module" || indexerMode === "module") {
        return [executionMode, String(sequencerEndpoint || ""), indexerMode, String(indexerEndpoint || "")].concat(suffix)
    }
    return [String(sequencerEndpoint || ""), String(indexerEndpoint || "")].concat(suffix)
}

function lezLookupArgs(root, executionSourceMode, sequencerEndpoint, indexerSourceMode, indexerEndpoint, target) {
    return accountLookupArgs(root, executionSourceMode, sequencerEndpoint, indexerSourceMode, indexerEndpoint, target, "", "")
}

function deliverySourceReportArgs(root, sourceMode, restEndpoint, metricsEndpoint) {
    return [
        String(sourceModePolicy(root, "delivery", resolvedSourceModeKey(root, "delivery", sourceMode)).effective || "rest"),
        sourceModeUsesEndpoint(root, "delivery", sourceMode, "rest") ? String(restEndpoint || "") : "",
        sourceModeUsesEndpoint(root, "delivery", sourceMode, "metrics") ? String(metricsEndpoint || "") : ""
    ]
}

function storageSourceReportArgs(root, sourceMode, restEndpoint, metricsEndpoint, cid, includeCidProbe, privilegedDebugEnabled) {
    return [
        String(sourceModePolicy(root, "storage", resolvedSourceModeKey(root, "storage", sourceMode)).effective || "rest"),
        sourceModeUsesEndpoint(root, "storage", sourceMode, "rest") ? String(restEndpoint || "") : "",
        sourceModeUsesEndpoint(root, "storage", sourceMode, "metrics") ? String(metricsEndpoint || "") : "",
        includeCidProbe === true && sourceModeSupportsCidProbe(root, "storage", sourceMode) ? String(cid || "") : "",
        privilegedDebugEnabled === true
    ]
}

function sourceTarget(root, family, sourceMode, targets) {
    const targetValues = targets && typeof targets === "object" ? targets : ({})
    switch (sourceModeTargetKind(root, family, sourceMode)) {
    case "module":
        return String(targetValues.module || "")
    case "rest_endpoint":
        return String(targetValues.rest || "")
    case "metrics_endpoint":
        return String(targetValues.metrics || "")
    default:
        return ""
    }
}

function sourceLabel(root, family, sourceMode, fallbackLabel) {
    const source = sourceModePolicy(root, family, resolvedSourceModeKey(root, family, sourceMode))
    const label = String(source.source_label || source.label || fallbackLabel || "")
    return String(sourceModePolicy(root, family, sourceMode).key || "auto") === "auto" && root.prefersBasecampModules()
        ? qsTr("Auto: %1").arg(label)
        : label
}

function coreSourceLabel(root, sourceMode, rpcLabel) {
    const source = resolvedSourceModeKey(root, "core", sourceMode)
    if (source === "module") {
        return String(sourceModePolicy(root, "core", sourceMode).key || "auto") === "auto"
            ? qsTr("Auto: Basecamp module")
            : qsTr("Basecamp module")
    }
    if (source === "rpc") {
        return rpcLabel
    }
    return qsTr("Auto: %1").arg(rpcLabel)
}

function sourceModeOptionCount(options) {
    if (Array.isArray(options)) {
        return options.length
    }
    return options && options.count !== undefined ? Number(options.count || 0) : 0
}

function sourceModeOptionAt(options, index) {
    if (index < 0 || index >= sourceModeOptionCount(options)) {
        return null
    }
    if (Array.isArray(options)) {
        return options[index] || null
    }
    return options && typeof options.get === "function" ? options.get(index) : null
}
