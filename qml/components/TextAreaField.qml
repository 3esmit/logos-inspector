import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string label: ""
    property int rows: 5
    property alias text: field.text
    property alias placeholderText: field.placeholderText

    spacing: 6
    Layout.fillWidth: true

    Text {
        text: root.label
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.Medium
        Layout.fillWidth: true
    }

    TextArea {
        id: field
        wrapMode: TextArea.Wrap
        color: root.theme.text
        placeholderTextColor: root.theme.textDim
        selectionColor: root.theme.accent
        selectedTextColor: root.theme.selectedText
        font.family: "monospace"
        font.pixelSize: root.theme.secondaryText
        leftPadding: 12
        rightPadding: 12
        topPadding: 10
        bottomPadding: 10
        hoverEnabled: true
        Layout.fillWidth: true
        Layout.preferredHeight: Math.max(96, root.rows * 24)

        background: Rectangle {
            radius: root.theme.radius
            color: field.hovered || field.activeFocus ? root.theme.surfaceRaised : root.theme.field
            border.width: field.activeFocus ? 2 : 1
            border.color: field.activeFocus ? root.theme.accent : root.theme.outlineMuted
        }

        Accessible.name: root.label.length > 0 ? root.label : root.placeholderText
    }
}
