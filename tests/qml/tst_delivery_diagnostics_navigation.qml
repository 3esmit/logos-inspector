pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/delivery/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "DeliveryDiagnosticsNavigation"
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

            sourceComponent: model.shell.currentView === "diagnosticsDelivery"
                ? deliveryDiagnosticsComponent : emptyComponent
            width: testWindow.width
        }
    }

    Component {
        id: deliveryDiagnosticsComponent

        DeliveryPage {
            theme: theme
            model: model
            width: testWindow.width
        }
    }

    Component {
        id: emptyComponent

        Item {}
    }

    function init() {
        fakeHost.reset()
        model.metrics.messagingRefreshRate = 0
        model.deliveryDiagnosticsTab = "overview"
        model.shell.currentView = "diagnosticsDelivery"
        model.navigationBackStack = []
        model.navigationForwardStack = []
        tryVerify(function () {
            return pageLoader.item !== null
                && findAccessibleByName(pageLoader.item, "Overview selected") !== null
        })
        wait(100)
    }

    function test_settings_back_restores_selected_tab() {
        const storeTab = findAccessibleByName(pageLoader.item, "Store")
        verify(storeTab !== null)
        mouseClick(storeTab, storeTab.width / 2, storeTab.height / 2)
        tryCompare(model, "deliveryDiagnosticsTab", "store")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Store selected") !== null
        })

        const openSettings = findAccessibleByName(
            pageLoader.item, "Open Delivery settings")
        verify(openSettings !== null)
        mouseClick(openSettings, openSettings.width / 2, openSettings.height / 2)
        compare(model.shell.currentView, "settings")
        verify(model.canNavigateBack())

        model.deliveryDiagnosticsTab = "overview"
        model.navigateBack()

        compare(model.shell.currentView, "diagnosticsDelivery")
        compare(model.deliveryDiagnosticsTab, "store")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Store selected") !== null
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
        for (let index = 0; index < children.length; ++index) {
            const match = findAccessibleByName(children[index], expectedName)
            if (match) {
                return match
            }
        }
        return null
    }
}
