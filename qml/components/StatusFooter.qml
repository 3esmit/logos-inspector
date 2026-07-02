pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

Pane {
    id: root

    required property Theme theme
    required property AppModel model

    readonly property bool compact: width < 900

    leftPadding: root.theme.gap
    rightPadding: root.theme.gap
    topPadding: 5
    bottomPadding: 5
    Layout.fillWidth: true
    Layout.preferredHeight: footerGrid.implicitHeight + topPadding + bottomPadding

    background: Rectangle {
        color: root.theme.sidebar
    }

    contentItem: GridLayout {
        id: footerGrid

        columns: root.compact ? 1 : 2
        columnSpacing: root.theme.gapLarge
        rowSpacing: root.theme.gapTiny

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignLeft | Qt.AlignVCenter

            Repeater {
                model: root.contextItems()

                StatusToken {
                    required property var modelData

                    visible: !root.compact || String(modelData.priority || "normal") !== "low"
                    theme: root.theme
                    label: String(modelData.label || "")
                    value: String(modelData.value || "")
                    accessibleValue: String(modelData.accessibleValue || modelData.value || "-")
                    tone: String(modelData.tone || "neutral")
                    fullName: String(modelData.fullName || modelData.label || "")
                    maximumTokenWidth: modelData.maximumWidth || 150
                    Layout.alignment: Qt.AlignVCenter
                }
            }
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: root.compact
            Layout.alignment: (root.compact ? Qt.AlignLeft : Qt.AlignRight) | Qt.AlignVCenter

            Repeater {
                model: root.healthItems()

                StatusToken {
                    required property var modelData

                    theme: root.theme
                    label: String(modelData.label || "")
                    value: String(modelData.value || "")
                    accessibleValue: String(modelData.accessibleValue || modelData.value || "-")
                    tone: String(modelData.tone || "neutral")
                    fullName: String(modelData.fullName || modelData.label || "")
                    maximumTokenWidth: modelData.maximumWidth || 132
                    Layout.alignment: Qt.AlignVCenter
                }
            }
        }
    }

    function contextItems() {
        return [
            { label: qsTr("Net"), fullName: qsTr("Current network"), value: root.networkLabel(), tone: "info" },
            { label: qsTr("Sync"), fullName: qsTr("Bedrock sync state"), value: root.bedrockSyncState(), tone: root.syncTone(), maximumWidth: 170 },
            { label: qsTr("Peers"), fullName: qsTr("Bedrock peer count"), value: root.numberText(root.networkValue("n_peers")), tone: root.numberTone(root.networkValue("n_peers")) },
            { label: qsTr("LIB"), fullName: qsTr("Bedrock LIB"), value: root.numberText(root.cryptarchiaValue("lib_slot")), tone: "neutral", priority: "low" },
            { label: qsTr("TIP"), fullName: qsTr("Bedrock TIP"), value: root.numberText(root.cryptarchiaValue("slot")), tone: "neutral", priority: "low" },
            { label: qsTr("Final"), fullName: qsTr("LEZ finalized height"), value: root.numberText(root.lezBlockHeight()), tone: root.numberTone(root.lezBlockHeight()) },
            { label: qsTr("Height"), fullName: qsTr("LEZ block height"), value: root.numberText(root.probeValue("sequencer", "head")), tone: root.numberTone(root.probeValue("sequencer", "head")) }
        ]
    }

    function healthItems() {
        return [
            { label: qsTr("Bedrock"), fullName: qsTr("Bedrock health"), value: root.healthDisplayText("node", "consensus"), accessibleValue: root.healthAccessibleText("node", "consensus"), tone: root.toneForProbe("node", "consensus") },
            { label: qsTr("LEZ"), fullName: qsTr("LEZ health"), value: root.healthDisplayText("sequencer", "health"), accessibleValue: root.healthAccessibleText("sequencer", "health"), tone: root.toneForProbe("sequencer", "health") },
            { label: qsTr("Indexer"), fullName: qsTr("Indexer status"), value: root.indexerDisplayStatus(), accessibleValue: root.indexerStatus(), tone: root.indexerStatusTone(), maximumWidth: 148 }
        ]
    }

    function overview() {
        return root.model.dashboardOverview || {}
    }

    function nodeReport() {
        return root.model.dashboardNode || {}
    }

    function probe(section, field) {
        const target = root.overview()[section]
        return target ? target[field] : null
    }

    function probeValue(section, field) {
        const target = root.probe(section, field)
        return target && target.value !== undefined && target.value !== null ? target.value : null
    }

    function probeOk(section, field) {
        const target = root.probe(section, field)
        return target && target.ok === true
    }

    function probeKnown(section, field) {
        return root.probe(section, field) !== null
    }

    function healthText(section, field) {
        if (!root.probeKnown(section, field)) {
            return qsTr("unknown")
        }
        return root.probeOk(section, field) ? qsTr("ok") : qsTr("error")
    }

    function healthDisplayText(section, field) {
        if (!root.probeKnown(section, field)) {
            return qsTr("unknown")
        }
        return root.probeOk(section, field) ? "" : qsTr("error")
    }

    function healthAccessibleText(section, field) {
        return root.healthText(section, field)
    }

    function toneForProbe(section, field) {
        if (!root.probeKnown(section, field)) {
            return "neutral"
        }
        return root.probeOk(section, field) ? "success" : "error"
    }

    function consensusValue() {
        const value = root.probeValue("node", "consensus")
        return value && typeof value === "object" ? value : {}
    }

    function cryptarchiaInfo() {
        const value = root.consensusValue().cryptarchia_info
        return value && typeof value === "object" ? value : {}
    }

    function cryptarchiaValue(key) {
        const value = root.cryptarchiaInfo()[key]
        return value === undefined || value === null ? null : value
    }

    function reportValue(name) {
        const report = root.nodeReport()[name]
        return report && report.value ? report.value : {}
    }

    function networkValue(key) {
        const value = root.reportValue("network_info")[key]
        return value === undefined || value === null ? null : value
    }

    function bedrockSyncState() {
        const value = root.consensusValue()
        if (typeof value.sync_state === "string") {
            return value.sync_state
        }
        if (typeof value.syncState === "string") {
            return value.syncState
        }
        const mode = value.mode
        if (typeof mode === "string") {
            return mode
        }
        if (mode && mode.Started) {
            return mode.Started
        }
        return qsTr("unknown")
    }

    function syncTone() {
        const value = String(root.bedrockSyncState() || "").toLowerCase()
        if (value === "unknown") {
            return "neutral"
        }
        if (value.indexOf("sync") >= 0 || value.indexOf("catch") >= 0 || value.indexOf("start") >= 0) {
            return "warning"
        }
        return "success"
    }

    function lezBlockHeight() {
        const blocks = root.model.dashboardBlocks || []
        if (blocks.length > 0) {
            const block = blocks[0] || {}
            if (block.block_id !== undefined && block.block_id !== null) {
                return block.block_id
            }
        }
        return root.probeValue("sequencer", "head")
    }

    function indexerStatus() {
        if (!root.probeKnown("indexer", "health")) {
            return qsTr("unknown")
        }
        if (!root.probeOk("indexer", "health")) {
            return qsTr("stalled")
        }
        const indexerHead = Number(root.probeValue("indexer", "head"))
        const sequencerHead = Number(root.probeValue("sequencer", "head"))
        if (Number.isFinite(indexerHead) && Number.isFinite(sequencerHead) && indexerHead < sequencerHead) {
            return qsTr("backfilling")
        }
        return qsTr("running")
    }

    function indexerStatusTone() {
        const value = root.indexerStatus()
        if (value === qsTr("running")) {
            return "success"
        }
        if (value === qsTr("backfilling")) {
            return "warning"
        }
        if (value === qsTr("stalled")) {
            return "error"
        }
        return "neutral"
    }

    function indexerDisplayStatus() {
        const value = root.indexerStatus()
        return value === qsTr("running") ? "" : value
    }

    function networkLabel() {
        const profile = String(root.model.networkProfile || "").toLowerCase()
        const sequencer = String(root.model.sequencerUrl || "").toLowerCase()
        if (profile.indexOf("local") >= 0 || sequencer.indexOf("127.0.0.1") >= 0 || sequencer.indexOf("localhost") >= 0) {
            return qsTr("local")
        }
        if (profile.indexOf("mainnet") >= 0 || sequencer.indexOf("mainnet") >= 0) {
            return qsTr("mainnet")
        }
        if (profile === "custom") {
            return qsTr("custom")
        }
        return qsTr("testnet")
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        if (typeof value === "number") {
            return value.toLocaleString(Qt.locale(), "f", 0)
        }
        const number = Number(value)
        if (Number.isFinite(number) && String(value).match(/^[0-9]+$/)) {
            return number.toLocaleString(Qt.locale(), "f", 0)
        }
        return String(value)
    }

    function numberTone(value) {
        const number = Number(value)
        return Number.isFinite(number) && number > 0 ? "success" : "neutral"
    }

    component StatusToken: Control {
        id: token

        required property Theme theme
        property string label: ""
        property string value: "-"
        property string accessibleValue: value
        property string tone: "neutral"
        property string fullName: ""
        property int maximumTokenWidth: 140

        hoverEnabled: true
        padding: 0
        implicitWidth: Math.min(tokenRow.implicitWidth, maximumTokenWidth)
        implicitHeight: 22

        background: Item {}

        contentItem: RowLayout {
            id: tokenRow

            clip: true
            spacing: token.theme.gapTiny

            Rectangle {
                color: token.toneColor()
                radius: width / 2
                Layout.preferredWidth: 7
                Layout.preferredHeight: 7
                Layout.alignment: Qt.AlignVCenter
                Accessible.ignored: true
            }

            Text {
                text: token.label
                color: token.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: token.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                elide: Text.ElideRight
                Layout.maximumWidth: 74
            }

            Text {
                text: token.value
                visible: token.value.length > 0
                color: token.valueColor()
                textFormat: Text.PlainText
                font.pixelSize: token.theme.dataText
                font.family: "monospace"
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.maximumWidth: Math.max(44, token.maximumTokenWidth - 84)
            }
        }

        ToolTip.visible: hovered && token.fullName.length > 0
        ToolTip.text: qsTr("%1: %2").arg(token.fullName).arg(token.accessibleValue)

        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("%1: %2").arg(token.fullName.length > 0 ? token.fullName : token.label).arg(token.accessibleValue)

        function toneColor() {
            if (token.tone === "success") {
                return token.theme.success
            }
            if (token.tone === "warning") {
                return token.theme.warning
            }
            if (token.tone === "error") {
                return token.theme.error
            }
            if (token.tone === "info") {
                return token.theme.info
            }
            return token.theme.textDim
        }

        function valueColor() {
            if (token.tone === "error") {
                return token.theme.error
            }
            if (token.tone === "warning") {
                return token.theme.warning
            }
            if (token.tone === "success") {
                return token.theme.success
            }
            return token.theme.text
        }
    }
}
