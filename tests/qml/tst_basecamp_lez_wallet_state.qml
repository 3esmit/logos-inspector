import QtQuick
import QtTest
import "../../qml/state/wallet"

TestCase {
    id: testRoot

    name: "BasecampLezWalletState"

    readonly property string sender: "11".repeat(32)
    readonly property string recipient: "22".repeat(32)

    QtObject {
        id: provider

        property var calls: []
        property bool versionFails: false
        property bool transferSucceeds: true
        property var accountRows: []

        function reset() {
            calls = []
            versionFails = false
            transferSucceeds = true
            accountRows = [
                { account_id: testRoot.sender, is_public: true },
                { account_id: testRoot.recipient, is_public: false }
            ]
        }

        function callModuleAsync(moduleName, method, args, callback) {
            calls.push({ moduleName: moduleName, method: method, args: args })
            Qt.callLater(function() {
                if (method === "version") {
                    callback(versionFails
                        ? { ok: false, value: null, error: "LEZ Core is not installed" }
                        : { ok: true, value: "0.3.0", error: "" })
                    return
                }
                if (method === "list_accounts") {
                    callback({ ok: true, value: accountRows, error: "" })
                    return
                }
                if (method === "get_balance") {
                    callback({
                        ok: true,
                        value: args[0] === testRoot.sender ? "42" : "7",
                        error: ""
                    })
                    return
                }
                if (method === "transfer_public") {
                    callback({
                        ok: true,
                        value: transferSucceeds
                            ? "{\"success\":true,\"tx_hash\":\"tx-test\",\"error\":\"\"}"
                            : "{\"success\":false,\"tx_hash\":\"\",\"error\":\"insufficient funds\"}",
                        error: ""
                    })
                    return
                }
                callback({ ok: false, value: null, error: "Unexpected method: " + method })
            })
            return calls.length
        }
    }

    BasecampLezWalletState {
        id: wallet

        bridge: provider
    }

    function init() {
        provider.reset()
        wallet.requestEpoch += 1
        wallet.busy = false
        wallet.availability = "unknown"
        wallet.availabilityDetail = ""
        wallet.version = ""
        wallet.accounts = []
        wallet.error = ""
        wallet.notice = ""
        wallet.transferResult = null
        wallet.operations = []
    }

    function callsFor(method) {
        return provider.calls.filter(function(call) {
            return call.method === method
        })
    }

    function test_refresh_uses_direct_lez_core_calls_and_loads_balances() {
        verify(wallet.refresh())

        tryVerify(function() {
            return !wallet.busy && wallet.accounts.length === 2
                && wallet.accounts[0].balance === "42"
                && wallet.accounts[1].balance === "7"
        })
        compare(wallet.providerModule, "lez_core")
        compare(wallet.availability, "available")
        compare(wallet.version, "0.3.0")
        compare(callsFor("version").length, 1)
        compare(callsFor("list_accounts").length, 1)
        compare(callsFor("get_balance").length, 2)
        verify(provider.calls.every(function(call) {
            return call.moduleName === "lez_core"
        }))
    }

    function test_unavailable_core_is_reported_without_a_wallet_fallback() {
        provider.versionFails = true

        verify(wallet.refresh())
        tryCompare(wallet, "busy", false)
        compare(wallet.availability, "unavailable")
        compare(wallet.error, "LEZ Core is not installed")
        compare(callsFor("list_accounts").length, 0)
    }

    function test_transfer_encodes_atomic_units_and_refreshes_after_success() {
        verify(wallet.submitPublicTransfer(sender, recipient, "258"))

        tryVerify(function() {
            return !wallet.busy && wallet.transferResult !== null
                && wallet.transferResult.success === true
                && wallet.notice.indexOf("tx-test") >= 0
        })
        const calls = callsFor("transfer_public")
        compare(calls.length, 1)
        compare(calls[0].args[0], sender)
        compare(calls[0].args[1], recipient)
        compare(calls[0].args[2], "02010000000000000000000000000000")
        verify(callsFor("version").length >= 1)
    }

    function test_core_rejection_is_not_reported_as_a_submitted_transfer() {
        provider.transferSucceeds = false

        verify(wallet.submitPublicTransfer(sender, recipient, "1"))
        tryCompare(wallet, "busy", false)
        compare(wallet.transferResult.success, false)
        compare(wallet.error, "insufficient funds")
        verify(wallet.operations.some(function(operation) {
            return operation.label === qsTr("Public transfer") && operation.status === qsTr("failed")
        }))
    }

    function test_invalid_transfer_never_reaches_the_module() {
        verify(!wallet.submitPublicTransfer("not-an-account", recipient, "0"))
        compare(wallet.error, qsTr("Source account must be a 32-byte hexadecimal account ID."))
        compare(callsFor("transfer_public").length, 0)
        compare(wallet.decimalToLe16Hex("340282366920938463463374607431768211456"), "")
    }
}
