pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property int value: 30
    property string accessibleName: qsTr("Auto refresh")
    property string accessibleDescription: qsTr("Automatic refresh interval in seconds. Set to 0 to turn it off.")
    signal rateEdited(int value)

    spacing: 6
    Layout.fillWidth: true

    Text {
        text: qsTr("Auto refresh")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.Medium
        Layout.fillWidth: true
    }

    SpinBox {
        id: refreshSpin

        from: 0
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
            return value === 0 ? qsTr("Off") : qsTr("%1 s").arg(Number(value).toLocaleString(locale, "f", 0))
        }
        valueFromText: function (text, locale) {
            const parsed = Number(String(text || "").replace(/[^0-9]/g, ""))
            if (!Number.isFinite(parsed) || parsed === 0) {
                return 0
            }
            return Math.max(5, Math.min(3600, parsed))
        }
        onValueModified: root.rateEdited(value === 0 ? 0 : Math.max(5, value))

        contentItem: TextInput {
            text: refreshSpin.textFromValue(refreshSpin.value, refreshSpin.locale)
            color: root.theme.text
            selectionColor: root.theme.accent
            selectedTextColor: root.theme.selectedText
            font.pixelSize: root.theme.primaryText
            horizontalAlignment: Qt.AlignHCenter
            verticalAlignment: Qt.AlignVCenter
            readOnly: !refreshSpin.editable
            validator: refreshSpin.validator
            inputMethodHints: Qt.ImhDigitsOnly
        }

        background: Rectangle {
            radius: root.theme.radius
            color: refreshSpin.hovered || refreshSpin.activeFocus ? root.theme.surfaceRaised : root.theme.field
            border.width: refreshSpin.activeFocus ? 2 : 1
            border.color: refreshSpin.activeFocus ? root.theme.accent : root.theme.outlineMuted
        }
    }
}
