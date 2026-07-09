.pragma library

function isProgramContext(value) {
    return value && typeof value === "object" && !Array.isArray(value)
        && value.type === "program"
        && value.program_id !== undefined
}

function responseProgramText(page, value) {
    if (Array.isArray(value)) {
        return page.numberText(value.length)
    }
    if (isProgramContext(value)) {
        return page.shortHash(value.program_id_base58 || value.program_id_hex || value.program_id)
    }
    if (page.isProgramFile(value)) {
        return page.shortHash(value.program_id_hex)
    }
    return "-"
}

function responseProgramDelta(page, value) {
    if (Array.isArray(value)) {
        return qsTr("Known program IDs")
    }
    if (isProgramContext(value)) {
        return value.in_chain ? qsTr("verified in chain") : qsTr("not verified")
    }
    if (page.isProgramFile(value)) {
        return qsTr("%1 bytes").arg(page.numberText(value.bytecode_len))
    }
    return qsTr("Sequencer")
}

function rows(page, value) {
    if (!isProgramContext(value)) {
        return []
    }
    const programId = page.valueText(value.program_id)
    const programHex = programHexText(value.program_id_hex)
    const programBase58 = page.valueText(value.program_id_base58)
    const accountLookup = programBase58 !== "-" ? programBase58 : programHex
    const verified = value.in_chain === true
    const result = [
        { label: qsTr("Known program"), value: verificationText(value), linkKind: "" },
        { label: qsTr("Program ID"), value: programBase58 !== "-" ? programBase58 : programId, linkKind: verified ? "program" : "" },
        { label: qsTr("Program ID (0x)"), value: programHex, linkKind: verified ? "program" : "" },
        { label: qsTr("Inspect as account"), value: accountLookup, linkKind: accountLookup !== "-" ? "account" : "" },
        { label: qsTr("Sequencer label"), value: page.valueText(value.known_label), linkKind: "" }
    ]
    if (value.verification_detail !== undefined && String(value.verification_detail || "").length > 0) {
        result.push({ label: qsTr("Verification error"), value: String(value.verification_detail || ""), linkKind: "" })
    }
    return result
}

function verificationText(value) {
    if (!isProgramContext(value)) {
        return "-"
    }
    if (value.in_chain === true) {
        return qsTr("yes")
    }
    if (String(value.verification || "") === "unavailable") {
        return qsTr("verification unavailable")
    }
    return qsTr("not in getProgramIds")
}

function programHexText(value) {
    const text = String(value || "").replace(/^0x/i, "")
    return text.length ? "0x" + text : "-"
}

function idlRows(page, value) {
    const entries = isProgramContext(value) && Array.isArray(value.idls) ? value.idls : []
    return entries.map(function (entry) {
        const json = String(entry.json || "")
        return {
            title: page.valueText(entry.name || entry.programId || entry.programIdHex),
            detail: qsTr("%1 field(s), program %2").arg(page.numberText(page.idlFieldCount(json))).arg(page.shortHash(entry.programId || entry.programIdHex))
        }
    })
}

function transactionRows(page, value) {
    const rows = isProgramContext(value) && Array.isArray(value.recent_transactions) ? value.recent_transactions : []
    return rows.slice(0, 8).map(function (tx) {
        return {
            title: page.shortHash(tx.hash),
            detail: qsTr("block %1, %2, %3 word(s)").arg(page.valueText(tx.block_id)).arg(page.valueText(tx.kind)).arg(page.numberText(tx.ops))
        }
    })
}

function account(value) {
    return isProgramContext(value) && value.account && typeof value.account === "object" ? value.account : null
}
