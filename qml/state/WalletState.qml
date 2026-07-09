import QtQml
import "OperationHistoryVocabulary.js" as OperationHistoryVocabulary

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
    property var connectorConfig: ({})
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
        connectorConfig = profile.wallet_connector_config
            || profile.walletConnectorConfig
            || value.wallet_connector_config
            || value.walletConnectorConfig
            || ({})
        operations = Array.isArray(value.operations) ? value.operations : []
    }

    function clearStatus() {
        status = null
        statusError = ""
    }

    function profile(networkProfile, prefersBasecamp) {
        return {
            label: String(profileLabel || qsTr("Local wallet")),
            wallet_binary: String(binary || ""),
            wallet_home: String(home || ""),
            network_profile: String(networkProfile || ""),
            public_key_probe: String(publicKeyProbe || ""),
            wallet_connector_config: connectorConfigPayload(prefersBasecamp === true)
        }
    }

    function payload(networkProfile, prefersBasecamp) {
        return {
            version: 1,
            profile: profile(networkProfile, prefersBasecamp === true),
            operations: Array.isArray(operations) ? operations.slice(-50) : []
        }
    }

    function connectorConfigPayload(prefersBasecamp) {
        return normalizedConnectorConfig(connectorConfig, prefersBasecamp === true)
    }

    function normalizedConnectorConfig(value, prefersBasecamp) {
        const defaults = defaultConnectorConfig(prefersBasecamp === true).scopes
        const source = value && typeof value === "object" ? value : ({})
        const scopes = source.scopes && typeof source.scopes === "object" ? source.scopes : source
        const result = { scopes: ({}) }
        const keys = ["wallet.l1", "wallet.l2"]
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            const fallback = defaults[key] || {}
            const entry = scopes[key] && typeof scopes[key] === "object" ? scopes[key] : fallback
            result.scopes[key] = {
                connector_id: String(entry.connector_id || entry.connectorId || entry.id || fallback.connector_id || ""),
                endpoint: String(entry.endpoint || entry.url || entry.rest_endpoint || entry.rpc_endpoint || ""),
                provenance: String(entry.provenance || entry.connector_provenance || (entry === fallback ? "build_default" : "wallet_profile"))
            }
        }
        return result
    }

    function defaultConnectorConfig(prefersBasecamp) {
        return {
            scopes: {
                "wallet.l1": {
                    connector_id: prefersBasecamp === true ? "blockchain_module" : "composed_wallet",
                    endpoint: "",
                    provenance: "build_default"
                },
                "wallet.l2": {
                    connector_id: prefersBasecamp === true ? "lez_core" : "composed_wallet",
                    endpoint: "",
                    provenance: "build_default"
                }
            }
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
        return OperationHistoryVocabulary.syntheticHistoryStatus(statusText)
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
