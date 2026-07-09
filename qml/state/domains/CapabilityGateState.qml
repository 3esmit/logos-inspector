import QtQml

QtObject {
    id: root

    property var gateway: null
    property var registryReport: ({ schema_version: 1, capabilities: [] })
    property bool registryLoaded: false
    property string registryError: ""
    property var compatibilityAvailability: ({})
    property int revision: 0

    function loadRegistry(prefersBasecamp, runtimeInputs) {
        const response = callGateway("capabilityRegistryReport", [prefersBasecamp === true, runtimeInputs || ({})])
        if (response && response.ok === true && response.value && typeof response.value === "object" && Array.isArray(response.value.capabilities)) {
            registryReport = response.value
            registryLoaded = true
            registryError = ""
            revision += 1
            return true
        }
        registryLoaded = registryLoaded && Array.isArray(registryReport && registryReport.capabilities)
        registryError = String(response && response.error ? response.error : qsTr("Capability registry is unavailable."))
        revision += 1
        return false
    }

    function callGateway(method, args) {
        if (gateway && typeof gateway.callInspector === "function") {
            return gateway.callInspector(method, args || [])
        }
        if (gateway && typeof gateway.call === "function") {
            return gateway.call(method, args || [], qsTr("Capabilities"))
        }
        return {
            ok: false,
            value: null,
            text: "",
            error: qsTr("Capability registry bridge is unavailable.")
        }
    }

    function gateFor(expression, options) {
        const opts = options && typeof options === "object" ? options : ({})
        const input = inputGate(opts)
        if (!input.enabled) {
            return input
        }
        return normalizeGate(evaluateExpression(expression))
    }

    function storageGate(action, options) {
        return gateFor(storageDependency(action), options || {})
    }

    function deliveryGate(action, options) {
        return gateFor(deliveryDependency(action), options || {})
    }

    function diagnosticsGate(action, options) {
        return gateFor(diagnosticsDependency(action), options || {})
    }

    function socialGate(action, options) {
        return gateFor(socialDependency(action), options || {})
    }

    function walletGate(action, options) {
        return gateFor(walletDependency(action), options || {})
    }

    function programDecodeGate(options) {
        return gateFor("program_decode.static", options || {})
    }

    function enabled(gate) {
        return gate && gate.enabled === true
    }

    function inputGate(options) {
        const opts = options && typeof options === "object" ? options : ({})
        const required = Array.isArray(opts.required_inputs) ? opts.required_inputs : []
        const missing = []
        for (let i = 0; i < required.length; ++i) {
            const entry = required[i] || {}
            const key = String(entry.key || entry.dependency || "")
            const ok = entry.present === true || (entry.value !== undefined && entry.value !== null && String(entry.value).trim().length > 0)
            if (!ok) {
                missing.push(missingRecord(key, String(entry.label || key || qsTr("Input")), "input_required", key, "input"))
            }
        }
        if (missing.length > 0) {
            return {
                enabled: false,
                status: "input_required",
                missing: missing,
                warnings: [],
                provenance: ["input"]
            }
        }
        return enabledGate([], ["input"])
    }

    function evaluateExpression(expression) {
        if (Array.isArray(expression)) {
            return allOf(expression)
        }
        if (expression && typeof expression === "object") {
            if (Array.isArray(expression.all_of)) {
                return allOf(expression.all_of)
            }
            if (Array.isArray(expression.any_of)) {
                return anyOf(expression.any_of)
            }
            if (expression.dependency !== undefined) {
                return dependencyGate(String(expression.dependency || ""))
            }
        }
        return dependencyGate(String(expression || ""))
    }

    function allOf(dependencies) {
        const rows = Array.isArray(dependencies) ? dependencies : []
        const missing = []
        const warnings = []
        const provenance = []
        let degraded = false
        for (let i = 0; i < rows.length; ++i) {
            const gate = normalizeGate(evaluateExpression(rows[i]))
            appendList(missing, gate.missing)
            appendList(warnings, gate.warnings)
            appendList(provenance, gate.provenance)
            degraded = degraded || gate.status === "degraded"
        }
        if (missing.length > 0) {
            return disabledGate(combinedMissingStatus(missing), missing, warnings, provenance)
        }
        return enabledGate(warnings, provenance, degraded)
    }

    function anyOf(dependencies) {
        const rows = Array.isArray(dependencies) ? dependencies : []
        const missing = []
        const warnings = []
        const provenance = []
        for (let i = 0; i < rows.length; ++i) {
            const gate = normalizeGate(evaluateExpression(rows[i]))
            if (gate.enabled) {
                appendList(warnings, gate.warnings)
                appendList(provenance, gate.provenance)
                return enabledGate(warnings, provenance, gate.status === "degraded")
            }
            appendList(missing, gate.missing)
            appendList(warnings, gate.warnings)
            appendList(provenance, gate.provenance)
        }
        return disabledGate(combinedMissingStatus(missing), missing, warnings, provenance)
    }

    function dependencyGate(dependency) {
        const key = String(dependency || "")
        if (!key.length) {
            return enabledGate([], ["none"])
        }
        if (key === "program_decode.static") {
            return enabledGate([], ["program_decode_static"])
        }

        const capability = reportCapabilityForDependency(key)
        if (!capability) {
            const compatibility = compatibilityGate(key)
            if (compatibility !== null) {
                return compatibility
            }
            if (!registryLoaded) {
                return disabledGate("loading", [missingRecord(key, key, "loading", key, "capability_registry")], [], ["capability_registry"])
            }
            return disabledGate("disabled", [missingRecord(key, key, "unavailable", key, "capability_registry")], [], ["capability_registry"])
        }

        const status = normalizedCapabilityStatus(capability.status)
        const label = String(capability.label || key)
        const capabilityKey = String(capability.key || key)
        const provenance = ["capability_registry", String(capability.connector_provenance || "")]
        const unavailable = subCapabilityUnavailable(capability, key)
        if (status === "enabled" || (status === "degraded" && !unavailable)) {
            const warnings = registryWarnings(capability, key, capabilityKey)
            return enabledGate(warnings, provenance, status === "degraded")
        }
        if (status === "loading") {
            return disabledGate("loading", [missingRecord(key, label, "loading", capabilityKey, "capability_registry")], [], provenance)
        }
        if (status === "input_required") {
            return disabledGate("input_required", [missingRecord(key, label, "input_required", capabilityKey, "capability_registry")], [], provenance)
        }
        return disabledGate("disabled", [missingRecord(key, label, "unavailable", capabilityKey, "capability_registry")], [], provenance)
    }

    function compatibilityGate(dependency) {
        const map = compatibilityAvailability && typeof compatibilityAvailability === "object" ? compatibilityAvailability : ({})
        if (map[dependency] === undefined) {
            return null
        }
        const entry = map[dependency]
        const status = normalizedCompatibilityStatus(entry)
        const label = compatibilityLabel(entry, dependency)
        const provenance = [compatibilityProvenance(entry)]
        if (status === "enabled") {
            return enabledGate([], provenance)
        }
        if (status === "degraded") {
            return enabledGate([warningRecord(dependency, label, dependency, provenance[0])], provenance, true)
        }
        if (status === "loading") {
            return disabledGate("loading", [missingRecord(dependency, label, "loading", dependency, provenance[0])], [], provenance)
        }
        if (status === "input_required") {
            return disabledGate("input_required", [missingRecord(dependency, label, "input_required", dependency, provenance[0])], [], provenance)
        }
        return disabledGate("disabled", [missingRecord(dependency, label, "unavailable", dependency, provenance[0])], [], provenance)
    }

    function normalizedCompatibilityStatus(entry) {
        if (entry === true) {
            return "enabled"
        }
        if (entry === false) {
            return "disabled"
        }
        if (typeof entry === "string") {
            return normalizedCapabilityStatus(entry)
        }
        if (entry && typeof entry === "object") {
            return normalizedCapabilityStatus(entry.status)
        }
        return "disabled"
    }

    function compatibilityLabel(entry, fallback) {
        if (entry && typeof entry === "object" && entry.label !== undefined) {
            return String(entry.label || fallback)
        }
        return String(fallback || "")
    }

    function compatibilityProvenance(entry) {
        if (entry && typeof entry === "object" && entry.provenance !== undefined) {
            return String(entry.provenance || "compatibility")
        }
        return "compatibility"
    }

    function reportCapabilityForDependency(dependency) {
        const capabilities = registryReport && Array.isArray(registryReport.capabilities) ? registryReport.capabilities : []
        for (let i = 0; i < capabilities.length; ++i) {
            const capability = capabilities[i] || {}
            if (String(capability.key || "") === dependency) {
                return capability
            }
        }
        const scopedKey = scopedCapabilityKeyForDependency(dependency)
        if (scopedKey.length > 0) {
            for (let j = 0; j < capabilities.length; ++j) {
                const scoped = capabilities[j] || {}
                if (String(scoped.key || "") === scopedKey
                        && (listContains(scoped.sub_capabilities, dependency)
                            || listContains(scoped.unavailable_sub_capabilities, dependency))) {
                    return scoped
                }
            }
        }
        for (let k = 0; k < capabilities.length; ++k) {
            const current = capabilities[k] || {}
            if (listContains(current.sub_capabilities, dependency) || listContains(current.unavailable_sub_capabilities, dependency)) {
                return current
            }
        }
        return null
    }

    function scopedCapabilityKeyForDependency(dependency) {
        const key = String(dependency || "")
        if (key.indexOf("wallet.l1.") === 0) {
            return "wallet.l1"
        }
        if (key.indexOf("wallet.l2.") === 0) {
            return "wallet.l2"
        }
        return ""
    }

    function subCapabilityUnavailable(capability, dependency) {
        return listContains(capability && capability.unavailable_sub_capabilities, dependency)
    }

    function registryWarnings(capability, dependency, capabilityKey) {
        const rows = []
        const warnings = Array.isArray(capability && capability.warnings) ? capability.warnings : []
        for (let i = 0; i < warnings.length; ++i) {
            rows.push(warningRecord(dependency, String(warnings[i] || qsTr("Capability is degraded.")), capabilityKey, "capability_registry"))
        }
        if (rows.length === 0 && normalizedCapabilityStatus(capability && capability.status) === "degraded") {
            rows.push(warningRecord(dependency, qsTr("Capability is degraded."), capabilityKey, "capability_registry"))
        }
        return rows
    }

    function normalizedCapabilityStatus(value) {
        const status = String(value || "").toLowerCase()
        if (status === "available" || status === "enabled") {
            return "enabled"
        }
        if (status === "degraded") {
            return "degraded"
        }
        if (status === "unknown" || status === "loading") {
            return "loading"
        }
        if (status === "input_required") {
            return "input_required"
        }
        return "disabled"
    }

    function storageDependency(action) {
        switch (String(action || "")) {
        case "manifests":
            return "storage.manifests.read"
        case "exists":
            return "storage.content.exists"
        case "read_by_cid":
        case "fetch":
            return "storage.content.read_by_cid"
        case "upload":
            return "storage.content.upload"
        case "backup_read_by_cid":
            return { all_of: ["storage.content.read_by_cid", "storage.backup.sync_read_by_cid"] }
        case "backup_upload":
            return { all_of: ["storage.content.upload", "storage.backup.sync_upload"] }
        case "cache":
        case "download":
            return "storage.content.download_to_file"
        case "remove":
            return "storage.content.remove"
        default:
            return "storage"
        }
    }

    function deliveryDependency(action) {
        switch (String(action || "")) {
        case "store_query":
            return "delivery.store.query"
        case "subscribe":
            return "delivery.subscribe"
        case "send":
            return "delivery.send"
        default:
            return "delivery"
        }
    }

    function diagnosticsDependency(action) {
        switch (String(action || "")) {
        case "modules.status":
            return "diagnostics.modules.status.read"
        case "modules.info":
            return "diagnostics.modules.info.read"
        case "modules.metrics":
            return "diagnostics.modules.metrics.read"
        case "probe":
            return "diagnostics.provider.probe"
        case "storage":
            return "diagnostics.storage.read"
        case "delivery":
            return "diagnostics.delivery.read"
        case "wallet":
            return "diagnostics.wallet.read"
        case "local_nodes":
            return "diagnostics.local_nodes.read"
        default:
            return "diagnostics"
        }
    }

    function socialDependency(action) {
        switch (String(action || "")) {
        case "comments.read":
            return "delivery.store.query"
        case "comments.write":
            return { all_of: ["delivery.send", "social.identity.local"] }
	        case "shared_idl.read":
	            return { all_of: ["delivery.store.query", "storage.content.read_by_cid", "storage.shared_idl.sync_read"] }
	        case "shared_idl.write":
	            return { all_of: ["storage.content.upload", "storage.shared_idl.sync_upload", "delivery.send", "social.identity.local"] }
        case "sync.live":
            return "delivery.subscribe"
        default:
            return "delivery"
        }
    }

    function walletDependency(action) {
        switch (String(action || "")) {
        case "l1.read":
            return "wallet.l1.accounts.read"
        case "l1.sign":
            return { all_of: ["wallet.l1.sign", "wallet.l1.submit"] }
        case "l2.preview":
            return { all_of: ["program_decode.static", "wallet.l2.instruction.preview"] }
        case "l2.submit":
            return { all_of: ["program_decode.static", "wallet.l2.instruction.submit"] }
        case "program.deploy":
            return "wallet.l2.program.deploy"
        default:
            return "wallet"
        }
    }

    function enabledGate(warnings, provenance, degraded) {
        const warn = compactList(warnings)
        return {
            enabled: true,
            status: degraded === true || warn.length > 0 ? "degraded" : "enabled",
            missing: [],
            warnings: warn,
            provenance: compactList(provenance)
        }
    }

    function disabledGate(status, missing, warnings, provenance) {
        return {
            enabled: false,
            status: status || "disabled",
            missing: Array.isArray(missing) ? missing : [],
            warnings: compactList(warnings),
            provenance: compactList(provenance)
        }
    }

    function normalizeGate(gate) {
        if (!gate || typeof gate !== "object") {
            return enabledGate([], ["none"])
        }
        return {
            enabled: gate.enabled === true,
            status: String(gate.status || (gate.enabled === true ? "enabled" : "disabled")),
            missing: Array.isArray(gate.missing) ? gate.missing : [],
            warnings: Array.isArray(gate.warnings) ? gate.warnings : [],
            provenance: Array.isArray(gate.provenance) ? compactList(gate.provenance) : []
        }
    }

    function combinedMissingStatus(missing) {
        const rows = Array.isArray(missing) ? missing : []
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i] && rows[i].status ? rows[i].status : "") === "input_required") {
                return "input_required"
            }
        }
        for (let j = 0; j < rows.length; ++j) {
            if (String(rows[j] && rows[j].status ? rows[j].status : "") === "loading") {
                return "loading"
            }
        }
        return "disabled"
    }

    function missingRecord(dependency, label, status, capability, provenance) {
        return {
            dependency: String(dependency || ""),
            label: String(label || dependency || ""),
            status: String(status || "unavailable"),
            capability: String(capability || dependency || ""),
            provenance: String(provenance || "")
        }
    }

    function warningRecord(dependency, label, capability, provenance) {
        return {
            dependency: String(dependency || ""),
            label: String(label || dependency || ""),
            capability: String(capability || dependency || ""),
            provenance: String(provenance || "")
        }
    }

    function appendList(target, values) {
        const rows = Array.isArray(values) ? values : []
        for (let i = 0; i < rows.length; ++i) {
            target.push(rows[i])
        }
    }

    function compactList(values) {
        const rows = Array.isArray(values) ? values : []
        const result = []
        const seen = ({})
        for (let i = 0; i < rows.length; ++i) {
            const value = rows[i]
            const key = typeof value === "string" ? value : JSON.stringify(value)
            if (!key.length || seen[key] === true) {
                continue
            }
            seen[key] = true
            result.push(value)
        }
        return result
    }

    function listContains(values, wanted) {
        const rows = Array.isArray(values) ? values : []
        const target = String(wanted || "")
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i] || "") === target) {
                return true
            }
        }
        return false
    }
}
