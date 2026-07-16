pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/dashboard/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "OverviewPage"
    when: windowShown
    width: 900
    height: 700

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

        OverviewPage {
            id: page

            theme: theme
            model: model
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.metrics.blockchainRefreshRate = 0
        model.metrics.messagingRefreshRate = 0
        model.metrics.storageRefreshRate = 0
        model.metrics.dashboardMetricHistory = ({})
        model.metrics.dashboardMetricLastSeen = ({})
        model.metrics.dashboardMetricHistoryRevision = 0
        model.dashboardNode = null
        model.capabilityRegistryLoaded = true
        model.capabilityRegistryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "l1",
                label: "L1",
                status: "available"
            }]
        })
        model.metrics.setDashboardGraphEnabled("bedrock.peer_count", true)
    }

    function test_live_dashboard_history_reaches_visible_graph_tile() {
        model.dashboardNode = nodeWithPeerCount(27)
        model.metrics.recordDashboardSnapshot()

        let graphTile = null
        tryVerify(function () {
            graphTile = findChild(page, "dashboardGraphTile")
            return graphTile !== null
        })

        verify(!!graphTile, "Object exists")
        compare(graphTile.title, "peer count")
        tryCompare(graphTile, "value", "27")
        tryCompare(graphTile, "historyPointCount", 1)

        model.dashboardNode = nodeWithPeerCount(28)
        model.metrics.recordDashboardSnapshot()

        compare(model.metrics.dashboardMetricHistory["bedrock.peer_count"].length, 2)
        tryVerify(function () {
            graphTile = findChild(page, "dashboardGraphTile")
            return graphTile !== null && graphTile.value === "28"
        })

        tryCompare(graphTile, "value", "28")
        tryCompare(graphTile, "historyPointCount", 2)
        compare(graphTile.samples.length, 2)
        compare(graphTile.samples[0].value, 27)
        compare(graphTile.samples[1].value, 28)
        compare(graphTile.validSampleCount(), 2)
        compare(graphTile.Accessible.description, "2 history points; current value 28")
    }

    function nodeWithPeerCount(peerCount) {
        return {
            network_info: {
                value: {
                    n_peers: peerCount
                }
            }
        }
    }
}
