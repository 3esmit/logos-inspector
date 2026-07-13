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
}
