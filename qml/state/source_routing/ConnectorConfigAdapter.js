.pragma library

function connectorScope(config, scope) {
    const key = String(scope || "")
    const source = config && typeof config === "object" ? config : ({})
    const scopes = source.scopes && typeof source.scopes === "object" ? source.scopes : source
    const aliases = scopeAliases(key)
    for (let i = 0; i < aliases.length; ++i) {
        const entry = scopes[aliases[i]]
        if (entry && typeof entry === "object") {
            return normalizedConnectorScope(key, entry)
        }
    }
    return {
        scope: key,
        connector_id: "",
        source_mode: "",
        endpoint: "",
        provenance: ""
    }
}

function connectorSourceMode(config, scope, fallbackMode) {
    const entry = connectorScope(config, scope)
    if (entry.source_mode.length > 0) {
        return entry.source_mode
    }
    const connectorMode = sourceModeForConnector(entry.connector_id)
    return connectorMode.length > 0 ? connectorMode : String(fallbackMode || "")
}

function connectorEndpoint(config, scope, fallbackEndpoint) {
    const entry = connectorScope(config, scope)
    return entry.endpoint.length > 0 ? entry.endpoint : String(fallbackEndpoint || "")
}

function normalizedConnectorScope(scope, entry) {
    return {
        scope: String(entry.scope || scope || ""),
        connector_id: connectorId(entry),
        source_mode: sourceModeForEntry(entry),
        endpoint: String(entry.endpoint || entry.url || entry.rest_endpoint || entry.rpc_endpoint || ""),
        provenance: String(entry.provenance || entry.connector_provenance || "")
    }
}

function connectorId(entry) {
    return String(entry.connector_id || entry.connectorId || entry.id || entry.provider_instance || entry.providerInstance || "")
}

function sourceModeForEntry(entry) {
    const explicit = String(entry.source_mode || entry.sourceMode || "").trim()
    if (explicit.length > 0) {
        return explicit
    }
    return sourceModeForConnector(connectorId(entry))
}

function sourceModeForConnector(connectorId) {
    switch (String(connectorId || "")) {
    case "blockchain_module":
    case "storage_module":
    case "delivery_module":
        return "module"
    case "direct_l1_rpc":
        return "rpc"
    case "direct_storage_rest":
    case "direct_delivery_rest":
        return "rest"
    case "storage_metrics":
    case "delivery_metrics":
        return "metrics"
    case "delivery_network_monitor":
        return "network-monitor"
    default:
        return ""
    }
}

function connectorIdForMode(scope, mode) {
    const normalized = String(mode || "").trim().toLowerCase()
    switch (String(scope || "")) {
    case "l1":
        return normalized === "module" ? "blockchain_module" : "direct_l1_rpc"
    case "storage":
        if (normalized === "module") {
            return "storage_module"
        }
        if (normalized === "metrics") {
            return "storage_metrics"
        }
        return "direct_storage_rest"
    case "delivery":
        if (normalized === "module") {
            return "delivery_module"
        }
        if (normalized === "metrics") {
            return "delivery_metrics"
        }
        if (normalized === "network-monitor") {
            return "delivery_network_monitor"
        }
        return "direct_delivery_rest"
    default:
        return ""
    }
}

function scopeAliases(scope) {
    switch (String(scope || "")) {
    case "l1":
        return ["l1", "blockchain", "bedrock"]
    case "storage":
        return ["storage"]
    case "delivery":
        return ["delivery", "messaging"]
    case "wallet.l1":
        return ["wallet.l1", "wallet_l1"]
    case "wallet.l2":
        return ["wallet.l2", "wallet_l2"]
    default:
        return [String(scope || "")]
    }
}
