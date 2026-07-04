pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../theme"

Rectangle {
    id: root

    required property Theme theme
    property string timeText: ""
    property string labelText: ""
    property string statusText: ""
    property string detailText: ""

    radius: root.theme.radius
    color: root.theme.field
    border.width: 1
    border.color: root.statusText === "error" ? root.theme.error : root.theme.outlineMuted
    implicitHeight: 62
    Layout.fillWidth: true

    GridLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        columns: 4
        columnSpacing: root.theme.gap

        Text {
            text: root.timeText
            color: root.theme.textDim
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.theme.dataText
            Layout.preferredWidth: 64
        }

        Text {
            text: root.labelText
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.Medium
            elide: Text.ElideRight
            Layout.preferredWidth: 150
        }

        Text {
            text: root.statusText
            color: root.statusText === "error" ? root.theme.error : root.theme.success
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.preferredWidth: 56
        }

        Text {
            text: root.detailText
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }
}
