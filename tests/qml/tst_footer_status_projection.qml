import QtQuick
import QtTest
import "../../qml/state/status/FooterStatusProjection.js" as FooterStatusProjection

TestCase {
    id: testRoot

    name: "FooterStatusProjection"

    QtObject {
        id: model

        property int footerFieldRevision: 0
        property string storageNetworkPreset: ""

        function footerFieldEnabled(key) {
            return key === "overall.status"
                || key === "overall.main_risk"
                || key === "overall.operator_action"
                || key === "storage.node_reachable"
        }

        function scalarValue(value) { return value }
        function dashboardMetricValue(key) { return null }
        function openMetricValue(kind, names) { return null }
        function reportProbeValue(report, method) { return null }
        function moduleReport(kind) { return null }
        function moduleLastError(kind) { return "" }
        function storageSourceTarget() { return "rest" }
    }

    QtObject {
        id: footerRoot

        property var model: model

        function toneForProbe(section, field) {
            return section === "sequencer" ? "error" : "success"
        }

        function indexerStatusTone() { return "success" }
        function moduleTone(kind) { return "success" }
        function healthDisplayText(section, field) { return "" }
        function healthAccessibleText(section, field) { return "ok" }
        function networkLabel() { return "Testnet" }
        function valueOrNa(value) { return value === null || value === undefined || value === "" ? "n/a" : String(value) }
        function networkValue(key) { return null }
        function probeValue(section, field) { return null }
        function numberText(value) { return value === null || value === undefined ? "-" : String(value) }
        function cryptarchiaValue(key) { return null }
        function shortHash(value) { return "-" }
        function tipMinusLib() { return null }
        function finalityLagSeconds() { return null }
        function lezBlockHeight() { return null }
        function latestSequencerBlockValue(key) { return null }
        function latestIndexerBlockValue(key) { return null }
        function timeText(value) { return "n/a" }
        function indexerDisplayStatus() { return "" }
        function indexerStatus() { return "ok" }
        function indexerLag() { return null }
        function moduleDisplayStatus(kind) { return "" }
        function connectionAccessibleStatus(kind) { return "yes" }
        function connectionReachableStatus(kind) { return "yes" }
        function yesNo(value) { return "n/a" }
        function portStatus(kind, names) { return "n/a" }
        function syncTone() { return "neutral" }
        function booleanTone(value) { return value === "yes" ? "success" : "neutral" }
        function portTone(value) { return "neutral" }
        function countProblemTone(value) { return "neutral" }
        function statusWordTone(value) { return "neutral" }
        function moduleAccessibleStatus(kind) { return "ok" }
    }

    function test_overall_rows_are_projected_from_health_facts() {
        const status = FooterStatusProjection.footerFieldItem(footerRoot, "overall.status")
        const risk = FooterStatusProjection.footerFieldItem(footerRoot, "overall.main_risk")
        const action = FooterStatusProjection.footerFieldItem(footerRoot, "overall.operator_action")

        compare(status.value, "down")
        compare(status.tone, "error")
        compare(risk.value, "lez rpc")
        verify(!risk.hidden)
        compare(action.value, "check rpc")
    }

    function test_footer_groups_filter_disabled_and_hidden_fields() {
        const groups = FooterStatusProjection.footerGroups(footerRoot, "right")

        verify(groups.length > 0)
        verify(groups[0].items.some(function (item) {
            return item.fullName === "Overall"
        }))
    }
}
