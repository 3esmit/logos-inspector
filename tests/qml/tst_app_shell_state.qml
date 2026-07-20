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
        model.shell.navExpanded = ({ l1: true, zones: true, network: true, diagnostics: false, local: true, system: true })
        model.deliveryDiagnosticsTab = "overview"
        model.zoneInspection.verification = "empty"
        model.zoneInspection.networkScopeKey = ""
        model.zoneInspection.summaryStale = false
        model.zoneInspection.zoneSummaries = []
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

    function test_zones_route_has_zone_group_metadata_and_catalog_entry() {
        model.shell.selectView("zones")

        compare(model.shell.currentView, "zones")
        compare(model.shell.viewTitle(), "Zone Catalog")
        compare(model.shell.parentNavKeyForView("zones"), "zones")
        const rows = model.shell.navRows()
        verify(rows.some(function (row) {
            return String(row.type || "") === "group"
                && String(row.key || "") === "zones"
        }))
        verify(rows.some(function (row) {
            return String(row.view || "") === "zones"
        }))
    }

    function test_zones_group_lists_each_configured_dashboard() {
        verify(!model.shell.navRows().some(function (row) {
            return String(row.view || "") === "sequencerDashboard"
        }))

        const channelId = "22".repeat(32)
        model.zoneInspection.verification = "verified"
        model.zoneInspection.networkScopeKey = "genesis_id:" + "11".repeat(32)
        model.zoneInspection.zoneSummaries = [configuredZone(channelId)]
        wait(0)

        const rows = model.shell.navRows()
        const sequencer = rows.filter(function (row) {
            return String(row.view || "") === "sequencerDashboard"
        })
        compare(sequencer.length, 1)
        compare(sequencer[0].channelId, channelId)
        compare(sequencer[0].label, "Alpha")
        compare(sequencer[0].accessibleName,
                "Open Zone dashboard for Alpha (" + channelId + ")")
        compare(sequencer[0].parentKey, "zones")
        compare(sequencer[0].depth, 1)
        compare(model.shell.parentNavKeyForView("sequencerDashboard"), "zones")

        verify(model.openZoneDashboard(channelId))
        compare(model.zoneInspection.activeZoneId, channelId)
        compare(model.shell.currentView, "sequencerDashboard")
        compare(model.shell.viewTitle(), "Alpha")
        model.currentInspectionEntityRef = {
            layer: "l2",
            channel_id: channelId,
            entity_kind: "account",
            canonical_key: "account-a"
        }
        const snapshot = model.shell.navigationSnapshot()
        verify(snapshot.values.inspectionEntityRef !== null)
        compare(snapshot.values.inspectionEntityRef.canonical_key, "account-a")
    }

    function test_zones_group_hides_unverified_configured_dashboards() {
        const channelId = "22".repeat(32)
        model.zoneInspection.networkScopeKey = "genesis_id:" + "11".repeat(32)
        model.zoneInspection.zoneSummaries = [configuredZone(channelId)]
        model.zoneInspection.verification = "checking"
        wait(0)

        verify(!model.shell.navRows().some(function (row) {
            return String(row.view || "") === "sequencerDashboard"
        }))
    }

    function configuredZone(channelId) {
        return {
            channel_id: channelId,
            kind: "sequencer_zone",
            display: {
                alias: "Alpha",
                title: "Alpha Zone",
                short_channel_id: "2222…2222"
            },
            active_zone_context_fields: {
                network_scope: {
                    kind: "genesis_id",
                    genesis_id: "11".repeat(32)
                },
                channel_id: channelId,
                zone_kind: "sequencer_zone",
                selected_sequencer_source_id: "seq-a",
                indexer_source_id: "idx-a",
                source_config_revision: 1
            },
            settlement_link: {
                selected_sequencer_source_id: "seq-a",
                indexer_source_id: "idx-a"
            }
        }
    }
}
