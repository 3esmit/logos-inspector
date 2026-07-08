pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import "../../../theme"

CheckBox {
    id: root

    required property Theme theme
    property string detail: ""

    hoverEnabled: true
    implicitWidth: 220
    implicitHeight: 34

    indicator: Rectangle {
        x: 0
        y: (root.height - height) / 2
        width: 18
        height: 18
        radius: 4
        color: root.checked ? root.theme.accent : root.theme.field
        border.width: root.activeFocus ? 2 : 1
        border.color: root.checked ? root.theme.accentHover : root.theme.outline

        Rectangle {
            visible: root.checked
            anchors.centerIn: parent
            width: 8
            height: 8
            radius: 2
            color: root.theme.selectedText
        }
    }

    contentItem: Text {
        text: root.text
        color: root.enabled ? root.theme.text : root.theme.textDim
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        elide: Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        leftPadding: 26
    }

    ToolTip.visible: (hovered || activeFocus) && root.detail.length > 0
    ToolTip.text: root.detail
    Accessible.role: Accessible.CheckBox
    Accessible.name: root.text
    Accessible.description: root.detail
}
