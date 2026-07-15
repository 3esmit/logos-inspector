import QtQuick
import QtTest
import "../../qml/state/domains" as Domains
import "fixtures"

TestCase {
    id: testRoot

    name: "CapabilityGateState"

    StateGatewayFixture {
        id: gateway
    }

    Domains.CapabilityGateState {
        id: gates

        gateway: gateway
    }

    function init() {
        gateway.reset()
        gates.registryReport = ({ schema_version: 1, capabilities: [] })
        gates.registryLoaded = false
        gates.registryError = ""
        gates.compatibilityAvailability = ({})
    }

    function test_load_registry_report_uses_build_mode_argument() {
        gateway.callResponses = ({
            capabilityRegistryReport: {
                ok: true,
                value: {
                    schema_version: 1,
                    build_mode: "basecamp",
                    capabilities: []
                },
                text: "OK",
                error: ""
            }
        })

        verify(gates.loadRegistry(true))

        compare(gateway.lastMethod, "capabilityRegistryReport")
        compare(gateway.lastArgs[0], true)
        verify(gateway.lastArgs[1] !== undefined)
        verify(gates.registryLoaded)
        compare(gates.registryReport.build_mode, "basecamp")
    }

    function test_all_of_reports_missing_dependencies_in_order() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [
                {
                    key: "storage",
                    label: "Storage",
                    status: "unavailable",
                    sub_capabilities: ["storage.content.upload"]
                },
                {
                    key: "delivery",
                    label: "Delivery",
                    status: "loading",
                    sub_capabilities: ["delivery.send"]
                }
            ]
        })

        const gate = gates.gateFor({ all_of: ["storage.content.upload", "delivery.send"] })

        verify(!gate.enabled)
        compare(gate.status, "loading")
        compare(gate.missing.length, 2)
        compare(gate.missing[0].dependency, "storage.content.upload")
        compare(gate.missing[1].dependency, "delivery.send")
    }

    function test_any_of_enables_when_one_dependency_available() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [
                {
                    key: "storage",
                    label: "Storage",
                    status: "unavailable",
                    sub_capabilities: ["storage.content.upload"]
                },
                {
                    key: "delivery",
                    label: "Delivery",
                    status: "available",
                    sub_capabilities: ["delivery.send"]
                }
            ]
        })

        const gate = gates.gateFor({ any_of: ["storage.content.upload", "delivery.send"] })

        verify(gate.enabled)
        compare(gate.status, "enabled")
        compare(gate.missing.length, 0)
    }

    function test_degraded_dependency_is_enabled_with_warning() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "delivery",
                label: "Delivery",
                status: "degraded",
                sub_capabilities: ["delivery.store.query", "delivery.send"],
                unavailable_sub_capabilities: ["delivery.send"],
                warnings: ["Delivery send is unavailable."],
                connector_provenance: "runtime_registry"
            }]
        })

        const gate = gates.gateFor("delivery.store.query")

        verify(gate.enabled)
        compare(gate.status, "degraded")
        compare(gate.warnings.length, 1)
        compare(gate.provenance[0], "capability_registry")
    }

    function test_disabled_and_input_required_states_are_structured() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "unavailable",
                sub_capabilities: ["storage.content.upload"]
            }]
        })

        const disabled = gates.gateFor("storage.content.upload")
        const inputRequired = gates.gateFor("storage.content.upload", {
            required_inputs: [{ key: "cid", label: "CID", value: "" }]
        })

        verify(!disabled.enabled)
        compare(disabled.status, "disabled")
        compare(disabled.missing[0].capability, "storage")
        verify(!inputRequired.enabled)
        compare(inputRequired.status, "input_required")
        compare(inputRequired.missing[0].dependency, "cid")
    }

    function test_registry_input_required_state_is_structured() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "input_required",
                sub_capabilities: ["storage.content.upload"],
                connector_provenance: "runtime_registry"
            }]
        })

        const gate = gates.gateFor("storage.content.upload")

        verify(!gate.enabled)
        compare(gate.status, "input_required")
        compare(gate.missing.length, 1)
        compare(gate.missing[0].dependency, "storage.content.upload")
        compare(gate.missing[0].status, "input_required")
        compare(gate.missing[0].capability, "storage")
        compare(gate.missing[0].provenance, "capability_registry")
    }

    function test_registry_report_precedes_compatibility_availability() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "degraded",
                sub_capabilities: ["storage.rest.upload"],
                unavailable_sub_capabilities: ["storage.rest.upload"],
                connector_provenance: "build_default"
            }]
        })
        gates.compatibilityAvailability = ({
            "storage.rest.upload": true
        })

        const gate = gates.gateFor("storage.rest.upload")

        verify(!gate.enabled)
        compare(gate.missing[0].dependency, "storage.rest.upload")
        compare(gate.missing[0].provenance, "capability_registry")
    }


    function test_program_decode_gate_is_not_capability_gated() {
        gates.registryLoaded = true
        gates.registryReport = ({ schema_version: 1, capabilities: [] })

        const decode = gates.programDecodeGate()
        const combined = gates.gateFor({ all_of: ["program_decode.static", "storage.content.upload"] })

        verify(decode.enabled)
        compare(decode.status, "enabled")
        verify(!combined.enabled)
        compare(combined.missing.length, 1)
        compare(combined.missing[0].dependency, "storage.content.upload")
    }

    function test_social_comments_do_not_require_storage_but_shared_idls_do() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [
	                {
	                    key: "delivery",
	                    label: "Delivery",
	                    status: "available",
	                    sub_capabilities: ["delivery.store.query", "delivery.send"]
	                },
	                {
	                    key: "storage",
	                    label: "Storage",
	                    status: "degraded",
	                    sub_capabilities: ["storage.content.read_by_cid", "storage.content.upload"],
	                    unavailable_sub_capabilities: ["storage.content.read_by_cid", "storage.content.upload"]
	                }
	            ]
	        })
	        gates.compatibilityAvailability = ({
	            "social.identity.local": true
	        })

        const comments = gates.socialGate("comments.write")
        const sharedRead = gates.socialGate("shared_idl.read")
        const sharedWrite = gates.socialGate("shared_idl.write")

	        verify(comments.enabled)
	        verify(!sharedRead.enabled)
	        compare(sharedRead.missing[0].dependency, "storage.content.read_by_cid")
	        verify(!sharedWrite.enabled)
	        compare(sharedWrite.missing[0].dependency, "storage.content.upload")
	    }

    function test_missing_registry_capability_ignores_qml_compatibility_fallback() {
        gates.registryLoaded = true
        gates.registryReport = ({ schema_version: 1, capabilities: [] })
        gates.compatibilityAvailability = ({
            "storage.shared_idl.sync_read": {
                status: "available",
                provenance: "source_routing"
            }
        })

        const gate = gates.gateFor("storage.shared_idl.sync_read")

        verify(!gate.enabled)
        compare(gate.status, "disabled")
        compare(gate.missing[0].dependency, "storage.shared_idl.sync_read")
        compare(gate.missing[0].provenance, "capability_registry")
    }

    function test_local_identity_remains_an_app_owned_compatibility_dependency() {
        gates.registryLoaded = true
        gates.registryReport = ({ schema_version: 1, capabilities: [] })
        gates.compatibilityAvailability = ({
            "social.identity.local": {
                status: "available",
                provenance: "local_identity"
            }
        })

        const gate = gates.gateFor("social.identity.local")

        verify(gate.enabled)
        compare(gate.provenance[0], "local_identity")
    }

    function test_shared_idl_blocks_when_registry_reports_sync_unavailable() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [
                {
                    key: "delivery",
                    label: "Delivery",
                    status: "available",
                    sub_capabilities: ["delivery.store.query", "delivery.send"]
                },
                {
                    key: "storage",
                    label: "Storage",
                    status: "degraded",
                    sub_capabilities: [
                        "storage.content.read_by_cid",
                        "storage.content.upload",
                        "storage.shared_idl.sync_read",
                        "storage.shared_idl.sync_upload"
                    ],
                    unavailable_sub_capabilities: [
                        "storage.shared_idl.sync_read",
                        "storage.shared_idl.sync_upload"
                    ]
                }
            ]
        })
        gates.compatibilityAvailability = ({ "social.identity.local": true })

        const sharedRead = gates.socialGate("shared_idl.read")
        const sharedWrite = gates.socialGate("shared_idl.write")

        verify(!sharedRead.enabled)
        compare(sharedRead.missing[0].dependency, "storage.shared_idl.sync_read")
        compare(sharedRead.missing[0].provenance, "capability_registry")
        verify(!sharedWrite.enabled)
        compare(sharedWrite.missing[0].dependency, "storage.shared_idl.sync_upload")
        compare(sharedWrite.missing[0].provenance, "capability_registry")
    }

    function test_backup_storage_gate_requires_sync_transport_subcapability() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "storage",
                label: "Storage",
                status: "available",
                sub_capabilities: ["storage.content.upload", "storage.content.read_by_cid"]
            }]
        })

        const upload = gates.storageGate("backup_upload")
        const read = gates.storageGate("backup_read_by_cid")

        verify(!upload.enabled)
        compare(upload.missing[0].dependency, "storage.backup.sync_upload")
        verify(!read.enabled)
        compare(read.missing[0].dependency, "storage.backup.sync_read_by_cid")
    }

    function test_wallet_scoped_dependencies_prefer_scoped_capability() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [
                {
                    key: "wallet",
                    label: "Wallet",
                    status: "available",
                    sub_capabilities: ["wallet.l2.instruction.submit"]
                },
                {
                    key: "wallet.l2",
                    label: "L2 Wallet",
                    status: "unavailable",
                    sub_capabilities: ["wallet.l2.instruction.submit"],
                    unavailable_sub_capabilities: ["wallet.l2.instruction.submit"]
                }
            ]
        })

        const gate = gates.walletGate("l2.submit")

        verify(!gate.enabled)
        compare(gate.missing[0].capability, "wallet.l2")
    }

    function test_diagnostics_gate_maps_action_to_registry_subcapability() {
        gates.registryLoaded = true
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [{
                key: "diagnostics",
                label: "Diagnostics",
                status: "degraded",
                sub_capabilities: ["diagnostics.storage.read", "diagnostics.wallet.read"],
                unavailable_sub_capabilities: ["diagnostics.wallet.read"]
            }]
        })

        const storage = gates.diagnosticsGate("storage")
        const wallet = gates.diagnosticsGate("wallet")

        verify(storage.enabled)
        verify(!wallet.enabled)
        compare(wallet.missing[0].dependency, "diagnostics.wallet.read")
    }

}
