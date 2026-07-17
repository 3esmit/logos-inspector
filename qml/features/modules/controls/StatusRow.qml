pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../theme"

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

    Accessible.role: Accessible.StaticText
    Accessible.name: root.accessibleName()
    Accessible.description: root.accessibleDescription()

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
                Accessible.ignored: true
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
            Accessible.ignored: true
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
            Accessible.ignored: true
        }

        Text {
            visible: root.layoutWidth >= 760
            text: qsTr("%1 / %2").arg(root.source).arg(root.freshness)
            color: root.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.preferredWidth: 192
            Accessible.ignored: true
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

    function accessibleName() {
        const labelText = String(root.label || "").trim()
        const status = String(root.stateText || "").trim()
        const evidenceText = root.accessibleEvidence()
        let summary = labelText.length > 0 ? labelText : qsTr("Status")
        if (labelText.length > 0 && status.length > 0) {
            summary = qsTr("%1: %2").arg(labelText).arg(status)
        }
        if (evidenceText.length > 0 && evidenceText !== "-") {
            return qsTr("%1. %2").arg(summary).arg(evidenceText)
        }
        return summary
    }

    function accessibleEvidence() {
        const normalized = String(root.evidence || "").replace(/\s+/g, " ").trim()
        const limit = 240
        if (normalized.length <= limit) {
            return normalized
        }
        return normalized.slice(0, limit - 3) + "..."
    }

    function accessibleDescription() {
        const values = []
        const details = [root.source, root.freshness]
        for (let i = 0; i < details.length; ++i) {
            const value = String(details[i] || "").trim()
            if (value.length > 0 && value !== "-") {
                values.push(value)
            }
        }
        return values.join(". ")
    }
}
