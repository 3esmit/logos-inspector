import QtQuick
import QtQuick.Controls.Basic
import "../theme"

Control {
    id: root

    required property Theme theme
    property string text: ""

    implicitHeight: 22
    leftPadding: 8
    rightPadding: 8
    topPadding: 3
    bottomPadding: 3

    contentItem: Text {
        text: root.text
        color: root.textColor()
        textFormat: Text.PlainText
        elide: Text.ElideRight
        font.pixelSize: root.theme.labelText
        font.weight: Font.DemiBold
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        radius: 4
        color: root.backgroundColor()
        border.width: 1
        border.color: root.borderColor()
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.text

    function normalized() {
        return String(root.text || "").toLowerCase()
    }

    function backgroundColor() {
        const value = normalized()
        if (value.indexOf("l1") >= 0 || value.indexOf("bedrock") >= 0) {
            return root.theme.infoMuted
        }
        if (value.indexOf("l2") >= 0 || value.indexOf("lez") >= 0) {
            return root.theme.successMuted
        }
        if (value.indexOf("module") >= 0) {
            return root.theme.warningMuted
        }
        return root.theme.surfaceRaised
    }

    function borderColor() {
        const value = normalized()
        if (value.indexOf("l1") >= 0 || value.indexOf("bedrock") >= 0) {
            return root.theme.info
        }
        if (value.indexOf("l2") >= 0 || value.indexOf("lez") >= 0) {
            return root.theme.success
        }
        if (value.indexOf("module") >= 0) {
            return root.theme.warning
        }
        return root.theme.outline
    }

    function textColor() {
        return root.theme.text
    }
}
