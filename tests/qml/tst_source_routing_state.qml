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
        indexerModule: "lez_indexer_module"
        deliveryModule: "delivery_module"
        storageModule: "storage_module"
        blockchainSourceMode: "rpc"
        indexerSourceMode: "rpc"
        executionSourceMode: "rpc"
        messagingSourceMode: "rest"
        storageSourceMode: "rest"
        nodeUrl: "http://node"
        indexerUrl: "http://indexer"
        sequencerUrl: "http://sequencer"
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
        state.indexerSourceMode = "rpc"
        state.executionSourceMode = "rpc"
        state.messagingSourceMode = "rest"
        state.storageSourceMode = "rest"
        state.nodeUrl = "http://node"
        state.indexerUrl = "http://indexer"
        state.sequencerUrl = "http://sequencer"
        state.storageRestUrl = "http://storage"
        state.messagingRestUrl = "http://delivery"
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

    function test_lez_indexer_and_sequencer_routes_use_split_connectors() {
        state.connectorConfig = ({
            scopes: {
                "lez.indexer": {
                    connector_id: "lez_indexer_module",
                    provenance: "network_profile"
                },
                "lez.sequencer": {
                    connector_id: "direct_sequencer_rpc",
                    endpoint: "http://configured-sequencer",
                    provenance: "network_profile"
                }
            }
        })

        const indexer = state.coreSourceView("indexer")
        const sequencer = state.coreSourceView("execution")

        compare(indexer.connector.connector_id, "lez_indexer_module")
        compare(indexer.effectiveMode, "module")
        compare(sequencer.connector.connector_id, "direct_sequencer_rpc")
        compare(sequencer.effectiveMode, "rpc")
        compare(sequencer.endpoint, "http://configured-sequencer")
        compare(state.lezArgs("tx-1")[0], "rpc")
        compare(state.lezArgs("tx-1")[1], "http://configured-sequencer")
        compare(state.lezArgs("tx-1")[2], "module")
    }
}
