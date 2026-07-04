.pragma library

function sourceGroups() {
    return [
        { statusKey: "network.network", keys: [
            "network.network",
            "network.chain_id",
            "network.zone_id",
            "network.channel_id",
            "network.report_time"
        ] },
        { statusKey: "bedrock.node_health", keys: [
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
        ] },
        { statusKey: "lez.rpc_health", keys: [
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
        ] },
        { statusKey: "indexer.rpc_health", keys: [
            "indexer.rpc_health",
            "indexer.indexer_version",
            "indexer.indexed_finalized_height",
            "indexer.indexed_finalized_hash",
            "indexer.indexed_channel_message",
            "indexer.indexer_lag_vs_sequencer_head",
            "indexer.last_indexed_time",
            "indexer.db_health",
            "indexer.ingestion_status"
        ] },
        { statusKey: "storage.module", keys: [
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
        ] },
        { statusKey: "messaging.connection_state", keys: [
            "messaging.connection_state",
            "messaging.module",
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
        ] },
        { statusKey: "overall.status", alignRight: true, keys: [
            "overall.status",
            "overall.main_risk",
            "overall.operator_action"
        ] }
    ]
}
