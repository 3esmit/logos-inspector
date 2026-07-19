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

function metricUnavailableEvidence(page, key) {
    const metrics = page.model.metrics
    if (key === "messaging.message_sent_events_recent"
            || key === "messaging.message_propagated_events_recent") {
        if (typeof metrics.deliveryModuleEventMetricUnavailableReason
                === "function") {
            return metrics.deliveryModuleEventMetricUnavailableReason(key)
        }
        const status = String(metrics.deliveryModuleEventStreamStatus || "unknown")
        const reason = String(metrics.deliveryModuleEventStreamReason || "").trim()
        if (status === "unavailable") {
            return reason.length > 0
                ? qsTr("Delivery event watcher unavailable: %1").arg(reason)
                : qsTr("Delivery event watcher unavailable.")
        }
        return qsTr("Waiting for Delivery event watcher readiness.")
    }
    const raw = metrics.dashboardMetricRawValue(key)
    if (metrics.dashboardMetricUsesWindow(key)
            && raw !== null && raw !== undefined) {
        return qsTr("Waiting for another source observation.")
    }
    return qsTr("Metric not exposed by current source.")
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
        page.statusRow(qsTr("Recent transfer failures"), page.metricKnown("storage.failed_transfers_recent") ? page.metricDisplay("storage.failed_transfers_recent") : qsTr("unknown"), page.metricKnown("storage.failed_transfers_recent") ? qsTr("%1 s window").arg(page.rollingWindow()) : metricUnavailableEvidence(page, "storage.failed_transfers_recent"), page.metricKnown("storage.failed_transfers_recent") ? (Number(page.model.metrics.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? "error" : "success") : "neutral"),
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
    const reportCid = page.reportStorageCid().trim()
    const exists = reportCid === cid ? page.probe("exists") : null
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
    return page.statusRow(label, known ? page.metricDisplay(key) : qsTr("n/a"), known ? page.metricEvidence(key) : metricUnavailableEvidence(page, key), tone)
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
    const discovered = page.networkMonitorPeerCount()
    const discoveryKnown = discovered !== null
    return [
        page.statusRow(qsTr("Source and lifecycle"), status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"), status.detail || qsTr("Not queried"), page.statusTone()),
        page.statusRow(qsTr("Identity"), page.sourceFactAvailable("identity") || identity !== null ? qsTr("present") : qsTr("unknown"), page.identityEvidence(), page.sourceFactAvailable("identity") || identity !== null ? "success" : "neutral"),
        page.statusRow(qsTr("Node health"), nodeHealth !== null ? page.valueSummary(nodeHealth) : qsTr("unknown"), page.valueSummary(connectionStatus), nodeTone),
        page.statusRow(qsTr("Preset, cluster, shards"), page.sourceNetworkPreset().length ? qsTr("configured") : qsTr("unknown"), page.sourceNetworkPreset() || qsTr("No preset"), page.sourceNetworkPreset().length ? "success" : "neutral"),
        page.statusRow(qsTr("REST and metrics access"), page.restMetricsState(), page.restMetricsEvidence(), page.restMetricsTone()),
        deliveryHealthProtocolRow(page, qsTr("Relay"), "Relay", "messaging.pubsub_peers", "relay", qsTr("No relay fact.")),
        deliveryHealthProtocolRow(page, qsTr("Store"), "Store", "messaging.store_peers", "store", qsTr("No Store fact.")),
        deliveryHealthProtocolRow(page, qsTr("Filter"), "Filter", "messaging.filter_peers", "filter", qsTr("No Filter fact.")),
        deliveryHealthProtocolRow(page, qsTr("Lightpush"), "Lightpush", "messaging.lightpush_peers", "lightpush", qsTr("No Lightpush fact.")),
        deliveryHealthProtocolRow(page, qsTr("Discovery"), "Rendezvous", "", "network_monitor", qsTr("No Delivery Network Monitor peer snapshot."), discoveryKnown, discoveryKnown ? qsTr("%1 peer(s)").arg(discovered) : ""),
        deliveryHealthProtocolRow(page, qsTr("RLN / spam protection"), "Rln Relay", "", "", qsTr("No passive metric selected"))
    ]
}

function deliveryHealthProtocolRow(page, label, protocolName, metricKey, factKey, fallbackEvidence, fallbackKnown, fallbackObservedEvidence) {
    const protocolRow = deliveryExactProtocolHealthRow(page, protocolName)
    if (protocolRow !== null) {
        return page.statusRow(
            label,
            protocolRow.state,
            deliveryHealthProtocolEvidence(page, protocolRow, metricKey),
            protocolRow.tone)
    }
    const metricKnown = metricKey.length > 0 && page.metricKnown(metricKey)
    const factKnown = factKey.length > 0 && page.sourceFactAvailable(factKey)
    const observedKnown = fallbackKnown === true
    const known = metricKnown || observedKnown || factKnown
    const evidence = metricKnown
        ? page.metricDisplay(metricKey)
        : (observedKnown ? fallbackObservedEvidence
            : (factKey.length > 0 ? page.sourceFactEvidence(factKey, fallbackEvidence) : fallbackEvidence))
    return page.statusRow(label, known ? qsTr("observed") : qsTr("unknown"), evidence, known ? "success" : "neutral")
}

function deliveryExactProtocolHealthRow(page, protocolName) {
    const wanted = normalizedDeliveryProtocolName(protocolName)
    const rows = page.protocolHealthRows()
    for (let i = 0; i < rows.length; ++i) {
        if (normalizedDeliveryProtocolName(rows[i].protocolName) === wanted) {
            return rows[i]
        }
    }
    return null
}

function deliveryHealthProtocolEvidence(page, row, metricKey) {
    const values = []
    if (metricKey.length > 0 && page.metricKnown(metricKey)) {
        values.push(qsTr("%1 peer(s)").arg(page.metricDisplay(metricKey)))
    }
    const description = String(row.protocolDescription || "").trim()
    if (description.length > 0) {
        values.push(description)
    }
    if (values.length === 0) {
        values.push(String(row.protocolName || row.evidence || qsTr("No passive evidence")))
    }
    return values.join("; ")
}

function normalizedDeliveryProtocolName(value) {
    return String(value || "").trim().toLowerCase().replace(/[^a-z0-9]+/g, "")
}

function deliveryTopologyRows(page) {
    const discovered = page.networkMonitorPeerCount()
    const topics = page.networkMonitorTopicCount()
    const servicePeers = page.servicePeerCount()
    return [
        page.statusRow(qsTr("Local connected peers"), page.metricKnown("messaging.peer_count") ? qsTr("observed") : qsTr("unknown"), page.metricDisplay("messaging.peer_count"), page.metricKnown("messaging.peer_count") ? "success" : "neutral"),
        page.statusRow(qsTr("Pubsub peer instances"), page.metricKnown("messaging.pubsub_peers") ? qsTr("observed") : qsTr("unknown"), page.metricDisplay("messaging.pubsub_peers"), page.metricKnown("messaging.pubsub_peers") ? "success" : "neutral"),
        page.statusRow(qsTr("Discovery peers"), discovered !== null ? qsTr("observed") : qsTr("unknown"), discovered !== null ? qsTr("%1 peer(s)").arg(discovered) : qsTr("No Delivery Network Monitor peer snapshot."), discovered !== null ? "success" : "neutral"),
        page.statusRow(qsTr("Service peer instances"), servicePeers !== null ? qsTr("observed") : qsTr("unknown"), servicePeers !== null ? qsTr("%1 combined Store/Filter/Lightpush peer instance(s)").arg(servicePeers) : qsTr("Complete Store/Filter/Lightpush peer metrics unavailable."), servicePeers !== null ? "success" : "neutral"),
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
        page.metricRow(qsTr("Confirmed sends"), "messaging.message_sent_events_recent"),
        page.metricRow(qsTr("Network propagations"), "messaging.message_propagated_events_recent"),
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
                rows.push(deliveryProtocolHealthStatusRow(page, item))
            }
        }
        return rows
    }
    if (value.protocol !== undefined || value.name !== undefined || value.health !== undefined || value.status !== undefined) {
        const single = page.protocolHealthEntry(value)
        rows.push(deliveryProtocolHealthStatusRow(page, single))
        return rows
    }
    const keys = Object.keys(value).sort()
    for (let i = 0; i < keys.length; ++i) {
        const key = keys[i]
        if (key === "desc" || key === "description") {
            continue
        }
        const state = value[key]
        rows.push(deliveryProtocolHealthStatusRow(page, {
            protocol: key,
            health: state,
            description: null,
            detail: key
        }))
    }
    return rows
}

function deliveryProtocolHealthStatusRow(page, item) {
    const row = page.statusRow(
        page.protocolLabel(item.protocol),
        page.valueSummary(item.health),
        item.detail,
        page.healthValueTone(item.health))
    row.protocolName = String(item.protocol || "")
    row.protocolDescription = item.description === undefined || item.description === null
        ? "" : String(item.description)
    return row
}

function deliveryProtocolHealthEntry(page, item) {
    if (!item || typeof item !== "object" || Array.isArray(item)) {
        return null
    }
    const explicitProtocol = item.protocol !== undefined ? item.protocol : item.name
    const explicitHealth = item.health !== undefined ? item.health : item.status
    if (explicitProtocol !== undefined || explicitHealth !== undefined) {
        const protocol = explicitProtocol !== undefined ? explicitProtocol : qsTr("Protocol")
        const description = item.desc !== undefined ? item.desc : item.description
        return {
            protocol: protocol,
            health: explicitHealth,
            description: description,
            detail: page.protocolHealthDetail(protocol, description)
        }
    }
    const keys = Object.keys(item).filter(key => key !== "desc" && key !== "description").sort()
    if (!keys.length) {
        return null
    }
    const protocolKey = keys[0]
    const description = item.desc !== undefined ? item.desc : item.description
    return {
        protocol: protocolKey,
        health: item[protocolKey],
        description: description,
        detail: page.protocolHealthDetail(protocolKey, description)
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
    const normalized = String(value).trim().toLowerCase().replace(/[^a-z0-9]+/g, "")
    if (normalized === "notmounted" || normalized === "disabled") {
        return "neutral"
    }
    if (normalized === "notready" || normalized === "initializing" || normalized === "synchronizing") {
        return "warning"
    }
    return page.model.metrics.deliveryHealthValueOk(value, false) ? "success" : "error"
}

function deliveryCombinedHealthTone(page, left, right) {
    const leftTone = page.healthValueTone(left)
    const rightTone = page.healthValueTone(right)
    if (leftTone === "error" || rightTone === "error") {
        return "error"
    }
    if (leftTone === "warning" || rightTone === "warning") {
        return "warning"
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
        page.metricRow(qsTr("Store queries in window"), "messaging.store_query_requests_recent"),
        page.metricRow(qsTr("Filter requests in window"), "messaging.filter_requests_recent")
    ]
}

function deliveryStoreRows(page) {
    const queryAvailable = deliverySourceCapabilityAvailable(
        page, "delivery.store.query")
    const sourceName = String(page.sourceName() || qsTr("Current Delivery source"))
    return [
        page.protocolStatusRow(qsTr("Store mounted state"), "Store", "messaging.store_peers"),
        page.metricRow(qsTr("Store peers"), "messaging.store_peers"),
        page.metricRow(qsTr("Stored messages"), "messaging.store_messages"),
        page.metricRow(qsTr("Store queries in window"), "messaging.store_query_requests_recent"),
        page.metricRow(qsTr("Store/archive errors in window"), "messaging.store_errors_recent"),
        page.statusRow(
            qsTr("Manual query"),
            queryAvailable ? qsTr("available") : qsTr("unavailable"),
            queryAvailable
                ? qsTr("Network / Delivery Store uses Direct Waku REST. Payloads are excluded by default.")
                : qsTr("%1 does not expose Store queries. Choose Direct Waku REST in Delivery settings.").arg(sourceName),
            queryAvailable ? "success" : "neutral"),
        page.statusRow(
            qsTr("Payload viewing"),
            queryAvailable ? qsTr("opt-in") : qsTr("unavailable"),
            queryAvailable
                ? qsTr("Enable Include payloads for one Store query.")
                : qsTr("Payload viewing requires a Store-query-capable source."),
            "neutral")
    ]
}

function deliverySourceCapabilityAvailable(page, key) {
    const route = page && typeof page.sourceRoute === "function"
        ? page.sourceRoute() : null
    const capabilities = route && Array.isArray(route.capabilities)
        ? route.capabilities : []
    const expected = String(key || "")
    for (let index = 0; index < capabilities.length; ++index) {
        if (String(capabilities[index] || "") === expected) {
            return true
        }
    }
    return false
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
    return page.statusRow(label, known ? page.metricDisplay(key) : qsTr("n/a"), known ? page.metricEvidence(key) : metricUnavailableEvidence(page, key), known ? "success" : "neutral")
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
    const keys = ["messaging.store_peers", "messaging.filter_peers", "messaging.lightpush_peers"]
    for (let i = 0; i < keys.length; ++i) {
        const rawValue = page.model.metrics.dashboardMetricValue(keys[i])
        if (rawValue === null || rawValue === undefined) {
            return null
        }
        const value = Number(rawValue)
        if (Number.isFinite(value)) {
            total += value
        } else {
            return null
        }
    }
    return total
}

function deliveryCountValue(page, value) {
    return UiFormat.countValue(value, {
        scalarValue: page.model.metrics.scalarValue,
        nestedKeys: ["peers", "allPeers", "all_peers", "contentTopics", "content_topics", "topics", "items", "value", "result"]
    })
}
