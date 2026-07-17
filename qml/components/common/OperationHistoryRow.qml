pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../theme"

Rectangle {
    id: root

    required property Theme theme
    property string timeText: ""
    property string labelText: ""
    property string statusText: ""
    property string detailText: ""

    radius: root.theme.radius
    color: root.theme.field
    border.width: 1
    border.color: root.statusText === "error" ? root.theme.error : root.theme.outlineMuted
    implicitHeight: 62
    Layout.fillWidth: true

    Accessible.role: Accessible.StaticText
    Accessible.name: root.accessibleName()
    Accessible.description: root.accessibleDescription()

    GridLayout {
        anchors.fill: parent
        anchors.margins: root.theme.gapSmall
        columns: 4
        columnSpacing: root.theme.gap

        Text {
            text: root.timeText
            color: root.theme.textDim
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.theme.dataText
            Layout.preferredWidth: 64
        }

        Text {
            text: root.labelText
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.Medium
            elide: Text.ElideRight
            Layout.preferredWidth: 150
        }

        Text {
            text: root.statusText
            color: root.statusText === "error" ? root.theme.error : root.theme.success
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            Layout.preferredWidth: 56
        }

        Text {
            text: root.detailText
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }

    function accessibleName() {
        const label = String(root.labelText || "").trim()
        const status = String(root.statusText || "").trim()
        if (label.length > 0 && label !== "-"
                && status.length > 0 && status !== "-") {
            return qsTr("%1: %2").arg(label).arg(status)
        }
        if (label.length > 0 && label !== "-") {
            return label
        }
        if (status.length > 0 && status !== "-") {
            return status
        }
        return qsTr("Operation")
    }

    function accessibleDescription() {
        const values = []
        const time = String(root.timeText || "").trim()
        const detail = String(root.detailText || "").trim()
        if (time.length > 0 && time !== "-") {
            values.push(time)
        }
        if (detail.length > 0 && detail !== "-") {
            values.push(detail)
        }
        return values.join(". ")
    }
}
