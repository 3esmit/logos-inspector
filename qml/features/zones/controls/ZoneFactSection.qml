pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property var rows: []

    spacing: root.theme.gapSmall
    Layout.fillWidth: true
    Layout.minimumWidth: 260

    Text {
        text: root.title
        color: root.theme.text
        textFormat: Text.PlainText
        elide: Text.ElideRight
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    Rectangle {
        color: root.theme.outlineMuted
        Layout.fillWidth: true
        Layout.preferredHeight: 1
    }

    Repeater {
        model: root.rows

        ZoneFactRow {
            required property var modelData

            theme: root.theme
            label: String(modelData.label || "")
            value: String(modelData.value === undefined || modelData.value === null ? "-" : modelData.value)
            tone: String(modelData.tone || "neutral")
            copyable: modelData.copyable === true
            monospace: modelData.monospace === true
            fitSingleLine: modelData.fitSingleLine === undefined
                ? copyable || value.length > 38
                : modelData.fitSingleLine === true
        }
    }
}
