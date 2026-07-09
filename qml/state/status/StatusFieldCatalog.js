function footerSelectorGroups() {
    return [
        { title: qsTr("Network"), fields: fields([
            "network.network",
            "network.chain_id",
            "network.zone_id",
            "network.channel_id",
            "network.report_time"
        ]) },
        { title: qsTr("Bedrock Blockchain"), fields: fields([
            "bedrock.node_health",
            "bedrock.peer_count",
            "bedrock.sync_state",
            "bedrock.tip_height",
            "bedrock.tip_hash",
            "bedrock.lib_height",
            "bedrock.lib_hash",
            "bedrock.tip_minus_lib",
            "bedrock.last_tip_time",
            "bedrock.last_lib_time",
            "bedrock.finality_lag_seconds"
        ]) },
        { title: qsTr("LEZ Sequencer"), fields: fields([
            "lez.rpc_health",
            "lez.sequencer_version",
            "lez.last_lez_block_id",
            "lez.last_lez_block_hash",
            "lez.last_lez_block_time",
            "lez.pending_tx_count",
            "lez.mempool_tx_count",
            "lez.rejected_tx_count_recent",
            "lez.blocks_produced_recent",
            "lez.publish_to_bedrock_status",
            "lez.last_published_channel_update",
            "lez.last_finalized_callback_height",
            "lez.pending_blocks_count"
        ]) },
        { title: qsTr("Indexer"), fields: fields([
            "indexer.rpc_health",
            "indexer.indexer_version",
            "indexer.indexed_finalized_height",
            "indexer.indexed_finalized_hash",
            "indexer.indexed_channel_message",
            "indexer.indexer_lag_vs_sequencer_head",
            "indexer.last_indexed_time",
            "indexer.db_health",
            "indexer.ingestion_status"
        ]) },
        { title: qsTr("Storage"), fields: fields([
            "storage.module",
            "storage.network",
            "storage.node_reachable",
            "storage.nat_mode",
            "storage.udp_discovery_port",
            "storage.tcp_transfer_port",
            "storage.peer_count",
            "storage.dht_connected",
            "storage.shared_files_count",
            "storage.manifest_count",
            "storage.local_storage_used",
            "storage.active_uploads",
            "storage.active_downloads",
            "storage.failed_transfers_recent",
            "storage.cid_fetch_test",
            "storage.last_error"
        ]) },
        { title: qsTr("Messaging / Delivery"), fields: fields([
            "messaging.module",
            "messaging.connection_state",
            "messaging.peer_count",
            "messaging.active_subscriptions",
            "messaging.content_topics",
            "messaging.outbound_queue",
            "messaging.message_sent_events_recent",
            "messaging.message_propagated_events_recent",
            "messaging.message_received_events_recent",
            "messaging.message_error_events_recent",
            "messaging.publish_latency_ms",
            "messaging.receive_latency_ms",
            "messaging.last_error"
        ]) },
        { title: qsTr("Overall"), fields: fields([
            "overall.status",
            "overall.main_risk",
            "overall.operator_action"
        ]) }
    ]
}

function dashboardGraphGroups() {
    return [
        { title: qsTr("Bedrock Blockchain"), fields: fields([
            "bedrock.peer_count",
            "bedrock.tip_minus_lib",
            "bedrock.finality_lag_seconds"
        ], "dashboard") },
        { title: qsTr("LEZ Sequencer"), fields: fields([
            "lez.pending_tx_count",
            "lez.mempool_tx_count",
            "lez.rejected_tx_count_recent",
            "lez.blocks_produced_recent",
            "lez.pending_blocks_count"
        ], "dashboard") },
        { title: qsTr("Indexer"), fields: fields([
            "indexer.indexer_lag_vs_sequencer_head"
        ], "dashboard") },
        { title: qsTr("Storage"), fields: fields([
            "storage.peer_count",
            "storage.shared_files_count",
            "storage.manifest_count",
            "storage.local_storage_used",
            "storage.active_uploads",
            "storage.active_downloads",
            "storage.failed_transfers_total"
        ], "dashboard") },
        { title: qsTr("Messaging / Delivery"), fields: fields([
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
        ], "dashboard") }
    ]
}

function footerSourceGroups() {
    const groups = footerSelectorGroups()
    return groups.map(function (group) {
        const keys = group.fields.map(function (field) {
            return field.key
        })
        const statusKey = keys.length ? keys[0] : ""
        return {
            statusKey: statusKey,
            alignRight: statusKey === "overall.status",
            keys: keys
        }
    })
}

function defaultFooterFieldSelections() {
    return selectionMap([
        "network.network",
        "bedrock.node_health",
        "bedrock.sync_state",
        "bedrock.tip_height",
        "bedrock.tip_minus_lib",
        "lez.rpc_health",
        "lez.last_lez_block_id",
        "indexer.rpc_health",
        "indexer.indexed_finalized_height",
        "messaging.connection_state",
        "messaging.peer_count",
        "messaging.message_error_events_recent",
        "storage.module",
        "storage.node_reachable",
        "storage.peer_count",
        "storage.failed_transfers_recent",
        "overall.status",
        "overall.main_risk",
        "overall.operator_action"
    ])
}

function defaultDashboardGraphSelections() {
    return selectionMap([
        "bedrock.peer_count",
        "bedrock.tip_minus_lib",
        "bedrock.finality_lag_seconds",
        "lez.blocks_produced_recent",
        "indexer.indexer_lag_vs_sequencer_head"
    ])
}

function fieldLabel(key) {
    const lookup = selectorLabels()
    return lookup[key] || key
}

function fieldDetail(key, mode) {
    const lookup = String(mode || "") === "dashboard" ? dashboardDetails() : footerDetails()
    return lookup[key] || qsTr("Status field")
}

function shortLabel(key) {
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

function fieldWidth(key) {
    if (key.indexOf("hash") >= 0 || key.indexOf("error") >= 0 || key === "overall.operator_action" || key === "overall.main_risk") {
        return 190
    }
    return 150
}

function fieldPriority(key) {
    return key.indexOf("_hash") >= 0
            || key.indexOf("_time") >= 0
            || key.indexOf("version") >= 0
            || key.indexOf("last_error") >= 0
            || key === "network.report_time"
            || key === "overall.operator_action" ? "low" : "normal"
}

function usesColorOnly(key) {
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

function showsDot(key) {
    return key === "network.network" || usesColorOnly(key)
}

function fields(keys, mode) {
    return keys.map(function (key) {
        return {
            key: key,
            label: fieldLabel(key),
            detail: fieldDetail(key, mode)
        }
    })
}

function selectionMap(keys) {
    const selected = {}
    for (let i = 0; i < keys.length; ++i) {
        selected[keys[i]] = true
    }
    return selected
}

function selectorLabels() {
    return {
        "network.network": qsTr("network"),
        "network.chain_id": qsTr("chain_id"),
        "network.zone_id": qsTr("zone_id"),
        "network.channel_id": qsTr("channel_id"),
        "network.report_time": qsTr("report_time"),
        "bedrock.node_health": qsTr("node_health"),
        "bedrock.peer_count": qsTr("peer_count"),
        "bedrock.sync_state": qsTr("sync_state"),
        "bedrock.tip_height": qsTr("tip_height"),
        "bedrock.tip_hash": qsTr("tip_hash"),
        "bedrock.lib_height": qsTr("lib_height"),
        "bedrock.lib_hash": qsTr("lib_hash"),
        "bedrock.tip_minus_lib": qsTr("tip_minus_lib"),
        "bedrock.last_tip_time": qsTr("last_tip_time"),
        "bedrock.last_lib_time": qsTr("last_lib_time"),
        "bedrock.finality_lag_seconds": qsTr("finality_lag_seconds"),
        "lez.rpc_health": qsTr("rpc_health"),
        "lez.sequencer_version": qsTr("sequencer_version"),
        "lez.last_lez_block_id": qsTr("last_lez_block_id"),
        "lez.last_lez_block_hash": qsTr("last_lez_block_hash"),
        "lez.last_lez_block_time": qsTr("last_lez_block_time"),
        "lez.pending_tx_count": qsTr("pending_tx_count"),
        "lez.mempool_tx_count": qsTr("mempool_tx_count"),
        "lez.rejected_tx_count_recent": qsTr("rejected_tx_count_recent"),
        "lez.blocks_produced_recent": qsTr("blocks_produced_recent"),
        "lez.publish_to_bedrock_status": qsTr("publish_to_bedrock_status"),
        "lez.last_published_channel_update": qsTr("last_published_channel_update"),
        "lez.last_finalized_callback_height": qsTr("last_finalized_callback_height"),
        "lez.pending_blocks_count": qsTr("pending_blocks_count"),
        "indexer.rpc_health": qsTr("rpc_health"),
        "indexer.indexer_version": qsTr("indexer_version"),
        "indexer.indexed_finalized_height": qsTr("indexed_finalized_height"),
        "indexer.indexed_finalized_hash": qsTr("indexed_finalized_hash"),
        "indexer.indexed_channel_message": qsTr("indexed_channel_message"),
        "indexer.indexer_lag_vs_sequencer_head": qsTr("indexer_lag_vs_sequencer_head"),
        "indexer.last_indexed_time": qsTr("last_indexed_time"),
        "indexer.db_health": qsTr("db_health"),
        "indexer.ingestion_status": qsTr("ingestion_status"),
        "storage.module": qsTr("source"),
        "storage.network": qsTr("network"),
        "storage.node_reachable": qsTr("node_reachable"),
        "storage.nat_mode": qsTr("nat_mode"),
        "storage.udp_discovery_port": qsTr("udp_discovery_port"),
        "storage.tcp_transfer_port": qsTr("tcp_transfer_port"),
        "storage.peer_count": qsTr("peer_count"),
        "storage.dht_connected": qsTr("dht_connected"),
        "storage.shared_files_count": qsTr("shared_files_count"),
        "storage.manifest_count": qsTr("manifest_count"),
        "storage.local_storage_used": qsTr("local_storage_used"),
        "storage.active_uploads": qsTr("upload_requests_total"),
        "storage.active_downloads": qsTr("download_requests_total"),
        "storage.failed_transfers_recent": qsTr("failed_transfers_recent"),
        "storage.failed_transfers_total": qsTr("transfer_failures_total"),
        "storage.cid_fetch_test": qsTr("cid_fetch_test"),
        "storage.last_error": qsTr("last_error"),
        "messaging.module": qsTr("source"),
        "messaging.connection_state": qsTr("connection_state"),
        "messaging.peer_count": qsTr("peer_count"),
        "messaging.active_subscriptions": qsTr("active_subscriptions"),
        "messaging.content_topics": qsTr("content_topics"),
        "messaging.outbound_queue": qsTr("outbound_queue"),
        "messaging.message_sent_events_recent": qsTr("message_sent_events_recent"),
        "messaging.message_propagated_events_recent": qsTr("message_propagated_events_recent"),
        "messaging.message_received_events_recent": qsTr("waku_node_messages_total"),
        "messaging.message_error_events_recent": qsTr("waku_node_errors_total"),
        "messaging.publish_latency_ms": qsTr("publish_latency_ms"),
        "messaging.receive_latency_ms": qsTr("receive_latency_ms"),
        "messaging.last_error": qsTr("last_error"),
        "overall.status": qsTr("status"),
        "overall.main_risk": qsTr("main_risk"),
        "overall.operator_action": qsTr("operator_action")
    }
}

function footerDetails() {
    return {
        "network.network": qsTr("testnet, mainnet, local, or custom"),
        "network.chain_id": qsTr("Bedrock chain identifier"),
        "network.zone_id": qsTr("Execution zone identifier"),
        "network.channel_id": qsTr("Active delivery channel identifier"),
        "network.report_time": qsTr("Last local report timestamp"),
        "bedrock.node_health": qsTr("ok, degraded, or down"),
        "bedrock.peer_count": qsTr("Connected Bedrock peers"),
        "bedrock.sync_state": qsTr("synced, syncing, or stalled"),
        "bedrock.tip_height": qsTr("Current tip height"),
        "bedrock.tip_hash": qsTr("Current tip hash"),
        "bedrock.lib_height": qsTr("Last irreversible block height"),
        "bedrock.lib_hash": qsTr("Last irreversible block hash"),
        "bedrock.tip_minus_lib": qsTr("Distance from tip to LIB"),
        "bedrock.last_tip_time": qsTr("Last tip observation time"),
        "bedrock.last_lib_time": qsTr("Last LIB observation time"),
        "bedrock.finality_lag_seconds": qsTr("Approximate finality lag"),
        "lez.rpc_health": qsTr("Sequencer RPC availability"),
        "lez.sequencer_version": qsTr("Sequencer version"),
        "lez.last_lez_block_id": qsTr("Latest LEZ block id"),
        "lez.last_lez_block_hash": qsTr("Latest LEZ block hash"),
        "lez.last_lez_block_time": qsTr("Latest LEZ block time"),
        "lez.pending_tx_count": qsTr("Pending sequencer transactions"),
        "lez.mempool_tx_count": qsTr("Mempool transaction count"),
        "lez.rejected_tx_count_recent": qsTr("Recent rejected transactions"),
        "lez.blocks_produced_recent": qsTr("Recent LEZ blocks produced"),
        "lez.publish_to_bedrock_status": qsTr("Bedrock publish state"),
        "lez.last_published_channel_update": qsTr("Last channel update publication"),
        "lez.last_finalized_callback_height": qsTr("Last finalized callback height"),
        "lez.pending_blocks_count": qsTr("Pending LEZ blocks"),
        "indexer.rpc_health": qsTr("Indexer RPC availability"),
        "indexer.indexer_version": qsTr("Indexer version"),
        "indexer.indexed_finalized_height": qsTr("Indexed finalized height"),
        "indexer.indexed_finalized_hash": qsTr("Indexed finalized hash"),
        "indexer.indexed_channel_message": qsTr("Indexed channel message"),
        "indexer.indexer_lag_vs_sequencer_head": qsTr("Indexer lag versus sequencer"),
        "indexer.last_indexed_time": qsTr("Last indexed timestamp"),
        "indexer.db_health": qsTr("Database health"),
        "indexer.ingestion_status": qsTr("running, stalled, or backfilling"),
        "storage.module": qsTr("REST or metrics source status"),
        "storage.network": qsTr("Storage preset or network name"),
        "storage.node_reachable": qsTr("Storage node reachability"),
        "storage.nat_mode": qsTr("upnp, port-forward, or manual"),
        "storage.udp_discovery_port": qsTr("UDP discovery port state"),
        "storage.tcp_transfer_port": qsTr("TCP transfer port state"),
        "storage.peer_count": qsTr("Storage peers"),
        "storage.dht_connected": qsTr("DHT connectivity"),
        "storage.shared_files_count": qsTr("Shared files"),
        "storage.manifest_count": qsTr("Manifest count"),
        "storage.local_storage_used": qsTr("Local storage usage"),
        "storage.active_uploads": qsTr("Upload request counter total"),
        "storage.active_downloads": qsTr("Download request counter total"),
        "storage.failed_transfers_recent": qsTr("Recent transfer failures"),
        "storage.failed_transfers_total": qsTr("Historical transfer failure counter total"),
        "storage.cid_fetch_test": qsTr("CID fetch probe result"),
        "storage.last_error": qsTr("Latest storage error"),
        "messaging.module": qsTr("REST or metrics source status"),
        "messaging.connection_state": qsTr("connected, disconnected, or connecting"),
        "messaging.peer_count": qsTr("Delivery peers"),
        "messaging.active_subscriptions": qsTr("Not exposed by current Delivery metrics"),
        "messaging.content_topics": qsTr("Subscribed content topics"),
        "messaging.outbound_queue": qsTr("Outbound message queue"),
        "messaging.message_sent_events_recent": qsTr("Not exposed by current Delivery metrics"),
        "messaging.message_propagated_events_recent": qsTr("Not exposed by current Delivery metrics"),
        "messaging.message_received_events_recent": qsTr("Delivery message counter total"),
        "messaging.message_error_events_recent": qsTr("Delivery error counter total"),
        "messaging.publish_latency_ms": qsTr("Not exposed by current Delivery metrics"),
        "messaging.receive_latency_ms": qsTr("Not exposed by current Delivery metrics"),
        "messaging.last_error": qsTr("Latest Delivery error"),
        "overall.status": qsTr("healthy, degraded, or down"),
        "overall.main_risk": qsTr("Most important current risk"),
        "overall.operator_action": qsTr("Suggested operator action")
    }
}

function dashboardDetails() {
    const details = footerDetails()
    details["bedrock.tip_minus_lib"] = qsTr("Tip to LIB distance")
    details["lez.blocks_produced_recent"] = qsTr("Recent produced blocks")
    details["storage.manifest_count"] = qsTr("Manifests")
    details["messaging.content_topics"] = qsTr("Content topics")
    details["messaging.outbound_queue"] = qsTr("Outbound queue")
    return details
}
