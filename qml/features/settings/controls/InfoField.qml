pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string label: ""
    property string value: ""

    spacing: 6
    Layout.fillWidth: true

    Text {
        text: root.label
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.Medium
        Layout.fillWidth: true
    }

    Rectangle {
        color: root.theme.field
        radius: root.theme.radius
        border.width: 1
        border.color: root.theme.outlineMuted
        Layout.fillWidth: true
        Layout.preferredHeight: root.theme.controlHeight

        Text {
            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            text: root.value.length ? root.value : "-"
            color: root.theme.text
            textFormat: Text.PlainText
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            font.family: "monospace"
            font.pixelSize: root.theme.primaryText
        }
    }
}
