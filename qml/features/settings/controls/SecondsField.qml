pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string label: ""
    property int value: 120
    property string accessibleName: label
    property string accessibleDescription: qsTr("Value in seconds.")
    signal valueEdited(int value)

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

    SpinBox {
        id: secondsSpin

        from: 5
        to: 3600
        stepSize: 5
        value: root.value
        editable: true
        hoverEnabled: true
        focusPolicy: Qt.StrongFocus
        Accessible.name: root.accessibleName
        Accessible.description: root.accessibleDescription
        Layout.fillWidth: true
        Layout.preferredHeight: root.theme.controlHeight
        textFromValue: function (value, locale) {
            return qsTr("%1 s").arg(Number(value).toLocaleString(locale, "f", 0))
        }
        valueFromText: function (text, locale) {
            const parsed = Number(String(text || "").replace(/[^0-9]/g, ""))
            return Number.isFinite(parsed) ? parsed : root.value
        }
        onValueModified: root.valueEdited(value)

        contentItem: TextInput {
            text: secondsSpin.textFromValue(secondsSpin.value, secondsSpin.locale)
            color: root.theme.text
            selectionColor: root.theme.accent
            selectedTextColor: root.theme.selectedText
            font.pixelSize: root.theme.primaryText
            horizontalAlignment: Qt.AlignHCenter
            verticalAlignment: Qt.AlignVCenter
            readOnly: !secondsSpin.editable
            validator: secondsSpin.validator
            inputMethodHints: Qt.ImhDigitsOnly
        }

        background: Rectangle {
            radius: root.theme.radius
            color: secondsSpin.hovered || secondsSpin.activeFocus ? root.theme.surfaceRaised : root.theme.field
            border.width: secondsSpin.activeFocus ? 2 : 1
            border.color: secondsSpin.activeFocus ? root.theme.accent : root.theme.outlineMuted
        }
    }
}
