import QtQuick
import QtQuick.Controls.Basic
import QtQml
import QtQuick.Layouts
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string label: ""
    property alias text: field.text
    property string sourceText: ""
    property bool syncSourceText: false
    property string errorMessage: ""
    readonly property bool invalid: errorMessage.length > 0
    property alias placeholderText: field.placeholderText
    signal textEdited(string text)

    function syncFromSourceText() {
        if (root.syncSourceText && !field.activeFocus && field.text !== root.sourceText) {
            field.text = root.sourceText
        }
    }

    onSourceTextChanged: syncFromSourceText()
    onSyncSourceTextChanged: syncFromSourceText()

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

    TextField {
        id: field

        objectName: root.objectName.length > 0 ? root.objectName + "Input" : ""
        color: root.theme.text
        placeholderTextColor: root.theme.textDim
        selectionColor: root.theme.accent
        selectedTextColor: root.theme.selectedText
        font.pixelSize: root.theme.primaryText
        leftPadding: 12
        rightPadding: 12
        hoverEnabled: true
        Layout.fillWidth: true
        Layout.preferredHeight: root.theme.controlHeight
        Component.onCompleted: root.syncFromSourceText()
        onActiveFocusChanged: root.syncFromSourceText()
        onTextEdited: root.textEdited(text)

        Binding {
            target: field
            property: "text"
            value: root.sourceText
            when: root.syncSourceText && !field.activeFocus
        }

        background: Rectangle {
            radius: root.theme.radius
            color: field.hovered || field.activeFocus ? root.theme.surfaceRaised : root.theme.field
            border.width: root.invalid || field.activeFocus ? 2 : 1
            border.color: root.invalid ? root.theme.error
                : field.activeFocus ? root.theme.accent : root.theme.outlineMuted
        }

        Accessible.name: root.label.length > 0 ? root.label : root.placeholderText
        Accessible.description: root.invalid
            ? qsTr("Error: %1").arg(root.errorMessage) : ""
    }
}
