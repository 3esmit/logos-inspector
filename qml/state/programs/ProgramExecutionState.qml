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

    property var idlInstructionDraft: null
    property var idlInstructionDraftEntry: null
    property var idlInstructionDraftRequest: null
    property var idlInstructionDraftTargetDisplay: null
    property var idlInstructionDraftContext: null
    property int idlInstructionDraftRevision: 0

    property var idlInstructionPlanValue: null
    property string idlInstructionPlanError: ""
    property bool idlInstructionPlanPending: false
    property bool idlInstructionPreviewPending: false
    property bool idlInstructionSubmitPending: false

    property var idlInstructionFrozenEntry: null
    property var idlInstructionFrozenRequest: null
    property var idlInstructionFrozenPreview: null
    property var idlInstructionFrozenTarget: null
    property var idlInstructionFrozenTargetDisplay: null
    property int idlInstructionFrozenDraftRevision: 0
    property var idlInstructionFrozenArtifact: null
    property var idlInstructionConfirmation: null
    property var idlInstructionReceipt: null
    property var idlInstructionReceiptTarget: null
    // Session-only evidence for decoding a privacy-preserving submission after
    // exact-source readback. This is never derived from remote chain payloads.
    property var idlInstructionReceiptTraceInput: null

    property int idlInstructionPlanRequestTicket: 0
    property int idlInstructionPreviewRequestTicket: 0
    property int idlInstructionSubmitRequestTicket: 0
    property string idlInstructionBusyLane: ""
    property int idlInstructionBusyTicket: 0

    signal idlInstructionSubmitted(var response, var backendTarget)

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
        clearIdlInstructionPreviewArtifacts()
        if (!walletActionEnabled("l2.preview", [])) {
            idlInstructionError = walletGateProblem("l2.preview")
            setResult(qsTr("IDL instruction"), idlInstructionError, true, null)
            return null
        }
        const ticket = idlInstructionPreviewRequestTicket + 1
        idlInstructionPreviewRequestTicket = ticket
        idlInstructionPreviewPending = true
        acquireIdlInstructionBusy("preview", ticket)
        setStatus(qsTr("IDL instruction"))
        return request("localWalletInstructionPreview", [requestPayload || {}], qsTr("IDL instruction"), false, function (response) {
            if (ticket !== idlInstructionPreviewRequestTicket) {
                return
            }
            idlInstructionPreviewPending = false
            releaseIdlInstructionBusy("preview", ticket)
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

        clearIdlInstructionPreviewArtifacts()
        clearIdlInstructionReceiptArtifacts()
        const ticket = idlInstructionSubmitRequestTicket + 1
        idlInstructionSubmitRequestTicket = ticket
        idlInstructionSubmitPending = true
        acquireIdlInstructionBusy("submit", ticket)
        return request("localWalletInstructionSubmit", [walletProfile(), requestPayload || {}, target, ConfirmationPolicy.token("wallet-instruction-submit")], qsTr("IDL instruction"), true, function (response) {
            if (ticket !== idlInstructionSubmitRequestTicket) {
                return
            }
            idlInstructionSubmitPending = false
            releaseIdlInstructionBusy("submit", ticket)
            const detail = response.ok
                ? idlInstructionOperationDetail(response.value)
                : String(response.error || qsTr("Instruction send failed."))
            if (response.ok) {
                idlInstructionPreviewValue = response.value || null
                idlInstructionReceipt = frozenValue(response.value || null)
                idlInstructionReceiptTarget = frozenValue((response.value || {}).target || null)
                idlInstructionReceiptTraceInput = null
                idlInstructionError = ""
            } else {
                idlInstructionPreviewValue = null
                clearIdlInstructionReceiptArtifacts()
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

    function reviseIdlInstructionDraft(entry, requestPayload, targetDisplay) {
        const nextEntry = instructionEntryMetadata(entry)
        const nextRequest = frozenValue(requestPayload || {})
        const nextTargetDisplay = frozenValue(targetDisplay)
        const nextContext = currentInstructionContext()
        if (jsonEqual(idlInstructionDraftEntry, nextEntry)
                && jsonEqual(idlInstructionDraftRequest, nextRequest)
                && jsonEqual(idlInstructionDraftTargetDisplay, nextTargetDisplay)
                && jsonEqual(idlInstructionDraftContext, nextContext)) {
            return false
        }
        idlInstructionDraftEntry = nextEntry
        idlInstructionDraftRequest = nextRequest
        idlInstructionDraftTargetDisplay = nextTargetDisplay
        idlInstructionDraftContext = nextContext
        reviseIdlInstructionArtifacts()
        refreshIdlInstructionDraft()
        return true
    }

    function syncIdlInstructionContext(targetDisplay) {
        if (!idlInstructionDraft) {
            return false
        }
        const nextContext = currentInstructionContext()
        const nextTargetDisplay = arguments.length > 0
            ? frozenValue(targetDisplay) : frozenValue(idlInstructionDraftTargetDisplay)
        if (jsonEqual(idlInstructionDraftContext, nextContext)
                && jsonEqual(idlInstructionDraftTargetDisplay, nextTargetDisplay)) {
            return instructionContextUsable(nextContext)
        }
        idlInstructionDraftContext = nextContext
        idlInstructionDraftTargetDisplay = nextTargetDisplay
        reviseIdlInstructionArtifacts()
        refreshIdlInstructionDraft()
        return instructionContextUsable(nextContext)
    }

    function planIdlInstruction() {
        if (!idlInstructionDraftRequest || !instructionEntryUsable(idlInstructionDraftEntry)) {
            idlInstructionPlanValue = null
            idlInstructionPlanError = qsTr("Select a registered IDL before planning an instruction.")
            return null
        }
        const ticket = idlInstructionPlanRequestTicket + 1
        const revision = idlInstructionDraftRevision
        const plannedRequest = frozenValue(idlInstructionDraftRequest)
        idlInstructionPlanRequestTicket = ticket
        idlInstructionPlanPending = true
        idlInstructionPlanError = ""
        return request("localWalletInstructionPlan", [plannedRequest], qsTr("IDL instruction plan"), false, function (response) {
            if (ticket !== idlInstructionPlanRequestTicket
                    || revision !== idlInstructionDraftRevision) {
                return
            }
            idlInstructionPlanPending = false
            if (response.ok) {
                idlInstructionPlanValue = frozenValue(response.value || null)
                idlInstructionPlanError = ""
            } else {
                idlInstructionPlanValue = null
                idlInstructionPlanError = String(response.error || qsTr("Instruction plan failed."))
            }
        })
    }

    function previewIdlInstructionDraft() {
        if (!idlInstructionDraftRequest || !instructionEntryUsable(idlInstructionDraftEntry)) {
            idlInstructionError = qsTr("Select a registered IDL before previewing an instruction.")
            return null
        }
        syncIdlInstructionContext()
        const target = nextInstructionTargetForContext(idlInstructionDraftContext)
        if (!target) {
            idlInstructionError = qsTr("Select a verified Zone with a Sequencer source before previewing an instruction.")
            return null
        }
        if (busy()) {
            setResult(qsTr("IDL instruction"), qsTr("Another inspection is already running."), true, null)
            return null
        }
        if (!walletActionEnabled("l2.preview", [])) {
            idlInstructionError = walletGateProblem("l2.preview")
            setResult(qsTr("IDL instruction"), idlInstructionError, true, null)
            return null
        }

        clearIdlInstructionPreviewArtifacts()
        const ticket = idlInstructionPreviewRequestTicket + 1
        const revision = idlInstructionDraftRevision
        const previewedEntry = frozenValue(idlInstructionDraftEntry)
        const previewedRequest = frozenValue(idlInstructionDraftRequest)
        const previewedTarget = frozenValue(target)
        const previewedTargetDisplay = frozenValue(idlInstructionDraftTargetDisplay)
        idlInstructionPreviewRequestTicket = ticket
        idlInstructionPreviewPending = true
        acquireIdlInstructionBusy("preview", ticket)
        setStatus(qsTr("IDL instruction"))
        return request("localWalletInstructionPreview", [previewedRequest], qsTr("IDL instruction"), false, function (response) {
            if (ticket !== idlInstructionPreviewRequestTicket
                    || revision !== idlInstructionDraftRevision) {
                return
            }
            idlInstructionPreviewPending = false
            releaseIdlInstructionBusy("preview", ticket)
            if (response.ok) {
                const preview = frozenValue(response.value || null)
                idlInstructionPreviewValue = preview
                idlInstructionFrozenEntry = previewedEntry
                idlInstructionFrozenRequest = previewedRequest
                idlInstructionFrozenPreview = preview
                idlInstructionFrozenTarget = previewedTarget
                idlInstructionFrozenTargetDisplay = previewedTargetDisplay
                idlInstructionFrozenDraftRevision = revision
                idlInstructionFrozenArtifact = {
                    entry: frozenValue(previewedEntry),
                    request: frozenValue(previewedRequest),
                    preview: frozenValue(preview),
                    target: frozenValue(previewedTarget),
                    targetDisplay: frozenValue(previewedTargetDisplay),
                    draftRevision: revision
                }
                idlInstructionError = ""
            } else {
                idlInstructionError = String(response.error || qsTr("Instruction preview failed."))
            }
        })
    }

    function idlInstructionPreviewCurrent() {
        if (!idlInstructionFrozenPreview
                || idlInstructionFrozenDraftRevision !== idlInstructionDraftRevision) {
            return false
        }
        const currentContext = currentInstructionContext()
        return jsonEqual(idlInstructionFrozenEntry, idlInstructionDraftEntry)
            && jsonEqual(idlInstructionFrozenRequest, idlInstructionDraftRequest)
            && jsonEqual(idlInstructionFrozenTargetDisplay, idlInstructionDraftTargetDisplay)
            && jsonEqual((idlInstructionFrozenTarget || {}).context || null, currentContext)
            && jsonEqual(idlInstructionDraftContext, currentContext)
    }

    function beginIdlInstructionConfirmation() {
        syncIdlInstructionContext()
        if (!idlInstructionPreviewCurrent()) {
            idlInstructionConfirmation = null
            idlInstructionError = qsTr("Preview the current instruction before confirming it.")
            return false
        }
        idlInstructionConfirmation = {
            entry: frozenValue(idlInstructionFrozenEntry),
            request: frozenValue(idlInstructionFrozenRequest),
            preview: frozenValue(idlInstructionFrozenPreview),
            targetDisplay: frozenValue(idlInstructionFrozenTargetDisplay),
            target: frozenValue(idlInstructionFrozenTarget),
            draftRevision: idlInstructionFrozenDraftRevision
        }
        return true
    }

    function cancelIdlInstructionConfirmation() {
        idlInstructionConfirmation = null
    }

    function confirmIdlInstruction(callback) {
        if (!idlInstructionConfirmation || idlInstructionSubmitPending) {
            return null
        }
        if (!idlInstructionPreviewCurrent()) {
            idlInstructionConfirmation = null
            idlInstructionError = qsTr("Instruction preview is no longer current.")
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
        if (busy()) {
            setResult(qsTr("IDL instruction"), qsTr("Another inspection is already running."), true, null)
            return null
        }

        const confirmation = frozenValue(idlInstructionConfirmation)
        idlInstructionConfirmation = null
        clearIdlInstructionReceiptArtifacts()
        idlInstructionError = ""
        const ticket = idlInstructionSubmitRequestTicket + 1
        idlInstructionSubmitRequestTicket = ticket
        idlInstructionSubmitPending = true
        acquireIdlInstructionBusy("submit", ticket)
        return request("localWalletInstructionSubmit", [
            walletProfile(),
            confirmation.request,
            confirmation.target,
            ConfirmationPolicy.token("wallet-instruction-submit")
        ], qsTr("IDL instruction"), true, function (response) {
            if (ticket !== idlInstructionSubmitRequestTicket) {
                return
            }
            idlInstructionSubmitPending = false
            releaseIdlInstructionBusy("submit", ticket)
            const detail = response.ok
                ? idlInstructionOperationDetail(response.value)
                : String(response.error || qsTr("Instruction send failed."))
            const backendTarget = response.ok
                ? frozenValue((response.value || {}).target || null) : null
            if (response.ok) {
                const receipt = frozenValue(response.value || null)
                idlInstructionReceipt = receipt
                idlInstructionReceiptTarget = backendTarget
                idlInstructionReceiptTraceInput = privateReceiptTraceInput(
                    confirmation, receipt, backendTarget)
                idlInstructionError = ""
            } else {
                clearIdlInstructionReceiptArtifacts()
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
            root.idlInstructionSubmitted(response, backendTarget)
            if (typeof callback === "function") {
                callback(response, backendTarget)
            }
        })
    }

    function nextInstructionTarget() {
        return nextInstructionTargetForContext(currentInstructionContext())
    }

    function nextInstructionTargetForContext(context) {
        if (!instructionContextUsable(context)) {
            return null
        }
        instructionTargetRequestRevision += 1
        return {
            context: JSON.parse(JSON.stringify(context)),
            request_revision: instructionTargetRequestRevision
        }
    }

    function currentInstructionContext() {
        const context = gateway && typeof gateway.activeZoneContext === "function"
            ? gateway.activeZoneContext() : null
        return context ? frozenValue(context) : null
    }

    function instructionContextUsable(context) {
        return context
            && String(context.channel_id || "").length > 0
            && String(context.selected_sequencer_source_id || "").length > 0
            && Number(context.source_config_revision || 0) > 0
            && Number(context.context_revision || 0) > 0
    }

    function instructionEntryMetadata(entry) {
        const value = entry || {}
        return {
            key: String(value.key || ""),
            name: String(value.name || ""),
            programIdHex: String(value.programIdHex || value.program_id_hex || "")
        }
    }

    function instructionEntryUsable(entry) {
        const value = entry || {}
        return String(value.key || "").length > 0
            && String(value.programIdHex || "").length > 0
    }

    function refreshIdlInstructionDraft() {
        idlInstructionDraft = {
            entry: frozenValue(idlInstructionDraftEntry),
            request: frozenValue(idlInstructionDraftRequest),
            targetDisplay: frozenValue(idlInstructionDraftTargetDisplay),
            context: frozenValue(idlInstructionDraftContext),
            revision: idlInstructionDraftRevision
        }
    }

    function reviseIdlInstructionArtifacts() {
        idlInstructionDraftRevision += 1
        idlInstructionPlanRequestTicket += 1
        idlInstructionPreviewRequestTicket += 1
        idlInstructionPlanPending = false
        idlInstructionPreviewPending = false
        idlInstructionPlanValue = null
        idlInstructionPlanError = ""
        clearIdlInstructionPreviewArtifacts()
        releaseIdlInstructionPreviewBusy()
    }

    function dismissIdlInstructionReceipt() {
        clearIdlInstructionReceiptArtifacts()
    }

    function clearIdlInstructionReceiptArtifacts() {
        idlInstructionReceipt = null
        idlInstructionReceiptTarget = null
        idlInstructionReceiptTraceInput = null
    }

    function privateReceiptTraceInput(confirmation, receipt, backendTarget) {
        const confirmed = confirmation || ({})
        const report = receipt || ({})
        const entry = confirmed.entry || ({})
        const request = confirmed.request || ({})
        const context = confirmed.target && confirmed.target.context
            ? confirmed.target.context : null
        const transactionId = String(report.tx_hash || "").trim()
        const programId = String(report.program_id_hex || "").trim().toLowerCase()
        const entryProgramId = String(entry.programIdHex || "").trim().toLowerCase()
        const requestProgramId = String(request.programIdHex || "").trim().toLowerCase()
        const idlJson = String(request.idlJson || "")
        const words = normalizedInstructionWords(report.instruction_words)
        const accountIds = normalizedReceiptAccountIds(report.accounts)
        if (String(report.status || "") !== "submitted"
                || String(report.mode || "") !== "private"
                || transactionId.length === 0 || programId.length === 0
                || entryProgramId.length === 0 || requestProgramId.length === 0
                || entryProgramId !== programId || requestProgramId !== programId
                || String(entry.key || "").length === 0 || idlJson.length === 0
                || words.length === 0 || accountIds === null
                || !receiptTargetMatchesContext(backendTarget, context)) {
            return null
        }
        return frozenValue({
            txHash: transactionId,
            mode: "private",
            target: backendTarget,
            context: context,
            idlKey: String(entry.key),
            idlJson: idlJson,
            programIdHex: programId,
            instructionWords: words,
            accountIds: accountIds
        })
    }

    function receiptTargetMatchesContext(target, context) {
        const expected = context || ({})
        const actual = target || ({})
        return instructionNetworkScopeKey(actual.network_scope)
                === instructionNetworkScopeKey(expected.network_scope)
            && String(actual.channel_id || "") === String(expected.channel_id || "")
            && String(actual.source_id || "")
                === String(expected.selected_sequencer_source_id || "")
            && Number(actual.source_config_revision || 0)
                === Number(expected.source_config_revision || 0)
            && Number(actual.context_revision || 0)
                === Number(expected.context_revision || 0)
    }

    function instructionNetworkScopeKey(scope) {
        const value = scope || ({})
        const kind = String(value.kind || "")
        if (kind === "genesis_id") {
            return kind + ":" + String(value.genesis_id || "")
        }
        if (kind === "finalized_anchor") {
            return kind + ":" + String(value.genesis_time || "")
                + ":" + String(value.block_slot === undefined ? "" : value.block_slot)
                + ":" + String(value.block_id || "")
                + ":" + String(value.parent_id || "")
        }
        return JSON.stringify(value)
    }

    function normalizedInstructionWords(value) {
        if (!Array.isArray(value) || value.length === 0) {
            return []
        }
        const words = []
        for (let index = 0; index < value.length; ++index) {
            const word = Number(value[index])
            if (!Number.isFinite(word) || word < 0 || word > 4294967295
                    || Math.floor(word) !== word) {
                return []
            }
            words.push(word)
        }
        return words
    }

    function normalizedReceiptAccountIds(value) {
        if (!Array.isArray(value)) {
            return null
        }
        const accountIds = []
        for (let index = 0; index < value.length; ++index) {
            const accountId = String(value[index] && value[index].account_id || "").trim()
            if (accountId.length === 0) {
                return null
            }
            accountIds.push(accountId)
        }
        return accountIds
    }

    function clearIdlInstructionPreviewArtifacts() {
        idlInstructionPreviewValue = null
        idlInstructionError = ""
        idlInstructionFrozenEntry = null
        idlInstructionFrozenRequest = null
        idlInstructionFrozenPreview = null
        idlInstructionFrozenTarget = null
        idlInstructionFrozenTargetDisplay = null
        idlInstructionFrozenDraftRevision = 0
        idlInstructionFrozenArtifact = null
        idlInstructionConfirmation = null
    }

    function acquireIdlInstructionBusy(lane, ticket) {
        idlInstructionBusyLane = String(lane || "")
        idlInstructionBusyTicket = Number(ticket || 0)
        setBusy(true)
    }

    function releaseIdlInstructionBusy(lane, ticket) {
        if (idlInstructionBusyLane !== String(lane || "")
                || idlInstructionBusyTicket !== Number(ticket || 0)) {
            return false
        }
        idlInstructionBusyLane = ""
        idlInstructionBusyTicket = 0
        setBusy(false)
        return true
    }

    function releaseIdlInstructionPreviewBusy() {
        if (idlInstructionBusyLane !== "preview") {
            return false
        }
        idlInstructionBusyLane = ""
        idlInstructionBusyTicket = 0
        setBusy(false)
        return true
    }

    function frozenValue(value) {
        if (value === undefined || value === null) {
            return null
        }
        return JSON.parse(JSON.stringify(value))
    }

    function jsonEqual(left, right) {
        return JSON.stringify(left) === JSON.stringify(right)
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
