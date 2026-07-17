pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtTest
import "../../qml/features/settings/controls"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "StorageConnectionPanel"
    when: windowShown
    width: 1180
    height: 760

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

    ListModel {
        id: sourceOptions

        ListElement {
            value: "logoscore_cli_storage_module"
            label: "LogosCore CLI"
        }
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        StorageConnectionPanel {
            id: panel

            theme: theme
            title: qsTr("Storage")
            subtitle: qsTr("Configure the Storage inspection source.")
            pageWidth: testWindow.width
            modelRef: model
            sourceOptions: sourceOptions
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.storageLocalDiagnosticsEnabled = false
        wait(0)
    }

    function test_path_privacy_control_is_truthful_and_has_no_fake_path_editor() {
        compare(
            model.storageDisplayPath("/tmp/legacy-storage-data"),
            ".../legacy-storage-data")
        let showLocalPaths = null
        tryVerify(function () {
            showLocalPaths = findAccessibleByName(panel, "Show local paths")
            return showLocalPaths !== null
        })

        compare(showLocalPaths.Accessible.role, Accessible.CheckBox)
        compare(
            String(showLocalPaths.Accessible.description),
            "Shows full local storage paths in diagnostics and enables their Copy actions.")
        verify(findAccessibleByName(panel, "Local OS diagnostics") === null)
        verify(findAccessibleByName(panel, "Data directory") === null)
        verify(!hasVisibleText(panel, "Data directory"))

        mouseClick(showLocalPaths,
                   showLocalPaths.width / 2,
                   showLocalPaths.height / 2)

        tryCompare(model, "storageLocalDiagnosticsEnabled", true)
        compare(model.storageDisplayPath("/tmp/legacy-storage-data"),
                "/tmp/legacy-storage-data")
        verify(findAccessibleByName(panel, "Data directory") === null)
        verify(!hasVisibleText(panel, "Data directory"))
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible
                && String(item.Accessible.name || "") === expectedName
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

    function hasVisibleText(item, expectedText) {
        if (!item) {
            return false
        }
        if (item.text !== undefined
                && String(item.text) === expectedText
                && item.visible) {
            return true
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            if (hasVisibleText(children[i], expectedText)) {
                return true
            }
        }
        return false
    }
}
