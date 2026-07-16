pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/dashboard/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"
import "fixtures/ZoneFixtureData.js" as ZoneFixtureData

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
        testWindow.width = testRoot.width
        model.metrics.blockchainRefreshRate = 0
        model.metrics.messagingRefreshRate = 0
        model.metrics.storageRefreshRate = 0
        model.metrics.dashboardMetricHistory = ({})
        model.metrics.dashboardMetricLastSeen = ({})
        model.metrics.dashboardMetricHistoryRevision = 0
        model.dashboardNode = null
        model.dashboardLezBlockRows = []
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
        model.zoneInspection.networkScope = ZoneFixtureData.networkScope()
        model.zoneInspection.networkScopeKey = "genesis_id:"
            + ZoneFixtureData.networkScope().genesis_id
        model.zoneInspection.verification = "verified"
        model.zoneInspection.summaryStale = false
        model.zoneInspection.summaryInFlight = false
        model.zoneInspection.summaryError = ""
        model.zoneInspection.zoneSummaries = dashboardZones()
        model.zoneInspection.activeZoneContext = null
        model.shell.currentView = "overview"
        wait(0)
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

    function test_dashboard_uses_zone_catalog_instead_of_global_l2_rows() {
        const panel = findChild(page, "dashboardZonesPanel")
        verify(panel !== null)
        compare(panel.zones.length, 3)
        compare(panel.sequencerCount, 1)
        compare(panel.dataCount, 1)
        verify(hasVisibleText(panel, "Zones"))
        verify(hasVisibleText(panel, "Devnet Settlement / 11111111...111111"))
        verify(!hasVisibleText(page, "Recent L2 Blocks"))
        verify(!hasVisibleText(page, "Recent L2 Transactions"))

        const channelId = ZoneFixtureData.identity("1")
        const row = findChild(panel, "dashboardZoneRow_" + channelId)
        verify(row !== null)
        compare(row.cells[0].text, "Devnet Settlement / 11111111...111111")
        compare(row.cells[1].text, "Active")
        compare(row.cells[2].text, "Reachable")
        compare(row.cells[3].text, "Safe")

        model.dashboardLezBlockRows = [{
            block_id: 999,
            header_hash: ZoneFixtureData.identity("f"),
            transactions: [{ hash: ZoneFixtureData.identity("e") }]
        }]
        compare(panel.zones.length, 3)
        verify(!hasVisibleText(page, "999"))
    }

    function test_narrow_dashboard_layout_stacks_activity_panels() {
        testWindow.width = 820
        const grid = findChild(page, "dashboardActivityGrid")
        const blocks = findChild(page, "dashboardL1BlocksPanel")
        const transactions = findChild(page, "dashboardL1TransactionsPanel")
        const zones = findChild(page, "dashboardZonesPanel")
        verify(grid !== null)
        verify(blocks !== null)
        verify(transactions !== null)
        verify(zones !== null)

        tryVerify(function () {
            return grid.columns === 1
                && blocks.width > 0
                && transactions.width > 0
                && zones.width > 0
        })

        compare(blocks.x, transactions.x)
        compare(blocks.x, zones.x)
        compare(blocks.width, transactions.width)
        compare(blocks.width, zones.width)
        verify(transactions.y >= blocks.y + blocks.height,
               "Recent L1 Transactions must follow Recent L1 Blocks")
        verify(zones.y >= transactions.y + transactions.height,
               "Zones must follow Recent L1 Transactions")
    }

    function test_wide_dashboard_layout_keeps_l1_panels_readable_beside_zones() {
        testWindow.width = 900
        const zonesFixture = dashboardZones()
        const manyZones = []
        for (let index = 0; index < 20; ++index) {
            manyZones.push(zonesFixture[index % zonesFixture.length])
        }
        model.zoneInspection.zoneSummaries = manyZones
        const grid = findChild(page, "dashboardActivityGrid")
        const blocks = findChild(page, "dashboardL1BlocksPanel")
        const transactions = findChild(page, "dashboardL1TransactionsPanel")
        const zones = findChild(page, "dashboardZonesPanel")
        verify(grid !== null)
        verify(blocks !== null)
        verify(transactions !== null)
        verify(zones !== null)

        tryVerify(function () {
            return grid.columns === 2
                && blocks.width > 0
                && transactions.width > 0
                && zones.width > 0
        })

        verify(blocks.width >= 300,
               "Recent L1 Blocks must keep a readable wide-layout column")
        verify(transactions.width >= 300,
               "Recent L1 Transactions must keep a readable wide-layout column")
        verify(zones.width >= 300,
               "Zones must keep a readable wide-layout column")
        compare(blocks.x, transactions.x)
        compare(blocks.width, transactions.width)
        verify(Math.abs(blocks.width - zones.width) <= 1,
               "Dashboard wide-layout columns must be balanced")
        verify(blocks.x + blocks.width + grid.columnSpacing <= zones.x + 1,
               "Recent L1 Blocks must not overlap Zones")
        verify(transactions.x + transactions.width
               + grid.columnSpacing <= zones.x + 1,
               "Recent L1 Transactions must not overlap Zones")
        verify(transactions.y >= blocks.y + blocks.height,
               "Recent L1 Transactions must render below Recent L1 Blocks")
    }

    function test_dashboard_zone_activation_opens_zones() {
        const panel = findChild(page, "dashboardZonesPanel")
        const channelId = ZoneFixtureData.identity("8")
        verify(panel.openZone(channelId))
        compare(model.zoneInspection.activeZoneId, channelId)
        compare(model.shell.currentView, "zones")

        model.zoneInspection.summaryStale = true
        verify(!panel.openZone(ZoneFixtureData.identity("1")))
        compare(model.zoneInspection.activeZoneId, channelId)
    }

    function test_dashboard_configured_sequencer_opens_sequencer_dashboard() {
        const panel = findChild(page, "dashboardZonesPanel")
        const channelId = ZoneFixtureData.identity("1")

        verify(panel.openZone(channelId))
        compare(model.zoneInspection.activeZoneId, channelId)
        compare(model.shell.currentView, "sequencerDashboard")
    }

    function dashboardZones() {
        const scope = ZoneFixtureData.networkScope()
        return ZoneFixtureData.zones().map(function (zone) {
            const sequencer = String(zone.kind || "") === "sequencer_zone"
            return Object.assign({}, zone, {
                active_zone_context_fields: {
                    network_scope: scope,
                    channel_id: String(zone.channel_id || ""),
                    zone_kind: String(zone.kind || "unknown"),
                    selected_sequencer_source_id: sequencer
                        ? "src_11111111111111111111111111111111" : null,
                    indexer_source_id: sequencer
                        ? "src_33333333333333333333333333333333" : null,
                    source_config_revision: sequencer ? 7 : 0
                }
            })
        })
    }

    function hasVisibleText(item, expected) {
        if (!item) {
            return false
        }
        if (item.text !== undefined && String(item.text) === expected
                && item.visible && item.width > 0 && item.height > 0) {
            return true
        }
        if (item.contentItem && hasVisibleText(item.contentItem, expected)) {
            return true
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            if (hasVisibleText(children[i], expected)) {
                return true
            }
        }
        return false
    }
}
