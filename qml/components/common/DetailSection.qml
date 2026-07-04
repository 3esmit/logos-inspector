pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property var rows: []
    property int labelWidth: 132
    property color surfaceColor: theme.surface
    property int titlePixelSize: theme.primaryText
    signal linkActivated(string kind, string value)

    visible: root.rows.length > 0
    spacing: 6
    Layout.fillWidth: true

    Text {
        visible: root.title.length > 0
        text: root.title
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.titlePixelSize
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    Frame {
        padding: 0
        Layout.fillWidth: true

        background: Rectangle {
            color: root.surfaceColor
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: 0

            Repeater {
                model: root.rows

                DetailValueRow {
                    required property var modelData

                    theme: root.theme
                    label: String(modelData.label || "")
                    value: String(modelData.value || "-")
                    subvalue: String(modelData.subvalue || "")
                    linkKind: String(modelData.linkKind || "")
                    linkValue: root.valueToString(modelData.linkValue)
                    copyText: modelData.copyText !== undefined ? String(modelData.copyText || "") : (root.valueToString(modelData.linkValue).length > 0 ? root.valueToString(modelData.linkValue) : String(modelData.value || ""))
                    monospace: modelData.monospaced !== undefined ? modelData.monospaced : (modelData.monospace !== undefined ? modelData.monospace : true)
                    copyable: modelData.copyable !== undefined ? modelData.copyable : String(modelData.linkKind || "").length > 0
                    labelWidth: modelData.labelWidth !== undefined ? Number(modelData.labelWidth) : root.labelWidth
                    onActivated: function (kind, value) {
                        root.linkActivated(kind, value)
                    }
                }
            }
        }
    }

    function valueToString(value) {
        if (value === undefined || value === null) {
            return ""
        }
        return String(value)
    }
}
