import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

Frame {
    id: root

    required property Theme theme
    property string title: ""
    default property alias content: body.data

    padding: 16
    Layout.fillWidth: true

    background: Rectangle {
        color: theme.surface
        radius: theme.radius
        border.width: 1
        border.color: theme.outlineMuted
    }

    contentItem: ColumnLayout {
        spacing: 12

        Text {
            visible: root.title.length > 0
            text: root.title
            color: theme.text
            textFormat: Text.PlainText
            font.pixelSize: 18
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        ColumnLayout {
            id: body
            spacing: 12
            Layout.fillWidth: true
        }
    }
}
