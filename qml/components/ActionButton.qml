import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

Button {
    id: root

    required property Theme theme
    property bool primary: false
    property bool selected: false

    implicitHeight: theme.controlHeight
    padding: 10
    hoverEnabled: true

    contentItem: Text {
        text: root.text
        color: root.enabled
            ? (root.primary ? "#21160F" : theme.text)
            : theme.textDim
        elide: Text.ElideRight
        textFormat: Text.PlainText
        verticalAlignment: Text.AlignVCenter
        horizontalAlignment: Text.AlignHCenter
        font.pixelSize: 14
        font.weight: root.primary ? Font.DemiBold : Font.Medium
    }

    background: Rectangle {
        radius: theme.radius
        color: root.primary
            ? (root.enabled ? theme.accent : theme.outlineMuted)
            : (root.selected ? theme.accentMuted : (root.hovered ? theme.hover : theme.surfaceRaised))
        border.width: 1
        border.color: root.selected || root.primary ? theme.accent : theme.outline
    }

    Accessible.role: Accessible.Button
    Accessible.name: root.text
}
