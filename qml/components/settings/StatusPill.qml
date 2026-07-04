pragma ComponentBehavior: Bound

import QtQuick
import "../../theme"

Rectangle {
    id: root

    required property Theme theme
    property string text: ""
    property color colorToken: theme.textMuted

    radius: root.theme.radius
    color: root.colorToken === root.theme.success ? root.theme.successMuted : (root.colorToken === root.theme.warning ? root.theme.warningMuted : root.theme.field)
    border.width: 1
    border.color: root.colorToken
    implicitWidth: pillText.implicitWidth + 18
    implicitHeight: 26

    Text {
        id: pillText

        anchors.centerIn: parent
        text: root.text.length ? root.text : qsTr("Unknown")
        color: root.colorToken === root.theme.textMuted ? root.theme.textMuted : root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        font.weight: Font.DemiBold
    }
}
