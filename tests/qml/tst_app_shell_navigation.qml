pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"
import "fixtures/ZoneFixtureData.js" as ZoneFixtureData

TestCase {
    id: testRoot

    name: "AppShellNavigation"
    when: windowShown
    width: 1180
    height: 820

    ApplicationWindow {
        id: testWindow

        visible: true
        width: 1180
        height: 820

        AppShell {
            id: shell

            anchors.fill: parent
        }
    }

    function test_visible_nav_buttons_replace_loaded_page() {
        const model = findChild(shell, "appModel")
        const loader = findChild(shell, "pageLoader")
        verify(model !== null)
        verify(loader !== null)
        tryVerify(function () { return loader.sourceComponent !== null && loader.item !== null })
        compare(model.shell.currentView, "overview")
        model.chainPages.dashboardNode = null
        compare(model.chainPages.sourceEmptyText("blockchain", "", "No blocks"), "No blocks")

        verifyButtonNavigation(model, loader, "blocks")

        const views = [
            "transactions",
            "zones",
            "blockchain",
            "storage",
            "messaging",
            "favorites",
            "programs",
            "localWallet",
            "localNodes",
            "overview"
        ]
        for (let i = 0; i < views.length; ++i) {
            verifyStateNavigation(model, loader, views[i])
        }
    }

    function init() {
        testWindow.width = testRoot.width
    }

    function cleanup() {
        const model = findChild(shell, "appModel")
        if (model !== null) {
            model.shell.clearResult()
            model.zoneInspection.zoneSummaries = []
        }
    }

    function test_blockchain_raw_response_exposes_exact_accessible_text() {
        const model = findChild(shell, "appModel")
        const loader = findChild(shell, "pageLoader")
        const responseText = "{\n  \"cryptarchia_info\": {\"ok\": true}\n}"
        verify(model !== null)
        verify(loader !== null)

        model.shell.setResult(
            "Blockchain node",
            responseText,
            false,
            ({ cryptarchia_info: { ok: true } }),
            "blockchain"
        )
        model.shell.selectView("blockchain")
        tryVerify(function () { return loader.item !== null })

        let rawResponse = null
        tryVerify(function () {
            rawResponse = findChild(loader.item, "moduleRawResponse")
            return rawResponse !== null
        })
        compare(rawResponse.Accessible.role, Accessible.StaticText)
        compare(rawResponse.Accessible.name, "Raw module response")
        compare(rawResponse.Accessible.description, responseText)

        const errorText = "Blockchain node request failed"
        model.shell.setResult(
            "Blockchain node",
            errorText,
            true,
            null,
            "blockchain"
        )
        tryCompare(rawResponse.Accessible,
                   "name", "Raw module error response")
        tryCompare(rawResponse.Accessible, "description", errorText)
    }

    function test_blockchain_range_validation_is_actionable() {
        const model = findChild(shell, "appModel")
        const loader = findChild(shell, "pageLoader")
        verify(model !== null)
        verify(loader !== null)
        model.shell.selectView("blockchain")
        tryVerify(function () { return loader.item !== null })

        let slotFrom = null
        let slotTo = null
        let slotFromInput = null
        let slotToInput = null
        let loadBlocks = null
        let validation = null
        tryVerify(function () {
            slotFrom = findChild(loader.item, "moduleSlotFrom")
            slotTo = findChild(loader.item, "moduleSlotTo")
            slotFromInput = findChild(loader.item, "moduleSlotFromInput")
            slotToInput = findChild(loader.item, "moduleSlotToInput")
            loadBlocks = findChild(loader.item, "moduleLoadBlocks")
            validation = findChild(loader.item, "moduleBlockRangeValidation")
            return slotFrom !== null && slotTo !== null
                && slotFromInput !== null && slotToInput !== null
                && loadBlocks !== null && validation !== null
        })

        slotFrom.text = "1e3"
        slotTo.text = "1000"
        verify(!loadBlocks.enabled)
        verify(validation.visible)
        compare(validation.Accessible.name,
                "Invalid slot range. Slots must use unsigned decimal integers without signs, spaces, or leading zeros.")
        compare(slotFromInput.Accessible.description,
                "Error: Slots must use unsigned decimal integers without signs, spaces, or leading zeros.")
        compare(slotToInput.Accessible.description, "")

        slotTo.text = "9007199254740992"
        compare(slotFromInput.Accessible.description,
                "Error: Slots must use unsigned decimal integers without signs, spaces, or leading zeros.")
        compare(slotToInput.Accessible.description, "")
        slotFrom.text = "1"
        compare(slotFromInput.Accessible.description, "")
        compare(slotToInput.Accessible.description,
                "Error: Slots exceed the supported numeric range.")

        slotFrom.text = "20"
        slotTo.text = "10"
        verify(!loadBlocks.enabled)
        compare(validation.Accessible.name,
                "Invalid slot range. Slot from must be less than or equal to Slot to.")
        compare(slotFromInput.Accessible.description, "")
        compare(slotToInput.Accessible.description,
                "Error: Slot from must be less than or equal to Slot to.")

        slotFrom.text = "0"
        slotTo.text = "2001"
        verify(!loadBlocks.enabled)
        compare(validation.Accessible.name,
                "Invalid slot range. Slot range cannot contain more than 2,001 slots.")
        compare(slotToInput.Accessible.description,
                "Error: Slot range cannot contain more than 2,001 slots.")

        slotTo.text = "2000"
        verify(loadBlocks.enabled)
        verify(!validation.visible)
        compare(slotFromInput.Accessible.description, "")
        compare(slotToInput.Accessible.description, "")

        slotFrom.text = ""
        slotTo.text = ""
    }

    function test_block_id_validation_is_actionable() {
        const model = findChild(shell, "appModel")
        const loader = findChild(shell, "pageLoader")
        verify(model !== null)
        verify(loader !== null)
        model.shell.selectView("blockchain")
        tryVerify(function () { return loader.item !== null })

        let blockId = null
        let blockIdInput = null
        let loadBlock = null
        let validation = null
        tryVerify(function () {
            blockId = findChild(loader.item, "moduleBlockId")
            blockIdInput = findChild(loader.item, "moduleBlockIdInput")
            loadBlock = findChild(loader.item, "moduleLoadBlock")
            validation = findChild(loader.item, "moduleBlockIdValidation")
            return blockId !== null && blockIdInput !== null && loadBlock !== null
                && validation !== null
        })

        compare(blockIdInput.Accessible.name, "Block ID (required)")
        compare(blockId.placeholderText, "64 hexadecimal characters")
        verify(!loadBlock.enabled)
        compare(blockIdInput.Accessible.description, "")
        verify(!validation.visible)

        blockId.text = "%2e%2e"
        verify(!loadBlock.enabled)
        verify(validation.visible)
        compare(validation.Accessible.name,
                "Invalid block ID. Block ID must be 64 hexadecimal characters (optional 0x prefix).")
        compare(blockIdInput.Accessible.description,
                "Error: Block ID must be 64 hexadecimal characters (optional 0x prefix).")

        blockId.text = "ab".repeat(32)
        verify(loadBlock.enabled)
        verify(!validation.visible)
        compare(blockIdInput.Accessible.description, "")

        blockId.text = ""
    }

    function test_dashboard_wide_layout_keeps_l1_panels_beside_zones() {
        testWindow.width = 1560
        const model = findChild(shell, "appModel")
        const loader = findChild(shell, "pageLoader")
        verify(model !== null)
        verify(loader !== null)
        model.shell.selectView("overview")
        tryVerify(function () {
            return loader.item !== null
                && findChild(loader.item, "dashboardL1BlocksPanel") !== null
                && findChild(loader.item, "dashboardL1TransactionsPanel") !== null
                && findChild(loader.item, "dashboardZonesPanel") !== null
        })

        const fixtureZones = ZoneFixtureData.zones()
        const manyZones = []
        for (let index = 0; index < 20; ++index) {
            manyZones.push(fixtureZones[index % fixtureZones.length])
        }
        model.zoneInspection.verification = "verified"
        model.zoneInspection.summaryStale = false
        model.zoneInspection.zoneSummaries = manyZones

        const grid = findChild(loader.item, "dashboardActivityGrid")
        const blocks = findChild(loader.item, "dashboardL1BlocksPanel")
        const transactions = findChild(loader.item, "dashboardL1TransactionsPanel")
        const zones = findChild(loader.item, "dashboardZonesPanel")
        verify(grid !== null)
        verify(blocks !== null)
        verify(transactions !== null)
        verify(zones !== null)
        tryVerify(function () {
            return blocks.width > 0 && transactions.width > 0 && zones.width > 0
        })

        verify(blocks.width >= 300)
        verify(transactions.width >= 300)
        verify(zones.width >= 300)
        verify(Math.abs(blocks.width - zones.width) <= 1)
        verify(Math.abs(transactions.width - zones.width) <= 1)
        verify(blocks.x + blocks.width + grid.columnSpacing <= zones.x + 1)
        verify(transactions.x + transactions.width
               + grid.columnSpacing <= zones.x + 1)
    }

    function verifyButtonNavigation(model, loader, view) {
        const previousSource = loader.sourceComponent
        let button = null
        tryVerify(function () {
            button = findChild(shell, "navButton_" + view)
            return button !== null && button.visible
        })

        mouseClick(button, button.width / 2, button.height / 2)

        verifyLoadedView(model, loader, view, previousSource)
    }

    function verifyStateNavigation(model, loader, view) {
        const previousSource = loader.sourceComponent
        model.shell.selectView(view)
        verifyLoadedView(model, loader, view, previousSource)
    }

    function verifyLoadedView(model, loader, view, previousSource) {
        tryCompare(model.shell, "currentView", view)
        tryVerify(function () { return loader.sourceComponent !== previousSource })
        tryVerify(function () { return loader.item !== null })
        tryVerify(function () { return loader.item.implicitHeight > 0 && loader.item.height > 0 })
    }
}
