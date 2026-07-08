.import "../../services/BridgeHelpers.js" as BridgeHelpers

function values(args) {
    if (Array.isArray(args)) {
        return args
    }
    if (args === undefined || args === null) {
        return []
    }
    return [args]
}

function first(args) {
    const rows = values(args)
    return rows.length > 0 ? rows[0] : args
}

function object(args) {
    const parsed = payload(first(args))
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : ({})
}

function payload(value) {
    if (value === undefined || value === null) {
        return null
    }
    if (typeof value === "object") {
        return value
    }
    const text = String(value || "").trim()
    if (!text.length) {
        return ""
    }
    if ((text.charAt(0) === "{" && text.charAt(text.length - 1) === "}")
            || (text.charAt(0) === "[" && text.charAt(text.length - 1) === "]")) {
        const parsed = BridgeHelpers.parseJson(text)
        if (parsed.ok) {
            return parsed.value
        }
    }
    return text
}

function storagePayload(args) {
    const raw = first(args)
    if (raw && typeof raw === "object") {
        return raw
    }
    const text = String(raw || "").trim()
    if (!text.length) {
        return null
    }
    const parsed = BridgeHelpers.parseJson(text)
    if (parsed.ok && parsed.value && typeof parsed.value === "object") {
        return parsed.value
    }
    return {
        value: text
    }
}
