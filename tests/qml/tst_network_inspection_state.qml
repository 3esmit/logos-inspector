import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "NetworkInspectionState"

    Domains.CapabilityGateState {
        id: gates
    }

    QtObject {
        id: gateway

        function requestModule() { return null }
        function requestModuleAsync() { return null }
        function setResult() {}
        function blockchainArgs(extra) { return extra || [] }
        function indexerArgs(extra) { return extra || [] }
        function executionArgs(extra) { return extra || [] }
        function blockchainRpcArgs(extra) { return extra || [] }
        function networkConnectionState() { return ({}) }
        function valueToString(value) { return String(value) }
        function canonicalProgramIdHex(value) { return String(value || "") }
        function normalizedHexText(value) { return String(value || "") }
    }

    Domains.NetworkInspectionState {
        id: state

        gateway: gateway
        capabilityFacade: gates
    }

    function init() {
        gates.compatibilityAvailability = ({})
        gates.registryLoaded = true
        gates.registryReport = ({ schema_version: 1, capabilities: [] })
    }

    function test_indexer_and_sequencer_gates_are_independent() {
        gates.registryReport = ({
            schema_version: 1,
            capabilities: [
                {
                    key: "lez.indexer",
                    label: "LEZ Indexer",
                    status: "unavailable",
                    sub_capabilities: ["lez.indexer.blocks.finalized.read"]
                },
                {
                    key: "lez.sequencer",
                    label: "LEZ Sequencer",
                    status: "available",
                    sub_capabilities: ["lez.sequencer.blocks.pending.read"]
                }
            ]
        })

        verify(!state.indexerGate().enabled)
        verify(state.sequencerGate().enabled)
        verify(state.targetResolutionGate().enabled)
    }
}
