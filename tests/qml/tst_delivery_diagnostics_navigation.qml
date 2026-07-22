pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/delivery/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "DeliveryDiagnosticsNavigation"
    when: windowShown
    width: 1280
    height: 900

    BridgeHostFixture {
        id: fakeHost
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    Theme {
        id: theme
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        Loader {
            id: pageLoader

            sourceComponent: model.shell.currentView === "diagnosticsDelivery"
                ? deliveryDiagnosticsComponent
                : (model.shell.currentView === "messaging"
                    ? deliveryWorkspaceComponent : emptyComponent)
            width: testWindow.width
        }
    }

    Component {
        id: deliveryDiagnosticsComponent

        DeliveryPage {
            theme: theme
            model: model
            width: testWindow.width
        }
    }

    Component {
        id: deliveryWorkspaceComponent

        DeliveryAppPage {
            theme: theme
            model: model
            width: testWindow.width
        }
    }

    Component {
        id: emptyComponent

        Item {}
    }

    function init() {
        fakeHost.reset()
        model.messagingSourceMode = "logoscore_cli"
        model.networkConnectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "logoscore_cli_delivery_module",
                    provenance: "test"
                }
            }
        })
        wait(0)
        model.metrics.messagingRefreshRate = 0
        model.metrics.messagingMetricsReport = null
        model.metrics.messagingMetricsRevision += 1
        model.metrics.dashboardMetricHistory = ({})
        model.metrics.dashboardMetricLastSeen = ({})
        model.metrics.dashboardMetricSeriesHistory = ({})
        model.metrics.dashboardMetricSeriesLastSeen = ({})
        model.metrics.dashboardMetricHistoryRevision += 1
        model.metrics.resetDeliveryModuleEventTelemetry("unknown", "")
        model.deliveryAppTab = "identity"
        model.deliveryDiagnosticsTab = "overview"
        model.shell.currentView = "diagnosticsDelivery"
        model.navigationBackStack = []
        model.navigationForwardStack = []
        tryVerify(function () {
            return pageLoader.item !== null
                && findAccessibleByName(pageLoader.item, "Overview selected") !== null
        })
        wait(100)
    }

    function test_settings_back_restores_selected_tab() {
        const storeTab = findAccessibleByName(pageLoader.item, "Store")
        verify(storeTab !== null)
        mouseClick(storeTab, storeTab.width / 2, storeTab.height / 2)
        tryCompare(model, "deliveryDiagnosticsTab", "store")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Store selected") !== null
        })

        const openSettings = findAccessibleByName(
            pageLoader.item, "Open Delivery settings")
        verify(openSettings !== null)
        mouseClick(openSettings, openSettings.width / 2, openSettings.height / 2)
        compare(model.shell.currentView, "settings")
        verify(model.canNavigateBack())

        model.deliveryDiagnosticsTab = "overview"
        model.navigateBack()

        compare(model.shell.currentView, "diagnosticsDelivery")
        compare(model.deliveryDiagnosticsTab, "store")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Store selected") !== null
        })
    }

    function test_live_delivery_workflow_routes_replace_dead_placeholders() {
        model.deliveryDiagnosticsTab = "diagnostics"
        tryCompare(model, "deliveryDiagnosticsTab", "diagnostics")
        tryVerify(function () {
            return findAccessibleByName(
                pageLoader.item, "Diagnostics selected") !== null
        })

        let messageTools = null
        let storeTools = null
        tryVerify(function () {
            messageTools = findAccessibleByName(
                pageLoader.item, "Open Delivery message tools")
            storeTools = findAccessibleByName(
                pageLoader.item, "Open Delivery Store tools")
            return messageTools !== null && storeTools !== null
        })

        verify(messageTools.enabled)
        verify(storeTools.enabled)
        verify(findAccessibleByName(pageLoader.item, "Ping peer") === null)
        verify(findAccessibleByName(pageLoader.item, "Lightpush test") === null)
        verify(findAccessibleByName(pageLoader.item, "Adapters pending") === null)

        mouseClick(messageTools,
            messageTools.width / 2, messageTools.height / 2)

        compare(model.deliveryAppTab, "messages")
        compare(model.shell.currentView, "messaging")
        verify(model.canNavigateBack())
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Messages selected") !== null
                && findAccessibleByName(pageLoader.item, "Subscribe") !== null
                && findAccessibleByName(pageLoader.item, "Send") !== null
        })

        model.navigateBack()
        compare(model.shell.currentView, "diagnosticsDelivery")
        compare(model.deliveryAppTab, "identity")
        compare(model.deliveryDiagnosticsTab, "diagnostics")

        tryVerify(function () {
            messageTools = findAccessibleByName(
                pageLoader.item, "Open Delivery message tools")
            storeTools = findAccessibleByName(
                pageLoader.item, "Open Delivery Store tools")
            return messageTools !== null && storeTools !== null
        })

        mouseClick(storeTools, storeTools.width / 2, storeTools.height / 2)

        compare(model.deliveryAppTab, "store")
        compare(model.shell.currentView, "messaging")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Store selected") !== null
                && findAccessibleByName(pageLoader.item, "Query Store") !== null
        })
    }

    function test_store_availability_matches_selected_source_capability() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "delivery",
                label: "Delivery",
                status: "input_required",
                sub_capabilities: ["delivery.store.query"],
                unavailable_sub_capabilities: ["delivery.store.query"]
            }]
        })
        model.deliveryDiagnosticsTab = "store"
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Store selected") !== null
                && findAccessibleByName(pageLoader.item,
                    "Manual query: unavailable. Configure a Store provider multiaddress in Delivery settings.") !== null
                && findAccessibleByName(pageLoader.item,
                    "Payload viewing: unavailable. Payload viewing requires a Store-query-capable source.") !== null
        })

        model.messagingStorePeerAddress = "/dns4/provider.example/tcp/30303/p2p/peer"
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "delivery",
                label: "Delivery",
                status: "available",
                sub_capabilities: ["delivery.store.query"]
            }]
        })

        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Manual query: available. Network / Delivery Store uses LogosCore CLI through the loaded Delivery module. Payloads are excluded by default.") !== null
                && findAccessibleByName(pageLoader.item,
                    "Payload viewing: opt-in. Enable Include payloads for one Store query.") !== null
        })

        model.messagingSourceMode = "rest"
        model.networkConnectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "direct_delivery_rest",
                    endpoint: "http://127.0.0.1:8645",
                    provenance: "test"
                }
            }
        })

        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Manual query: available. Network / Delivery Store uses Direct Waku REST. Payloads are excluded by default.") !== null
                && findAccessibleByName(pageLoader.item,
                    "Payload viewing: opt-in. Enable Include payloads for one Store query.") !== null
        })
    }

    function test_cli_store_page_accepts_one_off_provider() {
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "delivery",
                label: "Delivery",
                status: "available",
                sub_capabilities: ["delivery.store.query"]
            }]
        })
        model.messagingStorePeerAddress = ""
        model.deliveryAppTab = "store"
        model.shell.currentView = "messaging"

        let query = null
        let peer = null
        tryVerify(function () {
            query = findAccessibleByName(pageLoader.item, "Query Store")
            peer = findAccessibleByName(pageLoader.item, "Peer address")
            return query !== null && peer !== null
        })
        verify(!query.enabled)

        peer.text = "/dns4/provider.example/tcp/30303/p2p/peer"
        tryVerify(function () {
            return query.enabled
        })
    }

    function test_topics_report_only_observed_source_facts() {
        model.metrics.messagingMetricsReport = ({
            probes: [{
                probe_key: "collectOpenMetricsText",
                label: "delivery.collectOpenMetricsText",
                ok: true,
                value: "libp2p_pubsub_peers 14\nlibp2p_pubsub_topics 8\n"
            }]
        })
        model.metrics.messagingMetricsRevision += 1
        model.deliveryDiagnosticsTab = "topics"

        tryVerify(function () {
            return findAccessibleByName(pageLoader.item, "Topics selected") !== null
                && findAccessibleByName(pageLoader.item,
                    "Subscribed pubsub topics: 8. OpenMetrics value") !== null
                && findAccessibleByName(pageLoader.item,
                    "Observed content topics: unavailable. LogosCore CLI (Delivery) does not expose a content-topic catalog. Use Delivery Network Monitor for observed topic activity.") !== null
                && findAccessibleByName(pageLoader.item,
                    "Topic-to-shard mapping: unavailable. Current Delivery sources do not expose content-topic-to-pubsub-topic or shard relationships.") !== null
        })
        verify(findAccessibleByName(pageLoader.item,
            "Topic-to-shard mapping: observed. 8 content topic(s)") === null)
        verify(findAccessibleByName(pageLoader.item,
            "Search and filters on this screen never subscribe to topics in the background.") === null)

        model.messagingSourceMode = "network-monitor"
        model.networkConnectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "delivery_network_monitor",
                    endpoint: "http://127.0.0.1:8009",
                    provenance: "test"
                }
            }
        })
        model.metrics.setSourceReport("messaging", ({
            probes: [{
                probe_key: "contentTopics",
                label: "delivery_network_monitor.contentTopics",
                ok: true,
                value: {
                    "/logos/1/zeta/proto": 2,
                    "/logos/1/alpha/proto": 5
                }
            }]
        }), {
            origin: "test",
            checkedAtMs: Date.now()
        })

        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Observed content topics: observed. 2 content topic(s) with received-message counts from Delivery Network Monitor.") !== null
                && findAccessibleByName(pageLoader.item,
                    "Content topic 1: /logos/1/alpha/proto") !== null
                && findAccessibleByName(pageLoader.item,
                    "Copy Content topic 1") !== null
                && findAccessibleByName(pageLoader.item,
                    "Content topic 2: /logos/1/zeta/proto") !== null
        })
    }

    function test_throughput_requires_two_real_observations() {
        const key = "messaging.network_ingress_recent"
        const now = Date.now()
        model.metrics.messagingMetricsReport = deliveryMetricsReport(11)
        model.metrics.messagingMetricsRevision += 1
        model.metrics.dashboardMetricHistory = ({
            "messaging.network_ingress_recent": [
                { timestamp: now, value: 11 }
            ]
        })
        model.metrics.dashboardMetricLastSeen = ({
            "messaging.network_ingress_recent": {
                timestamp: now,
                value: 11
            }
        })
        model.metrics.dashboardMetricHistoryRevision += 1

        const throughputTab = findAccessibleByName(pageLoader.item, "Throughput")
        verify(throughputTab !== null)
        mouseClick(throughputTab,
            throughputTab.width / 2, throughputTab.height / 2)
        tryCompare(model, "deliveryDiagnosticsTab", "throughput")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Network ingress: n/a. Waiting for another source observation.") !== null
        })

        model.metrics.dashboardMetricHistory = ({
            "messaging.network_ingress_recent": [
                { timestamp: now - 1000, value: 5 },
                { timestamp: now, value: 11 }
            ]
        })
        model.metrics.dashboardMetricLastSeen = ({
            "messaging.network_ingress_recent": {
                timestamp: now,
                value: 11
            }
        })
        model.metrics.dashboardMetricHistoryRevision += 1

        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Network ingress: 6. 120 s window") !== null
        })
        compare(model.metrics.dashboardMetricValue(key), 6)
    }

    function test_throughput_accumulates_counter_reset() {
        const key = "messaging.network_ingress_recent"
        const now = Date.now()
        model.metrics.messagingMetricsReport = deliveryMetricsReport(8)
        model.metrics.messagingMetricsRevision += 1
        model.metrics.dashboardMetricHistory = ({
            "messaging.network_ingress_recent": [
                { timestamp: now - 3000, value: 100 },
                { timestamp: now - 2000, value: 110 },
                { timestamp: now - 1000, value: 3 },
                { timestamp: now, value: 8 }
            ]
        })
        model.metrics.dashboardMetricLastSeen = ({
            "messaging.network_ingress_recent": {
                timestamp: now,
                value: 8
            }
        })
        model.metrics.dashboardMetricHistoryRevision += 1

        const throughputTab = findAccessibleByName(pageLoader.item, "Throughput")
        verify(throughputTab !== null)
        mouseClick(throughputTab,
            throughputTab.width / 2, throughputTab.height / 2)
        tryCompare(model, "deliveryDiagnosticsTab", "throughput")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Network ingress: 18. 120 s window") !== null
        })
        compare(model.metrics.dashboardMetricValue(key), 18)
    }

    function test_sent_and_propagated_rows_follow_native_event_watcher() {
        const sentKey = "messaging.message_sent_events_recent"
        const propagatedKey = "messaging.message_propagated_events_recent"

        const throughputTab = findAccessibleByName(pageLoader.item, "Throughput")
        verify(throughputTab !== null)
        mouseClick(throughputTab,
            throughputTab.width / 2, throughputTab.height / 2)
        tryCompare(model, "deliveryDiagnosticsTab", "throughput")
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Confirmed sends: n/a. Waiting for Delivery event watcher readiness.") !== null
                && findAccessibleByName(pageLoader.item,
                    "Network propagations: n/a. Waiting for Delivery event watcher readiness.") !== null
        })

        verify(model.metrics.recordDeliveryModuleEvent("eventStreamReady", {
            object: { status: "ready" }
        }))
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Confirmed sends: n/a. Building continuous Delivery event coverage (120 s remaining).") !== null
                && findAccessibleByName(pageLoader.item,
                    "Network propagations: n/a. Building continuous Delivery event coverage (120 s remaining).") !== null
        })
        const now = model.metrics.deliveryModuleEventNowMs
        model.metrics.deliveryModuleEventCoverageStartedAtMs =
            model.metrics.emptyDeliveryModuleEventCoverage(now - 120001)
        model.metrics.deliveryModuleEventRevision += 1
        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Confirmed sends: 0. 120 s window") !== null
                && findAccessibleByName(pageLoader.item,
                    "Network propagations: 0. 120 s window") !== null
        })

        verify(model.metrics.recordDeliveryModuleEvent("messageSent", {}))
        verify(model.metrics.recordDeliveryModuleEvent("messagePropagated", {}))

        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Confirmed sends: 1. 120 s window") !== null
                && findAccessibleByName(pageLoader.item,
                    "Network propagations: 1. 120 s window") !== null
        })
        compare(model.metrics.dashboardMetricValue(sentKey), 1)
        compare(model.metrics.dashboardMetricValue(propagatedKey), 1)
    }

    function deliveryMetricsReport(networkIngress) {
        return {
            probes: [{
                probe_key: "collectOpenMetricsText",
                label: "delivery.collectOpenMetricsText",
                source: "delivery collectOpenMetricsText",
                ok: true,
                value: "libp2p_network_bytes_total{direction=\"in\"} "
                    + String(networkIngress) + "\n"
            }]
        }
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name || "") === expectedName
                && item.visible) {
            return item
        }
        const children = item.children || []
        for (let index = 0; index < children.length; ++index) {
            const match = findAccessibleByName(children[index], expectedName)
            if (match) {
                return match
            }
        }
        return null
    }
}
