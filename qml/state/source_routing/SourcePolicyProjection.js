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
    const token = String(value || defaultSourceMode(family)).trim().toLowerCase()
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
    const fallbackKey = defaultSourceMode(family)
    for (let k = 0; k < modes.length; ++k) {
        const mode = modes[k] || {}
        if (String(mode.key || "") === fallbackKey) {
            return mode
        }
    }
    return modes.length > 0 ? modes[0] : ({ key: defaultSourceMode(family), effective: family === "core" ? "rpc" : "rest" })
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
        if (!key.length || !sourceModeVisible(root, mode)) {
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

function sourceModeVisible(root, mode) {
    const adapter = mode && mode.adapter && typeof mode.adapter === "object" ? mode.adapter : ({})
    return String(adapter.connection_type || "") !== "module" || root.prefersBasecampModules()
}

function sourceModeIndexFor(root, family, value, options) {
    const source = String(sourceModePolicy(root, family, value).key || defaultSourceMode(family))
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
    if (option && option.key !== undefined) {
        return String(option.key || "")
    }
    const first = sourceModeOptionAt(options, 0)
    return first && first.key !== undefined ? String(first.key || "") : "rest"
}

function sourceModeAdapter(root, family, value) {
    return sourceModeDescriptor(root, family, value).adapter
}

function sourceModeDescriptor(root, family, value) {
    const policy = sourceModePolicy(root, family, value)
    const adapter = policy.adapter && typeof policy.adapter === "object" ? policy.adapter : ({})
    const inputs = Array.isArray(adapter.inputs) ? adapter.inputs : []
    return {
        key: String(policy.key || defaultSourceMode(family)),
        effective: String(policy.effective || (family === "core" ? "rpc" : "rest")),
        label: String(policy.label || policy.key || ""),
        sourceLabel: String(policy.source_label || policy.label || ""),
        summary: String(policy.summary || ""),
        implemented: policy.implemented === true,
        adapter: adapter,
        connectorId: String(adapter.connector_id || ""),
        connectionType: String(adapter.connection_type || ""),
        target: String(adapter.target || "none"),
        moduleId: String(adapter.module_id || ""),
        inputs: inputs,
        capabilities: Array.isArray(adapter.capabilities) ? adapter.capabilities : [],
        usesRestEndpoint: adapterUsesInput(adapter, "rest_endpoint"),
        usesMetricsEndpoint: adapterUsesInput(adapter, "metrics_endpoint"),
        supportsCidProbe: adapter.supports_cid_probe === true,
        supportsMutatingDiagnostics: adapter.supports_mutating_diagnostics === true
    }
}

function adapterUsesInput(adapter, inputKey) {
    const inputs = adapter && Array.isArray(adapter.inputs) ? adapter.inputs : []
    const key = String(inputKey || "")
    for (let i = 0; i < inputs.length; ++i) {
        if (String(inputs[i] && inputs[i].key || "") === key) {
            return true
        }
    }
    return false
}

function sourceModeUsesInput(root, family, value, inputKey) {
    return adapterUsesInput(sourceModeDescriptor(root, family, value).adapter, inputKey)
}

function resolvedSourceModeKey(root, family, value) {
    return sourceModeDescriptor(root, family, value).key
}

function sourceModeTargetKind(root, family, value) {
    return sourceModeDescriptor(root, family, value).target
}

function sourceModeUsesEndpoint(root, family, value, endpointKind) {
    switch (String(endpointKind || "")) {
    case "rest":
        return sourceModeUsesInput(root, family, value, "rest_endpoint")
    case "metrics":
        return sourceModeUsesInput(root, family, value, "metrics_endpoint")
    case "rpc":
        return sourceModeUsesInput(root, family, value, "rpc_endpoint")
    default:
        return false
    }
}

function sourceModeSupportsCidProbe(root, family, value) {
    return sourceModeDescriptor(root, family, value).supportsCidProbe
}

function sourceModeSupportsMutatingDiagnostics(root, family, value) {
    return sourceModeDescriptor(root, family, value).supportsMutatingDiagnostics
}

function storageSourceSupportsNetworkDebug(root, sourceMode) {
    const effective = sourceModeDescriptor(
        root, "storage", sourceMode).effective
    return effective === "module" || effective === "rest"
}

function coreSourceArgs(root, sourceMode, endpoint, extra) {
    const rest = Array.isArray(extra) ? extra : []
    const descriptor = sourceModeDescriptor(root, "core", sourceMode)
    if (descriptor.effective === "module") {
        return [descriptor.key].concat(rest)
    }
    return [String(endpoint || "")].concat(rest)
}

function deliverySourceReportArgs(root, sourceMode, restEndpoint, metricsEndpoint, runtimeDiagnosticsEnabled) {
    const initialization = adapterInitialization(root, "delivery", sourceMode, {
        rest_endpoint: String(restEndpoint || ""),
        metrics_endpoint: String(metricsEndpoint || "")
    })
    // Store provider selection belongs to an individual Store operation, not
    // Delivery source inspection. Keeping it out of the report request also
    // prevents a saved provider from invalidating healthy CLI source evidence.
    delete initialization.inputs.store_peer_addr
    initialization.options = {
        runtime_diagnostics_enabled: runtimeDiagnosticsEnabled !== false
    }
    return [initialization]
}

function storageSourceReportArgs(root, sourceMode, restEndpoint, metricsEndpoint, cid, includeCidProbe, privilegedDebugEnabled, runtimeDiagnosticsEnabled) {
    const initialization = adapterInitialization(root, "storage", sourceMode, {
        rest_endpoint: String(restEndpoint || ""),
        metrics_endpoint: String(metricsEndpoint || "")
    })
    initialization.options = {
        cid: includeCidProbe === true && sourceModeSupportsCidProbe(root, "storage", sourceMode) ? String(cid || "") : "",
        privileged_debug_enabled: privilegedDebugEnabled === true
            && storageSourceSupportsNetworkDebug(root, sourceMode),
        runtime_diagnostics_enabled: runtimeDiagnosticsEnabled !== false
    }
    return [initialization]
}

function adapterInitialization(root, family, sourceMode, values) {
    const descriptor = sourceModeDescriptor(root, family, sourceMode)
    const provided = values && typeof values === "object" ? values : ({})
    const inputs = {}
    for (let i = 0; i < descriptor.inputs.length; ++i) {
        const input = descriptor.inputs[i] || {}
        const key = String(input.key || "")
        if (key.length > 0) {
            inputs[key] = String(provided[key] || "")
        }
    }
    return {
        source_mode: descriptor.key,
        inputs: inputs
    }
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
    const descriptor = sourceModeDescriptor(root, family, sourceMode)
    return String(descriptor.sourceLabel || descriptor.label || fallbackLabel || "")
}

function coreSourceLabel(root, sourceMode, rpcLabel) {
    const descriptor = sourceModeDescriptor(root, "core", sourceMode)
    return descriptor.effective === "module"
        ? String(descriptor.sourceLabel || descriptor.label || qsTr("Module"))
        : rpcLabel
}

function defaultSourceMode(family) {
    return family === "core" ? "rpc" : "rest"
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
