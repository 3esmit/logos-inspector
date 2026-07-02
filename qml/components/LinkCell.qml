import QtQuick
import QtQuick.Controls.Basic
import "../theme"

Control {
    id: root

    required property Theme theme
    property string text: ""
    property bool header: false
    property bool link: false
    property bool monospace: true
    property bool wrap: false
    property color textColor: link ? theme.accent : (header ? theme.textMuted : theme.text)
    signal activated()

    hoverEnabled: link
    activeFocusOnTab: link
    padding: 0
    implicitHeight: content.implicitHeight

    Keys.onPressed: function (event) {
        if (!root.link) {
            return;
        }
        if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
            root.activated();
            event.accepted = true;
        }
    }

    background: Rectangle {
        radius: root.theme.radius
        color: root.hovered && root.link ? root.theme.accentMuted : "transparent"
        border.width: root.activeFocus && root.link ? 1 : 0
        border.color: root.theme.accent
    }

    contentItem: Text {
        id: content

        text: root.text
        color: root.link && root.hovered ? root.theme.accentHover : root.textColor
        textFormat: Text.PlainText
        font.family: root.header || !root.monospace ? "" : "monospace"
        font.pixelSize: root.header ? root.theme.labelText : root.theme.dataText
        font.weight: root.header ? Font.DemiBold : Font.Normal
        font.capitalization: root.header ? Font.AllUppercase : Font.MixedCase
        font.underline: root.link && (root.hovered || root.activeFocus)
        wrapMode: root.wrap ? Text.WrapAnywhere : Text.NoWrap
        elide: root.wrap ? Text.ElideNone : Text.ElideRight
        verticalAlignment: Text.AlignVCenter
        leftPadding: root.link ? root.theme.gapTiny : 0
        rightPadding: root.link ? root.theme.gapTiny : 0
    }

    MouseArea {
        anchors.fill: parent
        enabled: root.link
        cursorShape: Qt.PointingHandCursor
        onClicked: root.activated()
    }

    Accessible.role: root.link ? Accessible.Link : Accessible.StaticText
    Accessible.name: root.text
}
