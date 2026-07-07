import QtQml

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
