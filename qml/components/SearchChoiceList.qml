pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var rows: []

    signal activated(int index, var row)

    spacing: 0

    Repeater {
        model: root.rows

        RowLayout {
            id: rowRoot

            required property int index
            required property var modelData

            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            LayerBadge {
                theme: root.theme
                text: String(rowRoot.modelData.layer || "")
                Layout.preferredWidth: 92
            }

            ColumnLayout {
                spacing: 2
                Layout.fillWidth: true

                Text {
                    text: String(rowRoot.modelData.title || "")
                    color: root.theme.text
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Text {
                    text: String(rowRoot.modelData.value || "")
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    elide: Text.ElideMiddle
                    font.family: "monospace"
                    font.pixelSize: root.theme.dataText
                    Layout.fillWidth: true
                }
            }

            Text {
                text: String(rowRoot.modelData.source || "")
                color: root.theme.textDim
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.dataText
                Layout.preferredWidth: 140
            }

            ActionButton {
                theme: root.theme
                text: String(rowRoot.modelData.actionLabel || qsTr("Open"))
                Layout.preferredWidth: 78
                onClicked: root.activated(rowRoot.index, rowRoot.modelData)
            }
        }
    }
}
