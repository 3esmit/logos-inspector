import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/components"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "StatusBar"
    when: windowShown
    width: 900
    height: 120

    readonly property string transactionHash: "0340d657f1ad70b7ba276b896e0480976f65424a28d833dafb69f7de76873f29"

    QtObject {
        id: fakeHost

        function callModuleJson(moduleName, method, argsJson) {
            return JSON.stringify({
                ok: true,
                value: {},
                text: "",
                error: ""
            })
        }
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    Theme {
        id: theme
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        StatusBar {
            id: statusBar

            theme: theme
            model: model
            width: testWindow.width
        }
    }

    function init() {
        model.shell.busy = false
        model.shell.currentView = "overview"
        model.transactionDetailValue = null
        model.transactionsPageRows = [{
            hash: transactionHash,
            block: "31568dea8aa83b2888bcac3bf486262fd4273b6aca5b2ec877bff7e8e2390575",
            slot: 1431649,
            index: 0,
            operations: [],
            raw: {}
        }]
    }

    function test_mantle_prefix_is_a_valid_transaction_lookup() {
        const colonQuery = "mantle:" + transactionHash
        const spaceQuery = "mantle " + transactionHash

        compare(statusBar.lookupCode(colonQuery), "transaction")
        compare(statusBar.lookupCode(spaceQuery), "transaction")
        compare(statusBar.lookupLabel("transaction"), "TX")
        verify(statusBar.lookupCanOpen(colonQuery))
        verify(statusBar.lookupCanOpen(spaceQuery))
    }

    function test_mantle_prefix_search_opens_the_cached_transaction() {
        const lookup = findChild(statusBar, "globalReferenceLookup")
        const search = findChild(statusBar, "globalReferenceSearch")

        verify(lookup !== null)
        verify(search !== null)
        lookup.text = "mantle:" + transactionHash
        tryCompare(search, "enabled", true)

        mouseClick(search, search.width / 2, search.height / 2)

        tryCompare(model.shell, "currentView", "transactionDetail")
        compare(model.transactionDetailValue.hash, transactionHash)
        tryCompare(lookup, "text", "")
    }
}
