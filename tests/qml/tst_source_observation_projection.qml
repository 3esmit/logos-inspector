import QtQuick
import QtTest
import "../../qml/state/source_routing/SourceObservationProjection.js" as SourceObservation
import "../../qml/state/source_routing/SourceInspectionReadModel.js" as SourceReadModel

TestCase {
    id: testRoot

    name: "SourceObservationProjection"
    property bool storageMetricsConfigured: false
    property string storageDataDirProbe: ""
    property var storageDebugProbe: null

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
        property string storageDataDir: "/tmp/stale-configured-path"
        property bool storageLocalDiagnosticsEnabled: false
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
        function storageDisplayPath(value) {
            const text = String(value || "")
            if (!text.length) {
                return ""
            }
            return storageLocalDiagnosticsEnabled ? text : "redacted"
        }
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
            if (method === "dataDir" && testRoot.storageDataDirProbe.length > 0) {
                return testRoot.storageDataDirProbe
            }
            if (method === "debug") {
                return testRoot.storageDebugProbe
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
        function sourceLabel() { return "Direct REST" }
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
        function valueSummary(value) {
            if (value === null || value === undefined) {
                return "n/a"
            }
            return typeof value === "object" ? JSON.stringify(value) : String(value)
        }
        function copyValue(value) {
            if (value !== null && typeof value === "object") {
                return JSON.stringify(value)
            }
            return String(value || "")
        }
        function shortText(value, maxLength) { return String(value || "").slice(0, maxLength || 32) }
        function statusRow(label, state, evidence, tone) {
            return { label: label, state: state, evidence: evidence, tone: tone }
        }
        function detailRow(label, value) {
            return { label: label, value: value === null ? "n/a" : String(value) }
        }
        function pathDetailRow(label, value) {
            return SourceObservation.storagePathDetailRow(storagePage, label, value)
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
        testRoot.storageDataDirProbe = ""
        testRoot.storageDebugProbe = null
        storageModel.storageLocalDiagnosticsEnabled = false
    }

    function storageDebugFixture(nodeCount) {
        const nodes = []
        for (let i = 0; i < nodeCount; ++i) {
            nodes.push({
                peerId: "routing-peer-" + (i + 1),
                address: "/ip4/10.0.0." + (i + 1) + "/tcp/3000",
                nodeId: "routing-node-" + (i + 1),
                record: "routing-record-" + (i + 1),
                seen: i + 10
            })
        }
        return {
            id: "debug-peer-id",
            addrs: [
                "/ip4/127.0.0.1/tcp/8070",
                "/ip4/10.0.0.2/tcp/8070"
            ],
            announceAddresses: ["/dns4/storage.test/tcp/443/wss"],
            libp2pPubKey: "debug-libp2p-public-key",
            mixPubKey: null,
            providerRecord: "debug-provider-record",
            spr: "debug-self-peer-record",
            storage: {
                version: "0.1.0-test",
                revision: "debug-revision"
            },
            table: {
                localNode: {
                    peerId: "debug-peer-id",
                    address: "/ip4/127.0.0.1/tcp/8070",
                    nodeId: "debug-local-node",
                    record: "debug-local-record",
                    seen: 1
                },
                nodes: nodes
            }
        }
    }

    function detailRowByLabel(rows, label) {
        for (let i = 0; i < rows.length; ++i) {
            if (rows[i].label === label) {
                return rows[i]
            }
        }
        return null
    }

    function test_storage_projection_extracts_identity_and_rows() {
        compare(SourceObservation.storageIdentityEvidence(storagePage), "peer id present")
        compare(SourceObservation.storageTransferSummary(storagePage), "4 upload requests / 2 download requests")
        compare(SourceObservation.storageSpaceRow(storagePage, "Quota used", ["usedBytes"]).state, "4096")
        compare(SourceObservation.storageHealthRows(storagePage).length, 8)
    }

    function test_storage_paths_require_source_evidence_and_respect_privacy() {
        let repository = SourceObservation.storageRepositoryRows(storagePage)
        let identity = SourceObservation.storageIdentityRows(storagePage)

        compare(repository[0].state, "unknown")
        compare(repository[0].evidence, "No path reported by current source.")
        compare(repository[0].tone, "neutral")
        compare(identity[2].value, "n/a")
        compare(identity[2].copyText, "")

        testRoot.storageDataDirProbe = "/var/lib/logos/storage"
        repository = SourceObservation.storageRepositoryRows(storagePage)
        identity = SourceObservation.storageIdentityRows(storagePage)

        compare(repository[0].state, "reported")
        compare(repository[0].evidence, "redacted")
        compare(repository[0].tone, "success")
        compare(identity[2].value, "redacted")
        compare(identity[2].copyText, "")

        storageModel.storageLocalDiagnosticsEnabled = true
        repository = SourceObservation.storageRepositoryRows(storagePage)
        identity = SourceObservation.storageIdentityRows(storagePage)

        compare(repository[0].evidence, "/var/lib/logos/storage")
        compare(identity[2].value, "/var/lib/logos/storage")
        compare(identity[2].copyText, "/var/lib/logos/storage")
    }

    function test_storage_data_directory_probe_evidence_respects_privacy() {
        const probe = {
            probe_key: "dataDir",
            label: "Data directory",
            source: "storage_module",
            ok: true,
            value: "/var/lib/logos/storage"
        }

        let row = SourceReadModel.sourceProbeRow(
            storagePage, true, probe, "Probe")

        compare(row.evidence, "redacted")

        storageModel.storageLocalDiagnosticsEnabled = true
        row = SourceReadModel.sourceProbeRow(
            storagePage, true, probe, "Probe")

        compare(row.evidence, "/var/lib/logos/storage")
    }

    function test_storage_network_debug_rows_expose_structured_payload() {
        const debug = storageDebugFixture(2)
        testRoot.storageDebugProbe = debug

        const rows = SourceObservation.storageNetworkDebugRows(storagePage, 50)
        const snapshot = detailRowByLabel(rows, "Network snapshot")
        const peerId = detailRowByLabel(rows, "Network peer ID")
        const listenAddress = detailRowByLabel(rows, "Listen address 1")
        const announceAddress = detailRowByLabel(rows, "Announce address 1")
        const localNode = detailRowByLabel(rows, "DHT local node")
        const routingNodes = detailRowByLabel(rows, "DHT routing nodes")
        const firstNode = detailRowByLabel(rows, "Routing node 1")

        verify(snapshot !== null)
        compare(snapshot.value, "9 field(s); 2 routing node(s)")
        compare(snapshot.copyText, JSON.stringify(debug))
        compare(peerId.value, debug.id)
        compare(peerId.copyText, debug.id)
        compare(listenAddress.value, debug.addrs[0])
        compare(listenAddress.copyText, debug.addrs[0])
        compare(announceAddress.value, debug.announceAddresses[0])
        compare(localNode.copyText, JSON.stringify(debug.table.localNode))
        compare(routingNodes.value, "2 node(s); showing 2")
        compare(routingNodes.copyText, JSON.stringify(debug.table.nodes))
        compare(firstNode.value, "routing-peer-1 | /ip4/10.0.0.1/tcp/3000 | routing-node-1")
        compare(firstNode.copyText, JSON.stringify(debug.table.nodes[0]))
        compare(detailRowByLabel(rows, "libp2p public key").value,
                debug.libp2pPubKey)
        compare(detailRowByLabel(rows, "Provider record").value,
                debug.providerRecord)
        compare(detailRowByLabel(rows, "Self peer record").value, debug.spr)
        compare(detailRowByLabel(rows, "Storage version").value,
                debug.storage.version)
        compare(detailRowByLabel(rows, "Storage revision").value,
                debug.storage.revision)
    }

    function test_storage_network_debug_rows_bound_copy() {
        const debug = storageDebugFixture(55)
        debug.addrs = []
        for (let i = 0; i < 55; ++i) {
            debug.addrs.push("/ip4/10.1.0." + (i + 1) + "/tcp/8070")
        }
        testRoot.storageDebugProbe = debug

        const rows = SourceObservation.storageNetworkDebugRows(storagePage, 3)
        let routingNodeRows = 0
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i].label).indexOf("Routing node ") === 0) {
                routingNodeRows += 1
            }
        }

        compare(routingNodeRows, 3)
        compare(detailRowByLabel(rows, "DHT routing nodes").value,
                "55 node(s); showing 3")
        compare(detailRowByLabel(rows, "DHT routing nodes").copyText,
                JSON.stringify(debug.table.nodes))
        compare(detailRowByLabel(rows, "Network snapshot").copyText,
                JSON.stringify(debug))
        verify(detailRowByLabel(rows, "Routing node 4") === null)
        compare(detailRowByLabel(rows, "Listen address list").value,
                "55 item(s); showing 3")
        compare(detailRowByLabel(rows, "Listen address list").copyText,
                JSON.stringify(debug.addrs))
        verify(detailRowByLabel(rows, "Listen address 3") !== null)
        verify(detailRowByLabel(rows, "Listen address 4") === null)
    }

    function test_storage_network_debug_rows_are_empty_without_debug_probe() {
        compare(SourceObservation.storageNetworkDebugRows(storagePage, 50).length, 0)
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
