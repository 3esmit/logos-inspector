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
    readonly property real availableOverlayHeight: parent
        ? Math.max(0, parent.height - 2 * root.theme.gapLarge)
        : 480
    readonly property real maximumMessageHeight: Math.max(
        root.theme.controlHeight,
        root.availableOverlayHeight
            - root.theme.controlHeight
            - root.theme.primaryText
            - 2 * root.padding
            - 2 * root.theme.gapSmall)

    signal accepted()

    parent: Overlay.overlay
    modal: true
    focus: true
    padding: root.theme.gap
    width: Math.min(root.maximumWidth, parent ? Math.max(0, parent.width - 24) : root.maximumWidth)
    height: Math.min(implicitHeight, root.availableOverlayHeight)
    x: parent ? Math.max(0, (parent.width - width) / 2) : 0
    y: parent ? Math.max(0, (parent.height - height) / 2) : 0
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
            objectName: "titleText"
            text: root.title
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.primaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }

        Flickable {
            id: messageViewport

            objectName: "messageViewport"
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(
                messageText.implicitHeight,
                root.maximumMessageHeight)
            Layout.maximumHeight: root.maximumMessageHeight
            contentWidth: width
            contentHeight: messageText.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            flickableDirection: Flickable.VerticalFlick
            interactive: contentHeight > height
            clip: true

            ScrollBar.vertical: ScrollBar {
                policy: messageViewport.contentHeight > messageViewport.height
                    ? ScrollBar.AlwaysOn
                    : ScrollBar.AlwaysOff
            }

            Text {
                id: messageText

                objectName: "messageText"
                width: messageViewport.width
                text: root.message
                color: root.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.pixelSize: root.theme.secondaryText
                Accessible.role: Accessible.StaticText
                Accessible.name: text
            }
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
