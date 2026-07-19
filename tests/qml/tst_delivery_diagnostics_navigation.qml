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
                ? deliveryDiagnosticsComponent : emptyComponent
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
        id: emptyComponent

        Item {}
    }

    function init() {
        fakeHost.reset()
        model.metrics.messagingRefreshRate = 0
        model.metrics.messagingMetricsReport = null
        model.metrics.messagingMetricsRevision += 1
        model.metrics.dashboardMetricHistory = ({})
        model.metrics.dashboardMetricLastSeen = ({})
        model.metrics.dashboardMetricSeriesHistory = ({})
        model.metrics.dashboardMetricSeriesLastSeen = ({})
        model.metrics.dashboardMetricHistoryRevision += 1
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

    function test_sent_and_propagated_rows_require_two_observations() {
        const sentKey = "messaging.message_sent_events_recent"
        const propagatedKey = "messaging.message_propagated_events_recent"
        const now = Date.now()
        model.metrics.messagingMetricsReport = deliveryEventMetricsReport(
            100, 500)
        model.metrics.messagingMetricsRevision += 1
        model.metrics.dashboardMetricHistory = ({
            "messaging.message_sent_events_recent": [
                { timestamp: now, value: 100 }
            ],
            "messaging.message_propagated_events_recent": [
                { timestamp: now, value: 500 }
            ]
        })
        model.metrics.dashboardMetricLastSeen = ({
            "messaging.message_sent_events_recent": {
                timestamp: now,
                value: 100
            },
            "messaging.message_propagated_events_recent": {
                timestamp: now,
                value: 500
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
                "Sent events: n/a. Waiting for another source observation.") !== null
                && findAccessibleByName(pageLoader.item,
                    "Propagated events: n/a. Waiting for another source observation.") !== null
        })

        model.metrics.messagingMetricsReport = deliveryEventMetricsReport(
            104, 507)
        model.metrics.messagingMetricsRevision += 1
        model.metrics.dashboardMetricHistory = ({
            "messaging.message_sent_events_recent": [
                { timestamp: now - 1000, value: 100 },
                { timestamp: now, value: 104 }
            ],
            "messaging.message_propagated_events_recent": [
                { timestamp: now - 1000, value: 500 },
                { timestamp: now, value: 507 }
            ]
        })
        model.metrics.dashboardMetricLastSeen = ({
            "messaging.message_sent_events_recent": {
                timestamp: now,
                value: 104
            },
            "messaging.message_propagated_events_recent": {
                timestamp: now,
                value: 507
            }
        })
        model.metrics.dashboardMetricHistoryRevision += 1

        tryVerify(function () {
            return findAccessibleByName(pageLoader.item,
                "Sent events: 4. 120 s window") !== null
                && findAccessibleByName(pageLoader.item,
                    "Propagated events: 7. 120 s window") !== null
        })
        compare(model.metrics.dashboardMetricValue(sentKey), 4)
        compare(model.metrics.dashboardMetricValue(propagatedKey), 7)
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

    function deliveryEventMetricsReport(sent, propagated) {
        return {
            probes: [{
                probe_key: "collectOpenMetricsText",
                label: "delivery.collectOpenMetricsText",
                source: "delivery collectOpenMetricsText",
                ok: true,
                value: [
                    "waku_service_requests_total{service=\"/vac/waku/lightpush/3.0.0\",state=\"served\"} "
                        + String(sent),
                    "waku_node_messages_total{type=\"relay\"} "
                        + String(propagated)
                ].join("\n")
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
