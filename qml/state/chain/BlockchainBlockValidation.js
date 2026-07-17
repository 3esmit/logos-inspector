.pragma library

function validate(value) {
    const blockId = textValue(value).trim()
    if (!blockId.length) {
        return invalid("")
    }
    const match = /^(0[xX])?([0-9a-fA-F]{64})$/.exec(blockId)
    if (!match) {
        return invalid(qsTr(
            "Block ID must be 64 hexadecimal characters (optional 0x prefix)."))
    }
    return {
        valid: true,
        message: "",
        blockId: match[2].toLowerCase()
    }
}

function textValue(value) {
    return value === null || value === undefined ? "" : String(value)
}

function invalid(message) {
    return {
        valid: false,
        message: String(message || ""),
        blockId: ""
    }
}
