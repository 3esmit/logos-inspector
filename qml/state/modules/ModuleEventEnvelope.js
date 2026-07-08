.import "../../services/BridgeHelpers.js" as BridgeHelpers

function fromRaw(moduleName, eventName, args) {
    const rows = values(args)
    const firstValue = first(rows)
    const parsed = payload(firstValue)
    return {
        __moduleEventEnvelope: true,
        moduleName: String(moduleName || ""),
        eventName: String(eventName || ""),
        args: rows,
        first: firstValue,
        payload: parsed,
        object: parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : ({}),
        storagePayload: storagePayload(rows)
    }
}

function isEnvelope(value) {
    return value && value.__moduleEventEnvelope === true
}

function values(args) {
    if (isEnvelope(args)) {
        return Array.isArray(args.args) ? args.args : []
    }
    if (Array.isArray(args)) {
        return args
    }
    if (args === undefined || args === null) {
        return []
    }
    return [args]
}

function first(args) {
    if (isEnvelope(args)) {
        return args.first
    }
    const rows = values(args)
    return rows.length > 0 ? rows[0] : args
}

function object(args) {
    if (isEnvelope(args)) {
        return args.object || {}
    }
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
    if (isEnvelope(args)) {
        return args.storagePayload
    }
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
