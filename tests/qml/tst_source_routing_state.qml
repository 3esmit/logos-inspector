import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "SourceRoutingState"

    QtObject {
        id: gateway

        function callInspector(method, args) {
            return {
                ok: false,
                value: null,
                text: "",
                error: String(method || "") + String(args || "")
            }
        }

        function prefersBasecampModules() {
            return false
        }
    }

    Domains.SourceRoutingState {
        id: state

        gateway: gateway
        blockchainModule: "blockchain_module"
        deliveryModule: "delivery_module"
        storageModule: "storage_module"
        blockchainSourceMode: "rpc"
        messagingSourceMode: "rest"
        storageSourceMode: "rest"
        nodeUrl: "http://node"
        messagingRestUrl: "http://delivery"
        messagingMetricsUrl: "http://delivery-metrics"
        messagingNetworkPreset: "logos.test"
        storageRestUrl: "http://storage"
        storageMetricsUrl: "http://storage-metrics"
        storageNetworkPreset: "logos.test"
    }

    function init() {
        state.connectorConfig = ({})
        state.blockchainSourceMode = "rpc"
        state.messagingSourceMode = "rest"
        state.storageSourceMode = "rest"
        state.nodeUrl = "http://node"
        state.storageRestUrl = "http://storage"
        state.messagingRestUrl = "http://delivery"
        state.messagingStorePeerAddress = ""
    }

    function test_messaging_network_preset_normalization_is_owned_by_source_routing() {
        compare(state.normalizedMessagingNetworkPreset(""), "logos.test")
        compare(state.normalizedMessagingNetworkPreset(" testnet "), "logos.test")
        compare(state.normalizedMessagingNetworkPreset(" custom.network "), "custom.network")
    }

    function test_storage_network_preset_normalization_is_owned_by_source_routing() {
        compare(state.normalizedStorageNetworkPreset(""), "logos.test")
        compare(state.normalizedStorageNetworkPreset(" testnet "), "logos.test")
        compare(state.normalizedStorageNetworkPreset(" custom.network "), "custom.network")

        state.storageNetworkPreset = ""
        compare(state.storageSourceView().networkPreset, "logos.test")
        state.storageNetworkPreset = "custom.network"
        compare(state.storageSourceView().networkPreset, "custom.network")
    }

    function test_connector_config_is_storage_source_of_truth() {
        state.connectorConfig = ({
            scopes: {
                storage: {
                    connector_id: "storage_module",
                    provenance: "network_profile"
                }
            }
        })

        const view = state.storageSourceView()

        compare(state.storageSourceMode, "rest")
        compare(view.configuredMode, "rest")
        compare(view.connector.connector_id, "storage_module")
        compare(view.effectiveMode, "module")
        compare(view.target, "storage_module")
    }

    function test_untagged_blockchain_module_event_authority_uses_canonical_connector() {
        state.connectorConfig = ({
            scopes: {
                l1: {
                    connector_id: "direct_l1_rpc",
                    provenance: "test"
                }
            }
        })
        verify(!state.acceptsUntaggedBlockchainModuleEvents())
        verify(!state.acceptsTrustedLogoscoreCliBlockchainEvents())

        state.connectorConfig = ({
            scopes: {
                l1: {
                    connector_id: "blockchain_module",
                    provenance: "test"
                }
            }
        })
        verify(state.acceptsUntaggedBlockchainModuleEvents())
        verify(!state.acceptsTrustedLogoscoreCliBlockchainEvents())

        state.connectorConfig = ({
            scopes: {
                l1: {
                    connector_id: "logoscore_cli_blockchain_module",
                    provenance: "test"
                }
            }
        })
        verify(!state.acceptsUntaggedBlockchainModuleEvents())
        verify(state.acceptsTrustedLogoscoreCliBlockchainEvents())
    }

    function test_source_report_view_does_not_change_configured_connector() {
        state.connectorConfig = ({
            scopes: {
                storage: {
                    connector_id: "direct_storage_rest",
                    endpoint: "http://configured-storage",
                    provenance: "network_profile"
                }
            }
        })

        const before = state.connectorScope("storage").connector_id
        const report = state.storageReportView({
            health: {
                status: "unavailable",
                reachable: false
            }
        })

        compare(before, "direct_storage_rest")
        compare(report.ready, false)
        compare(state.connectorScope("storage").connector_id, "direct_storage_rest")
        compare(state.configuredStorageRestUrl(), "http://configured-storage")
    }

    function test_healthless_source_report_uses_its_own_probe_evidence() {
        const report = state.storageReportView({
            module: "storage_module",
            module_info: {
                ok: true,
                value: {},
                error: null
            },
            probes: [{
                probe_key: "version",
                label: "storage_module.version",
                ok: true,
                value: "1.2.3",
                error: null
            }]
        })

        verify(report.reachable)
        verify(report.ready)
        compare(report.summary, "version 1.2.3")
    }

    function test_source_family_view_combines_route_and_report_facts() {
        state.connectorConfig = ({
            scopes: {
                storage: {
                    connector_id: "direct_storage_rest",
                    endpoint: "http://configured-storage",
                    provenance: "network_profile"
                }
            }
        })

        const view = state.sourceFamilyView("storage", "", {
            health: {
                status: "ready",
                ready: true,
                reachable: true,
                summary: "storage ready"
            },
            capability_facts: [{
                key: "storage.content.read_by_cid",
                available: true,
                evidence: "probe"
            }]
        })

        compare(view.family, "storage")
        compare(view.route.connector.connector_id, "direct_storage_rest")
        compare(view.connector.connector_id, "direct_storage_rest")
        compare(view.effectiveMode, "rest")
        compare(view.ready, true)
        compare(view.report.ready, true)
        compare(view.capabilityAvailable("storage.content.read_by_cid"), true)
        compare(view.capabilityEvidence("storage.content.read_by_cid"), "probe")
        compare(state.connectorScope("storage").connector_id, "direct_storage_rest")
    }

    function test_source_mode_descriptor_reports_adapter_traits() {
        const storageModule = state.sourceModeDescriptor("storage", "module")
        compare(storageModule.key, "module")
        compare(storageModule.effective, "module")
        compare(storageModule.target, "module")
        compare(storageModule.supportsCidProbe, true)
        compare(storageModule.supportsMutatingDiagnostics, true)
        compare(storageModule.usesRestEndpoint, false)
        compare(storageModule.connectorId, "storage_module")
        compare(storageModule.inputs.length, 0)

        const deliveryMetrics = state.sourceModeDescriptor("delivery", "metrics")
        compare(deliveryMetrics.key, "metrics")
        compare(deliveryMetrics.effective, "metrics")
        compare(deliveryMetrics.target, "metrics_endpoint")
        compare(deliveryMetrics.usesMetricsEndpoint, true)
        compare(deliveryMetrics.usesRestEndpoint, false)
        compare(deliveryMetrics.connectorId, "delivery_metrics")
        compare(deliveryMetrics.inputs.length, 1)
        compare(deliveryMetrics.inputs[0].key, "metrics_endpoint")
    }

    function test_module_source_args_do_not_include_rpc_input() {
        const args = state.coreSourceArgs("module", "http://unused", ["payload"])

        compare(args.length, 2)
        compare(args[0], "module")
        compare(args[1], "payload")
    }

    function test_logoscore_cli_storage_report_uses_structured_adapter_initialization() {
        state.connectorConfig = ({
            scopes: {
                storage: {
                    connector_id: "logoscore_cli_storage_module",
                    provenance: "build_default"
                }
            }
        })
        const sourceMode = state.connectorSourceMode("storage", state.storageSourceMode)

        const args = state.storageSourceReportArgs(
            sourceMode,
            "http://unused-storage",
            "http://unused-metrics",
            "cid-test",
            true,
            true
        )

        compare(sourceMode, "logoscore_cli")
        compare(args.length, 1)
        compare(args[0].source_mode, "logoscore_cli")
        compare(Object.keys(args[0].inputs).length, 0)
        compare(args[0].options.cid, "cid-test")
        compare(args[0].options.privileged_debug_enabled, true)
    }

    function test_logoscore_cli_delivery_report_includes_health_endpoint() {
        state.messagingStorePeerAddress = "/dns4/provider.example/tcp/30303/p2p/peer"
        state.connectorConfig = ({
            scopes: {
                delivery: {
                    connector_id: "logoscore_cli_delivery_module",
                    provenance: "build_default"
                }
            }
        })
        const sourceMode = state.connectorSourceMode(
            "delivery", state.messagingSourceMode)
        const source = state.sourceModeDescriptor("delivery", sourceMode)
        const args = state.deliverySourceReportArgs(
            sourceMode,
            "http://delivery",
            "http://unused-metrics"
        )

        compare(sourceMode, "logoscore_cli")
        compare(source.usesRestEndpoint, false)
        compare(source.inputs.length, 1)
        compare(state.deliverySourceView().usesHealthEndpoint, true)
        verify(state.deliverySourceView().capabilities.indexOf(
            "delivery.store.query") >= 0)
        verify(state.deliverySourceView().capabilities.indexOf(
            "delivery.topics.read") < 0)
        compare(args.length, 1)
        compare(args[0].source_mode, "logoscore_cli")
        compare(args[0].inputs.rest_endpoint, undefined)
        compare(args[0].inputs.metrics_endpoint, undefined)
        compare(args[0].inputs.store_peer_addr,
                "/dns4/provider.example/tcp/30303/p2p/peer")
        compare(args[0].options.health_endpoint, "http://delivery")

        const actionArgs = state.deliverySourceView().actionArgs(["topic", "payload"])
        compare(actionArgs.length, 3)
        compare(actionArgs[0], "module")
        compare(actionArgs[1], "topic")
        compare(actionArgs[2], "payload")

        state.messagingRestUrl = ""
        const clearedArgs = state.deliverySourceReportArgs()
        compare(clearedArgs[0].source_mode, "logoscore_cli")
        compare(clearedArgs[0].inputs.rest_endpoint, undefined)
        compare(clearedArgs[0].inputs.store_peer_addr,
                "/dns4/provider.example/tcp/30303/p2p/peer")
        compare(clearedArgs[0].options.health_endpoint, "")
    }

}
