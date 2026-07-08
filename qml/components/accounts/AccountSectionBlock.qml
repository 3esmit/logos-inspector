pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../state"
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property var rows: []
    property AppModel modelRef

    visible: root.rows.length > 0
    spacing: 6
    Layout.fillWidth: true

    Text {
        visible: root.title.length > 0
        text: root.title
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: 14
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    Frame {
        padding: 0
        Layout.fillWidth: true

        background: Rectangle {
            color: root.theme.surface
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: 0

            Repeater {
                model: root.rows

                AccountDetailRow {
                    required property var modelData

                    theme: root.theme
                    label: String(modelData.label || "")
                    value: String(modelData.value || "-")
                    subvalue: String(modelData.subvalue || "")
                    subvalueCopyText: String(modelData.subvalueCopyText || "")
                    linkKind: String(modelData.linkKind || "")
                    linkValue: root.modelRef ? root.modelRef.valueToString(modelData.linkValue) : String(modelData.linkValue || "")
                    tooltipText: String(modelData.tooltipText || "")
                    monospace: modelData.monospace !== undefined ? modelData.monospace : true
                    onActivated: {
                        if (root.modelRef !== null) {
                            root.modelRef.entityNavigation.openReference(modelData.linkKind, modelData.linkValue)
                        }
                    }
                }
            }
        }
    }
}
