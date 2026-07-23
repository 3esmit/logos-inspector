import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/wallet/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"
import "fixtures"

TestCase {
    id: testRoot

    name: "BasecampLezWalletPage"
    when: windowShown
    width: 900
    height: 700

    readonly property string sender: "11".repeat(32)
    readonly property string recipient: "22".repeat(32)

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

        width: testRoot.width
        height: testRoot.height
        visible: true
        color: theme.background

        BasecampLezWalletPage {
            id: page

            theme: theme
            model: model
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        fakeHost.responses = {
            version: { ok: true, value: "0.3.0", text: "OK", error: "" },
            list_accounts: {
                ok: true,
                value: [{ account_id: testRoot.sender, is_public: true }],
                text: "OK",
                error: ""
            },
            get_balance: { ok: true, value: "42", text: "OK", error: "" },
            transfer_public: {
                ok: true,
                value: "{\"success\":true,\"tx_hash\":\"tx-page\",\"error\":\"\"}",
                text: "OK",
                error: ""
            }
        }
        model.basecampWallet.requestEpoch += 1
        model.basecampWallet.busy = false
        model.basecampWallet.availability = "unknown"
        model.basecampWallet.availabilityDetail = ""
        model.basecampWallet.version = ""
        model.basecampWallet.accounts = []
        model.basecampWallet.error = ""
        model.basecampWallet.notice = ""
        model.basecampWallet.transferResult = null
        model.basecampWallet.operations = []
        model.basecampWalletTab = "provider"
        page.transferFrom = ""
        page.transferTo = ""
        page.transferAmount = ""
        model.basecampWallet.refresh()
    }

    function waitForChild(parent, objectName) {
        let child = null
        tryVerify(function() {
            child = findChild(parent, objectName)
            return child !== null
        })
        verify(!!child, "Object exists")
        return child
    }

    function callsFor(method) {
        return fakeHost.calls.filter(function(call) {
            return call.method === method
        })
    }

    function test_page_loads_official_core_accounts_not_local_wallet_controls() {
        tryVerify(function() {
            return !model.basecampWallet.busy && model.basecampWallet.accounts.length === 1
        })
        compare(model.basecampWallet.providerModule, "lez_core")
        compare(model.basecampWallet.accounts[0].balance, "42")
        verify(findChild(page, "createAccountButton") === null)
        verify(callsFor("version").every(function(call) {
            return call.module === "lez_core"
        }))
    }

    function test_transfer_requires_confirmation_and_dispatches_to_lez_core() {
        page.transferFrom = sender
        page.transferTo = recipient
        page.transferAmount = "258"
        model.basecampWalletTab = "transfer"

        const button = waitForChild(page, "basecampLezWalletTransferButton")
        const popup = waitForChild(page, "basecampLezWalletTransferConfirm")
        verify(button.enabled)
        mouseClick(button, button.width / 2, button.height / 2)
        tryCompare(popup, "opened", true)

        const confirmButton = waitForChild(popup.contentItem, "confirmButton")
        mouseClick(confirmButton, confirmButton.width / 2, confirmButton.height / 2)
        tryCompare(popup, "opened", false)
        tryVerify(function() {
            return !model.basecampWallet.busy
                && model.basecampWallet.transferResult !== null
                && model.basecampWallet.transferResult.success === true
        })
        const calls = callsFor("transfer_public")
        compare(calls.length, 1)
        compare(calls[0].module, "lez_core")
        compare(calls[0].args[2], "02010000000000000000000000000000")
    }
}
