import QtQuick
import QtQuick.Controls.Basic
import "../../../qml/services"
import "../../../qml/state"
import "../../../qml/features/settings/pages"
import "../../../qml/theme"

Window {
    id: window

    readonly property string outputPath: argumentValue("--out", "/tmp/logoscore-settings-evidence.png")
    width: Number(argumentValue("--width", "1280"))
    height: Number(argumentValue("--height", "960"))
    visible: true
    color: theme.background
    title: qsTr("Settings visual fixture")

    Theme {
        id: theme
    }

    BridgeClient {
        id: bridgeClient
    }

    AppModel {
        id: appModel

        bridge: bridgeClient

        Component.onCompleted: {
            logoscoreHome = "/opt/logos-node"
            logoscoreModulesDir = "/opt/logos-node/modules"
            localNodesEnabled = true
            localDevnetEnabled = false
        }
    }

    Rectangle {
        id: captureRoot

        anchors.fill: parent
        color: theme.background

        ScrollView {
            id: visualScroll

            anchors.fill: parent
            leftPadding: theme.pageMargin
            rightPadding: theme.pageMargin
            topPadding: theme.gapLarge
            bottomPadding: theme.gapLarge
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            SettingsPage {
                id: settingsPage

                theme: theme
                model: appModel
                width: parent ? parent.width : 1200
            }
        }
    }

    Timer {
        interval: 400
        running: true
        repeat: false
        onTriggered: {
            captureRoot.grabToImage(function (result) {
                if (!result.saveToFile(window.outputPath)) {
                    console.error("failed to save Settings visual fixture")
                    Qt.exit(2)
                    return
                }
                console.log("Saved Settings visual fixture to " + window.outputPath)
                Qt.quit()
            }, Qt.size(window.width, window.height))
        }
    }

    function argumentValue(name, fallback) {
        const args = Qt.application.arguments || []
        for (let i = 0; i < args.length - 1; ++i) {
            if (args[i] === name) {
                return String(args[i + 1])
            }
        }
        return fallback
    }
}
