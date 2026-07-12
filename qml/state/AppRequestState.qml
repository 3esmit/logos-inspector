import QtQml

QtObject {
    id: root

    required property var bridge
    required property var shell
    required property string inspectorModule

    property var updateDashboardCache: null
    property var updateNetworkConnectionStatus: null

    function callInspector(method, args, label) {
        return callModule(inspectorModule, method, args, label)
    }

    function callModule(moduleName, method, args, label) {
        return requestModule(moduleName, method, args, label, true)
    }

    function requestModule(moduleName, method, args, label, showResult, cacheResult) {
        if (shell.busy) {
            return {
                ok: false,
                text: "",
                error: qsTr("Another inspection is already running.")
            }
        }

        const target = targetCall(moduleName, method, args)
        shell.busy = true
        shell.statusText = String(label || "")
        const response = bridge.callModule(target.moduleName, target.method, target.args)
        shell.busy = false

        handleResponse(method, label, response, showResult === true, cacheResult !== false)
        return response
    }

    function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) {
        const target = targetCall(moduleName, method, args)

        if (showResult) {
            shell.statusText = String(label || "")
        }

        return bridge.callModuleAsync(target.moduleName, target.method, target.args, function (response) {
            if (acceptResponse && !acceptResponse(response)) {
                return
            }
            handleResponse(method, label, response, showResult === true, true)
            if (callback) {
                callback(response)
            }
        })
    }

    function targetCall(moduleName, method, args) {
        const targetModule = moduleName === inspectorModule ? moduleName : inspectorModule
        const targetMethod = moduleName === inspectorModule ? method : "callModule"
        const targetArgs = moduleName === inspectorModule ? args : [moduleName, method, args || []]
        return {
            moduleName: targetModule,
            method: targetMethod,
            args: targetArgs
        }
    }

    function handleResponse(method, label, response, showResult, cacheResult) {
        if (response && response.ok) {
            if (cacheResult && updateDashboardCache) {
                updateDashboardCache(method, response.value)
            }
            if (showResult) {
                shell.setResult(label, response.text, false, response.value)
            }
        } else if (showResult) {
            shell.setResult(label, response ? response.error : qsTr("Bridge call failed."), true, null)
        }
        if (updateNetworkConnectionStatus) {
            updateNetworkConnectionStatus(method, response)
        }
    }
}
