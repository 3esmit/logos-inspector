import QtQml

QtObject {
    id: root

    required property var bridge
    required property var shell
    required property string inspectorModule

    property var updateDashboardCache: null
    property var updateNetworkConnectionStatus: null
    property int nextAsyncGeneration: 1
    property var latestAsyncGenerationByMethod: ({})
    property int activePresentationGeneration: 0
    readonly property bool presentationBusy: activePresentationGeneration !== 0

    function callInspector(method, args, label) {
        return callModule(inspectorModule, method, args, label)
    }

    function callInspectorAsync(method, args, label, callback, acceptResponse) {
        return requestModuleAsync(
            inspectorModule,
            method,
            args,
            label,
            true,
            callback,
            acceptResponse
        )
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
        const resultOwner = String(shell.currentView || "")
        shell.busy = true
        shell.statusText = String(label || "")
        const response = bridge.callModule(target.moduleName, target.method, target.args)
        shell.busy = false

        handleResponse(
            method,
            label,
            response,
            showResult === true,
            cacheResult !== false,
            resultOwner
        )
        return response
    }

    function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) {
        const target = targetCall(moduleName, method, args)
        const methodKey = JSON.stringify([
            String(moduleName || ""),
            String(method || "")
        ])
        const generation = nextAsyncGeneration
        const presentationOwner = showResult === true ? String(shell.currentView || "") : ""
        nextAsyncGeneration += 1
        const generations = copyAsyncGenerations()
        generations[methodKey] = generation
        latestAsyncGenerationByMethod = generations

        if (showResult) {
            activePresentationGeneration = generation
            shell.statusText = String(label || "")
        }

        return bridge.callModuleAsync(target.moduleName, target.method, target.args, function (response) {
            const ownsPresentation = showResult === true
                && activePresentationGeneration === generation
            const ownsMethod = latestAsyncGenerationByMethod[methodKey] === generation
            if (ownsPresentation) {
                activePresentationGeneration = 0
            }
            if (acceptResponse && !acceptResponse(response)) {
                return
            }
            if (ownsPresentation || ownsMethod) {
                handleResponse(
                    method,
                    label,
                    response,
                    ownsPresentation,
                    ownsMethod,
                    presentationOwner
                )
            }
            if (callback) {
                callback(response)
            }
        })
    }

    function copyAsyncGenerations() {
        const copy = {}
        const current = latestAsyncGenerationByMethod || {}
        for (const method in current) {
            copy[method] = current[method]
        }
        return copy
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

    function handleResponse(method, label, response, showResult, cacheResult, resultOwner) {
        if (response && response.ok) {
            if (cacheResult && updateDashboardCache) {
                updateDashboardCache(method, response.value)
            }
            if (showResult) {
                shell.setResult(label, response.text, false, response.value, resultOwner)
            }
        } else if (showResult) {
            shell.setResult(
                label,
                response ? response.error : qsTr("Bridge call failed."),
                true,
                null,
                resultOwner
            )
        }
        if (updateNetworkConnectionStatus) {
            updateNetworkConnectionStatus(method, response)
        }
    }
}
