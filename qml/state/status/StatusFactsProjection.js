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
    const raw = model.dashboardMetricValue(key)
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
