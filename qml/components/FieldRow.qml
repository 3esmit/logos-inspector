import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string label: ""
    property alias text: field.text
    property alias placeholderText: field.placeholderText

    spacing: 6
    Layout.fillWidth: true

    Text {
        text: root.label
        color: theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: 13
        font.weight: Font.Medium
        Layout.fillWidth: true
    }

    TextField {
        id: field
        color: theme.text
        placeholderTextColor: theme.textDim
        selectionColor: theme.accent
        selectedTextColor: "#21160F"
        font.pixelSize: 14
        leftPadding: 12
        rightPadding: 12
        Layout.fillWidth: true
        Layout.preferredHeight: theme.controlHeight

        background: Rectangle {
            radius: theme.radius
            color: theme.field
            border.width: 1
            border.color: field.activeFocus ? theme.accent : theme.outline
        }
    }
}
