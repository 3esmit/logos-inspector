import QtQuick
import QtQuick.Controls.Basic
import "../theme"

Button {
    id: root

    required property Theme theme
    property bool primary: false
    property bool selected: false
    property bool iconOnly: false
    property string iconName: ""
    property string accessibleName: text

    implicitHeight: theme.controlHeight
    padding: 10
    hoverEnabled: true

    contentItem: Item {
        implicitWidth: root.iconOnly ? 18 : label.implicitWidth
        implicitHeight: 18

        Text {
            id: label

            visible: !root.iconOnly
            anchors.fill: parent
            text: root.text
            color: root.contentColor()
            elide: Text.ElideRight
            textFormat: Text.PlainText
            verticalAlignment: Text.AlignVCenter
            horizontalAlignment: Text.AlignHCenter
            font.pixelSize: 14
            font.weight: root.primary ? Font.DemiBold : Font.Medium
        }

        Item {
            visible: root.iconOnly && root.iconName === "search"
            width: 18
            height: 18
            anchors.centerIn: parent
            Accessible.ignored: true

            Rectangle {
                x: 3
                y: 2
                width: 10
                height: 10
                radius: width / 2
                color: "transparent"
                border.width: 2
                border.color: root.contentColor()
            }

            Rectangle {
                x: 12
                y: 11
                width: 2
                height: 7
                radius: 1
                color: root.contentColor()
                transform: Rotation {
                    origin.x: 1
                    origin.y: 0
                    angle: -45
                }
            }
        }

        Text {
            visible: root.iconOnly && (root.iconName === "back" || root.iconName === "forward")
            anchors.fill: parent
            text: root.iconName === "forward" ? ">" : "<"
            color: root.contentColor()
            textFormat: Text.PlainText
            horizontalAlignment: Text.AlignHCenter
            verticalAlignment: Text.AlignVCenter
            font.family: "monospace"
            font.pixelSize: 17
            font.weight: Font.DemiBold
            Accessible.ignored: true
        }
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

    function contentColor() {
        if (!root.enabled) {
            return root.theme.textDim
        }
        return root.primary ? root.theme.selectedText : root.theme.text
    }
}
