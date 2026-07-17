pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/storage/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "StorageDiagnosticsPage"
    when: windowShown
    width: 1280
    height: 900

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

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        Loader {
            id: pageLoader

            sourceComponent: model.shell.currentView === "storage"
                ? storageWorkspaceComponent : storageDiagnosticsComponent
            width: testWindow.width
        }
    }

    Component {
        id: storageDiagnosticsComponent

        StoragePage {
            theme: theme
            model: model
            width: testWindow.width
        }
    }

    Component {
        id: storageWorkspaceComponent

        StorageAppPage {
            theme: theme
            model: model.storageApp
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.metrics.storageRefreshRate = 0
        model.storageAppTab = "files"
        model.storageDiagnosticsTab = "diagnostics"
        model.storageCidProbe = "z-selected-diagnostic-cid"
        model.shell.currentView = "diagnosticsStorage"
        model.navigationBackStack = []
        model.navigationForwardStack = []
        tryVerify(function () { return pageLoader.item !== null })
    }

    function test_live_storage_workflow_routes_replace_dead_placeholders() {
        let cidTools = null
        let transferTools = null
        tryVerify(function () {
            cidTools = findAccessibleByName(pageLoader.item, "Open Storage CID tools")
            transferTools = findAccessibleByName(pageLoader.item, "Open Storage transfer tools")
            return cidTools !== null && transferTools !== null
        })

        verify(cidTools.enabled)
        verify(transferTools.enabled)
        verify(findAccessibleByName(pageLoader.item, "Manifest fetch") === null)
        verify(findAccessibleByName(pageLoader.item, "Provider lookup") === null)
        verify(findAccessibleByName(pageLoader.item, "Download probe") === null)

        mouseClick(cidTools, cidTools.width / 2, cidTools.height / 2)

        compare(model.storageAppTab, "cid")
        compare(model.storageCidProbe, "z-selected-diagnostic-cid")
        compare(model.shell.currentView, "storage")
        verify(model.canNavigateBack())
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "CID selected") !== null
        })

        model.navigateBack()
        compare(model.shell.currentView, "diagnosticsStorage")
        compare(model.storageAppTab, "files")
        compare(model.storageDiagnosticsTab, "diagnostics")

        tryVerify(function () {
            transferTools = findAccessibleByName(
                pageLoader.item, "Open Storage transfer tools")
            return transferTools !== null
        })

        mouseClick(transferTools, transferTools.width / 2, transferTools.height / 2)

        compare(model.storageAppTab, "transfer")
        compare(model.storageCidProbe, "z-selected-diagnostic-cid")
        compare(model.shell.currentView, "storage")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Transfer selected") !== null
        })
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name || "") === expectedName
                && item.visible) {
            return item
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            const match = findAccessibleByName(children[i], expectedName)
            if (match) {
                return match
            }
        }
        return null
    }
}
