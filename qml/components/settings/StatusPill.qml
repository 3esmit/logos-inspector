pragma ComponentBehavior: Bound

import QtQuick
import "../../theme"

Rectangle {
    id: root

    required property Theme theme
    property string text: ""
    property color colorToken: theme.textMuted

    radius: root.theme.radius
    color: root.fillColor()
    border.width: 1
    border.color: root.colorToken
    implicitWidth: pillText.implicitWidth + 18
    implicitHeight: 26

    Text {
        id: pillText

        anchors.centerIn: parent
        text: root.text.length ? root.text : qsTr("Unknown")
        color: root.textColor()
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        font.weight: Font.DemiBold
    }

    function fillColor() {
        if (root.colorToken === root.theme.success) {
            return root.theme.successMuted
        }
        if (root.colorToken === root.theme.warning) {
            return root.theme.warningMuted
        }
        if (root.colorToken === root.theme.error) {
            return root.theme.errorMuted
        }
        if (root.colorToken === root.theme.info) {
            return root.theme.infoMuted
        }
        return root.theme.field
    }

    function textColor() {
        if (root.colorToken === root.theme.textMuted) {
            return root.theme.textMuted
        }
        return root.theme.text
    }
}
