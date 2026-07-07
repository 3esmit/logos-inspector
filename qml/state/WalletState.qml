import QtQml

QtObject {
    id: root

    property bool loaded: false
    property string profileLabel: qsTr("Local wallet")
    property string binary: ""
    property string home: ""
    property string publicKeyProbe: ""
    property string createPrivacy: "public"
    property string createLabel: ""
    property string sendFrom: ""
    property string sendTo: ""
    property string sendToKeys: ""
    property string sendToNpk: ""
    property string sendToVpk: ""
    property string sendToIdentifier: ""
    property string sendAmount: ""
    property string advancedCommand: ""
    property string bedrockBalanceTip: ""
    property var status: null
    property string statusError: ""
    property var operations: []
    property var accountsValue: null
    property string accountsError: ""

    function load(value) {
        loaded = true
        if (!value || typeof value !== "object") {
            return
        }
        const profile = value.profile && typeof value.profile === "object" ? value.profile : value
        profileLabel = String(profile.label || profile.name || qsTr("Local wallet"))
        binary = String(profile.wallet_binary || profile.walletBinary || "")
        home = String(profile.wallet_home || profile.walletHome || "")
        publicKeyProbe = String(profile.public_key_probe || profile.publicKeyProbe || "")
        operations = Array.isArray(value.operations) ? value.operations : []
    }

    function clearStatus() {
        status = null
        statusError = ""
    }

    function profile(networkProfile) {
        return {
            label: String(profileLabel || qsTr("Local wallet")),
            wallet_binary: String(binary || ""),
            wallet_home: String(home || ""),
            network_profile: String(networkProfile || ""),
            public_key_probe: String(publicKeyProbe || "")
        }
    }

    function payload(networkProfile) {
        return {
            version: 1,
            profile: profile(networkProfile),
            operations: Array.isArray(operations) ? operations.slice(-50) : []
        }
    }

    function homeConfigured() {
        if (String(home || "").trim().length > 0) {
            return true
        }
        const source = String(status && status.home_source ? status.home_source : "")
        return source.length > 0 && source !== "none"
    }

    function profileConfigured() {
        return String(binary || "").trim().length > 0 && homeConfigured()
    }

    function profileUsable() {
        return profileConfigured()
            && status
            && String(status.status || "") === "ok"
    }

    function operationStatus(statusText) {
        const value = String(statusText || "").toLowerCase()
        if (value === "down" || value === "failed" || value === "error") {
            return "failed"
        }
        return "completed"
    }

    function appendOperation(label, statusText, detail) {
        const rows = Array.isArray(operations) ? operations.slice(-49) : []
        const record = {
            time: new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss"),
            label: String(label || qsTr("Local wallet")),
            status: String(statusText || "unknown"),
            detail: String(detail || "")
        }
        rows.push(record)
        operations = rows
        return record
    }
}
