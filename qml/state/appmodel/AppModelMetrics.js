.import "../../services/BridgeHelpers.js" as BridgeHelpers

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

function moduleProbeValue(root, kind, method) {
    with (root) {
        const probe = root.moduleProbe(kind, method)
        if (!probe || probe.ok !== true || probe.value === undefined || probe.value === null) {
            return null
        }
        return probe.value
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
        const value = root.moduleProbeValue(kind, kind === "storage" ? "collectMetrics" : "collectOpenMetricsText")
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
        const value = root.moduleProbeValue(kind, kind === "storage" ? "collectMetrics" : "collectOpenMetricsText")
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
        const status = networkConnectionStatus.indexer
        const statusValue = status ? root.scalarValue(status.value) : null
        if (statusValue !== null) {
            return statusValue
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
        const status = networkConnectionStatus.execution
        return status ? root.scalarValue(status.value) : null
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
        const tip = Number(root.cryptarchiaValue("slot"))
        const lib = Number(root.cryptarchiaValue("lib_slot"))
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
        const manifests = root.moduleProbeValue("storage", "manifests")
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
    with (root) {
        switch (key) {
        case "bedrock.peer_count":
            return root.networkValue("n_peers")
        case "bedrock.tip_minus_lib":
            return root.tipMinusLib()
        case "bedrock.finality_lag_seconds":
            return root.finalityLagSeconds()
        case "lez.pending_tx_count":
            return root.mantleValue(["pending_tx_count", "pending_txs", "pending_transactions"])
        case "lez.mempool_tx_count":
            return root.mantleValue(["mempool_tx_count", "mempool_txs", "mempool_size"])
        case "lez.rejected_tx_count_recent":
            return root.mantleValue(["rejected_tx_count_recent", "rejected_txs_recent"])
        case "lez.blocks_produced_recent":
            return Array.isArray(dashboardBlocks) ? dashboardBlocks.length : null
        case "lez.pending_blocks_count":
            return root.mantleValue(["pending_blocks_count", "pending_blocks"])
        case "indexer.indexer_lag_vs_sequencer_head":
            return root.indexerLag()
        case "storage.peer_count":
            return root.moduleMetricValue("storage", [
                { name: "libp2p_peers", labels: { type: "connected" } },
                "storage_peer_count",
                "storage_libp2p_peers",
                "peers"
            ])
        case "storage.shared_files_count":
            return root.moduleMetricValue("storage", ["storage_shared_files_count", "shared_files_count"])
        case "storage.manifest_count":
            return root.storageManifestCount()
        case "storage.local_storage_used":
            return root.moduleMetricValue("storage", ["storage_local_storage_used_bytes", "local_storage_used_bytes", "storage_used_bytes", "storage_repostore_bytes_used"])
        case "storage.active_uploads":
            return root.moduleMetricValue("storage", ["storage_active_uploads", "active_uploads", "storage_api_uploads"])
        case "storage.active_downloads":
            return root.moduleMetricValue("storage", ["storage_active_downloads", "active_downloads", "storage_api_downloads"])
        case "storage.failed_transfers_recent":
            return root.dashboardMetricRawValue("storage.failed_transfers_total")
        case "storage.failed_transfers_total":
            return root.moduleMetricSum("storage", ["storage_block_exchange_requests_failed_total", "storage_block_exchange_peer_timeouts_total"])
        case "messaging.peer_count":
            return root.moduleMetricValue("messaging", ["libp2p_peers", "waku_peers", "messaging_peer_count", "peer_count"])
        case "messaging.active_subscriptions":
            return root.moduleMetricValue("messaging", ["active_subscriptions"])
        case "messaging.pubsub_peers":
            return root.moduleMetricValue("messaging", ["libp2p_pubsub_peers"])
        case "messaging.store_peers":
            return root.moduleMetricValue("messaging", ["waku_store_peers"])
        case "messaging.filter_peers":
            return root.moduleMetricValue("messaging", ["waku_filter_peers"])
        case "messaging.lightpush_peers":
            return root.moduleMetricValue("messaging", ["waku_lightpush_peers"])
        case "messaging.content_topics":
            return root.moduleMetricValue("messaging", ["content_topics"])
        case "messaging.outbound_queue":
            return root.moduleMetricValue("messaging", ["outbound_queue"])
        case "messaging.message_sent_events_recent":
            return null
        case "messaging.message_propagated_events_recent":
            return null
        case "messaging.message_received_events_recent":
            return root.moduleMetricValue("messaging", ["waku_node_messages_total", "waku_node_messages", "message_received_events_recent"])
        case "messaging.message_error_events_recent":
            return root.moduleMetricValue("messaging", ["waku_node_errors_total", "waku_node_errors", "message_error_events_recent"])
        case "messaging.publish_latency_ms":
            return null
        case "messaging.receive_latency_ms":
            return null
        default:
            return null
        }
    }
}

function dashboardMetricValue(root, key) {
    with (root) {
        switch (key) {
        case "messaging.message_received_events_recent":
        case "messaging.message_error_events_recent":
        case "storage.failed_transfers_recent":
            return root.dashboardMetricWindowDelta(key)
        default:
            return root.dashboardMetricRawValue(key)
        }
    }
}

function dashboardMetricUsesWindow(root, key) {
    with (root) {
        return key === "messaging.message_received_events_recent"
            || key === "messaging.message_error_events_recent"
            || key === "storage.failed_transfers_recent"
    }
}

function dashboardMetricWindowDelta(root, key) {
    with (root) {
        const current = Number(root.dashboardMetricRawValue(key))
        if (!Number.isFinite(current)) {
            return null
        }
        const timestamp = Date.now()
        const samples = root.normalizedDashboardSamples(dashboardMetricHistory[String(key || "")]).slice()
        if (samples.length === 0 || Number(samples[samples.length - 1].value) !== current) {
            samples.push({ timestamp: timestamp, value: current })
        }
        return root.windowDeltaFromSamples(samples, timestamp, root.dashboardMetricWindowMs(key))
    }
}

function dashboardMetricWindowMs(root, key) {
    with (root) {
        if (String(key || "").indexOf("storage.") === 0) {
            return Math.max(1, Number(storageRollingWindow || 0)) * 1000
        }
        return Math.max(1, Number(messagingRollingWindow || 0)) * 1000
    }
}

function dashboardMetricText(root, key) {
    with (root) {
        return root.valueText(root.dashboardMetricValue(key))
    }
}

function recordDashboardSnapshot(root) {
    with (root) {
        const keys = [
            "bedrock.peer_count",
            "bedrock.tip_minus_lib",
            "bedrock.finality_lag_seconds",
            "lez.pending_tx_count",
            "lez.mempool_tx_count",
            "lez.rejected_tx_count_recent",
            "lez.blocks_produced_recent",
            "lez.pending_blocks_count",
            "indexer.indexer_lag_vs_sequencer_head",
            "storage.peer_count",
            "storage.shared_files_count",
            "storage.manifest_count",
            "storage.local_storage_used",
            "storage.active_uploads",
            "storage.active_downloads",
            "storage.failed_transfers_recent",
            "storage.failed_transfers_total",
            "messaging.peer_count",
            "messaging.active_subscriptions",
            "messaging.content_topics",
            "messaging.outbound_queue",
            "messaging.message_sent_events_recent",
            "messaging.message_propagated_events_recent",
            "messaging.message_received_events_recent",
            "messaging.message_error_events_recent",
            "messaging.publish_latency_ms",
            "messaging.receive_latency_ms"
        ]
        const next = copyMap(dashboardMetricHistory)
        const nextSeen = copyMap(dashboardMetricLastSeen)
        const now = Date.now()
        let historyChanged = false
        let seenChanged = false
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            const value = Number(root.dashboardMetricRawValue(key))
            if (!Number.isFinite(value)) {
                continue
            }
            const update = dashboardMetricSampleUpdate(root, next[key], nextSeen[key], now, value)
            nextSeen[key] = update.lastSeen
            seenChanged = true
            if (update.changed) {
                next[key] = update.samples
                historyChanged = true
            }
        }
        if (seenChanged) {
            dashboardMetricLastSeen = nextSeen
        }
        if (historyChanged) {
            dashboardMetricHistory = next
            dashboardMetricHistoryRevision += 1
        }
    }
}

function dashboardMetricSampleUpdate(root, stored, lastSeen, now, value) {
    with (root) {
        const samples = root.normalizedDashboardSamples(stored)
        const previous = normalizedDashboardSample(root, lastSeen) || (samples.length > 0 ? samples[samples.length - 1] : null)
        const timestamp = nextDashboardSampleTimestamp(root, previous, now)
        const current = { timestamp: timestamp, value: value }
        const lastStored = samples.length > 0 ? samples[samples.length - 1] : null
        let changed = false

        if (!lastStored) {
            samples.push(current)
            changed = true
        } else if (!previous || Number(previous.value) !== value) {
            if (previous && previous.timestamp > lastStored.timestamp && Number(previous.value) === Number(lastStored.value)) {
                samples.push(previous)
            }
            samples.push(current)
            changed = true
        }

        return {
            samples: trimDashboardMetricSamples(root, samples),
            lastSeen: current,
            changed: changed
        }
    }
}

function dashboardMetricSamples(root, key) {
    with (root) {
        const revision = dashboardMetricHistoryRevision
        if (root.dashboardMetricUsesWindow(key)) {
            return root.dashboardMetricWindowSamples(key)
        }
        const samples = root.normalizedDashboardSamples(dashboardMetricHistory[String(key || "")])
        if (Array.isArray(samples) && samples.length > 0) {
            return samples
        }
        const value = Number(root.dashboardMetricValue(key))
        return Number.isFinite(value) ? [{ timestamp: Date.now(), value: value }] : []
    }
}

function normalizedDashboardSample(root, sample) {
    with (root) {
        if (!sample || typeof sample !== "object") {
            return null
        }
        const value = Number(sample.value)
        const timestamp = Number(sample.timestamp)
        if (!Number.isFinite(value) || !Number.isFinite(timestamp)) {
            return null
        }
        return {
            timestamp: timestamp,
            value: value
        }
    }
}

function normalizedDashboardSamples(root, samples) {
    with (root) {
        const rows = []
        const raw = Array.isArray(samples) ? samples : []
        for (let i = 0; i < raw.length; ++i) {
            const sample = raw[i]
            const value = Number(sample && typeof sample === "object" ? sample.value : sample)
            if (!Number.isFinite(value)) {
                continue
            }
            const timestamp = Number(sample && typeof sample === "object" ? sample.timestamp : i)
            rows.push({
                timestamp: Number.isFinite(timestamp) ? timestamp : i,
                value: value
            })
        }
        return rows
    }
}

function nextDashboardSampleTimestamp(root, previous, now) {
    with (root) {
        const timestamp = Number(now)
        const candidate = Number.isFinite(timestamp) ? timestamp : Date.now()
        const last = previous ? Number(previous.timestamp) : NaN
        return Number.isFinite(last) && candidate <= last ? last + 1 : candidate
    }
}

function trimDashboardMetricSamples(root, samples) {
    with (root) {
        const rows = root.normalizedDashboardSamples(samples)
        return rows.length > 300 ? rows.slice(rows.length - 300) : rows
    }
}

function dashboardMetricWindowSamples(root, key) {
    with (root) {
        const samples = root.normalizedDashboardSamples(dashboardMetricHistory[String(key || "")])
        const windowMs = root.dashboardMetricWindowMs(key)
        const rows = []
        for (let i = 0; i < samples.length; ++i) {
            const delta = root.windowDeltaFromSamples(samples.slice(0, i + 1), samples[i].timestamp, windowMs)
            if (delta !== null) {
                rows.push({
                    timestamp: samples[i].timestamp,
                    value: delta
                })
            }
        }
        return rows
    }
}

function windowDeltaFromSamples(root, samples, timestamp, windowMs) {
    with (root) {
        const rows = root.normalizedDashboardSamples(samples)
        if (rows.length < 2) {
            return null
        }
        const cutoff = timestamp - windowMs
        let baseline = null
        for (let i = rows.length - 1; i >= 0; --i) {
            if (rows[i].timestamp <= cutoff) {
                baseline = rows[i]
                break
            }
            if (i === 0) {
                baseline = rows[i]
            }
        }
        const latest = rows[rows.length - 1]
        if (!baseline || latest.timestamp === baseline.timestamp) {
            return null
        }
        return Math.max(0, latest.value - baseline.value)
    }
}

function defaultFooterFieldSelections(root) {
    with (root) {
        return {
            "network.network": true,
            "bedrock.node_health": true,
            "bedrock.sync_state": true,
            "bedrock.tip_height": true,
            "bedrock.tip_minus_lib": true,
            "lez.rpc_health": true,
            "lez.last_lez_block_id": true,
            "indexer.rpc_health": true,
            "indexer.indexed_finalized_height": true,
            "messaging.connection_state": true,
            "messaging.peer_count": true,
            "messaging.message_error_events_recent": true,
            "storage.module": true,
            "storage.node_reachable": true,
            "storage.peer_count": true,
            "storage.failed_transfers_recent": true,
            "overall.status": true,
            "overall.main_risk": true,
            "overall.operator_action": true
        }
    }
}

function defaultDashboardGraphSelections(root) {
    with (root) {
        return {
            "bedrock.peer_count": true,
            "bedrock.tip_minus_lib": true,
            "bedrock.finality_lag_seconds": true,
            "lez.blocks_produced_recent": true,
            "indexer.indexer_lag_vs_sequencer_head": true
        }
    }
}

function clearDashboardMetricHistoryForPrefix(root, prefix) {
    with (root) {
        const text = String(prefix || "")
        if (!text.length) {
            return
        }
        const next = copyMap(dashboardMetricHistory)
        const seen = copyMap(dashboardMetricLastSeen)
        let changed = false
        for (const key in next) {
            if (String(key || "").indexOf(text) === 0) {
                delete next[key]
                changed = true
            }
        }
        for (const seenKey in seen) {
            if (String(seenKey || "").indexOf(text) === 0) {
                delete seen[seenKey]
                changed = true
            }
        }
        if (changed) {
            dashboardMetricHistory = next
            dashboardMetricLastSeen = seen
            dashboardMetricHistoryRevision += 1
        }
    }
}
