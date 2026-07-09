.pragma library

.import "ProgramContextPresentation.js" as ProgramContextPresentation
.import "../../utils/UiFormat.js" as UiFormat

function activeTabLabel(page) {
    if (page.model.programTab === "programIds") {
        return qsTr("Known IDs")
    }
    if (page.model.programTab === "binaries") {
        return qsTr("Binaries")
    }
    if (page.model.programTab === "sharing") {
        return qsTr("Sharing")
    }
    if (page.model.programTab === "events") {
        return qsTr("Events")
    }
    return qsTr("IDLs")
}

function activeTabDelta(page) {
    if (page.model.programTab === "programIds") {
        return qsTr("Static table")
    }
    if (page.model.programTab === "binaries") {
        return qsTr("File inspection")
    }
    if (page.model.programTab === "sharing") {
        return qsTr("Shared IDLs")
    }
    if (page.model.programTab === "events") {
        return qsTr("Event decode")
    }
    return qsTr("Registry")
}

function activeTabMessage(page) {
    if (page.model.programTab === "programIds") {
        return qsTr("Load the sequencer known-program table before binding local IDLs or binaries.")
    }
    if (page.model.programTab === "binaries") {
        return qsTr("Inspect compiled program bytecode, then deploy it with the configured local wallet.")
    }
    if (page.model.programTab === "sharing") {
        return page.sharedPolicyText()
    }
    if (page.model.programTab === "events") {
        return qsTr("Decode event payloads with a user-provided IDL. Program-specific decoding stays local to the supplied IDL.")
    }
    return qsTr("Save local IDLs, summarize their instruction/account shape, or load program IDs from the sequencer.")
}

function sharedPolicyText(page) {
    if (page.model.sharedIdlPolicy === "disabled") {
        return qsTr("Shared account IDLs are ignored.")
    }
    if (page.model.sharedIdlPolicy === "autoRegister") {
        return qsTr("Verified shared IDLs are saved to the local registry with shared source metadata.")
    }
    if (page.model.sharedIdlPolicy === "sessionOnly") {
        return qsTr("Verified shared IDLs are usable for this session without saving them.")
    }
    return qsTr("Verified shared IDLs are shown as suggestions; local IDLs stay preferred.")
}

function validProgramId(page, value) {
    const text = String(value || "").trim()
    return text.length > 0 && page.model.canonicalProgramIdHex(text).length > 0
}

function lastResultText(page) {
    if (!page.hasResponse) {
        return qsTr("Idle")
    }
    return page.model.resultIsError ? qsTr("Error") : qsTr("OK")
}

function lastResultDelta(page) {
    if (!page.hasResponse) {
        return qsTr("No output")
    }
    return page.model.resultTitle.length ? page.model.resultTitle : qsTr("Program call")
}

function lastResultColor(page) {
    if (!page.hasResponse) {
        return page.theme.textMuted
    }
    return page.model.resultIsError ? page.theme.warning : page.theme.success
}

function responsePayloadText(page) {
    const value = page.responseValue
    if (value === null || value === undefined) {
        return "-"
    }
    if (Array.isArray(value)) {
        return page.numberText(value.length)
    }
    if (typeof value === "object") {
        return page.numberText(Object.keys(value).length)
    }
    return page.valueText(value)
}

function responseKindText(page) {
    const value = page.responseValue
    if (Array.isArray(value)) {
        return qsTr("Array items")
    }
    if (value && typeof value === "object") {
        return qsTr("Object fields")
    }
    return qsTr("Scalar value")
}

function responseIdlName(page) {
    const value = page.responseValue
    if (value && typeof value === "object" && value.name !== undefined) {
        return page.valueText(value.name)
    }
    return qsTr("IDL summary")
}

function responseProgramText(page) {
    const value = page.responseValue
    if (Array.isArray(value)) {
        return page.numberText(value.length)
    }
    if (ProgramContextPresentation.isProgramContext(value)) {
        return page.shortHash(value.program_id_base58 || value.program_id_hex || value.program_id)
    }
    if (isProgramFile(value)) {
        return page.shortHash(value.program_id_hex)
    }
    return "-"
}

function responseProgramDelta(page) {
    const value = page.responseValue
    if (Array.isArray(value)) {
        return qsTr("Known program IDs")
    }
    if (ProgramContextPresentation.isProgramContext(value)) {
        return value.in_chain ? qsTr("verified in chain") : qsTr("not verified")
    }
    if (isProgramFile(value)) {
        return qsTr("%1 bytes").arg(page.numberText(value.bytecode_len))
    }
    return qsTr("Sequencer")
}

function programRows(page) {
    return Array.isArray(page.responseValue) ? page.responseValue : []
}

function programTableRows(page) {
    return programRows(page).map(function (row) {
        const hex = String(row.hex || "")
        const base58 = String(row.base58 || "")
        return {
            label: String(row.label || "-"),
            hex: hex,
            base58: base58,
            programIdText: base58.length ? base58 : hex,
            knownIdl: knownIdlText(page, hex)
        }
    })
}

function isProgramContext(value) {
    return ProgramContextPresentation.isProgramContext(value)
}

function programContextRows(page) {
    return ProgramContextPresentation.rows(page, page.responseValue || {})
}

function programVerificationText(value) {
    return ProgramContextPresentation.verificationText(value)
}

function programHexText(value) {
    return ProgramContextPresentation.programHexText(value)
}

function programContextIdlRows(page) {
    return ProgramContextPresentation.idlRows(page, page.responseValue || {})
}

function programContextTransactionRows(page) {
    return ProgramContextPresentation.transactionRows(page, page.responseValue || {})
}

function programContextAccount(page) {
    return ProgramContextPresentation.account(page.responseValue || {})
}

function knownIdlText(page, programId) {
    const entries = page.model.idlEntriesForProgram(programId)
    if (entries.length > 0) {
        return entries[0].name || qsTr("registered")
    }
    return qsTr("none")
}

function isIdlReport(value) {
    return value && typeof value === "object" && !Array.isArray(value)
        && value.instructions !== undefined
        && value.accounts !== undefined
        && value.counts !== undefined
}

function isProgramFile(value) {
    return value && typeof value === "object" && !Array.isArray(value)
        && value.program_id_hex !== undefined
        && value.deployment_tx_hash !== undefined
}

function idlCount(page, key) {
    const value = page.responseValue
    if (isIdlReport(value) && value.counts && value.counts[key] !== undefined) {
        return Number(value.counts[key] || 0)
    }
    return 0
}

function idlInstructionRows(page) {
    const value = page.responseValue
    const instructions = isIdlReport(value) && Array.isArray(value.instructions) ? value.instructions : []
    return instructions.slice(0, 6).map(function (item) {
        const args = Array.isArray(item.args) ? item.args.length : 0
        const accounts = Array.isArray(item.accounts) ? item.accounts.length : 0
        return {
            title: page.valueText(item.name),
            detail: qsTr("%1 instruction account role(s), %2 arg(s)").arg(page.numberText(accounts)).arg(page.numberText(args))
        }
    })
}

function idlAccountRows(page) {
    const value = page.responseValue
    const accounts = isIdlReport(value) && Array.isArray(value.accounts) ? value.accounts : []
    return accounts.slice(0, 6).map(function (item) {
        return {
            title: page.valueText(item.name),
            detail: page.valueText(item.type_label)
        }
    })
}

function idlWarningRows(page) {
    const value = page.responseValue
    const warnings = isIdlReport(value) && Array.isArray(value.warnings) ? value.warnings : []
    return warnings.slice(0, 4).map(function (item) {
        return {
            title: qsTr("Warning"),
            detail: page.valueText(item)
        }
    })
}

function programFileRows(page) {
    const value = page.responseValue || {}
    if (!isProgramFile(value)) {
        return []
    }
    const rows = [
        { label: qsTr("Path"), value: page.valueText(value.path), linkKind: "" },
        { label: qsTr("Bytecode"), value: qsTr("%1 bytes").arg(page.numberText(value.bytecode_len)), linkKind: "" },
        { label: qsTr("Program ID (0x)"), value: page.valueText(value.program_id_hex), linkKind: "program" },
        { label: qsTr("Program ID"), value: page.valueText(value.program_id_base58), linkKind: "" },
        { label: qsTr("Deployment tx"), value: page.valueText(value.deployment_tx_hash), linkKind: "transaction" }
    ]
    if (String(value.source || "") === "local_wallet_cli") {
        rows.unshift({ label: qsTr("Deploy status"), value: page.valueText(value.status), linkKind: "" })
        rows.push({ label: qsTr("Wallet command"), value: page.valueText(value.command), linkKind: "" })
        rows.push({ label: qsTr("Wallet home"), value: page.valueText(value.wallet_home_source), linkKind: "" })
        rows.push({ label: qsTr("Submitted at"), value: page.valueText(value.submitted_at), linkKind: "" })
        rows.push({ label: qsTr("Exit status"), value: page.valueText(value.exit_status), linkKind: "" })
        if (String(value.stdout || "").length > 0) {
            rows.push({ label: qsTr("stdout"), value: String(value.stdout || ""), linkKind: "" })
        }
        if (String(value.stderr || "").length > 0) {
            rows.push({ label: qsTr("stderr"), value: String(value.stderr || ""), linkKind: "" })
        }
    }
    return rows
}

function idlFieldCount(json) {
    try {
        const parsed = JSON.parse(json || "{}")
        return parsed && typeof parsed === "object" ? Object.keys(parsed).length : 0
    } catch (error) {
        return 0
    }
}

function endpointLabel(value) {
    const text = String(value || "")
    if (!text.length) {
        return "-"
    }
    if (text.indexOf("127.0.0.1") >= 0 || text.indexOf("localhost") >= 0) {
        return qsTr("Local")
    }
    if (text.indexOf("testnet") >= 0) {
        return qsTr("Testnet")
    }
    return qsTr("Custom")
}

function shortEndpoint(value) {
    const text = String(value || "")
    if (!text.length) {
        return qsTr("Not configured")
    }
    return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
}

function shortHash(value) {
    const text = String(value || "")
    if (text.length <= 16) {
        return text.length ? text : "-"
    }
    return text.slice(0, 8) + "..." + text.slice(-6)
}

function shortPath(value) {
    const text = String(value || "").trim()
    if (!text.length) {
        return qsTr("the selected binary")
    }
    if (text.length <= 48) {
        return text
    }
    return "..." + text.slice(-45)
}

function localPathFromFileUrl(fileUrl) {
    const text = String(fileUrl || "")
    if (!text.length) {
        return ""
    }
    if (text.indexOf("file://") === 0) {
        let path = decodeURIComponent(text.slice(7))
        if (/^\/[A-Za-z]:\//.test(path)) {
            path = path.slice(1)
        }
        return path
    }
    return text
}

function valueText(value) {
    return UiFormat.valueText(value, {
        emptyText: "-",
        objectMode: "json"
    })
}

function numberText(value) {
    return UiFormat.numberText(value, {
        emptyText: "-",
        coerceNumericStrings: true
    })
}
