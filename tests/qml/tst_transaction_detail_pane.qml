import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/components"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "TransactionDetailPane"
    when: windowShown
    width: 900
    height: 700

    readonly property string transactionHash: "5fe02847e96d5b51334150e0479995778147ab75db5a7ef69e7b3e1e32aaf995"
    property var transactionValue: null

    QtObject {
        id: fakeHost

        function callModuleJson(moduleName, method, argsJson) {
            const value = method === "socialCommentTopic"
                ? "/logos/test/cryptarchia/transaction/" + testRoot.transactionHash
                : {}
            return JSON.stringify({
                ok: true,
                value: value,
                text: "",
                error: ""
            })
        }
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

        TransactionDetailPane {
            id: pane

            theme: theme
            model: model
            width: testWindow.width
            value: testRoot.transactionValue
        }
    }

    function init() {
        model.shell.busy = false
        model.shell.currentView = "transactions"
        model.transactionDetailValue = null
        model.transactionsPageRows = [{
            hash: transactionHash,
            block: "80a10055cc8ca01df8134aaacb14935f430848e11c6742dbf690c115101014e2",
            slot: 1430781,
            index: 0,
            operations: [],
            raw: {}
        }]
        transactionValue = {
            type: "blockchain_transaction",
            hash: transactionHash,
            block: "80a10055cc8ca01df8134aaacb14935f430848e11c6742dbf690c115101014e2",
            slot: 1430781,
            index: 0,
            ops: [],
            raw: {}
        }
    }

    function test_primary_hash_is_linked_copyable_and_accessible() {
        const hashLink = findChild(pane, "transactionHashLink")

        verify(hashLink !== null)
        compare(hashLink.text, transactionHash)
        compare(hashLink.link, true)
        compare(hashLink.copyable, true)
        compare(hashLink.copyText, transactionHash)
        compare(hashLink.Accessible.role, Accessible.Link)
        compare(hashLink.Accessible.name, transactionHash)
        compare(pane.primaryHashReferenceKind(), "mantleTransaction")
    }

    function test_primary_hash_reopens_the_same_mantle_transaction() {
        const hashLink = findChild(pane, "transactionHashLink")

        verify(hashLink !== null)
        hashLink.activated()

        compare(model.shell.currentView, "transactionDetail")
        compare(model.transactionDetailValue.hash, transactionHash)
    }

    function test_primary_hash_uses_typed_search_for_lez_transactions() {
        transactionValue = {
            hash: "21828aa8ba4d550d202914cfef9cf38eb35b075c59d86bdf096912c7831ee98f",
            kind: "transfer"
        }

        const hashLink = findChild(pane, "transactionHashLink")
        tryCompare(hashLink, "text", transactionValue.hash)
        compare(pane.primaryHashReferenceKind(), "transaction")
    }

    function test_block_slot_renders_without_scientific_notation() {
        transactionValue = JSON.parse(JSON.stringify({
            type: "blockchain_transaction",
            hash: transactionHash,
            block: transactionValue.block,
            slot: 1486580,
            index: 0,
            ops: [],
            raw: {}
        }))
        const expected = pane.shortHash(transactionValue.block)
            + " (slot " + pane.valueText(transactionValue.slot) + ")"
        let blockLink = null
        tryVerify(function () {
            blockLink = findAccessibleByName(pane, expected)
            return blockLink !== null
        })

        compare(blockLink.text, expected)
        compare(blockLink.link, true)
        compare(blockLink.copyText, transactionValue.block)
        verify(!/[eE][+-][0-9]+/.test(blockLink.text))
    }

    function test_block_slot_preserves_zero_and_numeric_strings() {
        const block = transactionValue.block
        const largestSafeInteger = Number.MAX_SAFE_INTEGER

        compare(pane.blockText({ block: "", slot: 0 }), "slot 0")
        compare(pane.blockText({ block: block, slot: 0 }),
                pane.shortHash(block) + " (slot 0)")
        compare(pane.blockText({ block: "", slot: largestSafeInteger }),
                "slot " + pane.valueText(largestSafeInteger))
        verify(!/[eE][+-][0-9]+/.test(
                   pane.blockText({ block: "", slot: largestSafeInteger })))
        compare(pane.blockText({ block: block, slot: "9007199254740993" }),
                pane.shortHash(block) + " (slot 9007199254740993)")
        compare(pane.blockText({ block: block, slot: null }), block)
    }

    function findAccessibleByName(item, expectedName) {
        if (!item) {
            return null
        }
        if (item.Accessible && String(item.Accessible.name) === expectedName) {
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
