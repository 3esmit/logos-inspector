import QtQuick
import QtTest
import "../../qml/state/source_routing/SourceObservationProjection.js" as SourceObservation

TestCase {
    id: testRoot

    name: "SourceObservationProjection"
    property bool storageMetricsConfigured: false

    QtObject {
        id: sourceRoutingStub

        function storageSourceTarget() { return "http://storage" }
    }

    QtObject {
        id: storageMetrics

        function dashboardMetricValue(key) {
            return storageModel.metricValues[String(key || "")]
        }
        function sourceCapabilityAvailable(report, key) { return false }
        function valueText(value) { return String(value) }
        function scalarValue(value) { return value }
    }

    QtObject {
        id: storageModel

        property int storageRollingWindow: 30
        property string storageCidProbe: ""
        property string storageDataDir: "/tmp/storage"
        property bool storageMutatingDiagnosticsEnabled: false
        property var sourceRouting: sourceRoutingStub
        property var metricValues: ({
            "storage.active_uploads": 4,
            "storage.active_downloads": 2,
            "storage.failed_transfers_recent": 0,
            "storage.local_storage_used": 1024
        })
        readonly property alias metrics: storageMetrics

        function valueText(value) { return String(value) }
        function storageDisplayPath(value) { return String(value || "") }
    }

    QtObject {
        id: storagePage

        property var model: storageModel
        property var theme: ({ textMuted: "muted", error: "error", success: "success" })

        function sourceFactEvidence(key, fallback) { return fallback }
        function sourceFactAvailable(key) { return false }
        function probeValue(method) {
            if (method === "peerId") {
                return "peer-1"
            }
            if (method === "space") {
                return { usedBytes: 4096, quotaMaxBytes: 8192 }
            }
            return null
        }
        function probeKnown(method) { return probeValue(method) !== null }
        function probe(method) { return null }
        function metricDisplay(key) {
            const value = storageModel.metrics.dashboardMetricValue(key)
            return value === undefined ? "n/a" : String(value)
        }
        function metricKnown(key) { return storageModel.metrics.dashboardMetricValue(key) !== undefined }
        function failedProbeCount() { return 0 }
        function sourceName() { return "Direct REST" }
        function status() { return { known: true, ok: true, detail: "ok" } }
        function statusTone() { return "success" }
        function storageSourceMode() { return "rest" }
        function rollingWindow() { return 30 }
        function storageCidProbe() { return "" }
        function sourceMutatingDiagnosticsEnabled() { return false }
        function sourceTarget() { return "http://storage" }
        function sourceNetworkPreset() { return "logos.test" }
        function sourceRestEndpoint() { return "http://storage" }
        function sourceMetricsEndpoint() { return "http://storage/metrics" }
        function metricsEndpointConfigured() { return testRoot.storageMetricsConfigured }
        function report() { return null }
        function valueSummary(value) { return value === null || value === undefined ? "n/a" : JSON.stringify(value) }
        function copyValue(value) { return String(value || "") }
        function shortText(value, maxLength) { return String(value || "").slice(0, maxLength || 32) }
        function statusRow(label, state, evidence, tone) {
            return { label: label, state: state, evidence: evidence, tone: tone }
        }
        function metricRow(label, key) { return SourceObservation.storageMetricRow(storagePage, label, key) }
        function metricEvidence(key) { return SourceObservation.storageMetricEvidence(storagePage, key) }
        function capacitySummary() { return SourceObservation.storageCapacitySummary(storagePage) }
        function activeDownloadRow() { return SourceObservation.storageActiveDownloadRow(storagePage) }
        function activeStorageOperation() { return null }
        function activeStorageOperationDetail(operation) { return SourceObservation.storageActiveStorageOperationDetail(storagePage, operation) }
        function restMetricsState() { return SourceObservation.storageRestMetricsState(storagePage) }
        function restMetricsEvidence() { return SourceObservation.storageRestMetricsEvidence(storagePage) }
        function restMetricsTone() { return SourceObservation.storageRestMetricsTone(storagePage) }
        function protocolRow(label, protocolId, observed, evidence) { return SourceObservation.storageProtocolRow(label, protocolId, observed, evidence) }
        function spaceRow(label, keys) { return SourceObservation.storageSpaceRow(storagePage, label, keys) }
        function manifestCountRow() { return SourceObservation.storageManifestCountRow(storagePage) }
    }

    QtObject {
        id: deliveryMetrics

        function dashboardMetricValue(key) {
            return deliveryModel.metricValues[String(key || "")]
        }
        function dashboardMetricUsesWindow(key) { return true }
        function deliveryHealthValueOk(value, fallback) {
            return String(value || "") === "ready"
        }
        function sourceCapabilityAvailable(report, key) { return false }
        function valueText(value) { return String(value) }
        function scalarValue(value) { return value }
    }

    QtObject {
        id: deliveryModel

        property int messagingRollingWindow: 45
        property string messagingNetworkPreset: "logos.test"
        property var metricValues: ({
            "messaging.store_peers": 2,
            "messaging.filter_peers": 1,
            "messaging.lightpush_peers": 3,
            "messaging.content_topics": 5
        })
        readonly property alias metrics: deliveryMetrics

        function normalizedMessagingNetworkPreset(value) { return value }
        function scalarValue(value) { return value }
    }

    QtObject {
        id: deliveryPage

        property var model: deliveryModel

        function sourceFactEvidence(key, fallback) { return fallback }
        function sourceFactAvailable(key) { return false }
        function probeValue(method) {
            if (method === "protocolsHealth") {
                return [{ protocol: "relay", health: "ready", desc: "ok" }]
            }
            if (method === "allPeersInfo") {
                return { peers: ["a", "b"] }
            }
            return null
        }
        function metricKnown(key) { return deliveryModel.metrics.dashboardMetricValue(key) !== undefined }
        function metricDisplay(key) { return String(deliveryModel.metrics.dashboardMetricValue(key)) }
        function valueSummary(value) { return value === undefined || value === null ? "unknown" : String(value) }
        function statusRow(label, state, evidence, tone) {
            return { label: label, state: state, evidence: evidence, tone: tone }
        }
        function protocolHealthEntry(item) { return SourceObservation.deliveryProtocolHealthEntry(deliveryPage, item) }
        function protocolHealthDetail(protocol, description) { return SourceObservation.deliveryProtocolHealthDetail(deliveryPage, protocol, description) }
        function protocolLabel(key) { return SourceObservation.deliveryProtocolLabel(key) }
        function healthValueTone(value) { return SourceObservation.deliveryHealthValueTone(deliveryPage, value) }
        function protocolHealthRows() { return SourceObservation.deliveryProtocolHealthRows(deliveryPage) }
        function deliverySourceMode() { return "metrics" }
        function rollingWindow() { return 45 }
        function sourceNetworkPreset() { return "logos.test" }
        function sourceRestEndpoint() { return "http://delivery" }
        function sourceMetricsEndpoint() { return "http://delivery/metrics" }
        function sourceTarget() { return "http://delivery" }
        function report() { return null }
        function statusTone() { return "success" }
    }

    function init() {
        testRoot.storageMetricsConfigured = false
    }

    function test_storage_projection_extracts_identity_and_rows() {
        compare(SourceObservation.storageIdentityEvidence(storagePage), "peer id present")
        compare(SourceObservation.storageTransferSummary(storagePage), "4 upload requests / 2 download requests")
        compare(SourceObservation.storageSpaceRow(storagePage, "Quota used", ["usedBytes"]).state, "4096")
        compare(SourceObservation.storageHealthRows(storagePage).length, 8)
    }

    function test_delivery_projection_extracts_protocol_health_and_counts() {
        const rows = SourceObservation.deliveryProtocolHealthRows(deliveryPage)

        compare(rows.length, 1)
        compare(rows[0].label, "Relay")
        compare(rows[0].tone, "success")
        compare(SourceObservation.deliveryNetworkMonitorPeerCount(deliveryPage), 2)
        compare(SourceObservation.deliveryServicePeerCount(deliveryPage), 6)
    }

    function test_projection_reads_focused_metrics_interface() {
        testRoot.storageMetricsConfigured = true

        compare(SourceObservation.storageRestMetricsTone(storagePage), "warning")
        compare(SourceObservation.deliveryHealthValueTone(deliveryPage, "ready"), "success")
        compare(SourceObservation.deliveryMetricEvidence(deliveryPage, "messaging.store_peers"), "45 s window")
        compare(SourceObservation.deliveryRestMetricsTone(deliveryPage), "warning")
    }
}
