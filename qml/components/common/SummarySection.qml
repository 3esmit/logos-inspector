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

    visible: root.rows.length > 0
    spacing: 6
    Layout.fillWidth: true

    Text {
        text: root.title
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.theme.primaryText
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    Frame {
        padding: 0
        Layout.fillWidth: true

        background: Rectangle {
            color: root.theme.field
            radius: root.theme.radius
            border.width: 1
            border.color: root.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: 0

            Repeater {
                model: root.rows

                SummaryRow {
                    required property var modelData

                    theme: root.theme
                    title: String(modelData.title || "-")
                    detail: String(modelData.detail || "")
                }
            }
        }
    }

    component SummaryRow: Item {
        id: rowRoot

        required property Theme theme
        property string title: ""
        property string detail: ""

        Layout.fillWidth: true
        implicitHeight: Math.max(40, body.implicitHeight + rowRoot.theme.gapLarge)

        ColumnLayout {
            id: body

            anchors.fill: parent
            anchors.leftMargin: rowRoot.theme.gap
            anchors.rightMargin: rowRoot.theme.gap
            anchors.topMargin: rowRoot.theme.gapSmall
            anchors.bottomMargin: rowRoot.theme.gapSmall
            spacing: rowRoot.theme.gapTiny

            Text {
                text: rowRoot.title
                color: rowRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: rowRoot.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            Text {
                visible: rowRoot.detail.length > 0
                text: rowRoot.detail
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: rowRoot.theme.dataText
                wrapMode: Text.WrapAnywhere
                Layout.fillWidth: true
            }
        }
    }
}
