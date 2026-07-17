.import "../../services/BridgeHelpers.js" as BridgeHelpers

.import "DashboardMetricCatalog.js" as DashboardMetricCatalog
.import "../status/StatusFieldCatalog.js" as StatusFieldCatalog

function valueText(root, value) {
    with (root) {
        const scalar = root.scalarValue(value)
        if (scalar === null) {
            return "-"
        }
        if (typeof scalar === "number") {
            return scalar.toLocaleString(Qt.locale(), "f", Number.isInteger(scalar) ? 0 : 2)
        }
        return String(scalar)
    }
}

function valueToString(root, value) {
    with (root) {
        if (value === undefined || value === null) {
            return ""
        }
        return String(value)
    }
}

function moduleReport(root, kind) {
    with (root) {
        if (kind === "blockchain") {
            return blockchainModuleReport || null
        }
        if (kind === "storage") {
            return storageModuleReport || null
        }
        if (kind === "messaging") {
            return messagingModuleReport || null
        }
        return null
    }
}

function moduleProbe(root, kind, method) {
    with (root) {
        const report = root.moduleReport(kind)
        const probes = report && Array.isArray(report.probes) ? report.probes : []
        const wanted = String(method || "")
        if (report && report.module_info && String(report.module_info.probe_key || "") === wanted) {
            return report.module_info
        }
        for (let i = 0; i < probes.length; ++i) {
            const probe = probes[i] || {}
            if (String(probe.probe_key || "") === wanted) {
                return probe
            }
            const label = String(probe.label || "")
            const source = String(probe.source || "")
            if (label.indexOf("." + wanted) >= 0 || source.indexOf(" " + wanted) >= 0) {
                return probe
            }
        }
        return null
    }
}

function moduleProbeValue(root, kind, method) {
    with (root) {
        const probe = root.moduleProbe(kind, method)
        if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
            return null
        }
        return probe.value
    }
}

function sourceProbe(root, kind, method) {
    with (root) {
        return root.reportProbe(root.sourceReport(kind), method)
    }
}

function sourceProbeValue(root, kind, method) {
    with (root) {
        const probe = root.sourceProbe(kind, method)
        if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
            return null
        }
        return probe.value
    }
}

function observationProbeValue(root, kind, method) {
    with (root) {
        return String(kind || "") === "blockchain"
            ? root.moduleProbeValue(kind, method)
            : root.sourceProbeValue(kind, method)
    }
}

function moduleProbeError(root, kind, method) {
    with (root) {
        const probe = root.moduleProbe(kind, method)
        return probe && probe.error ? String(probe.error) : ""
    }
}

function moduleLastError(root, kind) {
    with (root) {
        const report = root.moduleReport(kind)
        if (!report) {
            return ""
        }
        if (report.module_info && report.module_info.ok === false && report.module_info.error) {
            return String(report.module_info.error)
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            const probe = probes[i] || {}
            if (probe.ok === false && probe.error) {
                return String(probe.error)
            }
        }
        return ""
    }
}

function openMetricsText(root, kind) {
    with (root) {
        const value = root.observationProbeValue(kind,
            kind === "storage" ? "collectMetrics" : "collectOpenMetricsText")
        return root.openMetricsTextFromValue(value)
    }
}

function openMetricsTextFromValue(root, value) {
    with (root) {
        if (typeof value === "string") {
            return value
        }
        const scalar = root.scalarValue(value)
        return scalar === null ? "" : String(scalar)
    }
}

function openMetricValue(root, kind, names) {
    with (root) {
        const wanted = Array.isArray(names) ? names : [names]
        const value = root.observationProbeValue(kind,
            kind === "storage" ? "collectMetrics" : "collectOpenMetricsText")
        const jsonMetric = root.metricJsonValue(value, wanted)
        if (jsonMetric !== null) {
            return jsonMetric
        }
        const text = root.openMetricsTextFromValue(value)
        if (!text.length) {
            return null
        }
        const lines = text.split(/\r?\n/)
        for (let i = 0; i < lines.length; ++i) {
            const line = lines[i].trim()
            if (!line.length || line[0] === "#") {
                continue
            }
            const match = line.match(/^([^{\s]+)(?:\{([^}]*)\})?\s+(-?(?:[0-9]+(?:\.[0-9]*)?|\.[0-9]+)(?:e[+-]?[0-9]+)?)/i)
            if (!match) {
                continue
            }
            const name = match[1]
            const labels = root.openMetricLabels(match[2] || "")
            for (let j = 0; j < wanted.length; ++j) {
                if (name === root.metricSpecName(wanted[j]) && root.metricLabelsMatch(labels, root.metricSpecLabels(wanted[j]))) {
                    const number = Number(match[3])
                    return Number.isFinite(number) ? number : null
                }
            }
        }
        return null
    }
}

function openMetricLabels(root, text) {
    with (root) {
        const labels = {}
        const pattern = /([A-Za-z_:][A-Za-z0-9_:]*)\s*=\s*"((?:\\.|[^"\\])*)"/g
        let match = pattern.exec(String(text || ""))
        while (match !== null) {
            labels[match[1]] = match[2].replace(/\\"/g, "\"").replace(/\\\\/g, "\\")
            match = pattern.exec(String(text || ""))
        }
        return labels
    }
}

function metricJsonValue(root, value, names) {
    with (root) {
        if (value === undefined || value === null) {
            return null
        }
        const wanted = Array.isArray(names) ? names : [names]
        if (Array.isArray(value)) {
            for (let i = 0; i < value.length; ++i) {
                const match = root.metricJsonValue(value[i], wanted)
                if (match !== null) {
                    return match
                }
            }
            return null
        }
        if (typeof value !== "object") {
            return null
        }
        if (Array.isArray(value.metrics)) {
            return root.metricJsonValue(value.metrics, wanted)
        }
        const metricName = String(value.name || value.metric || value.key || "")
        for (let i = 0; i < wanted.length; ++i) {
            const wantedName = root.metricSpecName(wanted[i])
            const wantedLabels = root.metricSpecLabels(wanted[i])
            if (metricName === wantedName && root.metricLabelsMatch(root.metricJsonLabels(value), wantedLabels)) {
                return root.metricNumber(value.value !== undefined ? value.value : (value.count !== undefined ? value.count : value.total))
            }
            if (Object.keys(wantedLabels).length === 0 && value[wantedName] !== undefined) {
                return root.metricNumber(value[wantedName])
            }
        }
        return null
    }
}

function metricSpecName(root, spec) {
    with (root) {
        return spec && typeof spec === "object" ? String(spec.name || spec.metric || spec.key || "") : String(spec || "")
    }
}

function metricSpecLabels(root, spec) {
    with (root) {
        return spec && typeof spec === "object" && spec.labels && typeof spec.labels === "object" ? spec.labels : {}
    }
}

function metricJsonLabels(root, value) {
    with (root) {
        if (!value || typeof value !== "object") {
            return {}
        }
        if (value.labels && typeof value.labels === "object") {
            return value.labels
        }
        if (value.label && typeof value.label === "object") {
            return value.label
        }
        return value
    }
}

function metricLabelsMatch(root, actual, wanted) {
    with (root) {
        const keys = Object.keys(wanted || {})
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            if (String(actual && actual[key] !== undefined ? actual[key] : "") !== String(wanted[key])) {
                return false
            }
        }
        return true
    }
}

function metricNumber(root, value) {
    with (root) {
        const scalar = root.scalarValue(value)
        const number = Number(scalar)
        return Number.isFinite(number) ? number : null
    }
}

function overviewProbeValue(root, section, field) {
    with (root) {
        const sectionValue = dashboardOverview ? dashboardOverview[section] : null
        const probe = sectionValue ? sectionValue[field] : null
        return probe && probe.value !== undefined && probe.value !== null ? root.scalarValue(probe.value) : null
    }
}

function indexerHeadValue(root) {
    with (root) {
        const overviewValue = root.overviewProbeValue("indexer", "head")
        if (overviewValue !== null) {
            return overviewValue
        }
        const blocks = dashboardBlocks || []
        if (blocks.length > 0) {
            return root.scalarValue((blocks[0] || {}).block_id)
        }
        return null
    }
}

function sequencerHeadValue(root) {
    with (root) {
        const overviewValue = root.overviewProbeValue("sequencer", "head")
        if (overviewValue !== null) {
            return overviewValue
        }
        return null
    }
}

function nodeProbeValue(root, name) {
    with (root) {
        const report = dashboardNode || {}
        const probe = report[name]
        return probe && probe.value !== undefined && probe.value !== null ? probe.value : null
    }
}

function cryptarchiaInfo(root) {
    with (root) {
        const fromOverview = dashboardOverview && dashboardOverview.node && dashboardOverview.node.consensus
            ? dashboardOverview.node.consensus.value
            : null
        if (fromOverview && typeof fromOverview === "object") {
            return fromOverview.cryptarchia_info || fromOverview
        }
        const fromNode = root.nodeProbeValue("cryptarchia_info")
        if (fromNode && typeof fromNode === "object") {
            return fromNode.cryptarchia_info || fromNode
        }
        return {}
    }
}

function cryptarchiaValue(root, key) {
    with (root) {
        const value = root.cryptarchiaInfo()[key]
        return value === undefined || value === null ? null : root.scalarValue(value)
    }
}

function networkInfo(root) {
    with (root) {
        const value = root.nodeProbeValue("network_info")
        return value && typeof value === "object" ? value : {}
    }
}

function networkValue(root, key) {
    with (root) {
        const value = root.networkInfo()[key]
        return value === undefined || value === null ? null : root.scalarValue(value)
    }
}

function mantleMetrics(root) {
    with (root) {
        const value = root.nodeProbeValue("mantle_metrics")
        return value && typeof value === "object" ? value : {}
    }
}

function mantleValue(root, keys) {
    with (root) {
        const list = Array.isArray(keys) ? keys : [keys]
        const metrics = root.mantleMetrics()
        for (let i = 0; i < list.length; ++i) {
            const value = metrics[list[i]]
            if (value !== undefined && value !== null) {
                return root.scalarValue(value)
            }
        }
        return null
    }
}

function tipMinusLib(root) {
    with (root) {
        const tipValue = root.cryptarchiaValue("slot")
        const libValue = root.cryptarchiaValue("lib_slot")
        if (tipValue === null || libValue === null) {
            return null
        }
        const tip = Number(tipValue)
        const lib = Number(libValue)
        return Number.isFinite(tip) && Number.isFinite(lib) ? Math.max(0, tip - lib) : null
    }
}

function finalityLagSeconds(root) {
    with (root) {
        const gap = root.tipMinusLib()
        return gap === null ? null : gap * 2
    }
}

function indexerLag(root) {
    with (root) {
        const sequencerValue = root.sequencerHeadValue()
        const indexerValue = root.indexerHeadValue()
        if (sequencerValue === null || indexerValue === null) {
            return null
        }
        const sequencerHead = Number(sequencerValue)
        const indexerHead = Number(indexerValue)
        return Number.isFinite(sequencerHead) && Number.isFinite(indexerHead) ? Math.max(0, sequencerHead - indexerHead) : null
    }
}

function moduleMetricValue(root, kind, names) {
    with (root) {
        const metric = root.openMetricValue(kind, names)
        if (metric !== null) {
            return metric
        }
        return null
    }
}

function moduleMetricSum(root, kind, names) {
    with (root) {
        const wanted = Array.isArray(names) ? names : [names]
        let total = 0
        let found = false
        for (let i = 0; i < wanted.length; ++i) {
            const value = root.moduleMetricValue(kind, wanted[i])
            if (value !== null) {
                total += Number(value)
                found = true
            }
        }
        return found ? total : null
    }
}

function storageManifestCount(root) {
    with (root) {
        const manifests = root.observationProbeValue("storage", "manifests")
        if (Array.isArray(manifests)) {
            return manifests.length
        }
        if (manifests && typeof manifests === "object" && Array.isArray(manifests.content)) {
            return manifests.content.length
        }
        const scalar = root.scalarValue(manifests)
        if (typeof scalar === "number") {
            return scalar
        }
        return root.moduleMetricValue("storage", ["storage_manifest_count", "manifest_count"])
    }
}

function dashboardMetricRawValue(root, key) {
    return DashboardMetricCatalog.dashboardMetricRawValue(root, key)
}

function dashboardMetricValue(root, key) {
    return DashboardMetricCatalog.dashboardMetricValue(root, key)
}

function dashboardMetricUsesWindow(root, key) {
    return DashboardMetricCatalog.dashboardMetricUsesWindow(key)
}

function dashboardMetricWindowDelta(root, key) {
    return DashboardMetricCatalog.dashboardMetricWindowDelta(root, key)
}

function dashboardMetricWindowMs(root, key) {
    return DashboardMetricCatalog.dashboardMetricWindowMs(root, key)
}

function dashboardMetricText(root, key) {
    return DashboardMetricCatalog.dashboardMetricTextForKey(root, key)
}

function recordDashboardSnapshot(root) {
    return DashboardMetricCatalog.recordDashboardSnapshot(root)
}

function dashboardMetricSampleUpdate(root, stored, lastSeen, now, value) {
    return DashboardMetricCatalog.dashboardMetricSampleUpdate(root, stored, lastSeen, now, value)
}

function dashboardMetricSamples(root, key) {
    return DashboardMetricCatalog.dashboardMetricSamples(root, key)
}

function normalizedDashboardSample(root, sample) {
    return DashboardMetricCatalog.normalizedDashboardSample(sample)
}

function normalizedDashboardSamples(root, samples) {
    return DashboardMetricCatalog.normalizedDashboardSamples(samples)
}

function nextDashboardSampleTimestamp(root, previous, now) {
    return DashboardMetricCatalog.nextDashboardSampleTimestamp(previous, now)
}

function trimDashboardMetricSamples(root, samples) {
    return DashboardMetricCatalog.trimDashboardMetricSamples(samples)
}

function dashboardMetricWindowSamples(root, key) {
    return DashboardMetricCatalog.dashboardMetricWindowSamples(root, key)
}

function windowDeltaFromSamples(root, samples, timestamp, windowMs) {
    return DashboardMetricCatalog.windowDeltaFromSamples(samples, timestamp, windowMs)
}

function defaultFooterFieldSelections(root) {
    with (root) {
        return StatusFieldCatalog.defaultFooterFieldSelections()
    }
}

function defaultDashboardGraphSelections(root) {
    with (root) {
        return StatusFieldCatalog.defaultDashboardGraphSelections()
    }
}

function clearDashboardMetricHistoryForPrefix(root, prefix) {
    return DashboardMetricCatalog.clearDashboardMetricHistoryForPrefix(root, prefix)
}

function clearDashboardMetricHistoryForPrefixes(root, prefixes) {
    return DashboardMetricCatalog.clearDashboardMetricHistoryForPrefixes(root, prefixes)
}
