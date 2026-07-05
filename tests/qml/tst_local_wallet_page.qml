import QtQuick
import QtTest
import "../../qml/pages"
import "../../qml/services"
import "../../qml/state"
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "LocalWalletPage"
    when: windowShown
    width: 900
    height: 700

    QtObject {
        id: fakeHost

        function callModuleJson(moduleName, method, argsJson) {
            return JSON.stringify({
                ok: true,
                value: {},
                text: "OK",
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
        localWalletTab: "controls"
        walletBinary: "/usr/bin/wallet"
        walletHome: "/tmp/wallet-home"
    }

    LocalWalletPage {
        id: page

        theme: theme
        model: model
        width: testRoot.width
    }

    function compareArgs(actual, expected) {
        verify(actual !== null)
        compare(actual.length, expected.length)
        for (let i = 0; i < expected.length; ++i) {
            compare(actual[i], expected[i])
        }
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
}
