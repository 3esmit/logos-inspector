import QtQml

QtObject {
    id: root

    property var bridge: null
    readonly property string providerModule: "lez_core"
    readonly property string providerLabel: qsTr("LEZ Core wallet")

    property bool busy: false
    property string availability: "unknown"
    property string availabilityDetail: ""
    property string version: ""
    property var accounts: []
    property string error: ""
    property string notice: ""
    property var transferResult: null
    property var operations: []
    property int requestEpoch: 0

    function refresh(preserveNotice) {
        if (busy) {
            return false
        }
        const epoch = requestEpoch + 1
        requestEpoch = epoch
        busy = true
        error = ""
        availability = "checking"
        availabilityDetail = ""
        return invoke("version", [], function(response) {
            if (epoch !== requestEpoch) {
                return
            }
            if (!response.ok) {
                busy = false
                availability = "unavailable"
                availabilityDetail = responseMessage(response)
                error = availabilityDetail
                appendOperation(qsTr("Check LEZ Core"), qsTr("failed"), availabilityDetail, "error")
                return
            }
            version = scalarText(response.value)
            availability = "available"
            availabilityDetail = version.length > 0
                ? qsTr("Version %1").arg(version)
                : qsTr("Module responded")
            loadAccounts(epoch, preserveNotice === true)
        })
    }

    function loadAccounts(epoch, preserveNotice) {
        return invoke("list_accounts", [], function(response) {
            if (epoch !== requestEpoch) {
                return
            }
            if (!response.ok) {
                busy = false
                accounts = []
                error = responseMessage(response)
                notice = ""
                appendOperation(qsTr("Load accounts"), qsTr("failed"), error, "error")
                return
            }
            accounts = normalizedAccounts(response.value)
            if (accounts.length === 0) {
                const emptyNotice = qsTr("No accounts are available. Open or create an account in the official LEZ Wallet UI, then refresh.")
                if (!preserveNotice) {
                    notice = emptyNotice
                }
                appendOperation(qsTr("Load accounts"), qsTr("completed"), emptyNotice, "neutral")
                busy = false
                return
            }
            const accountNotice = qsTr("Loaded %1 wallet accounts.").arg(accounts.length)
            if (!preserveNotice) {
                notice = accountNotice
            }
            appendOperation(qsTr("Load accounts"), qsTr("completed"), accountNotice, "success")
            loadBalanceAt(epoch, 0)
        })
    }

    function loadBalanceAt(epoch, index) {
        if (epoch !== requestEpoch) {
            return false
        }
        if (index >= accounts.length) {
            busy = false
            return true
        }
        const account = accounts[index]
        return invoke("get_balance", [account.accountId, account.isPublic], function(response) {
            if (epoch !== requestEpoch) {
                return
            }
            const next = accounts.slice()
            const current = next[index]
            if (response.ok) {
                current.balance = scalarText(response.value)
                current.balanceError = ""
            } else {
                current.balance = ""
                current.balanceError = responseMessage(response)
            }
            next[index] = current
            accounts = next
            loadBalanceAt(epoch, index + 1)
        })
    }

    function submitPublicTransfer(from, to, amount) {
        if (busy) {
            return false
        }
        const draft = publicTransferDraft(from, to, amount)
        if (!draft.ok) {
            error = draft.error
            notice = ""
            return false
        }
        const epoch = requestEpoch + 1
        requestEpoch = epoch
        busy = true
        error = ""
        notice = ""
        transferResult = null
        return invoke("transfer_public", [draft.from, draft.to, draft.amountLe16Hex], function(response) {
            if (epoch !== requestEpoch) {
                return
            }
            busy = false
            if (!response.ok) {
                error = responseMessage(response)
                appendOperation(qsTr("Public transfer"), qsTr("failed"), error, "error")
                return
            }
            const result = objectValue(response.value)
            transferResult = result
            if (result.success !== true) {
                error = String(result.error || qsTr("LEZ Core rejected the public transfer."))
                appendOperation(qsTr("Public transfer"), qsTr("failed"), error, "error")
                return
            }
            const transactionId = String(result.tx_hash || result.txHash || "")
            notice = transactionId.length > 0
                ? qsTr("Public transfer submitted: %1").arg(transactionId)
                : qsTr("Public transfer submitted.")
            appendOperation(qsTr("Public transfer"), qsTr("submitted"), transactionId.length > 0 ? transactionId : notice, "success")
            refresh(true)
        })
    }

    function publicTransferDraft(from, to, amount) {
        const source = normalizedAccountId(from)
        if (source.length === 0) {
            return { ok: false, error: qsTr("Source account must be a 32-byte hexadecimal account ID.") }
        }
        const recipient = normalizedAccountId(to)
        if (recipient.length === 0) {
            return { ok: false, error: qsTr("Recipient account must be a 32-byte hexadecimal account ID.") }
        }
        const amountLe16Hex = decimalToLe16Hex(amount)
        if (amountLe16Hex.length === 0) {
            return { ok: false, error: qsTr("Amount must be a positive unsigned 128-bit atomic-unit value.") }
        }
        return {
            ok: true,
            from: source,
            to: recipient,
            amountLe16Hex: amountLe16Hex
        }
    }

    function transferReady(from, to, amount) {
        return publicTransferDraft(from, to, amount).ok === true
    }

    function normalizedAccountId(value) {
        const accountId = String(value || "").trim().replace(/^0x/i, "").toLowerCase()
        return /^[0-9a-f]{64}$/.test(accountId) ? accountId : ""
    }

    function decimalToLe16Hex(value) {
        let remaining = String(value || "").trim()
        if (!/^[0-9]+$/.test(remaining)) {
            return ""
        }
        remaining = remaining.replace(/^0+/, "")
        if (remaining.length === 0) {
            return ""
        }
        const bytes = []
        while (remaining !== "0") {
            let quotient = ""
            let remainder = 0
            for (let index = 0; index < remaining.length; ++index) {
                const current = remainder * 10 + remaining.charCodeAt(index) - 48
                const digit = Math.floor(current / 256)
                remainder = current % 256
                if (quotient.length > 0 || digit > 0) {
                    quotient += String.fromCharCode(48 + digit)
                }
            }
            bytes.push(remainder)
            if (bytes.length > 16) {
                return ""
            }
            remaining = quotient.length > 0 ? quotient : "0"
        }
        while (bytes.length < 16) {
            bytes.push(0)
        }
        let result = ""
        for (let byteIndex = 0; byteIndex < bytes.length; ++byteIndex) {
            result += bytes[byteIndex].toString(16).padStart(2, "0")
        }
        return result
    }

    function accountRows() {
        if (!Array.isArray(accounts) || accounts.length === 0) {
            return [{
                accountId: "",
                cells: [
                    { text: qsTr("No wallet accounts loaded"), width: 360, fill: true, monospace: false },
                    { text: "-", width: 120, monospace: false },
                    { text: "-", width: 180, monospace: false }
                ]
            }]
        }
        return accounts.map(function(account) {
            const balance = String(account.balance || "")
            const balanceError = String(account.balanceError || "")
            return {
                accountId: account.accountId,
                cells: [
                    { text: account.accountId, width: 360, fill: true, link: true, copyText: account.accountId },
                    { text: account.isPublic ? qsTr("Public") : qsTr("Private"), width: 120, monospace: false },
                    { text: balance.length > 0 ? balance : (balanceError.length > 0 ? balanceError : qsTr("Loading")), width: 180, monospace: balance.length > 0, tone: balanceError.length > 0 ? "warning" : "neutral" }
                ]
            }
        })
    }

    function operationRows() {
        if (!Array.isArray(operations) || operations.length === 0) {
            return [{
                cells: [
                    { text: qsTr("No LEZ Core operations yet"), width: 180, monospace: false },
                    { text: "-", width: 120, monospace: false },
                    { text: "-", width: 360, fill: true, monospace: false }
                ]
            }]
        }
        return operations.map(function(operation) {
            return {
                cells: [
                    { text: String(operation.label || "-"), width: 180, monospace: false },
                    { text: String(operation.status || "-"), width: 120, monospace: false, tone: String(operation.tone || "neutral") },
                    { text: String(operation.detail || "-"), width: 360, fill: true, monospace: false }
                ]
            }
        })
    }

    function availabilityLabel() {
        switch (availability) {
        case "checking":
            return qsTr("Checking")
        case "available":
            return qsTr("Available")
        case "unavailable":
            return qsTr("Unavailable")
        default:
            return qsTr("Not checked")
        }
    }

    function availabilityTone() {
        if (availability === "available") {
            return "success"
        }
        if (availability === "unavailable") {
            return "error"
        }
        if (availability === "checking") {
            return "info"
        }
        return "neutral"
    }

    function appendOperation(label, status, detail, tone) {
        const rows = Array.isArray(operations) ? operations.slice(-24) : []
        rows.push({
            label: String(label || ""),
            status: String(status || ""),
            detail: String(detail || ""),
            tone: String(tone || "neutral")
        })
        operations = rows
    }

    function normalizedAccounts(value) {
        const parsed = objectValue(value)
        const rows = Array.isArray(parsed) ? parsed : (parsed && Array.isArray(parsed.accounts) ? parsed.accounts : [])
        return rows.map(function(row) {
            const source = row && typeof row === "object" ? row : ({})
            return {
                accountId: normalizedAccountId(source.account_id || source.accountId || source.id || ""),
                isPublic: source.is_public === true || source.isPublic === true,
                balance: "",
                balanceError: ""
            }
        }).filter(function(row) {
            return row.accountId.length > 0
        })
    }

    function objectValue(value) {
        if (value && typeof value === "object") {
            return value
        }
        if (typeof value !== "string") {
            return ({})
        }
        try {
            const parsed = JSON.parse(value)
            return parsed === null ? ({}) : parsed
        } catch (ignored) {
            return ({})
        }
    }

    function scalarText(value) {
        const parsed = typeof value === "string" ? objectValue(value) : value
        if (typeof parsed === "string" || typeof parsed === "number" || typeof parsed === "boolean") {
            return String(parsed)
        }
        return typeof value === "string" ? value : ""
    }

    function responseMessage(response) {
        return String(response && response.error ? response.error : qsTr("LEZ Core did not return a usable response."))
    }

    function invoke(method, args, callback) {
        if (!bridge || typeof bridge.callModuleAsync !== "function") {
            callback({ ok: false, value: null, error: qsTr("Basecamp module bridge is unavailable.") })
            return false
        }
        try {
            bridge.callModuleAsync(providerModule, method, args || [], callback)
            return true
        } catch (exception) {
            callback({ ok: false, value: null, error: String(exception) })
            return false
        }
    }
}
