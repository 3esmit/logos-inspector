pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import ".."
import "../../state"
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property var rows: []
    property AppModel modelRef

    visible: root.rows.length > 0
    spacing: 6
    Layout.fillWidth: true

    Text {
        visible: root.title.length > 0
        text: root.title
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

            Repeater {
                model: root.rows

                SequencerDetailRow {
                    required property var modelData

                    theme: root.theme
                    label: String(modelData.label || "")
                    value: String(modelData.value || "-")
                    linkKind: String(modelData.linkKind || "")
                    linkValue: root.modelRef ? root.modelRef.valueToString(modelData.linkValue) : String(modelData.linkValue || "")
                    monospace: modelData.monospace !== undefined ? modelData.monospace : true
                    modelRef: root.modelRef
                }
            }
        }
    }

    component SequencerDetailRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string linkKind: ""
        property string linkValue: ""
        property bool monospace: true
        property AppModel modelRef

        Layout.fillWidth: true
        implicitHeight: Math.max(42, rowGrid.implicitHeight + 18)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            columns: 2
            columnSpacing: 14
            rowSpacing: 3

            Text {
                text: rowRoot.label
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: rowRoot.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 128
                Layout.alignment: Qt.AlignTop
            }

            LinkCell {
                text: rowRoot.value
                theme: rowRoot.theme
                link: rowRoot.linkKind.length > 0 && rowRoot.linkValue.length > 0 && rowRoot.linkValue !== "-"
                copyText: rowRoot.linkValue.length > 0 ? rowRoot.linkValue : rowRoot.value
                monospace: rowRoot.monospace
                wrap: true
                Layout.fillWidth: true
                onActivated: {
                    if (rowRoot.modelRef !== null) {
                        rowRoot.modelRef.openReference(rowRoot.linkKind, rowRoot.linkValue)
                    }
                }
            }
        }
    }
}
