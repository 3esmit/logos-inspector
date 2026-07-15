pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../state/status/FooterStatusProjection.js" as FooterStatusProjection
import "../theme"
import "status"

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
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gapTiny

        Flow {
            id: leftFlow

            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Repeater {
                model: root.footerGroups("left")

                FooterSourceGroup {
                    required property var modelData

                    theme: root.theme
                    compact: root.compact
                    first: modelData.first === true
                    items: modelData.items || []
                }
            }
        }

        Flow {
            id: rightFlow

            visible: root.footerGroups("right").length > 0
            spacing: root.theme.gapSmall
            Layout.fillWidth: root.compact
            Layout.alignment: root.compact ? Qt.AlignLeft : Qt.AlignRight

            Repeater {
                model: root.footerGroups("right")

                FooterSourceGroup {
                    required property var modelData

                    theme: root.theme
                    compact: root.compact
                    first: modelData.first === true
                    items: modelData.items || []
                }
            }
        }
    }

    function footerGroups(region) {
        return FooterStatusProjection.footerGroups(root, region)
    }

    function footerGroupItems(group) {
        return FooterStatusProjection.footerGroupItems(root, group)
    }

    function footerGroupVisible(keys) {
        return FooterStatusProjection.footerGroupVisible(root, keys)
    }

    function footerSourceGroups() {
        return FooterStatusProjection.footerSourceGroups()
    }

    function footerFieldItem(key) {
        return FooterStatusProjection.footerFieldItem(root, key)
    }

    function footerFieldLabel(key) {
        return FooterStatusProjection.footerFieldLabel(key)
    }

    function footerFieldName(key) {
        return FooterStatusProjection.footerFieldName(key)
    }

    function footerFieldValue(key) {
        return FooterStatusProjection.footerFieldValue(root, key)
    }

    function footerFieldAccessibleValue(key) {
        return FooterStatusProjection.footerFieldAccessibleValue(root, key)
    }

    function footerFieldTone(key) {
        return FooterStatusProjection.footerFieldTone(root, key)
    }

    function footerFieldWidth(key) {
        return FooterStatusProjection.footerFieldWidth(key)
    }

    function footerFieldPriority(key) {
        return FooterStatusProjection.footerFieldPriority(key)
    }

    function footerFieldUsesColorOnly(key) {
        return FooterStatusProjection.footerFieldUsesColorOnly(key)
    }

    function footerFieldShowsDot(key) {
        return FooterStatusProjection.footerFieldShowsDot(key)
    }

    function footerFieldHidden(key) {
        return FooterStatusProjection.footerFieldHidden(root, key)
    }

    function overview() {
        return root.model.metrics.dashboardOverview || {}
    }

    function nodeReport() {
        return root.model.metrics.dashboardNode || {}
    }

    function probe(section, field) {
        const target = root.overview()[section]
        return target ? target[field] : null
    }

    function probeValue(section, field) {
        const target = root.probe(section, field)
        return target && target.value !== undefined && target.value !== null ? root.model.metrics.scalarValue(target.value) : null
    }

    function probeRawValue(section, field) {
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
        const value = root.probeRawValue("node", "consensus")
        if (value && typeof value.value === "object") {
            return value.value
        }
        return value && typeof value === "object" ? value : {}
    }

    function cryptarchiaInfo() {
        const value = root.consensusValue().cryptarchia_info
        return value && typeof value === "object" ? value : {}
    }

    function cryptarchiaValue(key) {
        const value = root.cryptarchiaInfo()[key]
        return value === undefined || value === null ? null : root.model.metrics.scalarValue(value)
    }

    function reportValue(name) {
        const report = root.nodeReport()[name]
        return report && report.value ? report.value : {}
    }

    function networkValue(key) {
        const value = root.reportValue("network_info")[key]
        return value === undefined || value === null ? null : root.model.metrics.scalarValue(value)
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
        if (value === "stalled" || value.indexOf("fail") >= 0 || value.indexOf("error") >= 0) {
            return "error"
        }
        if (value === "synced" || value === "ready" || value === "running") {
            return "success"
        }
        if (value.indexOf("syncing") >= 0 || value.indexOf("catch") >= 0 || value.indexOf("start") >= 0) {
            return "warning"
        }
        return "success"
    }

    function lezBlockHeight() {
        const blocks = root.model.metrics.dashboardProvisionalBlocks || []
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
        const node = String(root.model.nodeUrl || "").toLowerCase()
        if (node.indexOf("127.0.0.1") >= 0 || node.indexOf("localhost") >= 0) {
            return qsTr("local")
        }
        if (profile.indexOf("mainnet") >= 0 || node.indexOf("mainnet") >= 0) {
            return qsTr("mainnet")
        }
        if (profile.indexOf("testnet") >= 0 || node.indexOf("testnet") >= 0) {
            return qsTr("testnet")
        }
        if (profile === "custom") {
            return qsTr("custom")
        }
        return qsTr("testnet")
    }

    function valueOrNa(value) {
        const scalar = root.model.metrics.scalarValue(value)
        if (scalar === undefined || scalar === null || scalar === "") {
            return qsTr("n/a")
        }
        return root.numberText(scalar)
    }

    function shortHash(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("n/a")
        }
        if (text.length <= 14) {
            return text
        }
        return text.slice(0, 8) + "..." + text.slice(-4)
    }

    function tipMinusLib() {
        return root.model.metrics.tipMinusLib()
    }

    function finalityLagSeconds() {
        return root.model.metrics.finalityLagSeconds()
    }

    function indexerLag() {
        return root.model.metrics.indexerLag()
    }

    function connectionStatus(kind) {
        return root.model.metrics.networkConnectionState(kind)
    }

    function moduleDisplayStatus(kind) {
        const report = root.model.metrics.moduleReport(kind)
        if (!report) {
            return qsTr("unknown")
        }
        return root.model.metrics.moduleReportReachable(report) ? "" : qsTr("stopped")
    }

    function moduleAccessibleStatus(kind) {
        const report = root.model.metrics.moduleReport(kind)
        if (!report) {
            return qsTr("unknown")
        }
        return root.model.metrics.moduleReportReachable(report)
            ? qsTr("running") : qsTr("stopped")
    }

    function connectionAccessibleStatus(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return qsTr("unknown")
        }
        return status.ok ? qsTr("connected") : qsTr("disconnected")
    }

    function connectionReachableStatus(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return qsTr("unknown")
        }
        return status.ok ? qsTr("yes") : qsTr("no")
    }

    function moduleTone(kind) {
        const report = root.model.metrics.moduleReport(kind)
        if (!report) {
            return "neutral"
        }
        return root.model.metrics.moduleReportReachable(report) ? "success" : "error"
    }

    function connectionTone(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return "neutral"
        }
        return status.ok ? "success" : "error"
    }

    function overallTone() {
        return FooterStatusProjection.overallTone(root)
    }

    function overallStatusText() {
        return FooterStatusProjection.overallStatusText(root)
    }

    function overallStatusDisplay() {
        return FooterStatusProjection.overallStatusDisplay(root)
    }

    function mainRisk() {
        return FooterStatusProjection.mainRisk(root)
    }

    function operatorAction() {
        return FooterStatusProjection.operatorAction(root)
    }

    function latestSequencerBlockValue(key) {
        const blocks = root.model.metrics.dashboardProvisionalBlocks || []
        if (!blocks.length) {
            return null
        }
        const block = blocks[0] || {}
        const value = block[key]
        return value === undefined || value === null ? null : value
    }

    function latestIndexerBlockValue(key) {
        const blocks = root.model.metrics.dashboardBlocks || []
        if (!blocks.length) {
            return null
        }
        const block = blocks[0] || {}
        const value = block[key]
        return value === undefined || value === null ? null : value
    }

    function timeText(value) {
        const scalar = root.model.metrics.scalarValue(value)
        if (scalar === null) {
            return qsTr("n/a")
        }
        const number = Number(scalar)
        if (!Number.isFinite(number) || number <= 0) {
            return root.numberText(scalar)
        }
        const millis = number > 1000000000000 ? number : number * 1000
        return new Date(millis).toLocaleTimeString(Qt.locale(), "hh:mm")
    }

    function yesNo(value) {
        const scalar = root.model.metrics.scalarValue(value)
        if (scalar === null) {
            return qsTr("n/a")
        }
        if (typeof scalar === "boolean") {
            return scalar ? qsTr("yes") : qsTr("no")
        }
        const number = Number(scalar)
        if (Number.isFinite(number)) {
            return number > 0 ? qsTr("yes") : qsTr("no")
        }
        const text = String(scalar).toLowerCase()
        if (text === "true" || text === "yes" || text === "open" || text === "connected") {
            return qsTr("yes")
        }
        if (text === "false" || text === "no" || text === "blocked" || text === "disconnected") {
            return qsTr("no")
        }
        return String(scalar)
    }

    function portStatus(kind, metricNames) {
        const value = root.model.metrics.openMetricValue(kind, metricNames)
        if (value === null) {
            return qsTr("n/a")
        }
        return Number(value) > 0 ? qsTr("open") : qsTr("blocked")
    }

    function numberText(value) {
        const scalar = root.model.metrics.scalarValue(value)
        if (scalar === undefined || scalar === null || scalar === "") {
            return "-"
        }
        if (typeof scalar === "number") {
            return scalar.toLocaleString(Qt.locale(), "f", Number.isInteger(scalar) ? 0 : 2)
        }
        const number = Number(scalar)
        if (Number.isFinite(number) && String(scalar).match(/^[0-9]+$/)) {
            return number.toLocaleString(Qt.locale(), "f", 0)
        }
        return String(scalar)
    }

    function numberTone(value) {
        const number = Number(value)
        return Number.isFinite(number) && number > 0 ? "success" : "neutral"
    }

    function countProblemTone(value) {
        const number = Number(root.model.metrics.scalarValue(value))
        if (!Number.isFinite(number) || number <= 0) {
            return "neutral"
        }
        return "warning"
    }

    function booleanTone(value) {
        const text = String(value || "").toLowerCase()
        if (text === String(qsTr("yes")).toLowerCase()
                || text === String(qsTr("open")).toLowerCase()
                || text === String(qsTr("connected")).toLowerCase()
                || text === String(qsTr("running")).toLowerCase()) {
            return "success"
        }
        if (text === String(qsTr("no")).toLowerCase()
                || text === String(qsTr("blocked")).toLowerCase()
                || text === String(qsTr("disconnected")).toLowerCase()
                || text === String(qsTr("stopped")).toLowerCase()) {
            return "error"
        }
        return "neutral"
    }

    function portTone(value) {
        const text = String(value || "").toLowerCase()
        if (text === String(qsTr("open")).toLowerCase()) {
            return "success"
        }
        if (text === String(qsTr("blocked")).toLowerCase()) {
            return "error"
        }
        return "neutral"
    }

    function statusWordTone(value) {
        const text = String(value || "").toLowerCase()
        if (!text.length || text === String(qsTr("n/a")).toLowerCase() || text === String(qsTr("unknown")).toLowerCase()) {
            return "neutral"
        }
        if (text.indexOf("fail") >= 0 || text.indexOf("error") >= 0 || text.indexOf("reject") >= 0 || text.indexOf("stalled") >= 0 || text.indexOf("down") >= 0) {
            return "error"
        }
        if (text.indexOf("pending") >= 0 || text.indexOf("sync") >= 0 || text.indexOf("backfill") >= 0 || text.indexOf("degraded") >= 0) {
            return "warning"
        }
        return "success"
    }

}
