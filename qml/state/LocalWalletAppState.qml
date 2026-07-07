import QtQml
import "../utils/UiFormat.js" as UiFormat

WalletState {
    id: root

    property var gateway: null

    function loadPersisted(value) {
        load(value)
    }

    function savePersisted(networkProfile) {
        if (!loaded || !gateway) {
            return
        }
        gateway.call("saveWalletState", [payload(networkProfile)])
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
            savePersisted(gateway.networkProfile())
        }
        return detectedBinary.length > 0 || detectedHome.length > 0
    }

    function checkProfile(showResult) {
        if (!gateway) {
            return null
        }
        statusError = ""
        gateway.setStatus(qsTr("Local wallet"))
        return gateway.request("localWalletProfileStatus", [profile(gateway.networkProfile())], qsTr("Local wallet"), showResult === true, function (response) {
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
        const response = gateway.requestBlocking("localWalletProfileStatus", [profile(gateway.networkProfile())], qsTr("Local wallet"), false)
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

        return runRequest(qsTr("Wallet account"), "localWalletCreateAccount", [profile(gateway.networkProfile()), privacy, label, "confirm-create-account"], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Create account"), "created", commandOperationDetail(response.value))
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

        return runRequest(qsTr("Wallet send"), "localWalletSendTransaction", [profile(gateway.networkProfile()), request, "confirm-send-transaction"], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Send transaction"), "submitted", commandOperationDetail(response.value))
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

        return runRequest(qsTr("Read incoming"), "localWalletSyncPrivate", [profile(gateway.networkProfile()), "confirm-sync-private"], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Read incoming"), "submitted", privateSyncOperationDetail(response.value))
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

        return runRequest(qsTr("Wallet command"), "localWalletCommand", [profile(gateway.networkProfile()), args, "confirm-wallet-command"], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Wallet command"), "completed", commandOperationDetail(response.value))
            } else {
                appendHistory(qsTr("Wallet command"), "down", response.error || qsTr("Wallet command failed."))
            }
        })
    }

    function deployProgram(programPath) {
        const path = String(programPath || "").trim()
        if (busyGuard(qsTr("Program deploy"))) {
            return null
        }
        if (!path.length) {
            gateway.setResult(qsTr("Program deploy"), qsTr("Program binary path is required."), true, null)
            return null
        }
        if (!profileGuard(qsTr("Program deploy"), qsTr("Configure wallet binary and wallet home before deploying a program."), "profiles")) {
            return null
        }

        return runRequest(qsTr("Program deploy"), "localWalletDeployProgram", [profile(gateway.networkProfile()), path, "confirm-deploy-program"], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Deploy program"), "submitted", deployProgramOperationDetail(response.value))
            } else {
                appendHistory(qsTr("Deploy program"), "down", response.error || qsTr("Program deployment failed."))
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

        return runRequest(qsTr("Private sync"), "localWalletSyncPrivate", [profile(gateway.networkProfile()), "confirm-sync-private"], true, function (response) {
            if (response.ok) {
                appendHistory(qsTr("Private sync"), "submitted", privateSyncOperationDetail(response.value))
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

        return runRequest(qsTr("Wallet accounts"), "localWalletAccounts", [profile(gateway.networkProfile())], showResult === true, function (response) {
            if (response.ok) {
                accountsValue = response.value || null
                accountsError = ""
                const count = response.value && Array.isArray(response.value.accounts) ? response.value.accounts.length : 0
                appendHistory(qsTr("Wallet accounts"), "loaded", qsTr("%1 accounts").arg(count))
            } else {
                accountsValue = null
                accountsError = response.error || qsTr("Wallet account list failed.")
                appendHistory(qsTr("Wallet accounts"), "down", accountsError)
            }
        })
    }

    function previewInstruction(request) {
        if (busyGuard(qsTr("IDL instruction"))) {
            return null
        }

        return runRequest(qsTr("IDL instruction"), "localWalletInstructionPreview", [request || {}], false, function (response) {
            if (response.ok) {
                gateway.setIdlInstructionState(response.value || null, "")
            } else {
                gateway.setIdlInstructionState(null, response.error || qsTr("Instruction preview failed."))
            }
        }, function () {
            gateway.setIdlInstructionState(null, "")
        })
    }

    function sendInstruction(request) {
        if (busyGuard(qsTr("IDL instruction"))) {
            return null
        }
        if (!homeConfigured()) {
            gateway.openLocalWallet("", "profiles")
            gateway.setResult(qsTr("IDL instruction"), qsTr("Configure wallet home before sending an IDL instruction."), true, null)
            return null
        }

        return runRequest(qsTr("IDL instruction"), "localWalletInstructionSubmit", [profile(gateway.networkProfile()), request || {}, "confirm-idl-instruction"], true, function (response) {
            if (response.ok) {
                gateway.setIdlInstructionState(response.value || null, "")
                appendHistory(qsTr("IDL instruction"), "submitted", idlInstructionOperationDetail(response.value))
            } else {
                const error = response.error || qsTr("Instruction send failed.")
                gateway.setIdlInstructionState(null, error)
                appendHistory(qsTr("IDL instruction"), "down", error)
            }
        }, function () {
            gateway.setIdlInstructionState(null, "")
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

    function deployProgramOperationDetail(value) {
        const report = value || {}
        const program = String(report.program_id_base58 || report.program_id_hex || "")
        const tx = String(report.deployment_tx_hash || "")
        if (program.length > 0 && tx.length > 0) {
            return qsTr("%1, tx %2").arg(UiFormat.shortHash(program)).arg(UiFormat.shortHash(tx))
        }
        if (tx.length > 0) {
            return qsTr("tx %1").arg(UiFormat.shortHash(tx))
        }
        return qsTr("submitted")
    }

    function privateSyncOperationDetail(value) {
        const report = value || {}
        const status = String(report.status || "submitted")
        const home = String(report.wallet_home_source || "")
        return home.length ? qsTr("%1, home %2").arg(status).arg(home) : status
    }

    function idlInstructionOperationDetail(value) {
        const report = value || {}
        const tx = String(report.tx_hash || report.txHash || "")
        if (tx.length > 0) {
            return qsTr("%1 %2, tx %3")
                .arg(String(report.mode || "tx"))
                .arg(String(report.instruction || "instruction"))
                .arg(UiFormat.shortHash(tx))
        }
        const words = Array.isArray(report.instruction_words) ? report.instruction_words.length : 0
        return qsTr("%1 %2, %3 word(s)")
            .arg(String(report.mode || "preview"))
            .arg(String(report.instruction || "instruction"))
            .arg(words)
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
            gateway.appendNodeOperationHistory({
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
            savePersisted(gateway.networkProfile())
        }
        return record
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
