import QtQuick
import QtQuick.Controls.Basic
import "../../../qml"
import "../../../qml/theme"
import "../../../qml/services"
import "../../../qml/state"

Window {
    id: window

    readonly property string outputPath: argumentValue("--out", "/tmp/logoscore-service-management-full-app-evidence.png")
    width: Number(argumentValue("--width", "1280"))
    height: Number(argumentValue("--height", "960"))
    visible: true
    color: theme.background
    title: qsTr("Logos Inspector - Full Standalone App")

    Theme {
        id: theme
    }

    BridgeClient {
        id: bridgeClient
    }

    AppModel {
        id: appModel
        bridge: bridgeClient
    }

    Rectangle {
        id: captureRoot

        anchors.fill: parent
        color: theme.background

        AppShell {
            id: appShell
            anchors.fill: parent
        }
    }

    Timer {
        interval: 500
        running: true
        repeat: false
        onTriggered: {
            captureRoot.grabToImage(function (result) {
                if (!result.saveToFile(window.outputPath)) {
                    console.error("failed to save Full Standalone App visual fixture")
                    Qt.exit(2)
                    return
                }
                console.log("Saved Full Standalone App visual fixture to " + window.outputPath)
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
