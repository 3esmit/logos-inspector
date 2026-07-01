import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

Pane {
    id: root

    required property Theme theme
    required property AppModel model

    padding: 16

    background: Rectangle {
        color: theme.background
    }

    contentItem: RowLayout {
        spacing: 12

        ColumnLayout {
            spacing: 2
            Layout.fillWidth: true

            Text {
                text: qsTr("Blockchain explorer")
                color: theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 12
                font.weight: Font.Medium
                Layout.fillWidth: true
            }

            Text {
                text: root.model.viewTitle()
                color: theme.text
                elide: Text.ElideRight
                textFormat: Text.PlainText
                font.pixelSize: 24
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }
        }

        BusyIndicator {
            running: root.model.busy
            visible: root.model.busy
            Layout.preferredWidth: 32
            Layout.preferredHeight: 32
        }

        ActionButton {
            theme: root.theme
            text: root.model.busy ? qsTr("Working") : qsTr("Ready")
            enabled: false
            Layout.preferredWidth: 96
        }
    }
}
