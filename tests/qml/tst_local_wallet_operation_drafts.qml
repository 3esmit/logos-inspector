import QtQuick
import QtTest
import "../../qml/state/wallet/LocalWalletOperationDrafts.js" as Drafts

TestCase {
    id: testRoot

    name: "LocalWalletOperationDrafts"

    QtObject {
        id: gateway

        property bool busyValue: false

        function busy() { return busyValue }
        function nodeUrl() { return "http://node" }
    }

    QtObject {
        id: walletRoot

        property var gateway: gateway
        property bool profileOk: true
        property string createPrivacy: "private"
        property string createLabel: "Alpha"
        property string sendFrom: "from-a"
        property string sendTo: "to-b"
        property string sendToKeys: ""
        property string sendToNpk: ""
        property string sendToVpk: ""
        property string sendToIdentifier: ""
        property string sendAmount: "42"
        property string publicKeyProbe: "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        property string bedrockBalanceTip: ""

        function profileConfigured() { return profileOk }
        function currentProfile() { return { wallet_home: "/tmp/wallet" } }
        function isBedrockHexId(value) { return /^(0x)?[0-9a-fA-F]{64}$/.test(String(value || "").trim()) }
    }

    function init() {
        gateway.busyValue = false
        walletRoot.profileOk = true
        walletRoot.createPrivacy = "private"
        walletRoot.createLabel = "Alpha"
        walletRoot.sendFrom = "from-a"
        walletRoot.sendTo = "to-b"
        walletRoot.sendToKeys = ""
        walletRoot.sendToNpk = ""
        walletRoot.sendToVpk = ""
        walletRoot.sendAmount = "42"
        walletRoot.publicKeyProbe = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        walletRoot.bedrockBalanceTip = ""
    }

    function test_create_account_draft_contains_confirmation_and_history_metadata() {
        const draft = Drafts.createAccount(walletRoot)

        verify(draft.ok)
        compare(draft.method, "localWalletCreateAccount")
        compare(draft.args[1], "private")
        compare(draft.args[2], "Alpha")
        compare(draft.args[3], "confirm-create-account")
        compare(draft.historyLabel, "Create account")
        compare(draft.successStatus, "created")
    }

    function test_send_transaction_draft_validates_required_fields() {
        walletRoot.sendAmount = ""

        const draft = Drafts.sendTransaction(walletRoot)

        verify(!draft.ok)
        compare(draft.message, "Sender and amount are required.")
    }

    function test_bedrock_balance_draft_validates_tip_and_builds_query_args() {
        walletRoot.bedrockBalanceTip = "bad-tip"
        const invalid = Drafts.queryBedrockBalance(walletRoot)
        verify(!invalid.ok)
        compare(invalid.balanceError, "Balance tip must be a 64-hex header id.")

        walletRoot.bedrockBalanceTip = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"
        const draft = Drafts.queryBedrockBalance(walletRoot)
        verify(draft.ok)
        compare(draft.method, "bedrockWalletBalance")
        compare(draft.args[0], "http://node")
        compare(draft.args[2], walletRoot.bedrockBalanceTip)
    }
}
