pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../theme"

Item {
    id: root

    required property Theme theme
    property string text: ""
    property string copyText: text
    property string tooltipText: ""
    property bool monospace: true
    property color textColor: theme.text
    property int textPixelSize: theme.dataText
    property int textWeight: Font.Normal

    Layout.fillWidth: true
    implicitHeight: Math.max(copyLineText.implicitHeight, copyButton.implicitHeight)

    Row {
        id: copyLineRow

        width: root.width
        spacing: root.theme.gapTiny

        Text {
            id: copyLineText

            text: root.text
            width: Math.min(implicitWidth, Math.max(80, root.width - copyButton.implicitWidth - copyLineRow.spacing))
            color: root.textColor
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            font.family: root.monospace ? "monospace" : ""
            font.pixelSize: root.textPixelSize
            font.weight: root.textWeight

            MouseArea {
                id: copyLineHover

                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.NoButton
            }

            ToolTip.visible: copyLineHover.containsMouse && root.tooltipText.length > 0
            ToolTip.text: root.tooltipText
        }

        ToolButton {
            id: copyButton

            visible: root.copyText.length > 0
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
        id: copyBuffer

        visible: false
        text: root.copyText
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.text

    function copyToClipboard() {
        copyBuffer.text = root.copyText
        copyBuffer.forceActiveFocus()
        copyBuffer.selectAll()
        copyBuffer.copy()
        copyBuffer.deselect()
        root.forceActiveFocus()
    }
}
