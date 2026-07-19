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
    const parsed = dashboardMetricNumber(raw)
    const numeric = parsed === null ? NaN : parsed
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
    // Keep the legacy key for persisted graph/footer selections and history.
    case "lez.blocks_produced_recent":
        return qsTr("provisional block records available")
    case "messaging.message_received_events_recent":
        return qsTr("messages in window")
    case "messaging.message_error_events_recent":
        return qsTr("errors in window")
    case "messaging.store_query_requests_recent":
        return qsTr("store queries in window")
    case "messaging.filter_requests_recent":
        return qsTr("filter requests in window")
    case "messaging.lightpush_requests_recent":
        return qsTr("Lightpush requests in window")
    case "messaging.peer_exchange_requests_recent":
        return qsTr("peer exchange requests in window")
    case "messaging.store_errors_recent":
        return qsTr("Store/archive errors in window")
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

function preferredModuleMetricValue(root, kind, names) {
    for (let i = 0; i < names.length; ++i) {
        const value = root.moduleMetricValue(kind, names[i])
        if (value !== null && value !== undefined) {
            return value
        }
    }
    return null
}

function dashboardMetricAggregateDefinition(key) {
    switch (String(key || "")) {
    case "storage.failed_transfers_recent":
    case "storage.failed_transfers_total":
        return {
            kind: "storage",
            members: [
                [
                    "storage_block_exchange_requests_failed_total_total",
                    "storage_block_exchange_requests_failed_total",
                    "storage_block_exchange_requests_failed"
                ]
            ]
        }
    case "messaging.message_sent_events_recent":
        return {
            kind: "messaging",
            members: [
                [
                    "waku_lightpush_messages_total",
                    "waku_lightpush_messages",
                    {
                        name: "waku_service_requests_total",
                        labels: {
                            service: "/vac/waku/lightpush/2.0.0-beta1"
                        }
                    },
                    {
                        name: "waku_service_requests",
                        labels: {
                            service: "/vac/waku/lightpush/2.0.0-beta1"
                        }
                    }
                ],
                [
                    "waku_lightpush_v3_messages_total",
                    "waku_lightpush_v3_messages",
                    {
                        name: "waku_service_requests_total",
                        labels: { service: "/vac/waku/lightpush/3.0.0" }
                    },
                    {
                        name: "waku_service_requests",
                        labels: { service: "/vac/waku/lightpush/3.0.0" }
                    }
                ]
            ]
        }
    case "messaging.message_error_events_recent":
        return {
            kind: "messaging",
            members: [
                ["waku_node_errors_total", "waku_node_errors"],
                [
                    "waku_store_errors_total",
                    "waku_store_errors"
                ],
                [
                    "waku_archive_errors_total",
                    "waku_archive_errors"
                ],
                ["waku_filter_errors_total", "waku_filter_errors"],
                ["waku_lightpush_errors_total", "waku_lightpush_errors"],
                [
                    "waku_lightpush_v3_errors_total",
                    "waku_lightpush_v3_errors"
                ]
            ],
            fallback: [
                "message_error_events_recent"
            ]
        }
    case "messaging.store_query_requests_recent":
        return {
            kind: "messaging",
            members: [
                [
                    "waku_store_queries_total",
                    "waku_store_queries",
                    {
                        name: "waku_service_requests_total",
                        labels: { service: "/vac/waku/store-query/3.0.0" }
                    },
                    {
                        name: "waku_service_requests",
                        labels: { service: "/vac/waku/store-query/3.0.0" }
                    }
                ]
            ]
        }
    case "messaging.filter_requests_recent":
        return {
            kind: "messaging",
            members: [
                [
                    "waku_filter_requests_total",
                    "waku_filter_requests",
                    {
                        name: "waku_service_requests_total",
                        labels: {
                            service: "/vac/waku/filter-subscribe/2.0.0-beta1"
                        }
                    },
                    {
                        name: "waku_service_requests",
                        labels: {
                            service: "/vac/waku/filter-subscribe/2.0.0-beta1"
                        }
                    }
                ]
            ]
        }
    case "messaging.lightpush_requests_recent":
        return {
            kind: "messaging",
            members: [
                [
                    "waku_lightpush_messages_total",
                    "waku_lightpush_messages",
                    {
                        name: "waku_service_requests_total",
                        labels: {
                            service: "/vac/waku/lightpush/2.0.0-beta1"
                        }
                    },
                    {
                        name: "waku_service_requests",
                        labels: {
                            service: "/vac/waku/lightpush/2.0.0-beta1"
                        }
                    }
                ],
                [
                    "waku_lightpush_v3_messages_total",
                    "waku_lightpush_v3_messages",
                    {
                        name: "waku_service_requests_total",
                        labels: { service: "/vac/waku/lightpush/3.0.0" }
                    },
                    {
                        name: "waku_service_requests",
                        labels: { service: "/vac/waku/lightpush/3.0.0" }
                    }
                ]
            ]
        }
    case "messaging.peer_exchange_requests_recent":
        return {
            kind: "messaging",
            members: [
                [
                    {
                        name: "waku_service_requests_total",
                        labels: {
                            service: "/vac/waku/peer-exchange/2.0.0-alpha1"
                        }
                    },
                    {
                        name: "waku_service_requests",
                        labels: {
                            service: "/vac/waku/peer-exchange/2.0.0-alpha1"
                        }
                    }
                ]
            ]
        }
    case "messaging.store_errors_recent":
        return {
            kind: "messaging",
            members: [
                ["waku_store_errors_total", "waku_store_errors"],
                ["waku_archive_errors_total", "waku_archive_errors"]
            ]
        }
    default:
        return null
    }
}

function dashboardMetricSpecIdentity(spec) {
    const name = spec && typeof spec === "object"
        ? String(spec.name || spec.metric || spec.key || "")
        : String(spec || "")
    const labels = spec && typeof spec === "object"
            && spec.labels && typeof spec.labels === "object"
        ? spec.labels : {}
    const keys = Object.keys(labels).sort()
    const parts = []
    for (let i = 0; i < keys.length; ++i) {
        const label = keys[i]
        parts.push(JSON.stringify(label) + ":"
            + JSON.stringify(String(labels[label])))
    }
    return name + "{" + parts.join(",") + "}"
}

function dashboardMetricAggregateObservation(root, key) {
    const definition = dashboardMetricAggregateDefinition(key)
    if (!definition) {
        return null
    }
    const members = Array.isArray(definition.members)
        ? definition.members : []
    const series = []
    let total = 0
    for (let i = 0; i < members.length; ++i) {
        const candidates = Array.isArray(members[i])
            ? members[i] : [members[i]]
        for (let j = 0; j < candidates.length; ++j) {
            const value = dashboardMetricNumber(
                root.moduleMetricValue(definition.kind, candidates[j]))
            if (value === null) {
                continue
            }
            series.push({
                id: "member:" + String(i) + ":"
                    + dashboardMetricSpecIdentity(candidates[j]),
                value: value
            })
            total += value
            break
        }
    }
    if (series.length === 0) {
        const fallback = Array.isArray(definition.fallback)
            ? definition.fallback : []
        for (let i = 0; i < fallback.length; ++i) {
            const value = dashboardMetricNumber(
                root.moduleMetricValue(definition.kind, fallback[i]))
            if (value === null) {
                continue
            }
            series.push({
                id: "fallback:" + dashboardMetricSpecIdentity(fallback[i]),
                value: value
            })
            total = value
            break
        }
        if (series.length === 0) {
            return null
        }
    }
    return {
        value: total,
        signature: series.map(function (item) { return item.id }).join("|"),
        series: series
    }
}

function dashboardMetricAggregateValue(root, key) {
    const observation = dashboardMetricAggregateObservation(root, key)
    return observation ? observation.value : null
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
        return Array.isArray(root.dashboardProvisionalBlocks)
            ? root.dashboardProvisionalBlocks.length : null
    case "lez.pending_blocks_count":
        return root.mantleValue(["pending_blocks_count", "pending_blocks"])
    case "indexer.indexer_lag_vs_sequencer_head":
        return root.indexerLag()
    case "storage.peer_count":
        return preferredModuleMetricValue(root, "storage", [
            "dht_routing_table_nodes",
            { name: "libp2p_peers", labels: { type: "connected" } },
            "libp2p_peers",
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
    case "storage.failed_transfers_total":
        return dashboardMetricAggregateValue(root, key)
    case "messaging.peer_count":
        return preferredModuleMetricValue(root, "messaging", [
            { name: "libp2p_peers", labels: { type: "connected" } },
            "libp2p_peers",
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
        return dashboardMetricAggregateValue(root, key)
    case "messaging.message_propagated_events_recent":
        return root.moduleMetricValue("messaging", ["waku_node_messages_total", "waku_node_messages"])
    case "messaging.message_received_events_recent":
        return root.moduleMetricValue("messaging", ["waku_node_messages_total", "waku_node_messages", "message_received_events_recent"])
    case "messaging.message_error_events_recent":
        return dashboardMetricAggregateValue(root, key)
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
        return dashboardMetricAggregateValue(root, key)
    case "messaging.filter_requests_recent":
        return dashboardMetricAggregateValue(root, key)
    case "messaging.lightpush_requests_recent":
        return dashboardMetricAggregateValue(root, key)
    case "messaging.peer_exchange_requests_recent":
        return dashboardMetricAggregateValue(root, key)
    case "messaging.store_messages":
        return root.moduleMetricValue("messaging", ["waku_store_messages", "waku_archive_messages"])
    case "messaging.store_errors_recent":
        return dashboardMetricAggregateValue(root, key)
    case "messaging.publish_latency_ms":
    case "messaging.receive_latency_ms":
        return null
    default:
        return null
    }
}

function dashboardMetricValue(root, key) {
    switch (String(key || "")) {
    case "messaging.message_sent_events_recent":
    case "messaging.message_propagated_events_recent":
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
    case "messaging.message_sent_events_recent":
    case "messaging.message_propagated_events_recent":
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
    if (dashboardMetricAggregateDefinition(key)
            && dashboardMetricSeriesEvidenceAvailable(root, key)) {
        return dashboardMetricSeriesWindowDelta(
            root, key, Date.now(), dashboardMetricWindowMs(root, key))
    }
    const current = dashboardMetricNumber(dashboardMetricRawValue(root, key))
    if (current === null) {
        return null
    }
    const timestamp = Date.now()
    const history = root.dashboardMetricHistory || {}
    const samples = normalizedDashboardSamples(history[String(key || "")]).slice()
    const seen = normalizedDashboardSample(
        (root.dashboardMetricLastSeen || {})[String(key || "")]
    )
    if (seen && (samples.length === 0
            || seen.timestamp > samples[samples.length - 1].timestamp)) {
        samples.push(seen)
    }
    if (samples.length === 0) {
        return null
    }
    const latest = samples[samples.length - 1]
    if (Number(latest.value) !== current) {
        return null
    }
    return windowDeltaFromSamples(
        samples, timestamp, dashboardMetricWindowMs(root, key))
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

function recordDashboardSnapshot(root, prefixes) {
    const keys = dashboardGraphKeys()
    const wantedPrefixes = Array.isArray(prefixes) ? prefixes : []
    const next = root.copyMap(root.dashboardMetricHistory)
    const nextSeen = root.copyMap(root.dashboardMetricLastSeen)
    const nextSeries = root.copyMap(root.dashboardMetricSeriesHistory || {})
    const nextSeriesSeen = root.copyMap(
        root.dashboardMetricSeriesLastSeen || {})
    const now = Date.now()
    let historyChanged = false
    let seenChanged = false
    let seriesHistoryChanged = false
    let seriesSeenChanged = false
    for (let i = 0; i < keys.length; ++i) {
        const key = keys[i]
        if (!dashboardSnapshotIncludesKey(key, wantedPrefixes)) {
            continue
        }
        const aggregate = dashboardMetricUsesWindow(key)
            ? dashboardMetricAggregateObservation(root, key) : null
        const value = dashboardMetricNumber(aggregate
            ? aggregate.value : dashboardMetricRawValue(root, key))
        if (value === null) {
            continue
        }
        const update = dashboardMetricSampleUpdate(root, next[key], nextSeen[key], now, value)
        nextSeen[key] = update.lastSeen
        seenChanged = true
        if (update.changed) {
            next[key] = update.samples
            historyChanged = true
        }
        if (aggregate) {
            const seriesUpdate = dashboardMetricSeriesSampleUpdate(
                nextSeries[key], nextSeriesSeen[key],
                update.lastSeen.timestamp, aggregate)
            nextSeriesSeen[key] = seriesUpdate.lastSeen
            seriesSeenChanged = true
            if (seriesUpdate.changed) {
                nextSeries[key] = seriesUpdate.samples
                seriesHistoryChanged = true
            }
        }
    }
    if (seenChanged) {
        root.dashboardMetricLastSeen = nextSeen
    }
    if (historyChanged) {
        root.dashboardMetricHistory = next
    }
    if (seriesSeenChanged) {
        root.dashboardMetricSeriesLastSeen = nextSeriesSeen
    }
    if (seriesHistoryChanged) {
        root.dashboardMetricSeriesHistory = nextSeries
    }
    if (historyChanged || seriesHistoryChanged) {
        root.dashboardMetricHistoryRevision += 1
    }
}

function dashboardSnapshotIncludesKey(key, prefixes) {
    if (!Array.isArray(prefixes) || prefixes.length === 0) {
        return true
    }
    const metricKey = String(key || "")
    for (let i = 0; i < prefixes.length; ++i) {
        if (metricKey.indexOf(String(prefixes[i] || "")) === 0) {
            return true
        }
    }
    return false
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

function normalizedDashboardMetricSeriesFrame(frame) {
    if (!frame || typeof frame !== "object") {
        return null
    }
    const timestamp = dashboardMetricNumber(frame.timestamp)
    const rawSeries = Array.isArray(frame.series) ? frame.series : []
    if (timestamp === null || rawSeries.length === 0) {
        return null
    }
    const series = []
    for (let i = 0; i < rawSeries.length; ++i) {
        const item = rawSeries[i]
        const id = item && typeof item === "object"
            ? String(item.id || "") : ""
        const value = dashboardMetricNumber(
            item && typeof item === "object" ? item.value : null)
        if (!id.length || value === null) {
            return null
        }
        series.push({ id: id, value: value })
    }
    const signature = series.map(function (item) {
        return item.id
    }).join("|")
    if (String(frame.signature || signature) !== signature) {
        return null
    }
    return {
        timestamp: timestamp,
        signature: signature,
        series: series
    }
}

function normalizedDashboardMetricSeriesFrames(frames) {
    const rows = []
    const raw = Array.isArray(frames) ? frames : []
    for (let i = 0; i < raw.length; ++i) {
        const frame = normalizedDashboardMetricSeriesFrame(raw[i])
        if (frame) {
            rows.push(frame)
        }
    }
    return rows
}

function dashboardMetricSeriesFramesEqual(left, right) {
    const first = normalizedDashboardMetricSeriesFrame(left)
    const second = normalizedDashboardMetricSeriesFrame(right)
    if (!first || !second || first.signature !== second.signature
            || first.series.length !== second.series.length) {
        return false
    }
    for (let i = 0; i < first.series.length; ++i) {
        if (first.series[i].id !== second.series[i].id
                || first.series[i].value !== second.series[i].value) {
            return false
        }
    }
    return true
}

function dashboardMetricSeriesSampleUpdate(stored, lastSeen, timestamp,
        observation) {
    const samples = normalizedDashboardMetricSeriesFrames(stored)
    const previous = normalizedDashboardMetricSeriesFrame(lastSeen)
        || (samples.length > 0 ? samples[samples.length - 1] : null)
    const source = observation && typeof observation === "object"
        ? observation : null
    const current = normalizedDashboardMetricSeriesFrame({
        timestamp: timestamp,
        signature: source ? source.signature : "",
        series: source ? source.series : []
    })
    if (!current) {
        return { samples: samples, lastSeen: null, changed: false }
    }
    const lastStored = samples.length > 0 ? samples[samples.length - 1] : null
    let changed = false
    if (!lastStored) {
        samples.push(current)
        changed = true
    } else if (!previous
            || !dashboardMetricSeriesFramesEqual(previous, current)) {
        if (previous && previous.timestamp > lastStored.timestamp
                && dashboardMetricSeriesFramesEqual(previous, lastStored)) {
            samples.push(previous)
        }
        samples.push(current)
        changed = true
    }
    const trimmed = samples.length > 300
        ? samples.slice(samples.length - 300) : samples
    return {
        samples: normalizedDashboardMetricSeriesFrames(trimmed),
        lastSeen: current,
        changed: changed
    }
}

function dashboardMetricSeriesEvidenceAvailable(root, key) {
    const metricKey = String(key || "")
    const history = root.dashboardMetricSeriesHistory || {}
    const seen = root.dashboardMetricSeriesLastSeen || {}
    return history[metricKey] !== undefined || seen[metricKey] !== undefined
}

function dashboardMetricAcceptedSeriesFrames(root, key) {
    const metricKey = String(key || "")
    const history = root.dashboardMetricSeriesHistory || {}
    const frames = normalizedDashboardMetricSeriesFrames(
        history[metricKey]).slice()
    const seen = normalizedDashboardMetricSeriesFrame(
        (root.dashboardMetricSeriesLastSeen || {})[metricKey])
    if (seen && (frames.length === 0
            || seen.timestamp > frames[frames.length - 1].timestamp)) {
        frames.push(seen)
    }
    return frames
}

function dashboardMetricLatestSeriesEpoch(frames) {
    const rows = normalizedDashboardMetricSeriesFrames(frames)
    if (rows.length === 0) {
        return []
    }
    const signature = rows[rows.length - 1].signature
    let start = rows.length - 1
    while (start > 0 && rows[start - 1].signature === signature) {
        start -= 1
    }
    return rows.slice(start)
}

function dashboardMetricSeriesObservationMatches(frame, observation) {
    if (!frame || !observation) {
        return false
    }
    return dashboardMetricSeriesFramesEqual(frame, {
        timestamp: frame.timestamp,
        signature: observation.signature,
        series: observation.series
    })
}

function dashboardMetricSeriesWindowDelta(root, key, timestamp, windowMs) {
    const observation = dashboardMetricAggregateObservation(root, key)
    if (!observation) {
        return null
    }
    const frames = dashboardMetricAcceptedSeriesFrames(root, key)
    if (frames.length === 0
            || !dashboardMetricSeriesObservationMatches(
                frames[frames.length - 1], observation)) {
        return null
    }
    return windowDeltaFromSeriesFrames(frames, timestamp, windowMs)
}

function windowDeltaFromSeriesFrames(frames, timestamp, windowMs) {
    const rows = normalizedDashboardMetricSeriesFrames(frames)
    if (rows.length < 2) {
        return null
    }
    const latest = rows[rows.length - 1]
    let epochStart = rows.length - 1
    while (epochStart > 0
            && rows[epochStart - 1].signature === latest.signature) {
        epochStart -= 1
    }
    if (epochStart === rows.length - 1) {
        return null
    }
    const cutoff = timestamp - windowMs
    let baselineIndex = epochStart
    for (let i = rows.length - 1; i >= epochStart; --i) {
        if (rows[i].timestamp <= cutoff) {
            baselineIndex = i
            break
        }
    }
    if (latest.timestamp === rows[baselineIndex].timestamp) {
        return null
    }
    let delta = 0
    for (let i = baselineIndex + 1; i < rows.length; ++i) {
        const previous = rows[i - 1]
        const current = rows[i]
        for (let j = 0; j < current.series.length; ++j) {
            const before = previous.series[j].value
            const after = current.series[j].value
            delta += after >= before ? after - before : Math.max(0, after)
        }
    }
    return delta
}

function dashboardMetricSamples(root, key) {
    const revision = root.dashboardMetricHistoryRevision
    if (dashboardMetricUsesWindow(key)) {
        return dashboardMetricWindowSamples(root, key)
    }
    const history = root.dashboardMetricHistory || {}
    const metricKey = String(key || "")
    const samples = normalizedDashboardSamples(history[metricKey])
    const seen = normalizedDashboardSample(
        (root.dashboardMetricLastSeen || {})[metricKey]
    )
    if (seen && (samples.length === 0
            || seen.timestamp > samples[samples.length - 1].timestamp)) {
        samples.push(seen)
    }
    if (Array.isArray(samples) && samples.length > 0) {
        return samples
    }
    const value = dashboardMetricNumber(dashboardMetricValue(root, key))
    return value === null ? [] : [{ timestamp: Date.now(), value: value }]
}

function dashboardMetricNumber(value) {
    if (value === undefined || value === null || typeof value === "boolean") {
        return null
    }
    if (typeof value === "string" && value.trim().length === 0) {
        return null
    }
    if (typeof value !== "number" && typeof value !== "string") {
        return null
    }
    const numeric = Number(value)
    return Number.isFinite(numeric) ? numeric : null
}

function normalizedDashboardSample(sample) {
    if (!sample || typeof sample !== "object") {
        return null
    }
    const value = dashboardMetricNumber(sample.value)
    const timestamp = dashboardMetricNumber(sample.timestamp)
    if (value === null || timestamp === null) {
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
        const value = dashboardMetricNumber(
            sample && typeof sample === "object" ? sample.value : sample)
        if (value === null) {
            continue
        }
        const timestamp = dashboardMetricNumber(
            sample && typeof sample === "object" ? sample.timestamp : i)
        rows.push({
            timestamp: timestamp === null ? i : timestamp,
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
    if (dashboardMetricAggregateDefinition(key)
            && dashboardMetricSeriesEvidenceAvailable(root, key)) {
        const frames = dashboardMetricLatestSeriesEpoch(
            dashboardMetricAcceptedSeriesFrames(root, key))
        const rows = []
        const windowMs = dashboardMetricWindowMs(root, key)
        for (let i = 0; i < frames.length; ++i) {
            const delta = windowDeltaFromSeriesFrames(
                frames.slice(0, i + 1), frames[i].timestamp, windowMs)
            if (delta !== null) {
                rows.push({
                    timestamp: frames[i].timestamp,
                    value: delta
                })
            }
        }
        return rows
    }
    const history = root.dashboardMetricHistory || {}
    const samples = normalizedDashboardSamples(history[String(key || "")])
    const seen = normalizedDashboardSample(
        (root.dashboardMetricLastSeen || {})[String(key || "")]
    )
    if (seen && (samples.length === 0
            || seen.timestamp > samples[samples.length - 1].timestamp)) {
        samples.push(seen)
    }
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
    let baselineIndex = 0
    for (let i = rows.length - 1; i >= 0; --i) {
        if (rows[i].timestamp <= cutoff) {
            baselineIndex = i
            break
        }
    }
    const latest = rows[rows.length - 1]
    if (latest.timestamp === rows[baselineIndex].timestamp) {
        return null
    }
    let delta = 0
    let previous = rows[baselineIndex].value
    for (let i = baselineIndex + 1; i < rows.length; ++i) {
        const current = rows[i].value
        delta += current >= previous ? current - previous : Math.max(0, current)
        previous = current
    }
    return delta
}

function clearDashboardMetricHistoryForPrefixes(root, prefixes) {
    const raw = Array.isArray(prefixes) ? prefixes : [prefixes]
    const values = []
    for (let i = 0; i < raw.length; ++i) {
        const value = String(raw[i] || "")
        if (value.length > 0) {
            values.push(value)
        }
    }
    if (values.length === 0) {
        return
    }
    const next = root.copyMap(root.dashboardMetricHistory)
    const seen = root.copyMap(root.dashboardMetricLastSeen)
    const seriesNext = root.copyMap(root.dashboardMetricSeriesHistory || {})
    const seriesSeen = root.copyMap(root.dashboardMetricSeriesLastSeen || {})
    let changed = false
    for (const key in next) {
        if (values.some(function (prefix) {
            return String(key || "").indexOf(prefix) === 0
        })) {
            delete next[key]
            changed = true
        }
    }
    for (const seenKey in seen) {
        if (values.some(function (prefix) {
            return String(seenKey || "").indexOf(prefix) === 0
        })) {
            delete seen[seenKey]
            changed = true
        }
    }
    for (const seriesKey in seriesNext) {
        if (values.some(function (prefix) {
            return String(seriesKey || "").indexOf(prefix) === 0
        })) {
            delete seriesNext[seriesKey]
            changed = true
        }
    }
    for (const seriesSeenKey in seriesSeen) {
        if (values.some(function (prefix) {
            return String(seriesSeenKey || "").indexOf(prefix) === 0
        })) {
            delete seriesSeen[seriesSeenKey]
            changed = true
        }
    }
    if (changed) {
        root.dashboardMetricHistory = next
        root.dashboardMetricLastSeen = seen
        root.dashboardMetricSeriesHistory = seriesNext
        root.dashboardMetricSeriesLastSeen = seriesSeen
        root.dashboardMetricHistoryRevision += 1
    }
}

function clearDashboardMetricHistoryForPrefix(root, prefix) {
    return clearDashboardMetricHistoryForPrefixes(root, [prefix])
}
