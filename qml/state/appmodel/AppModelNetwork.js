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
            root.recordDashboardSnapshot()
        }, function () {
            return configRevision === networkConfigurationRevision
        })
    }
}

function refreshIndexerStatus(root) {
    with (root) {
        const statusResponse = root.requestModule(root.inspectorModule, "indexerStatus", [indexerUrl], qsTr("Indexer status"), false, false)
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

        const healthResponse = root.requestModule(root.inspectorModule, "indexerHealth", [indexerUrl], qsTr("Indexer health"), false, false)
        const headResponse = root.requestModule(root.inspectorModule, "indexerFinalizedHead", [indexerUrl], qsTr("Indexer head"), false, false)
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
            return { module: inspectorModule, method: "blockchainNode", args: [nodeUrl], label: qsTr("Blockchain node") }
        case "indexer":
            return { module: inspectorModule, method: "indexerFinalizedHead", args: [indexerUrl], label: qsTr("Indexer head") }
        case "execution":
            return { module: inspectorModule, method: "head", args: [sequencerUrl], label: qsTr("Sequencer head") }
        case "messaging":
            return { module: inspectorModule, method: "deliverySourceReport", args: root.deliverySourceReportArgs(), label: qsTr("Delivery source") }
        case "storage":
            return { module: inspectorModule, method: "storageSourceReport", args: root.storageSourceReportArgs(includeSensitiveProbe), label: qsTr("Storage source") }
        default:
            return null
        }
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

function networkConnectionSummary(root, kind, value) {
    with (root) {
        if (kind === "blockchain") {
            const info = value && value.cryptarchia_info ? value.cryptarchia_info : null
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
            if (!root.moduleReportReachable(value)) {
                return root.moduleReportError(value) || qsTr("source unavailable")
            }
            if (!root.deliveryReportHealthy(value)) {
                const nodeHealth = root.reportProbeValue(value, "nodeHealth")
                const connectionStatus = root.reportProbeValue(value, "connectionStatus")
                const moduleName = String(value && value.module ? value.module : "")
                if (moduleName === deliveryModule && nodeHealth === null && connectionStatus === null) {
                    return qsTr("runtime health unavailable")
                }
                return qsTr("health %1 / %2").arg(root.valueText(nodeHealth)).arg(root.valueText(connectionStatus))
            }
            const version = root.moduleProbeValue("messaging", "version")
            return version !== null ? qsTr("version %1").arg(root.valueText(version)) : qsTr("%1 reachable").arg(root.deliverySourceLabel())
        }
        if (kind === "storage") {
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
        if (!root.moduleReportReachable(report)) {
            return false
        }
        const moduleName = String(report && report.module ? report.module : "")
        if (moduleName === "storage_metrics") {
            return true
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
        const moduleName = String(report && report.module ? report.module : "")
        if (moduleName === "delivery_metrics") {
            return true
        }
        if (moduleName === "delivery_rest" && !root.reportProbeOk(report, "health")) {
            return false
        }
        const nodeProbe = root.reportProbe(report, "nodeHealth")
        const connectionProbe = root.reportProbe(report, "connectionStatus")
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
        return [
            root.normalizedMessagingSourceMode(messagingSourceMode),
            String(messagingRestUrl || ""),
            String(messagingMetricsUrl || ""),
            String(messagingNodeInfoId || "")
        ]
    }
}

function deliverySourceLabel(root) {
    with (root) {
        switch (root.normalizedMessagingSourceMode(messagingSourceMode)) {
        case "rest":
            return qsTr("Direct Waku REST")
        case "metrics":
            return qsTr("Metrics only")
        default:
            return qsTr("Basecamp module")
        }
    }
}

function deliverySourceTarget(root) {
    with (root) {
        switch (root.normalizedMessagingSourceMode(messagingSourceMode)) {
        case "rest":
            return String(messagingRestUrl || "")
        case "metrics":
            return String(messagingMetricsUrl || "")
        default:
            return String(deliveryModule || "")
        }
    }
}

function normalizedMessagingSourceMode(root, value) {
    with (root) {
        const source = String(value || "module").trim().toLowerCase()
        switch (source) {
        case "rest":
        case "direct-rest":
        case "direct waku rest":
        case "waku-rest":
            return "rest"
        case "metrics":
        case "metrics-only":
        case "metrics only":
            return "metrics"
        case "module":
        case "basecamp":
        case "basecamp-module":
        case "basecamp module":
        default:
            return "module"
        }
    }
}

function storageSourceReportArgs(root, includeCidProbe) {
    with (root) {
        return [
            root.normalizedStorageSourceMode(storageSourceMode),
            String(storageRestUrl || ""),
            String(storageMetricsUrl || ""),
            includeCidProbe === true ? String(storageCidProbe || "") : "",
            storagePrivilegedDebugEnabled === true
        ]
    }
}

function storageSourceLabel(root) {
    with (root) {
        switch (root.normalizedStorageSourceMode(storageSourceMode)) {
        case "rest":
            return qsTr("Standalone REST")
        case "metrics":
            return qsTr("Metrics only")
        case "c-library":
            return qsTr("C library")
        case "local-os":
            return qsTr("Local OS diagnostics")
        default:
            return qsTr("Basecamp module")
        }
    }
}

function storageSourceTarget(root) {
    with (root) {
        switch (root.normalizedStorageSourceMode(storageSourceMode)) {
        case "rest":
            return String(storageRestUrl || "")
        case "metrics":
            return String(storageMetricsUrl || "")
        case "c-library":
        case "local-os":
            return String(storageDataDir || storageNetworkPreset || "")
        default:
            return String(storageModule || "")
        }
    }
}

function normalizedStorageSourceMode(root, value) {
    with (root) {
        const source = String(value || "module").trim().toLowerCase()
        switch (source) {
        case "rest":
        case "standalone-rest":
        case "standalone rest":
        case "direct-rest":
            return "rest"
        case "metrics":
        case "metrics-only":
        case "metrics only":
            return "metrics"
        case "c-library":
        case "c library":
        case "library":
            return "c-library"
        case "local-os":
        case "local os":
        case "local diagnostics":
            return "local-os"
        case "module":
        case "basecamp":
        case "basecamp-module":
        case "basecamp module":
        default:
            return "module"
        }
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
        const testnetSeq = root.normalizeEndpoint("https://testnet.lez.logos.co/")
        const localSeq = root.normalizeEndpoint("http://127.0.0.1:3040/")
        const localIndexer = root.normalizeEndpoint("http://127.0.0.1:8779/")
        const localNode = root.normalizeEndpoint("http://127.0.0.1:8080/")

        if (seq === localSeq && idx === localIndexer && nod === localNode) {
            return "local"
        }
        if (seq === testnetSeq && idx === localIndexer && nod === localNode) {
            return "default"
        }
        return "custom"
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
