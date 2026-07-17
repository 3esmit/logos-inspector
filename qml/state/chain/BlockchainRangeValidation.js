.pragma library

const MAX_BLOCK_RANGE_SLOTS = 2001

function validate(slotFromValue, slotToValue) {
    const fromText = textValue(slotFromValue)
    const toText = textValue(slotToValue)
    if (!fromText.length && !toText.length) {
        return invalid("", "")
    }
    if (!fromText.length || !toText.length) {
        return invalid(qsTr("Enter both Slot from and Slot to."),
            !fromText.length ? "from" : "to")
    }

    const fromResult = parseSlot(fromText)
    const toResult = parseSlot(toText)
    if (fromResult.error === "format" || toResult.error === "format") {
        return invalid(
            qsTr("Slots must use unsigned decimal integers without signs, spaces, or leading zeros."),
            invalidField(fromResult, toResult, "format"))
    }
    if (fromResult.error.length || toResult.error.length) {
        return invalid(qsTr("Slots exceed the supported numeric range."),
            invalidField(fromResult, toResult, "range"))
    }
    const slotFrom = fromResult.value
    const slotTo = toResult.value
    if (slotFrom > slotTo) {
        return invalid(qsTr("Slot from must be less than or equal to Slot to."), "to")
    }
    if (slotTo - slotFrom >= MAX_BLOCK_RANGE_SLOTS) {
        return invalid(qsTr("Slot range cannot contain more than 2,001 slots."), "to")
    }
    return {
        valid: true,
        message: "",
        invalidField: "",
        slotFrom: slotFrom,
        slotTo: slotTo
    }
}

function parseSlot(value) {
    const text = textValue(value)
    if (!/^(0|[1-9][0-9]*)$/.test(text)) {
        return { value: null, error: "format" }
    }
    const number = Number(text)
    if (!Number.isSafeInteger(number) || number < 0) {
        return { value: null, error: "range" }
    }
    return { value: number, error: "" }
}

function textValue(value) {
    return value === null || value === undefined ? "" : String(value)
}

function invalidField(fromResult, toResult, errorKind) {
    const fromInvalid = fromResult.error === errorKind
    const toInvalid = toResult.error === errorKind
    if (fromInvalid && toInvalid) {
        return "both"
    }
    return fromInvalid ? "from" : "to"
}

function invalid(message, field) {
    return {
        valid: false,
        message: String(message || ""),
        invalidField: String(field || ""),
        slotFrom: null,
        slotTo: null
    }
}

function maximumSlotCount() {
    return MAX_BLOCK_RANGE_SLOTS
}
