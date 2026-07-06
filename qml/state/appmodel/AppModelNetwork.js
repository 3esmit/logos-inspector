.import "../../services/BridgeHelpers.js" as BridgeHelpers

function refreshInterval(root, seconds) {
    with (root) {
        return Math.max(5, Number(seconds || 0)) * 1000
    }
}

function dashboardRefreshInterval(root) {
    with (root) {
        const rates = [
            blockchainRefreshRate,
            indexerRefreshRate,
            executionRefreshRate,
            messagingRefreshRate,
            storageRefreshRate
        ]
        let selected = 0
        for (let i = 0; i < rates.length; ++i) {
            const value = root.canonicalRefreshRate(rates[i])
            if (value > 0 && (selected === 0 || value < selected)) {
                selected = value
            }
        }
        return selected > 0 ? root.refreshInterval(selected) : 0
    }
}

function loadSourcePolicy(root) {
    with (root) {
        const response = bridge.callModule(inspectorModule, "sourcePolicy", [])
        if (response.ok === true && response.value && typeof response.value === "object") {
            sourcePolicy = response.value
            sourcePolicyLoaded = true
            return true
        }
        sourcePolicy = ({})
        sourcePolicyLoaded = false
        return false
    }
}

function sourcePolicyDefault(root, key, fallback) {
    with (root) {
        const defaults = sourcePolicy && sourcePolicy.defaults && typeof sourcePolicy.defaults === "object"
            ? sourcePolicy.defaults
            : null
        const value = defaults && defaults[key] !== undefined ? String(defaults[key] || "").trim() : ""
        return value.length > 0 ? value : String(fallback || "")
    }
}

function sourceModePolicy(root, family, value) {
    with (root) {
        const token = String(value || "auto").trim().toLowerCase()
        const modes = sourcePolicy && sourcePolicy.source_modes && Array.isArray(sourcePolicy.source_modes[family])
            ? sourcePolicy.source_modes[family]
            : []
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
        return fallbackSourceModePolicy(family, token)
    }
}

function fallbackSourceModePolicy(family, token) {
    if (family === "core") {
        switch (token) {
        case "rpc":
        case "direct-rpc":
        case "direct rpc":
        case "standalone":
        case "standalone-rpc":
        case "standalone rpc":
            return { key: "rpc", effective: "rpc", label_key: "direct_rpc" }
        default:
            return { key: "auto", effective: "rpc", label_key: "auto_rpc" }
        }
    }
    if (family === "delivery") {
        switch (token) {
        case "module":
        case "basecamp":
        case "basecamp-module":
        case "basecamp module":
            return { key: "module", effective: "module", label_key: "delivery_module" }
        case "network-monitor":
        case "network monitor":
        case "discovery-crawler":
        case "discovery crawler":
        case "crawler":
            return { key: "network-monitor", effective: "network-monitor", label_key: "network_monitor" }
        case "rest":
        case "direct-rest":
        case "direct waku rest":
        case "waku-rest":
            return { key: "rest", effective: "rest", label_key: "delivery_rest" }
        case "metrics":
        case "metrics-only":
        case "metrics only":
            return { key: "metrics", effective: "metrics", label_key: "metrics_only" }
        case "unsupported":
            return { key: "unsupported", effective: "unsupported", label_key: "unsupported" }
        default:
            return { key: "auto", effective: "rest", label_key: "auto_delivery_rest" }
        }
    }
    switch (token) {
    case "module":
    case "basecamp":
    case "basecamp-module":
    case "basecamp module":
        return { key: "module", effective: "module", label_key: "storage_module" }
    case "rest":
    case "standalone":
    case "standalone-rest":
    case "standalone rest":
    case "direct-rest":
    case "direct rest":
        return { key: "rest", effective: "rest", label_key: "storage_rest" }
    case "metrics":
    case "metrics-only":
    case "metrics only":
        return { key: "metrics", effective: "metrics", label_key: "metrics_only" }
    case "c-library":
    case "c library":
    case "library":
    case "local-os":
    case "local os":
    case "local diagnostics":
    case "unsupported":
        return { key: "unsupported", effective: "unsupported", label_key: "unsupported" }
    default:
        return { key: "auto", effective: "rest", label_key: "auto_storage_rest" }
    }
}

function canonicalRefreshRate(root, seconds) {
    with (root) {
        const value = Math.max(0, Number(seconds || 0))
        if (value === 0) {
            return 0
        }
        return Math.max(5, Math.min(3600, value))
    }
}

function networkConnectionRate(root, kind) {
    with (root) {
        switch (kind) {
        case "blockchain":
            return root.canonicalRefreshRate(blockchainRefreshRate)
        case "indexer":
            return root.canonicalRefreshRate(indexerRefreshRate)
        case "execution":
            return root.canonicalRefreshRate(executionRefreshRate)
        case "messaging":
            return root.canonicalRefreshRate(messagingRefreshRate)
        case "storage":
            return root.canonicalRefreshRate(storageRefreshRate)
        default:
            return 30
        }
    }
}

function setNetworkConnectionRate(root, kind, seconds) {
    with (root) {
        const value = root.canonicalRefreshRate(seconds)
        switch (kind) {
        case "blockchain":
            blockchainRefreshRate = value
            return
        case "indexer":
            indexerRefreshRate = value
            return
        case "execution":
            executionRefreshRate = value
            return
        case "messaging":
            messagingRefreshRate = value
            return
        case "storage":
            storageRefreshRate = value
        }
    }
}

function queryNetworkConnection(root, kind, showResult, includeSensitiveProbe) {
    with (root) {
        const target = String(kind || "")
        const configRevision = networkConfigurationRevision
        const request = root.networkConnectionRequest(target, includeSensitiveProbe === true)
        if (!request) {
            return {
                ok: false,
                text: "",
                error: qsTr("Unknown connection.")
            }
        }

        if (root.networkConnectionPending[target] === true) {
            return {
                ok: false,
                text: "",
                error: qsTr("Connection query already running.")
            }
        }

        root.setNetworkConnectionPending(target, true)
        return requestModuleAsync(request.module, request.method, request.args, request.label, showResult, function (response) {
            root.setNetworkConnectionPending(target, false)
            root.updateNetworkConnectionStatus(target, response)
            root.cacheNetworkConnectionResult(target, response)
            root.recordDashboardSnapshot()
        }, function () {
            return configRevision === networkConfigurationRevision
        })
    }
}

function refreshIndexerStatus(root) {
    with (root) {
        const statusResponse = root.requestModule(root.inspectorModule, "indexerStatus", root.indexerArgs([]), qsTr("Indexer status"), false, false)
        if (!statusResponse.ok) {
            root.setResult(qsTr("Indexer status"), statusResponse.error, true, null)
            return statusResponse
        }

        const statusValue = statusResponse.value && typeof statusResponse.value === "object" && !Array.isArray(statusResponse.value)
            ? root.copyMap(statusResponse.value)
            : { state: root.valueToString(statusResponse.value) }
        if (!root.indexerStatusNeedsFallback(statusValue)) {
            root.updateNetworkConnectionStatus("indexer", statusResponse)
            root.setResult(qsTr("Indexer status"), statusResponse.text, false, statusResponse.value)
            return statusResponse
        }

        const healthResponse = root.requestModule(root.inspectorModule, "indexerHealth", root.indexerArgs([]), qsTr("Indexer health"), false, false)
        const headResponse = root.requestModule(root.inspectorModule, "indexerFinalizedHead", root.indexerArgs([]), qsTr("Indexer head"), false, false)
        if (statusValue.indexedBlockId === undefined && headResponse.ok === true) {
            const head = root.scalarValue(headResponse.value)
            if (head !== null) {
                statusValue.indexedBlockId = head
            }
        }
        if ((statusValue.lastError === undefined || statusValue.lastError === null || statusValue.lastError === "")
                && (healthResponse.ok !== true || headResponse.ok !== true)) {
            const errors = []
            if (healthResponse.ok !== true && healthResponse.error) {
                errors.push(String(healthResponse.error))
            }
            if (headResponse.ok !== true && headResponse.error) {
                errors.push(String(headResponse.error))
            }
            if (errors.length > 0) {
                statusValue.lastError = errors.join("\n")
            }
        }

        const fallbackValue = {
            status: statusValue,
            indexer: {
                endpoint: indexerUrl,
                health: root.probeFieldFromResponse(healthResponse),
                head: root.probeFieldFromResponse(headResponse),
                programs: null
            }
        }
        root.updateNetworkConnectionStatus("indexer", {
            ok: true,
            value: fallbackValue,
            text: BridgeHelpers.formatValue(fallbackValue),
            error: ""
        })
        root.setResult(qsTr("Indexer status"), BridgeHelpers.formatValue(fallbackValue), false, fallbackValue)
        return {
            ok: true,
            value: fallbackValue,
            text: BridgeHelpers.formatValue(fallbackValue),
            error: ""
        }
    }
}

function indexerStatusNeedsFallback(root, value) {
    with (root) {
        const status = value && value.status && typeof value.status === "object" ? value.status : value
        if (!status || typeof status !== "object") {
            return false
        }
        const state = String(status.state || "").toLowerCase()
        const error = String(status.lastError || status.last_error || "").toLowerCase()
        return state === "unavailable"
            || state === "unsupported"
            || error.indexOf("method not found") >= 0
            || error.indexOf("-32601") >= 0
    }
}

function probeFieldFromResponse(root, response) {
    with (root) {
        if (response && response.ok === true) {
            return {
                ok: true,
                value: response.value === undefined ? null : response.value,
                error: null
            }
        }
        return {
            ok: false,
            value: null,
            error: response && response.error ? String(response.error) : qsTr("unavailable")
        }
    }
}

function networkConnectionRequest(root, kind, includeSensitiveProbe) {
    with (root) {
        switch (kind) {
        case "blockchain":
            return { module: inspectorModule, method: "blockchainNode", args: root.blockchainArgs([]), label: qsTr("Blockchain node") }
        case "indexer":
            return { module: inspectorModule, method: "indexerFinalizedHead", args: root.indexerArgs([]), label: qsTr("Indexer head") }
        case "execution":
            return { module: inspectorModule, method: "head", args: root.executionArgs([]), label: qsTr("Sequencer head") }
        case "messaging":
            return { module: inspectorModule, method: "deliverySourceReport", args: root.deliverySourceReportArgs(), label: qsTr("Delivery source") }
        case "storage":
            return { module: inspectorModule, method: "storageSourceReport", args: root.storageSourceReportArgs(includeSensitiveProbe), label: qsTr("Storage source") }
        default:
            return null
        }
    }
}

function blockchainArgs(root, extra) {
    with (root) {
        return coreSourceArgs(root, blockchainSourceMode, blockchainModule, nodeUrl, extra)
    }
}

function indexerArgs(root, extra) {
    with (root) {
        return coreSourceArgs(root, indexerSourceMode, indexerModule, indexerUrl, extra)
    }
}

function executionArgs(root, extra) {
    with (root) {
        return root.executionRpcArgs(extra)
    }
}

function blockchainRpcArgs(root, extra) {
    with (root) {
        return [String(nodeUrl || "")].concat(Array.isArray(extra) ? extra : [])
    }
}

function executionRpcArgs(root, extra) {
    with (root) {
        return [String(sequencerUrl || "")].concat(Array.isArray(extra) ? extra : [])
    }
}

function coreSourceArgs(root, sourceMode, moduleName, endpoint, extra) {
    with (root) {
        const rest = Array.isArray(extra) ? extra : []
        return [String(endpoint || "")].concat(rest)
    }
}

function accountLookupArgs(root, account, idlJson, accountType) {
    with (root) {
        const suffix = [String(account || "")]
        const idl = String(idlJson || "").trim()
        if (idl.length > 0) {
            suffix.push(idl)
            if (accountType !== undefined && accountType !== null && String(accountType).trim().length > 0) {
                suffix.push(String(accountType).trim())
            }
        }
        return [String(sequencerUrl || ""), String(indexerUrl || "")].concat(suffix)
    }
}

function updateNetworkConnectionStatusForMethod(root, method, response) {
    with (root) {
        const kind = root.networkConnectionKindForMethod(method)
        if (kind.length > 0) {
            root.updateNetworkConnectionStatus(kind, response)
        }
    }
}

function networkConnectionKindForMethod(root, method) {
    with (root) {
        switch (String(method || "")) {
        case "blockchainNode":
        case "blockchainLiveBlocks":
            return "blockchain"
        case "indexerStatus":
        case "indexerFinalizedHead":
            return "indexer"
        case "head":
            return "execution"
        case "deliveryReport":
        case "deliverySourceReport":
            return "messaging"
        case "storageReport":
        case "storageSourceReport":
            return "storage"
        default:
            return ""
        }
    }
}

function setNetworkConnectionPending(root, kind, pending) {
    with (root) {
        const next = copyMap(networkConnectionPending)
        next[String(kind || "")] = pending === true
        networkConnectionPending = next
        networkConnectionPendingRevision += 1
    }
}

function networkConnectionIsPending(root, kind) {
    with (root) {
        const revision = networkConnectionPendingRevision
        return networkConnectionPending[String(kind || "")] === true
    }
}

function updateNetworkConnectionStatus(root, kind, response) {
    with (root) {
        const next = copyMap(networkConnectionStatus)
        const value = response && response.value !== undefined ? response.value : null
        const ok = response && response.ok === true && root.connectionValueOk(kind, value)
        next[kind] = {
            known: true,
            ok: ok,
            text: ok ? qsTr("OK") : qsTr("Error"),
            detail: response && response.ok ? networkConnectionSummary(kind, value) : (response && response.error ? response.error : qsTr("No response")),
            value: value,
            checkedAt: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        }
        networkConnectionStatus = next
        networkConnectionStatusRevision += 1
    }
}

function cacheNetworkConnectionResult(root, kind, response) {
    with (root) {
        if (!response || response.ok !== true) {
            return
        }
        const target = String(kind || "")
        const value = response.value
        if (target === "blockchain") {
            dashboardNode = value || null
            const probe = value && value.cryptarchia_info ? value.cryptarchia_info : null
            const overview = root.copyMap(dashboardOverview || {})
            const node = root.copyMap(overview.node || {})
            if (probe) {
                node.consensus = {
                    ok: probe.ok === true,
                    value: probe.value === undefined ? null : probe.value,
                    error: probe.error === undefined ? null : probe.error
                }
            }
            node.endpoint = nodeUrl
            overview.node = node
            dashboardOverview = overview
            return
        }
        if (target === "execution") {
            const overview = root.copyMap(dashboardOverview || {})
            const sequencer = root.copyMap(overview.sequencer || {})
            sequencer.endpoint = sequencerUrl
            sequencer.health = { ok: true, value: "ok", error: null }
            sequencer.head = { ok: true, value: value === undefined ? null : value, error: null }
            overview.sequencer = sequencer
            dashboardOverview = overview
            return
        }
        if (target === "indexer") {
            const overview = root.copyMap(dashboardOverview || {})
            const indexer = root.copyMap(overview.indexer || {})
            indexer.endpoint = indexerUrl
            indexer.health = { ok: true, value: "ok", error: null }
            indexer.head = { ok: true, value: value === undefined ? null : value, error: null }
            overview.indexer = indexer
            dashboardOverview = overview
        }
    }
}

function networkConnectionSummary(root, kind, value) {
    with (root) {
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
            const health = root.sourceHealth(value)
            if (health && health.summary) {
                return health.ready === true ? String(health.summary) : String(health.detail || health.summary)
            }
            if (!root.moduleReportReachable(value)) {
                return root.moduleReportError(value) || qsTr("source unavailable")
            }
            if (!root.deliveryReportHealthy(value)) {
                const nodeHealth = root.reportProbeValue(value, "nodeHealth")
                const connectionStatus = root.reportProbeValue(value, "connectionStatus")
                return qsTr("health %1 / %2").arg(root.valueText(nodeHealth)).arg(root.valueText(connectionStatus))
            }
            const version = root.moduleProbeValue("messaging", "version")
            return version !== null ? qsTr("version %1").arg(root.valueText(version)) : qsTr("%1 reachable").arg(root.deliverySourceLabel())
        }
        if (kind === "storage") {
            const health = root.sourceHealth(value)
            if (health && health.summary) {
                return health.ready === true ? String(health.summary) : String(health.detail || health.summary)
            }
            if (!root.moduleReportReachable(value)) {
                return root.moduleReportError(value) || qsTr("source unavailable")
            }
            if (String(value && value.module ? value.module : "") === "storage_metrics") {
                return qsTr("metrics available")
            }
            const version = root.moduleProbeValue("storage", "version") || root.moduleProbeValue("storage", "moduleVersion")
            return version !== null ? qsTr("version %1").arg(root.valueText(version)) : qsTr("%1 reachable").arg(root.storageSourceLabel())
        }
        return qsTr("reachable")
    }
}

function connectionValueOk(root, kind, value) {
    with (root) {
        if (kind === "messaging") {
            return root.moduleReportReachable(value) && root.deliveryReportHealthy(value)
        }
        if (kind === "storage") {
            return root.storageReportReady(value)
        }
        return true
    }
}

function storageReportReady(root, report) {
    with (root) {
        const ready = root.sourceHealthReady(report)
        if (ready !== null) {
            return ready
        }
        if (!root.moduleReportReachable(report)) {
            return false
        }
        const moduleName = String(report && report.module ? report.module : "")
        if (moduleName === "storage_metrics") {
            return true
        }
        if (moduleName === "storage_rest") {
            return root.reportProbeOk(report, "peerId")
                && root.reportProbeOk(report, "spr")
                && root.reportProbeOk(report, "space")
                && root.reportProbeOk(report, "manifests")
        }
        return root.reportProbeOk(report, "peerId")
            || root.reportProbeOk(report, "spr")
            || root.reportProbeOk(report, "space")
            || root.reportProbeOk(report, "debug")
            || root.reportProbeOk(report, "manifests")
    }
}

function moduleReportReachable(root, report) {
    with (root) {
        if (!report || typeof report !== "object") {
            return false
        }
        const health = root.sourceHealth(report)
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
}

function sourceHealth(root, report) {
    with (root) {
        const health = report && report.health && typeof report.health === "object" && !Array.isArray(report.health)
            ? report.health
            : null
        return health
    }
}

function sourceHealthReady(root, report) {
    with (root) {
        const health = root.sourceHealth(report)
        if (health && health.ready !== undefined) {
            return health.ready === true
        }
        return null
    }
}

function sourceCapability(root, report, key) {
    with (root) {
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
}

function sourceCapabilityAvailable(root, report, key) {
    with (root) {
        const fact = root.sourceCapability(report, key)
        return fact !== null ? fact.available === true : null
    }
}

function sourceCapabilityEvidence(root, report, key) {
    with (root) {
        const fact = root.sourceCapability(report, key)
        return fact && fact.evidence !== undefined ? String(fact.evidence) : ""
    }
}

function sourceCapabilityValue(root, report, key) {
    with (root) {
        const fact = root.sourceCapability(report, key)
        return fact && fact.value !== undefined ? fact.value : null
    }
}

function reportProbeValue(root, report, method) {
    with (root) {
        const probe = root.reportProbe(report, method)
        if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
            return null
        }
        return probe.value
    }
}

function reportProbeOk(root, report, method) {
    with (root) {
        const probe = root.reportProbe(report, method)
        return probe !== null && probe.ok === true
    }
}

function reportProbe(root, report, method) {
    with (root) {
        if (!report || typeof report !== "object") {
            return null
        }
        const wanted = String(method || "")
        const moduleInfo = report.module_info || null
        if (moduleInfo) {
            const label = String(moduleInfo.label || "")
            const source = String(moduleInfo.source || "")
            if (label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0) {
                return moduleInfo
            }
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            const probe = probes[i] || {}
            const label = String(probe.label || "")
            const source = String(probe.source || "")
            if (label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0) {
                return probe
            }
        }
        return null
    }
}

function deliveryReportHealthy(root, report) {
    with (root) {
        const ready = root.sourceHealthReady(report)
        if (ready !== null) {
            return ready
        }
        const moduleName = String(report && report.module ? report.module : "")
        if (moduleName === "delivery_metrics") {
            return deliveryMetricsEvidencePresent(root, report)
        }
        if (moduleName === "delivery_network_monitor") {
            return root.reportProbeOk(report, "allPeersInfo")
                || root.reportProbeOk(report, "contentTopics")
                || deliveryMetricsEvidencePresent(root, report)
        }
        if (moduleName === "delivery_rest" && !root.reportProbeOk(report, "health")) {
            return false
        }
        const nodeProbe = root.reportProbe(report, "nodeHealth")
        const connectionProbe = root.reportProbe(report, "connectionStatus")
        if (moduleName === "delivery_rest" && !connectionProbe) {
            return false
        }
        if (moduleName === deliveryModule && !nodeProbe && !connectionProbe) {
            return root.deliveryModuleRuntimeHealthy(report)
        }
        if (!nodeProbe && !connectionProbe) {
            return true
        }
        const nodeHealth = nodeProbe && nodeProbe.ok === true ? nodeProbe.value : null
        const connectionStatus = connectionProbe && connectionProbe.ok === true ? connectionProbe.value : null
        return root.deliveryHealthValueOk(nodeHealth, false) && root.deliveryHealthValueOk(connectionStatus, false)
    }
}

function deliveryMetricsEvidencePresent(root, report) {
    with (root) {
        const probe = root.reportProbe(report, "collectOpenMetricsText")
        if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
            return false
        }
        const text = root.openMetricsTextFromValue(probe.value)
        if (!text.length) {
            return false
        }
        const knownNames = [
            "libp2p_peers",
            "waku_peers",
            "libp2p_pubsub_peers",
            "waku_node_messages_total",
            "waku_node_errors_total",
            "waku_store_queries_total",
            "waku_filter_peers",
            "waku_lightpush_peers"
        ]
        for (let i = 0; i < knownNames.length; ++i) {
            if (text.indexOf(knownNames[i]) >= 0) {
                return true
            }
        }
        return false
    }
}

function deliveryModuleRuntimeHealthy(root, report) {
    with (root) {
        const runtimeMethods = ["Metrics", "collectOpenMetricsText"]
        for (let i = 0; i < runtimeMethods.length; ++i) {
            if (root.deliveryProbeHasRuntimeValue(root.reportProbe(report, runtimeMethods[i]))) {
                return true
            }
        }
        return false
    }
}

function deliveryProbeHasRuntimeValue(root, probe) {
    with (root) {
        if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
            return false
        }
        if (Array.isArray(probe.value)) {
            return probe.value.length > 0
        }
        if (typeof probe.value === "object") {
            return Object.keys(probe.value).length > 0
        }
        const scalar = root.scalarValue(probe.value)
        if (typeof scalar === "boolean") {
            return scalar
        }
        return scalar !== null && String(scalar).trim().length > 0
    }
}

function deliveryHealthValueOk(root, value, unknownOk) {
    with (root) {
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
}

function moduleReportError(root, report) {
    with (root) {
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
}

function deliverySourceReportArgs(root) {
    with (root) {
        const source = root.effectiveMessagingSourceMode(messagingSourceMode)
        return [
            source,
            source === "rest" || source === "network-monitor" ? root.configuredMessagingRestUrl() : "",
            source === "rest" || source === "metrics" || source === "network-monitor" ? String(messagingMetricsUrl || "") : ""
        ]
    }
}

function deliverySourceLabel(root) {
    with (root) {
        const source = root.normalizedMessagingSourceMode(messagingSourceMode)
        if (source === "auto") {
            return qsTr("Auto: Direct Waku REST")
        }
        const effective = root.effectiveMessagingSourceMode(messagingSourceMode)
        switch (effective) {
        case "module":
            return qsTr("Delivery module")
        case "rest":
            return qsTr("Direct Waku REST")
        case "metrics":
            return qsTr("Metrics only")
        case "network-monitor":
            return qsTr("Network monitor")
        case "unsupported":
            return qsTr("Unsupported source")
        default:
            return qsTr("Direct Waku REST")
        }
    }
}

function deliverySourceTarget(root) {
    with (root) {
        switch (root.effectiveMessagingSourceMode(messagingSourceMode)) {
        case "module":
            return deliveryModule
        case "rest":
            return root.configuredMessagingRestUrl()
        case "metrics":
            return String(messagingMetricsUrl || "")
        case "network-monitor":
            return root.configuredMessagingRestUrl()
        default:
            return ""
        }
    }
}

function configuredMessagingRestUrl(root) {
    with (root) {
        const value = String(messagingRestUrl || "").trim()
        return value.length ? value : root.sourcePolicyDefault("delivery_rest_endpoint", "http://127.0.0.1:8645")
    }
}

function normalizedCoreSourceMode(root, value) {
    with (root) {
        const source = root.sourceModePolicy("core", value)
        return source.key === "module" ? "auto" : String(source.key || "auto")
    }
}

function effectiveCoreSourceMode(root, value) {
    with (root) {
        const source = root.sourceModePolicy("core", root.normalizedCoreSourceMode(value))
        return String(source.effective || "rpc")
    }
}

function blockchainSourceLabel(root) {
    with (root) {
        return coreSourceLabel(root, blockchainSourceMode, qsTr("Bedrock RPC"))
    }
}

function blockchainSourceTarget(root) {
    with (root) {
        return String(nodeUrl || "")
    }
}

function indexerSourceLabel(root) {
    with (root) {
        return coreSourceLabel(root, indexerSourceMode, qsTr("Indexer RPC"))
    }
}

function indexerSourceTarget(root) {
    with (root) {
        return String(indexerUrl || "")
    }
}

function executionSourceLabel(root) {
    with (root) {
        return qsTr("Sequencer RPC")
    }
}

function executionSourceTarget(root) {
    with (root) {
        return String(sequencerUrl || "")
    }
}

function coreSourceLabel(root, sourceMode, rpcLabel) {
    with (root) {
        const source = root.normalizedCoreSourceMode(sourceMode)
        if (source === "rpc") {
            return rpcLabel
        }
        return qsTr("Auto: %1").arg(rpcLabel)
    }
}

function normalizedMessagingSourceMode(root, value) {
    with (root) {
        return String(root.sourceModePolicy("delivery", value).key || "auto")
    }
}

function effectiveMessagingSourceMode(root, value) {
    with (root) {
        return String(root.sourceModePolicy("delivery", value).effective || "rest")
    }
}

function storageSourceReportArgs(root, includeCidProbe) {
    with (root) {
        const source = root.effectiveStorageSourceMode(storageSourceMode)
        return [
            source,
            source === "rest" ? root.configuredStorageRestUrl() : "",
            source === "rest" || source === "metrics" ? String(storageMetricsUrl || "") : "",
            includeCidProbe === true ? String(storageCidProbe || "") : "",
            storagePrivilegedDebugEnabled === true
        ]
    }
}

function storageSourceLabel(root) {
    with (root) {
        const source = root.normalizedStorageSourceMode(storageSourceMode)
        if (source === "auto") {
            return qsTr("Auto: Standalone REST")
        }
        const effective = root.effectiveStorageSourceMode(storageSourceMode)
        switch (effective) {
        case "module":
            return qsTr("Storage module")
        case "rest":
            return qsTr("Standalone REST")
        case "metrics":
            return qsTr("Metrics only")
        case "unsupported":
            return qsTr("Unsupported source")
        default:
            return qsTr("Standalone REST")
        }
    }
}

function storageSourceTarget(root) {
    with (root) {
        switch (root.effectiveStorageSourceMode(storageSourceMode)) {
        case "module":
            return storageModule
        case "rest":
            return root.configuredStorageRestUrl()
        case "metrics":
            return String(storageMetricsUrl || "")
        case "unsupported":
            return ""
        default:
            return ""
        }
    }
}

function configuredStorageRestUrl(root) {
    with (root) {
        const value = String(storageRestUrl || "").trim()
        return value.length ? value : root.sourcePolicyDefault("storage_rest_endpoint", "http://127.0.0.1:8080/api/storage/v1")
    }
}

function normalizedStorageSourceMode(root, value) {
    with (root) {
        return String(root.sourceModePolicy("storage", value).key || "auto")
    }
}

function effectiveStorageSourceMode(root, value) {
    with (root) {
        return String(root.sourceModePolicy("storage", value).effective || "rest")
    }
}

function networkConnectionState(root, kind) {
    with (root) {
        const revision = networkConnectionStatusRevision
        const status = networkConnectionStatus[String(kind || "")]
        if (!status) {
            return {
                known: false,
                ok: false,
                text: qsTr("Unknown"),
                detail: qsTr("Not queried"),
                checkedAt: ""
            }
        }
        return status
    }
}

function setFooterFieldEnabled(root, key, enabled) {
    with (root) {
        const next = copyMap(footerFieldSelections)
        next[String(key || "")] = enabled === true
        footerFieldSelections = next
        footerFieldRevision += 1
    }
}

function footerFieldEnabled(root, key) {
    with (root) {
        const revision = footerFieldRevision
        const value = footerFieldSelections[String(key || "")]
        return value === true
    }
}

function setDashboardGraphEnabled(root, key, enabled) {
    with (root) {
        const next = copyMap(dashboardGraphSelections)
        next[String(key || "")] = enabled === true
        dashboardGraphSelections = next
        dashboardGraphRevision += 1
    }
}

function dashboardGraphEnabled(root, key) {
    with (root) {
        const revision = dashboardGraphRevision
        const value = dashboardGraphSelections[String(key || "")]
        return value === true
    }
}

function copyMap(root, source) {
    with (root) {
        const next = {}
        const current = source || {}
        for (const key in current) {
            next[key] = current[key]
        }
        return next
    }
}

function mergeMap(root, base, overrides) {
    with (root) {
        const next = root.copyMap(base)
        const current = overrides || {}
        for (const key in current) {
            next[key] = current[key]
        }
        return next
    }
}

function stringSetting(root, value, key, fallback) {
    with (root) {
        const raw = value ? value[key] : undefined
        return raw === undefined || raw === null ? String(fallback || "") : String(raw)
    }
}

function numberSetting(root, value, key, fallback) {
    with (root) {
        const number = Number(value ? value[key] : undefined)
        return Number.isFinite(number) ? number : Number(fallback || 0)
    }
}

function boolSetting(root, value, key, fallback) {
    with (root) {
        const raw = value ? value[key] : undefined
        if (raw === true || raw === false) {
            return raw
        }
        return fallback === true
    }
}

function normalizedNetworkProfile(root, value) {
    with (root) {
        const profile = String(value || "default")
        if (profile === "local" || profile === "custom") {
            return profile
        }
        return "default"
    }
}

function resolvedNetworkProfile(root, storedProfile, sequencer, indexer, node) {
    with (root) {
        const inferred = root.inferNetworkProfileFromEndpoints(sequencer, indexer, node)
        if (inferred !== "custom") {
            return inferred
        }
        return root.normalizedNetworkProfile(storedProfile) === "custom" ? "custom" : inferred
    }
}

function inferNetworkProfileFromEndpoints(root, sequencer, indexer, node) {
    with (root) {
        const seq = root.normalizeEndpoint(sequencer)
        const idx = root.normalizeEndpoint(indexer)
        const nod = root.normalizeEndpoint(node)
        const profiles = sourcePolicy && Array.isArray(sourcePolicy.network_profiles)
            ? sourcePolicy.network_profiles
            : fallbackNetworkProfiles(root)
        for (let i = 0; i < profiles.length; ++i) {
            const profile = profiles[i] || {}
            if (seq === root.normalizeEndpoint(profile.sequencer_endpoint)
                    && idx === root.normalizeEndpoint(profile.indexer_endpoint)
                    && nod === root.normalizeEndpoint(profile.node_endpoint)) {
                return String(profile.id || "custom")
            }
        }
        return "custom"
    }
}

function fallbackNetworkProfiles(root) {
    with (root) {
        return [
            {
                id: "default",
                sequencer_endpoint: root.sourcePolicyDefault("sequencer_endpoint", "https://testnet.lez.logos.co/"),
                indexer_endpoint: root.sourcePolicyDefault("indexer_endpoint", "http://127.0.0.1:8779/"),
                node_endpoint: root.sourcePolicyDefault("node_endpoint", "http://127.0.0.1:8080/")
            },
            {
                id: "local",
                sequencer_endpoint: root.sourcePolicyDefault("local_sequencer_endpoint", "http://127.0.0.1:3040/"),
                indexer_endpoint: root.sourcePolicyDefault("indexer_endpoint", "http://127.0.0.1:8779/"),
                node_endpoint: root.sourcePolicyDefault("node_endpoint", "http://127.0.0.1:8080/")
            }
        ]
    }
}

function normalizeEndpoint(root, value) {
    with (root) {
        return String(value || "").trim().replace(/\/+$/, "")
    }
}

function normalizedMessagingNetworkPreset(root, value) {
    with (root) {
        const preset = String(value || "").trim()
        if (!preset.length || preset === "testnet") {
            return "logos.test"
        }
        return preset
    }
}

function scalarValue(root, value) {
    with (root) {
        if (value === undefined || value === null || value === "") {
            return null
        }
        if (typeof value === "number" || typeof value === "string" || typeof value === "boolean") {
            return value
        }
        if (Array.isArray(value)) {
            return value.length
        }
        if (typeof value === "object") {
            if (value.result !== undefined && value.result !== null) {
                return root.scalarValue(value.result)
            }
            if (value.value !== undefined && value.value !== null) {
                return root.scalarValue(value.value)
            }
            if (value.count !== undefined && value.count !== null) {
                return root.scalarValue(value.count)
            }
            if (value.total !== undefined && value.total !== null) {
                return root.scalarValue(value.total)
            }
        }
        return null
    }
}
