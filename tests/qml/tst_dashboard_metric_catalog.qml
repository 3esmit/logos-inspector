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
        property var dashboardMetricSeriesHistory: ({})
        property var dashboardMetricSeriesLastSeen: ({})
        property int dashboardMetricHistoryRevision: 0
        property int storageRollingWindow: 60
        property int messagingRollingWindow: 60
        property var peerMetricValue: 6
        property var storageFailures: 7
        property var metricRows: []
        property string deliveryModuleEventStreamStatus: "unknown"
        property var deliveryModuleEventValues: ({})
        property var deliveryModuleEventSamples: ({})

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

        function deliveryModuleEventMetricValue(key) {
            if (deliveryModuleEventStreamStatus !== "ready") {
                return null
            }
            const value = deliveryModuleEventValues[String(key || "")]
            return value === undefined ? 0 : value
        }

        function deliveryModuleEventMetricSamples(key) {
            const rows = deliveryModuleEventSamples[String(key || "")]
            return Array.isArray(rows) ? rows : []
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
            if (metricRows.length > 0) {
                const wanted = Array.isArray(names) ? names : [names]
                let total = 0
                let found = false
                for (let i = 0; i < wanted.length; ++i) {
                    const value = moduleMetricValue(kind, wanted[i])
                    if (value !== null) {
                        total += Number(value)
                        found = true
                    }
                }
                return found ? total : null
            }
            if (kind === "storage" && names.indexOf("storage_block_exchange_requests_failed_total") >= 0) {
                return storageFailures
            }
            if (kind === "messaging" && names.indexOf("waku_store_errors_total") >= 0) {
                return 3
            }
            return null
        }

        function moduleMetricValue(kind, names) {
            if (metricRows.length > 0) {
                const wanted = Array.isArray(names) ? names : [names]
                for (let i = 0; i < wanted.length; ++i) {
                    const spec = wanted[i]
                    const name = spec && typeof spec === "object"
                        ? String(spec.name || "") : String(spec || "")
                    const labels = spec && typeof spec === "object"
                        ? (spec.labels || {}) : {}
                    for (let j = 0; j < metricRows.length; ++j) {
                        const row = metricRows[j]
                        if (String(row.name || "") === name
                                && labelsMatch(row.labels || {}, labels)) {
                            return row.value
                        }
                    }
                }
                return null
            }
            if (kind === "storage" && metricNames(names).indexOf(
                    "storage_block_exchange_requests_failed_total") >= 0) {
                return storageFailures
            }
            if (kind === "storage" && metricNames(names).indexOf("storage_shared_files_count") >= 0) {
                return 5
            }
            if (kind === "messaging" && metricNames(names).indexOf("libp2p_network_bytes_in_total") >= 0) {
                return 11
            }
            return null
        }

        function moduleMetricSeries(kind, spec) {
            const name = spec && typeof spec === "object"
                ? String(spec.name || "") : String(spec || "")
            const labels = spec && typeof spec === "object"
                ? (spec.labels || {}) : {}
            const rows = []
            if (metricRows.length > 0) {
                for (let i = 0; i < metricRows.length; ++i) {
                    const row = metricRows[i]
                    if (String(row.name || "") === name
                            && labelsMatch(row.labels || {}, labels)) {
                        rows.push({
                            name: name,
                            labels: row.labels || {},
                            value: row.value
                        })
                    }
                }
                return rows
            }
            const value = moduleMetricValue(kind, spec)
            return value === null ? [] : [{
                name: name,
                labels: labels,
                value: value
            }]
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

    function init() {
        model.dashboardMetricHistory = ({
            "storage.failed_transfers_recent": [
                { timestamp: 0, value: 2 },
                { timestamp: 2000, value: 7 }
            ]
        })
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricSeriesHistory = ({})
        model.dashboardMetricSeriesLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.peerMetricValue = 6
        model.storageFailures = 7
        model.metricRows = []
        model.deliveryModuleEventStreamStatus = "unknown"
        model.deliveryModuleEventValues = ({})
        model.deliveryModuleEventSamples = ({})
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
        compare(DashboardMetricCatalog.dashboardMetricLabel(
            "messaging.store_query_requests_recent"
        ), "store queries in window")
        compare(DashboardMetricCatalog.dashboardMetricLabel(
            "messaging.filter_requests_recent"
        ), "filter requests in window")
        compare(DashboardMetricCatalog.dashboardMetricLabel(
            "messaging.lightpush_requests_recent"
        ), "Lightpush requests in window")
        compare(DashboardMetricCatalog.dashboardMetricLabel(
            "messaging.peer_exchange_requests_recent"
        ), "peer exchange requests in window")
        compare(DashboardMetricCatalog.dashboardMetricLabel(
            "messaging.store_errors_recent"
        ), "Store/archive errors in window")
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
        model.dashboardMetricLastSeen = ({
            "storage.failed_transfers_recent": { timestamp: now, value: 7 }
        })
        model.storageFailures = 7

        compare(DashboardMetricCatalog.dashboardMetricValue(model, "storage.failed_transfers_recent"), 5)
        compare(DashboardMetricCatalog.windowDeltaFromSamples([{ timestamp: 0, value: 1 }, { timestamp: 1000, value: 4 }], 1000, 1000), 3)
    }

    function test_sent_and_propagated_metrics_use_independent_module_events() {
        const service = "/vac/waku/lightpush/3.0.0"
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricSeriesHistory = ({})
        model.dashboardMetricSeriesLastSeen = ({})
        model.metricRows = [
            {
                name: "waku_service_requests_total",
                labels: { service: service, state: "served" },
                value: 100
            },
            {
                name: "waku_node_messages_total",
                labels: { type: "relay" },
                value: 500
            }
        ]

        verify(DashboardMetricCatalog.dashboardMetricUsesWindow(
            "messaging.message_sent_events_recent"))
        verify(DashboardMetricCatalog.dashboardMetricUsesWindow(
            "messaging.message_propagated_events_recent"))
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_sent_events_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_propagated_events_recent"), null)
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_sent_events_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_propagated_events_recent"), null)

        model.metricRows = [
            {
                name: "waku_service_requests_total",
                labels: { service: service, state: "served" },
                value: 104
            },
            {
                name: "waku_node_messages_total",
                labels: { type: "relay" },
                value: 507
            }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_sent_events_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_propagated_events_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_received_events_recent"), 7)

        const now = Date.now()
        model.deliveryModuleEventStreamStatus = "ready"
        model.deliveryModuleEventValues = ({
            "messaging.message_sent_events_recent": 1,
            "messaging.message_propagated_events_recent": 1
        })
        model.deliveryModuleEventSamples = ({
            "messaging.message_sent_events_recent": [
                { timestamp: now - 10, value: 1 }
            ],
            "messaging.message_propagated_events_recent": [
                { timestamp: now, value: 1 }
            ]
        })
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_sent_events_recent"), 1)
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_propagated_events_recent"), 1)
        const sentGraph = DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.message_sent_events_recent")
        const propagatedGraph = DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.message_propagated_events_recent")
        compare(sentGraph.length, 1)
        compare(sentGraph[0].value, 1)
        compare(propagatedGraph.length, 1)
        compare(propagatedGraph[0].value, 1)
        model.metricRows = []
    }

    function test_window_metric_accumulates_activity_across_counter_reset() {
        const now = Date.now()
        const samples = [
            { timestamp: now - 3000, value: 100 },
            { timestamp: now - 2000, value: 110 },
            { timestamp: now - 1000, value: 3 },
            { timestamp: now, value: 8 }
        ]

        compare(DashboardMetricCatalog.windowDeltaFromSamples(
            samples, now, 3000), 18)
        compare(samples[0].value, 100)
        compare(samples[2].value, 3)

        model.dashboardMetricHistory = ({
            "storage.failed_transfers_recent": samples
        })
        model.dashboardMetricLastSeen = ({
            "storage.failed_transfers_recent": samples[3]
        })
        model.storageFailures = 8
        const graph = DashboardMetricCatalog.dashboardMetricSamples(
            model, "storage.failed_transfers_recent")
        compare(graph.length, 3)
        compare(graph[0].value, 10)
        compare(graph[1].value, 13)
        compare(graph[2].value, 18)
    }

    function test_window_metric_accumulates_multiple_counter_resets() {
        compare(DashboardMetricCatalog.windowDeltaFromSamples([
            { timestamp: 0, value: 10 },
            { timestamp: 1000, value: 2 },
            { timestamp: 2000, value: 7 },
            { timestamp: 3000, value: 1 },
            { timestamp: 4000, value: 4 }
        ], 4000, 4000), 11)
    }

    function test_aggregate_window_accumulates_each_constituent_across_partial_reset() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.metricRows = [
            { name: "waku_node_errors_total", value: 100 },
            { name: "waku_store_errors_total", value: 50 }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        model.metricRows = [
            { name: "waku_node_errors_total", value: 3 },
            { name: "waku_store_errors_total", value: 55 }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_error_events_recent"), 58)
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_error_events_recent"), 8)
        const graph = DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.message_error_events_recent")
        compare(graph.length, 1)
        compare(graph[0].value, 8)
        model.metricRows = []
    }

    function test_aggregate_window_records_constituent_change_when_total_is_stable() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.metricRows = [
            { name: "waku_node_errors_total", value: 100 },
            { name: "waku_store_errors_total", value: 50 }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        model.metricRows = [
            { name: "waku_node_errors_total", value: 0 },
            { name: "waku_store_errors_total", value: 150 }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_error_events_recent"), 150)
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.message_error_events_recent"), 100)
        const graph = DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.message_error_events_recent")
        compare(graph.length, 1)
        compare(graph[0].value, 100)
        compare(model.dashboardMetricHistory[
            "messaging.message_error_events_recent"].length, 1)
        compare(model.dashboardMetricSeriesHistory[
            "messaging.message_error_events_recent"].length, 2)
        model.metricRows = []
    }

    function test_aggregate_window_stable_second_observation_reports_zero() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.metricRows = [
            { name: "storage_block_exchange_requests_failed_total", value: 100 },
            { name: "storage_block_exchange_peer_timeouts_total", value: 50 }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["storage."])
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["storage."])

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "storage.failed_transfers_recent"), 0)
        const graph = DashboardMetricCatalog.dashboardMetricSamples(
            model, "storage.failed_transfers_recent")
        compare(graph.length, 1)
        compare(graph[0].value, 0)
        model.metricRows = []
    }

    function test_aggregate_alias_change_starts_new_counter_baseline() {
        const service = "/vac/waku/store-query/3.0.0"
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.metricRows = [
            { name: "waku_store_queries_total", value: 100 }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        model.metricRows = [
            { name: "waku_store_queries_total", value: 105 }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.store_query_requests_recent"), 5)
        compare(DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.store_query_requests_recent").length, 1)

        model.metricRows = [{
            name: "waku_service_requests_total",
            labels: { service: service },
            value: 4
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.store_query_requests_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.store_query_requests_recent").length, 0)

        model.metricRows = [{
            name: "waku_service_requests_total",
            labels: { service: service },
            value: 7
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.store_query_requests_recent"), 3)
        const graph = DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.store_query_requests_recent")
        compare(graph.length, 1)
        compare(graph[0].value, 3)
        model.metricRows = []
    }

    function test_aggregate_taxonomy_uses_fallbacks_without_double_counting() {
        model.metricRows = [
            { name: "waku_store_queries_total", value: 9 },
            {
                name: "waku_service_requests_total",
                labels: { service: "/vac/waku/store-query/3.0.0" },
                value: 9
            },
            { name: "waku_filter_requests_total", value: 8 },
            {
                name: "waku_service_requests_total",
                labels: {
                    service: "/vac/waku/filter-subscribe/2.0.0-beta1"
                },
                value: 8
            },
            { name: "waku_lightpush_messages_total", value: 5 },
            { name: "waku_lightpush_v3_messages_total", value: 7 },
            {
                name: "waku_service_requests_total",
                labels: { service: "/vac/waku/lightpush/2.0.0-beta1" },
                value: 5
            },
            {
                name: "waku_service_requests_total",
                labels: { service: "/vac/waku/lightpush/3.0.0" },
                value: 7
            },
            { name: "waku_px_peers_sent_total", value: 25 },
            {
                name: "waku_service_requests_total",
                labels: { service: "/vac/waku/peer-exchange/2.0.0-alpha1" },
                value: 3
            },
            { name: "waku_node_errors_total", value: 2 },
            { name: "waku_node_errors", value: 2 },
            { name: "waku_store_errors_total", value: 4 },
            { name: "waku_archive_errors_total", value: 4 },
            { name: "waku_filter_errors_total", value: 3 },
            { name: "waku_lightpush_errors_total", value: 4 },
            { name: "waku_lightpush_v3_errors_total", value: 5 },
            { name: "message_error_events_recent", value: 18 }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.store_query_requests_recent"), 9)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.filter_requests_recent"), 8)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.lightpush_requests_recent"), 12)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_sent_events_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.peer_exchange_requests_recent"), 3)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.store_errors_recent"), 8)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_error_events_recent"), 22)
        model.metricRows = []
    }

    function test_labeled_relay_bytes_sum_net_topics_and_ignore_gross() {
        model.metricRows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: 100
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "beta" },
                value: 200
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "gross", direction: "in", topic: "alpha" },
                value: 900
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "out", topic: "alpha" },
                value: 40
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "out", topic: "beta" },
                value: 60
            }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.relay_ingress_recent"), 300)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.relay_egress_recent"), 100)
        model.metricRows = []
    }

    function test_labeled_series_window_handles_partial_reset_and_new_topic() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricSeriesHistory = ({})
        model.dashboardMetricSeriesLastSeen = ({})
        model.metricRows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: 100
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "beta" },
                value: 50
            }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        model.metricRows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: 3
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "beta" },
                value: 55
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "gamma" },
                value: 1000
            }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.relay_ingress_recent"), 1058)
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), 8)

        model.metricRows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: 8
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "beta" },
                value: 60
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "gamma" },
                value: 1005
            }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), 23)
        model.metricRows = []
    }

    function test_labeled_series_reordering_does_not_change_identity() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricSeriesHistory = ({})
        model.dashboardMetricSeriesLastSeen = ({})
        model.metricRows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: 100
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "beta" },
                value: 50
            }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        model.metricRows = [
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "beta" },
                value: 55
            },
            {
                name: "waku_relay_network_bytes_total",
                labels: { type: "net", direction: "in", topic: "alpha" },
                value: 107
            }
        ]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), 12)
        model.metricRows = []
    }

    function test_disjoint_topics_keep_prior_activity_for_same_source_family() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricSeriesHistory = ({})
        model.dashboardMetricSeriesLastSeen = ({})
        model.metricRows = [{
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in", topic: "alpha" },
            value: 100
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        model.metricRows = [{
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in", topic: "alpha" },
            value: 105
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), 5)

        model.metricRows = [{
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in", topic: "beta" },
            value: 200
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), 5)

        model.metricRows = [{
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in", topic: "beta" },
            value: 203
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), 8)
        const samples = DashboardMetricCatalog.dashboardMetricSamples(
            model, "messaging.relay_ingress_recent")
        compare(samples[samples.length - 1].value, 8)
        model.metricRows = []
    }

    function test_labeled_series_absence_breaks_only_that_series_continuity() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricSeriesHistory = ({})
        model.dashboardMetricSeriesLastSeen = ({})
        model.metricRows = [{
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in", topic: "alpha" },
            value: 100
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        model.metricRows = []
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])

        model.metricRows = [{
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in", topic: "alpha" },
            value: 105
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), null)

        model.metricRows = [{
            name: "waku_relay_network_bytes_total",
            labels: { type: "net", direction: "in", topic: "alpha" },
            value: 108
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["messaging."])
        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "messaging.relay_ingress_recent"), 3)
        model.metricRows = []
    }

    function test_labeled_fallback_and_error_families_sum_all_matching_series() {
        model.metricRows = [
            {
                name: "waku_service_requests_total",
                labels: {
                    service: "/vac/waku/store-query/3.0.0",
                    state: "served"
                },
                value: 10
            },
            {
                name: "waku_service_requests_total",
                labels: {
                    service: "/vac/waku/store-query/3.0.0",
                    state: "rejected"
                },
                value: 2
            },
            {
                name: "waku_node_errors_total",
                labels: { type: "keep_alive_failure" },
                value: 3
            },
            {
                name: "waku_node_errors_total",
                labels: { type: "dial_failure" },
                value: 4
            }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.store_query_requests_recent"), 12)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_error_events_recent"), 7)
        model.metricRows = []
    }

    function test_labeled_canonical_families_sum_types_before_service_fallbacks() {
        model.metricRows = [
            {
                name: "waku_filter_requests_total",
                labels: { type: "PING" },
                value: 3
            },
            {
                name: "waku_filter_requests_total",
                labels: { type: "SUBSCRIBE" },
                value: 4
            },
            {
                name: "waku_service_requests_total",
                labels: {
                    service: "/vac/waku/filter-subscribe/2.0.0-beta1",
                    state: "served"
                },
                value: 100
            },
            {
                name: "waku_lightpush_messages_total",
                labels: { type: "request" },
                value: 5
            },
            {
                name: "waku_lightpush_messages_total",
                labels: { type: "response" },
                value: 6
            },
            {
                name: "waku_lightpush_v3_messages_total",
                labels: { type: "request" },
                value: 7
            },
            {
                name: "waku_lightpush_v3_messages_total",
                labels: { type: "response" },
                value: 8
            },
            {
                name: "waku_node_messages_total",
                labels: { type: "relay" },
                value: 10
            },
            {
                name: "waku_node_messages_total",
                labels: { type: "store" },
                value: 2
            }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.filter_requests_recent"), 7)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.lightpush_requests_recent"), 26)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_sent_events_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_received_events_recent"), 12)
        model.metricRows = []
    }

    function test_service_bytes_sum_each_direction_across_services() {
        model.metricRows = [
            {
                name: "waku_service_network_bytes_total",
                labels: { service: "store", direction: "in" },
                value: 10
            },
            {
                name: "waku_service_network_bytes_total",
                labels: { service: "filter", direction: "in" },
                value: 20
            },
            {
                name: "waku_service_network_bytes_total",
                labels: { service: "store", direction: "out" },
                value: 7
            },
            {
                name: "waku_service_network_bytes_total",
                labels: { service: "filter", direction: "out" },
                value: 9
            }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.service_ingress_recent"), 30)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.service_egress_recent"), 16)
        model.metricRows = []
    }

    function test_storage_failure_uses_terminal_counter_and_generated_suffix() {
        model.metricRows = [
            {
                name: "storage_block_exchange_requests_failed_total_total",
                value: 5
            },
            {
                name: "storage_block_exchange_peer_timeouts_total_total",
                value: 7
            }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "storage.failed_transfers_total"), 5)
        model.metricRows = [{
            name: "storage_block_exchange_peer_timeouts_total_total",
            value: 7
        }]
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "storage.failed_transfers_total"), null)
        model.metricRows = []
    }

    function test_lightpush_counter_sample_names_include_total_suffix() {
        model.metricRows = [
            { name: "waku_lightpush_messages_total", value: 5 },
            { name: "waku_lightpush_v3_messages_total", value: 7 }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.lightpush_requests_recent"), 12)
        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.message_sent_events_recent"), null)
        model.metricRows = []
    }

    function test_peer_exchange_peer_quantity_is_not_a_request_count() {
        model.metricRows = [
            { name: "waku_px_peers_sent_total", value: 25 }
        ]

        compare(DashboardMetricCatalog.dashboardMetricRawValue(
            model, "messaging.peer_exchange_requests_recent"), null)
        model.metricRows = []
    }

    function test_window_metric_excludes_reset_before_selected_baseline() {
        compare(DashboardMetricCatalog.windowDeltaFromSamples([
            { timestamp: 0, value: 100 },
            { timestamp: 1000, value: 3 },
            { timestamp: 2000, value: 8 }
        ], 2000, 1000), 5)
    }

    function test_one_observed_window_sample_stays_unknown() {
        model.dashboardMetricHistory = ({
            "storage.failed_transfers_recent": [
                { timestamp: 1000, value: 7 }
            ]
        })
        model.dashboardMetricLastSeen = ({
            "storage.failed_transfers_recent": { timestamp: 1000, value: 7 }
        })
        model.storageFailures = 7

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "storage.failed_transfers_recent"), null)
        compare(DashboardMetricCatalog.dashboardMetricSamples(
            model, "storage.failed_transfers_recent").length, 0)
    }

    function test_current_counter_change_requires_recorded_observation() {
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.metricRows = [{
            name: "storage_block_exchange_requests_failed_total",
            value: 2
        }]
        DashboardMetricCatalog.recordDashboardSnapshot(model, ["storage."])

        model.metricRows = [{
            name: "storage_block_exchange_requests_failed_total",
            value: 7
        }]

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "storage.failed_transfers_recent"), null)
        compare(model.dashboardMetricHistory[
            "storage.failed_transfers_recent"].length, 1)

        DashboardMetricCatalog.recordDashboardSnapshot(model, ["storage."])

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "storage.failed_transfers_recent"), 5)
        model.metricRows = []
    }

    function test_stale_observed_window_is_unknown() {
        const now = Date.now()
        model.dashboardMetricHistory = ({
            "storage.failed_transfers_recent": [
                { timestamp: now - 120000, value: 2 },
                { timestamp: now - 119000, value: 7 }
            ]
        })
        model.dashboardMetricLastSeen = ({
            "storage.failed_transfers_recent": {
                timestamp: now - 119000,
                value: 7
            }
        })
        model.storageFailures = 7

        compare(DashboardMetricCatalog.dashboardMetricValue(
            model, "storage.failed_transfers_recent"), null)
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
