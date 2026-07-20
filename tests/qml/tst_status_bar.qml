import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/components"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "StatusBar"
    when: windowShown
    width: 900
    height: 120

    readonly property string transactionHash: "0340d657f1ad70b7ba276b896e0480976f65424a28d833dafb69f7de76873f29"

    QtObject {
        id: fakeHost

        property var calls: []

        function callModuleJson(moduleName, method, argsJson) {
            const next = calls.slice()
            next.push({
                moduleName: moduleName,
                method: method,
                args: JSON.parse(argsJson)
            })
            calls = next
            return JSON.stringify({
                ok: true,
                value: {},
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

        StatusBar {
            id: statusBar

            theme: theme
            model: model
            width: testWindow.width
        }
    }

    function init() {
        model.shell.busy = false
        model.shell.currentView = "overview"
        model.transactionDetailValue = null
        model.transactionsPageRows = [{
            hash: transactionHash,
            block: "31568dea8aa83b2888bcac3bf486262fd4273b6aca5b2ec877bff7e8e2390575",
            slot: 1431649,
            index: 0,
            operations: [],
            raw: {}
        }]
        fakeHost.calls = []
    }

    function callFor(method) {
        for (let i = fakeHost.calls.length - 1; i >= 0; --i) {
            if (String(fakeHost.calls[i].method || "") === method) {
                return fakeHost.calls[i]
            }
        }
        return null
    }

    function test_mantle_prefix_is_a_valid_transaction_lookup() {
        const colonQuery = "mantle:" + transactionHash
        const spaceQuery = "mantle " + transactionHash

        compare(statusBar.lookupCode(colonQuery), "transaction")
        compare(statusBar.lookupCode(spaceQuery), "transaction")
        compare(statusBar.lookupLabel("transaction"), "TX")
        verify(statusBar.lookupCanOpen(colonQuery))
        verify(statusBar.lookupCanOpen(spaceQuery))
    }

    function test_mantle_prefix_search_opens_the_cached_transaction() {
        const lookup = findChild(statusBar, "globalReferenceLookup")
        const search = findChild(statusBar, "globalReferenceSearch")

        verify(lookup !== null)
        verify(search !== null)
        lookup.text = "mantle:" + transactionHash
        tryCompare(search, "enabled", true)

        mouseClick(search, search.width / 2, search.height / 2)

        tryCompare(model.shell, "currentView", "transactionDetail")
        compare(model.transactionDetailValue.hash, transactionHash)
        tryCompare(lookup, "text", "")
    }

    function test_transaction_prefix_search_opens_the_cached_transaction_data() {
        return [
            { tag: "tx", query: "tx:" + transactionHash },
            { tag: "transaction", query: "transaction:" + transactionHash }
        ]
    }

    function test_transaction_prefix_search_opens_the_cached_transaction(data) {
        const lookup = findChild(statusBar, "globalReferenceLookup")
        const search = findChild(statusBar, "globalReferenceSearch")

        verify(lookup !== null)
        verify(search !== null)
        lookup.text = data.query
        tryCompare(search, "enabled", true)

        mouseClick(search, search.width / 2, search.height / 2)

        tryCompare(model.shell, "currentView", "transactionDetail")
        compare(model.transactionDetailValue.hash, transactionHash)
        verify(callFor("inspectionResolveTarget") === null)
        tryCompare(lookup, "text", "")
    }

    function test_resolver_prefixes_are_searchable_data() {
        const account = "9JDLE5Qr8dXKBstucN5sZi5tCCYy7SnfCEKax77JZTd7"
        return [
            { tag: "l1 block", query: "l1:27102", code: "block", label: "BLK" },
            { tag: "slot block", query: "slot 27102", code: "block", label: "BLK" },
            { tag: "l2 block", query: "l2:27102", code: "block", label: "BLK" },
            { tag: "lez block", query: "lez 27102", code: "block", label: "BLK" },
            { tag: "typed block", query: "block:27102", code: "block", label: "BLK" },
            { tag: "transaction", query: "tx:" + transactionHash, code: "transaction", label: "TX" },
            { tag: "account", query: "account:" + account, code: "account", label: "ACC" },
            { tag: "program", query: "program:" + transactionHash, code: "program", label: "PRG" },
            { tag: "zone", query: "zone:" + transactionHash, code: "zone", label: "ZONE" },
            { tag: "channel", query: "channel " + transactionHash, code: "zone", label: "ZONE" },
            { tag: "local wallet", query: "wallet:accounts", code: "any", label: "ANY" },
            { tag: "targetless mantle", query: "mantle:", code: "transaction", label: "TX" },
            { tag: "opaque mantle", query: "mantle:remote-transaction", code: "transaction", label: "TX" },
            { tag: "targetless storage", query: "storage:", code: "any", label: "ANY" }
        ]
    }

    function test_resolver_prefixes_are_searchable(data) {
        compare(statusBar.lookupCode(data.query), data.code)
        compare(statusBar.lookupLabel(data.code), data.label)
        verify(statusBar.lookupCanOpen(data.query))
    }

    function test_typed_lookup_rejects_missing_or_malformed_targets() {
        compare(statusBar.lookupCode("l2:"), "invalid")
        compare(statusBar.lookupCode("l2:not-a-block"), "invalid")
        compare(statusBar.lookupCode("tx:not-a-hash"), "invalid")
        compare(statusBar.lookupCode("unknown:value"), "invalid")
        compare(statusBar.lookupCode("recipient:value"), "invalid")
        compare(statusBar.lookupCode("a1".repeat(20)), "invalid")
        compare(statusBar.lookupCode(
            "9JDLE5Qr8dXKBstucN5sZi5tCCYy7SnfCEKax77JZTd7"), "invalid")
        compare(statusBar.lookupCode("18446744073709551616"), "invalid")
        compare(statusBar.lookupCode("18446744073709551615"), "block")
        compare(statusBar.lookupCode("11".repeat(32)), "any")
        verify(!statusBar.lookupCanOpen("l2:"))
    }

    function test_public_account_reference_is_searchable() {
        const account = "Public/9JDLE5Qr8dXKBstucN5sZi5tCCYy7SnfCEKax77JZTd7"
        compare(statusBar.lookupCode(account), "account")
        verify(statusBar.lookupCanOpen(account))
        compare(statusBar.lookupCode(account.replace("Public/", "public/")), "invalid")
        compare(statusBar.lookupCode("account:" + account.replace(
            "Public/", "public/")), "account")
        compare(statusBar.lookupCode("account:" + "00".repeat(32)), "account")
        compare(statusBar.lookupCode("account:Public/" + "00".repeat(32)), "account")
        compare(statusBar.lookupCode(account.replace("Public/", "Private/")), "invalid")
    }

    function test_uppercase_hex_prefix_remains_an_ambiguous_hash() {
        compare(statusBar.lookupCode("0X" + "ab".repeat(32)), "any")
    }

    function test_l2_lookup_dispatches_the_exact_typed_query() {
        const lookup = findChild(statusBar, "globalReferenceLookup")
        const search = findChild(statusBar, "globalReferenceSearch")

        verify(lookup !== null)
        verify(search !== null)
        lookup.text = "l2:27102"
        tryCompare(search, "enabled", true)
        mouseClick(search, search.width / 2, search.height / 2)

        tryVerify(function () { return callFor("inspectionResolveTarget") !== null })
        const call = callFor("inspectionResolveTarget")
        verify(Array.isArray(call.args))
        compare(call.args.length, 1)
        compare(call.args[0].query, "l2:27102")
        tryCompare(lookup, "text", "")
    }
}
