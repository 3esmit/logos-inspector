.pragma library

function parseModuleResponseJson(raw) {
    try {
        const parsed = JSON.parse(raw)
        const unwrapped = unwrapBridgeResponse(parsed)
        if (unwrapped !== null) {
            return unwrapped
        }
        return {
            ok: true,
            value: parsed,
            text: formatValue(parsed),
            error: ""
        }
    } catch (error) {
        return {
            ok: false,
            text: "",
            error: "Logos bridge call failed: " + errorMessage(error)
        }
    }
}

function unwrapBridgeResponse(value) {
    if (value && typeof value.ok === "boolean") {
        return value
    }
    if (typeof value !== "string") {
        return null
    }

    try {
        const parsed = JSON.parse(value)
        if (parsed && typeof parsed.ok === "boolean") {
            return parsed
        }
    } catch (error) {
        return null
    }
    return null
}

function missingBridge() {
    return {
        ok: false,
        text: "",
        error: "Logos bridge not available. Run this QML UI from Logos Basecamp or the standalone host."
    }
}

function formatValue(value) {
    if (value === undefined || value === null) {
        return "No value returned"
    }
    if (typeof value === "string") {
        return value
    }

    try {
        return JSON.stringify(value, null, 2)
    } catch (error) {
        return String(value)
    }
}

function parseJson(text) {
    try {
        return {
            ok: true,
            value: JSON.parse(text),
            error: ""
        }
    } catch (error) {
        return {
            ok: false,
            value: null,
            error: errorMessage(error)
        }
    }
}

function errorMessage(error) {
    return error && error.message ? error.message : String(error)
}
