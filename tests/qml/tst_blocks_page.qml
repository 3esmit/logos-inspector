pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/bedrock/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "BlocksPage"
    when: windowShown
    width: 900
    height: 700

    BridgeHostFixture {
        id: fakeHost
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

    Component {
        id: pageComponent

        BlocksPage {
            theme: theme
            model: model
        }
    }

    ApplicationWindow {
        visible: true
        width: testRoot.width
        height: testRoot.height

        Loader {
            id: pageLoader

            anchors.fill: parent
            active: false
            sourceComponent: pageComponent
        }
    }

    function init() {
        pageLoader.active = false
        model.chainPages.invalidateOperations("test reset")
        wait(0)
        fakeHost.reset()
        model.blocksPageRows = []
        model.blocksPageSlotFrom = 0
        model.blocksPageSlotTo = 0
        model.blocksPageError = ""
        model.blocksLiveEnabled = false
    }

    function cleanup() {
        pageLoader.active = false
        model.chainPages.invalidateOperations("test cleanup")
        wait(0)
    }

    function runtimeOperationCallCount(method) {
        return fakeHost.calls.filter(function (call) {
            const request = call.method === "runtimeOperationStart" && call.args
                ? call.args[0] || null : null
            return request && String(request.method || "") === String(method || "")
        }).length
    }

    function test_cached_rows_refresh_once_on_page_entry() {
        model.blocksPageRows = [{
            header: { slot: 20, id: "cached-block" },
            transactions: []
        }]
        model.blocksPageSlotFrom = 1
        model.blocksPageSlotTo = 20

        pageLoader.active = true

        tryVerify(function () { return pageLoader.item !== null })
        tryVerify(function () {
            return runtimeOperationCallCount("blockchainNode") === 1
        })
        wait(100)
        compare(runtimeOperationCallCount("blockchainNode"), 1)
    }
}
