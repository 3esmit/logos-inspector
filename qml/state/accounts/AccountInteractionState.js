function accountFields(root, instruction) {
    const currentInstruction = instruction || root.interactionInstruction()
    const accounts = currentInstruction && Array.isArray(currentInstruction.accounts) ? currentInstruction.accounts : []
    const rows = []
    const seen = {}
    for (let i = 0; i < accounts.length; ++i) {
        const account = accounts[i] || {}
        if (account.pda !== undefined) {
            continue
        }
        const name = String(account.name || "")
        if (!name.length) {
            continue
        }
        const rest = account.rest === true
        const signer = account.signer === true
        rows.push({
            name: name,
            label: signer ? qsTr("%1 signer").arg(root.displayLabel(name)) : root.displayLabel(name),
            placeholder: rest ? qsTr("Public/<id>, Private/<id>") : qsTr("Public/<id> or Private/<id>"),
            required: !rest,
            rest: rest
        })
        seen[name] = true
    }
    for (let j = 0; j < accounts.length; ++j) {
        const pda = accounts[j] && accounts[j].pda ? accounts[j].pda : null
        const seeds = pda && Array.isArray(pda.seeds) ? pda.seeds : []
        for (let k = 0; k < seeds.length; ++k) {
            const seed = seeds[k] || {}
            const path = String(seed.path || "")
            if (String(seed.kind || "") === "account" && path.length > 0 && seen[path] !== true) {
                rows.push({
                    name: path,
                    label: qsTr("%1 seed").arg(root.displayLabel(path)),
                    placeholder: qsTr("Public/<id>"),
                    required: true,
                    rest: false
                })
                seen[path] = true
            }
        }
    }
    return rows
}

function argFields(root, instruction) {
    const currentInstruction = instruction || root.interactionInstruction()
    const args = currentInstruction && Array.isArray(currentInstruction.args) ? currentInstruction.args : []
    const rows = []
    for (let i = 0; i < args.length; ++i) {
        const arg = args[i] || {}
        const name = String(arg.name || "")
        if (!name.length) {
            continue
        }
        const typeLabel = typeLabelText(arg.type)
        rows.push({
            name: name,
            label: qsTr("%1 (%2)").arg(root.displayLabel(name)).arg(typeLabel),
            placeholder: placeholder(arg.type),
            required: true
        })
    }
    return rows
}

function typeLabelText(typeValue) {
    if (typeof typeValue === "string") {
        return typeValue
    }
    if (!typeValue || typeof typeValue !== "object") {
        return "value"
    }
    if (typeValue.array && Array.isArray(typeValue.array)) {
        const elem = typeLabelText(typeValue.array[0])
        const count = typeValue.array.length > 1 ? String(typeValue.array[1]) : "?"
        return "[" + elem + "; " + count + "]"
    }
    if (typeValue.vec !== undefined) {
        return "Vec<" + typeLabelText(typeValue.vec) + ">"
    }
    if (typeValue.option !== undefined) {
        return "Option<" + typeLabelText(typeValue.option) + ">"
    }
    if (typeValue.defined !== undefined) {
        return String(typeValue.defined || "defined")
    }
    return "value"
}

function placeholder(typeValue) {
    const label = typeLabelText(typeValue)
    if (label === "bool") {
        return qsTr("true or false")
    }
    if (label.indexOf("[u8;") === 0) {
        return qsTr("0x...")
    }
    if (label.indexOf("Vec<") === 0) {
        return qsTr("comma values")
    }
    return qsTr("value")
}

function fieldValue(root, kind, name) {
    const revision = root.interactionRevision
    const values = kind === "account" ? root.interactionAccountValues : root.interactionArgValues
    return String((values || {})[name] || "")
}

function copyMap(source) {
    const copy = {}
    const current = source || {}
    for (const key in current) {
        copy[key] = current[key]
    }
    return copy
}

function privateMode(root) {
    const values = root.interactionAccountValues || {}
    for (const key in values) {
        if (String(values[key] || "").trim().toLowerCase().indexOf("private/") === 0) {
            return true
        }
    }
    return false
}

function inputsComplete(root) {
    if (!root.canInteractWithIdl() || !root.interactionInstruction()) {
        return false
    }
    const accounts = accountFields(root)
    for (let i = 0; i < accounts.length; ++i) {
        if (accounts[i].required === true && !fieldValue(root, "account", accounts[i].name).trim().length) {
            return false
        }
    }
    const args = argFields(root)
    for (let j = 0; j < args.length; ++j) {
        if (args[j].required === true && !fieldValue(root, "arg", args[j].name).trim().length) {
            return false
        }
    }
    return !privateMode(root) || root.interactionProgramBinary.trim().length > 0
}

function request(root) {
    const entry = root.interactionIdlEntry() || {}
    const instruction = root.interactionInstruction() || {}
    return {
        idl_json: String(entry.json || ""),
        program_id_hex: String(entry.programIdHex || root.ownerProgramId()),
        program_binary: String(root.interactionProgramBinary || "").trim(),
        dependency_binaries: [],
        instruction: String(instruction.name || ""),
        accounts: copyMap(root.interactionAccountValues),
        args: copyMap(root.interactionArgValues)
    }
}

function previewText(root) {
    const report = root.model.idlInstructionPreviewValue
    const instruction = root.interactionInstruction()
    if (!report || !instruction || String(report.instruction || "") !== String(instruction.name || "")) {
        return ""
    }
    const tx = String(report.tx_hash || report.txHash || "")
    if (tx.length > 0) {
        return qsTr("%1 transaction %2").arg(String(report.mode || "submitted")).arg(root.shortId(tx))
    }
    const words = Array.isArray(report.instruction_words) ? report.instruction_words.length : 0
    return qsTr("%1 preview, %2 word(s)").arg(String(report.mode || "public")).arg(words)
}

function confirmMessage(root) {
    const instruction = root.interactionInstruction()
    const name = instruction ? String(instruction.name || qsTr("instruction")) : qsTr("instruction")
    if (privateMode(root)) {
        return qsTr("Submit private transaction for %1. Wallet will execute and prove locally.").arg(name)
    }
    return qsTr("Submit public transaction for %1.").arg(name)
}

function decodedRows(root) {
    const decode = root.activeDecode
    if (!decode) {
        return []
    }

    const rows = []
    if (decode.remaining_data_hex) {
        rows.push({ label: qsTr("Remaining data"), value: root.shortLong(decode.remaining_data_hex), monospace: true })
    }

    const decoded = Array.isArray(decode.rows) ? decode.rows : []
    for (let i = 0; i < decoded.length; ++i) {
        const row = decoded[i]
        const rawValue = root.valueText(row.value)
        const kind = root.referenceKind(row.path, row.value)
        const useAlias = (kind === "account" || kind === "program") && root.isNullAddress(rawValue, rawValue)
        const aliased = useAlias ? root.addressLabel(rawValue, "") : rawValue
        rows.push({
            label: root.displayLabel(row.path || qsTr("Field")),
            value: aliased,
            monospace: true,
            linkKind: kind,
            linkValue: useAlias ? root.addressCopyValue(rawValue, rawValue) : rawValue,
            tooltipText: useAlias ? root.addressCopyValue(rawValue, rawValue) : ""
        })
    }
    return rows
}

function relatedRows(root) {
    const revision = root.relatedTransactionDecodeRevision
    const rows = root.detail ? root.detail.related_transactions : []
    if (!rows.length) {
        return [{
            hashText: qsTr("No related transactions loaded"),
            direction: "-",
            instruction: "-",
            programText: "-",
            accounts: "-",
            txHash: "",
            programId: ""
        }]
    }
    return rows.map(function (tx) {
        const txHash = String(tx.hash || "")
        const programId = String(tx.program_id_hex || "")
        const decoded = tx.decoded_instruction || root.relatedTransactionDecode(txHash)
        return {
            hashText: root.shortId(txHash),
            direction: root.directionText(tx.direction),
            instruction: decoded ? String(decoded.instruction || "-") : String(tx.kind || "-"),
            programText: decoded && decoded.idl_name ? String(decoded.idl_name) : root.shortId(programId),
            accounts: root.numberText(Array.isArray(tx.account_ids) ? tx.account_ids.length : 0),
            txHash: txHash,
            programId: programId
        }
    })
}

function relatedTransactionSummary(tx) {
    if (!tx || typeof tx !== "object") {
        return null
    }
    const words = Array.isArray(tx.instruction_data) ? tx.instruction_data : []
    if (String(tx.kind || "") !== "Public" || words.length === 0) {
        return null
    }
    return {
        hash: String(tx.hash || ""),
        kind: String(tx.kind || ""),
        program_id_hex: String(tx.program_id_hex || ""),
        account_ids: Array.isArray(tx.account_ids) ? tx.account_ids : [],
        nonces: Array.isArray(tx.nonces) ? tx.nonces : [],
        instruction_data: words,
        bytecode_len: tx.bytecode_len === undefined ? null : tx.bytecode_len,
        raw_signature_valid: null,
        message_prehash: null,
        prehash_signature_valid: null
    }
}
