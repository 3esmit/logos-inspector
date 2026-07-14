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

function probeBasecampInspectorAsyncBridge(host) {
    if (!prefersBasecampModules(host) || !host["callModule"]) {
        return {
            status: "absent",
            schema: "",
            error: ""
        }
    }
    try {
        const raw = host["callModule"]("logos_inspector", "asyncBridgeSchema", [])
        const response = parseBasecampDirectResponseJson(
            raw,
            "logos_inspector",
            "asyncBridgeSchema"
        )
        if (response.ok === true && typeof response.value === "string") {
            const schema = response.value
            return {
                status: schema.length ? "present" : "absent",
                schema: schema,
                error: ""
            }
        }
        if (missingBasecampAsyncBridge(response)) {
            return {
                status: "absent",
                schema: "",
                error: ""
            }
        }
        return {
            status: "probe_failed",
            schema: "",
            error: response && response.error
                ? String(response.error)
                : "unknown capability response"
        }
    } catch (error) {
        return {
            status: "probe_failed",
            schema: "",
            error: BridgeHelpers.errorMessage(error)
        }
    }
}

function basecampInspectorOwnsRuntimeModuleEvents(host) {
    if (!prefersBasecampModules(host) || !host["callModule"]) {
        return false
    }
    try {
        const compatibilityValue = host["logosInspectorOwnsRuntimeModuleEvents"]
        if (typeof compatibilityValue !== "undefined") {
            return compatibilityValue === true
        }
        const raw = host["callModule"](
            "logos_inspector",
            "logosInspectorOwnsRuntimeModuleEvents",
            []
        )
        const response = parseBasecampDirectResponseJson(
            raw,
            "logos_inspector",
            "logosInspectorOwnsRuntimeModuleEvents"
        )
        return response.ok === true && response.value === true
    } catch (error) {
        return false
    }
}

function usesBasecampInspectorPolling(host, moduleName, method, expectedSchema, reportedSchema) {
    return !!(prefersBasecampModules(host)
        && host["callModuleAsync"]
        && String(reportedSchema || "") === String(expectedSchema || "")
        && moduleName === "logos_inspector"
        && method !== "moduleVersion")
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
            return parseBasecampInspectorResponseJson(raw)
        }
        const raw = host.callModule(moduleName, method, args || [])
        return parseBasecampDirectResponseJson(
            raw,
            moduleName,
            method
        )
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
    if (moduleName === "logos_inspector" && method !== "moduleVersion") {
        return false
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
        host["callModuleAsync"](moduleName, method, args || [], function (responseJson) {
            complete(parseBasecampDirectResponseJson(
                responseJson,
                moduleName,
                method
            ))
        })
        return true
    } catch (error) {
        complete(callError(error))
        return true
    }
}

function beginBasecampInspectorCall(host, correlationId, method, argsJson, timeoutMs, finish) {
    return dispatchBasecampControl(
        host,
        "callAsync",
        [String(correlationId || ""), String(method || ""), String(argsJson || "[]")],
        timeoutMs,
        finish
    )
}

function pollBasecampInspectorCall(host, token, timeoutMs, finish) {
    return dispatchBasecampControl(
        host,
        "pollAsync",
        [String(token || "")],
        timeoutMs,
        finish
    )
}

function cancelBasecampInspectorCall(host, token, timeoutMs, finish) {
    return dispatchBasecampControl(
        host,
        "cancelAsync",
        [String(token || "")],
        timeoutMs,
        finish
    )
}

function releaseBasecampInspectorCall(host, token, timeoutMs, finish) {
    return dispatchBasecampControl(
        host,
        "releaseAsync",
        [String(token || "")],
        timeoutMs,
        finish
    )
}

function dispatchBasecampControl(host, method, args, timeoutMs, finish) {
    if (!prefersBasecampModules(host) || !host["callModuleAsync"]) {
        return false
    }
    let completed = false
    const complete = function (response) {
        if (completed) {
            return
        }
        completed = true
        if (typeof finish === "function") {
            finish(response)
        }
    }
    try {
        host["callModuleAsync"](
            "logos_inspector",
            method,
            args || [],
            function (responseJson) {
                complete(parseBasecampInspectorResponseJson(responseJson))
            },
            Number(timeoutMs || 0)
        )
        return true
    } catch (error) {
        complete(callError(error))
        return true
    }
}

function missingBasecampAsyncBridge(response) {
    return !!(response
        && response.ok !== true
        && String(response.error || "").indexOf("Invalid response") >= 0)
}

function retryableBasecampPollError(response) {
    return !!(response
        && response.ok !== true
        && String(response.error || "").toLowerCase().indexOf("timeout") >= 0)
}

function parseResponseJson(responseJson) {
    return BridgeHelpers.parseModuleResponseJson(responseJson)
}

function parseBasecampInspectorResponseJson(responseJson) {
    const decoded = BridgeHelpers.parseJson(responseJson)
    if (decoded.ok && isHostErrorObject(decoded.value)) {
        return hostErrorResponse(decoded.value.error)
    }
    return BridgeHelpers.parseModuleResponseJson(responseJson)
}

function parseBasecampDirectResponseJson(responseJson, moduleName, method) {
    const decoded = BridgeHelpers.parseJson(responseJson)
    if (!decoded.ok) {
        return hostErrorResponse("invalid response JSON: " + decoded.error)
    }
    if (isReservedHostError(decoded.value, moduleName, method)) {
        return hostErrorResponse(decoded.value.error)
    }
    return directResponse(decoded.value)
}

function directResponse(value) {
    return {
        ok: true,
        value: value,
        text: BridgeHelpers.formatValue(value),
        error: ""
    }
}

function hostErrorResponse(error) {
    return {
        ok: false,
        value: null,
        text: "",
        error: "Logos bridge call failed: " + String(error || "unknown host error")
    }
}

function isHostErrorObject(value) {
    return value
        && typeof value === "object"
        && !Array.isArray(value)
        && typeof value.ok !== "boolean"
        && typeof value.error === "string"
}

function isReservedHostError(value, moduleName, method) {
    if (!isHostErrorObject(value)) {
        return false
    }
    const keys = Object.keys(value).sort()
    if (keys.length === 3
            && keys[0] === "error"
            && keys[1] === "method"
            && keys[2] === "module") {
        return value.error === "timeout"
            && String(value.module || "") === String(moduleName || "")
            && String(value.method || "") === String(method || "")
    }
    if (keys.length !== 1 || keys[0] !== "error") {
        return false
    }
    return value.error === "view modules must be called via logos.module()"
        || value.error === "LogosAPI not available"
        || value.error === "Module not connected"
        || value.error === "Invalid response"
}

function callError(error) {
    return {
        ok: false,
        text: "",
        error: "Logos bridge call failed: " + BridgeHelpers.errorMessage(error)
    }
}
