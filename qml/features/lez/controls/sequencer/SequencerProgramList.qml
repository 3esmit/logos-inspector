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

    spacing: root.theme.gapSmall
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
            color: root.theme.surface
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: 0

            SequencerProgramRow {
                theme: root.theme
                header: true
                columns: [qsTr("Label"), qsTr("Program ID"), qsTr("Base58")]
            }

            Repeater {
                model: root.rows

                SequencerProgramRow {
                    required property var modelData

                    theme: root.theme
                    columns: [
                        String(modelData.label || "-"),
                        root.shortHash(modelData.hex),
                        root.shortHash(modelData.base58)
                    ]
                    program: String(modelData.hex || modelData.base58 || "")
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

    component SequencerProgramRow: Item {
        id: programRow

        required property Theme theme
        property var columns: []
        property string program: ""
        property bool header: false
        property AppModel modelRef

        Layout.fillWidth: true
        Layout.preferredHeight: programRow.header ? 36 : 42

        Rectangle {
            anchors.fill: parent
            color: programRow.header ? programRow.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 3
            columnSpacing: 10

            Repeater {
                model: 3

                LinkCell {
                    required property int index

                    theme: programRow.theme
                    text: String(programRow.columns[index] || "-")
                    header: programRow.header
                    link: !programRow.header && index > 0 && programRow.program.length > 0
                    copyText: programRow.program.length > 0 ? programRow.program : String(programRow.columns[index] || "")
                    monospace: index > 0 && !programRow.header
                    Layout.preferredWidth: index === 0 ? 160 : 180
                    Layout.fillWidth: index > 0
                    onActivated: {
                        if (programRow.modelRef !== null) {
                            programRow.modelRef.openReference("program", programRow.program)
                        }
                    }
                }
            }
        }
    }
}
