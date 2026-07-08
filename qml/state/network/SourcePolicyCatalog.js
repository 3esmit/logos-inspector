function fallbackPolicy() {
    return {
        version: 2,
        defaults: {
            core: "auto",
            delivery: "auto",
            storage: "auto"
        },
        source_modes: {
            core: [
                sourceModeRecord("auto", ["auto"], "rpc", "Auto", "Auto: Direct RPC", "Use configured direct RPC endpoint", "rpc_endpoint", false, false, false, false),
                sourceModeRecord("rpc", ["rpc"], "rpc", "Direct RPC", "Direct RPC", "Use configured standalone RPC endpoint", "rpc_endpoint", false, false, false, false),
                sourceModeRecord("module", ["module", "basecamp", "basecamp-module", "basecamp module"], "module", "Basecamp module", "Basecamp module", "Use Basecamp module APIs where available", "module", false, false, false, true)
            ],
            delivery: [
                sourceModeRecord("auto", ["auto"], "rest", "Auto", "Auto: Direct Waku REST", "Use direct Waku REST", "rest_endpoint", true, true, false, true),
                sourceModeRecord("module", ["module", "basecamp", "basecamp-module", "basecamp module"], "module", "Delivery module", "Delivery module", "Use delivery_module through module APIs", "module", false, false, false, true),
                sourceModeRecord("rest", ["rest"], "rest", "Direct Waku REST", "Direct Waku REST", "Read-only health, info, version, and optional metrics", "rest_endpoint", true, true, false, true),
                sourceModeRecord("metrics", ["metrics", "prometheus"], "metrics", "Metrics only", "Metrics only", "Scrape delivery metrics without REST diagnostics", "metrics_endpoint", false, true, false, false),
                sourceModeRecord("network-monitor", ["network-monitor", "network monitor", "discovery crawler"], "network-monitor", "Network monitor", "Network monitor", "Inspect fleet topology from discovery monitor data", "rest_endpoint", true, true, false, false),
                sourceModeRecord("unsupported", ["unsupported"], "unsupported", "Unsupported saved source", "Unsupported source", "Select a supported source to replace this saved value", "none", false, false, false, false)
            ],
            storage: [
                sourceModeRecord("auto", ["auto"], "rest", "Auto", "Auto: Standalone REST", "Use standalone REST", "rest_endpoint", true, true, true, true),
                sourceModeRecord("module", ["module", "basecamp", "basecamp-module", "basecamp module"], "module", "Storage module", "Storage module", "Use storage_module through module APIs", "module", false, false, true, true),
                sourceModeRecord("rest", ["rest", "standalone rest"], "rest", "Standalone REST", "Standalone REST", "Read-only space, identity, local data, debug, and metrics", "rest_endpoint", true, true, true, true),
                sourceModeRecord("metrics", ["metrics", "prometheus"], "metrics", "Metrics only", "Metrics only", "Scrape storage metrics without REST diagnostics", "metrics_endpoint", false, true, false, false),
                sourceModeRecord("unsupported", ["unsupported", "c-library", "local-os"], "unsupported", "Unsupported saved source", "Unsupported source", "Select a supported source to replace this saved value", "none", false, false, false, false)
            ]
        }
    }
}

function sourceModes(family) {
    const modes = fallbackPolicy().source_modes[String(family || "")]
    return Array.isArray(modes) ? modes : []
}

function defaultValue(key, fallback) {
    const value = fallbackPolicy().defaults[String(key || "")]
    return value !== undefined ? String(value || "") : String(fallback || "")
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
