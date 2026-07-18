import QtQuick
import QtTest
import "../../qml/state/metrics/DashboardMetricCatalog.js" as DashboardMetricCatalog

TestCase {
    name: "DashboardMetricCatalog"

    QtObject {
        id: model

        property var dashboardBlocks: [1, 2, 3]
        property var dashboardProvisionalBlocks: [1, 2, 3, 4, 5]
        property var dashboardMetricHistory: ({ "storage.failed_transfers_recent": [{ timestamp: 0, value: 2 }, { timestamp: 2000, value: 7 }] })
        property var dashboardMetricLastSeen: ({})
        property int dashboardMetricHistoryRevision: 0
        property int storageRollingWindow: 60
        property int messagingRollingWindow: 60
        property var peerMetricValue: 6
        property var storageFailures: 7

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
                return storageFailures
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
            return key === "n_peers" ? peerMetricValue : null
        }

        function normalizedDashboardSamples(samples) {
            return DashboardMetricCatalog.normalizedDashboardSamples(samples)
        }

        function nextDashboardSampleTimestamp(previous, now) {
            return DashboardMetricCatalog.nextDashboardSampleTimestamp(previous, now)
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
        compare(DashboardMetricCatalog.dashboardMetricLabel(
            "lez.blocks_produced_recent"
        ), "provisional block records available")
        compare(DashboardMetricCatalog.dashboardMetricTone("storage.failed_transfers_recent", 1), "error")
    }

    function test_available_provisional_block_metric_describes_its_exact_value() {
        const item = DashboardMetricCatalog.dashboardGraphItem(
            model,
            "lez.blocks_produced_recent"
        )

        compare(item.title, "provisional block records available")
        compare(item.group, "LEZ Sequencer")
        compare(item.value, "5")
        compare(item.numericValue, 5)
        verify(!DashboardMetricCatalog.dashboardMetricUsesWindow(
            "lez.blocks_produced_recent"
        ))
    }

    function test_raw_values_are_resolved_from_model_facade() {
        compare(DashboardMetricCatalog.dashboardMetricRawValue(model, "bedrock.peer_count"), 6)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model,
            "lez.blocks_produced_recent"
        ), 5)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(model, "storage.manifest_count"), 9)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(model, "messaging.network_ingress_recent"), 11)
    }

    function test_messaging_peer_count_prefers_connected_libp2p_gauge_from_live_payload() {
        const root = liveMetricRoot("messaging", [
            {
                name: "waku_connected_peers",
                labels: {
                    direction: "Out",
                    protocol: "/vac/waku/relay/2.0.0"
                },
                value: 10
            },
            { name: "libp2p_peers", labels: {}, value: 14 },
            { name: "waku_total_unique_peers", labels: {}, value: 30 }
        ])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(root, "messaging.peer_count"), 14)
    }

    function test_messaging_peer_count_rejects_unaggregated_protocol_peer_series() {
        const root = liveMetricRoot("messaging", [
            {
                name: "waku_connected_peers",
                labels: {
                    direction: "In",
                    protocol: "/vac/waku/relay/2.0.0"
                },
                value: 0
            },
            {
                name: "waku_connected_peers",
                labels: {
                    direction: "Out",
                    protocol: "/vac/waku/relay/2.0.0"
                },
                value: 10
            }
        ])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(root, "messaging.peer_count"), null)
    }

    function test_messaging_peer_count_rejects_total_unique_peer_gauge() {
        const root = liveMetricRoot("messaging", [
            { name: "waku_total_unique_peers", labels: {}, value: 30 }
        ])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(root, "messaging.peer_count"), null)
    }

    function test_messaging_peer_count_rejects_peer_store_size() {
        const root = liveMetricRoot("messaging", [
            { name: "waku_peer_store_size", labels: {}, value: 12 }
        ])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(root, "messaging.peer_count"), null)
    }

    function test_storage_peer_count_prefers_dht_routing_nodes_from_live_payload() {
        const root = liveMetricRoot("storage", [
            { name: "libp2p_peers", labels: {}, value: 0 },
            { name: "dht_routing_table_nodes", labels: {}, value: 25 }
        ])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(root, "storage.peer_count"), 25)
    }

    function test_storage_peer_count_falls_back_to_bare_libp2p_peers_from_live_payload() {
        const root = liveMetricRoot("storage", [
            { name: "libp2p_peers", labels: {}, value: 0 }
        ])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(root, "storage.peer_count"), 0)
    }

    function test_window_metric_uses_sample_delta() {
        const now = Date.now()
        model.dashboardMetricHistory = ({ "storage.failed_transfers_recent": [{ timestamp: now - 1000, value: 2 }, { timestamp: now, value: 7 }] })

        compare(DashboardMetricCatalog.dashboardMetricValue(model, "storage.failed_transfers_recent"), 5)
        compare(DashboardMetricCatalog.windowDeltaFromSamples([{ timestamp: 0, value: 1 }, { timestamp: 1000, value: 4 }], 1000, 1000), 3)
    }

    function test_stable_counter_older_than_window_reports_zero() {
        const now = Date.now()
        model.dashboardMetricHistory = ({
            "storage.failed_transfers_recent": [
                { timestamp: now - 120000, value: 7 }
            ]
        })
        model.dashboardMetricLastSeen = ({
            "storage.failed_transfers_recent": { timestamp: now, value: 7 }
        })

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model,
            "storage.failed_transfers_recent"
        ), 0)
    }

    function test_non_window_graph_includes_latest_observation_endpoint() {
        model.dashboardMetricHistory = ({
            "bedrock.peer_count": [{ timestamp: 100, value: 6 }]
        })
        model.dashboardMetricLastSeen = ({
            "bedrock.peer_count": { timestamp: 200, value: 6 }
        })

        const samples = DashboardMetricCatalog.dashboardMetricSamples(
            model,
            "bedrock.peer_count"
        )

        compare(samples.length, 2)
        compare(samples[0].timestamp, 100)
        compare(samples[1].timestamp, 200)
        compare(samples[1].value, 6)
    }

    function test_sample_normalization_rejects_missing_values_and_keeps_zero() {
        const samples = DashboardMetricCatalog.normalizedDashboardSamples([
            null,
            "",
            " ",
            false,
            0,
            "0",
            { timestamp: 6, value: null },
            { timestamp: 0, value: 0 },
            { timestamp: "1", value: "0" }
        ])

        compare(samples.length, 4)
        compare(samples[0].value, 0)
        compare(samples[1].value, 0)
        compare(samples[2].timestamp, 0)
        compare(samples[2].value, 0)
        compare(samples[3].timestamp, 1)
        compare(samples[3].value, 0)
        compare(DashboardMetricCatalog.normalizedDashboardSample({
            timestamp: 0,
            value: null
        }), null)
    }

    function test_unavailable_metric_does_not_create_zero_history() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.peerMetricValue = null

        DashboardMetricCatalog.recordDashboardSnapshot(model)

        compare(model.dashboardMetricHistory["bedrock.peer_count"], undefined)
        compare(model.dashboardMetricLastSeen["bedrock.peer_count"], undefined)
        compare(DashboardMetricCatalog.dashboardMetricSamples(
            model, "bedrock.peer_count").length, 0)

        model.peerMetricValue = 0
        DashboardMetricCatalog.recordDashboardSnapshot(model)
        compare(model.dashboardMetricHistory["bedrock.peer_count"].length, 1)
        compare(model.dashboardMetricHistory["bedrock.peer_count"][0].value, 0)

        model.peerMetricValue = 6
    }

    function test_unavailable_window_counter_stays_unknown() {
        model.dashboardMetricHistory = ({
            "storage.failed_transfers_recent": [
                { timestamp: Date.now() - 1000, value: 7 }
            ]
        })
        model.dashboardMetricLastSeen = ({})
        model.storageFailures = null

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "storage.failed_transfers_recent"), null)

        model.storageFailures = 7
    }

    function test_selected_items_include_gate_state() {
        const rows = DashboardMetricCatalog.selectedDashboardGraphItems(model)

        compare(rows.length, 3)
        compare(rows[0].key, "bedrock.peer_count")
        compare(rows[2].key, "messaging.message_error_events_recent")
        compare(rows[2].tone, "warning")
        compare(rows[2].value, "blocked")
    }

    function test_graph_item_consumes_metrics_interface_directly() {
        const item = DashboardMetricCatalog.dashboardGraphItem(model, "bedrock.peer_count")

        compare(item.key, "bedrock.peer_count")
        compare(item.numericValue, 6)
        compare(item.value, "6")
    }

    function test_graph_item_keeps_missing_metric_numeric_value_unknown() {
        const missingModel = {
            dashboardMetricValue: function() { return null },
            dashboardMetricSamples: function() { return [] },
            dashboardGate: function() { return null },
            valueText: function(value) { return String(value) }
        }

        const item = DashboardMetricCatalog.dashboardGraphItem(
            missingModel, "bedrock.tip_minus_lib")

        compare(item.value, "n/a")
        verify(!Number.isFinite(item.numericValue))
    }

    function liveMetricRoot(kind, rows) {
        return {
            moduleMetricValue: function(requestKind, names) {
                if (requestKind !== kind) {
                    return null
                }
                const wanted = Array.isArray(names) ? names : [names]
                for (let i = 0; i < rows.length; ++i) {
                    const row = rows[i]
                    for (let j = 0; j < wanted.length; ++j) {
                        const spec = wanted[j]
                        const name = spec && typeof spec === "object"
                            ? String(spec.name || "")
                            : String(spec || "")
                        const labels = spec && typeof spec === "object"
                            ? (spec.labels || {})
                            : {}
                        if (row.name === name && labelsMatch(row.labels, labels)) {
                            return row.value
                        }
                    }
                }
                return null
            }
        }
    }

    function labelsMatch(actual, wanted) {
        const keys = Object.keys(wanted || {})
        for (let i = 0; i < keys.length; ++i) {
            if (String((actual || {})[keys[i]] || "") !== String(wanted[keys[i]])) {
                return false
            }
        }
        return true
    }
}
