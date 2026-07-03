import QtQuick
import QtQuick.Controls.Basic
import "../theme"

Control {
    id: root

    required property Theme theme
    property string text: ""
    property bool header: false
    property bool link: false
    property bool copyable: link
    property bool copyInline: copyable
    property bool monospace: true
    property bool wrap: false
    property string copyText: text
    property string tooltipText: ""
    property int textPixelSize: header ? theme.labelText : theme.dataText
    property int textWeight: header ? Font.DemiBold : Font.Normal
    property color textColor: link ? theme.accent : (header ? theme.textMuted : theme.text)
    signal activated()

    hoverEnabled: link || copyable || tooltipText.length > 0
    activeFocusOnTab: link
    padding: 0
    implicitHeight: contentRow.implicitHeight
    implicitWidth: contentRow.implicitWidth

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

    contentItem: Row {
        id: contentRow

        spacing: root.link || root.copyable ? root.theme.gapTiny : 0

        Text {
            id: content

            text: root.text
            color: root.link && root.hovered ? root.theme.accentHover : root.textColor
            textFormat: Text.PlainText
            font.family: root.header || !root.monospace ? "" : "monospace"
            font.pixelSize: root.textPixelSize
            font.weight: root.textWeight
            font.capitalization: root.header ? Font.AllUppercase : Font.MixedCase
            font.underline: root.link && (root.hovered || root.activeFocus)
            wrapMode: root.wrap ? Text.WrapAnywhere : Text.NoWrap
            elide: root.wrap ? Text.ElideNone : Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            leftPadding: root.link ? root.theme.gapTiny : 0
            rightPadding: root.link ? root.theme.gapTiny : 0
            width: root.textWidth(content.implicitWidth, copyButton.visible ? copyButton.implicitWidth + contentRow.spacing : 0)
            height: Math.max(implicitHeight, copyButton.visible ? copyButton.implicitHeight : 0)

            MouseArea {
                anchors.fill: parent
                enabled: root.link
                cursorShape: Qt.PointingHandCursor
                onClicked: root.activated()
            }
        }

        ToolButton {
            id: copyButton

            visible: root.copyable && root.copyText.length > 0 && !root.header
            hoverEnabled: true
            focusPolicy: Qt.TabFocus
            width: 26
            height: 26
            padding: 0
            onClicked: root.copyToClipboard()

            ToolTip.visible: hovered
            ToolTip.delay: 500
            ToolTip.text: qsTr("Copy")

            background: Rectangle {
                radius: root.theme.radius
                color: copyButton.down ? root.theme.accentMuted : (copyButton.hovered || copyButton.activeFocus ? root.theme.hover : "transparent")
                border.width: copyButton.activeFocus ? 1 : 0
                border.color: root.theme.accent
            }

            contentItem: Item {
                Rectangle {
                    x: 7
                    y: 5
                    width: 10
                    height: 12
                    radius: 2
                    color: "transparent"
                    border.width: 1
                    border.color: copyButton.hovered || copyButton.activeFocus ? root.theme.accentHover : root.theme.textMuted
                }

                Rectangle {
                    x: 10
                    y: 8
                    width: 10
                    height: 12
                    radius: 2
                    color: root.theme.surface
                    border.width: 1
                    border.color: copyButton.hovered || copyButton.activeFocus ? root.theme.accentHover : root.theme.textMuted
                }
            }

            Accessible.role: Accessible.Button
            Accessible.name: qsTr("Copy %1").arg(root.text)
        }
    }

    TextArea {
        id: clipboardBuffer

        visible: false
        text: root.copyText
    }

    ToolTip.visible: root.hovered && root.tooltipText.length > 0
    ToolTip.text: root.tooltipText

    Accessible.role: root.link ? Accessible.Link : Accessible.StaticText
    Accessible.name: root.text

    function copyToClipboard() {
        clipboardBuffer.text = root.copyText
        clipboardBuffer.forceActiveFocus()
        clipboardBuffer.selectAll()
        clipboardBuffer.copy()
        clipboardBuffer.deselect()
        root.forceActiveFocus()
    }

    function textWidth(implicitTextWidth, copyGap) {
        const available = root.width > 0 ? Math.max(80, root.width - copyGap) : implicitTextWidth
        if (root.copyInline) {
            return Math.min(implicitTextWidth, available)
        }
        return root.width > 0 ? available : implicitTextWidth
    }
}
