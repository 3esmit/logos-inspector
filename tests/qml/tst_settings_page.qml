import QtQuick
import QtTest
import "../../qml/features/settings/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"

Item {
    id: root

    width: 1280
    height: 960

    Theme {
        id: testTheme
    }

    QtObject {
        id: standaloneHost

        function callModuleJson(moduleName, method, argsJson) {
            return JSON.stringify({ ok: true, value: {}, text: "OK", error: "" })
        }
    }

    QtObject {
        id: basecampHost

        function callModule(moduleName, method, args) {
            return JSON.stringify({ ok: true, value: {}, text: "OK", error: "" })
        }
    }

    BridgeClient {
        id: standaloneBridge

        host: standaloneHost
    }

    BridgeClient {
        id: basecampBridge

        host: basecampHost
    }

    AppModel {
        id: standaloneModel

        bridge: standaloneBridge
    }

    AppModel {
        id: basecampModel

        bridge: basecampBridge
    }

    Component {
        id: standalonePageComponent

        SettingsPage {
            theme: testTheme
            model: standaloneModel
            width: root.width
        }
    }

    Component {
        id: basecampPageComponent

        SettingsPage {
            theme: testTheme
            model: basecampModel
            width: root.width
        }
    }

    TestCase {
        name: "SettingsPage"
        when: windowShown

        function test_logoscore_runtime_settings_are_standalone_only() {
            const standalonePage = standalonePageComponent.createObject(root)
            verify(!!standalonePage, "Standalone Settings page created")
            tryVerify(function () {
                return !!findChild(standalonePage, "logoscoreRuntimeConfiguration")
            }, 1000)
            const standaloneRuntime = findChild(standalonePage, "logoscoreRuntimeConfiguration")
            verify(!!standaloneRuntime, "LogosCore runtime panel exists")
            verify(standaloneRuntime.visible, "Standalone exposes LogosCore runtime settings")
            standalonePage.destroy()

            const basecampPage = basecampPageComponent.createObject(root)
            verify(!!basecampPage, "Basecamp Settings page created")
            tryVerify(function () {
                return !!findChild(basecampPage, "logoscoreRuntimeConfiguration")
            }, 1000)
            const basecampRuntime = findChild(basecampPage, "logoscoreRuntimeConfiguration")
            verify(!!basecampRuntime, "Basecamp runtime panel remains addressable for the contract")
            verify(!basecampRuntime.visible, "Basecamp hides standalone LogosCore runtime settings")
            basecampPage.destroy()
        }
    }
}
