import QtQuick
import QtTest
import "../../qml/state/status/StatusFieldCatalog.js" as StatusFieldCatalog

TestCase {
    name: "StatusFieldCatalog"

    function test_footer_selector_groups_drive_footer_source_groups() {
        const selectorGroups = StatusFieldCatalog.footerSelectorGroups()
        const sourceGroups = StatusFieldCatalog.footerSourceGroups()

        compare(sourceGroups.length, selectorGroups.length + 1)
        compare(selectorGroups[0].fields[0].key, "network.network")
        compare(sourceGroups[0].statusKey, "network.network")
        compare(sourceGroups[2].dynamic, "channels")
        compare(sourceGroups[sourceGroups.length - 1].alignRight, true)
    }

    function test_footer_source_groups_reuse_static_projection() {
        const first = StatusFieldCatalog.footerSourceGroups()
        const second = StatusFieldCatalog.footerSourceGroups()

        verify(first === second)
    }

    function test_defaults_are_catalog_owned() {
        const footer = StatusFieldCatalog.defaultFooterFieldSelections()
        const dashboard = StatusFieldCatalog.defaultDashboardGraphSelections()

        verify(footer["overall.status"])
        verify(footer["channels.summary"] === undefined)
        verify(footer["storage.failed_transfers_recent"])
        verify(!footer["network.chain_id"])
        verify(dashboard["bedrock.finality_lag_seconds"])
        verify(!dashboard["messaging.publish_latency_ms"])
    }

    function test_footer_selection_migration_removes_single_zone_fields() {
        const source = {
            "lez.rpc_health": true,
            "indexer.rpc_health": true,
            "channels.summary": false,
            "storage.module": false
        }
        source["channel." + "a".repeat(64)] = true
        const selections = StatusFieldCatalog.normalizedFooterFieldSelections(source)

        verify(selections["lez.rpc_health"] === undefined)
        verify(selections["indexer.rpc_health"] === undefined)
        verify(selections["channels.summary"] === undefined)
        verify(selections["channel." + "a".repeat(64)])
        verify(!selections["storage.module"])
    }

    function test_configured_zones_are_individual_footer_toggles() {
        const fields = StatusFieldCatalog.footerSelectorGroups([{
            channel_id: "a".repeat(64),
            short_channel_id: "aaaa…aaaa",
            label: "Alpha",
            sequencer: { configured: true },
            indexer: { configured: true }
        }, {
            channel_id: "b".repeat(64),
            short_channel_id: "bbbb…bbbb",
            label: "Beta",
            sequencer: { configured: true },
            indexer: { configured: false }
        }]).filter(function (group) {
            return group.title === "Configured Zones"
        })[0].fields

        compare(fields.length, 2)
        compare(fields[0].key, "channel." + "a".repeat(64))
        compare(fields[0].label, "Alpha · aaaa…aaaa")
        compare(fields[1].key, "channel." + "b".repeat(64))
        compare(fields[1].label, "Beta · bbbb…bbbb")
    }

    function test_footer_row_policy_uses_catalog_metadata() {
        compare(StatusFieldCatalog.shortLabel("storage.node_reachable"), "Storage node")
        compare(StatusFieldCatalog.fieldWidth("overall.operator_action"), 190)
        compare(StatusFieldCatalog.fieldPriority("network.report_time"), "low")
        verify(StatusFieldCatalog.usesColorOnly("overall.status"))
        verify(StatusFieldCatalog.showsDot("network.network"))
    }

    function test_footer_labels_name_the_service_and_status() {
        compare(StatusFieldCatalog.shortLabel("messaging.connection_state"), "Delivery")
        compare(StatusFieldCatalog.shortLabel("messaging.peer_count"), "Delivery peers")
        compare(StatusFieldCatalog.shortLabel("messaging.message_error_events_recent"), "Errors")
        compare(StatusFieldCatalog.shortLabel("storage.module"), "Storage")
        compare(StatusFieldCatalog.shortLabel("storage.peer_count"), "Storage peers")
        compare(StatusFieldCatalog.fieldWidth("messaging.peer_count"), 176)
    }

    function test_available_provisional_block_labels_do_not_claim_a_time_window() {
        const key = "lez.blocks_produced_recent"

        compare(StatusFieldCatalog.fieldLabel(key), "Provisional block records")
        compare(StatusFieldCatalog.shortLabel(key), "Prov recs")
        compare(StatusFieldCatalog.fieldDetail(key),
                "Provisional block records available for the active Zone from loaded Sequencer rows or the latest head summary; not a time-window production count")
        compare(StatusFieldCatalog.fieldDetail(key, "dashboard"),
                "Provisional block records available for the active Zone from loaded Sequencer rows or the latest head summary; not a time-window production count")
    }

    function test_selector_labels_are_human_facing_without_changing_keys() {
        const labels = StatusFieldCatalog.selectorLabels()
        const expected = {
            "network.network": "Network",
            "network.chain_id": "Bedrock chain ID",
            "network.zone_id": "Execution Zone ID",
            "network.channel_id": "Active Channel ID",
            "network.report_time": "Last report time",
            "bedrock.node_health": "Node health",
            "bedrock.peer_count": "Connected peers",
            "bedrock.sync_state": "Sync status",
            "bedrock.tip_height": "Tip height",
            "bedrock.tip_hash": "Tip hash",
            "bedrock.lib_height": "Last irreversible block height",
            "bedrock.lib_hash": "Last irreversible block hash",
            "bedrock.tip_minus_lib": "Tip-to-LIB gap",
            "bedrock.last_tip_time": "Last tip observation",
            "bedrock.last_lib_time": "Last LIB observation",
            "bedrock.finality_lag_seconds": "Finality lag",
            "lez.rpc_health": "Sequencer RPC health",
            "lez.sequencer_version": "Sequencer version",
            "lez.last_lez_block_id": "Latest Sequencer block ID",
            "lez.last_lez_block_hash": "Latest Sequencer block hash",
            "lez.last_lez_block_time": "Latest Sequencer block time",
            "lez.pending_tx_count": "Pending transactions",
            "lez.mempool_tx_count": "Mempool transactions",
            "lez.rejected_tx_count_recent": "Recent rejected transactions",
            "lez.blocks_produced_recent": "Provisional block records",
            "lez.publish_to_bedrock_status": "Bedrock publication status",
            "lez.last_published_channel_update": "Last Channel update publication",
            "lez.last_finalized_callback_height": "Last finalized callback height",
            "lez.pending_blocks_count": "Pending Sequencer blocks",
            "indexer.rpc_health": "Indexer RPC health",
            "indexer.indexer_version": "Indexer version",
            "indexer.indexed_finalized_height": "Indexed finalized height",
            "indexer.indexed_finalized_hash": "Indexed finalized hash",
            "indexer.indexed_channel_message": "Indexed Channel message",
            "indexer.indexer_lag_vs_sequencer_head": "Indexer lag behind Sequencer",
            "indexer.last_indexed_time": "Last indexed time",
            "indexer.db_health": "Database health",
            "indexer.ingestion_status": "Ingestion status",
            "storage.module": "Storage source status",
            "storage.network": "Storage network",
            "storage.node_reachable": "Storage node reachability",
            "storage.nat_mode": "NAT mode",
            "storage.udp_discovery_port": "UDP discovery port",
            "storage.tcp_transfer_port": "TCP transfer port",
            "storage.peer_count": "Storage peers",
            "storage.dht_connected": "DHT connection",
            "storage.shared_files_count": "Shared files",
            "storage.manifest_count": "Manifests",
            "storage.local_storage_used": "Local storage used",
            "storage.active_uploads": "Total upload requests",
            "storage.active_downloads": "Total download requests",
            "storage.failed_transfers_recent": "Recent transfer failures",
            "storage.failed_transfers_total": "Total transfer failures",
            "storage.cid_fetch_test": "CID fetch test",
            "storage.last_error": "Last storage error",
            "messaging.module": "Delivery source status",
            "messaging.connection_state": "Delivery connection",
            "messaging.peer_count": "Delivery peers",
            "messaging.active_subscriptions": "Active subscriptions",
            "messaging.content_topics": "Content topics",
            "messaging.outbound_queue": "Outbound queue",
            "messaging.message_sent_events_recent": "Recent sent-message events",
            "messaging.message_propagated_events_recent": "Recent propagation events",
            "messaging.message_received_events_recent": "Total received messages",
            "messaging.message_error_events_recent": "Total Delivery errors",
            "messaging.publish_latency_ms": "Publish latency",
            "messaging.receive_latency_ms": "Receive latency",
            "messaging.last_error": "Last Delivery error",
            "overall.status": "Overall status",
            "overall.main_risk": "Main risk",
            "overall.operator_action": "Suggested operator action"
        }

        compare(Object.keys(labels).length, Object.keys(expected).length)
        for (const key in expected) {
            compare(labels[key], expected[key])
            verify(String(labels[key]).indexOf("_") === -1,
                   key + " label must not expose an internal identifier")
        }
        verify(labels["network.chain_id"] !== "network.chain_id")
        verify(labels["lez.pending_tx_count"] !== "lez.pending_tx_count")
        verify(labels["storage.node_reachable"] !== "storage.node_reachable")
        verify(labels["messaging.connection_state"] !== "messaging.connection_state")
    }
}
