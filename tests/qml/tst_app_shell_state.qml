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
        model.currentView = "overview"
        model.statusText = "Ready"
        model.busy = false
        model.resultTitle = "Output"
        model.resultText = ""
        model.resultValue = null
        model.resultIsError = false
        model.resultOwner = ""
        model.navigationBackStack = []
        model.navigationForwardStack = []
        model.navigationRevision = 0
        model.navigationRestoring = false
        model.settingsSection = "general"
        model.settingsNetworkSection = "blockchain"
        model.settingsUiSection = "footer"
    }

    function test_shell_state_owns_result_aliases() {
        model.shell.setResult("Storage", "done", false, { cid: "z-cid" }, "storage")

        compare(model.resultTitle, "Storage")
        compare(model.resultText, "done")
        compare(model.resultOwner, "storage")
        verify(model.pageHasOutput("storage"))

        model.shell.clearResult()

        compare(model.resultTitle, "Output")
        compare(model.resultText, "")
        verify(!model.pageHasOutput("storage"))
    }

    function test_shell_state_controls_navigation_and_settings() {
        model.shell.selectView("storage")

        compare(model.currentView, "storage")
        verify(model.canNavigateBack())

        model.shell.openSettings("network", "storage")

        compare(model.currentView, "settings")
        compare(model.settingsSection, "network")
        compare(model.settingsNetworkSection, "storage")

        model.shell.navigateBack()

        compare(model.currentView, "storage")
    }

    function test_hidden_zones_route_has_l1_metadata_without_nav_entry() {
        model.shell.selectView("zones")

        compare(model.currentView, "zones")
        compare(model.viewTitle(), "Zones")
        compare(model.parentNavKeyForView("zones"), "l1")
        const rows = model.navRows()
        verify(!rows.some(function (row) {
            return String(row.view || "") === "zones"
        }))
    }
}
