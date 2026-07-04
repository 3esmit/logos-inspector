pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import ".."
import "../../theme"

Item {
    id: root

    required property Theme theme
    property string label: ""
    property string protocolId: ""
    property string stateText: ""
    property string evidence: ""
    property string tone: "neutral"
    property int labelWidth: 132
    readonly property real layoutWidth: root.width > 0 ? root.width : (parent ? parent.width : 900)

    Layout.fillWidth: true
    implicitHeight: Math.max(52, rowGrid.implicitHeight + root.theme.gapSmall * 2)

    GridLayout {
        id: rowGrid

        anchors.fill: parent
        anchors.leftMargin: root.theme.gapSmall
        anchors.rightMargin: root.theme.gapSmall
        columns: root.layoutWidth < 780 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: 2

        Text {
            text: root.label
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            Layout.preferredWidth: root.layoutWidth < 780 ? 0 : root.labelWidth
            Layout.fillWidth: root.layoutWidth < 780
        }

        LinkCell {
            theme: root.theme
            text: root.protocolId
            copyable: root.protocolId.length > 0
            copyText: root.protocolId
            link: false
            Layout.fillWidth: true
        }

        Text {
            text: root.stateText
            color: root.toneColor()
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            Layout.preferredWidth: root.layoutWidth < 780 ? 0 : 110
            Layout.fillWidth: root.layoutWidth < 780
        }

        Text {
            text: root.evidence
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.fillWidth: true
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        height: 1
        color: root.theme.outlineMuted
        Accessible.ignored: true
    }

    function toneColor() {
        if (root.tone === "success") {
            return root.theme.success
        }
        if (root.tone === "warning") {
            return root.theme.warning
        }
        if (root.tone === "error") {
            return root.theme.error
        }
        return root.theme.textDim
    }
}
