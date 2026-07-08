.pragma library

function optionObject(value) {
    return value !== null && typeof value === "object" && !Array.isArray(value)
}

function numberText(value, decimalsOrOptions) {
    const options = optionObject(decimalsOrOptions) ? decimalsOrOptions : {}
    const decimals = optionObject(decimalsOrOptions) ? options.decimals : decimalsOrOptions
    const empty = options.emptyText === undefined ? "-" : options.emptyText
    if (value === undefined || value === null || value === "") {
        return empty
    }
    if (options.coerceNumericStrings === true) {
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return numeric % 1 === 0 ? numeric.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
    }
    if (typeof value === "number") {
        return value.toLocaleString(Qt.locale(), "f", decimals === undefined ? 0 : decimals)
    }
    return String(value)
}

function valueText(value, emptyTextOrOptions) {
    const options = optionObject(emptyTextOrOptions) ? emptyTextOrOptions : {}
    const empty = optionObject(emptyTextOrOptions)
        ? (options.emptyText === undefined ? "-" : options.emptyText)
        : (emptyTextOrOptions === undefined ? "-" : emptyTextOrOptions)
    if (value === undefined || value === null || value === "") {
        return empty
    }
    if (options.coerceNumericStrings === true) {
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return numeric % 1 === 0 ? numeric.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
    }
    if (typeof value === "number") {
        return value % 1 === 0 ? value.toLocaleString(Qt.locale(), "f", 0) : String(value)
    }
    if (typeof value === "object" && options.objectMode === "json") {
        return JSON.stringify(value)
    }
    return String(value)
}

function shortMiddle(value, maxLength, headLength, tailLength) {
    const text = String(value || "")
    const max = Math.max(4, Number(maxLength || 16))
    if (text.length <= max) {
        return text.length ? text : "-"
    }
    const head = Math.max(1, Number(headLength || 8))
    const tail = Math.max(1, Number(tailLength || 6))
    return text.slice(0, head) + "..." + text.slice(-tail)
}

function shortHash(value) {
    return shortMiddle(value, 16, 8, 6)
}

function shortId(value) {
    return shortMiddle(value, 16, 8, 6)
}

function shortText(value, limitOrOptions) {
    const options = optionObject(limitOrOptions) ? limitOrOptions : {}
    const max = Math.max(Number(options.minimum || 8), Number(options.limit || limitOrOptions || 24))
    const tail = Math.max(1, Number(options.tailLength || 6))
    const head = Math.max(4, max - tail - 3)
    const empty = options.emptyText === undefined ? "-" : options.emptyText
    const text = String(value || "")
    if (text.length <= max) {
        return text.length ? text : empty
    }
    return text.slice(0, head) + "..." + text.slice(-tail)
}

function endpointLabel(value) {
    const text = String(value || "")
    if (!text.length) {
        return "-"
    }
    if (text.indexOf("127.0.0.1") >= 0 || text.indexOf("localhost") >= 0) {
        return qsTr("Local")
    }
    if (text.indexOf("testnet") >= 0) {
        return qsTr("Testnet")
    }
    return qsTr("Custom")
}

function shortEndpoint(value) {
    const text = String(value || "")
    if (!text.length) {
        return qsTr("Not configured")
    }
    return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
}

function valueSummary(value, options) {
    const config = optionObject(options) ? options : {}
    const empty = config.emptyText === undefined ? "-" : config.emptyText
    if (value === undefined || value === null || value === "") {
        return empty
    }
    if (Array.isArray(value)) {
        const shortLimit = config.shortArrayLimit === undefined ? -1 : Number(config.shortArrayLimit)
        if (value.length === 0 && config.emptyArrayText !== undefined) {
            return config.emptyArrayText
        }
        if (shortLimit >= 0 && value.length <= shortLimit) {
            return value.map(function (item) { return String(item) }).join(", ")
        }
        return qsTr("%1 item(s)").arg(value.length)
    }
    if (typeof value === "object") {
        const unwrapKeys = Array.isArray(config.unwrapKeys) ? config.unwrapKeys : []
        for (let i = 0; i < unwrapKeys.length; ++i) {
            const key = unwrapKeys[i]
            if (value[key] !== undefined) {
                return valueSummary(value[key], config)
            }
        }
        if (config.objectSummary === "json") {
            return JSON.stringify(value)
        }
        if (config.objectSummary === "count") {
            return String(Object.keys(value).length)
        }
        return qsTr("%1 field(s)").arg(Object.keys(value).length)
    }
    return String(value)
}

function countValue(value, options) {
    const config = optionObject(options) ? options : {}
    if (value === undefined || value === null) {
        return null
    }
    if (typeof config.scalarValue === "function") {
        const scalar = config.scalarValue(value)
        if (typeof scalar === "number" && Number.isFinite(scalar)) {
            return scalar
        }
    }
    if (typeof value === "number" && Number.isFinite(value)) {
        return value
    }
    if (Array.isArray(value)) {
        return value.length
    }
    if (typeof value === "object") {
        const keys = Array.isArray(config.nestedKeys) ? config.nestedKeys : []
        for (let i = 0; i < keys.length; ++i) {
            if (value[keys[i]] !== undefined) {
                const nested = countValue(value[keys[i]], config)
                if (nested !== null) {
                    return nested
                }
            }
        }
        const unwrapKeys = Array.isArray(config.unwrapKeys) ? config.unwrapKeys : []
        for (let j = 0; j < unwrapKeys.length; ++j) {
            if (value[unwrapKeys[j]] !== undefined) {
                const unwrapped = countValue(value[unwrapKeys[j]], config)
                if (unwrapped !== null) {
                    return unwrapped
                }
            }
        }
        return Object.keys(value).length
    }
    return null
}

function copyValue(value) {
    if (value === undefined || value === null) {
        return ""
    }
    if (typeof value === "object") {
        return JSON.stringify(value, null, 2)
    }
    return String(value)
}
