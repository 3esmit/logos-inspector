pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import ".."
import "../../theme"

Popup {
    id: root

    required property Theme theme
    property string title: ""
    property string message: ""
    property string confirmText: qsTr("Confirm")
    property string cancelText: qsTr("Cancel")
    property bool confirmEnabled: true
    property int maximumWidth: 460

    signal accepted()

    parent: Overlay.overlay
    modal: true
    focus: true
    padding: root.theme.gap
    width: Math.min(root.maximumWidth, parent ? Math.max(0, parent.width - 24) : root.maximumWidth)
    x: parent ? Math.max(0, (parent.width - width) / 2) : 0
    y: 96
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

    background: Rectangle {
        color: root.theme.surface
        radius: root.theme.radius
        border.width: 1
        border.color: root.theme.outline
    }

    contentItem: ColumnLayout {
        spacing: root.theme.gapSmall

        Text {
            text: root.title
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.primaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Text {
            objectName: "messageText"
            text: root.message
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            font.pixelSize: root.theme.secondaryText
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Item {
                Layout.fillWidth: true
            }

            ActionButton {
                objectName: "cancelButton"
                theme: root.theme
                text: root.cancelText
                onClicked: root.close()
            }

            ActionButton {
                objectName: "confirmButton"
                theme: root.theme
                text: root.confirmText
                primary: true
                enabled: root.confirmEnabled
                onClicked: {
                    root.close()
                    root.accepted()
                }
            }
        }
    }
}
