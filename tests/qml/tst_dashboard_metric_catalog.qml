import QtQuick
import QtTest
import "../../qml/state/metrics/DashboardMetricCatalog.js" as DashboardMetricCatalog

TestCase {
    name: "DashboardMetricCatalog"

    QtObject {
        id: model

        property var dashboardBlocks: [1, 2, 3]
        property var dashboardMetricHistory: ({ "storage.failed_transfers_recent": [{ timestamp: 0, value: 2 }, { timestamp: 2000, value: 7 }] })
        property var dashboardMetricLastSeen: ({})
        property int dashboardMetricHistoryRevision: 0
        property int storageRollingWindow: 60
        property int messagingRollingWindow: 60

        function copyMap(value) {
            const copy = {}
            const source = value || {}
            for (const key in source) {
                copy[key] = source[key]
            }
            return copy
        }

        function dashboardGate(key) {
            return {
                enabled: key !== "messaging.message_error_events_recent",
                status: "blocked",
                missing: [],
                warnings: [],
                provenance: ["test"]
            }
        }

        function dashboardGraphEnabled(key) {
            return key === "bedrock.peer_count" || key === "storage.failed_transfers_recent" || key === "messaging.message_error_events_recent"
        }

        function dashboardMetricRawValue(key) {
            return DashboardMetricCatalog.dashboardMetricRawValue(model, key)
        }

        function dashboardMetricSamples(key) {
            return DashboardMetricCatalog.dashboardMetricSamples(model, key)
        }

        function dashboardMetricValue(key) {
            return DashboardMetricCatalog.dashboardMetricValue(model, key)
        }

        function dashboardMetricWindowMs(key) {
            return DashboardMetricCatalog.dashboardMetricWindowMs(model, key)
        }

        function indexerLag() {
            return 4
        }

        function finalityLagSeconds() {
            return 2
        }

        function mantleValue(names) {
            return names && names.indexOf("pending_tx_count") >= 0 ? 8 : null
        }

        function moduleMetricSum(kind, names) {
            if (kind === "storage" && names.indexOf("storage_block_exchange_requests_failed_total") >= 0) {
                return 7
            }
            if (kind === "messaging" && names.indexOf("waku_store_errors_total") >= 0) {
                return 3
            }
            return null
        }

        function moduleMetricValue(kind, names) {
            if (kind === "storage" && metricNames(names).indexOf("storage_shared_files_count") >= 0) {
                return 5
            }
            if (kind === "messaging" && metricNames(names).indexOf("libp2p_network_bytes_in_total") >= 0) {
                return 11
            }
            return null
        }

        function metricNames(values) {
            const rows = []
            const raw = Array.isArray(values) ? values : [values]
            for (let i = 0; i < raw.length; ++i) {
                const value = raw[i]
                rows.push(value && typeof value === "object" ? String(value.name || "") : String(value || ""))
            }
            return rows
        }

        function networkValue(key) {
            return key === "n_peers" ? 6 : null
        }

        function normalizedDashboardSamples(samples) {
            return DashboardMetricCatalog.normalizedDashboardSamples(samples)
        }

        function storageManifestCount() {
            return 9
        }

        function tipMinusLib() {
            return 1
        }

        function trimDashboardMetricSamples(samples) {
            return DashboardMetricCatalog.trimDashboardMetricSamples(samples)
        }

        function valueText(value) {
            return String(value)
        }

        function windowDeltaFromSamples(samples, timestamp, windowMs) {
            return DashboardMetricCatalog.windowDeltaFromSamples(samples, timestamp, windowMs)
        }
    }

    function test_catalog_metadata_drives_status_facade() {
        const keys = DashboardMetricCatalog.dashboardGraphKeys()

        verify(keys.indexOf("bedrock.peer_count") >= 0)
        verify(keys.indexOf("messaging.store_errors_recent") >= 0)
        compare(DashboardMetricCatalog.dashboardMetricGroup("storage.failed_transfers_recent"), "Storage")
        compare(DashboardMetricCatalog.dashboardMetricLabel("storage.failed_transfers_recent"), "transfer failures in window")
        compare(DashboardMetricCatalog.dashboardMetricTone("storage.failed_transfers_recent", 1), "error")
    }

    function test_raw_values_are_resolved_from_model_facade() {
        compare(DashboardMetricCatalog.dashboardMetricRawValue(model, "bedrock.peer_count"), 6)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(model, "lez.blocks_produced_recent"), 3)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(model, "storage.manifest_count"), 9)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(model, "messaging.network_ingress_recent"), 11)
    }

    function test_window_metric_uses_sample_delta() {
        const now = Date.now()
        model.dashboardMetricHistory = ({ "storage.failed_transfers_recent": [{ timestamp: now - 1000, value: 2 }, { timestamp: now, value: 7 }] })

        compare(DashboardMetricCatalog.dashboardMetricValue(model, "storage.failed_transfers_recent"), 5)
        compare(DashboardMetricCatalog.windowDeltaFromSamples([{ timestamp: 0, value: 1 }, { timestamp: 1000, value: 4 }], 1000, 1000), 3)
    }

    function test_selected_items_include_gate_state() {
        const rows = DashboardMetricCatalog.selectedDashboardGraphItems(model)

        compare(rows.length, 3)
        compare(rows[0].key, "bedrock.peer_count")
        compare(rows[2].key, "messaging.message_error_events_recent")
        compare(rows[2].tone, "warning")
        compare(rows[2].value, "blocked")
    }
}
