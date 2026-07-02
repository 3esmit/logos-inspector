import QtQuick
import QtQuick.Controls.Basic
import "../theme"

Button {
    id: root

    required property Theme theme
    property bool primary: false
    property bool selected: false
    property string accessibleName: text

    implicitHeight: theme.controlHeight
    padding: 10
    hoverEnabled: true

    contentItem: Text {
        text: root.text
        color: root.enabled
            ? (root.primary ? root.theme.selectedText : root.theme.text)
            : root.theme.textDim
        elide: Text.ElideRight
        textFormat: Text.PlainText
        verticalAlignment: Text.AlignVCenter
        horizontalAlignment: Text.AlignHCenter
        font.pixelSize: 14
        font.weight: root.primary ? Font.DemiBold : Font.Medium
    }

    background: Rectangle {
        radius: root.theme.radius
        color: root.primary
            ? (root.enabled ? (root.down ? root.theme.accentHover : root.theme.accent) : root.theme.outlineMuted)
            : (root.selected ? root.theme.accentMuted : (root.hovered ? root.theme.hover : root.theme.surfaceRaised))
        border.width: root.activeFocus ? 2 : 1
        border.color: !root.enabled ? root.theme.outlineMuted : (root.selected || root.primary ? root.theme.accent : root.theme.outline)
    }

    Accessible.role: Accessible.Button
    Accessible.name: root.accessibleName
}
