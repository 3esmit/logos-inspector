import QtQuick
import ".."

QtObject {
    id: root

    required property QtObject registryGateway

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
    property int transactionAutoDecodeSerial: 0
    property int searchResolveSerial: 0
    property int programOpenSerial: 0
}
