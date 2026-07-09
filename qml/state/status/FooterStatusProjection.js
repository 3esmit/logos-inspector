.import "../../components/status/FooterFieldGroups.js" as FooterFieldGroups

function footerGroups(root, region) {
    const revision = root.model.footerFieldRevision
    const groups = footerSourceGroups()
    const rows = []
    for (let i = 0; i < groups.length; ++i) {
        const group = groups[i]
        const alignRight = group.alignRight === true
        if ((String(region || "left") === "right") !== alignRight) {
            continue
        }
        const items = footerGroupItems(root, group)
        if (items.length > 0) {
            rows.push({
                first: rows.length === 0,
                items: items
            })
        }
    }
    return rows
}

function footerGroupItems(root, group) {
    const rows = []
    const keys = group.keys || []
    const statusKey = String(group.statusKey || "")
    if (statusKey.length > 0 && root.model.footerFieldEnabled(statusKey) && footerGroupVisible(root, keys)) {
        const statusItem = footerFieldItem(root, statusKey)
        if (!statusItem.hidden) {
            rows.push(statusItem)
        }
    }
    for (let i = 0; i < keys.length; ++i) {
        const key = keys[i]
        if (key !== statusKey && root.model.footerFieldEnabled(key)) {
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
        if (root.model.footerFieldEnabled(keys[i]) && !footerFieldHidden(root, keys[i])) {
            return true
        }
    }
    return false
}

function footerSourceGroups() {
    return FooterFieldGroups.sourceGroups()
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
    const lookup = {
        "network.network": qsTr("Net"),
        "network.chain_id": qsTr("Chain"),
        "network.zone_id": qsTr("Zone"),
        "network.channel_id": qsTr("Chan"),
        "network.report_time": qsTr("Report"),
        "bedrock.node_health": qsTr("Bedrock"),
        "bedrock.peer_count": qsTr("Peers"),
        "bedrock.sync_state": qsTr("Sync"),
        "bedrock.tip_height": qsTr("TIP"),
        "bedrock.tip_hash": qsTr("Tip hash"),
        "bedrock.lib_height": qsTr("LIB"),
        "bedrock.lib_hash": qsTr("LIB hash"),
        "bedrock.tip_minus_lib": qsTr("Gap"),
        "bedrock.last_tip_time": qsTr("Tip time"),
        "bedrock.last_lib_time": qsTr("LIB time"),
        "bedrock.finality_lag_seconds": qsTr("Final lag"),
        "lez.rpc_health": qsTr("LEZ"),
        "lez.sequencer_version": qsTr("Seq ver"),
        "lez.last_lez_block_id": qsTr("LEZ block"),
        "lez.last_lez_block_hash": qsTr("LEZ hash"),
        "lez.last_lez_block_time": qsTr("LEZ time"),
        "lez.pending_tx_count": qsTr("Pending"),
        "lez.mempool_tx_count": qsTr("Mempool"),
        "lez.rejected_tx_count_recent": qsTr("Rejects"),
        "lez.blocks_produced_recent": qsTr("Blocks"),
        "lez.publish_to_bedrock_status": qsTr("Publish"),
        "lez.last_published_channel_update": qsTr("Channel"),
        "lez.last_finalized_callback_height": qsTr("Final"),
        "lez.pending_blocks_count": qsTr("Blk pend"),
        "indexer.rpc_health": qsTr("Indexer"),
        "indexer.indexer_version": qsTr("Idx ver"),
        "indexer.indexed_finalized_height": qsTr("Idx final"),
        "indexer.indexed_finalized_hash": qsTr("Idx hash"),
        "indexer.indexed_channel_message": qsTr("Idx chan"),
        "indexer.indexer_lag_vs_sequencer_head": qsTr("Idx lag"),
        "indexer.last_indexed_time": qsTr("Idx time"),
        "indexer.db_health": qsTr("DB"),
        "indexer.ingestion_status": qsTr("Ingest"),
        "storage.module": qsTr("Storage"),
        "storage.network": qsTr("ST net"),
        "storage.node_reachable": qsTr("ST node"),
        "storage.nat_mode": qsTr("NAT"),
        "storage.udp_discovery_port": qsTr("UDP"),
        "storage.tcp_transfer_port": qsTr("TCP"),
        "storage.peer_count": qsTr("ST peers"),
        "storage.dht_connected": qsTr("DHT"),
        "storage.shared_files_count": qsTr("Files"),
        "storage.manifest_count": qsTr("Manifests"),
        "storage.local_storage_used": qsTr("Storage"),
        "storage.active_uploads": qsTr("Uploads total"),
        "storage.active_downloads": qsTr("Downloads total"),
        "storage.failed_transfers_recent": qsTr("Failures win"),
        "storage.failed_transfers_total": qsTr("Failures total"),
        "storage.cid_fetch_test": qsTr("CID"),
        "storage.last_error": qsTr("ST error"),
        "messaging.module": qsTr("Delivery"),
        "messaging.connection_state": qsTr("Conn"),
        "messaging.bootstrap_connected": qsTr("Bootstrap"),
        "messaging.peer_count": qsTr("DLV peers"),
        "messaging.active_subscriptions": qsTr("Subs"),
        "messaging.content_topics": qsTr("Topics"),
        "messaging.outbound_queue": qsTr("Queue"),
        "messaging.message_sent_events_recent": qsTr("Sent n/a"),
        "messaging.message_propagated_events_recent": qsTr("Prop n/a"),
        "messaging.message_received_events_recent": qsTr("Msgs win"),
        "messaging.message_error_events_recent": qsTr("Errors win"),
        "messaging.publish_latency_ms": qsTr("Pub n/a"),
        "messaging.receive_latency_ms": qsTr("Recv n/a"),
        "messaging.last_error": qsTr("Delivery error"),
        "overall.status": qsTr("Overall"),
        "overall.main_risk": qsTr("Risk"),
        "overall.operator_action": qsTr("Action")
    }
    return lookup[key] || key
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
        return qsTr("n/a")
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
        return root.valueOrNa(root.model.dashboardMetricValue(key))
    case "lez.publish_to_bedrock_status":
        return root.valueOrNa(root.latestSequencerBlockValue("bedrock_status"))
    case "lez.last_published_channel_update":
        return qsTr("n/a")
    case "lez.last_finalized_callback_height":
        return root.valueOrNa(root.model.indexerHeadValue())
    case "indexer.rpc_health":
        return root.indexerDisplayStatus()
    case "indexer.indexer_version":
        return qsTr("n/a")
    case "indexer.indexed_finalized_height":
        return root.valueOrNa(root.model.indexerHeadValue())
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
        return root.model.storageNetworkPreset || root.model.storageSourceTarget()
    case "storage.node_reachable":
        return root.connectionReachableStatus("storage")
    case "storage.nat_mode":
        return root.valueOrNa(root.model.openMetricValue("storage", ["storage_nat_mode", "nat_mode"]))
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
        return root.valueOrNa(root.model.dashboardMetricValue(key))
    case "storage.dht_connected":
        return root.yesNo(root.model.openMetricValue("storage", ["storage_dht_connected", "dht_connected"]))
    case "storage.cid_fetch_test":
        return root.valueOrNa(root.model.reportProbeValue(root.model.moduleReport("storage"), "exists"))
    case "storage.last_error":
        return root.valueOrNa(root.model.moduleLastError("storage"))
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
    case "messaging.publish_latency_ms":
    case "messaging.receive_latency_ms":
        return root.valueOrNa(root.model.dashboardMetricValue(key))
    case "messaging.bootstrap_connected":
        return qsTr("n/a")
    case "messaging.last_error":
        return root.valueOrNa(root.model.moduleLastError("messaging"))
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

function footerFieldWidth(key) {
    if (key.indexOf("hash") >= 0 || key.indexOf("error") >= 0 || key === "overall.operator_action" || key === "overall.main_risk") {
        return 190
    }
    return 150
}

function footerFieldPriority(key) {
    return key.indexOf("_hash") >= 0
            || key.indexOf("_time") >= 0
            || key.indexOf("version") >= 0
            || key.indexOf("last_error") >= 0
            || key === "network.report_time"
            || key === "overall.operator_action" ? "low" : "normal"
}

function footerFieldUsesColorOnly(key) {
    const lookup = {
        "bedrock.node_health": true,
        "bedrock.sync_state": true,
        "lez.rpc_health": true,
        "lez.publish_to_bedrock_status": true,
        "indexer.rpc_health": true,
        "indexer.db_health": true,
        "indexer.ingestion_status": true,
        "storage.module": true,
        "storage.node_reachable": true,
        "storage.udp_discovery_port": true,
        "storage.tcp_transfer_port": true,
        "storage.dht_connected": true,
        "storage.cid_fetch_test": true,
        "messaging.module": true,
        "messaging.connection_state": true,
        "overall.status": true
    }
    return lookup[key] === true
}

function footerFieldShowsDot(key) {
    return key === "network.network" || footerFieldUsesColorOnly(key)
}

function footerFieldHidden(root, key) {
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
    if (root.toneForProbe("node", "consensus") === "error"
            || root.toneForProbe("sequencer", "health") === "error"
            || root.indexerStatusTone() === "error"
            || root.moduleTone("storage") === "error"
            || root.moduleTone("messaging") === "error") {
        return "error"
    }
    if (root.toneForProbe("node", "consensus") === "warning"
            || root.toneForProbe("sequencer", "health") === "warning"
            || root.indexerStatusTone() === "warning"
            || root.moduleTone("storage") === "warning"
            || root.moduleTone("messaging") === "warning") {
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
    if (root.toneForProbe("sequencer", "health") === "error") {
        return qsTr("lez rpc")
    }
    if (root.indexerStatusTone() === "error" || root.indexerStatusTone() === "warning") {
        return qsTr("indexer")
    }
    if (root.moduleTone("storage") === "error") {
        return qsTr("storage")
    }
    if (root.moduleTone("messaging") === "error") {
        return qsTr("messaging")
    }
    return qsTr("none")
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
    return qsTr("check rpc")
}
