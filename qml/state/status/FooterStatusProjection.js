.import "StatusFieldCatalog.js" as StatusFieldCatalog

function footerGroups(root, region) {
    const regions = footerRegions(root)
    return String(region || "left") === "right" ? regions.right : regions.left
}

function footerRegions(root) {
    const revision = root.model.metrics.footerFieldRevision
    const groups = footerSourceGroups()
    const regions = { left: [], right: [] }
    for (let i = 0; i < groups.length; ++i) {
        const group = groups[i]
        if (String(group.dynamic || "") === "channels") {
            const channelGroups = channelFooterGroups(root)
            for (let j = 0; j < channelGroups.length; ++j) {
                regions.left.push({
                    first: regions.left.length === 0,
                    items: channelGroups[j].items
                })
            }
            continue
        }
        const alignRight = group.alignRight === true
        const items = footerGroupItems(root, group)
        if (items.length > 0) {
            const rows = alignRight ? regions.right : regions.left
            rows.push({
                first: rows.length === 0,
                items: items
            })
        }
    }
    return regions
}

function footerGroupItems(root, group) {
    const rows = []
    const keys = group.keys || []
    const statusKey = String(group.statusKey || "")
    if (statusKey.length > 0 && root.model.metrics.footerFieldEnabled(statusKey) && footerGroupVisible(root, keys)) {
        const statusItem = footerFieldItem(root, statusKey)
        if (!statusItem.hidden) {
            rows.push(statusItem)
        }
    }
    for (let i = 0; i < keys.length; ++i) {
        const key = keys[i]
        if (key !== statusKey && root.model.metrics.footerFieldEnabled(key)) {
            const item = footerFieldItem(root, key)
            if (!item.hidden) {
                rows.push(item)
            }
        }
    }
    return rows
}

function footerGroupVisible(root, keys) {
    for (let i = 0; i < keys.length; ++i) {
        if (root.model.metrics.footerFieldEnabled(keys[i]) && !footerFieldHidden(root, keys[i])) {
            return true
        }
    }
    return false
}

function footerSourceGroups() {
    return StatusFieldCatalog.footerSourceGroups()
}

function channelFooterGroups(root) {
    if (!root || !root.model || !root.model.metrics) {
        return []
    }
    const statuses = channelStatuses(root)
    return statuses.filter(function (status) {
        return root.model.metrics.footerFieldEnabled(
            StatusFieldCatalog.channelFooterKey(status && status.channel_id))
    }).map(function (status) {
        return { items: channelStatusItems(root, status) }
    })
}

function channelStatuses(root) {
    const rows = root && root.model && Array.isArray(root.model.dashboardChannelStatuses)
        ? root.model.dashboardChannelStatuses : []
    return rows.filter(function (row) {
        const value = row || ({})
        const sequencer = value.sequencer || ({})
        const indexer = value.indexer || ({})
        return sequencer.configured === true || indexer.configured === true
    })
}

function channelStatusItems(root, channel) {
    const value = channel || ({})
    const sequencer = value.sequencer || ({})
    const indexer = value.indexer || ({})
    const channelText = channelShortId(value)
    const channelName = channelDisplayName(value)
    return [{
        label: qsTr("Channel"),
        fullName: qsTr("Channel %1").arg(channelName),
        value: channelText,
        accessibleValue: channelAccessibleValue(value),
        tone: channelTone(value),
        maximumWidth: 176,
        priority: "normal",
        valueVisible: true,
        showDot: true
    }, channelSourceItem(root, value, sequencer, qsTr("Sequencer"), qsTr("Sequencer")),
        channelSourceItem(root, value, indexer, qsTr("Indexer"), qsTr("Indexer"))]
}

function channelSourceItem(root, channel, source, label, sourceName) {
    return {
        label: label,
        fullName: qsTr("%1 %2").arg(channelDisplayName(channel)).arg(sourceName),
        value: channelSourceValue(root, source),
        accessibleValue: channelSourceAccessibleValue(root, source),
        tone: channelSourceTone(source),
        maximumWidth: 176,
        priority: "normal",
        valueVisible: true,
        showDot: true
    }
}

function channelDisplayName(channel) {
    const value = channel || ({})
    const label = String(value.label || "")
    return label.length > 0 ? label : channelShortId(value)
}

function channelShortId(channel) {
    const value = channel || ({})
    const shortId = String(value.short_channel_id || "")
    if (shortId.length > 0) {
        return shortId
    }
    const channelId = String(value.channel_id || "")
    if (channelId.length <= 12) {
        return channelId
    }
    return channelId.slice(0, 6) + "…" + channelId.slice(-6)
}

function channelAccessibleValue(channel) {
    const value = channel || ({})
    const id = String(value.channel_id || "")
    const label = channelDisplayName(value)
    return id.length > 0 && label !== id ? qsTr("%1 (%2)").arg(label).arg(id) : label
}

function channelSourceValue(root, source) {
    const value = source || ({})
    if (value.configured !== true) {
        return qsTr("off")
    }
    const head = value.head
    if (head !== undefined && head !== null && String(head).length > 0) {
        return root.numberText(head)
    }
    const status = channelSourceStatus(value)
    return status === "reachable" ? qsTr("ready") : channelSourceStatusText(status)
}

function channelSourceAccessibleValue(root, source) {
    const value = source || ({})
    if (value.configured !== true) {
        return qsTr("not configured")
    }
    const status = channelSourceStatusText(channelSourceStatus(value))
    const head = value.head
    if (head === undefined || head === null || String(head).length === 0) {
        return status
    }
    return qsTr("%1; head %2").arg(status).arg(root.numberText(head))
}

function channelSourceStatus(source) {
    const value = source || ({})
    const status = String(value.status || "unknown").toLowerCase()
    if (status !== "reachable") {
        return status
    }
    switch (String(value.indexer_state || "").toLowerCase()) {
    case "starting":
    case "syncing":
    case "caught_up":
    case "running":
    case "stopped":
    case "error":
    case "failed":
    case "stalled":
    case "unavailable":
    case "offline":
        return String(value.indexer_state).toLowerCase()
    default:
        return status
    }
}

function channelSourceStatusText(status) {
    switch (String(status || "unknown")) {
    case "reachable":
        return qsTr("reachable")
    case "starting":
        return qsTr("starting")
    case "syncing":
        return qsTr("syncing")
    case "caught_up":
        return qsTr("caught up")
    case "running":
        return qsTr("running")
    case "stopped":
        return qsTr("stopped")
    case "error":
    case "failed":
        return qsTr("error")
    case "stalled":
        return qsTr("stalled")
    case "degraded":
        return qsTr("degraded")
    case "stale":
        return qsTr("stale")
    case "unreachable":
        return qsTr("unreachable")
    case "unconfigured":
        return qsTr("not configured")
    default:
        return qsTr("unknown")
    }
}

function channelSourceTone(source) {
    const value = source || ({})
    if (value.configured !== true) {
        return "neutral"
    }
    switch (channelSourceStatus(value)) {
    case "reachable":
    case "caught_up":
    case "running":
        return "success"
    case "starting":
    case "syncing":
    case "degraded":
    case "stale":
        return "warning"
    case "stopped":
    case "error":
    case "failed":
    case "stalled":
    case "unavailable":
    case "offline":
    case "unreachable":
        return "error"
    default:
        return "neutral"
    }
}

function channelTone(channel) {
    const value = channel || ({})
    const tones = [channelSourceTone(value.sequencer), channelSourceTone(value.indexer)]
    if (tones.indexOf("error") >= 0) {
        return "error"
    }
    if (tones.indexOf("warning") >= 0) {
        return "warning"
    }
    if (tones.indexOf("success") >= 0) {
        return "success"
    }
    return "neutral"
}

function channelSourceFacts(root) {
    const facts = []
    const statuses = channelStatuses(root)
    for (let i = 0; i < statuses.length; ++i) {
        const channel = statuses[i] || ({})
        const sequencer = channel.sequencer || ({})
        const indexer = channel.indexer || ({})
        if (sequencer.configured === true) {
            facts.push({ channel: channel, role: "sequencer", source: sequencer })
        }
        if (indexer.configured === true) {
            facts.push({ channel: channel, role: "indexer", source: indexer })
        }
    }
    return facts
}

function channelFleetTone(root) {
    const facts = channelSourceFacts(root)
    let tone = "neutral"
    for (let i = 0; i < facts.length; ++i) {
        const current = channelSourceTone(facts[i].source)
        if (current === "error") {
            return "error"
        }
        if (current === "warning") {
            tone = "warning"
        } else if (current === "success" && tone === "neutral") {
            tone = "success"
        }
    }
    return tone
}

function footerFieldItem(root, key) {
    return {
        label: footerFieldLabel(key),
        fullName: footerFieldName(key),
        value: footerFieldValue(root, key),
        accessibleValue: footerFieldAccessibleValue(root, key),
        tone: footerFieldTone(root, key),
        maximumWidth: footerFieldWidth(key),
        priority: footerFieldPriority(key),
        valueVisible: !footerFieldUsesColorOnly(key),
        showDot: footerFieldShowsDot(key),
        hidden: footerFieldHidden(root, key)
    }
}

function footerFieldLabel(key) {
    return StatusFieldCatalog.shortLabel(key)
}

function footerFieldName(key) {
    return footerFieldLabel(key)
}

function footerFieldValue(root, key) {
    switch (key) {
    case "network.network":
        return root.networkLabel()
    case "network.chain_id":
        return root.valueOrNa(root.networkValue("chain_id"))
    case "network.zone_id":
        return root.valueOrNa(root.probeValue("sequencer", "zone_id"))
    case "network.channel_id":
        return root.valueOrNa(root.probeValue("sequencer", "channel_id"))
    case "network.report_time":
        return new Date().toLocaleTimeString(Qt.locale(), "hh:mm")
    case "bedrock.node_health":
        return root.healthDisplayText("node", "consensus")
    case "bedrock.peer_count":
        return root.numberText(root.networkValue("n_peers"))
    case "bedrock.sync_state":
        return root.bedrockSyncState()
    case "bedrock.tip_height":
        return root.numberText(root.cryptarchiaValue("slot"))
    case "bedrock.tip_hash":
        return root.shortHash(root.cryptarchiaValue("tip_hash") || root.cryptarchiaValue("hash"))
    case "bedrock.lib_height":
        return root.numberText(root.cryptarchiaValue("lib_slot"))
    case "bedrock.lib_hash":
        return root.shortHash(root.cryptarchiaValue("lib_hash"))
    case "bedrock.tip_minus_lib":
        return root.numberText(root.tipMinusLib())
    case "bedrock.last_tip_time":
    case "bedrock.last_lib_time":
        return root.valueOrNa(sourceReportObservedAt(root, "blockchain"))
    case "bedrock.finality_lag_seconds":
        return root.valueOrNa(root.finalityLagSeconds())
    case "lez.rpc_health":
        return root.healthDisplayText("sequencer", "health")
    case "lez.last_lez_block_id":
        return root.numberText(root.lezBlockHeight())
    case "lez.sequencer_version":
        return qsTr("n/a")
    case "lez.last_lez_block_hash":
        return root.shortHash(root.latestSequencerBlockValue("header_hash"))
    case "lez.last_lez_block_time":
        return root.timeText(root.latestSequencerBlockValue("timestamp"))
    case "lez.pending_tx_count":
    case "lez.mempool_tx_count":
    case "lez.rejected_tx_count_recent":
    case "lez.blocks_produced_recent":
    case "lez.pending_blocks_count":
        return root.valueOrNa(root.model.metrics.dashboardMetricValue(key))
    case "lez.publish_to_bedrock_status":
        return root.valueOrNa(root.latestSequencerBlockValue("bedrock_status"))
    case "lez.last_published_channel_update":
        return qsTr("n/a")
    case "lez.last_finalized_callback_height":
        return root.valueOrNa(root.model.metrics.indexerHeadValue())
    case "indexer.rpc_health":
        return root.indexerDisplayStatus()
    case "indexer.indexer_version":
        return qsTr("n/a")
    case "indexer.indexed_finalized_height":
        return root.valueOrNa(root.model.metrics.indexerHeadValue())
    case "indexer.indexed_finalized_hash":
        return root.shortHash(root.latestIndexerBlockValue("header_hash"))
    case "indexer.indexed_channel_message":
        return qsTr("n/a")
    case "indexer.indexer_lag_vs_sequencer_head":
        return root.valueOrNa(root.indexerLag())
    case "indexer.last_indexed_time":
        return root.timeText(root.latestIndexerBlockValue("timestamp"))
    case "indexer.db_health":
        return root.indexerDisplayStatus()
    case "indexer.ingestion_status":
        return root.indexerStatus()
    case "storage.module":
        return root.moduleDisplayStatus("storage")
    case "storage.network":
        return root.model.storageNetworkPreset || root.model.sourceRouting.storageSourceTarget()
    case "storage.node_reachable":
        return root.connectionReachableStatus("storage")
    case "storage.nat_mode":
        return root.valueOrNa(root.model.metrics.openMetricValue("storage", ["storage_nat_mode", "nat_mode"]))
    case "storage.udp_discovery_port":
        return root.portStatus("storage", ["storage_udp_discovery_port_open", "udp_discovery_port_open"])
    case "storage.tcp_transfer_port":
        return root.portStatus("storage", ["storage_tcp_transfer_port_open", "tcp_transfer_port_open"])
    case "storage.peer_count":
    case "storage.shared_files_count":
    case "storage.manifest_count":
    case "storage.local_storage_used":
    case "storage.active_uploads":
    case "storage.active_downloads":
    case "storage.failed_transfers_recent":
    case "storage.failed_transfers_total":
        return root.valueOrNa(root.model.metrics.dashboardMetricValue(key))
    case "storage.dht_connected":
        return root.yesNo(root.model.metrics.openMetricValue("storage", ["storage_dht_connected", "dht_connected"]))
    case "storage.cid_fetch_test":
        return root.valueOrNa(root.model.metrics.reportProbeValue(
            root.model.metrics.sourceReport("storage"), "exists"))
    case "storage.last_error":
        return root.valueOrNa(configuredSourceError(root, "storage"))
    case "messaging.module":
        return root.moduleDisplayStatus("messaging")
    case "messaging.connection_state":
        return root.connectionAccessibleStatus("messaging")
    case "messaging.peer_count":
    case "messaging.active_subscriptions":
    case "messaging.content_topics":
    case "messaging.outbound_queue":
    case "messaging.message_sent_events_recent":
    case "messaging.message_propagated_events_recent":
    case "messaging.message_received_events_recent":
    case "messaging.message_error_events_recent":
        return root.valueOrNa(root.model.metrics.dashboardMetricValue(key))
    case "messaging.bootstrap_connected":
        return qsTr("n/a")
    case "messaging.last_error":
        return root.valueOrNa(configuredSourceError(root, "messaging"))
    case "overall.status":
        return overallStatusDisplay(root)
    case "overall.main_risk":
        return mainRisk(root)
    case "overall.operator_action":
        return operatorAction(root)
    default:
        return qsTr("n/a")
    }
}

function footerFieldAccessibleValue(root, key) {
    const value = footerFieldValue(root, key)
    if (value.length > 0) {
        return value
    }
    if (key === "bedrock.node_health") {
        return root.healthAccessibleText("node", "consensus")
    }
    if (key === "lez.rpc_health") {
        return root.healthAccessibleText("sequencer", "health")
    }
    if (key === "indexer.rpc_health") {
        return root.indexerStatus()
    }
    if (key === "storage.module") {
        return root.moduleAccessibleStatus("storage")
    }
    if (key === "messaging.module") {
        return root.moduleAccessibleStatus("messaging")
    }
    if (key === "messaging.connection_state") {
        return root.connectionAccessibleStatus("messaging")
    }
    if (key === "overall.status") {
        return overallStatusText(root)
    }
    return value
}

function footerFieldTone(root, key) {
    if (key === "network.network" || key === "network.report_time") {
        return "info"
    }
    if (key === "bedrock.node_health") {
        return root.toneForProbe("node", "consensus")
    }
    if (key === "bedrock.peer_count") {
        return "neutral"
    }
    if (key === "bedrock.sync_state") {
        return root.syncTone()
    }
    if (key === "lez.rpc_health") {
        return root.toneForProbe("sequencer", "health")
    }
    if (key === "lez.publish_to_bedrock_status") {
        return root.statusWordTone(footerFieldValue(root, key))
    }
    if (key === "indexer.rpc_health" || key === "indexer.db_health" || key === "indexer.ingestion_status") {
        return root.indexerStatusTone()
    }
    if (key === "storage.node_reachable" || key === "storage.dht_connected" || key === "storage.cid_fetch_test") {
        return root.booleanTone(footerFieldValue(root, key))
    }
    if (key === "storage.udp_discovery_port" || key === "storage.tcp_transfer_port") {
        return root.portTone(footerFieldValue(root, key))
    }
    if (key === "messaging.connection_state") {
        return root.booleanTone(footerFieldValue(root, key))
    }
    if (key === "messaging.message_error_events_recent") {
        return root.countProblemTone(footerFieldValue(root, key))
    }
    if (key === "storage.failed_transfers_recent") {
        return root.countProblemTone(footerFieldValue(root, key))
    }
    if (key === "storage.last_error" || key === "messaging.last_error") {
        return footerFieldValue(root, key) === qsTr("n/a") ? "neutral" : "error"
    }
    if (key === "overall.status") {
        return overallTone(root)
    }
    if (key === "overall.main_risk" || key === "overall.operator_action") {
        return overallTone(root) === "success" ? "neutral" : overallTone(root)
    }
    return "neutral"
}

function configuredSourceError(root, kind) {
    const observation = root.model.metrics.sourceObservation(kind) || {}
    const attempt = observation.latestAttempt || null
    if (attempt && attempt.transportOk === false && String(attempt.error || "").length > 0) {
        return String(attempt.error)
    }
    const report = observation.sourceReport || root.model.metrics.sourceReport(kind)
    const health = report && report.health && typeof report.health === "object"
        ? report.health : null
    // A ready source may retain failed optional capability probes. Keep those
    // visible in diagnostics without presenting them as a source outage.
    if (health && health.ready === true) {
        return ""
    }
    const reportError = root.model.metrics.moduleReportError(report)
    if (String(reportError || "").length > 0) {
        return String(reportError)
    }
    if (health && health.ready === false) {
        const healthError = String(health.detail || health.summary || "")
        if (healthError.length > 0) {
            return healthError
        }
    }
    const status = observation.status || null
    return status && status.known === true && status.ok !== true
        ? String(status.detail || "") : ""
}

function sourceReportObservedAt(root, kind) {
    const observation = root.model.metrics.sourceObservation(kind) || {}
    return String(observation.reportCheckedAt || "")
}

function footerFieldWidth(key) {
    return StatusFieldCatalog.fieldWidth(key)
}

function footerFieldPriority(key) {
    return StatusFieldCatalog.fieldPriority(key)
}

function footerFieldUsesColorOnly(key) {
    return StatusFieldCatalog.usesColorOnly(key)
}

function footerFieldShowsDot(key) {
    return StatusFieldCatalog.showsDot(key)
}

function footerFieldHidden(root, key) {
    if ((key === "storage.last_error" || key === "messaging.last_error")
            && footerFieldValue(root, key) === qsTr("n/a")) {
        return true
    }
    if (key === "overall.main_risk") {
        return overallTone(root) === "success" && mainRisk(root) === qsTr("none")
    }
    if (key === "overall.operator_action") {
        return overallTone(root) === "success" && operatorAction(root) === qsTr("monitor")
    }
    if (key === "messaging.bootstrap_connected") {
        return true
    }
    return false
}

function overallTone(root) {
    const facts = channelSourceFacts(root)
    const channelToneValue = channelFleetTone(root)
    const legacyZoneStatus = facts.length === 0
    if (root.toneForProbe("node", "consensus") === "error"
            || channelToneValue === "error"
            || (legacyZoneStatus && (root.toneForProbe("sequencer", "health") === "error"
                || root.indexerStatusTone() === "error"))
            || root.moduleTone("storage") === "error"
            || root.moduleTone("messaging") === "error"
            || root.connectionTone("storage") === "error"
            || root.connectionTone("messaging") === "error") {
        return "error"
    }
    if (root.toneForProbe("node", "consensus") === "warning"
            || channelToneValue === "warning"
            || (legacyZoneStatus && (root.toneForProbe("sequencer", "health") === "warning"
                || root.indexerStatusTone() === "warning"))
            || root.moduleTone("storage") === "warning"
            || root.moduleTone("messaging") === "warning"
            || root.connectionTone("storage") === "warning"
            || root.connectionTone("messaging") === "warning") {
        return "warning"
    }
    return "success"
}

function overallStatusText(root) {
    const tone = overallTone(root)
    if (tone === "success") {
        return qsTr("healthy")
    }
    if (tone === "error") {
        return qsTr("down")
    }
    return qsTr("degraded")
}

function overallStatusDisplay(root) {
    const tone = overallTone(root)
    if (tone === "success") {
        return ""
    }
    if (tone === "error") {
        return qsTr("down")
    }
    return qsTr("degraded")
}

function mainRisk(root) {
    if (root.toneForProbe("node", "consensus") === "error") {
        return qsTr("bedrock")
    }
    const channelRisk = channelMainRisk(root)
    if (channelRisk.length > 0) {
        return channelRisk
    }
    if (channelSourceFacts(root).length === 0) {
        if (root.toneForProbe("sequencer", "health") === "error") {
            return qsTr("lez rpc")
        }
        if (root.indexerStatusTone() === "error" || root.indexerStatusTone() === "warning") {
            return qsTr("indexer")
        }
    }
    if (root.moduleTone("storage") === "error"
            || root.connectionTone("storage") === "error") {
        return qsTr("storage")
    }
    if (root.moduleTone("messaging") === "error"
            || root.connectionTone("messaging") === "error") {
        return qsTr("messaging")
    }
    return qsTr("none")
}

function channelMainRisk(root) {
    const facts = channelSourceFacts(root)
    for (let i = 0; i < facts.length; ++i) {
        if (channelSourceTone(facts[i].source) === "error") {
            return channelRiskText(facts[i])
        }
    }
    for (let i = 0; i < facts.length; ++i) {
        if (channelSourceTone(facts[i].source) === "warning") {
            return channelRiskText(facts[i])
        }
    }
    return ""
}

function channelRiskText(fact) {
    const value = fact || ({})
    const sourceName = String(value.role || "source")
    return qsTr("channel %1 %2").arg(channelShortId(value.channel)).arg(sourceName)
}

function operatorAction(root) {
    const risk = mainRisk(root)
    if (risk === qsTr("none")) {
        return qsTr("monitor")
    }
    if (risk === qsTr("indexer")) {
        return qsTr("check indexer")
    }
    if (risk === qsTr("bedrock")) {
        return qsTr("check node")
    }
    if (risk === qsTr("storage")) {
        return qsTr("check storage")
    }
    if (risk === qsTr("messaging")) {
        return qsTr("check messaging")
    }
    if (risk.indexOf(qsTr("channel ")) === 0) {
        return qsTr("check channel source")
    }
    return qsTr("check rpc")
}
