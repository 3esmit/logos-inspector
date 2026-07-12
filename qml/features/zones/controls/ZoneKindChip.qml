import QtQuick
import "../../../theme"

Rectangle {
    id: root

    required property Theme theme
    property string label: ""
    property string tone: "neutral"

    objectName: "zoneKindChip"
    implicitWidth: Math.max(86, chipText.implicitWidth + root.theme.gapLarge)
    implicitHeight: 26
    radius: 6
    color: root.tone === "success" ? root.theme.successMuted
        : (root.tone === "warning" ? root.theme.warningMuted
        : (root.tone === "error" ? root.theme.errorMuted
        : (root.tone === "info" ? root.theme.infoMuted : root.theme.surfaceRaised)))
    border.width: 1
    border.color: root.tone === "success" ? root.theme.success
        : (root.tone === "warning" ? root.theme.warning
        : (root.tone === "error" ? root.theme.error
        : (root.tone === "info" ? root.theme.info : root.theme.outline)))

    Text {
        id: chipText

        anchors.centerIn: parent
        width: parent.width - root.theme.gap
        text: root.label
        color: root.theme.text
        textFormat: Text.PlainText
        elide: Text.ElideRight
        horizontalAlignment: Text.AlignHCenter
        font.pixelSize: root.theme.labelText
        font.weight: Font.DemiBold
    }

    Accessible.role: Accessible.StaticText
    Accessible.name: root.label
}
