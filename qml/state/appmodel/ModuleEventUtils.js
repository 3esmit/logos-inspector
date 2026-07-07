.import "../../services/BridgeHelpers.js" as BridgeHelpers

function eventValues(args) {
    if (Array.isArray(args)) {
        return args
    }
    if (args === undefined || args === null) {
        return []
    }
    return [args]
}

function firstEventValue(args) {
    const values = eventValues(args)
    return values.length > 0 ? values[0] : args
}

function eventObject(args) {
    const value = firstEventValue(args)
    const parsed = parsedPayload(value)
    return parsed && typeof parsed === "object" && !Array.isArray(parsed) ? parsed : ({})
}

function parsedPayload(value) {
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

function fieldText(object, keys) {
    const row = object || {}
    for (let i = 0; i < keys.length; ++i) {
        const value = row[keys[i]]
        if (value !== undefined && value !== null && String(value).length > 0) {
            return String(value)
        }
    }
    return ""
}

function eventTimeText(timestamp) {
    const text = String(timestamp || "").trim()
    if (text.length > 0 && /^[0-9]+$/.test(text)) {
        const number = Number(text)
        if (Number.isFinite(number) && number > 0) {
            return Qt.formatTime(new Date(number > 100000000000 ? number : number * 1000), "HH:mm:ss")
        }
    }
    return Qt.formatTime(new Date(), "HH:mm:ss")
}

function payloadSummary(payload) {
    const value = parsedPayload(payload)
    if (value && typeof value === "object") {
        return String(value.kind || BridgeHelpers.formatValue(value)).replace(/\s+/g, " ").slice(0, 80)
    }
    return shortText(value, 80)
}

function compactParts(parts) {
    const result = []
    for (let i = 0; i < parts.length; ++i) {
        const text = String(parts[i] || "")
        if (text.length > 0) {
            result.push(text)
        }
    }
    return result
}

function shortText(value, max) {
    const text = String(value || "")
    const limit = Math.max(8, Number(max || 32))
    if (text.length <= limit) {
        return text
    }
    return text.slice(0, limit - 1) + "..."
}

function valueContainsText(value, needle) {
    const target = String(needle || "").trim().toLowerCase()
    if (!target.length) {
        return false
    }
    const text = typeof value === "string" ? value : BridgeHelpers.formatValue(value)
    return String(text || "").toLowerCase().indexOf(target) >= 0
}
