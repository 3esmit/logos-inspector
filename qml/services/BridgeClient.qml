import QtQuick
import "BridgeHelpers.js" as BridgeHelpers

QtObject {
    id: root

    property QtObject host: null

    function callModule(moduleName, method, args) {
        if (!root.host) {
            return BridgeHelpers.missingBridge()
        }
        if (root.host.callModuleJson) {
            return BridgeHelpers.callModuleJson(root.host, moduleName, method, args || [])
        }
        return BridgeHelpers.callModule(root.host, moduleName, method, args || [])
    }
}
