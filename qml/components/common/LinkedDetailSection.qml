pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import ".."
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property string title: ""
    property var rows: []
    property int labelWidth: 132

    signal linkActivated(string kind, string value)

    visible: root.rows.length > 0
    spacing: 6
    Layout.fillWidth: true

    Text {
        visible: root.title.length > 0
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

                LinkedDetailRow {
                    required property var modelData

                    theme: root.theme
                    label: String(modelData.label || "")
                    value: String(modelData.value || "-")
                    linkKind: String(modelData.linkKind || "")
                    labelWidth: root.labelWidth
                    onActivated: root.linkActivated(linkKind, value)
                }
            }
        }
    }

    component LinkedDetailRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string linkKind: ""
        property int labelWidth: 132

        signal activated()

        Layout.fillWidth: true
        implicitHeight: Math.max(40, rowGrid.implicitHeight + rowRoot.theme.gapLarge)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: rowRoot.theme.gap
            anchors.rightMargin: rowRoot.theme.gap
            anchors.topMargin: rowRoot.theme.gapSmall
            anchors.bottomMargin: rowRoot.theme.gapSmall
            columns: 2
            columnSpacing: rowRoot.theme.gap

            Text {
                text: rowRoot.label
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: rowRoot.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: rowRoot.labelWidth
            }

            LinkCell {
                theme: rowRoot.theme
                text: rowRoot.value
                link: rowRoot.linkKind.length > 0
                copyText: rowRoot.value
                wrap: true
                Layout.fillWidth: true
                onActivated: rowRoot.activated()
            }
        }
    }
}
