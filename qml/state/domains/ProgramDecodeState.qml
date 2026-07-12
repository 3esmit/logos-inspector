import QtQuick
import ".."

QtObject {
    id: root

    required property QtObject registryGateway
    property var capabilityFacade: null

    property IdlRegistryState idlRegistry: IdlRegistryState {
        id: idlRegistryState

        gateway: root.registryGateway
    }

    property alias registeredIdls: idlRegistryState.registeredIdls
    property alias loaded: idlRegistryState.loaded
    property var accountIdlSelections: ({})
    property int accountIdlSelectionRevision: 0
    property var knownProgramIds: ({})
    property int knownProgramIdsRevision: 0
    property int accountAutoDecodeSerial: 0

    function decodeGate(requiredInputs) {
        if (capabilityFacade && typeof capabilityFacade.programDecodeGate === "function") {
            return capabilityFacade.programDecodeGate({
                required_inputs: Array.isArray(requiredInputs) ? requiredInputs : []
            })
        }
        return {
            enabled: true,
            status: "enabled",
            missing: [],
            warnings: [],
            provenance: ["program_decode_static"]
        }
    }
}
