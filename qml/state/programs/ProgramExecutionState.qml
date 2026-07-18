import QtQml
import "../ConfirmationPolicy.js" as ConfirmationPolicy
import "ProgramOperationDetails.js" as ProgramOperationDetails

QtObject {
    id: root

    required property var gateway
    property var capabilityFacade: null
    property var walletCapability: null

    property var idlInstructionPreviewValue: null
    property string idlInstructionError: ""
    property int instructionTargetRequestRevision: 0

    function deployProgramBinary(programPath) {
        const path = String(programPath || "").trim()
        if (busy()) {
            setResult(qsTr("Program deploy"), qsTr("Another inspection is already running."), true, null)
            return null
        }
        if (!path.length) {
            setResult(qsTr("Program deploy"), qsTr("Program binary path is required."), true, null)
            return null
        }
        if (!walletProfileConfigured()) {
            openLocalWallet("profiles")
            setResult(qsTr("Program deploy"), qsTr("Configure wallet binary and wallet home before deploying a program."), true, null)
            return null
        }
        if (!walletActionEnabled("program.deploy", [{ key: "path", label: qsTr("Program binary path"), value: path }])) {
            setResult(qsTr("Program deploy"), walletGateProblem("program.deploy"), true, null)
            return null
        }

        setBusy(true)
        return request("localWalletDeployProgram", [walletProfile(), path, ConfirmationPolicy.token("wallet-deploy-program")], qsTr("Program deploy"), true, function (response) {
            setBusy(false)
            const detail = response.ok
                ? deployProgramOperationDetail(response.value)
                : String(response.error || qsTr("Program deployment failed."))
            appendOperationHistory({
                domain: "execution",
                method: qsTr("Program deploy"),
                status: response.ok ? "completed" : "failed",
                label: qsTr("Program deploy"),
                result: response.ok ? response.value || {} : null,
                error: response.ok ? "" : detail
            }, detail)
        })
    }

    function previewIdlInstruction(requestPayload) {
        if (busy()) {
            setResult(qsTr("IDL instruction"), qsTr("Another inspection is already running."), true, null)
            return null
        }
        idlInstructionPreviewValue = null
        idlInstructionError = ""
        if (!walletActionEnabled("l2.preview", [])) {
            idlInstructionError = walletGateProblem("l2.preview")
            setResult(qsTr("IDL instruction"), idlInstructionError, true, null)
            return null
        }
        setBusy(true)
        setStatus(qsTr("IDL instruction"))
        return request("localWalletInstructionPreview", [requestPayload || {}], qsTr("IDL instruction"), false, function (response) {
            setBusy(false)
            if (response.ok) {
                idlInstructionPreviewValue = response.value || null
                idlInstructionError = ""
            } else {
                idlInstructionPreviewValue = null
                idlInstructionError = response.error || qsTr("Instruction preview failed.")
            }
        })
    }

    function sendIdlInstruction(requestPayload) {
        if (busy()) {
            setResult(qsTr("IDL instruction"), qsTr("Another inspection is already running."), true, null)
            return null
        }
        if (!walletInstructionSubmitReady()) {
            openLocalWallet("profiles")
            setResult(qsTr("IDL instruction"), qsTr("Configure wallet home before sending an IDL instruction."), true, null)
            return null
        }
        if (!walletActionEnabled("l2.submit", [])) {
            setResult(qsTr("IDL instruction"), walletGateProblem("l2.submit"), true, null)
            return null
        }
        const target = nextInstructionTarget()
        if (!target) {
            setResult(qsTr("IDL instruction"), qsTr("Select a verified Zone with a Sequencer source before sending an instruction."), true, null)
            return null
        }

        idlInstructionPreviewValue = null
        idlInstructionError = ""
        setBusy(true)
        return request("localWalletInstructionSubmit", [walletProfile(), requestPayload || {}, target, ConfirmationPolicy.token("wallet-instruction-submit")], qsTr("IDL instruction"), true, function (response) {
            setBusy(false)
            const detail = response.ok
                ? idlInstructionOperationDetail(response.value)
                : String(response.error || qsTr("Instruction send failed."))
            if (response.ok) {
                idlInstructionPreviewValue = response.value || null
                idlInstructionError = ""
            } else {
                idlInstructionPreviewValue = null
                idlInstructionError = detail
            }
            appendOperationHistory({
                domain: "execution",
                method: qsTr("IDL instruction"),
                status: response.ok ? "completed" : "failed",
                label: qsTr("IDL instruction"),
                result: response.ok ? response.value || {} : null,
                error: response.ok ? "" : detail
            }, detail)
        })
    }

    function nextInstructionTarget() {
        const context = gateway && typeof gateway.activeZoneContext === "function"
            ? gateway.activeZoneContext() : null
        if (!context
                || !String(context.channel_id || "").length
                || !String(context.selected_sequencer_source_id || "").length
                || Number(context.source_config_revision || 0) <= 0
                || Number(context.context_revision || 0) <= 0) {
            return null
        }
        instructionTargetRequestRevision += 1
        return {
            context: JSON.parse(JSON.stringify(context)),
            request_revision: instructionTargetRequestRevision
        }
    }

    function deployProgramOperationDetail(value) {
        return ProgramOperationDetails.deployProgramOperationDetail(value)
    }

    function idlInstructionOperationDetail(value) {
        return ProgramOperationDetails.idlInstructionOperationDetail(value)
    }

    function busy() {
        return gateway && typeof gateway.busy === "function" && gateway.busy()
    }

    function setBusy(value) {
        if (gateway && typeof gateway.setBusy === "function") {
            gateway.setBusy(value)
        }
    }

    function setStatus(value) {
        if (gateway && typeof gateway.setStatus === "function") {
            gateway.setStatus(value)
        }
    }

    function request(method, args, label, showResult, callback) {
        if (gateway && typeof gateway.request === "function") {
            return gateway.request(method, args, label, showResult, callback)
        }
        return null
    }

    function setResult(title, text, isError, value) {
        if (gateway && typeof gateway.setResult === "function") {
            gateway.setResult(title, text, isError, value)
        }
    }

    function walletProfile() {
        if (walletCapability && typeof walletCapability.profile === "function") {
            return walletCapability.profile()
        }
        return gateway && typeof gateway.walletProfile === "function" ? gateway.walletProfile() : ({})
    }

    function walletProfileConfigured() {
        if (walletCapability && typeof walletCapability.profileConfigured === "function") {
            return walletCapability.profileConfigured()
        }
        return gateway && typeof gateway.walletProfileConfigured === "function" && gateway.walletProfileConfigured()
    }

    function walletHomeConfigured() {
        if (walletCapability && typeof walletCapability.homeConfigured === "function") {
            return walletCapability.homeConfigured()
        }
        return gateway && typeof gateway.walletHomeConfigured === "function" && gateway.walletHomeConfigured()
    }

    function walletInstructionSubmitReady() {
        if (walletCapability && typeof walletCapability.actionReady === "function") {
            return walletCapability.actionReady("instruction_submit")
        }
        return walletHomeConfigured()
    }

    function openLocalWallet(tab) {
        if (walletCapability && typeof walletCapability.openLocalWallet === "function") {
            walletCapability.openLocalWallet(tab)
            return
        }
        if (gateway && typeof gateway.openLocalWallet === "function") {
            gateway.openLocalWallet(tab)
        }
    }

    function appendOperationHistory(operation, detail) {
        if (gateway && typeof gateway.appendOperationHistory === "function") {
            gateway.appendOperationHistory(operation, detail)
        }
    }

    function walletActionGate(action, requiredInputs) {
        if (walletCapability && typeof walletCapability.gate === "function") {
            return walletCapability.gate(action, requiredInputs)
        }
        if (capabilityFacade && typeof capabilityFacade.walletGate === "function") {
            return capabilityFacade.walletGate(action, {
                required_inputs: Array.isArray(requiredInputs) ? requiredInputs : []
            })
        }
        return {
            enabled: true,
            status: "enabled",
            missing: [],
            warnings: [],
            provenance: ["program_execution_compatibility"]
        }
    }

    function walletActionEnabled(action, requiredInputs) {
        return walletActionGate(action, requiredInputs).enabled === true
    }

    function walletGateProblem(action) {
        if (walletCapability && typeof walletCapability.problem === "function") {
            return walletCapability.problem(action, [])
        }
        const gate = walletActionGate(action, [])
        const missing = Array.isArray(gate.missing) ? gate.missing : []
        if (missing.length > 0) {
            return String(missing[0].label || missing[0].dependency || qsTr("Wallet capability unavailable."))
        }
        return qsTr("Wallet capability unavailable.")
    }
}
