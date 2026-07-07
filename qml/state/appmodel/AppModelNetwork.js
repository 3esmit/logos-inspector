.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "SourceHealthProjection.js" as SourceHealthProjection
.import "SourcePolicyProjection.js" as SourcePolicyProjection
.import "SourceRoutingUi.js" as SourceRoutingUi

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
    return SourcePolicyProjection.sourcePolicyDefault(root, key, fallback)
}

function sourceModePolicy(root, family, value) {
    return SourcePolicyProjection.sourceModePolicy(root, family, value)
}

function sourceModePolicies(root, family) {
    return SourcePolicyProjection.sourceModePolicies(root, family)
}

function sourceModeOptions(root, family) {
    return SourcePolicyProjection.sourceModeOptions(root, family)
}

function sourceModeIndexFor(root, family, value, options) {
    return SourcePolicyProjection.sourceModeIndexFor(root, family, value, options)
}

function sourceModeAt(root, index, options) {
    return SourcePolicyProjection.sourceModeAt(index, options)
}

function sourceModeAdapter(root, family, value) {
    return SourcePolicyProjection.sourceModeAdapter(root, family, value)
}

function resolvedSourceModeKey(root, family, value) {
    return SourcePolicyProjection.resolvedSourceModeKey(root, family, value)
}

function sourceModeTargetKind(root, family, value) {
    return SourcePolicyProjection.sourceModeTargetKind(root, family, value)
}

function sourceModeUsesEndpoint(root, family, value, endpointKind) {
    return SourcePolicyProjection.sourceModeUsesEndpoint(root, family, value, endpointKind)
}

function sourceModeSupportsCidProbe(root, family, value) {
    return SourcePolicyProjection.sourceModeSupportsCidProbe(root, family, value)
}

function sourceModeSupportsMutatingDiagnostics(root, family, value) {
    return SourcePolicyProjection.sourceModeSupportsMutatingDiagnostics(root, family, value)
}

function coreSourceView(root, role) {
    return SourceRoutingUi.coreSourceView(root, role)
}

function deliverySourceView(root) {
    return SourceRoutingUi.deliverySourceView(root)
}

function storageSourceView(root) {
    return SourceRoutingUi.storageSourceView(root)
}

function deliveryReportView(root, report) {
    return SourceRoutingUi.deliveryReportView(root, report)
}

function storageReportView(root, report) {
    return SourceRoutingUi.storageReportView(root, report)
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
        return coreSourceArgs(root, blockchainSourceMode, nodeUrl, extra)
    }
}

function indexerArgs(root, extra) {
    with (root) {
        return coreSourceArgs(root, indexerSourceMode, indexerUrl, extra)
    }
}

function executionArgs(root, extra) {
    with (root) {
        return coreSourceArgs(root, executionSourceMode, sequencerUrl, extra)
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

function coreSourceArgs(root, sourceMode, endpoint, extra) {
    return SourcePolicyProjection.coreSourceArgs(root, sourceMode, endpoint, extra)
}

function accountLookupArgs(root, account, idlJson, accountType) {
    with (root) {
        return SourcePolicyProjection.accountLookupArgs(
            root,
            executionSourceMode,
            sequencerUrl,
            indexerSourceMode,
            indexerUrl,
            account,
            idlJson,
            accountType
        )
    }
}

function lezLookupArgs(root, target) {
    with (root) {
        return SourcePolicyProjection.lezLookupArgs(
            root,
            executionSourceMode,
            sequencerUrl,
            indexerSourceMode,
            indexerUrl,
            target
        )
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
            return
        }
        if (target === "messaging") {
            messagingModuleReport = value || null
            return
        }
        if (target === "storage") {
            storageModuleReport = value || null
        }
    }
}

function networkConnectionSummary(root, kind, value) {
    return SourceHealthProjection.networkConnectionSummary(root, kind, value)
}

function connectionValueOk(root, kind, value) {
    return SourceHealthProjection.connectionValueOk(root, kind, value)
}

function storageReportReady(root, report) {
    return SourceHealthProjection.storageReportReady(root, report)
}

function moduleReportReachable(root, report) {
    return SourceHealthProjection.moduleReportReachable(root, report)
}

function sourceHealth(root, report) {
    return SourceHealthProjection.sourceHealth(report)
}

function sourceHealthReady(root, report) {
    return SourceHealthProjection.sourceHealthReady(report)
}

function sourceCapability(root, report, key) {
    return SourceHealthProjection.sourceCapability(report, key)
}

function sourceCapabilityAvailable(root, report, key) {
    return SourceHealthProjection.sourceCapabilityAvailable(report, key)
}

function sourceCapabilityEvidence(root, report, key) {
    return SourceHealthProjection.sourceCapabilityEvidence(report, key)
}

function sourceCapabilityValue(root, report, key) {
    return SourceHealthProjection.sourceCapabilityValue(report, key)
}

function sourceProbeFact(root, report, key) {
    return SourceHealthProjection.sourceProbeFact(report, key)
}

function sourceProbeValue(root, report, key) {
    return SourceHealthProjection.sourceProbeValue(report, key)
}

function reportProbeValue(root, report, method) {
    return SourceHealthProjection.reportProbeValue(report, method)
}

function reportProbeOk(root, report, method) {
    return SourceHealthProjection.reportProbeOk(report, method)
}

function reportProbe(root, report, method) {
    return SourceHealthProjection.reportProbe(report, method)
}

function deliveryReportHealthy(root, report) {
    return SourceHealthProjection.deliveryReportHealthy(root, report)
}

function deliveryHealthValueOk(root, value, unknownOk) {
    return SourceHealthProjection.deliveryHealthValueOk(root, value, unknownOk)
}

function moduleReportError(root, report) {
    return SourceHealthProjection.moduleReportError(report)
}

function deliverySourceReportArgs(root) {
    with (root) {
        return SourcePolicyProjection.deliverySourceReportArgs(
            root,
            messagingSourceMode,
            root.configuredMessagingRestUrl(),
            messagingMetricsUrl
        )
    }
}

function deliverySourceLabel(root) {
    with (root) {
        return SourcePolicyProjection.sourceLabel(root, "delivery", messagingSourceMode, qsTr("Direct Waku REST"))
    }
}

function deliverySourceTarget(root) {
    with (root) {
        return SourcePolicyProjection.sourceTarget(root, "delivery", messagingSourceMode, {
            module: deliveryModule,
            rest: root.configuredMessagingRestUrl(),
            metrics: messagingMetricsUrl
        })
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
        return String(source.key || "auto")
    }
}

function effectiveCoreSourceMode(root, value) {
    with (root) {
        const source = root.sourceModePolicy("core", root.resolvedSourceModeKey("core", value))
        return String(source.effective || "rpc")
    }
}

function blockchainSourceLabel(root) {
    with (root) {
        return SourcePolicyProjection.coreSourceLabel(root, blockchainSourceMode, qsTr("Bedrock RPC"))
    }
}

function blockchainSourceTarget(root) {
    with (root) {
        if (root.effectiveCoreSourceMode(blockchainSourceMode) === "module") {
            return blockchainModule
        }
        return String(nodeUrl || "")
    }
}

function indexerSourceLabel(root) {
    with (root) {
        return SourcePolicyProjection.coreSourceLabel(root, indexerSourceMode, qsTr("Indexer RPC"))
    }
}

function indexerSourceTarget(root) {
    with (root) {
        if (root.effectiveCoreSourceMode(indexerSourceMode) === "module") {
            return indexerModule
        }
        return String(indexerUrl || "")
    }
}

function executionSourceLabel(root) {
    with (root) {
        if (root.effectiveCoreSourceMode(executionSourceMode) === "module") {
            return qsTr("LEZ core module")
        }
        return qsTr("Sequencer RPC")
    }
}

function executionSourceTarget(root) {
    with (root) {
        if (root.effectiveCoreSourceMode(executionSourceMode) === "module") {
            return "lez_core"
        }
        return String(sequencerUrl || "")
    }
}

function normalizedMessagingSourceMode(root, value) {
    with (root) {
        return String(root.sourceModePolicy("delivery", value).key || "auto")
    }
}

function effectiveMessagingSourceMode(root, value) {
    with (root) {
        return String(root.sourceModePolicy("delivery", root.resolvedSourceModeKey("delivery", value)).effective || "rest")
    }
}

function storageSourceReportArgs(root, includeCidProbe) {
    with (root) {
        return SourcePolicyProjection.storageSourceReportArgs(
            root,
            storageSourceMode,
            root.configuredStorageRestUrl(),
            storageMetricsUrl,
            storageCidProbe,
            includeCidProbe,
            storagePrivilegedDebugEnabled
        )
    }
}

function storageSourceLabel(root) {
    with (root) {
        return SourcePolicyProjection.sourceLabel(root, "storage", storageSourceMode, qsTr("Standalone REST"))
    }
}

function storageSourceTarget(root) {
    with (root) {
        return SourcePolicyProjection.sourceTarget(root, "storage", storageSourceMode, {
            module: storageModule,
            rest: root.configuredStorageRestUrl(),
            metrics: storageMetricsUrl
        })
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
        return String(root.sourceModePolicy("storage", root.resolvedSourceModeKey("storage", value)).effective || "rest")
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
