.pragma library

function numberText(value, decimals) {
    if (value === undefined || value === null || value === "") {
        return "-"
    }
    if (typeof value === "number") {
        return value.toLocaleString(Qt.locale(), "f", decimals === undefined ? 0 : decimals)
    }
    return String(value)
}

function valueText(value, emptyText) {
    const empty = emptyText === undefined ? "-" : emptyText
    if (value === undefined || value === null || value === "") {
        return empty
    }
    if (typeof value === "number") {
        return value % 1 === 0 ? value.toLocaleString(Qt.locale(), "f", 0) : String(value)
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

function shortText(value, limit) {
    const max = Math.max(8, Number(limit || 24))
    const head = Math.max(4, max - 9)
    return shortMiddle(value, max, head, 6)
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
