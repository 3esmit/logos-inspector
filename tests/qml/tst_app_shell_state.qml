import QtQuick
import QtTest
import "../../qml/services"
import "../../qml/state"
import "fixtures"

TestCase {
    id: testRoot

    name: "AppShellState"

    BridgeHostFixture {
        id: fakeHost
    }

    BridgeClient {
        id: bridge
        host: fakeHost
    }

    AppModel {
        id: model
        bridge: bridge
    }

    function init() {
        fakeHost.reset()
        model.shell.currentView = "overview"
        model.shell.statusText = "Ready"
        model.shell.busy = false
        model.shell.resultTitle = "Output"
        model.shell.resultText = ""
        model.shell.resultValue = null
        model.shell.resultIsError = false
        model.shell.resultOwner = ""
        model.shell.navigationBackStack = []
        model.shell.navigationForwardStack = []
        model.shell.navigationRevision = 0
        model.shell.navigationRestoring = false
        model.shell.settingsSection = "general"
        model.shell.settingsNetworkSection = "blockchain"
        model.shell.settingsUiSection = "footer"
        model.deliveryDiagnosticsTab = "overview"
        model.zoneInspection.verification = "empty"
        model.zoneInspection.activeZoneContext = null
    }

    function test_shell_state_owns_result_aliases() {
        model.shell.setResult("Storage", "done", false, { cid: "z-cid" }, "storage")

        compare(model.shell.resultTitle, "Storage")
        compare(model.shell.resultText, "done")
        compare(model.shell.resultOwner, "storage")
        verify(model.shell.pageHasOutput("storage"))

        model.shell.clearResult()

        compare(model.shell.resultTitle, "Output")
        compare(model.shell.resultText, "")
        verify(!model.shell.pageHasOutput("storage"))
    }

    function test_shell_state_controls_navigation_and_settings() {
        model.shell.selectView("storage")

        compare(model.shell.currentView, "storage")
        verify(model.shell.canNavigateBack())

        model.shell.openSettings("network", "storage")

        compare(model.shell.currentView, "settings")
        compare(model.shell.settingsSection, "network")
        compare(model.shell.settingsNetworkSection, "storage")

        model.shell.navigateBack()

        compare(model.shell.currentView, "storage")
    }

    function test_delivery_diagnostics_tab_is_part_of_navigation_history() {
        model.shell.currentView = "diagnosticsDelivery"
        model.deliveryDiagnosticsTab = "store"

        model.shell.openSettings("network", "messaging")
        model.deliveryDiagnosticsTab = "overview"
        model.shell.navigateBack()

        compare(model.shell.currentView, "diagnosticsDelivery")
        compare(model.deliveryDiagnosticsTab, "store")
    }

    function test_zones_route_has_l1_metadata_and_nav_entry() {
        model.shell.selectView("zones")

        compare(model.shell.currentView, "zones")
        compare(model.shell.viewTitle(), "Zones")
        compare(model.shell.parentNavKeyForView("zones"), "l1")
        const rows = model.shell.navRows()
        verify(rows.some(function (row) {
            return String(row.view || "") === "zones"
        }))
    }

    function test_sequencer_route_appears_only_for_selected_source() {
        verify(!model.shell.navRows().some(function (row) {
            return String(row.view || "") === "sequencerDashboard"
        }))

        model.zoneInspection.verification = "verified"
        model.zoneInspection.activeZoneContext = {
            network_scope: {
                kind: "genesis_id",
                genesis_id: "11".repeat(32)
            },
            channel_id: "22".repeat(32),
            zone_kind: "sequencer_zone",
            selected_sequencer_source_id: "seq-a",
            indexer_source_id: "idx-a",
            source_config_revision: 1,
            context_revision: 1
        }
        wait(0)

        const rows = model.shell.navRows()
        const sequencer = rows.filter(function (row) {
            return String(row.view || "") === "sequencerDashboard"
        })
        compare(sequencer.length, 1)
        compare(sequencer[0].parentKey, "zones")
        compare(sequencer[0].depth, 2)
        compare(model.shell.parentNavKeyForView("sequencerDashboard"), "l1")

        model.shell.selectView("sequencerDashboard")
        compare(model.shell.currentView, "sequencerDashboard")
        compare(model.shell.viewTitle(), "Sequencer")
        model.currentInspectionEntityRef = {
            layer: "l2",
            channel_id: "22".repeat(32),
            entity_kind: "account",
            canonical_key: "account-a"
        }
        const snapshot = model.shell.navigationSnapshot()
        verify(snapshot.values.inspectionEntityRef !== null)
        compare(snapshot.values.inspectionEntityRef.canonical_key, "account-a")
    }
}
