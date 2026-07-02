import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

Frame {
    id: root

    required property Theme theme
    property string title: ""
    default property alias content: body.data

    padding: theme.gapLarge
    Layout.fillWidth: true

    background: Rectangle {
        color: root.theme.surface
        radius: root.theme.radiusLarge
        border.width: 1
        border.color: root.theme.outlineMuted
    }

    contentItem: ColumnLayout {
        spacing: root.theme.gap

        Text {
            visible: root.title.length > 0
            text: root.title
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.panelTitleText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        ColumnLayout {
            id: body
            spacing: root.theme.gap
            Layout.fillWidth: true
        }
    }
}
