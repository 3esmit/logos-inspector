.pragma library

const MAX_BYTES = 256

function optionalError(value) {
    const text = String(value || "").trim()
    if (!text.length) {
        return ""
    }
    if (utf8ByteLength(text) > MAX_BYTES) {
        return qsTr("Storage CID exceeds 256-byte limit.")
    }
    if (!hasOnlyRouteSafeAscii(text)) {
        return qsTr("Storage CID must contain only ASCII letters, digits, `-`, or `_`.")
    }
    return ""
}

function hasOnlyRouteSafeAscii(value) {
    for (let index = 0; index < value.length; ++index) {
        const code = value.charCodeAt(index)
        const upper = code >= 65 && code <= 90
        const lower = code >= 97 && code <= 122
        const digit = code >= 48 && code <= 57
        if (!upper && !lower && !digit && code !== 45 && code !== 95) {
            return false
        }
    }
    return true
}

function utf8ByteLength(value) {
    let length = 0
    for (let index = 0; index < value.length; ++index) {
        const code = value.charCodeAt(index)
        if (code <= 0x7f) {
            length += 1
        } else if (code <= 0x7ff) {
            length += 2
        } else if (code >= 0xd800 && code <= 0xdbff
                && index + 1 < value.length) {
            const next = value.charCodeAt(index + 1)
            if (next >= 0xdc00 && next <= 0xdfff) {
                length += 4
                index += 1
            } else {
                length += 3
            }
        } else {
            length += 3
        }
    }
    return length
}
