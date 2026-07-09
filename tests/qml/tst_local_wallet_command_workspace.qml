import QtQuick
import QtTest
import "../../qml/state/wallet/LocalWalletCommandWorkspace.js" as WalletCommand

TestCase {
    name: "LocalWalletCommandWorkspace"

    QtObject {
        id: model

        property bool busy: false
        property string walletSendFrom: "from"
        property string walletSendAmount: "5"
        property string walletSendTo: "to"
        property string walletSendToKeys: ""
        property string walletSendToNpk: ""
        property string walletSendToVpk: ""
        property string walletAdvancedCommand: ""
        property var localWalletOperations: []

        function walletProfileConfigured() { return !busy }
    }

    function init() {
        model.busy = false
        model.walletSendFrom = "from"
        model.walletSendAmount = "5"
        model.walletSendTo = "to"
        model.walletSendToKeys = ""
        model.walletSendToNpk = ""
        model.walletSendToVpk = ""
        model.walletAdvancedCommand = ""
        model.localWalletOperations = []
    }

    function test_send_ready_accepts_address_or_key_pair() {
        verify(WalletCommand.sendReady(model))

        model.walletSendTo = ""
        model.walletSendToNpk = "npk"
        model.walletSendToVpk = "vpk"
        verify(WalletCommand.sendReady(model))

        model.walletSendToVpk = ""
        verify(!WalletCommand.sendReady(model))
    }

    function test_command_parser_handles_quotes_and_wallet_prefix() {
        compare(WalletCommand.parseWalletCommandLine("wallet send 'two words' \"quoted\"").join("|"), "send|two words|quoted")
        compare(WalletCommand.parseWalletCommandLine("wallet send 'unterminated"), null)
    }

    function test_command_error_and_operation_rows() {
        compare(WalletCommand.advancedCommandError(model), "Wallet command arguments are required.")

        model.walletAdvancedCommand = "wallet status"
        compare(WalletCommand.advancedCommandError(model), "")
        compare(WalletCommand.walletCommandArgs(model).join("|"), "status")

        compare(WalletCommand.operationRows(model)[0].label, "No operations")
        model.localWalletOperations = [{ label: "first" }, { label: "second" }]
        compare(WalletCommand.operationRows(model)[0].label, "second")
    }
}
