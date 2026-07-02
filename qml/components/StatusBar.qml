import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

Pane {
    id: root

    required property Theme theme
    required property AppModel model

    padding: 12

    background: Rectangle {
        color: root.theme.background
    }

    contentItem: RowLayout {
        spacing: 12

        RowLayout {
            spacing: 10
            Layout.fillWidth: true

            Rectangle {
                color: root.model.busy ? root.theme.warning : root.theme.success
                radius: 4
                Layout.preferredWidth: 8
                Layout.preferredHeight: 8
                Layout.alignment: Qt.AlignVCenter
            }

            Text {
                text: root.model.busy ? root.model.statusText : qsTr("%1 | %2").arg(root.model.networkProfile).arg(root.model.statusText)
                color: root.theme.text
                elide: Text.ElideRight
                textFormat: Text.PlainText
                font.pixelSize: 13
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
