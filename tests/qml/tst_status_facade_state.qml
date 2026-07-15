import QtQuick
import QtTest
import "../../qml/state/domains" as Domains
import "../../qml/state/status/StatusFactsProjection.js" as StatusFactsProjection

TestCase {
    id: testRoot

    name: "StatusFacadeState"

    Domains.CapabilityGateState {
        id: gates
    }

    Domains.OperationHistoryState {
        id: history
    }

    Domains.StatusFacadeState {
        id: status

        capabilityFacade: gates
        operationHistory: history
    }

    function init() {
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "available",
                configured_connector: "direct_storage_rest",
                connector_provenance: "network_profile"
            }]
        })
        gates.registryLoaded = true
        gates.compatibilityAvailability = ({})
        history.runtimeOperationHistory = []
        history.runtimeOperationsRevision = 0
        status.reports = ({})
        status.events = ({})
    }

    function test_facts_preserve_capability_operation_report_event_provenance() {
        history.append({
            domain: "storage",
            method: "storageManifests",
            status: "completed"
        }, "listed")
        status.reports = ({ storage_source: { ok: true } })
        status.events = ({ storage_revision: 1 })

        const facts = status.facts()

        compare(facts.capabilities[0].key, "storage")
        compare(facts.capabilities[0].provenance[0], "capability_registry")
        compare(facts.operations[0].domain, "storage")
        compare(facts.reports[0].provenance[0], "report")
        compare(facts.events[0].provenance[0], "event")
    }

    function test_backup_import_summary_counts_policy_decisions() {
	        history.append({
	            domain: "backup",
	            method: "settingsBackupImportPolicy",
	            status: "stopped_for_import",
	            result: { action: "stop", operation_id: "op-read" }
	        }, "stopped")
	        history.append({
	            domain: "backup",
	            method: "settingsBackupImportPolicy",
	            status: "restarted_after_import",
	            result: { action: "restart", operation_id: "op-read" }
	        }, "restarted")
	        history.append({
	            domain: "backup",
	            method: "settingsBackupImportApply",
	            status: "applied_for_import"
	        }, "applied")

        const summary = status.facts().backup_import

        compare(summary.stops, 1)
        compare(summary.restarts, 1)
        compare(summary.applies, 1)
        compare(summary.provenance[0], "operation_history")
    }

    function test_dashboard_gate_maps_metric_family_to_capability() {
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "unavailable",
                connector_provenance: "runtime_registry"
            }]
        })
        gates.compatibilityAvailability = ({})

        const gate = status.dashboardGate("storage.peer_count")

        verify(!gate.enabled)
        compare(gate.missing[0].dependency, "storage")
        compare(gate.provenance[0], "capability_registry")
    }

    function test_dashboard_projection_marks_blocked_metric_without_hiding_card() {
        const item = StatusFactsProjection.dashboardGraphItem({
            metrics: {
                dashboardMetricValue: function () { return 12 },
                dashboardMetricSamples: function () { return [{ value: 12 }] },
                valueText: function (value) { return String(value) },
                dashboardGate: function () {
                    return {
                        enabled: false,
                        status: "disabled",
                        missing: [{ dependency: "storage" }],
                        warnings: [],
                        provenance: ["source_routing"]
                    }
                }
            }
        }, "storage.peer_count")

        compare(item.value, "disabled")
        compare(item.tone, "warning")
        compare(item.samples.length, 0)
        compare(item.gate.missing[0].dependency, "storage")
    }
}
