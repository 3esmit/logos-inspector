import QtQml

QtObject {
    id: root

    property var capabilityFacade: null
    property var operationHistory: null
    property var reports: ({})
    property var events: ({})
    property int revision: 0

    function facts() {
        const currentRevision = revision
        return {
            capabilities: capabilityRows(),
            operations: operationRows(""),
            backup_import: backupImportDecisionSummary(),
            reports: reportRows(),
            events: eventRows(),
            provenance: ["capability", "operation", "report", "event"]
        }
    }

    function capabilityRows() {
        const facade = capabilityFacade || null
        const report = facade && facade.registryReport ? facade.registryReport : ({})
        const capabilities = Array.isArray(report.capabilities) ? report.capabilities : []
        const rows = []
        for (let i = 0; i < capabilities.length; ++i) {
            const capability = capabilities[i] || {}
            const gate = facade && typeof facade.gateFor === "function"
                ? facade.gateFor(String(capability.key || ""))
                : null
            rows.push({
                key: String(capability.key || ""),
                label: String(capability.label || capability.key || ""),
                status: String(gate && gate.status ? gate.status : (capability.status || "loading")),
                connector: String(capability.configured_connector || capability.default_connector || ""),
                provenance: gate && Array.isArray(gate.provenance)
                    ? gate.provenance
                    : ["capability_registry", String(capability.connector_provenance || "")]
            })
        }
        return rows
    }

    function operationRows(domain) {
        if (operationHistory && typeof operationHistory.rows === "function") {
            return operationHistory.rows(domain)
        }
        return []
    }

    function reportRows() {
        return objectRows(reports, "report")
    }

    function eventRows() {
        return objectRows(events, "event")
    }

    function objectRows(values, provenance) {
        const source = values && typeof values === "object" && !Array.isArray(values) ? values : ({})
        const rows = []
        const keys = Object.keys(source)
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            rows.push({
                key: key,
                value: source[key],
                provenance: [provenance]
            })
        }
        return rows
    }

    function backupImportDecisionSummary() {
        const rows = operationRows("backup")
        const summary = {
            stops: 0,
            skips: 0,
            restarts: 0,
            blocked: 0,
            applies: 0,
            failures: 0,
            latest: "",
            decisions: [],
            provenance: ["operation_history"]
	        }
	        for (let i = 0; i < rows.length; ++i) {
	            const row = rows[i] || {}
	            if (String(row.method || "") === "settingsBackupImportApply") {
	                if (String(row.status || "") === "applied_for_import") {
	                    summary.applies += 1
	                }
	                if (!summary.latest.length) {
	                    summary.latest = String(row.detail || row.label || "")
	                }
	                continue
	            }
	            if (String(row.method || "") !== "settingsBackupImportPolicy") {
	                continue
	            }
	            const status = String(row.status || "")
	            if (status === "stopped_for_import") {
	                summary.stops += 1
	            } else if (status === "let_finish_for_import" || status === "restart_skipped_for_import") {
	                summary.skips += 1
	            } else if (status === "restarted_after_import") {
	                summary.restarts += 1
	            } else if (status === "restart_failed_after_import") {
	                summary.failures += 1
	            } else if (status === "blocked_for_import" || status === "stop_failed_for_import") {
	                summary.blocked += 1
	            }
            summary.decisions.push(row)
            if (!summary.latest.length) {
                summary.latest = String(row.detail || row.label || "")
            }
        }
        return summary
    }

    function dashboardGate(key) {
        const capability = dashboardCapability(String(key || ""))
        if (!capability.length || !capabilityFacade || typeof capabilityFacade.gateFor !== "function") {
            return {
                enabled: true,
                status: "enabled",
                missing: [],
                warnings: [],
                provenance: ["status_facade"]
            }
        }
        return capabilityFacade.gateFor(capability)
    }

    function dashboardCapability(key) {
        if (key.indexOf("storage.") === 0) {
            return "storage"
        }
        if (key.indexOf("messaging.") === 0) {
            return "delivery"
        }
        if (key.indexOf("bedrock.") === 0) {
            return "l1"
        }
        return ""
    }
}
