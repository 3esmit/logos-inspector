pragma ComponentBehavior: Bound

import QtQuick
import QtTest
import "../../qml/state/programs" as Programs

TestCase {
    id: testRoot

    name: "ProgramExecutionState"

    property var execution: null
    property var callbackResponse: null
    property var callbackTarget: null
    property alias testGateway: gateway
    property alias testWalletCapability: walletCapability

    QtObject {
        id: gateway

        property bool busyValue: false
        property int busyAcquireCount: 0
        property int busyReleaseCount: 0
        property var calls: []
        property var contextValue: ({})
        property var history: []
        property var results: []

        function reset() {
            busyValue = false
            busyAcquireCount = 0
            busyReleaseCount = 0
            calls = []
            contextValue = testRoot.zoneContext("seq-a", 7, 9)
            history = []
            results = []
        }

        function request(method, args, label, showResult, callback) {
            const next = calls.slice()
            next.push({
                method: String(method || ""),
                args: args || [],
                label: String(label || ""),
                showResult: showResult === true,
                callback: callback
            })
            calls = next
            return next.length
        }

        function complete(index, response) {
            const call = calls[index]
            if (!call || typeof call.callback !== "function") {
                return false
            }
            call.callback(response)
            return true
        }

        function callsFor(method) {
            return calls.filter(function (call) {
                return call.method === method
            })
        }

        function busy() {
            return busyValue
        }

        function setBusy(value) {
            const next = value === true
            if (next && !busyValue) {
                busyAcquireCount += 1
            } else if (!next && busyValue) {
                busyReleaseCount += 1
            }
            busyValue = next
        }

        function setStatus(value) {}

        function setResult(title, text, isError, value) {
            const next = results.slice()
            next.push({
                title: String(title || ""),
                text: String(text || ""),
                isError: isError === true,
                value: value
            })
            results = next
        }

        function activeZoneContext() {
            return contextValue
        }

        function appendOperationHistory(operation, detail) {
            const next = history.slice()
            next.push({ operation: operation, detail: String(detail || "") })
            history = next
        }
    }

    QtObject {
        id: walletCapability

        property bool submitReady: true

        function reset() {
            submitReady = true
        }

        function profile() {
            return {
                wallet_binary: "/usr/bin/lee-wallet",
                wallet_home: "/tmp/test-wallet"
            }
        }

        function profileConfigured() {
            return true
        }

        function actionReady(action) {
            return action === "instruction_submit" ? submitReady : true
        }

        function gate(action, inputs) {
            return {
                enabled: true,
                status: "enabled",
                missing: [],
                warnings: [],
                provenance: ["test"]
            }
        }

        function problem(action, inputs) {
            return "wallet unavailable"
        }

        function openLocalWallet(tab) {}
    }

    Component {
        id: executionComponent

        Programs.ProgramExecutionState {
            gateway: testRoot.testGateway
            walletCapability: testRoot.testWalletCapability
        }
    }

    SignalSpy {
        id: submittedSpy
        signalName: "idlInstructionSubmitted"
    }

    function init() {
        gateway.reset()
        walletCapability.reset()
        callbackResponse = null
        callbackTarget = null
        execution = executionComponent.createObject(testRoot)
        verify(execution !== null)
        submittedSpy.target = execution
        submittedSpy.clear()
    }

    function cleanup() {
        submittedSpy.target = null
        if (execution) {
            execution.destroy()
            execution = null
        }
    }

    function zoneContext(sourceId, sourceRevision, contextRevision) {
        return {
            network_scope: {
                kind: "genesis_id",
                genesis_id: "11".repeat(32)
            },
            channel_id: "22".repeat(32),
            zone_kind: "sequencer_zone",
            selected_sequencer_source_id: String(sourceId || "seq-a"),
            indexer_source_id: null,
            source_config_revision: Number(sourceRevision || 7),
            context_revision: Number(contextRevision || 9)
        }
    }

    function entry(name) {
        const label = String(name || "Token")
        return {
            key: "idl-" + label.toLowerCase(),
            name: label,
            programIdHex: "33".repeat(32),
            ignoredMutableField: "not part of confirmation"
        }
    }

    function instructionRequest(amount) {
        return {
            idlJson: JSON.stringify({
                name: "token",
                instructions: [{ name: "transfer", accounts: [], args: [] }]
            }),
            programIdHex: "33".repeat(32),
            programBinary: "",
            dependencyBinaries: [],
            instruction: "transfer",
            accounts: {
                sender: "Public/sender",
                recipient: "Public/recipient"
            },
            args: { amount: String(amount || "1") }
        }
    }

    function targetDisplay(sourceId) {
        return {
            zone: "Test zone",
            source: String(sourceId || "seq-a"),
            endpoint: "https://sequencer.example.test"
        }
    }

    function success(value) {
        return { ok: true, value: value, text: "OK", error: "" }
    }

    function failure(error) {
        return { ok: false, value: null, text: "", error: String(error || "failed") }
    }

    function planValue(instruction) {
        return {
            instruction: String(instruction || "transfer"),
            instructions: ["transfer", "mint"],
            accounts: [],
            args: [],
            private_mode: false,
            program_binary_required: false,
            inputs_complete: true
        }
    }

    function previewValue(amount) {
        return {
            mode: "preview",
            instruction: "transfer",
            args: [{ name: "amount", value: String(amount || "1") }],
            instruction_words: [1, 2, 3]
        }
    }

    function submitValue(hash, sourceId) {
        return {
            mode: "tx",
            instruction: "transfer",
            tx_hash: String(hash || "0xtx"),
            target: {
                network_scope: gateway.contextValue.network_scope,
                channel_id: gateway.contextValue.channel_id,
                source_id: String(sourceId || "seq-verified"),
                source_config_revision: gateway.contextValue.source_config_revision,
                context_revision: gateway.contextValue.context_revision,
                request_revision: 41,
                endpoint: "https://verified.example.test"
            }
        }
    }

    function privateSubmitValue(hash, sourceId) {
        const value = submitValue(hash, sourceId || "seq-a")
        value.status = "submitted"
        value.mode = "private"
        value.program_id_hex = "33".repeat(32)
        value.instruction_words = [0, 7, 9]
        value.accounts = [{
            account_id: "Public/sender"
        }, {
            account_id: "Private/recipient"
        }]
        return value
    }

    function preparePreview(amount, display) {
        execution.reviseIdlInstructionDraft(
            entry("Token"),
            instructionRequest(amount),
            display || targetDisplay("seq-a")
        )
        execution.previewIdlInstructionDraft()
        const index = gateway.calls.length - 1
        compare(gateway.calls[index].method, "localWalletInstructionPreview")
        verify(gateway.complete(index, success(previewValue(amount))))
        verify(execution.idlInstructionPreviewCurrent())
    }

    function test_plan_is_independent_of_global_busy_and_rejects_stale_completion() {
        execution.reviseIdlInstructionDraft(entry("Token"), instructionRequest("1"), targetDisplay("seq-a"))
        execution.planIdlInstruction()
        compare(gateway.calls.length, 1)
        compare(gateway.calls[0].method, "localWalletInstructionPlan")
        verify(execution.idlInstructionPlanPending)
        verify(!gateway.busyValue)
        compare(gateway.busyAcquireCount, 0)

        execution.reviseIdlInstructionDraft(entry("Token"), instructionRequest("2"), targetDisplay("seq-a"))
        execution.planIdlInstruction()
        compare(gateway.calls.length, 2)
        verify(execution.idlInstructionPlanPending)

        verify(gateway.complete(0, success(planValue("stale"))))
        verify(execution.idlInstructionPlanPending)
        compare(execution.idlInstructionPlanValue, null)

        verify(gateway.complete(1, success(planValue("transfer"))))
        verify(!execution.idlInstructionPlanPending)
        compare(execution.idlInstructionPlanValue.instruction, "transfer")
        compare(execution.idlInstructionPlanError, "")
        compare(gateway.busyAcquireCount, 0)
    }

    function test_preview_freezes_registered_entry_request_typed_target_and_display_target() {
        const selectedEntry = entry("Token")
        const draftRequest = instructionRequest("1")
        const display = targetDisplay("seq-a")
        execution.reviseIdlInstructionDraft(selectedEntry, draftRequest, display)
        execution.previewIdlInstructionDraft()

        selectedEntry.name = "Mutated"
        draftRequest.args.amount = "999"
        display.source = "seq-mutated"
        verify(gateway.complete(0, success(previewValue("1"))))

        verify(execution.idlInstructionPreviewCurrent())
        compare(execution.idlInstructionFrozenArtifact.entry.name, "Token")
        compare(execution.idlInstructionFrozenArtifact.entry.ignoredMutableField, undefined)
        compare(execution.idlInstructionFrozenArtifact.request.args.amount, "1")
        compare(execution.idlInstructionFrozenArtifact.target.context.selected_sequencer_source_id, "seq-a")
        compare(execution.idlInstructionFrozenArtifact.targetDisplay.source, "seq-a")
        compare(execution.idlInstructionFrozenArtifact.draftRevision, execution.idlInstructionDraftRevision)
    }

    function test_cancel_sends_nothing_and_double_confirm_sends_exact_frozen_four_args_once() {
        preparePreview("1", targetDisplay("seq-a"))
        verify(execution.beginIdlInstructionConfirmation())
        compare(execution.idlInstructionConfirmation.entry.name, "Token")
        compare(execution.idlInstructionConfirmation.request.args.amount, "1")
        compare(execution.idlInstructionConfirmation.preview.mode, "preview")
        compare(execution.idlInstructionConfirmation.targetDisplay.source, "seq-a")
        compare(execution.idlInstructionConfirmation.target.context.selected_sequencer_source_id, "seq-a")

        execution.cancelIdlInstructionConfirmation()
        compare(execution.idlInstructionConfirmation, null)
        compare(gateway.callsFor("localWalletInstructionSubmit").length, 0)

        verify(execution.beginIdlInstructionConfirmation())
        execution.confirmIdlInstruction(function (response, backendTarget) {
            testRoot.callbackResponse = response
            testRoot.callbackTarget = backendTarget
        })
        execution.confirmIdlInstruction()

        const submits = gateway.callsFor("localWalletInstructionSubmit")
        compare(submits.length, 1)
        compare(submits[0].args.length, 4)
        compare(submits[0].args[0].wallet_home, "/tmp/test-wallet")
        compare(submits[0].args[1].args.amount, "1")
        compare(submits[0].args[2].context.selected_sequencer_source_id, "seq-a")
        verify(Number(submits[0].args[2].request_revision) > 0)
        compare(submits[0].args[3], "confirm-idl-instruction")
        verify(execution.idlInstructionSubmitPending)
        verify(gateway.busyValue)

        const submitIndex = gateway.calls.length - 1
        verify(gateway.complete(submitIndex, success(submitValue("0xabc", "seq-verified"))))
        verify(!execution.idlInstructionSubmitPending)
        verify(!gateway.busyValue)
        compare(execution.idlInstructionReceipt.tx_hash, "0xabc")
        compare(execution.idlInstructionReceiptTarget.source_id, "seq-verified")
        compare(callbackResponse.value.tx_hash, "0xabc")
        compare(callbackTarget.endpoint, "https://verified.example.test")
        compare(gateway.history.length, 1)
        compare(submittedSpy.count, 1)
    }

    function test_draft_edit_releases_only_stale_preview_busy_lease() {
        execution.reviseIdlInstructionDraft(entry("Token"), instructionRequest("1"), targetDisplay("seq-a"))
        execution.previewIdlInstructionDraft()
        verify(execution.idlInstructionPreviewPending)
        verify(gateway.busyValue)

        execution.reviseIdlInstructionDraft(entry("Token"), instructionRequest("2"), targetDisplay("seq-a"))
        verify(!execution.idlInstructionPreviewPending)
        verify(!gateway.busyValue)
        execution.previewIdlInstructionDraft()
        verify(gateway.busyValue)
        verify(execution.idlInstructionPreviewPending)

        verify(gateway.complete(0, success(previewValue("1"))))
        verify(gateway.busyValue)
        verify(execution.idlInstructionPreviewPending)
        compare(execution.idlInstructionPreviewValue, null)

        verify(gateway.complete(1, success(previewValue("2"))))
        verify(!gateway.busyValue)
        verify(!execution.idlInstructionPreviewPending)
        compare(execution.idlInstructionPreviewValue.args[0].value, "2")
    }

    function test_draft_revision_invalidates_preview_but_preserves_owned_receipt() {
        execution.reviseIdlInstructionDraft(entry("Token"), instructionRequest("1"), targetDisplay("seq-a"))
        execution.planIdlInstruction()
        verify(gateway.complete(0, success(planValue("transfer"))))
        execution.previewIdlInstructionDraft()
        verify(gateway.complete(1, success(previewValue("1"))))
        verify(execution.beginIdlInstructionConfirmation())
        execution.confirmIdlInstruction()
        verify(gateway.complete(2, success(submitValue("0xprior", "seq-bound"))))
        verify(execution.beginIdlInstructionConfirmation())
        verify(execution.idlInstructionPlanValue !== null)
        verify(execution.idlInstructionPreviewValue !== null)
        verify(execution.idlInstructionReceipt !== null)

        execution.reviseIdlInstructionDraft(entry("Token"), instructionRequest("2"), targetDisplay("seq-a"))

        compare(execution.idlInstructionPlanValue, null)
        compare(execution.idlInstructionPreviewValue, null)
        compare(execution.idlInstructionFrozenArtifact, null)
        compare(execution.idlInstructionConfirmation, null)
        compare(execution.idlInstructionReceipt.tx_hash, "0xprior")
        compare(execution.idlInstructionReceiptTarget.source_id, "seq-bound")
        execution.dismissIdlInstructionReceipt()
        compare(execution.idlInstructionReceipt, null)
        compare(execution.idlInstructionReceiptTarget, null)
    }

    function test_edit_after_confirm_keeps_submit_owned_and_records_exact_backend_receipt() {
        preparePreview("1", targetDisplay("seq-a"))
        verify(execution.beginIdlInstructionConfirmation())
        execution.confirmIdlInstruction(function (response, backendTarget) {
            testRoot.callbackResponse = response
            testRoot.callbackTarget = backendTarget
        })
        const submitIndex = gateway.calls.length - 1
        const submitTicket = execution.idlInstructionSubmitRequestTicket
        verify(execution.idlInstructionSubmitPending)
        verify(gateway.busyValue)

        execution.reviseIdlInstructionDraft(entry("Token"), instructionRequest("2"), targetDisplay("seq-a"))
        compare(execution.idlInstructionPreviewValue, null)
        compare(execution.idlInstructionConfirmation, null)
        compare(execution.idlInstructionReceipt, null)
        compare(execution.idlInstructionSubmitRequestTicket, submitTicket)
        verify(execution.idlInstructionSubmitPending)
        verify(gateway.busyValue)

        verify(gateway.complete(submitIndex, success(submitValue("0xsubmitted", "seq-bound"))))
        verify(!execution.idlInstructionSubmitPending)
        verify(!gateway.busyValue)
        compare(execution.idlInstructionReceipt.tx_hash, "0xsubmitted")
        compare(execution.idlInstructionReceiptTarget.source_id, "seq-bound")
        compare(callbackResponse.value.tx_hash, "0xsubmitted")
        compare(callbackTarget.source_id, "seq-bound")
        compare(gateway.history.length, 1)
    }

    function test_private_submission_freezes_local_trace_input_and_dismiss_clears_it() {
        preparePreview("1", targetDisplay("seq-a"))
        verify(execution.beginIdlInstructionConfirmation())
        execution.confirmIdlInstruction()
        const submitIndex = gateway.calls.length - 1

        execution.reviseIdlInstructionDraft(entry("Token"),
            instructionRequest("2"), targetDisplay("seq-a"))
        verify(gateway.complete(submitIndex,
            success(privateSubmitValue("ab".repeat(32), "seq-a"))))

        const traceInput = execution.idlInstructionReceiptTraceInput
        verify(traceInput !== null)
        compare(traceInput.txHash, "ab".repeat(32))
        compare(traceInput.mode, "private")
        compare(traceInput.target.source_id, "seq-a")
        compare(traceInput.context.selected_sequencer_source_id, "seq-a")
        compare(traceInput.idlKey, "idl-token")
        compare(traceInput.programIdHex, "33".repeat(32))
        compare(traceInput.instructionWords.join(","), "0,7,9")
        compare(traceInput.accountIds.join(","),
            "Public/sender,Private/recipient")
        compare(JSON.parse(traceInput.idlJson).name, "token")

        execution.dismissIdlInstructionReceipt()
        compare(execution.idlInstructionReceiptTraceInput, null)
    }

    function test_full_context_change_invalidates_preview_and_confirmation() {
        preparePreview("1", targetDisplay("seq-a"))
        verify(execution.beginIdlInstructionConfirmation())
        gateway.contextValue = zoneContext("seq-b", 8, 10)

        verify(execution.syncIdlInstructionContext(targetDisplay("seq-b")))
        compare(execution.idlInstructionPreviewValue, null)
        compare(execution.idlInstructionFrozenArtifact, null)
        compare(execution.idlInstructionConfirmation, null)
        verify(!execution.idlInstructionPreviewCurrent())
        compare(gateway.callsFor("localWalletInstructionSubmit").length, 0)
    }

    function test_legacy_preview_and_send_contracts_remain_available() {
        execution.previewIdlInstruction({ instruction: "transfer" })
        compare(gateway.calls[0].method, "localWalletInstructionPreview")
        compare(gateway.calls[0].args[0].instruction, "transfer")
        verify(gateway.complete(0, success(previewValue("1"))))
        compare(execution.idlInstructionPreviewValue.mode, "preview")

        execution.sendIdlInstruction({ instruction: "transfer" })
        compare(gateway.calls[1].method, "localWalletInstructionSubmit")
        compare(gateway.calls[1].args.length, 4)
        compare(gateway.calls[1].args[1].instruction, "transfer")
        compare(gateway.calls[1].args[2].context.selected_sequencer_source_id, "seq-a")
        compare(gateway.calls[1].args[3], "confirm-idl-instruction")
        verify(gateway.complete(1, success(submitValue("0xlegacy", "seq-bound"))))
        compare(execution.idlInstructionPreviewValue.tx_hash, "0xlegacy")
        compare(gateway.history.length, 1)
    }
}
