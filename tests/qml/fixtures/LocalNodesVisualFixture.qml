import QtQuick
import QtQuick.Controls.Basic
import "../../../qml/services"
import "../../../qml/state"
import "../../../qml/features/local/pages"
import "../../../qml/theme"

Window {
    id: window

    readonly property string outputPath: argumentValue("--out", "/tmp/local-nodes-stopped-service-evidence.png")
    width: Number(argumentValue("--width", "1280"))
    height: Number(argumentValue("--height", "960"))
    visible: true
    color: theme.background
    title: qsTr("Local Nodes visual fixture")

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

        ScrollView {
            id: visualScroll

            anchors.fill: parent
            leftPadding: theme.pageMargin
            rightPadding: theme.pageMargin
            topPadding: theme.gapLarge
            bottomPadding: theme.gapLarge
            contentWidth: availableWidth
            ScrollBar.horizontal.policy: ScrollBar.AlwaysOff

            LocalNodesPage {
                id: page

                theme: theme
                model: appModel.localNodes
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
                    console.error("failed to save Local Nodes visual fixture")
                    Qt.exit(2)
                    return
                }
                console.log("Saved Local Nodes visual fixture to " + window.outputPath)
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
