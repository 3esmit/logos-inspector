import QtQuick
import "BridgeHelpers.js" as BridgeHelpers

QtObject {
    id: root

    property var host: null
    property int nextRequestId: 1
    property var pendingCalls: ({})

    function prefersBasecampModules() {
        return root.host && root.host["callModule"] && !root.host["callModuleJson"]
    }

    function callModule(moduleName, method, args) {
        if (!root.host) {
            return BridgeHelpers.missingBridge()
        }
        if (root.host["callModuleJson"]) {
            return BridgeHelpers.callModuleJson(root.host, moduleName, method, args || [])
        }
        return BridgeHelpers.callModule(root.host, moduleName, method, args || [])
    }

    function hasAsyncCalls() {
        return root.host && root.host["callModuleJsonAsync"]
    }

    function callModuleAsync(moduleName, method, args, callback) {
        const requestId = root.nextRequestId
        root.nextRequestId += 1
        const pending = root.copyPendingCalls()
        pending[requestId] = callback
        root.pendingCalls = pending

        if (root.host && root.host["callModuleJsonAsync"]) {
            try {
                root.host["callModuleJsonAsync"](requestId, moduleName, method, JSON.stringify(args || []))
                return requestId
            } catch (error) {
                root.finishAsyncCall(requestId, {
                    ok: false,
                    text: "",
                    error: "Logos bridge call failed: " + BridgeHelpers.errorMessage(error)
                })
                return requestId
            }
        }

        Qt.callLater(function () {
            root.finishAsyncCall(requestId, root.callModule(moduleName, method, args || []))
        })
        return requestId
    }

    function finishAsyncCall(requestId, response) {
        const pending = root.copyPendingCalls()
        const callback = pending[requestId]
        if (!callback) {
            return
        }
        delete pending[requestId]
        root.pendingCalls = pending
        callback(response)
    }

    function copyPendingCalls() {
        const copy = {}
        const current = root.pendingCalls || {}
        for (const key in current) {
            copy[key] = current[key]
        }
        return copy
    }

    property Connections hostConnections: Connections {
        target: root.host
        ignoreUnknownSignals: true

        function onModuleCallFinished(requestId, responseJson) {
            root.finishAsyncCall(requestId, BridgeHelpers.parseModuleResponseJson(responseJson))
        }
    }
}
