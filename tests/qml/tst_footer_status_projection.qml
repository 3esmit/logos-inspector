import QtQuick
import QtTest
import "../../qml/state/status/FooterStatusProjection.js" as FooterStatusProjection

TestCase {
    id: testRoot

    name: "FooterStatusProjection"

    QtObject {
        id: sourceRoutingStub

        function storageSourceTarget() { return "rest" }
    }

    QtObject {
        id: model

        property int footerFieldRevision: 0
        property string storageNetworkPreset: ""
        property var sourceRouting: sourceRoutingStub
        property var configuredSourceReports: ({})
        property var configuredSourceObservations: ({})
        property var moduleReports: ({})
        property var connectionStates: ({})
        property var dashboardChannelStatuses: []
        readonly property var metrics: model

        function footerFieldEnabled(key) {
            return key === "overall.status"
                || key === "overall.main_risk"
                || key === "overall.operator_action"
                || key === "channels.summary"
                || key === "storage.node_reachable"
        }

        function scalarValue(value) { return value }
        function dashboardMetricValue(key) { return null }
        function openMetricValue(kind, names) { return null }
        function reportProbeValue(report, method) {
            const probes = report && Array.isArray(report.probes) ? report.probes : []
            for (let i = 0; i < probes.length; ++i) {
                if (probes[i].method === method && probes[i].ok === true) {
                    return probes[i].value
                }
            }
            return null
        }
        function moduleReport(kind) { return moduleReports[String(kind || "")] || null }
        function sourceReport(kind) { return configuredSourceReports[String(kind || "")] || null }
        function sourceObservation(kind) {
            const target = String(kind || "")
            const observation = configuredSourceObservations[target]
            if (observation) {
                return observation
            }
            return {
                sourceReport: sourceReport(target),
                latestAttempt: null,
                status: networkConnectionState(target)
            }
        }
        function moduleReportReachable(report) {
            if (!report) {
                return false
            }
            if (report.health && report.health.ready !== undefined) {
                return report.health.ready === true
            }
            const probes = Array.isArray(report.probes) ? report.probes : []
            return probes.some(function (probe) { return probe.ok === true })
        }
        function moduleReportError(report) {
            if (!report) {
                return ""
            }
            if (String(report.error || "").length > 0) {
                return String(report.error)
            }
            const probes = Array.isArray(report.probes) ? report.probes : []
            for (let i = 0; i < probes.length; ++i) {
                if (probes[i].ok === false && String(probes[i].error || "").length > 0) {
                    return String(probes[i].error)
                }
            }
            return ""
        }
        function networkConnectionState(kind) {
            const target = String(kind || "")
            return connectionStates[target] || { known: false, ok: false, detail: "" }
        }
    }

    QtObject {
        id: footerRoot

        property var model: model

        function toneForProbe(section, field) {
            return section === "sequencer" ? "error" : "success"
        }

        function indexerStatusTone() { return "success" }
        function moduleTone(kind) {
            const report = model.metrics.moduleReport(kind)
            if (!report) {
                return "neutral"
            }
            return model.metrics.moduleReportReachable(report) ? "success" : "error"
        }
        function connectionTone(kind) {
            const state = model.metrics.networkConnectionState(kind)
            if (!state.known) {
                return "neutral"
            }
            return state.ok ? "success" : "error"
        }
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
        function moduleDisplayStatus(kind) {
            const report = model.metrics.moduleReport(kind)
            if (!report) {
                return "unknown"
            }
            return model.metrics.moduleReportReachable(report) ? "" : "stopped"
        }
        function connectionAccessibleStatus(kind) {
            const state = model.metrics.networkConnectionState(kind)
            return state.known ? (state.ok ? "yes" : "no") : "unknown"
        }
        function connectionReachableStatus(kind) { return connectionAccessibleStatus(kind) }
        function yesNo(value) { return "n/a" }
        function portStatus(kind, names) { return "n/a" }
        function syncTone() { return "neutral" }
        function booleanTone(value) { return value === "yes" ? "success" : "neutral" }
        function portTone(value) { return "neutral" }
        function countProblemTone(value) { return "neutral" }
        function statusWordTone(value) { return "neutral" }
        function moduleAccessibleStatus(kind) {
            const report = model.metrics.moduleReport(kind)
            if (!report) {
                return "unknown"
            }
            return model.metrics.moduleReportReachable(report) ? "running" : "stopped"
        }
    }

    function init() {
        model.configuredSourceReports = ({})
        model.configuredSourceObservations = ({})
        model.moduleReports = ({})
        model.connectionStates = ({})
        model.dashboardChannelStatuses = []
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

    function test_footer_regions_project_left_and_right_in_one_pass() {
        const regions = FooterStatusProjection.footerRegions(footerRoot)

        verify(Array.isArray(regions.left))
        verify(Array.isArray(regions.right))
        verify(regions.left.some(function (group) {
            return group.items.some(function (item) {
                return item.fullName === "ST node"
            })
        }))
        verify(regions.right.some(function (group) {
            return group.items.some(function (item) {
                return item.fullName === "Overall"
            })
        }))
    }

    function test_configured_channels_render_independent_sequencer_and_indexer_statuses() {
        model.dashboardChannelStatuses = [{
            channel_id: "a".repeat(64),
            short_channel_id: "aaaa…aaaa",
            label: "Alpha",
            sequencer: { configured: true, status: "reachable", head: 104 },
            indexer: { configured: true, status: "reachable", head: 101 }
        }, {
            channel_id: "b".repeat(64),
            short_channel_id: "bbbb…bbbb",
            label: "Beta",
            sequencer: { configured: true, status: "unreachable", head: null },
            indexer: { configured: true, status: "degraded", head: 88 }
        }]

        const groups = FooterStatusProjection.channelFooterGroups(footerRoot)

        compare(groups.length, 2)
        compare(groups[0].items[0].fullName, "Channel Alpha")
        compare(groups[0].items[1].accessibleValue, "reachable; head 104")
        compare(groups[0].items[2].tone, "success")
        compare(groups[1].items[1].value, "unreachable")
        compare(groups[1].items[1].tone, "error")
        compare(groups[1].items[2].tone, "warning")
    }

    function test_channel_health_replaces_stale_single_zone_risk() {
        model.dashboardChannelStatuses = [{
            channel_id: "a".repeat(64),
            short_channel_id: "aaaa…aaaa",
            label: "Alpha",
            sequencer: { configured: true, status: "reachable", head: 104 },
            indexer: { configured: true, status: "reachable", head: 101 }
        }]

        let status = FooterStatusProjection.footerFieldItem(footerRoot, "overall.status")
        let risk = FooterStatusProjection.footerFieldItem(footerRoot, "overall.main_risk")
        compare(status.value, "")
        compare(status.tone, "success")
        compare(risk.value, "none")
        verify(risk.hidden)

        model.dashboardChannelStatuses = [{
            channel_id: "b".repeat(64),
            short_channel_id: "bbbb…bbbb",
            label: "Beta",
            sequencer: { configured: true, status: "reachable", head: 104 },
            indexer: { configured: true, status: "unreachable", head: null }
        }]

        status = FooterStatusProjection.footerFieldItem(footerRoot, "overall.status")
        risk = FooterStatusProjection.footerFieldItem(footerRoot, "overall.main_risk")
        const action = FooterStatusProjection.footerFieldItem(footerRoot, "overall.operator_action")
        compare(status.value, "down")
        compare(risk.value, "channel bbbb…bbbb indexer")
        compare(action.value, "check channel source")
    }

    function test_module_status_and_configured_source_status_are_distinct() {
        model.moduleReports = ({
            storage: {
                health: { ready: false },
                probes: [{ method: "moduleInfo", ok: false, error: "module stopped" }]
            }
        })
        model.connectionStates = ({ storage: { known: true, ok: true, detail: "reachable" } })

        compare(FooterStatusProjection.footerFieldValue(footerRoot, "storage.module"), "stopped")
        compare(FooterStatusProjection.footerFieldValue(footerRoot, "storage.node_reachable"), "yes")
        compare(FooterStatusProjection.footerFieldValue(footerRoot, "storage.last_error"), "n/a")
    }

    function test_configured_source_report_owns_cid_probe_and_last_error() {
        const sourceReport = {
            probes: [{ method: "exists", ok: true, value: true }]
        }
        model.moduleReports = ({
            storage: {
                probes: [{ method: "exists", ok: false, error: "module probe failed" }]
            }
        })
        model.configuredSourceReports = ({ storage: sourceReport })
        model.configuredSourceObservations = ({
            storage: {
                sourceReport: sourceReport,
                latestAttempt: { transportOk: false, error: "configured endpoint unavailable" },
                status: { known: true, ok: false, detail: "unreachable" }
            }
        })

        compare(FooterStatusProjection.footerFieldValue(footerRoot, "storage.cid_fetch_test"), "true")
        compare(FooterStatusProjection.footerFieldValue(footerRoot, "storage.last_error"), "configured endpoint unavailable")
        compare(FooterStatusProjection.footerFieldTone(footerRoot, "storage.last_error"), "error")
    }

    function test_configured_source_health_detail_is_reported() {
        const sourceReport = {
            health: { ready: false, detail: "capacity unavailable" }
        }
        model.configuredSourceReports = ({ storage: sourceReport })
        model.configuredSourceObservations = ({
            storage: {
                sourceReport: sourceReport,
                latestAttempt: { transportOk: true, error: "" },
                status: { known: true, ok: false, detail: "degraded" }
            }
        })

        compare(FooterStatusProjection.footerFieldValue(footerRoot, "storage.last_error"), "capacity unavailable")
    }
}
