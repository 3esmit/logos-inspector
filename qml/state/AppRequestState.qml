import QtQml

QtObject {
    id: root

    required property var bridge
    required property var shell
    required property string inspectorModule

    property var updateDashboardCache: null
    property var updateNetworkConnectionStatus: null
    property var projectObservationResponse: null
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
        return requestModuleAsyncWithProjection(
            moduleName,
            method,
            args,
            label,
            showResult,
            callback,
            acceptResponse,
            true
        )
    }

    function requestModuleAsyncUnobserved(moduleName, method, args, label, showResult,
            callback, acceptResponse) {
        return requestModuleAsyncWithProjection(
            moduleName,
            method,
            args,
            label,
            showResult,
            callback,
            acceptResponse,
            false
        )
    }

    function requestModuleAsyncWithProjection(moduleName, method, args, label, showResult,
            callback, acceptResponse, projectResponse) {
        const target = targetCall(moduleName, method, args)
        const methodKey = JSON.stringify([
            String(moduleName || ""),
            String(method || "")
        ])
        const generation = nextAsyncGeneration
        nextAsyncGeneration += 1
        const generations = copyAsyncGenerations()
        generations[methodKey] = generation
        latestAsyncGenerationByMethod = generations
        const presentation = showResult === true
            ? beginPresentationWithGeneration(generation, label, shell.currentView) : null

        return bridge.callModuleAsync(target.moduleName, target.method, target.args, function (response) {
            const ownsPresentation = finishPresentation(presentation)
            const ownsMethod = latestAsyncGenerationByMethod[methodKey] === generation
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
                    presentation ? presentation.owner : "",
                    projectResponse
                )
            }
            if (callback) {
                callback(response)
            }
        })
    }

    function beginPresentation(label, owner) {
        const generation = nextAsyncGeneration
        nextAsyncGeneration += 1
        return beginPresentationWithGeneration(generation, label, owner)
    }

    function beginPresentationWithGeneration(generation, label, owner) {
        const lease = {
            generation: Number(generation),
            owner: owner === undefined ? String(shell.currentView || "") : String(owner || ""),
            resultGeneration: Number(shell.resultGeneration || 0)
        }
        activePresentationGeneration = lease.generation
        shell.statusText = String(label || "")
        return lease
    }

    function presentationCurrent(lease) {
        return lease !== null && lease !== undefined
            && Number(lease.generation || 0) !== 0
            && activePresentationGeneration === Number(lease.generation)
            && Number(shell.resultGeneration || 0) === Number(lease.resultGeneration || 0)
    }

    function finishPresentation(lease) {
        if (!lease || activePresentationGeneration !== Number(lease.generation || 0)) {
            return false
        }
        const canPresent = Number(shell.resultGeneration || 0)
            === Number(lease.resultGeneration || 0)
        activePresentationGeneration = 0
        return canPresent
    }

    function completePresentation(lease, title, text, isError, value) {
        if (!finishPresentation(lease)) {
            return false
        }
        shell.setResult(title, text, isError === true, value, String(lease.owner || ""))
        return true
    }

    function abandonPresentation(lease) {
        if (!finishPresentation(lease)) {
            return false
        }
        shell.statusText = qsTr("Ready")
        return true
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

    function handleResponse(method, label, response, showResult, cacheResult, resultOwner,
            projectResponse) {
        if (response && response.ok) {
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
        if (projectResponse === false) {
            return
        }
        if (projectObservationResponse) {
            projectObservationResponse(method, response, cacheResult)
            return
        }
        if (response && response.ok && cacheResult && updateDashboardCache) {
            updateDashboardCache(method, response.value)
        }
        if (updateNetworkConnectionStatus) {
            updateNetworkConnectionStatus(method, response)
        }
    }
}
