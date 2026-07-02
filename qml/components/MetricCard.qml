import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../theme"

Frame {
    id: root

    required property Theme theme
    property string label: ""
    property string value: "-"
    property string delta: ""
    property color deltaColor: theme.textMuted
    property bool compact: false

    padding: compact ? 12 : 14
    Layout.fillWidth: true
    Layout.minimumWidth: 150
    Layout.preferredHeight: compact ? 100 : 116

    background: Rectangle {
        color: root.theme.surface
        radius: root.theme.radius
        border.width: 1
        border.color: root.theme.outlineMuted
    }

    contentItem: ColumnLayout {
        spacing: root.compact ? 6 : 8

        Text {
            text: root.label
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: 11
            font.weight: Font.Medium
            font.capitalization: Font.AllUppercase
            elide: Text.ElideRight
            Layout.fillWidth: true
        }

        Text {
            text: root.value
            color: root.theme.text
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.compact ? 22 : 24
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: 6
            Layout.fillWidth: true

            Rectangle {
                color: root.deltaColor
                radius: 3
                Layout.preferredWidth: 6
                Layout.preferredHeight: 6
                Accessible.ignored: true
            }

            Text {
                text: root.delta.length ? root.delta : qsTr("No data")
                color: root.delta.length ? root.deltaColor : root.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: 12
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: qsTr("%1: %2. %3").arg(root.label).arg(root.value).arg(root.delta.length ? root.delta : qsTr("No data"))
}
