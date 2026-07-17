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
        model.chainPages.invalidateOperations("test reset")
        wait(0)
        fakeHost.reset()
        testWindow.width = testRoot.width
        model.metrics.blockchainRefreshRate = 0
        model.metrics.messagingRefreshRate = 0
        model.metrics.storageRefreshRate = 0
        model.metrics.dashboardMetricHistory = ({})
        model.metrics.dashboardMetricLastSeen = ({})
        model.metrics.dashboardMetricHistoryRevision = 0
        model.dashboardNode = null
        model.dashboardL1Blocks = []
        model.blocksPageRows = []
        model.blocksPageSlotTo = 0
        model.transactionsPageRows = []
        model.transactionsPageBeforeBlock = 0
        model.transactionsPageNextBeforeBlock = 0
        model.transactionsPageAtLatest = false
        model.blockDetailValue = null
        model.blockDetailError = ""
        model.transactionDetailValue = null
        model.transactionDetailError = ""
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

    function nodeWithSlots(slot, libSlot) {
        return {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: {
                        slot: slot,
                        lib_slot: libSlot
                    }
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

    function test_dashboard_view_all_actions_are_contextual_and_routed() {
        const blocks = findChild(page, "dashboardL1BlocksViewAll")
        const zones = findChild(page, "dashboardZonesViewAll")
        const transactions = findChild(page, "dashboardL1TransactionsViewAll")
        verify(blocks !== null)
        verify(zones !== null)
        verify(transactions !== null)

        compare(blocks.Accessible.name, "View all L1 blocks")
        compare(zones.Accessible.name, "View all Zones")
        compare(transactions.Accessible.name, "View all L1 transactions")

        mouseClick(blocks)
        compare(model.shell.currentView, "blocks")
        model.shell.selectView("overview")

        mouseClick(zones)
        compare(model.shell.currentView, "zones")
        model.shell.selectView("overview")

        mouseClick(transactions)
        compare(model.shell.currentView, "transactions")
    }

    function test_dashboard_row_actions_expose_entity_and_full_copy_semantics() {
        const blockHash = ZoneFixtureData.identity("a")
        const transactionBlockHash = ZoneFixtureData.identity("c")
        const transactionHash = ZoneFixtureData.identity("b")
        const channelId = ZoneFixtureData.identity("1")
        const unconfiguredChannelId = ZoneFixtureData.identity("8")
        model.dashboardL1Blocks = [{
            header: {
                slot: 123456,
                id: blockHash
            },
            transactions: []
        }]
        model.blocksPageRows = [{
            header: {
                slot: 123455,
                id: transactionBlockHash
            },
            transactions: [{
                mantle_tx: {
                    hash: transactionHash,
                    ops: []
                }
            }]
        }]
        tryVerify(function () {
            return findAccessibleByName(
                page, "Open L1 block at slot 123456") !== null
                && findAccessibleByName(
                    page, "Open Mantle transaction " + transactionHash) !== null
        })

        const slotLink = findAccessibleByName(
            page, "Open L1 block at slot 123456")
        const slotCopy = findAccessibleByName(
            page, "Copy L1 block slot 123456")
        verifyAccessibleControl(slotLink, Accessible.Link)
        verifyAccessibleControl(slotCopy, Accessible.Button)
        compare(slotLink.copyText, "123456")

        const blockLink = findAccessibleByName(
            page, "Open L1 block " + blockHash)
        const blockCopy = findAccessibleByName(
            page, "Copy full L1 block hash " + blockHash)
        verifyAccessibleControl(blockLink, Accessible.Link)
        verifyAccessibleControl(blockCopy, Accessible.Button)
        compare(blockLink.copyText, blockHash)

        const zoneLink = findAccessibleByName(
            page, "Open Sequencer dashboard for Zone " + channelId)
        const zoneCopy = findAccessibleByName(
            page, "Copy Zone channel ID " + channelId)
        verifyAccessibleControl(zoneLink, Accessible.Link)
        verifyAccessibleControl(zoneCopy, Accessible.Button)
        compare(zoneLink.copyText, channelId)
        compare(findAccessibleByName(page, "Open Zone " + channelId), null)

        const unconfiguredZoneLink = findAccessibleByName(
            page, "Open Zone " + unconfiguredChannelId)
        verifyAccessibleControl(unconfiguredZoneLink, Accessible.Link)
        compare(findAccessibleByName(page,
            "Open Sequencer dashboard for Zone "
                + unconfiguredChannelId), null)

        const transactionLink = findAccessibleByName(
            page, "Open Mantle transaction " + transactionHash)
        const transactionCopy = findAccessibleByName(
            page, "Copy full Mantle transaction hash " + transactionHash)
        verifyAccessibleControl(transactionLink, Accessible.Link)
        verifyAccessibleControl(transactionCopy, Accessible.Button)
        compare(transactionLink.copyText, transactionHash)

        const transactionBlockLink = findAccessibleByName(
            page, "Open L1 block " + transactionBlockHash)
        const transactionBlockCopy = findAccessibleByName(
            page, "Copy full L1 block hash " + transactionBlockHash)
        verifyAccessibleControl(transactionBlockLink, Accessible.Link)
        verifyAccessibleControl(transactionBlockCopy, Accessible.Button)
        compare(transactionBlockLink.copyText, transactionBlockHash)
    }

    function test_dashboard_prefers_loaded_transaction_page_rows() {
        const transactionHash = ZoneFixtureData.identity("b")
        const blockHash = ZoneFixtureData.identity("c")
        const oldTransactionHash = ZoneFixtureData.identity("d")
        model.blocksPageRows = [{
            header: {
                slot: 123456,
                id: ZoneFixtureData.identity("e")
            },
            transactions: [{
                mantle_tx: {
                    hash: oldTransactionHash,
                    ops: []
                }
            }]
        }]
        model.transactionsPageRows = [{
            slot: 123457,
            hash: transactionHash,
            block: blockHash,
            ops: 2,
            operations: [],
            raw: { payloadSentinel: "transactions-page" }
        }]
        model.dashboardNode = nodeWithSlots(123457, 123457)
        model.transactionsPageBeforeBlock = 123457
        model.transactionsPageAtLatest = true

        let transactionLink = null
        tryVerify(function () {
            transactionLink = findAccessibleByName(
                page, "Open Mantle transaction " + transactionHash)
            return transactionLink !== null
                && findAccessibleByName(
                    page, "Open L1 block " + blockHash) !== null
        })
        compare(findAccessibleByName(
            page, "Open Mantle transaction " + oldTransactionHash), null)

        const callsBefore = fakeHost.callCount
        transactionLink.activated()
        compare(model.shell.currentView, "transactionDetail")
        compare(model.transactionDetailValue.hash, transactionHash)
        compare(model.transactionDetailValue.raw.payloadSentinel,
                "transactions-page")
        compare(model.transactionDetailError, "")
        compare(fakeHost.callCount, callsBefore)
    }

    function test_dashboard_rejects_older_transaction_page_rows() {
        const currentTransactionHash = ZoneFixtureData.identity("b")
        const historicalTransactionHash = ZoneFixtureData.identity("d")
        model.dashboardNode = nodeWithSlots(6001, 6000)
        model.blocksPageSlotTo = 6001
        model.blocksPageRows = [{
            header: {
                slot: 6001,
                id: ZoneFixtureData.identity("e")
            },
            transactions: [{
                mantle_tx: {
                    hash: currentTransactionHash,
                    ops: []
                }
            }]
        }]
        model.transactionsPageBeforeBlock = 5999
        model.transactionsPageAtLatest = false
        model.transactionsPageRows = [{
            slot: 5999,
            hash: historicalTransactionHash,
            block: ZoneFixtureData.identity("c"),
            ops: 1,
            operations: []
        }]

        tryVerify(function () {
            return findAccessibleByName(
                page, "Open Mantle transaction "
                    + currentTransactionHash) !== null
        })
        compare(findAccessibleByName(
            page, "Open Mantle transaction "
                + historicalTransactionHash), null)
    }

    function test_dashboard_accepts_latest_transaction_page_without_dashboard_node() {
        const transactionHash = ZoneFixtureData.identity("a")
        model.transactionsPageBeforeBlock = 7000
        model.transactionsPageAtLatest = true
        model.transactionsPageRows = [{
            slot: 7000,
            hash: transactionHash,
            block: ZoneFixtureData.identity("c"),
            ops: 1,
            operations: []
        }]

        tryVerify(function () {
            return findAccessibleByName(
                page, "Open Mantle transaction " + transactionHash) !== null
        })
    }

    function test_dashboard_keeps_latest_snapshot_after_lib_advances() {
        const transactionHash = ZoneFixtureData.identity("a")
        model.dashboardNode = nodeWithSlots(6001, 6000)
        model.transactionsPageBeforeBlock = 5999
        model.transactionsPageAtLatest = true
        model.transactionsPageRows = [{
            slot: 5999,
            hash: transactionHash,
            block: ZoneFixtureData.identity("c"),
            ops: 1,
            operations: []
        }]

        tryVerify(function () {
            return findAccessibleByName(
                page, "Open Mantle transaction " + transactionHash) !== null
        })
    }

    function test_dashboard_accepts_block_fallback_without_dashboard_node() {
        const transactionHash = ZoneFixtureData.identity("f")
        model.blocksPageRows = [{
            header: {
                slot: 7000,
                id: ZoneFixtureData.identity("e")
            },
            transactions: [{
                mantle_tx: {
                    hash: transactionHash,
                    ops: []
                }
            }]
        }]

        tryVerify(function () {
            return findAccessibleByName(
                page, "Open Mantle transaction " + transactionHash) !== null
        })
    }

    function test_dashboard_live_block_links_open_exact_payload_data() {
        const hash = ZoneFixtureData.identity("a")
        return [
            {
                tag: "slot",
                hash: hash,
                accessibleName: "Open L1 block at slot 1439857"
            },
            {
                tag: "hash",
                hash: hash,
                accessibleName: "Open L1 block " + hash
            }
        ]
    }

    function test_dashboard_live_block_links_open_exact_payload(data) {
        const callsBefore = fakeHost.callCount
        model.dashboardL1Blocks = [{
            header: { slot: 1439857, id: data.hash },
            transactions: [],
            payloadSentinel: "dashboard-live-block"
        }]
        let link = null
        tryVerify(function () {
            link = findAccessibleByName(page, data.accessibleName)
            return link !== null
        })

        mouseClick(link)

        compare(model.shell.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, data.hash)
        compare(model.blockDetailValue.slot, 1439857)
        compare(model.blockDetailValue.raw.payloadSentinel,
                "dashboard-live-block")
        compare(model.blockDetailError, "")
        compare(fakeHost.callCount, callsBefore)
    }

    function test_dashboard_consensus_block_link_falls_back_to_lookup() {
        const hash = ZoneFixtureData.identity("d")
        model.dashboardNode = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: {
                        slot: 1439856,
                        tip: hash,
                        lib_slot: 1439855,
                        lib: ZoneFixtureData.identity("e")
                    }
                }
            }
        }
        let link = null
        tryVerify(function () {
            link = findAccessibleByName(
                page, "Open L1 block at slot 1439856")
            return link !== null
        })

        mouseClick(link)

        compare(model.shell.currentView, "blockDetail")
        tryVerify(function () { return fakeHost.callCount > 0 })
        compare(fakeHost.lastMethod, "runtimeOperationStart")
        compare(fakeHost.lastArgs[0].method, "blockchainBlock")
    }

    function test_stale_dashboard_zone_is_static_and_not_copyable() {
        const channelId = ZoneFixtureData.identity("1")
        const label = "Devnet Settlement / 11111111...111111"
        const row = findChild(page, "dashboardZoneRow_" + channelId)
        verify(row !== null)

        model.zoneInspection.summaryStale = true
        tryVerify(function () {
            return !row.cells[0].link
                && findAccessibleByName(page, label) !== null
        })

        const cell = findAccessibleByName(row, label)
        const hiddenCopy = findAccessibleByName(
            row, "Copy Zone channel ID " + channelId)
        verifyAccessibleControl(cell, Accessible.StaticText)
        compare(cell.copyText, channelId)
        verify(!row.cellCopyable(row.cells[0]))
        verify(hiddenCopy !== null)
        verify(!hiddenCopy.visible)
        compare(findAccessibleByName(
            row, "Open Zone " + channelId), null)
        compare(findAccessibleByName(row,
            "Open Sequencer dashboard for Zone " + channelId), null)
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

    function test_dashboard_unconfigured_sequencer_name_matches_zone_route() {
        const panel = findChild(page, "dashboardZonesPanel")
        const channelId = ZoneFixtureData.identity("1")
        const configured = dashboardZones()[0]
        const unconfigured = Object.assign({}, configured, {
            active_zone_context_fields: Object.assign(
                {}, configured.active_zone_context_fields, {
                    selected_sequencer_source_id: null
                }),
            settlement_link: Object.assign({}, configured.settlement_link, {
                selected_sequencer_source_id: null
            })
        })
        model.zoneInspection.zoneSummaries = [unconfigured]
        wait(0)

        const zoneLink = findAccessibleByName(page, "Open Zone " + channelId)
        verifyAccessibleControl(zoneLink, Accessible.Link)
        compare(findAccessibleByName(page,
            "Open Sequencer dashboard for Zone " + channelId), null)

        mouseClick(zoneLink)
        compare(model.zoneInspection.activeZoneId, channelId)
        compare(model.shell.currentView, "zones")
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

    function findAccessibleByName(item, expected) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name) === expected) {
            return item
        }
        const children = item.children || []
        for (let index = 0; index < children.length; ++index) {
            const match = findAccessibleByName(children[index], expected)
            if (match) {
                return match
            }
        }
        return null
    }

    function verifyAccessibleControl(item, role) {
        verify(item !== null)
        verify(item.visible)
        verify(item.enabled)
        compare(item.Accessible.role, role)
    }
}
