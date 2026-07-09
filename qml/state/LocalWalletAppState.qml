import QtQml
import "../utils/UiFormat.js" as UiFormat
import "ConfirmationPolicy.js" as ConfirmationPolicy

WalletState {
    id: root

    property var gateway: null

    function loadPersisted(value) {
        load(value)
    }

    function savePersisted(networkProfile, prefersBasecamp) {
        if (!loaded || !gateway) {
            return
        }
        gateway.call("saveWalletState", [payload(networkProfile, prefersBasecamp === true)])
    }

    function detectProfile(saveDetected) {
        if (!gateway) {
            return false
        }
        const response = gateway.call("detectWalletProfile", [])
        if (!response.ok || !response.value || typeof response.value !== "object") {
            statusError = response && response.error ? response.error : qsTr("Wallet autodetect failed.")
            return false
        }

        const detectedBinary = String(response.value.wallet_binary || response.value.walletBinary || "")
        const detectedHome = String(response.value.wallet_home || response.value.walletHome || "")
        if (detectedBinary.length > 0) {
            binary = detectedBinary
        }
        if (detectedHome.length > 0) {
            home = detectedHome
        }
        clearStatus()
        if (saveDetected !== false) {
            savePersisted(gateway.networkProfile(), gateway.prefersBasecampModules())
        }
        return detectedBinary.length > 0 || detectedHome.length > 0
    }

    function checkProfile(showResult) {
        if (!gateway) {
            return null
        }
        statusError = ""
        gateway.setStatus(qsTr("Local wallet"))
        return gateway.request("localWalletProfileStatus", [profile(gateway.networkProfile(), gateway.prefersBasecampModules())], qsTr("Local wallet"), showResult === true, function (response) {
            if (response.ok) {
                status = response.value || null
                statusError = ""
                appendHistory(qsTr("Profile status"), String(response.value && response.value.status ? response.value.status : "ok"), String(response.value && response.value.detail ? response.value.detail : ""))
            } else {
                status = null
                statusError = response.error || qsTr("Profile status failed.")
                appendHistory(qsTr("Profile status"), "down", statusError)
            }
        })
    }

    function checkedProfile() {
        if (!gateway) {
            return { ok: false, detail: qsTr("Wallet gateway is not available.") }
        }
        const response = gateway.requestBlocking("localWalletProfileStatus", [profile(gateway.networkProfile(), gateway.prefersBasecampModules())], qsTr("Local wallet"), false)
        if (response.ok) {
            status = response.value || null
            statusError = ""
            const value = String(response.value && response.value.status ? response.value.status : "")
            return {
                ok: value === "ok",
                detail: String(response.value && response.value.detail ? response.value.detail : "")
            }
        }
        status = null
        statusError = response.error || qsTr("Profile status failed.")
        return {
            ok: false,
            detail: statusError
        }
    }

    function createAccount() {
        if (busyGuard(qsTr("Wallet account"))) {
            return null
        }
        if (!profileGuard(qsTr("Wallet account"), qsTr("Configure wallet binary and wallet home before creating an account."), "profiles")) {
            return null
        }
        const privacy = String(createPrivacy || "public").toLowerCase() === "private" ? "private" : "public"
        const label = String(createLabel || "").trim()

        return runRequest(qsTr("Wallet account"), "localWalletCreateAccount", [currentProfile(), privacy, label, ConfirmationPolicy.token("wallet-create-account")], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Create account"), "created", operationDetail(response.value, "command"))
                createLabel = ""
            } else {
                appendHistory(qsTr("Create account"), "down", response.error || qsTr("Account creation failed."))
            }
        })
    }

    function sendTransaction() {
        if (busyGuard(qsTr("Wallet send"))) {
            return null
        }
        if (!profileGuard(qsTr("Wallet send"), qsTr("Configure wallet binary and wallet home before sending a transaction."), "profiles")) {
            return null
        }
        const request = {
            from: String(sendFrom || "").trim(),
            to: String(sendTo || "").trim(),
            to_keys: String(sendToKeys || "").trim(),
            to_npk: String(sendToNpk || "").trim(),
            to_vpk: String(sendToVpk || "").trim(),
            to_identifier: String(sendToIdentifier || "").trim(),
            amount: String(sendAmount || "").trim()
        }
        if (!request.from.length || !request.amount.length) {
            gateway.setResult(qsTr("Wallet send"), qsTr("Sender and amount are required."), true, null)
            return null
        }
        if (!request.to.length && !request.to_keys.length && (!request.to_npk.length || !request.to_vpk.length)) {
            gateway.setResult(qsTr("Wallet send"), qsTr("Recipient account, keys file, or NPK/VPK pair is required."), true, null)
            return null
        }

        return runRequest(qsTr("Wallet send"), "localWalletSendTransaction", [currentProfile(), request, ConfirmationPolicy.token("wallet-send-transaction")], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Send transaction"), "submitted", operationDetail(response.value, "command"))
            } else {
                appendHistory(qsTr("Send transaction"), "down", response.error || qsTr("Wallet send failed."))
            }
        })
    }

    function readIncomingTransactions() {
        if (busyGuard(qsTr("Read incoming"))) {
            return null
        }
        if (!profileGuard(qsTr("Read incoming"), qsTr("Configure wallet binary and wallet home before reading incoming transactions."), "profiles")) {
            return null
        }

        return runRequest(qsTr("Read incoming"), "localWalletSyncPrivate", [currentProfile(), ConfirmationPolicy.token("wallet-sync-private")], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Read incoming"), "submitted", operationDetail(response.value, "privateSync"))
            } else {
                appendHistory(qsTr("Read incoming"), "down", response.error || qsTr("Incoming transaction read failed."))
            }
        })
    }

    function runCommand(commandArgs) {
        const args = Array.isArray(commandArgs) ? commandArgs : []
        if (busyGuard(qsTr("Wallet command"))) {
            return null
        }
        if (!profileGuard(qsTr("Wallet command"), qsTr("Configure wallet binary and wallet home before running wallet commands."), "profiles")) {
            return null
        }
        if (!args.length) {
            gateway.setResult(qsTr("Wallet command"), qsTr("Wallet command arguments are required."), true, null)
            return null
        }

        return runRequest(qsTr("Wallet command"), "localWalletCommand", [currentProfile(), args, ConfirmationPolicy.token("wallet-command")], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Wallet command"), "completed", operationDetail(response.value, "command"))
            } else {
                appendHistory(qsTr("Wallet command"), "down", response.error || qsTr("Wallet command failed."))
            }
        })
    }

    function syncPrivate() {
        if (busyGuard(qsTr("Private sync"))) {
            return null
        }
        if (!profileGuard(qsTr("Private sync"), "", "profiles")) {
            return null
        }

        return runRequest(qsTr("Private sync"), "localWalletSyncPrivate", [currentProfile(), ConfirmationPolicy.token("wallet-sync-private")], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Private sync"), "submitted", operationDetail(response.value, "privateSync"))
            } else {
                appendHistory(qsTr("Private sync"), "down", response.error || qsTr("Private sync failed."))
            }
        })
    }

    function queryAccounts(showResult) {
        if (busyGuard(qsTr("Wallet accounts"))) {
            return null
        }
        if (!profileConfigured()) {
            accountsError = qsTr("Configure wallet binary and wallet home, or check a profile that resolves $LEE_WALLET_HOME_DIR.")
            gateway.setResult(qsTr("Wallet accounts"), accountsError, true, null)
            return null
        }

        return runRequest(qsTr("Wallet accounts"), "localWalletAccounts", [currentProfile()], showResult === true, function (response) {
            if (response.ok) {
                accountsValue = response.value || null
                accountsError = ""
                appendHistory(qsTr("Wallet accounts"), "loaded", operationDetail(response.value, "accounts"))
            } else {
                accountsValue = null
                accountsError = response.error || qsTr("Wallet account list failed.")
                appendHistory(qsTr("Wallet accounts"), "down", accountsError)
            }
        })
    }

    function queryBedrockBalance() {
        const publicKey = String(publicKeyProbe || "").trim()
        if (!publicKey.length) {
            gateway.setBedrockWalletBalance(null, qsTr("Wallet public key is required."))
            return null
        }
        if (!isBedrockHexId(publicKey)) {
            gateway.setBedrockWalletBalance(null, qsTr("Wallet public key must be 64 hex characters."))
            return null
        }
        const tip = String(bedrockBalanceTip || "").trim()
        if (tip.length > 0 && !isBedrockHexId(tip)) {
            gateway.setBedrockWalletBalance(null, qsTr("Balance tip must be a 64-hex header id."))
            return null
        }
        gateway.setBedrockWalletBalance(null, "")
        gateway.setStatus(qsTr("Bedrock wallet"))
        return gateway.request("bedrockWalletBalance", [gateway.nodeUrl(), publicKey, tip], qsTr("Bedrock wallet"), false, function (response) {
            if (response.ok) {
                gateway.setBedrockWalletBalance(response.value, "")
                appendHistory(qsTr("Bedrock balance"), "ok", publicKey)
            } else {
                const error = response.error || qsTr("Balance query failed.")
                gateway.setBedrockWalletBalance(null, error)
                appendHistory(qsTr("Bedrock balance"), "down", error)
            }
        })
    }

    function operationDetail(value, fallback) {
        const report = value || {}
        const detail = String(report.operation_detail || report.operationDetail || "")
        if (detail.length > 0) {
            return detail
        }
        switch (String(fallback || "")) {
        case "privateSync":
            return privateSyncOperationDetail(report)
        case "accounts":
            return accountsOperationDetail(report)
        default:
            return commandOperationDetail(report)
        }
    }

    function accountsOperationDetail(value) {
        const report = value || {}
        const count = Array.isArray(report.accounts) ? report.accounts.length : 0
        return qsTr("%1 accounts").arg(count)
    }

    function commandOperationDetail(value) {
        const report = value || {}
        const tx = String(report.tx_hash || report.txHash || "")
        if (tx.length) {
            return qsTr("tx %1").arg(UiFormat.shortHash(tx))
        }
        const account = String(report.account_id || report.accountId || "")
        if (account.length) {
            return UiFormat.shortId(account)
        }
        const command = String(report.command || "")
        if (command.length) {
            return command
        }
        return String(report.status || qsTr("completed"))
    }

    function privateSyncOperationDetail(value) {
        const report = value || {}
        const status = String(report.status || "submitted")
        const home = String(report.wallet_home_source || "")
        return home.length ? qsTr("%1, home %2").arg(status).arg(home) : status
    }

    function isBedrockHexId(value) {
        return /^(0x)?[0-9a-fA-F]{64}$/.test(String(value || "").trim())
    }

    function busyGuard(title) {
        if (!gateway || gateway.busy() !== true) {
            return false
        }
        gateway.setResult(title, qsTr("Another inspection is already running."), true, null)
        return true
    }

    function profileGuard(title, message, tab) {
        if (profileConfigured()) {
            return true
        }
        if (gateway) {
            gateway.openLocalWallet("", tab || "profiles")
            if (String(message || "").length) {
                gateway.setResult(title, message, true, null)
            }
        }
        return false
    }

    function runRequest(title, method, args, showResult, callback, beforeStart) {
        if (!gateway) {
            return null
        }
        if (beforeStart) {
            beforeStart()
        }
        gateway.setBusy(true)
        gateway.setStatus(title)
        return gateway.request(method, args, title, showResult === true, function (response) {
            gateway.setBusy(false)
            callback(response)
        })
    }

    function appendHistory(label, statusText, detail) {
        const labelText = String(label || "")
        const statusValue = String(statusText || "")
        const detailText = String(detail || "")
        const record = appendOperation(labelText, statusValue, detailText)
        const historyStatus = operationStatus(statusValue)
        if (gateway) {
            let appendHistoryCallback = typeof gateway.appendOperationHistory === "function"
                ? gateway.appendOperationHistory
                : null
            if (!appendHistoryCallback && typeof gateway.appendRuntimeOperationHistory === "function") {
                appendHistoryCallback = gateway.appendRuntimeOperationHistory
            }
            if (!appendHistoryCallback && typeof gateway.appendNodeOperationHistory === "function") {
                appendHistoryCallback = gateway.appendNodeOperationHistory
            }
            if (appendHistoryCallback) {
                appendHistoryCallback({
                    domain: "wallet",
                    method: labelText,
                    status: historyStatus,
                    label: labelText,
                    result: {
                        status: record.status,
                        detail: record.detail
                    },
                    error: historyStatus === "failed" ? detailText : ""
                }, detailText)
            }
            savePersisted(gateway.networkProfile(), gateway.prefersBasecampModules())
        }
        return record
    }

    function currentProfile() {
        return profile(gateway.networkProfile(), gateway.prefersBasecampModules())
    }

    function homeFallbackLabel() {
        if (String(home || "").trim().length > 0) {
            return gateway ? gateway.redactedPath(home) : String(home || "")
        }
        const source = String(status && status.home_source ? status.home_source : "")
        if (source.length > 0 && source !== "none" && source !== "profile") {
            return qsTr("$%1").arg(source)
        }
        return qsTr("Not configured")
    }

    function homeSourceLabel() {
        if (String(home || "").trim().length > 0) {
            return qsTr("profile home")
        }
        const source = String(status && status.home_source ? status.home_source : "")
        if (source.length > 0 && source !== "none" && source !== "profile") {
            return qsTr("$%1").arg(source)
        }
        return qsTr("home not configured")
    }

    function binaryDisplayLabel() {
        return gateway ? gateway.redactedPath(binary) : String(binary || "")
    }

    function homeDisplayLabel() {
        return homeFallbackLabel()
    }
}
