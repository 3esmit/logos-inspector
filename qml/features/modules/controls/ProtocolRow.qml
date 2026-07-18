pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"

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
    readonly property int accessibleNameLimit: 384

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
            Accessible.ignored: true
        }

        LinkCell {
            theme: root.theme
            text: root.protocolId
            copyable: root.protocolId.length > 0
            copyText: root.protocolId
            link: false
            accessibleName: root.rowAccessibleName()
            copyAccessibleName: root.copyActionAccessibleName()
            copyAccessibleDescription: root.copyActionAccessibleDescription()
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
            Accessible.ignored: true
        }

        Text {
            text: root.evidence
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.dataText
            elide: Text.ElideRight
            Layout.fillWidth: true
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

    function rowAccessibleName() {
        const labelText = root.boundedAccessibleText(root.label, 96)
        const state = root.boundedAccessibleText(root.stateText, 64)
        const protocol = root.boundedAccessibleText(root.protocolId, 192)
        const evidenceText = root.accessibleEvidence()
        let summary = labelText.length > 0 ? labelText : qsTr("Protocol")
        if (state.length > 0) {
            summary = qsTr("%1: %2").arg(summary).arg(state)
        }
        let name = summary
        if (protocol.length > 0) {
            name = qsTr("%1. Protocol ID: %2").arg(name).arg(protocol)
        }
        if (evidenceText.length > 0 && evidenceText !== "-") {
            name = qsTr("%1. %2").arg(name).arg(evidenceText)
        }
        return root.boundedAccessibleText(name, root.accessibleNameLimit)
    }

    function accessibleEvidence() {
        return root.boundedAccessibleText(root.evidence, 240)
    }

    function boundedAccessibleText(value, limit) {
        const normalized = String(value || "").replace(/\s+/g, " ").trim()
        if (normalized.length <= limit) {
            return normalized
        }
        return normalized.slice(0, limit - 3) + "..."
    }

    function copyActionAccessibleName() {
        const labelText = root.boundedAccessibleText(root.label, 96)
        return labelText.length > 0
            ? qsTr("Copy %1 protocol ID").arg(labelText)
            : qsTr("Copy protocol ID")
    }

    function copyActionAccessibleDescription() {
        const labelText = root.boundedAccessibleText(root.label, 96)
        return labelText.length > 0
            ? qsTr("Copies exact %1 protocol ID.").arg(labelText)
            : qsTr("Copies exact protocol ID.")
    }
}
