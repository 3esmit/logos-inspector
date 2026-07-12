.import "../source_routing/SourceHealthProjection.js" as SourceHealthProjection
.import "../source_routing/SourceRoutingUi.js" as SourceRoutingUi

function refreshInterval(root, seconds) {
    with (root) {
        return Math.max(5, Number(seconds || 0)) * 1000
    }
}

function dashboardRefreshInterval(root) {
    with (root) {
        const rates = [
            blockchainRefreshRate,
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

function coreSourceView(root, role) {
    if (root.sourceRouting && typeof root.sourceRouting.coreSourceView === "function") {
        return root.sourceRouting.coreSourceView(role)
    }
    return SourceRoutingUi.coreSourceView(root, role)
}

function deliverySourceView(root) {
    if (root.sourceRouting && typeof root.sourceRouting.deliverySourceView === "function") {
        return root.sourceRouting.deliverySourceView()
    }
    return SourceRoutingUi.deliverySourceView(root)
}

function storageSourceView(root) {
    if (root.sourceRouting && typeof root.sourceRouting.storageSourceView === "function") {
        return root.sourceRouting.storageSourceView()
    }
    return SourceRoutingUi.storageSourceView(root)
}

function deliveryReportView(root, report) {
    if (root.sourceRouting && typeof root.sourceRouting.deliveryReportView === "function") {
        return root.sourceRouting.deliveryReportView(report)
    }
    return SourceRoutingUi.deliveryReportView(root, report)
}

function storageReportView(root, report) {
    if (root.sourceRouting && typeof root.sourceRouting.storageReportView === "function") {
        return root.sourceRouting.storageReportView(report)
    }
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

function networkConnectionRequest(root, kind, includeSensitiveProbe) {
    with (root) {
        switch (kind) {
        case "blockchain":
            return { module: inspectorModule, method: "blockchainNode", args: root.blockchainArgs([]), label: qsTr("Blockchain node") }
        case "messaging":
            return { module: inspectorModule, method: "deliverySourceReport", args: root.deliverySourceReportArgs(), label: qsTr("Delivery source") }
        case "storage":
            return { module: inspectorModule, method: "storageSourceReport", args: root.storageSourceReportArgs(includeSensitiveProbe), label: qsTr("Storage source") }
        default:
            return null
        }
    }
}

function blockchainRpcArgs(root, extra) {
    with (root) {
        return [String(nodeUrl || "")].concat(Array.isArray(extra) ? extra : [])
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
        case "deliverySourceReport":
            return "messaging"
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
        if (typeof root.refreshCapabilityRegistryIfLoaded === "function"
                && typeof Qt !== "undefined"
                && typeof Qt.callLater === "function") {
            Qt.callLater(function () { root.refreshCapabilityRegistryIfLoaded() })
        } else if (typeof root.refreshCapabilityRegistryIfLoaded === "function") {
            root.refreshCapabilityRegistryIfLoaded()
        }
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
	        if (target === "messaging") {
	            messagingSourceReport = value || null
	            root.refreshCapabilityRegistryIfLoaded()
	            return
	        }
	        if (target === "storage") {
	            storageSourceReport = value || null
	            root.refreshCapabilityRegistryIfLoaded()
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
