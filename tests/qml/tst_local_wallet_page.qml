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

    name: "LocalWalletPage"
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
        localWalletTab: "controls"
        walletBinary: "/usr/bin/wallet"
        walletHome: "/tmp/wallet-home"
    }

    ApplicationWindow {
        id: testWindow

        width: testRoot.width
        height: testRoot.height
        visible: true
        color: theme.background

        LocalWalletPage {
            id: page

            theme: theme
            model: model
            width: testWindow.width
        }
    }

    function init() {
        fakeHost.reset()
        model.shell.busy = false
        model.localWalletTab = "controls"
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/wallet"
        model.walletHome = "/tmp/wallet-home"
        model.localWalletStatus = readyWalletStatus()
        model.walletCreatePrivacy = "public"
        model.walletCreateLabel = ""
        model.walletSendFrom = ""
        model.walletSendTo = ""
        model.walletSendToKeys = ""
        model.walletSendToNpk = ""
        model.walletSendToVpk = ""
        model.walletSendToIdentifier = ""
        model.walletSendAmount = ""
        model.walletAdvancedCommand = ""
        model.walletPublicKeyProbe = ""
        model.bedrockWalletBalanceTip = ""
        model.bedrockWalletBalanceValue = null
        model.bedrockWalletBalanceError = ""
        model.localWalletOperations = []
        model.runtimeOperationHistory = []
        closeAllPopups()
    }

    function compareArgs(actual, expected) {
        verify(actual !== null)
        compare(actual.length, expected.length)
        for (let i = 0; i < expected.length; ++i) {
            compare(actual[i], expected[i])
        }
    }

    function closeAllPopups() {
        const names = ["privateSyncConfirm", "createAccountConfirm", "sendTransactionConfirm", "readIncomingConfirm", "advancedWalletConfirm"]
        for (let i = 0; i < names.length; ++i) {
            const popup = findChild(page, names[i])
            if (popup) {
                popup.close()
            }
        }
    }

    function waitForChild(parent, objectName) {
        let child = null
        tryVerify(function () {
            child = findChild(parent, objectName)
            return child !== null
        })
        verify(!!child, "Object exists")
        return child
    }

    function configureWalletProfile() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/wallet"
        model.walletHome = "/tmp/wallet-home"
        model.localWalletStatus = readyWalletStatus()
    }

    function readyWalletStatus() {
        return {
            status: "ok",
            home_source: "profile",
            readiness: {
                wallet_binary_ready: true,
                wallet_home_ready: true,
                wallet_config_ready: true,
                wallet_storage_ready: true,
                command_ready: true,
                accounts_ready: true,
                instruction_submit_ready: true,
                backup_encryption_ready: true
            }
        }
    }

    function configureWalletOperation(operation) {
        configureWalletProfile()
        switch (operation) {
        case "send":
            model.walletSendFrom = "Public/source"
            model.walletSendTo = "Private/recipient"
            model.walletSendAmount = "37"
            break
        case "advanced":
            model.walletAdvancedCommand = "account get --account-id Public/abc"
            break
        default:
            break
        }
    }

    function openWalletConfirmation(operation, tab, buttonName, popupName) {
        configureWalletOperation(operation)
        model.localWalletTab = tab

        const button = waitForChild(page, buttonName)
        const popup = waitForChild(page, popupName)
        compare(popup.opened, false)
        verify(button.enabled)

        mouseClick(button, button.width / 2, button.height / 2)
        tryCompare(popup, "opened", true)
        return popup
    }

    function test_wallet_actions_open_expected_confirmation_popups_data() {
        return [
            { tag: "create account", operation: "create", tab: "controls", buttonName: "createAccountButton", popupName: "createAccountConfirm", title: qsTr("Create account") },
            { tag: "send transaction", operation: "send", tab: "controls", buttonName: "sendTransactionButton", popupName: "sendTransactionConfirm", title: qsTr("Send transaction") },
            { tag: "read incoming", operation: "read", tab: "controls", buttonName: "readIncomingButton", popupName: "readIncomingConfirm", title: qsTr("Read incoming") },
            { tag: "advanced command", operation: "advanced", tab: "controls", buttonName: "advancedWalletButton", popupName: "advancedWalletConfirm", title: qsTr("Run wallet command") },
            { tag: "private sync", operation: "sync", tab: "privateSync", buttonName: "privateSyncButton", popupName: "privateSyncConfirm", title: qsTr("Sync private wallet") }
        ]
    }

    function test_wallet_actions_open_expected_confirmation_popups(data) {
        const popup = openWalletConfirmation(data.operation, data.tab, data.buttonName, data.popupName)
        tryCompare(popup, "title", data.title)
    }

    function test_accepting_create_account_popup_calls_model_command() {
        model.walletCreatePrivacy = "private"
        model.walletCreateLabel = "receiver"
        fakeHost.responses = {
            localWalletCreateAccount: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "created",
                    command: "wallet account new private",
                    account_id: "Private/abc123"
                },
                text: "OK",
                error: ""
            }
        }

        const popup = openWalletConfirmation("create", "controls", "createAccountButton", "createAccountConfirm")
        const confirmButton = waitForChild(popup.contentItem, "confirmButton")
        verify(confirmButton.enabled)

        mouseClick(confirmButton, confirmButton.width / 2, confirmButton.height / 2)
        tryCompare(popup, "opened", false)
        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletCreateAccount"
            })
        })

        const calls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletCreateAccount"
        })
        compare(calls.length, 1)
        compare(calls[0].args[1], "private")
        compare(calls[0].args[2], "receiver")
        compare(calls[0].args[3], "confirm-create-account")
        tryCompare(model, "walletCreateLabel", "")
        tryVerify(function () {
            return model.localWalletOperations.length === 1
                && model.localWalletOperations[0].label === qsTr("Create account")
                && model.localWalletOperations[0].status === "created"
        })
    }

    function test_bedrock_balance_json_exposes_exact_accessible_text() {
        const publicKey = "26".repeat(32)
        model.walletPublicKeyProbe = publicKey
        model.bedrockWalletBalanceValue = {
            address: publicKey,
            balance: 1000,
            notes: { "note-id": 1000 },
            tip: "ab".repeat(32)
        }
        model.localWalletTab = "bedrockNotes"

        const balanceText = waitForChild(page, "bedrockBalanceJson")
        tryCompare(balanceText, "visible", true)
        const expected = page.balanceJson()
        compare(balanceText.text, expected)
        compare(balanceText.Accessible.role, Accessible.StaticText)
        compare(
            balanceText.Accessible.name,
            qsTr("Bedrock REST balance response: %1").arg(expected)
        )
    }

    function test_profile_incompatibility_exposes_complete_actionable_detail() {
        const detail = "wallet home configured; wallet binary responded; wallet binary cannot read configured wallet home: incompatible wallet storage schema"
        model.localWalletStatus = {
            status: "down",
            detail: detail,
            readiness: {
                command_ready: false,
                accounts_ready: false
            }
        }
        model.localWalletTab = "profiles"

        const message = waitForChild(page, "walletProfileReadinessMessage")
        tryCompare(message, "visible", true)
        compare(message.tone, "error")
        compare(message.title, qsTr("Profile not ready"))
        compare(message.message, detail)
        compare(
            message.Accessible.name,
            qsTr("%1. %2").arg(qsTr("Profile not ready")).arg(detail)
        )
    }

    function test_account_list_actions_require_account_readiness() {
        model.localWalletStatus = {
            status: "degraded",
            detail: "wallet accounts are unavailable",
            readiness: {
                command_ready: true,
                accounts_ready: false
            }
        }
        model.localWalletTab = "controls"

        const controlsButton = waitForChild(page, "controlsListAccountsButton")
        tryCompare(controlsButton, "visible", true)
        compare(controlsButton.enabled, false)

        model.localWalletTab = "lezAccounts"
        const accountsButton = waitForChild(page, "accountsListAccountsButton")
        tryCompare(accountsButton, "visible", true)
        compare(accountsButton.enabled, false)
    }

    function test_parse_wallet_command_line_preserves_backslash_paths() {
        compareArgs(
            page.parseWalletCommandLine("account import \"C:\\wallet\\recipient.keys\""),
            ["account", "import", "C:\\wallet\\recipient.keys"]
        )
        compareArgs(
            page.parseWalletCommandLine("account import C:\\wallet\\recipient.keys"),
            ["account", "import", "C:\\wallet\\recipient.keys"]
        )
    }

    function test_parse_wallet_command_line_keeps_targeted_escapes() {
        compareArgs(
            page.parseWalletCommandLine("account get --label Token\\ Label"),
            ["account", "get", "--label", "Token Label"]
        )
        compareArgs(
            page.parseWalletCommandLine("account get --label \"Token \\\"A\\\"\""),
            ["account", "get", "--label", "Token \"A\""]
        )
    }

    function test_short_text_keeps_middle_truncation() {
        compare(page.shortText("", 12), "-")
        compare(page.shortText("abcdefghijklmnopqrstuvwxyz", 12), "abcd...uvwxyz")
    }
}
