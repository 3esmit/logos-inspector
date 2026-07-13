import QtQml
import "../utils/UiFormat.js" as UiFormat
import "wallet/LocalWalletOperationDrafts.js" as LocalWalletOperationDrafts

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
        const requestedRevision = profileRevision
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
        }, function () {
            return profileRevision === requestedRevision
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
        return runOperationDraft(LocalWalletOperationDrafts.createAccount(root), function (response) {
            if (response.ok) {
                createLabel = ""
            }
        })
    }

    function sendTransaction() {
        return runOperationDraft(LocalWalletOperationDrafts.sendTransaction(root))
    }

    function readIncomingTransactions() {
        return runOperationDraft(LocalWalletOperationDrafts.readIncomingTransactions(root))
    }

    function runCommand(commandArgs) {
        return runOperationDraft(LocalWalletOperationDrafts.runCommand(root, commandArgs))
    }

    function syncPrivate() {
        return runOperationDraft(LocalWalletOperationDrafts.syncPrivate(root))
    }

    function queryAccounts(showResult) {
        return runOperationDraft(LocalWalletOperationDrafts.queryAccounts(root, showResult), function (response) {
            if (response.ok) {
                accountsValue = response.value || null
                accountsError = ""
            } else {
                accountsValue = null
                accountsError = response.error || qsTr("Wallet account list failed.")
            }
        })
    }

    function queryBedrockBalance() {
        const draft = LocalWalletOperationDrafts.queryBedrockBalance(root)
        if (!draft.ok) {
            gateway.setBedrockWalletBalance(null, draft.balanceError)
            return null
        }
        gateway.setBedrockWalletBalance(null, "")
        return runOperationDraft(draft, function (response) {
            if (response.ok) {
                gateway.setBedrockWalletBalance(response.value, "")
            } else {
                const error = response.error || qsTr("Balance query failed.")
                gateway.setBedrockWalletBalance(null, error)
            }
        }, true)
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

    function runOperationDraft(draft, afterResponse, skipGenericResult) {
        if (!draft || draft.ok !== true) {
            applyInvalidDraft(draft)
            return null
        }
        return runRequest(draft.title, draft.method, draft.args, draft.showResult === true, function (response) {
            if (afterResponse) {
                afterResponse(response)
            }
            if (skipGenericResult === true) {
                appendHistory(draft.historyLabel, response.ok ? draft.successStatus : "down",
                    response.ok ? draft.publicKey : (response.error || draft.failureMessage))
                return
            }
            appendHistory(draft.historyLabel, response.ok ? draft.successStatus : "down",
                response.ok ? operationDetail(response.value, draft.fallback) : (response.error || draft.failureMessage))
        })
    }

    function applyInvalidDraft(draft) {
        if (!draft || !gateway) {
            return
        }
        if (draft.balanceError !== undefined) {
            gateway.setBedrockWalletBalance(null, String(draft.balanceError || ""))
            return
        }
        if (String(draft.tab || "").length > 0) {
            gateway.openLocalWallet("", draft.tab)
        }
        if (String(draft.message || "").length > 0) {
            gateway.setResult(draft.title, draft.message, true, null)
        }
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
