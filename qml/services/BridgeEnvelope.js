.pragma library
.import "BridgeHelpers.js" as BridgeHelpers

function prefersBasecampModules(host) {
    return host && host["callModule"] && !host["callModuleJson"]
}

function hasAsyncCalls(host) {
    return !!(host
        && (host["callModuleJsonAsync"]
            || (prefersBasecampModules(host) && host["callModuleAsync"])))
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
            return parseBasecampResponseJson(raw)
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
    if (host["callModuleJsonAsync"]) {
        try {
            host["callModuleJsonAsync"](requestId, moduleName, method, JSON.stringify(args || []))
            return true
        } catch (error) {
            finish(callError(error))
            return true
        }
    }

    let completed = false
    const complete = function (response) {
        if (completed) {
            return
        }
        completed = true
        finish(response)
    }
    try {
        if (moduleName === "logos_inspector" && method !== "moduleVersion") {
            host["callModuleAsync"](
                moduleName,
                "call",
                [method, JSON.stringify(args || [])],
                function (responseJson) {
                    complete(parseBasecampResponseJson(responseJson))
                }
            )
        } else {
            host["callModuleAsync"](moduleName, method, args || [], function (responseJson) {
                complete(parseBasecampResponseJson(responseJson))
            })
        }
        return true
    } catch (error) {
        complete(callError(error))
        return true
    }
}

function parseResponseJson(responseJson) {
    return BridgeHelpers.parseModuleResponseJson(responseJson)
}

function parseBasecampResponseJson(responseJson) {
    const decoded = BridgeHelpers.parseJson(responseJson)
    if (decoded.ok
            && decoded.value
            && typeof decoded.value === "object"
            && !Array.isArray(decoded.value)
            && typeof decoded.value.ok !== "boolean"
            && typeof decoded.value.error === "string") {
        return {
            ok: false,
            value: null,
            text: "",
            error: "Logos bridge call failed: " + decoded.value.error
        }
    }
    return BridgeHelpers.parseModuleResponseJson(responseJson)
}

function callError(error) {
    return {
        ok: false,
        text: "",
        error: "Logos bridge call failed: " + BridgeHelpers.errorMessage(error)
    }
}
