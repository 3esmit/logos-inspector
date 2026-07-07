.pragma library
.import "BridgeHelpers.js" as BridgeHelpers

function prefersBasecampModules(host) {
    return host && host["callModule"] && !host["callModuleJson"]
}

function hasAsyncCalls(host) {
    return host && host["callModuleJsonAsync"]
}

function callModule(host, moduleName, method, args) {
    if (!host) {
        return BridgeHelpers.missingBridge()
    }
    if (host["callModuleJson"]) {
        return callModuleJson(host, moduleName, method, args || [])
    }
    return callBasecampModule(host, moduleName, method, args || [])
}

function callModuleJson(host, moduleName, method, args) {
    if (!host || !host["callModuleJson"]) {
        return BridgeHelpers.missingBridge()
    }

    try {
        const raw = host["callModuleJson"](moduleName, method, JSON.stringify(args || []))
        return BridgeHelpers.parseModuleResponseJson(raw)
    } catch (error) {
        return callError(error)
    }
}

function callBasecampModule(host, moduleName, method, args) {
    if (!host || !host.callModule) {
        return BridgeHelpers.missingBridge()
    }

    try {
        if (moduleName === "logos_inspector" && method !== "moduleVersion") {
            const raw = host.callModule(moduleName, "call", [method, JSON.stringify(args || [])])
            return BridgeHelpers.parseModuleResponseJson(raw)
        }
        const value = host.callModule(moduleName, method, args || [])
        return {
            ok: true,
            value: value,
            text: BridgeHelpers.formatValue(value),
            error: ""
        }
    } catch (error) {
        return callError(error)
    }
}

function dispatchAsync(host, requestId, moduleName, method, args, finish) {
    if (!hasAsyncCalls(host)) {
        return false
    }
    try {
        host["callModuleJsonAsync"](requestId, moduleName, method, JSON.stringify(args || []))
        return true
    } catch (error) {
        finish(callError(error))
        return true
    }
}

function parseResponseJson(responseJson) {
    return BridgeHelpers.parseModuleResponseJson(responseJson)
}

function callError(error) {
    return {
        ok: false,
        text: "",
        error: "Logos bridge call failed: " + BridgeHelpers.errorMessage(error)
    }
}
