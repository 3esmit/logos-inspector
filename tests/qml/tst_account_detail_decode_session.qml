import QtQuick
import QtTest
import "../../qml/state/accounts" as Accounts

TestCase {
    id: testRoot

    name: "AccountDetailDecodeSession"

    ListModel {
        id: registeredIdls
    }

    QtObject {
        id: model

        property var registeredIdls: registeredIdls
        property var favoriteStore: favoriteStoreFixture
        property var idlInstructionPreviewValue: null
        property string idlInstructionError: ""
        property int sharedIdlRevision: 0
        property var pendingAutoDecodeCallback: null
        property var pendingManualDecodeCallback: null
        property var pendingRelatedDecodeCallback: null
        property int cacheAccountIdlSelectionCalls: 0
        property int maybeAutoShareAccountIdlCalls: 0

        function idlKey(name, programId, json) { return String(name || "") + ":" + String(programId || "") }
        function idlEntryForKey(key) { return registeredIdls.count > 0 ? registeredIdls.get(0) : null }
        function cachedIdlEntryForAccount(account, owner) { return registeredIdls.count > 0 ? registeredIdls.get(0) : null }
        function accountDecodeFullyConsumed(decode) { return decode && !String(decode.remaining_data_hex || "").length }
        function refreshSharedIdlsForAccount(account, dataHex, owner) {}
        function autoDecodeAccountData(dataHex, accountId, ownerProgramId, callback) { pendingAutoDecodeCallback = callback }
        function decodeAccountDataAsync(dataHex, idlJson, accountType, callback) { pendingManualDecodeCallback = callback }
        function cacheAccountIdlSelection(accountId, entry, accountType, ownerProgramId) { cacheAccountIdlSelectionCalls += 1 }
        function maybeAutoShareAccountIdl(accountId, ownerProgramId, entry) { maybeAutoShareAccountIdlCalls += 1 }
        function sharedIdlSuggestions(account) { return [] }
        function socialCommentTopic(layer, kind, account) { return layer + ":" + kind + ":" + account }
        function socialLezAccountIdlTopic(account) { return "idl:" + account }
        function socialSharedIdlWriteAvailable(topic) { return true }
        function transactionDecodeCandidates(summary) { return [] }
        function programDecodeCandidatePayload(candidates) { return candidates }
        function resolveTransactionDecodeSessionAsync(summary, payload, callback) { pendingRelatedDecodeCallback = callback }
        function transactionDecodeSessionInstruction(response) { return response && response.instruction ? response : null }
    }

    QtObject {
        id: favoriteStoreFixture

        function accountEntry(detail) {
            return { kind: "account", value: String(detail && detail.account_id_base58 ? detail.account_id_base58 : "") }
        }

        function isFavoriteEntry(entry) {
            return false
        }
    }

    Accounts.AccountDetailDecodeSession {
        id: session

        model: model
    }

    Accounts.AccountDetailInspectionWorkspace {
        id: workspace

        model: model
    }

    function init() {
        registeredIdls.clear()
        registeredIdls.append({
            key: "demo:program",
            name: "Demo",
            programId: "program",
            programIdHex: "0xabc",
            programBinary: "demo.bin",
            json: JSON.stringify({
                accounts: [{ name: "Position" }],
                instructions: [{
                    name: "settle",
                    accounts: [{ name: "owner", signer: true }],
                    args: [{ name: "amount", type: "u64" }]
                }]
            }),
            source: "local"
        })
        session.detail = {
            account_id: "Public/account",
            account_id_base58: "Public/account",
            account_id_hex: "",
            owner_base58: "",
            owner_hex: "0xabc",
            data_hex: "0x0102",
            related_transactions: [],
            related_transactions_error: "",
            decode: { account_type: "Position", rows: [], remaining_data_hex: "" },
            decode_error: "",
            private_reference: false
        }
        session.resetDecodeState()
        session.resetInteractionState()
        resetModelCallbacks()
        workspace.value = null
    }

    function resetModelCallbacks() {
        model.pendingAutoDecodeCallback = null
        model.pendingManualDecodeCallback = null
        model.pendingRelatedDecodeCallback = null
        model.cacheAccountIdlSelectionCalls = 0
        model.maybeAutoShareAccountIdlCalls = 0
    }

    function test_rebuilds_idl_options_and_active_label() {
        compare(session.idlTypeOptions.length, 1)
        compare(session.idlTypeLabels[0], "Demo: Position")
        compare(session.selectedIdlTypeIndex, 0)
        compare(session.activeIdlTypeLabel(), "Demo: Position")
    }

    function test_interaction_request_is_session_owned() {
        verify(session.canInteractWithIdl())
        compare(session.interactionInstructionLabels()[0], "settle")

        session.setInteractionFieldValue("account", "owner", "Public/owner")
        session.setInteractionFieldValue("arg", "amount", "42")
        const request = session.interactionRequest()

        compare(request.instruction, "settle")
        compare(request.program_id_hex, "0xabc")
        compare(request.program_binary, "demo.bin")
        compare(request.accounts.owner, "Public/owner")
        compare(request.args.amount, "42")
    }

    function test_stale_auto_decode_callback_is_ignored_after_decode_serial_changes() {
        wait(0)
        resetModelCallbacks()
        session.activeDecode = null
        session.activeDecodeError = "kept"
        session.activeIdlLabel = "kept"
        session.selectedIdlTypeIndex = -1

        session.autoSelectDecode()
        const staleCallback = model.pendingAutoDecodeCallback
        verify(staleCallback !== null)

        session.decodeRequestSerial += 1
        staleCallback({
            ok: true,
            entry: {
                key: "demo:program",
                name: "Stale IDL"
            },
            value: {
                account_type: "Position",
                rows: [],
                remaining_data_hex: ""
            }
        })

        compare(session.activeDecode, null)
        compare(session.activeDecodeError, "kept")
        compare(session.activeIdlLabel, "kept")
        compare(session.selectedIdlTypeIndex, -1)
        compare(model.cacheAccountIdlSelectionCalls, 0)
        compare(model.maybeAutoShareAccountIdlCalls, 0)
    }

    function test_stale_selected_decode_callback_is_ignored_after_decode_serial_changes() {
        wait(0)
        resetModelCallbacks()
        session.activeDecode = {
            account_type: "Current",
            rows: [],
            remaining_data_hex: ""
        }
        session.activeDecodeError = "kept"

        session.selectIdlType(0)
        const staleCallback = model.pendingManualDecodeCallback
        verify(staleCallback !== null)

        session.decodeRequestSerial += 1
        staleCallback({
            ok: true,
            value: {
                account_type: "Stale",
                rows: [],
                remaining_data_hex: ""
            }
        })

        compare(session.activeDecode.account_type, "Current")
        compare(session.activeDecodeError, "kept")
        compare(model.cacheAccountIdlSelectionCalls, 0)
        compare(model.maybeAutoShareAccountIdlCalls, 0)
    }

    function test_related_transaction_decode_cache_projects_rows() {
        session.detail = {
            account_id: "Public/account",
            account_id_base58: "Public/account",
            account_id_hex: "",
            owner_base58: "",
            owner_hex: "0xabc",
            data_hex: "0x0102",
            related_transactions: [{
                hash: "0x1111111111111111111111111111111111111111111111111111111111111111",
                kind: "Public",
                direction: "incoming",
                program_id_hex: "0xabc",
                account_ids: ["a"],
                instruction_data: [1]
            }],
            related_transactions_error: "",
            decode: { account_type: "Position", rows: [], remaining_data_hex: "" },
            decode_error: "",
            private_reference: false
        }
        session.storeRelatedTransactionDecode("0x1111111111111111111111111111111111111111111111111111111111111111", {
            instruction: "settle",
            idl_name: "Demo"
        })

        const rows = session.relatedRows()
        compare(rows.length, 1)
        compare(rows[0].instruction, "settle")
        compare(rows[0].programText, "Demo")
    }

    function test_stale_related_transaction_decode_callback_is_ignored_after_related_serial_changes() {
        wait(0)
        resetModelCallbacks()
        const txHash = "0x1111111111111111111111111111111111111111111111111111111111111111"
        const summary = session.relatedTransactionSummary({
            hash: txHash,
            kind: "Public",
            direction: "incoming",
            program_id_hex: "0xabc",
            account_ids: ["a"],
            instruction_data: [1]
        })
        verify(summary !== null)
        const serial = session.relatedTransactionDecodeSerial + 1
        session.relatedTransactionDecodeSerial = serial
        session.relatedTransactionDecodeMap = ({})

        session.tryRelatedTransactionDecodeCandidate(serial, txHash, summary, [{ key: "demo:program" }], 0, null)
        const staleCallback = model.pendingRelatedDecodeCallback
        verify(staleCallback !== null)

        session.relatedTransactionDecodeSerial = serial + 1
        staleCallback({
            instruction: "settle",
            idl_name: "Demo"
        })

        compare(session.relatedTransactionDecode(txHash), null)
        compare(Object.keys(session.relatedTransactionDecodeMap).length, 0)
    }

    function test_workspace_projects_detail_and_delegates_decode_session() {
        workspace.value = {
            account: {
                account_id: "Public/account",
                account_id_base58: "Public/account",
                account_id_hex: "",
                balance: "1200",
                nonce: "7",
                owner_hex: "0xabc",
                data_hex: "0x0102",
                related_transactions: []
            },
            decode: { account_type: "Position", rows: [], remaining_data_hex: "" },
            decode_error: ""
        }
        workspace.resetDecodeState()
        workspace.resetInteractionState()

        compare(workspace.accountHeader(workspace.detail), "Public/account")
        compare(workspace.accountRows()[0].label, "Balance")
        compare(workspace.accountRows()[0].value, "1200")
        verify(workspace.canInteractWithIdl())

        workspace.setInteractionFieldValue("account", "owner", "Public/owner")
        workspace.setInteractionFieldValue("arg", "amount", "42")
        const request = workspace.interactionRequest()

        compare(request.instruction, "settle")
        compare(request.program_binary, "demo.bin")
        compare(workspace.favoriteButtonText(), "Favorite")
    }
}
