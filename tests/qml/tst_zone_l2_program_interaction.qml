pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/zones/controls"
import "../../qml/state/programs" as Programs
import "../../qml/theme"

TestCase {
    id: testRoot

    name: "ZoneL2ProgramInteraction"
    when: windowShown
    width: 1180
    height: 900

    property var execution: null
    property var interaction: null
    property var zoneDetail: ({
        channel_source_config: {
            config_revision: 7,
            selected_sequencer_source_id: "seq-a",
            sequencer_sources: [{
                source_id: "seq-a",
                label: "Testnet",
                target: {
                    kind: "rpc",
                    endpoint: "https://sequencer.example.test"
                }
            }]
        }
    })
    property alias testGateway: gateway
    property alias testWalletCapability: walletCapability
    property alias testAppModel: appModel
    property alias testZoneState: zoneState
    property alias testTheme: theme

    Theme {
        id: theme
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height
        color: theme.background

        Item {
            id: stage

            width: testRoot.width
            height: testRoot.height
        }
    }

    ListModel {
        id: registry
    }

    QtObject {
        id: appModel

        property alias registeredIdls: registry
        property var programExecution: null

        function idlEntryAt(index) {
            if (index < 0 || index >= registry.count) {
                return null
            }
            const row = registry.get(index)
            return {
                key: String(row.key || ""),
                name: String(row.name || ""),
                programIdHex: String(row.programIdHex || ""),
                programBinary: String(row.programBinary || ""),
                json: String(row.json || "")
            }
        }

        function idlEntryForKey(key) {
            for (let index = 0; index < registry.count; ++index) {
                const entry = idlEntryAt(index)
                if (entry.key === String(key || "")) {
                    return entry
                }
            }
            return null
        }
    }

    QtObject {
        id: zoneState

        property var activeZoneContext: testRoot.zoneContext()
    }

    QtObject {
        id: gateway

        property bool busyValue: false
        property var calls: []
        property var history: []

        function reset() {
            busyValue = false
            calls = []
            history = []
        }

        function request(method, args, label, showResult, callback) {
            const next = calls.slice()
            next.push({
                method: String(method || ""),
                args: args || [],
                callback: callback,
                completed: false
            })
            calls = next
            return next.length
        }

        function completeCall(call, response) {
            if (!call || call.completed || typeof call.callback !== "function") {
                return false
            }
            call.completed = true
            call.callback(response)
            return true
        }

        function callsFor(method) {
            return calls.filter(function (call) {
                return call.method === method
            })
        }

        function pendingCall(method) {
            const rows = callsFor(method)
            for (let index = 0; index < rows.length; ++index) {
                if (!rows[index].completed) {
                    return rows[index]
                }
            }
            return null
        }

        function lastCall(method) {
            const rows = callsFor(method)
            return rows.length > 0 ? rows[rows.length - 1] : null
        }

        function busy() {
            return busyValue
        }

        function setBusy(value) {
            busyValue = value === true
        }

        function setStatus(value) {}
        function setResult(title, text, isError, value) {}

        function activeZoneContext() {
            return zoneState.activeZoneContext
        }

        function appendOperationHistory(operation, detail) {
            const next = history.slice()
            next.push({ operation: operation, detail: String(detail || "") })
            history = next
        }
    }

    QtObject {
        id: walletCapability

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
            return true
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

    Component {
        id: interactionComponent

        ZoneL2ProgramInteraction {
            width: stage.width
            height: implicitHeight
            theme: testRoot.testTheme
            appModel: testRoot.testAppModel
            zoneState: testRoot.testZoneState
            zoneDetail: testRoot.zoneDetail
        }
    }

    SignalSpy {
        id: transactionSpy
        signalName: "transactionRequested"
    }

    SignalSpy {
        id: configureSpy
        signalName: "configureIdlsRequested"
    }

    function init() {
        gateway.reset()
        registry.clear()
        stage.width = 1180
        zoneDetail = baseZoneDetail()
        zoneState.activeZoneContext = zoneContext()
        execution = executionComponent.createObject(testRoot)
        verify(execution !== null)
        appModel.programExecution = execution
        transactionSpy.target = null
        configureSpy.target = null
    }

    function cleanup() {
        transactionSpy.target = null
        configureSpy.target = null
        if (interaction) {
            interaction.destroy()
            interaction = null
        }
        appModel.programExecution = null
        if (execution) {
            execution.destroy()
            execution = null
        }
        registry.clear()
    }

    function zoneContext() {
        return {
            network_scope: {
                kind: "genesis_id",
                genesis_id: "11".repeat(32)
            },
            channel_id: "22".repeat(32),
            zone_kind: "sequencer_zone",
            selected_sequencer_source_id: "seq-a",
            indexer_source_id: null,
            source_config_revision: 7,
            context_revision: 9
        }
    }

    function baseZoneDetail() {
        return {
            channel_source_config: {
                config_revision: 7,
                selected_sequencer_source_id: "seq-a",
                sequencer_sources: [{
                    source_id: "seq-a",
                    label: "Testnet",
                    target: {
                        kind: "rpc",
                        endpoint: "https://sequencer.example.test"
                    }
                }]
            }
        }
    }

    function tokenIdl() {
        return JSON.stringify({
            name: "token",
            instructions: [{
                name: "transfer",
                accounts: [{
                    name: "sender",
                    signer: true,
                    writable: true
                }, {
                    name: "recipient",
                    signer: false,
                    writable: true
                }],
                args: [{ name: "amount_to_transfer", type: "u128" }]
            }, {
                name: "mint",
                accounts: [],
                args: []
            }, {
                name: "new_definition_with_metadata",
                accounts: [],
                args: [{
                    name: "definition",
                    type: { defined: "NewTokenDefinition" }
                }, {
                    name: "metadata",
                    type: { defined: "Box" }
                }]
            }]
        })
    }

    function appendTokenIdl() {
        registry.append({
            key: "token-idl",
            name: "Token",
            programIdHex: "33".repeat(32),
            programBinary: "",
            json: tokenIdl()
        })
    }

    function createInteraction(includeIdl) {
        if (includeIdl !== false) {
            appendTokenIdl()
        }
        interaction = interactionComponent.createObject(stage)
        verify(interaction !== null)
        transactionSpy.target = interaction
        configureSpy.target = interaction
        transactionSpy.clear()
        configureSpy.clear()
        wait(0)
    }

    function success(value) {
        return { ok: true, value: value, text: "OK", error: "" }
    }

    function planValue(selected, complete, privateMode) {
        return {
            instruction: String(selected || ""),
            instructions: ["transfer", "mint", "new_definition_with_metadata"],
            accounts: selected ? [{
                name: "sender",
                label: "Sender signer",
                placeholder: "Public/<id> or Private/<id>",
                required: true,
                rest: false,
                kind: "account",
                type_label: ""
            }, {
                name: "recipient",
                label: "Recipient",
                placeholder: "Public/<id> or Private/<id>",
                required: true,
                rest: false,
                kind: "account",
                type_label: ""
            }] : [],
            args: selected ? [{
                name: "amount_to_transfer",
                label: "Amount to transfer (u128)",
                placeholder: "value",
                required: true,
                rest: false,
                kind: "arg",
                type_label: "u128"
            }] : [],
            private_mode: privateMode === true,
            program_binary_required: privateMode === true,
            inputs_complete: complete === true
        }
    }

    function zeroFieldPlanValue() {
        return {
            instruction: "mint",
            instructions: ["transfer", "mint", "new_definition_with_metadata"],
            accounts: [],
            args: [],
            private_mode: false,
            program_binary_required: false,
            inputs_complete: true
        }
    }

    function zeroFieldPreviewValue() {
        return {
            source: "registered_idl",
            status: "previewed",
            mode: "public",
            instruction: "mint",
            program_id_hex: "33".repeat(32),
            program_binary_required: false,
            accounts: [],
            args: [],
            instruction_words_hex: ["0x02"]
        }
    }

    function previewValue() {
        return {
            source: "registered_idl",
            status: "previewed",
            mode: "public",
            instruction: "transfer",
            program_id_hex: "33".repeat(32),
            program_binary_required: false,
            accounts: [{
                name: "sender",
                account_id: "sender-account",
                privacy: "public",
                signer: true,
                rest: false,
                pda: false
            }, {
                name: "recipient",
                account_id: "recipient-account",
                privacy: "public",
                signer: false,
                rest: false,
                pda: false
            }],
            args: [{
                name: "amount_to_transfer",
                type_label: "u128",
                value: "1"
            }],
            instruction_words_hex: ["0x01"]
        }
    }

    function receiptValue() {
        return {
            source: "registered_idl",
            status: "submitted",
            mode: "public",
            instruction: "transfer",
            tx_hash: "ab".repeat(32),
            target: {
                network_scope: zoneState.activeZoneContext.network_scope,
                channel_id: zoneState.activeZoneContext.channel_id,
                source_id: "seq-a",
                source_config_revision: 7,
                context_revision: 9,
                request_revision: 12,
                endpoint: "https://sequencer.example.test"
            }
        }
    }

    function prepareInstructionFields() {
        createInteraction(true)
        tryCompare(gateway.callsFor("localWalletInstructionPlan"), "length", 1)
        const bootstrap = gateway.pendingCall("localWalletInstructionPlan")
        compare(bootstrap.args[0].instruction, "")
        verify(gateway.completeCall(bootstrap, success(planValue("", false, false))))
        tryCompare(gateway.callsFor("localWalletInstructionPlan"), "length", 2)
        const selected = gateway.pendingCall("localWalletInstructionPlan")
        compare(selected.args[0].instruction, "transfer")
        verify(gateway.completeCall(selected, success(planValue("transfer", false, false))))
        tryVerify(function () {
            return findChild(interaction, "zoneProgramAccount_sender") !== null
                && findChild(interaction, "zoneProgramArgument_amount_to_transfer") !== null
        })
    }

    function completeLatestPlan(complete, privateMode) {
        const call = gateway.pendingCall("localWalletInstructionPlan")
        verify(call !== null)
        verify(gateway.completeCall(call,
            success(planValue("transfer", complete, privateMode))))
    }

    function fillPublicTransfer() {
        const senderField = findChild(interaction, "zoneProgramAccount_sender")
        verify(senderField !== null)
        verify(interaction.setFieldValue("accounts", "sender", "Public/sender-account"))
        compare(findChild(interaction, "zoneProgramAccount_sender"), senderField)
        completeLatestPlan(false, false)
        compare(findChild(interaction, "zoneProgramAccount_sender"), senderField)
        verify(interaction.setFieldValue("accounts", "recipient", "Public/recipient-account"))
        completeLatestPlan(false, false)
        compare(findChild(interaction, "zoneProgramAccount_sender"), senderField)
        verify(interaction.setFieldValue("args", "amount_to_transfer", "1"))
        completeLatestPlan(true, false)
        compare(findChild(interaction, "zoneProgramAccount_sender"), senderField)
        verify(interaction.planReady())
    }

    function test_empty_registry_routes_to_idl_registration() {
        createInteraction(false)
        compare(interaction.registeredIdlCount(), 0)
        const openButton = findChild(interaction, "zoneProgramOpenIdlsButton")
        verify(openButton !== null)
        verify(openButton.visible,
            "open button visible=" + openButton.visible
            + " interactionVisible=" + interaction.visible
            + " count=" + interaction.registeredIdlCount()
            + " execution=" + (interaction.execution !== null))
        openButton.clicked()
        compare(configureSpy.count, 1)
        compare(gateway.callsFor("localWalletInstructionPlan").length, 0)
    }

    function test_registered_idl_previews_confirms_submits_and_emits_exact_readback() {
        prepareInstructionFields()

        stage.width = 540
        const idlSelector = findChild(interaction, "zoneProgramIdlSelector")
        const instructionSelector = findChild(interaction, "zoneProgramInstructionSelector")
        const selectorGrid = findChild(interaction, "zoneProgramSelectorGrid")
        const fieldsGrid = findChild(interaction, "zoneProgramFieldsGrid")
        verify(idlSelector !== null)
        verify(instructionSelector !== null)
        compare(idlSelector.count, 1)
        compare(idlSelector.currentIndex, 0)
        verify(String(idlSelector.displayText).indexOf("Token") >= 0)
        verify(selectorGrid !== null)
        verify(fieldsGrid !== null)
        compare(selectorGrid.columns, 1)
        compare(fieldsGrid.columns, 1)
        tryVerify(function () {
            return idlSelector.width > 0 && idlSelector.width <= interaction.width
                && instructionSelector.width > 0
                && instructionSelector.width <= interaction.width
        })
        stage.width = 1180
        tryCompare(selectorGrid, "columns", 2)
        compare(fieldsGrid.columns, 2)

        fillPublicTransfer()
        const draft = execution.idlInstructionDraftRequest
        compare(draft.programIdHex, "33".repeat(32))
        compare(draft.instruction, "transfer")
        compare(draft.accounts.sender, "Public/sender-account")
        compare(draft.accounts.recipient, "Public/recipient-account")
        compare(draft.args.amount_to_transfer, "1")
        compare(draft.dependencyBinaries.length, 0)

        verify(interaction.previewInstruction() !== null)
        const previewCall = gateway.pendingCall("localWalletInstructionPreview")
        verify(previewCall !== null)
        compare(previewCall.args[0].args.amount_to_transfer, "1")
        verify(gateway.completeCall(previewCall, success(previewValue())))
        verify(interaction.previewCurrent())

        verify(interaction.openConfirmation())
        compare(execution.idlInstructionConfirmation.entry.name, "Token")
        compare(execution.idlInstructionConfirmation.request.args.amount_to_transfer, "1")
        compare(execution.idlInstructionConfirmation.targetDisplay.sourceId, "seq-a")
        verify(interaction.confirmationMessage().indexOf("https://sequencer.example.test") >= 0)
        const popup = findChild(interaction, "zoneProgramSendConfirmation")
        verify(popup !== null)
        tryVerify(function () { return popup.visible })
        zoneDetail = JSON.parse(JSON.stringify(zoneDetail))
        wait(10)
        verify(popup.visible)
        verify(execution.idlInstructionConfirmation !== null)
        const cancelButton = findChild(popup.contentItem, "cancelButton")
        verify(cancelButton !== null)
        mouseClick(cancelButton, cancelButton.width / 2, cancelButton.height / 2)
        tryVerify(function () { return !popup.visible })
        compare(gateway.callsFor("localWalletInstructionSubmit").length, 0)

        verify(interaction.openConfirmation())
        tryVerify(function () { return popup.visible })
        const confirmButton = findChild(popup.contentItem, "confirmButton")
        verify(confirmButton !== null)
        verify(confirmButton.enabled)
        mouseClick(confirmButton, confirmButton.width / 2, confirmButton.height / 2)
        const submitCall = gateway.pendingCall("localWalletInstructionSubmit")
        verify(submitCall !== null)
        compare(submitCall.args.length, 4)
        compare(submitCall.args[1].args.amount_to_transfer, "1")
        compare(submitCall.args[2].context.selected_sequencer_source_id, "seq-a")
        compare(submitCall.args[3], "confirm-idl-instruction")
        verify(gateway.completeCall(submitCall, success(receiptValue())))
        compare(transactionSpy.count, 0)
        const receiptButton = findChild(interaction, "zoneProgramOpenReceiptButton")
        verify(receiptButton !== null)
        verify(receiptButton.enabled)
        mouseClick(receiptButton, receiptButton.width / 2, receiptButton.height / 2)
        compare(transactionSpy.count, 1)
        compare(transactionSpy.signalArguments[0][0], "ab".repeat(32))
        compare(transactionSpy.signalArguments[0][1], "seq-a")
        compare(execution.idlInstructionReceiptTarget.source_id, "seq-a")
        compare(interaction.previewAccountRows()[1].cells[2].text, "Non-signer")
    }

    function test_backend_target_mismatch_retains_receipt_without_wrong_readback() {
        prepareInstructionFields()
        fillPublicTransfer()
        verify(interaction.previewInstruction() !== null)
        verify(gateway.completeCall(
            gateway.pendingCall("localWalletInstructionPreview"),
            success(previewValue())))
        verify(interaction.openConfirmation())
        const popup = findChild(interaction, "zoneProgramSendConfirmation")
        const confirmButton = popup ? findChild(popup.contentItem, "confirmButton") : null
        verify(confirmButton !== null)
        mouseClick(confirmButton, confirmButton.width / 2, confirmButton.height / 2)
        const submitCall = gateway.pendingCall("localWalletInstructionSubmit")
        verify(submitCall !== null)
        const mismatchedReceipt = receiptValue()
        mismatchedReceipt.target.network_scope = {
            kind: "genesis_id",
            genesis_id: "44".repeat(32)
        }
        verify(gateway.completeCall(submitCall, success(mismatchedReceipt)))
        compare(transactionSpy.count, 0)
        compare(execution.idlInstructionReceipt.tx_hash, "ab".repeat(32))
        compare(execution.idlInstructionReceiptTarget.source_id, "seq-a")
        const receiptButton = findChild(interaction, "zoneProgramOpenReceiptButton")
        verify(receiptButton !== null)
        verify(!receiptButton.enabled)
        verify(hasVisibleText(interaction, "active Zone or Sequencer changed"))
    }

    function test_completed_replans_preserve_field_and_keyboard_focus() {
        prepareInstructionFields()
        const field = findChild(interaction, "zoneProgramAccount_sender")
        const input = findChild(interaction, "zoneProgramAccount_senderInput")
        verify(field !== null)
        verify(input !== null)
        testWindow.requestActivate()
        input.forceActiveFocus(Qt.MouseFocusReason)
        tryVerify(function () { return input.activeFocus })

        const keys = [{ key: Qt.Key_P, modifiers: Qt.ShiftModifier },
            { key: Qt.Key_U, modifiers: Qt.NoModifier },
            { key: Qt.Key_B, modifiers: Qt.NoModifier }]
        for (let index = 0; index < keys.length; ++index) {
            keyClick(keys[index].key, keys[index].modifiers)
            const replan = gateway.pendingCall("localWalletInstructionPlan")
            verify(replan !== null)
            verify(gateway.completeCall(
                replan, success(planValue("transfer", false, false))))
            compare(findChild(interaction, "zoneProgramAccount_sender"), field)
            compare(findChild(interaction, "zoneProgramAccount_senderInput"), input)
            verify(input.activeFocus)
        }
        compare(input.text, "pub")
    }

    function test_zero_field_instruction_renders_preview_and_review_actions() {
        prepareInstructionFields()
        verify(interaction.selectInstruction(1))
        const planCall = gateway.pendingCall("localWalletInstructionPlan")
        verify(planCall !== null)
        compare(planCall.args[0].instruction, "mint")
        verify(gateway.completeCall(planCall, success(zeroFieldPlanValue())))

        tryVerify(function () {
            return interaction.renderedPlan !== null
                && interaction.renderedPlan.instruction === "mint"
        })
        const fieldsGrid = findChild(interaction, "zoneProgramFieldsGrid")
        const previewButton = findChild(interaction, "zoneProgramPreviewButton")
        const sendButton = findChild(interaction, "zoneProgramSendButton")
        verify(fieldsGrid !== null && fieldsGrid.visible)
        verify(previewButton !== null && previewButton.visible && previewButton.enabled)
        verify(sendButton !== null && sendButton.visible && !sendButton.enabled)

        verify(interaction.previewInstruction() !== null)
        const previewCall = gateway.pendingCall("localWalletInstructionPreview")
        verify(previewCall !== null)
        compare(previewCall.args[0].instruction, "mint")
        verify(gateway.completeCall(previewCall, success(zeroFieldPreviewValue())))
        tryVerify(function () {
            return interaction.previewCurrent() && sendButton.enabled
        })
    }

    function test_unsupported_instruction_fails_closed_and_recovers() {
        prepareInstructionFields()
        verify(interaction.selectInstruction(2))
        const unsupportedPlan = gateway.pendingCall("localWalletInstructionPlan")
        verify(unsupportedPlan !== null)
        compare(unsupportedPlan.args[0].instruction,
            "new_definition_with_metadata")
        verify(gateway.completeCall(unsupportedPlan, {
            ok: false,
            value: null,
            text: "",
            error: "instruction `new_definition_with_metadata` argument `definition` cannot be used: defined IDL arg type `NewTokenDefinition` is not supported for direct interaction"
        }))

        tryVerify(function () {
            return execution.idlInstructionPlanValue === null
                && execution.idlInstructionPlanError.indexOf(
                    "NewTokenDefinition") >= 0
        })
        compare(interaction.renderedPlan, null)
        const fieldsGrid = findChild(interaction, "zoneProgramFieldsGrid")
        const previewButton = findChild(interaction, "zoneProgramPreviewButton")
        verify(fieldsGrid !== null && !fieldsGrid.visible)
        verify(previewButton !== null && !previewButton.enabled)
        verify(hasVisibleText(interaction, "Instruction plan unavailable"))
        compare(gateway.callsFor("localWalletInstructionPreview").length, 0)
        compare(gateway.callsFor("localWalletInstructionSubmit").length, 0)

        verify(interaction.selectInstruction(0))
        const supportedPlan = gateway.pendingCall("localWalletInstructionPlan")
        verify(supportedPlan !== null)
        compare(supportedPlan.args[0].instruction, "transfer")
        verify(gateway.completeCall(
            supportedPlan, success(planValue("transfer", false, false))))
        tryVerify(function () {
            return interaction.renderedPlan !== null
                && interaction.renderedPlan.instruction === "transfer"
                && execution.idlInstructionPlanError.length === 0
        })
        verify(fieldsGrid.visible)
        compare(gateway.callsFor("localWalletInstructionPreview").length, 0)
        compare(gateway.callsFor("localWalletInstructionSubmit").length, 0)
    }

    function test_submit_receipt_survives_interaction_destruction_and_reopens_exact_source() {
        prepareInstructionFields()
        fillPublicTransfer()
        verify(interaction.previewInstruction() !== null)
        verify(gateway.completeCall(
            gateway.pendingCall("localWalletInstructionPreview"),
            success(previewValue())))
        verify(interaction.openConfirmation())
        const popup = findChild(interaction, "zoneProgramSendConfirmation")
        const confirmButton = popup ? findChild(popup.contentItem, "confirmButton") : null
        verify(confirmButton !== null)
        mouseClick(confirmButton, confirmButton.width / 2, confirmButton.height / 2)
        const submitCall = gateway.pendingCall("localWalletInstructionSubmit")
        verify(submitCall !== null)

        transactionSpy.target = null
        interaction.destroy()
        interaction = null
        wait(0)
        verify(gateway.completeCall(submitCall, success(receiptValue())))
        compare(execution.idlInstructionReceipt.tx_hash, "ab".repeat(32))

        const revisitedContext = zoneContext()
        revisitedContext.context_revision += 1
        zoneState.activeZoneContext = revisitedContext

        interaction = interactionComponent.createObject(stage)
        verify(interaction !== null)
        transactionSpy.target = interaction
        transactionSpy.clear()
        wait(0)
        compare(execution.idlInstructionReceipt.tx_hash, "ab".repeat(32))
        const receiptButton = findChild(interaction, "zoneProgramOpenReceiptButton")
        verify(receiptButton !== null)
        verify(receiptButton.enabled)
        mouseClick(receiptButton, receiptButton.width / 2, receiptButton.height / 2)
        compare(transactionSpy.count, 1)
        compare(transactionSpy.signalArguments[0][0], "ab".repeat(32))
        compare(transactionSpy.signalArguments[0][1], "seq-a")
    }

    function test_stale_target_blocks_preview_then_replans_current_detail() {
        prepareInstructionFields()
        fillPublicTransfer()
        verify(interaction.previewInstruction() !== null)
        verify(gateway.completeCall(
            gateway.pendingCall("localWalletInstructionPreview"),
            success(previewValue())))
        verify(interaction.previewCurrent())
        verify(interaction.openConfirmation())

        const changedContext = zoneContext()
        changedContext.source_config_revision = 8
        changedContext.context_revision = 10
        zoneState.activeZoneContext = changedContext
        const contextReplan = gateway.pendingCall("localWalletInstructionPlan")
        verify(contextReplan !== null)
        verify(gateway.completeCall(
            contextReplan, success(planValue("transfer", true, false))))
        verify(!interaction.targetDisplayReady())
        const previewButton = findChild(interaction, "zoneProgramPreviewButton")
        verify(previewButton !== null)
        verify(!previewButton.enabled)
        verify(hasVisibleText(interaction, "Sequencer target refreshing"))

        const changed = baseZoneDetail()
        changed.channel_source_config.config_revision = 8
        changed.channel_source_config.sequencer_sources[0].target.endpoint
            = "https://replacement.example.test"
        zoneDetail = changed
        const detailReplan = gateway.pendingCall("localWalletInstructionPlan")
        verify(detailReplan !== null)
        verify(gateway.completeCall(
            detailReplan, success(planValue("transfer", true, false))))
        tryVerify(function () { return !interaction.previewCurrent() })
        verify(interaction.targetDisplayReady())
        verify(interaction.planReady())
        verify(previewButton.enabled)
        compare(execution.idlInstructionConfirmation, null)
        compare(gateway.callsFor("localWalletInstructionSubmit").length, 0)
    }

    function test_edit_invalidates_preview_and_private_reference_blocks_send() {
        prepareInstructionFields()
        fillPublicTransfer()
        verify(interaction.previewInstruction() !== null)
        const previewCall = gateway.pendingCall("localWalletInstructionPreview")
        verify(gateway.completeCall(previewCall, success(previewValue())))
        verify(interaction.previewCurrent())
        verify(interaction.openConfirmation())

        verify(interaction.setFieldValue("args", "amount_to_transfer", "2"))
        verify(!interaction.previewCurrent())
        compare(execution.idlInstructionConfirmation, null)
        completeLatestPlan(true, false)

        verify(interaction.setFieldValue("accounts", "sender", "Private/sender-account"))
        verify(interaction.privateDraft())
        completeLatestPlan(true, true)
        verify(interaction.privateDraft())
        const previewButton = findChild(interaction, "zoneProgramPreviewButton")
        verify(previewButton !== null)
        verify(!previewButton.enabled)
        verify(hasVisibleText(interaction, "Private interaction not enabled here"),
            "private warning missing; privateDraft=" + interaction.privateDraft()
            + " planPrivate=" + (interaction.plan && interaction.plan.private_mode))
        compare(gateway.callsFor("localWalletInstructionSubmit").length, 0)

        verify(interaction.setFieldValue(
            "accounts", "sender", "Public/sender-account"))
        verify(!interaction.privateDraft())
        completeLatestPlan(true, false)
        verify(!interaction.privateDraft())
        verify(previewButton.enabled)
    }

    function hasVisibleText(item, value) {
        if (!item) {
            return false
        }
        if (item.visible !== false && item.text !== undefined
                && String(item.text).indexOf(value) >= 0) {
            return true
        }
        const children = item.children || []
        for (let index = 0; index < children.length; ++index) {
            if (hasVisibleText(children[index], value)) {
                return true
            }
        }
        return false
    }

}
