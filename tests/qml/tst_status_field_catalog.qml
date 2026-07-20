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

        compare(StatusFieldCatalog.fieldLabel(key), "provisional_block_records_available")
        compare(StatusFieldCatalog.shortLabel(key), "Prov recs")
        compare(StatusFieldCatalog.fieldDetail(key),
                "Provisional block records available for the active Zone from loaded Sequencer rows or the latest head summary; not a time-window production count")
        compare(StatusFieldCatalog.fieldDetail(key, "dashboard"),
                "Provisional block records available for the active Zone from loaded Sequencer rows or the latest head summary; not a time-window production count")
    }
}
