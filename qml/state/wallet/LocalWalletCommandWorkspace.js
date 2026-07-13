function operationRows(model) {
    const rows = Array.isArray(model.localWalletOperations) ? model.localWalletOperations.slice() : []
    if (!rows.length) {
        return [{ time: "-", label: qsTr("No operations"), status: "-", detail: "-" }]
    }
    rows.reverse()
    return rows
}

function sendReady(model) {
    if (model.shell.busy || !model.walletProfileConfigured()) {
        return false
    }
    const from = String(model.walletSendFrom || "").trim()
    const amount = String(model.walletSendAmount || "").trim()
    const to = String(model.walletSendTo || "").trim()
    const keys = String(model.walletSendToKeys || "").trim()
    const npk = String(model.walletSendToNpk || "").trim()
    const vpk = String(model.walletSendToVpk || "").trim()
    return from.length > 0 && amount.length > 0 && (to.length > 0 || keys.length > 0 || (npk.length > 0 && vpk.length > 0))
}

function walletCommandArgs(model) {
    const parsed = parseWalletCommandLine(model.walletAdvancedCommand)
    return parsed === null ? [] : parsed
}

function advancedCommandError(model) {
    const parsed = parseWalletCommandLine(model.walletAdvancedCommand)
    if (parsed === null) {
        return qsTr("Close quoted argument before running.")
    }
    if (!parsed.length) {
        return qsTr("Wallet command arguments are required.")
    }
    return ""
}

function parseWalletCommandLine(value) {
    const text = String(value || "")
    const args = []
    let current = ""
    let quote = ""
    for (let i = 0; i < text.length; ++i) {
        const ch = text.charAt(i)
        if (ch === "\\") {
            const next = i + 1 < text.length ? text.charAt(i + 1) : ""
            if (next.length > 0) {
                if (quote.length > 0 && next === quote) {
                    current += next
                    ++i
                    continue
                }
                if (quote.length === 0 && (next === "\"" || next === "'" || /\s/.test(next))) {
                    current += next
                    ++i
                    continue
                }
            }
            current += ch
            continue
        }
        if (quote.length > 0) {
            if (ch === quote) {
                quote = ""
            } else {
                current += ch
            }
            continue
        }
        if (ch === "\"" || ch === "'") {
            quote = ch
            continue
        }
        if (/\s/.test(ch)) {
            if (current.length > 0) {
                args.push(current)
                current = ""
            }
            continue
        }
        current += ch
    }
    if (quote.length > 0) {
        return null
    }
    if (current.length > 0) {
        args.push(current)
    }
    if (args.length > 0 && args[0] === "wallet") {
        args.shift()
    }
    return args
}
