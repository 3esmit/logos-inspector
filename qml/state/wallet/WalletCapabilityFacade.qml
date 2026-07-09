import QtQml

QtObject {
    id: facade

    property var wallet: null
    property var capabilityFacade: null
    property var gateway: null
    property string networkProfile: ""
    property bool prefersBasecampModules: false

    function profile() {
        if (wallet && typeof wallet.profile === "function") {
            return wallet.profile(networkProfile, prefersBasecampModules)
        }
        return ({})
    }

    function profileConfigured() {
        return wallet && typeof wallet.profileConfigured === "function" && wallet.profileConfigured()
    }

    function homeConfigured() {
        return wallet && typeof wallet.homeConfigured === "function" && wallet.homeConfigured()
    }

    function openLocalWallet(tab) {
        if (gateway && typeof gateway.openLocalWallet === "function") {
            return gateway.openLocalWallet(String(tab || "profiles"))
        }
        return null
    }

    function gate(action, requiredInputs) {
        if (capabilityFacade && typeof capabilityFacade.walletGate === "function") {
            return capabilityFacade.walletGate(action, {
                required_inputs: Array.isArray(requiredInputs) ? requiredInputs : []
            })
        }
        return compatibilityGate()
    }

    function walletGate(action, options) {
        const input = options && Array.isArray(options.required_inputs) ? options.required_inputs : []
        return gate(action, input)
    }

    function enabled(action, requiredInputs) {
        return gate(action, requiredInputs).enabled === true
    }

    function problem(action, requiredInputs) {
        const value = gate(action, requiredInputs)
        const missing = Array.isArray(value.missing) ? value.missing : []
        if (missing.length > 0) {
            return String(missing[0].label || missing[0].dependency || qsTr("Wallet capability unavailable."))
        }
        const warnings = Array.isArray(value.warnings) ? value.warnings : []
        if (warnings.length > 0) {
            return String(warnings[0])
        }
        return qsTr("Wallet capability unavailable.")
    }

    function compatibilityGate() {
        return {
            enabled: true,
            status: "enabled",
            missing: [],
            warnings: [],
            provenance: ["wallet_capability_facade_compatibility"]
        }
    }
}
