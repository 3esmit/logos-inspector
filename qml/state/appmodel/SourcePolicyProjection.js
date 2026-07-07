function sourcePolicyDefault(root, key, fallback) {
    const policy = root.sourcePolicy || {}
    const defaults = policy.defaults && typeof policy.defaults === "object"
        ? policy.defaults
        : null
    const value = defaults && defaults[key] !== undefined ? String(defaults[key] || "").trim() : ""
    return value.length > 0 ? value : String(fallback || "")
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
    return modes.length > 0 ? modes : fallbackSourceModePolicies(family)
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

function fallbackSourceModePolicies(family) {
    if (family === "core") {
        return [
            sourceModeRecord("auto", ["auto"], "rpc", "Auto", "Auto: Direct RPC", "Use configured direct RPC endpoint", "rpc_endpoint", false, false, false, false),
            sourceModeRecord("rpc", ["rpc"], "rpc", "Direct RPC", "Direct RPC", "Use configured standalone RPC endpoint", "rpc_endpoint", false, false, false, false),
            sourceModeRecord("module", ["module", "basecamp", "basecamp-module", "basecamp module"], "module", "Basecamp module", "Basecamp module", "Use Basecamp module APIs where available", "module", false, false, false, true)
        ]
    }
    if (family === "delivery") {
        return [
            sourceModeRecord("auto", ["auto"], "rest", "Auto", "Auto: Direct Waku REST", "Use direct Waku REST", "rest_endpoint", true, true, false, true),
            sourceModeRecord("module", ["module", "basecamp", "basecamp-module", "basecamp module"], "module", "Delivery module", "Delivery module", "Use delivery_module through module APIs", "module", false, false, false, true),
            sourceModeRecord("rest", ["rest"], "rest", "Direct Waku REST", "Direct Waku REST", "Read-only health, info, version, and optional metrics", "rest_endpoint", true, true, false, true),
            sourceModeRecord("unsupported", ["unsupported"], "unsupported", "Unsupported saved source", "Unsupported source", "Select a supported source to replace this saved value", "none", false, false, false, false)
        ]
    }
    return [
        sourceModeRecord("auto", ["auto"], "rest", "Auto", "Auto: Standalone REST", "Use standalone REST", "rest_endpoint", true, true, true, true),
        sourceModeRecord("module", ["module", "basecamp", "basecamp-module", "basecamp module"], "module", "Storage module", "Storage module", "Use storage_module through module APIs", "module", false, false, true, true),
        sourceModeRecord("rest", ["rest"], "rest", "Standalone REST", "Standalone REST", "Read-only space, identity, local data, debug, and metrics", "rest_endpoint", true, true, true, true),
        sourceModeRecord("unsupported", ["unsupported"], "unsupported", "Unsupported saved source", "Unsupported source", "Select a supported source to replace this saved value", "none", false, false, false, false)
    ]
}

function sourceModeRecord(key, aliases, effective, label, sourceLabel, summary, target, usesRest, usesMetrics, supportsCid, supportsMutating) {
    return {
        key: key,
        aliases: aliases,
        effective: effective,
        label: label,
        source_label: sourceLabel,
        summary: summary,
        adapter: {
            target: target,
            uses_rest_endpoint: usesRest,
            uses_metrics_endpoint: usesMetrics,
            supports_cid_probe: supportsCid,
            supports_mutating_diagnostics: supportsMutating
        }
    }
}
