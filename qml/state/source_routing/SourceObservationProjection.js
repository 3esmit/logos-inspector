.import "../../utils/UiFormat.js" as UiFormat

function storageIdentityEvidence(page) {
    const factEvidence = page.sourceFactEvidence("identity", "")
    if (factEvidence.length > 0 && factEvidence !== "not observed") {
        return factEvidence
    }
    const peerId = page.probeValue("peerId")
    if (peerId !== null) {
        return qsTr("peer id present")
    }
    const spr = page.probeValue("spr")
    if (spr !== null) {
        return qsTr("SPR present")
    }
    return page.sourceName()
}

function storageCapacitySummary(page) {
    const factEvidence = page.sourceFactEvidence("space", "")
    if (factEvidence.length > 0 && factEvidence !== "not observed") {
        return factEvidence
    }
    const used = page.metricDisplay("storage.local_storage_used")
    if (used !== qsTr("n/a")) {
        return used
    }
    const space = page.probeValue("space")
    return space !== null ? page.valueSummary(space) : qsTr("n/a")
}

function storageTransferSummary(page) {
    const uploads = page.metricDisplay("storage.active_uploads")
    const downloads = page.metricDisplay("storage.active_downloads")
    if (uploads === qsTr("n/a") && downloads === qsTr("n/a")) {
        return qsTr("n/a")
    }
    return qsTr("%1 upload requests / %2 download requests").arg(uploads).arg(downloads)
}

function storageReliabilityText(page) {
    if (page.failedProbeCount() > 0) {
        return qsTr("Degraded")
    }
    if (page.metricKnown("storage.failed_transfers_recent")) {
        return Number(page.model.metrics.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? qsTr("Recent failures") : qsTr("No failures")
    }
    return qsTr("Unknown")
}

function storageReliabilityTone(page) {
    if (page.failedProbeCount() > 0) {
        return page.theme.error
    }
    if (page.metricKnown("storage.failed_transfers_recent")) {
        return Number(page.model.metrics.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? page.theme.error : page.theme.success
    }
    return page.theme.textMuted
}

function storageTransferFailureTone(page) {
    if (!page.metricKnown("storage.failed_transfers_recent")) {
        return page.theme.textMuted
    }
    return Number(page.model.metrics.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? page.theme.error : page.theme.success
}

function storageHealthRows(page) {
    const status = page.status()
    const identityKnown = page.sourceFactAvailable("identity") || page.probeKnown("peerId") || page.probeKnown("spr")
    const debugKnown = page.sourceFactAvailable("debug") || page.probeKnown("debug")
    const spaceKnown = page.sourceFactAvailable("space") || page.probeKnown("space") || page.metricKnown("storage.local_storage_used")
    return [
        page.statusRow(qsTr("Source and lifecycle"), status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"), status.detail || qsTr("Not queried"), page.statusTone()),
        page.statusRow(qsTr("Identity"), identityKnown ? qsTr("present") : qsTr("unknown"), storageIdentityEvidence(page), identityKnown ? "success" : "neutral"),
        page.statusRow(qsTr("REST and metrics access"), page.restMetricsState(), page.restMetricsEvidence(), page.restMetricsTone()),
        page.statusRow(qsTr("DHT / discovery"), debugKnown ? qsTr("observed") : qsTr("unknown"), debugKnown ? page.sourceFactEvidence("debug", page.valueSummary(page.probeValue("debug"))) : qsTr("Debug source unavailable."), debugKnown ? "success" : "neutral"),
        page.statusRow(qsTr("Connected peers"), page.metricKnown("storage.peer_count") ? qsTr("observed") : qsTr("unknown"), page.metricDisplay("storage.peer_count"), page.metricKnown("storage.peer_count") ? "success" : "neutral"),
        page.statusRow(qsTr("Repository and host disk"), spaceKnown ? qsTr("observed") : qsTr("unknown"), storageCapacitySummary(page), spaceKnown ? "success" : "neutral"),
        page.statusRow(qsTr("Recent transfer failures"), page.metricKnown("storage.failed_transfers_recent") ? page.metricDisplay("storage.failed_transfers_recent") : qsTr("unknown"), page.metricKnown("storage.failed_transfers_recent") ? qsTr("%1 s window").arg(page.rollingWindow()) : qsTr("Metric not exposed by current source."), page.metricKnown("storage.failed_transfers_recent") ? (Number(page.model.metrics.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? "error" : "success") : "neutral"),
        page.statusRow(qsTr("Mix / private queries"), qsTr("not queried"), qsTr("No passive metric selected."), "neutral")
    ]
}

function storageActiveOperationRows(page) {
    return [
        page.metricRow(qsTr("Upload requests"), "storage.active_uploads"),
        page.metricRow(qsTr("Download requests"), "storage.active_downloads"),
        page.metricRow(qsTr("Recent transfer failures"), "storage.failed_transfers_recent"),
        page.metricRow(qsTr("Historical transfer failures"), "storage.failed_transfers_total"),
        page.statusRow(qsTr("Provider lookup"), qsTr("idle"), qsTr("Explicit diagnostic only."), "neutral"),
        page.activeDownloadRow()
    ]
}

function storageTopologyRows(page) {
    return [
        page.statusRow(qsTr("DHT routing table"), page.probeKnown("debug") ? qsTr("observed") : qsTr("unknown"), page.probeKnown("debug") ? page.valueSummary(page.probeValue("debug")) : qsTr("Current source has no DHT table."), page.probeKnown("debug") ? "success" : "neutral"),
        page.statusRow(qsTr("Connected peers"), page.metricKnown("storage.peer_count") ? page.metricDisplay("storage.peer_count") : qsTr("unknown"), page.metricKnown("storage.peer_count") ? qsTr("%1 s window").arg(page.rollingWindow()) : qsTr("Metric not exposed by current source."), page.metricKnown("storage.peer_count") ? "success" : "neutral"),
        page.statusRow(qsTr("Providers for CID"), page.storageCidProbe().length > 0 ? qsTr("not queried") : qsTr("no CID"), page.storageCidProbe().length > 0 ? qsTr("Provider lookup is explicit.") : qsTr("Select a CID first."), "neutral"),
        page.statusRow(qsTr("Block exchange peers"), qsTr("unknown"), qsTr("Passive source does not expose transfer edges."), "neutral"),
        page.statusRow(qsTr("Mix proxies"), qsTr("unknown"), qsTr("Private-query topology is not exposed passively."), "neutral")
    ]
}

function storageNetworkDebugRows(page, maxRoutingNodes) {
    const value = page.probeValue("debug")
    if (value === undefined || value === null) {
        return []
    }
    const debug = unwrappedObject(value)
    if (debug === null) {
        return [storageNetworkDebugDetailRow(
            page, qsTr("Network snapshot"), page.valueSummary(value), value)]
    }

    const tableValue = objectField(debug, ["table"])
    const table = tableValue && typeof tableValue === "object"
        && !Array.isArray(tableValue) ? tableValue : null
    const nodesValue = objectField(table, ["nodes"])
    const nodes = Array.isArray(nodesValue) ? nodesValue : []
    const configuredLimit = Number(maxRoutingNodes)
    const limit = Number.isFinite(configuredLimit)
        ? Math.max(0, Math.floor(configuredLimit)) : 50
    const shownCount = Math.min(nodes.length, limit)
    const rows = [storageNetworkDebugDetailRow(
        page,
        qsTr("Network snapshot"),
        qsTr("%1 field(s); %2 routing node(s)")
            .arg(Object.keys(debug).length)
            .arg(nodes.length),
        debug)]

    appendStorageDebugValue(rows, page, qsTr("Network peer ID"),
                            objectField(debug, ["id", "peerId", "peer_id"]))
    appendStorageDebugList(rows, page, qsTr("Listen address"),
                           objectField(debug, ["addrs", "listenAddresses", "listen_addresses"]),
                           limit)
    appendStorageDebugList(rows, page, qsTr("Announce address"),
                           objectField(debug, ["announceAddresses", "announce_addresses"]),
                           limit)
    appendStorageDebugValue(rows, page, qsTr("libp2p public key"),
                            objectField(debug, ["libp2pPubKey", "libp2p_pub_key"]))
    appendStorageDebugValue(rows, page, qsTr("Mix public key"),
                            objectField(debug, ["mixPubKey", "mix_pub_key"]))
    appendStorageDebugValue(rows, page, qsTr("Provider record"),
                            objectField(debug, ["providerRecord", "provider_record"]))
    appendStorageDebugValue(rows, page, qsTr("Self peer record"),
                            objectField(debug, ["spr"]))

    const storageValue = objectField(debug, ["storage"])
    appendStorageDebugValue(rows, page, qsTr("Storage version"),
                            objectField(storageValue, ["version"]))
    appendStorageDebugValue(rows, page, qsTr("Storage revision"),
                            objectField(storageValue, ["revision"]))

    const localNode = objectField(table, ["localNode", "local_node"])
    if (hasStorageDebugValue(localNode)) {
        rows.push(storageNetworkDebugDetailRow(
            page, qsTr("DHT local node"), storageDebugNodeSummary(page, localNode), localNode))
    }
    if (Array.isArray(nodesValue)) {
        rows.push(storageNetworkDebugDetailRow(
            page,
            qsTr("DHT routing nodes"),
            qsTr("%1 node(s); showing %2").arg(nodes.length).arg(shownCount),
            nodes))
    }
    for (let i = 0; i < shownCount; ++i) {
        rows.push(storageNetworkDebugDetailRow(
            page,
            qsTr("Routing node %1").arg(i + 1),
            storageDebugNodeSummary(page, nodes[i]),
            nodes[i]))
    }
    return rows
}

function storageNetworkDebugDetailRow(page, label, displayValue, copyValue) {
    const row = page.detailRow(label, displayValue)
    row.value = String(displayValue === undefined || displayValue === null
        ? qsTr("n/a") : displayValue)
    row.copyText = hasStorageDebugValue(copyValue) ? page.copyValue(copyValue) : ""
    row.source = page.sourceName()
    return row
}

function appendStorageDebugValue(rows, page, label, value) {
    if (!hasStorageDebugValue(value)) {
        return
    }
    rows.push(storageNetworkDebugDetailRow(page, label, page.valueSummary(value), value))
}

function appendStorageDebugList(rows, page, label, value, maxItems) {
    if (!Array.isArray(value)) {
        return
    }
    const shownCount = Math.min(value.length, maxItems)
    for (let i = 0; i < shownCount; ++i) {
        rows.push(storageNetworkDebugDetailRow(
            page, qsTr("%1 %2").arg(label).arg(i + 1), page.valueSummary(value[i]), value[i]))
    }
    if (shownCount < value.length) {
        rows.push(storageNetworkDebugDetailRow(
            page,
            qsTr("%1 list").arg(label),
            qsTr("%1 item(s); showing %2").arg(value.length).arg(shownCount),
            value))
    }
}

function storageDebugNodeSummary(page, value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
        return page.valueSummary(value)
    }
    const parts = []
    const peerId = objectField(value, ["peerId", "peer_id"])
    const address = objectField(value, ["address", "addr"])
    const nodeId = objectField(value, ["nodeId", "node_id"])
    if (hasStorageDebugValue(peerId)) {
        parts.push(String(peerId))
    }
    if (hasStorageDebugValue(address)) {
        parts.push(String(address))
    }
    if (hasStorageDebugValue(nodeId)) {
        parts.push(String(nodeId))
    }
    return parts.length > 0 ? parts.join(" | ") : page.valueSummary(value)
}

function hasStorageDebugValue(value) {
    return value !== undefined && value !== null && String(value).length > 0
}

function unwrappedObject(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
        return null
    }
    if (value.result !== undefined) {
        return unwrappedObject(value.result)
    }
    if (value.value !== undefined) {
        return unwrappedObject(value.value)
    }
    return value
}

function storageCapacityRows(page) {
    return [
        page.spaceRow(qsTr("Quota used"), ["quotaUsedBytes", "quota_used_bytes", "used", "usedBytes"]),
        page.spaceRow(qsTr("Quota reserved"), ["quotaReservedBytes", "quota_reserved_bytes", "reserved", "reservedBytes"]),
        page.spaceRow(qsTr("Quota max"), ["quotaMaxBytes", "quota_max_bytes", "max", "maxBytes"]),
        page.spaceRow(qsTr("Total blocks"), ["totalBlocks", "total_blocks", "blocks"]),
        page.metricRow(qsTr("Local storage used"), "storage.local_storage_used")
    ]
}

function storageRepositoryRows(page) {
    const dataDirKnown = page.probeKnown("dataDir")
    const dataDir = dataDirKnown
        ? page.model.storageDisplayPath(page.copyValue(page.probeValue("dataDir")))
        : qsTr("No path reported by current source.")
    return [
        page.statusRow(qsTr("Data directory"), dataDirKnown ? qsTr("reported") : qsTr("unknown"), dataDir, dataDirKnown ? "success" : "neutral"),
        page.metricRow(qsTr("Shared files"), "storage.shared_files_count"),
        page.manifestCountRow()
    ]
}

function storageTransferRows(page) {
    return [
        page.metricRow(qsTr("Upload requests"), "storage.active_uploads"),
        page.metricRow(qsTr("Download requests"), "storage.active_downloads"),
        page.metricRow(qsTr("Recent transfer failures"), "storage.failed_transfers_recent"),
        page.metricRow(qsTr("Historical transfer failures"), "storage.failed_transfers_total"),
        page.activeDownloadRow()
    ]
}

function storageActiveDownloadRow(page) {
    const operation = page.activeStorageOperation()
    const status = String(operation && operation.status ? operation.status : "")
    if (!operation || !status.length) {
        return page.statusRow(qsTr("Network download"), qsTr("idle"), qsTr("No active background download."), "success")
    }
    let tone = "neutral"
    if (status === "running" || status === "awaiting_external" || status === "canceling" || status === "dispatched") {
        tone = "warning"
    } else if (status === "completed") {
        tone = "success"
    } else if (status === "failed" || status === "timed_out") {
        tone = "error"
    }
    return page.statusRow(qsTr("Network download"), status, page.activeStorageOperationDetail(operation), tone)
}

function storageActiveStorageOperationDetail(page, operation) {
    const written = Number(operation && operation.bytesWritten ? operation.bytesWritten : 0)
    const total = Number(operation && operation.contentLength ? operation.contentLength : 0)
    const path = operation && operation.path ? page.shortText(operation.path, 42) : qsTr("n/a")
    if (Number.isFinite(total) && total > 0) {
        const percent = Math.min(100, Math.max(0, Math.floor((written / total) * 100)))
        return qsTr("%1 / %2 bytes (%3%) to %4")
            .arg(page.model.metrics.valueText(written))
            .arg(page.model.metrics.valueText(total))
            .arg(percent)
            .arg(path)
    }
    return qsTr("%1 bytes to %2").arg(page.model.metrics.valueText(written)).arg(path)
}

function storageCidRows(page) {
    const cid = page.storageCidProbe().trim()
    if (!cid.length) {
        return [
            page.detailRow(qsTr("Selected CID"), qsTr("n/a")),
            page.detailRow(qsTr("Network diagnostics"), qsTr("Not queried"))
        ]
    }
    const exists = page.probe("exists")
    return [
        page.detailRow(qsTr("Selected CID"), cid),
        page.detailRow(qsTr("Local exists"), exists ? (exists.ok ? page.valueSummary(exists.value) : String(exists.error || qsTr("problem"))) : qsTr("Not queried")),
        page.detailRow(qsTr("Manifest"), qsTr("Not fetched")),
        page.detailRow(qsTr("Providers"), qsTr("Not queried")),
        page.detailRow(qsTr("Transfer"), qsTr("Idle"))
    ]
}

function storageProtocolRows(page) {
    return [
        page.protocolRow(qsTr("Store / RepoStore"), "repository", page.probeKnown("space") || page.probeKnown("manifests"), page.probeKnown("space") ? page.valueSummary(page.probeValue("space")) : page.valueSummary(page.probeValue("manifests"))),
        page.protocolRow(qsTr("Dataset / Manifest"), "storage-manifest", page.probeKnown("manifests"), page.valueSummary(page.probeValue("manifests"))),
        page.protocolRow(qsTr("Merkle verification"), "storage-root", false, qsTr("No passive verification source.")),
        page.protocolRow(qsTr("DHT discovery"), "libp2p/kad-dht", page.probeKnown("debug"), page.probeKnown("debug") ? page.valueSummary(page.probeValue("debug")) : qsTr("No DHT table.")),
        page.protocolRow(qsTr("Block exchange"), "storage/blockexchange", page.metricKnown("storage.active_downloads") || page.metricKnown("storage.active_uploads"), page.transferSummary()),
        page.protocolRow(qsTr("REST / C API"), "/api/storage/v1", page.storageSourceMode() === "rest", page.sourceTarget()),
        page.protocolRow(qsTr("Mix / private queries"), "private queries", false, qsTr("No passive signal."))
    ]
}

function storageIdentityRows(page) {
    return [
        page.detailRow(qsTr("Peer ID"), page.probeValue("peerId")),
        page.detailRow(qsTr("SPR"), page.probeValue("spr")),
        page.pathDetailRow(qsTr("Data directory"), page.probeValue("dataDir")),
        page.detailRow(qsTr("Version"), page.probeValue("version") || page.probeValue("moduleVersion")),
        page.detailRow(qsTr("Network preset"), page.sourceNetworkPreset()),
        page.detailRow(qsTr("Source target"), page.sourceTarget())
    ]
}

function storageMetricRow(page, label, key) {
    const known = page.metricKnown(key)
    const tone = known && String(key || "") === "storage.failed_transfers_recent" && Number(page.model.metrics.dashboardMetricValue(key)) > 0 ? "error" : (known ? "success" : "neutral")
    return page.statusRow(label, known ? page.metricDisplay(key) : qsTr("n/a"), known ? page.metricEvidence(key) : qsTr("Metric not exposed by current source."), tone)
}

function storageMetricEvidence(page, key) {
    switch (String(key || "")) {
    case "storage.active_uploads":
    case "storage.active_downloads":
    case "storage.failed_transfers_total":
        return qsTr("Counter total")
    default:
        return qsTr("%1 s window").arg(page.rollingWindow())
    }
}

function storageManifestCountRow(page) {
    const manifests = page.probeValue("manifests")
    if (Array.isArray(manifests)) {
        return page.statusRow(qsTr("Manifests"), qsTr("%1").arg(manifests.length), qsTr("Local manifest list"), "success")
    }
    return page.metricRow(qsTr("Manifests"), "storage.manifest_count")
}

function storageSpaceRow(page, label, keys) {
    const value = objectField(page.probeValue("space"), keys)
    if (value !== null) {
        return page.statusRow(label, page.model.metrics.valueText(value), qsTr("space"), "success")
    }
    return page.statusRow(label, qsTr("n/a"), page.probeKnown("space") ? qsTr("Field not exposed by current space shape.") : qsTr("Space source unavailable."), "neutral")
}

function storageProtocolRow(label, protocolId, observed, evidence) {
    return {
        label: label,
        protocolId: protocolId,
        state: observed ? qsTr("observed") : qsTr("unknown"),
        evidence: evidence === undefined || evidence === null || evidence === "" ? qsTr("No passive evidence") : String(evidence),
        tone: observed ? "success" : "neutral"
    }
}

function storagePathDetailRow(page, label, value) {
    const raw = page.copyValue(value)
    const text = page.model.storageDisplayPath(raw)
    return {
        label: label,
        value: text.length ? text : qsTr("n/a"),
        copyText: page.model.storageLocalDiagnosticsEnabled ? page.copyValue(value) : "",
        source: page.sourceName()
    }
}

function storageRestMetricsState(page) {
    const sourceMode = page.storageSourceMode()
    const metricsKnown = page.sourceFactAvailable("metrics")
    if (sourceMode === "module") {
        return metricsKnown || page.probeValue("collectMetrics") !== null ? qsTr("metrics") : qsTr("module")
    }
    if (sourceMode === "rest") {
        const metricsProbe = page.probe("collectMetrics")
        if (page.metricsEndpointConfigured() && metricsProbe && metricsProbe.ok === false) {
            return qsTr("metrics error")
        }
        if (page.metricsEndpointConfigured() && (!metricsProbe || metricsProbe.ok !== true)) {
            return page.status().ok ? qsTr("REST only") : qsTr("unknown")
        }
        return metricsKnown ? qsTr("REST + metrics") : (page.status().ok ? qsTr("reachable") : qsTr("unknown"))
    }
    if (sourceMode === "metrics") {
        return page.status().ok ? qsTr("scraping") : qsTr("unknown")
    }
    return qsTr("pending")
}

function storageRestMetricsEvidence(page) {
    const sourceMode = page.storageSourceMode()
    const metricsEvidence = page.sourceFactEvidence("metrics", "")
    if (sourceMode === "module") {
        return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : qsTr("Module API")
    }
    if (sourceMode === "metrics") {
        return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : page.shortText(page.sourceMetricsEndpoint(), 48)
    }
    if (sourceMode === "rest" && page.metricsEndpointConfigured()) {
        const metricsProbe = page.probe("collectMetrics")
        if (metricsProbe && metricsProbe.ok === false && metricsProbe.error) {
            return qsTr("REST %1; metrics %2: %3")
                .arg(page.shortText(page.sourceRestEndpoint(), 24))
                .arg(page.shortText(page.sourceMetricsEndpoint(), 24))
                .arg(page.shortText(metricsProbe.error, 36))
        }
        return qsTr("REST %1; metrics %2")
            .arg(page.shortText(page.sourceRestEndpoint(), 28))
            .arg(page.shortText(page.sourceMetricsEndpoint(), 28))
    }
    return page.shortText(page.sourceRestEndpoint(), 48)
}

function storageRestMetricsTone(page) {
    const sourceMode = page.storageSourceMode()
    if (sourceMode === "c-library" || sourceMode === "local-os") {
        return "warning"
    }
    if (sourceMode === "rest") {
        const metricsProbe = page.probe("collectMetrics")
        if (page.metricsEndpointConfigured() && metricsProbe && metricsProbe.ok === false) {
            return "error"
        }
        if (page.metricsEndpointConfigured()
                && page.model.metrics.sourceCapabilityAvailable(page.report(), "metrics") === false) {
            return "warning"
        }
        if (page.metricsEndpointConfigured() && (!metricsProbe || metricsProbe.ok !== true)) {
            return "warning"
        }
    }
    return page.statusTone()
}

function objectField(value, keys) {
    if (value === undefined || value === null) {
        return null
    }
    if (typeof value !== "object") {
        return null
    }
    if (value.result !== undefined) {
        return objectField(value.result, keys)
    }
    if (value.value !== undefined) {
        return objectField(value.value, keys)
    }
    const wanted = Array.isArray(keys) ? keys : [keys]
    for (let i = 0; i < wanted.length; ++i) {
        const key = String(wanted[i] || "")
        if (value[key] !== undefined && value[key] !== null) {
            return value[key]
        }
    }
    return null
}

function deliveryIdentityEvidence(page) {
    const factEvidence = page.sourceFactEvidence("identity", "")
    if (factEvidence.length > 0 && factEvidence !== "not observed") {
        return factEvidence
    }
    const peerId = page.identityValue("peerId")
    if (peerId !== null) {
        return qsTr("peer id present")
    }
    const addresses = page.identityValue("listenAddresses")
    if (addresses !== null) {
        return qsTr("addresses present")
    }
    return page.sourceName()
}

function deliverySourceFactObservedState(page, key, fallbackKnown) {
    return page.sourceFactAvailable(key) || fallbackKnown ? qsTr("observed") : qsTr("unknown")
}

function deliverySourceFactObservedTone(page, key, fallbackKnown) {
    return page.sourceFactAvailable(key) || fallbackKnown ? "success" : "neutral"
}

function deliveryHealthRows(page) {
    const status = page.status()
    const identity = page.identityValue("peerId") || page.identityValue("enrUri") || page.identityValue("listenAddresses")
    const nodeHealth = page.probeValue("nodeHealth")
    const connectionStatus = page.probeValue("connectionStatus")
    const nodeTone = page.combinedHealthTone(nodeHealth, connectionStatus)
    const relayKnown = page.metricKnown("messaging.pubsub_peers")
    const storeKnown = page.metricKnown("messaging.store_peers")
    const filterKnown = page.metricKnown("messaging.filter_peers")
    const lightpushKnown = page.metricKnown("messaging.lightpush_peers")
    const discovered = page.networkMonitorPeerCount()
    const discoveryKnown = discovered !== null
    return [
        page.statusRow(qsTr("Source and lifecycle"), status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"), status.detail || qsTr("Not queried"), page.statusTone()),
        page.statusRow(qsTr("Identity"), page.sourceFactAvailable("identity") || identity !== null ? qsTr("present") : qsTr("unknown"), page.identityEvidence(), page.sourceFactAvailable("identity") || identity !== null ? "success" : "neutral"),
        page.statusRow(qsTr("Node health"), nodeHealth !== null ? page.valueSummary(nodeHealth) : qsTr("unknown"), page.valueSummary(connectionStatus), nodeTone),
        page.statusRow(qsTr("Preset, cluster, shards"), page.sourceNetworkPreset().length ? qsTr("configured") : qsTr("unknown"), page.sourceNetworkPreset() || qsTr("No preset"), page.sourceNetworkPreset().length ? "success" : "neutral"),
        page.statusRow(qsTr("REST and metrics access"), page.restMetricsState(), page.restMetricsEvidence(), page.restMetricsTone()),
        page.statusRow(qsTr("Relay"), page.sourceFactObservedState("relay", relayKnown), relayKnown ? page.metricDisplay("messaging.pubsub_peers") : page.sourceFactEvidence("relay", qsTr("No relay fact.")), page.sourceFactObservedTone("relay", relayKnown)),
        page.statusRow(qsTr("Store"), page.sourceFactObservedState("store", storeKnown), storeKnown ? page.metricDisplay("messaging.store_peers") : page.sourceFactEvidence("store", qsTr("No Store fact.")), page.sourceFactObservedTone("store", storeKnown)),
        page.statusRow(qsTr("Filter"), page.sourceFactObservedState("filter", filterKnown), filterKnown ? page.metricDisplay("messaging.filter_peers") : page.sourceFactEvidence("filter", qsTr("No Filter fact.")), page.sourceFactObservedTone("filter", filterKnown)),
        page.statusRow(qsTr("Lightpush"), page.sourceFactObservedState("lightpush", lightpushKnown), lightpushKnown ? page.metricDisplay("messaging.lightpush_peers") : page.sourceFactEvidence("lightpush", qsTr("No Lightpush fact.")), page.sourceFactObservedTone("lightpush", lightpushKnown)),
        page.statusRow(qsTr("Discovery"), page.sourceFactObservedState("network_monitor", discoveryKnown), discoveryKnown ? qsTr("%1 peer(s)").arg(discovered) : page.sourceFactEvidence("network_monitor", qsTr("No Delivery Network Monitor peer snapshot.")), page.sourceFactObservedTone("network_monitor", discoveryKnown)),
        page.statusRow(qsTr("RLN / spam protection"), qsTr("unknown"), qsTr("No passive metric selected"), "neutral")
    ]
}

function deliveryTopologyRows(page) {
    const discovered = page.networkMonitorPeerCount()
    const topics = page.networkMonitorTopicCount()
    const servicePeers = page.servicePeerCount()
    return [
        page.statusRow(qsTr("Local connected peers"), page.metricKnown("messaging.peer_count") ? qsTr("observed") : qsTr("unknown"), page.metricDisplay("messaging.peer_count"), page.metricKnown("messaging.peer_count") ? "success" : "neutral"),
        page.statusRow(qsTr("Relay mesh peers"), page.metricKnown("messaging.pubsub_peers") ? qsTr("observed") : qsTr("unknown"), page.metricDisplay("messaging.pubsub_peers"), page.metricKnown("messaging.pubsub_peers") ? "success" : "neutral"),
        page.statusRow(qsTr("Discovery peers"), discovered !== null ? qsTr("observed") : qsTr("unknown"), discovered !== null ? qsTr("%1 peer(s)").arg(discovered) : qsTr("No Delivery Network Monitor peer snapshot."), discovered !== null ? "success" : "neutral"),
        page.statusRow(qsTr("Service peers"), servicePeers !== null ? qsTr("observed") : qsTr("unknown"), servicePeers !== null ? qsTr("%1 service peer(s)").arg(servicePeers) : qsTr("No Store/Filter/Lightpush peer metrics."), servicePeers !== null ? "success" : "neutral"),
        page.statusRow(qsTr("Content topics"), topics !== null ? qsTr("observed") : qsTr("unknown"), topics !== null ? qsTr("%1 topic(s)").arg(topics) : qsTr("No Delivery Network Monitor topic snapshot."), topics !== null ? "success" : "neutral")
    ]
}

function deliveryThroughputRows(page) {
    return [
        page.metricRow(qsTr("Peer count"), "messaging.peer_count"),
        page.metricRow(qsTr("Pubsub peers"), "messaging.pubsub_peers"),
        page.metricRow(qsTr("Network ingress"), "messaging.network_ingress_recent"),
        page.metricRow(qsTr("Network egress"), "messaging.network_egress_recent"),
        page.metricRow(qsTr("Relay ingress"), "messaging.relay_ingress_recent"),
        page.metricRow(qsTr("Relay egress"), "messaging.relay_egress_recent"),
        page.metricRow(qsTr("Service ingress"), "messaging.service_ingress_recent"),
        page.metricRow(qsTr("Service egress"), "messaging.service_egress_recent"),
        page.metricRow(qsTr("Sent events"), "messaging.message_sent_events_recent"),
        page.metricRow(qsTr("Propagated events"), "messaging.message_propagated_events_recent"),
        page.metricRow(qsTr("Messages in window"), "messaging.message_received_events_recent"),
        page.metricRow(qsTr("Errors in window"), "messaging.message_error_events_recent"),
        page.metricRow(qsTr("Store peers"), "messaging.store_peers"),
        page.metricRow(qsTr("Filter peers"), "messaging.filter_peers"),
        page.metricRow(qsTr("Lightpush peers"), "messaging.lightpush_peers")
    ]
}

function deliveryProtocolRows(page) {
    const healthRows = page.protocolHealthRows()
    if (healthRows.length > 0) {
        return healthRows
    }
    return [
        page.protocolRow(qsTr("Relay"), "/vac/waku/relay/2.0.0", "messaging.pubsub_peers"),
        page.protocolRow(qsTr("Store"), "/vac/waku/store/3.0.0", "messaging.store_peers"),
        page.protocolRow(qsTr("Filter"), "/vac/waku/filter/2.0.0-beta1", "messaging.filter_peers"),
        page.protocolRow(qsTr("Lightpush"), "/vac/waku/lightpush/2.0.0-beta1", "messaging.lightpush_peers"),
        page.protocolRow(qsTr("Peer exchange"), "/vac/waku/peer-exchange/2.0.0-alpha1", ""),
        page.protocolRow(qsTr("Metadata"), "/vac/waku/metadata/1.0.0", "Version"),
        page.protocolRow(qsTr("Discv5"), "discv5", ""),
        page.protocolRow(qsTr("RLN relay"), "/vac/waku/rln-relay/2.0.0", "")
    ]
}

function deliveryProtocolHealthRows(page) {
    const value = page.probeValue("protocolsHealth")
    if (!value || typeof value !== "object") {
        return []
    }
    const rows = []
    if (Array.isArray(value)) {
        for (let i = 0; i < value.length; ++i) {
            const item = page.protocolHealthEntry(value[i])
            if (item) {
                rows.push(page.statusRow(page.protocolLabel(item.protocol), page.valueSummary(item.health), item.detail, page.healthValueTone(item.health)))
            }
        }
        return rows
    }
    if (value.protocol !== undefined || value.name !== undefined || value.health !== undefined || value.status !== undefined) {
        const single = page.protocolHealthEntry(value)
        rows.push(page.statusRow(page.protocolLabel(single.protocol), page.valueSummary(single.health), single.detail, page.healthValueTone(single.health)))
        return rows
    }
    const keys = Object.keys(value).sort()
    for (let i = 0; i < keys.length; ++i) {
        const key = keys[i]
        if (key === "desc" || key === "description") {
            continue
        }
        const state = value[key]
        rows.push(page.statusRow(page.protocolLabel(key), page.valueSummary(state), key, page.healthValueTone(state)))
    }
    return rows
}

function deliveryProtocolHealthEntry(page, item) {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
        return null
    }
    const explicitProtocol = item.protocol !== undefined ? item.protocol : item.name
    const explicitHealth = item.health !== undefined ? item.health : item.status
    if (explicitProtocol !== undefined || explicitHealth !== undefined) {
        const protocol = explicitProtocol !== undefined ? explicitProtocol : qsTr("Protocol")
        return {
            protocol: protocol,
            health: explicitHealth,
            detail: page.protocolHealthDetail(protocol, item.desc !== undefined ? item.desc : item.description)
        }
    }
    const keys = Object.keys(item).filter(key => key !== "desc" && key !== "description").sort()
    if (!keys.length) {
        return null
    }
    const protocolKey = keys[0]
    return {
        protocol: protocolKey,
        health: item[protocolKey],
        detail: page.protocolHealthDetail(protocolKey, item.desc !== undefined ? item.desc : item.description)
    }
}

function deliveryProtocolHealthDetail(page, protocol, description) {
    const detail = page.valueSummary(description)
    if (!detail.length || detail === qsTr("unknown")) {
        return String(protocol || "")
    }
    return "%1: %2".arg(String(protocol || "")).arg(detail)
}

function deliveryProtocolLabel(key) {
    const text = String(key || "")
    const normalized = text.toLowerCase()
    if (normalized.indexOf("lightpush") >= 0) {
        return qsTr("Lightpush")
    }
    if (normalized.indexOf("filter") >= 0) {
        return qsTr("Filter")
    }
    if (normalized.indexOf("store") >= 0) {
        return qsTr("Store")
    }
    if (normalized.indexOf("relay") >= 0) {
        return qsTr("Relay")
    }
    if (normalized.indexOf("metadata") >= 0) {
        return qsTr("Metadata")
    }
    if (normalized.indexOf("peer") >= 0) {
        return qsTr("Peer exchange")
    }
    return text.length ? text : qsTr("Protocol")
}

function deliveryHealthValueTone(page, value) {
    if (value === undefined || value === null) {
        return "neutral"
    }
    return page.model.metrics.deliveryHealthValueOk(value, false) ? "success" : "error"
}

function deliveryCombinedHealthTone(page, left, right) {
    const leftTone = page.healthValueTone(left)
    const rightTone = page.healthValueTone(right)
    if (leftTone === "error" || rightTone === "error") {
        return "error"
    }
    if (leftTone === "success" || rightTone === "success") {
        return "success"
    }
    return "neutral"
}

function deliveryTopicRows(page) {
    const topics = page.networkMonitorTopicCount()
    return [
        page.metricRow(qsTr("Pubsub peers"), "messaging.pubsub_peers"),
        page.metricRow(qsTr("Content topics"), "messaging.content_topics"),
        page.statusRow(qsTr("Topic-to-shard mapping"), topics !== null ? qsTr("observed") : qsTr("unknown"), topics !== null ? qsTr("%1 content topic(s)").arg(topics) : qsTr("Requires topic metadata or Delivery Network Monitor source."), topics !== null ? "success" : "neutral"),
        page.metricRow(qsTr("Store query pressure"), "messaging.store_query_requests_recent"),
        page.metricRow(qsTr("Filter query pressure"), "messaging.filter_requests_recent")
    ]
}

function deliveryStoreRows(page) {
    return [
        page.protocolStatusRow(qsTr("Store mounted state"), "Store", "messaging.store_peers"),
        page.metricRow(qsTr("Store peers"), "messaging.store_peers"),
        page.metricRow(qsTr("Stored messages"), "messaging.store_messages"),
        page.metricRow(qsTr("Store query rate"), "messaging.store_query_requests_recent"),
        page.metricRow(qsTr("Store errors"), "messaging.store_errors_recent"),
        page.statusRow(qsTr("Manual query"), qsTr("available"), qsTr("Network / Delivery Store tab uses includeData=false by default."), "success"),
        page.statusRow(qsTr("Payload viewing"), qsTr("disabled"), qsTr("Payload bytes stay hidden unless a future query opts in."), "success")
    ]
}

function deliveryIdentityRows(page) {
    return [
        page.detailRow(qsTr("Peer ID"), page.identityValue("peerId")),
        page.detailRow(qsTr("ENR"), page.identityValue("enrUri")),
        page.detailRow(qsTr("Multiaddresses"), page.identityValue("listenAddresses")),
        page.detailRow(qsTr("Protocol health"), page.probeValue("protocolsHealth")),
        page.detailRow(qsTr("Version"), page.probeValue("Version") || page.probeValue("version"))
    ]
}

function deliveryIdentityValue(page, kind) {
    switch (kind) {
    case "peerId":
        return page.probeValue("peerId") || page.probeValue("MyPeerId")
    case "enrUri":
        return page.probeValue("enrUri") || page.probeValue("MyENR")
    case "listenAddresses":
        return page.probeValue("listenAddresses") || page.probeValue("MyMultiaddresses")
    default:
        return null
    }
}

function deliveryMetricRow(page, label, key) {
    const known = page.metricKnown(key)
    return page.statusRow(label, known ? page.metricDisplay(key) : qsTr("n/a"), known ? page.metricEvidence(key) : qsTr("Metric not exposed by current source."), known ? "success" : "neutral")
}

function deliveryMetricEvidence(page, key) {
    return page.model.metrics.dashboardMetricUsesWindow(key)
        ? qsTr("%1 s window").arg(page.rollingWindow())
        : qsTr("OpenMetrics value")
}

function deliveryProtocolStatusRow(page, label, protocol, metricKey) {
    const rows = page.protocolHealthRows()
    const needle = String(protocol || "").toLowerCase()
    for (let i = 0; i < rows.length; ++i) {
        const row = rows[i]
        if (String(row.label || "").toLowerCase().indexOf(needle) >= 0) {
            return page.statusRow(label, row.state, row.evidence, row.tone)
        }
    }
    return page.statusRow(label, page.metricKnown(metricKey) ? qsTr("observed") : qsTr("unknown"), page.metricKnown(metricKey) ? page.metricDisplay(metricKey) : qsTr("No protocol health or peer metric."), page.metricKnown(metricKey) ? "success" : "neutral")
}

function deliveryProtocolRow(page, label, protocolId, signalKey) {
    let known = false
    let evidence = qsTr("No passive evidence")
    if (signalKey.indexOf("messaging.") === 0) {
        known = page.metricKnown(signalKey)
        evidence = page.metricDisplay(signalKey)
    } else if (signalKey.length > 0) {
        known = page.probeValue(signalKey) !== null
        evidence = page.valueSummary(page.probeValue(signalKey))
    }
    const row = page.statusRow(label, known ? qsTr("observed") : qsTr("unknown"), evidence, known ? "success" : "neutral")
    row.protocolId = protocolId
    return row
}

function deliveryRestMetricsState(page) {
    const sourceMode = page.deliverySourceMode()
    const metricsKnown = page.sourceFactAvailable("metrics")
    if (sourceMode === "module") {
        return metricsKnown || page.moduleMetricsText().length > 0 ? qsTr("metrics") : qsTr("module")
    }
    if (sourceMode === "rest") {
        return metricsKnown ? qsTr("REST + metrics") : (page.status().ok ? qsTr("reachable") : qsTr("unknown"))
    }
    if (sourceMode === "metrics") {
        return page.status().ok ? qsTr("scraping") : qsTr("unknown")
    }
    if (sourceMode === "network-monitor") {
        return metricsKnown ? qsTr("monitor + metrics") : (page.status().ok ? qsTr("monitor") : qsTr("unknown"))
    }
    return qsTr("pending")
}

function deliveryRestMetricsEvidence(page) {
    const sourceMode = page.deliverySourceMode()
    const metricsEvidence = page.sourceFactEvidence("metrics", "")
    if (sourceMode === "module") {
        return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : qsTr("Module API")
    }
    if (sourceMode === "metrics") {
        return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : page.shortText(page.sourceMetricsEndpoint(), 48)
    }
    if (sourceMode === "network-monitor") {
        return qsTr("%1; metrics %2")
            .arg(page.shortText(page.sourceTarget(), 24))
            .arg(page.shortText(page.sourceMetricsEndpoint(), 24))
    }
    return page.shortText(page.sourceRestEndpoint(), 48)
}

function deliveryModuleMetricsText(page) {
    const value = page.probeValue("collectOpenMetricsText")
    return typeof value === "string" ? value.trim() : ""
}

function deliveryRestMetricsTone(page) {
    const sourceMode = page.deliverySourceMode()
    if (sourceMode === "unsupported") {
        return "warning"
    }
    if (page.model.metrics.sourceCapabilityAvailable(page.report(), "metrics") === false
            && (sourceMode === "metrics" || sourceMode === "network-monitor")) {
        return "warning"
    }
    return page.statusTone()
}

function deliveryNetworkMonitorPeerCount(page) {
    return deliveryCountValue(page, page.probeValue("allPeersInfo"))
}

function deliveryNetworkMonitorTopicCount(page) {
    const value = page.probeValue("contentTopics")
    const count = deliveryCountValue(page, value)
    if (count !== null) {
        return count
    }
    const metric = page.model.metrics.dashboardMetricValue("messaging.content_topics")
    return metric === null || metric === undefined ? null : Number(metric)
}

function deliveryServicePeerCount(page) {
    let total = 0
    let found = false
    const keys = ["messaging.store_peers", "messaging.filter_peers", "messaging.lightpush_peers"]
    for (let i = 0; i < keys.length; ++i) {
        const value = Number(page.model.metrics.dashboardMetricValue(keys[i]))
        if (Number.isFinite(value)) {
            total += value
            found = true
        }
    }
    return found ? total : null
}

function deliveryCountValue(page, value) {
    return UiFormat.countValue(value, {
        scalarValue: page.model.metrics.scalarValue,
        nestedKeys: ["peers", "allPeers", "all_peers", "contentTopics", "content_topics", "topics", "items", "value", "result"]
    })
}
