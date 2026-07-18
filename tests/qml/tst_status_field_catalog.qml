import QtQuick
import QtTest
import "../../qml/state/status/StatusFieldCatalog.js" as StatusFieldCatalog

TestCase {
    name: "StatusFieldCatalog"

    function test_footer_selector_groups_drive_footer_source_groups() {
        const selectorGroups = StatusFieldCatalog.footerSelectorGroups()
        const sourceGroups = StatusFieldCatalog.footerSourceGroups()

        compare(selectorGroups.length, sourceGroups.length)
        compare(selectorGroups[0].fields[0].key, "network.network")
        compare(sourceGroups[0].statusKey, "network.network")
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
        verify(footer["storage.failed_transfers_recent"])
        verify(!footer["network.chain_id"])
        verify(dashboard["bedrock.finality_lag_seconds"])
        verify(!dashboard["messaging.publish_latency_ms"])
    }

    function test_footer_row_policy_uses_catalog_metadata() {
        compare(StatusFieldCatalog.shortLabel("storage.node_reachable"), "ST node")
        compare(StatusFieldCatalog.fieldWidth("overall.operator_action"), 190)
        compare(StatusFieldCatalog.fieldPriority("network.report_time"), "low")
        verify(StatusFieldCatalog.usesColorOnly("overall.status"))
        verify(StatusFieldCatalog.showsDot("network.network"))
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
