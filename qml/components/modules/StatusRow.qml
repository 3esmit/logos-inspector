pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../theme"

Item {
    id: root

    required property Theme theme
    property string label: ""
    property string stateText: ""
    property string evidence: ""
    property string source: ""
    property string freshness: ""
    property string tone: "neutral"
    readonly property real layoutWidth: root.width > 0 ? root.width : (parent ? parent.width : 900)

    Layout.fillWidth: true
    implicitHeight: Math.max(48, rowGrid.implicitHeight + root.theme.gapSmall * 2)

    GridLayout {
        id: rowGrid

        anchors.fill: parent
        anchors.leftMargin: root.theme.gapSmall
        anchors.rightMargin: root.theme.gapSmall
        columns: root.layoutWidth < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: 2

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.preferredWidth: root.layoutWidth < 760 ? 0 : 212
            Layout.fillWidth: root.layoutWidth < 760

            Rectangle {
                radius: 4
                color: root.toneColor()
                Layout.preferredWidth: 8
                Layout.preferredHeight: 8
                Layout.alignment: Qt.AlignVCenter
                Accessible.ignored: true
            }

            Text {
                text: root.label
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        Text {
            text: root.stateText
            color: root.toneColor()
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            font.weight: Font.DemiBold
            elide: Text.ElideRight
            Layout.preferredWidth: root.layoutWidth < 760 ? 110 : 118
        }

        Text {
            text: root.evidence
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.columnSpan: root.layoutWidth < 760 ? 2 : 1
            Layout.fillWidth: true
        }

        Text {
            visible: root.layoutWidth >= 760
            text: qsTr("%1 / %2").arg(root.source).arg(root.freshness)
            color: root.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.preferredWidth: 192
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
