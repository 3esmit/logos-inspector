pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../../components"
import "../../../../state"
import "../../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var rows: []
    property AppModel modelRef

    spacing: 6
    Layout.fillWidth: true

    Text {
        text: qsTr("Sequencer programs")
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.theme.primaryText
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    Frame {
        padding: 0
        Layout.fillWidth: true

        background: Rectangle {
            color: root.theme.field
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: 0

            ProgramRow {
                theme: root.theme
                header: true
                programIdText: qsTr("Known program ID")
                knownIdl: qsTr("Known IDL")
                binaryMatch: qsTr("Binary match")
                recentTx: qsTr("Recent tx")
                source: qsTr("Source")
            }

            Repeater {
                model: root.rows

                ProgramRow {
                    required property var modelData

                    theme: root.theme
                    label: String(modelData.label || "-")
                    hex: String(modelData.hex || "")
                    base58: String(modelData.base58 || "")
                    programIdText: String(modelData.programIdText || modelData.base58 || modelData.hex || "")
                    knownIdl: String(modelData.knownIdl || qsTr("none"))
                    binaryMatch: qsTr("unknown")
                    recentTx: qsTr("not loaded")
                    source: qsTr("sequencer")
                    modelRef: root.modelRef
                }
            }
        }
    }

    function shortHash(value) {
        const text = String(value || "")
        if (text.length <= 16) {
            return text.length ? text : "-"
        }
        return text.slice(0, 8) + "..." + text.slice(-6)
    }

    component ProgramRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string hex: ""
        property string base58: ""
        property string programIdText: ""
        property string knownIdl: ""
        property string binaryMatch: ""
        property string recentTx: ""
        property string source: ""
        property bool header: false
        property AppModel modelRef

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 34 : 42

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: rowRoot.theme.gap
            anchors.rightMargin: rowRoot.theme.gap
            columns: 5
            columnSpacing: rowRoot.theme.gap

            LinkCell {
                theme: rowRoot.theme
                text: rowRoot.header ? rowRoot.programIdText : root.shortHash(rowRoot.programIdText)
                header: rowRoot.header
                link: !rowRoot.header && (rowRoot.hex.length > 0 || rowRoot.base58.length > 0)
                copyText: rowRoot.base58.length ? rowRoot.base58 : rowRoot.hex
                monospace: !rowRoot.header
                Layout.fillWidth: true
                onActivated: {
                    if (rowRoot.modelRef !== null) {
                        rowRoot.modelRef.openReference("program", rowRoot.hex.length ? rowRoot.hex : rowRoot.base58)
                    }
                }
            }

            LinkCell {
                theme: rowRoot.theme
                text: rowRoot.knownIdl
                header: rowRoot.header
                monospace: false
                Layout.preferredWidth: 120
            }

            LinkCell {
                theme: rowRoot.theme
                text: rowRoot.binaryMatch
                header: rowRoot.header
                monospace: false
                Layout.preferredWidth: 110
            }

            LinkCell {
                theme: rowRoot.theme
                text: rowRoot.recentTx
                header: rowRoot.header
                monospace: !rowRoot.header
                Layout.preferredWidth: 96
            }

            LinkCell {
                theme: rowRoot.theme
                text: rowRoot.source
                header: rowRoot.header
                monospace: false
                Layout.preferredWidth: 92
            }
        }
    }
}
