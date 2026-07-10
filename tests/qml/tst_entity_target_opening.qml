import QtQuick
import QtTest
import "../../qml/state/chain/EntityTargetOpening.js" as EntityTargetOpening
import "../../qml/state/chain/LezTargetPresentation.js" as LezTargetPresentation

TestCase {
    id: testRoot

    name: "EntityTargetOpening"

    QtObject {
        id: model

        function valueToString(value) { return value === undefined || value === null ? "" : String(value) }
    }

    QtObject {
        id: session

        property var model: model
    }

    QtObject {
        id: pageState

        property string selectedView: ""
        property bool selectedRecordHistory: true
        property var blockDetailValue: null
        property var transactionDetailValue: null
        property string lezTransactionsPageError: ""
        property string accountTab: ""
        property var accountDetailValue: null
        property string resultTitle: ""
        property string resultText: ""
        property bool resultIsError: false
        property var resultValue: null
        property int autoDecodeCallCount: 0
        property var autoDecodedPayload: null

        function reset() {
            selectedView = ""
            selectedRecordHistory = true
            blockDetailValue = null
            transactionDetailValue = null
            lezTransactionsPageError = "stale"
            accountTab = ""
            accountDetailValue = null
            resultTitle = ""
            resultText = ""
            resultIsError = false
            resultValue = null
            autoDecodeCallCount = 0
            autoDecodedPayload = null
        }

        function selectView(view, recordHistory) {
            selectedView = String(view || "")
            selectedRecordHistory = recordHistory === true
        }

        function indexerBlockDetail(payload) {
            return { block: payload }
        }

        function setResult(title, text, isError, value) {
            resultTitle = String(title || "")
            resultText = String(text || "")
            resultIsError = isError === true
            resultValue = value
        }

        function autoDecodeTransactionDetail(payload) {
            autoDecodeCallCount += 1
            autoDecodedPayload = payload
        }
    }

    function init() {
        pageState.reset()
    }

    function test_reference_target_classifies_aliases() {
        compare(EntityTargetOpening.referenceTarget(session, "tx", "abc").command, "transaction")
        compare(EntityTargetOpening.referenceTarget(session, "signer", "acct").command, "account")
        compare(EntityTargetOpening.referenceTarget(session, "note", "wallet").command, "localWallet")
        compare(EntityTargetOpening.referenceTarget(session, "note", "wallet").tab, "bedrockNotes")
        compare(EntityTargetOpening.referenceTarget(session, "program", "prog").command, "program")
    }

    function test_reference_target_preserves_payload_backed_block_and_channel() {
        const blockPayload = { hash: "h" }
        const block = EntityTargetOpening.referenceTarget(session, "block", "h", blockPayload)
        const channel = EntityTargetOpening.referenceTarget(session, "channel", "c", { channel_id: "c" })

        compare(block.command, "blockchainBlock")
        compare(block.payload.hash, "h")
        compare(channel.command, "channel")
        compare(channel.payload.channel_id, "c")
    }

    function test_blank_without_payload_is_noop() {
        compare(EntityTargetOpening.referenceTarget(session, "account", "").command, "")
        compare(EntityTargetOpening.referenceTarget(session, "channel", "", { channel_id: "c" }).command, "channel")
    }

    function test_lez_target_presentation_classifies_backend_result() {
        const block = LezTargetPresentation.targetCommand({
            ok: true,
            value: { kind: "block", payload: { block_id: 7 } }
        }, "Lookup")
        const tx = LezTargetPresentation.targetCommand({
            ok: true,
            value: { kind: "transaction", payload: { hash: "abc" } }
        }, "Lookup")
        const missing = LezTargetPresentation.targetCommand({
            ok: true,
            value: { kind: "unknown", payload: null }
        }, "Lookup")

        compare(block.kind, "block")
        compare(block.view, "l2BlockDetail")
        compare(tx.kind, "transaction")
        compare(tx.autoDecode, true)
        compare(missing.kind, "not_found")
        compare(missing.error, true)
    }

    function test_lez_target_presentation_classifies_account_result() {
        const payload = { account_id: "acct", balance: "42" }
        const account = LezTargetPresentation.targetCommand({
            ok: true,
            value: { kind: "account", payload: payload }
        }, "Lookup")

        compare(account.handled, true)
        compare(account.kind, "account")
        compare(account.view, "accounts")
        compare(account.accountTab, "lookup")
        compare(account.title, qsTr("Account lookup"))
        compare(account.payload.account_id, "acct")
    }

    function test_lez_target_presentation_applies_account_command() {
        const payload = { account_id: "acct", balance: "42" }

        const applied = LezTargetPresentation.applyCommand(pageState, {
            kind: "account",
            view: "accounts",
            accountTab: "lookup",
            title: qsTr("Account lookup"),
            payload: payload
        })

        compare(applied, true)
        compare(pageState.selectedView, "accounts")
        compare(pageState.selectedRecordHistory, false)
        compare(pageState.accountTab, "lookup")
        compare(pageState.accountDetailValue.account_id, "acct")
        compare(pageState.resultTitle, qsTr("Account lookup"))
        compare(pageState.resultValue.account_id, "acct")
        compare(pageState.resultIsError, false)
    }

    function test_lez_target_presentation_auto_decodes_transaction_command() {
        const payload = { hash: "abc", summary: "decoded" }

        const applied = LezTargetPresentation.applyCommand(pageState, {
            kind: "transaction",
            view: "l2TransactionDetail",
            title: qsTr("LEZ transaction"),
            payload: payload,
            autoDecode: true
        })

        compare(applied, true)
        compare(pageState.selectedView, "l2TransactionDetail")
        compare(pageState.transactionDetailValue.hash, "abc")
        compare(pageState.lezTransactionsPageError, "")
        compare(pageState.resultValue.hash, "abc")
        compare(pageState.autoDecodeCallCount, 1)
        compare(pageState.autoDecodedPayload.hash, "abc")
    }

    function test_lez_target_presentation_ignores_failed_response() {
        compare(LezTargetPresentation.targetCommand({ ok: false, error: "missing" }).handled, false)
    }
}
