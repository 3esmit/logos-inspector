import QtQuick
import "BridgeEnvelope.js" as BridgeEnvelope

QtObject {
    id: root

    property var host: null
    property int nextRequestId: 1
    property var pendingCalls: ({})
    property var moduleEventSubscriptions: ({})

    signal moduleEventReceived(string moduleName, string eventName, var args)

    function prefersBasecampModules() {
        return BridgeEnvelope.prefersBasecampModules(root.host)
    }

    function callModule(moduleName, method, args) {
        return BridgeEnvelope.callModule(root.host, moduleName, method, args || [])
    }

    function hasAsyncCalls() {
        return BridgeEnvelope.hasAsyncCalls(root.host)
    }

    function callModuleAsync(moduleName, method, args, callback) {
        const requestId = root.nextRequestId
        root.nextRequestId += 1
        const pending = root.copyPendingCalls()
        pending[requestId] = callback
        root.pendingCalls = pending

        if (BridgeEnvelope.dispatchAsync(root.host, requestId, moduleName, method, args || [], function (response) {
                root.finishAsyncCall(requestId, response)
            })) {
            return requestId
        }

        Qt.callLater(function () {
            root.finishAsyncCall(requestId, root.callModule(moduleName, method, args || []))
        })
        return requestId
    }

    function subscribeModuleEvents(moduleName, events) {
        let count = 0
        const rows = Array.isArray(events) ? events : []
        for (let i = 0; i < rows.length; ++i) {
            if (root.subscribeModuleEvent(moduleName, rows[i])) {
                count += 1
            }
        }
        return count
    }

    function subscribeModuleEvent(moduleName, eventName) {
        const moduleText = String(moduleName || "")
        const eventText = String(eventName || "")
        if (!root.host || !moduleText.length || !eventText.length) {
            return false
        }
        const key = moduleText + "::" + eventText
        const current = root.moduleEventSubscriptions || {}
        if (current[key] && current[key].active === true) {
            return true
        }
        const callback = function () {
            const values = []
            for (let i = 0; i < arguments.length; ++i) {
                values.push(arguments[i])
            }
            root.moduleEventReceived(moduleText, eventText, values.length === 1 ? values[0] : values)
        }
        let active = false
        try {
            if (root.host && root.host["onModuleEvent"]) {
                root.host["onModuleEvent"](moduleText, eventText, callback)
                active = true
            }
        } catch (error) {
            active = false
        }
        try {
            if (!active && root.host && root.host["module"]) {
                const moduleObject = root.host["module"](moduleText)
                if (moduleObject && moduleObject["on"]) {
                    moduleObject["on"](eventText, callback)
                    active = true
                }
            }
        } catch (error) {
            active = false
        }
        if (!active) {
            return false
        }
        const next = {}
        for (const name in current) {
            next[name] = current[name]
        }
        next[key] = {
            active: true,
            callback: callback
        }
        root.moduleEventSubscriptions = next
        return true
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
            root.finishAsyncCall(requestId, BridgeEnvelope.parseResponseJson(responseJson))
        }

        function onModuleEvent(moduleName, eventName, args) {
            root.moduleEventReceived(String(moduleName || ""), String(eventName || ""), args)
        }
    }
}
