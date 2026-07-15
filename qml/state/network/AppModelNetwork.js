.import "../source_routing/SourceHealthProjection.js" as SourceHealthProjection

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

function blockchainRpcArgs(root, extra) {
    with (root) {
        return [String(nodeUrl || "")].concat(Array.isArray(extra) ? extra : [])
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
        const next = root.copyMap(footerFieldSelections)
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
        const next = root.copyMap(dashboardGraphSelections)
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
