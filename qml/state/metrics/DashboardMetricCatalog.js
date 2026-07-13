.pragma library

function dashboardGraphKeys() {
    return [
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
        "messaging.network_ingress_recent",
        "messaging.network_egress_recent",
        "messaging.relay_ingress_recent",
        "messaging.relay_egress_recent",
        "messaging.service_ingress_recent",
        "messaging.service_egress_recent",
        "messaging.store_query_requests_recent",
        "messaging.filter_requests_recent",
        "messaging.lightpush_requests_recent",
        "messaging.peer_exchange_requests_recent",
        "messaging.store_messages",
        "messaging.store_errors_recent",
        "messaging.publish_latency_ms",
        "messaging.receive_latency_ms"
    ]
}

function selectedDashboardGraphItems(model) {
    const revision = model.dashboardGraphRevision
    const keys = dashboardGraphKeys()
    const rows = []
    for (let i = 0; i < keys.length; ++i) {
        if (model.dashboardGraphEnabled(keys[i])) {
            rows.push(dashboardGraphItem(model, keys[i]))
        }
    }
    return rows
}

function dashboardGraphItem(model, key) {
    const raw = model.metrics.dashboardMetricValue(key)
    const numeric = Number(raw)
    const gate = model.dashboardGate ? model.dashboardGate(key) : null
    const blocked = gate && gate.enabled === false
    return {
        key: key,
        title: dashboardMetricLabel(key),
        group: dashboardMetricGroup(key),
        value: blocked ? String(gate.status || qsTr("unavailable")) : dashboardMetricText(model, raw),
        numericValue: numeric,
        tone: blocked ? "warning" : dashboardMetricTone(key, numeric),
        samples: blocked ? [] : model.dashboardMetricSamples(key),
        gate: gate || {
            enabled: true,
            status: "enabled",
            missing: [],
            warnings: [],
            provenance: ["status_projection"]
        },
        provenance: ["status_projection", "dashboard_metric"]
    }
}

function dashboardMetricTone(key, numeric) {
    if (!Number.isFinite(numeric)) {
        return "neutral"
    }
    if (key === "bedrock.peer_count" || key === "storage.peer_count" || key === "messaging.peer_count" || key === "lez.blocks_produced_recent") {
        return numeric > 0 ? "success" : "neutral"
    }
    if (key === "storage.failed_transfers_total") {
        return numeric > 0 ? "neutral" : "success"
    }
    if (key.indexOf("rejected_") >= 0 || key.indexOf("failed_") >= 0 || key.indexOf("_error_") >= 0) {
        return numeric > 0 ? "error" : "neutral"
    }
    if (key.indexOf("_lag") >= 0 || key.indexOf("_queue") >= 0 || key.indexOf("pending_") >= 0 || key.indexOf("mempool_") >= 0 || key === "bedrock.tip_minus_lib") {
        return numeric > 0 ? "warning" : "neutral"
    }
    return "neutral"
}

function dashboardMetricGroup(key) {
    if (key.indexOf("bedrock.") === 0) {
        return qsTr("Bedrock Blockchain")
    }
    if (key.indexOf("lez.") === 0) {
        return qsTr("LEZ Sequencer")
    }
    if (key.indexOf("indexer.") === 0) {
        return qsTr("Indexer")
    }
    if (key.indexOf("storage.") === 0) {
        return qsTr("Storage")
    }
    return qsTr("Messaging / Delivery")
}

function dashboardMetricLabel(key) {
    switch (String(key || "")) {
    case "messaging.message_received_events_recent":
        return qsTr("messages in window")
    case "messaging.message_error_events_recent":
        return qsTr("errors in window")
    case "storage.active_uploads":
        return qsTr("upload requests total")
    case "storage.active_downloads":
        return qsTr("download requests total")
    case "storage.failed_transfers_recent":
        return qsTr("transfer failures in window")
    case "storage.failed_transfers_total":
        return qsTr("transfer failures total")
    }
    const parts = String(key || "").split(".")
    return parts.length > 1 ? parts[1].replace(/_/g, " ") : key
}

function dashboardMetricText(model, value) {
    if (value === undefined || value === null || value === "") {
        return qsTr("n/a")
    }
    return model.valueText(value)
}

function dashboardMetricRawValue(root, key) {
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
        return Array.isArray(root.dashboardBlocks) ? root.dashboardBlocks.length : null
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
        return dashboardMetricRawValue(root, "storage.failed_transfers_total")
    case "storage.failed_transfers_total":
        return root.moduleMetricSum("storage", ["storage_block_exchange_requests_failed_total", "storage_block_exchange_peer_timeouts_total"])
    case "messaging.peer_count":
        return root.moduleMetricValue("messaging", [
            { name: "libp2p_peers", labels: { type: "connected" } },
            "libp2p_peers",
            "waku_peers",
            "messaging_peer_count",
            "peer_count"
        ])
    case "messaging.active_subscriptions":
        return root.moduleMetricValue("messaging", ["active_subscriptions"])
    case "messaging.pubsub_peers":
        return root.moduleMetricValue("messaging", ["libp2p_pubsub_peers", "waku_relay_peers", "relay_peers"])
    case "messaging.store_peers":
        return root.moduleMetricValue("messaging", [
            "waku_store_peers",
            { name: "waku_service_peers", labels: { service: "/vac/waku/store-query/3.0.0" } },
            "store_peers"
        ])
    case "messaging.filter_peers":
        return root.moduleMetricValue("messaging", [
            "waku_filter_peers",
            { name: "waku_service_peers", labels: { service: "/vac/waku/filter-subscribe/2.0.0-beta1" } },
            "filter_peers"
        ])
    case "messaging.lightpush_peers":
        return root.moduleMetricValue("messaging", [
            "waku_lightpush_peers",
            { name: "waku_service_peers", labels: { service: "/vac/waku/lightpush/2.0.0-beta1" } },
            { name: "waku_service_peers", labels: { service: "/vac/waku/lightpush/3.0.0" } },
            "lightpush_peers"
        ])
    case "messaging.content_topics":
        return root.moduleMetricValue("messaging", ["content_topics"])
    case "messaging.outbound_queue":
        return root.moduleMetricValue("messaging", ["outbound_queue"])
    case "messaging.message_sent_events_recent":
        return root.moduleMetricSum("messaging", [
            "waku_lightpush_v3_messages",
            "waku_lightpush_messages",
            { name: "waku_service_requests_total", labels: { service: "/vac/waku/lightpush/2.0.0-beta1" } },
            { name: "waku_service_requests_total", labels: { service: "/vac/waku/lightpush/3.0.0" } }
        ])
    case "messaging.message_propagated_events_recent":
        return root.moduleMetricValue("messaging", ["waku_node_messages_total", "waku_node_messages"])
    case "messaging.message_received_events_recent":
        return root.moduleMetricValue("messaging", ["waku_node_messages_total", "waku_node_messages", "message_received_events_recent"])
    case "messaging.message_error_events_recent":
        return root.moduleMetricSum("messaging", [
            "waku_node_errors_total",
            "waku_node_errors",
            "waku_store_errors_total",
            "waku_filter_errors_total",
            "waku_lightpush_errors_total",
            "waku_lightpush_v3_errors_total",
            "message_error_events_recent"
        ])
    case "messaging.network_ingress_recent":
        return root.moduleMetricValue("messaging", [
            { name: "libp2p_network_bytes_total", labels: { direction: "in" } },
            "libp2p_network_bytes_in_total"
        ])
    case "messaging.network_egress_recent":
        return root.moduleMetricValue("messaging", [
            { name: "libp2p_network_bytes_total", labels: { direction: "out" } },
            "libp2p_network_bytes_out_total"
        ])
    case "messaging.relay_ingress_recent":
        return root.moduleMetricValue("messaging", [
            { name: "waku_relay_network_bytes_total", labels: { direction: "in" } },
            "waku_relay_network_bytes_in_total"
        ])
    case "messaging.relay_egress_recent":
        return root.moduleMetricValue("messaging", [
            { name: "waku_relay_network_bytes_total", labels: { direction: "out" } },
            "waku_relay_network_bytes_out_total"
        ])
    case "messaging.service_ingress_recent":
        return root.moduleMetricValue("messaging", [
            { name: "waku_service_network_bytes_total", labels: { direction: "in" } },
            "waku_service_network_bytes_in_total"
        ])
    case "messaging.service_egress_recent":
        return root.moduleMetricValue("messaging", [
            { name: "waku_service_network_bytes_total", labels: { direction: "out" } },
            "waku_service_network_bytes_out_total"
        ])
    case "messaging.store_query_requests_recent":
        return root.moduleMetricSum("messaging", [
            "waku_store_queries_total",
            { name: "waku_service_requests_total", labels: { service: "/vac/waku/store-query/3.0.0" } }
        ])
    case "messaging.filter_requests_recent":
        return root.moduleMetricSum("messaging", [
            "waku_filter_requests_total",
            { name: "waku_service_requests_total", labels: { service: "/vac/waku/filter-subscribe/2.0.0-beta1" } }
        ])
    case "messaging.lightpush_requests_recent":
        return root.moduleMetricSum("messaging", [
            "waku_lightpush_v3_messages",
            { name: "waku_service_requests_total", labels: { service: "/vac/waku/lightpush/2.0.0-beta1" } },
            { name: "waku_service_requests_total", labels: { service: "/vac/waku/lightpush/3.0.0" } }
        ])
    case "messaging.peer_exchange_requests_recent":
        return root.moduleMetricSum("messaging", [
            "waku_px_peers_sent_total",
            { name: "waku_service_requests_total", labels: { service: "/vac/waku/peer-exchange/2.0.0-alpha1" } }
        ])
    case "messaging.store_messages":
        return root.moduleMetricValue("messaging", ["waku_store_messages", "waku_archive_messages"])
    case "messaging.store_errors_recent":
        return root.moduleMetricSum("messaging", ["waku_store_errors_total", "waku_archive_errors_total"])
    case "messaging.publish_latency_ms":
    case "messaging.receive_latency_ms":
        return null
    default:
        return null
    }
}

function dashboardMetricValue(root, key) {
    switch (String(key || "")) {
    case "messaging.message_received_events_recent":
    case "messaging.message_error_events_recent":
    case "storage.failed_transfers_recent":
    case "messaging.network_ingress_recent":
    case "messaging.network_egress_recent":
    case "messaging.relay_ingress_recent":
    case "messaging.relay_egress_recent":
    case "messaging.service_ingress_recent":
    case "messaging.service_egress_recent":
    case "messaging.store_query_requests_recent":
    case "messaging.filter_requests_recent":
    case "messaging.lightpush_requests_recent":
    case "messaging.peer_exchange_requests_recent":
    case "messaging.store_errors_recent":
        return dashboardMetricWindowDelta(root, key)
    default:
        return dashboardMetricRawValue(root, key)
    }
}

function dashboardMetricUsesWindow(key) {
    switch (String(key || "")) {
    case "messaging.message_received_events_recent":
    case "messaging.message_error_events_recent":
    case "messaging.network_ingress_recent":
    case "messaging.network_egress_recent":
    case "messaging.relay_ingress_recent":
    case "messaging.relay_egress_recent":
    case "messaging.service_ingress_recent":
    case "messaging.service_egress_recent":
    case "messaging.store_query_requests_recent":
    case "messaging.filter_requests_recent":
    case "messaging.lightpush_requests_recent":
    case "messaging.peer_exchange_requests_recent":
    case "messaging.store_errors_recent":
    case "storage.failed_transfers_recent":
        return true
    default:
        return false
    }
}

function dashboardMetricWindowDelta(root, key) {
    const current = Number(dashboardMetricRawValue(root, key))
    if (!Number.isFinite(current)) {
        return null
    }
    const timestamp = Date.now()
    const history = root.dashboardMetricHistory || {}
    const samples = normalizedDashboardSamples(history[String(key || "")]).slice()
    if (samples.length === 0 || Number(samples[samples.length - 1].value) !== current) {
        samples.push({ timestamp: timestamp, value: current })
    }
    return windowDeltaFromSamples(samples, timestamp, dashboardMetricWindowMs(root, key))
}

function dashboardMetricWindowMs(root, key) {
    if (String(key || "").indexOf("storage.") === 0) {
        return Math.max(1, Number(root.storageRollingWindow || 0)) * 1000
    }
    return Math.max(1, Number(root.messagingRollingWindow || 0)) * 1000
}

function dashboardMetricTextForKey(root, key) {
    return root.valueText(dashboardMetricValue(root, key))
}

function recordDashboardSnapshot(root) {
    const keys = dashboardGraphKeys()
    const next = root.copyMap(root.dashboardMetricHistory)
    const nextSeen = root.copyMap(root.dashboardMetricLastSeen)
    const now = Date.now()
    let historyChanged = false
    let seenChanged = false
    for (let i = 0; i < keys.length; ++i) {
        const key = keys[i]
        const value = Number(dashboardMetricRawValue(root, key))
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
        root.dashboardMetricLastSeen = nextSeen
    }
    if (historyChanged) {
        root.dashboardMetricHistory = next
        root.dashboardMetricHistoryRevision += 1
    }
}

function dashboardMetricSampleUpdate(root, stored, lastSeen, now, value) {
    const samples = normalizedDashboardSamples(stored)
    const previous = normalizedDashboardSample(lastSeen) || (samples.length > 0 ? samples[samples.length - 1] : null)
    const timestamp = nextDashboardSampleTimestamp(previous, now)
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
        samples: trimDashboardMetricSamples(samples),
        lastSeen: current,
        changed: changed
    }
}

function dashboardMetricSamples(root, key) {
    const revision = root.dashboardMetricHistoryRevision
    if (dashboardMetricUsesWindow(key)) {
        return dashboardMetricWindowSamples(root, key)
    }
    const history = root.dashboardMetricHistory || {}
    const samples = normalizedDashboardSamples(history[String(key || "")])
    if (Array.isArray(samples) && samples.length > 0) {
        return samples
    }
    const value = Number(dashboardMetricValue(root, key))
    return Number.isFinite(value) ? [{ timestamp: Date.now(), value: value }] : []
}

function normalizedDashboardSample(sample) {
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

function normalizedDashboardSamples(samples) {
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

function nextDashboardSampleTimestamp(previous, now) {
    const timestamp = Number(now)
    const candidate = Number.isFinite(timestamp) ? timestamp : Date.now()
    const last = previous ? Number(previous.timestamp) : NaN
    return Number.isFinite(last) && candidate <= last ? last + 1 : candidate
}

function trimDashboardMetricSamples(samples) {
    const rows = normalizedDashboardSamples(samples)
    return rows.length > 300 ? rows.slice(rows.length - 300) : rows
}

function dashboardMetricWindowSamples(root, key) {
    const history = root.dashboardMetricHistory || {}
    const samples = normalizedDashboardSamples(history[String(key || "")])
    const windowMs = dashboardMetricWindowMs(root, key)
    const rows = []
    for (let i = 0; i < samples.length; ++i) {
        const delta = windowDeltaFromSamples(samples.slice(0, i + 1), samples[i].timestamp, windowMs)
        if (delta !== null) {
            rows.push({
                timestamp: samples[i].timestamp,
                value: delta
            })
        }
    }
    return rows
}

function windowDeltaFromSamples(samples, timestamp, windowMs) {
    const rows = normalizedDashboardSamples(samples)
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

function clearDashboardMetricHistoryForPrefix(root, prefix) {
    const text = String(prefix || "")
    if (!text.length) {
        return
    }
    const next = root.copyMap(root.dashboardMetricHistory)
    const seen = root.copyMap(root.dashboardMetricLastSeen)
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
        root.dashboardMetricHistory = next
        root.dashboardMetricLastSeen = seen
        root.dashboardMetricHistoryRevision += 1
    }
}
