import QtQuick
import QtQuick.Controls.Basic
import "../../../qml/features/zones/pages"
import "../../../qml/theme"

Window {
    id: window

    readonly property string visualTab: argumentValue("--tab", "overview")
    readonly property string outputPath: argumentValue("--out", "")
    readonly property real scrollOffset: Number(argumentValue("--scroll",
        visualTab === "evidence" ? "400" : "0"))

    width: Number(argumentValue("--width", "1440"))
    height: Number(argumentValue("--height", "900"))
    visible: true
    color: theme.background
    title: qsTr("Zones visual fixture")

    Theme {
        id: theme
    }

    ZoneStateFixture {
        id: zoneState

        Component.onCompleted: {
            if (window.visualTab === "evidence") {
                loadEvidence("all")
                openEvidence(evidenceRows[2])
            }
        }
    }

    QtObject {
        id: appModel

        property var zoneInspection: zoneState
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

            ZonesPage {
                theme: theme
                model: appModel
                initialDetailTab: window.visualTab
                sourceEditorInitiallyOpen: window.visualTab === "sources"
                width: parent ? parent.width : 1200
            }
        }
    }

    Timer {
        interval: 250
        running: window.outputPath.length > 0
        repeat: false
        onTriggered: {
            if (visualScroll.contentItem) {
                visualScroll.contentItem.contentY = window.scrollOffset
            }
            captureTimer.start()
        }
    }

    Timer {
        id: captureTimer

        interval: 150
        repeat: false
        onTriggered: {
            captureRoot.grabToImage(function (result) {
                if (!result.saveToFile(window.outputPath)) {
                    console.error("failed to save Zones visual fixture")
                    Qt.exit(2)
                    return
                }
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
