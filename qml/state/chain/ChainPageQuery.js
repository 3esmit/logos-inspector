function cryptarchiaInfo(nodeValue) {
    const infoProbe = nodeValue ? nodeValue.cryptarchia_info : null
    return infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
}

function slotTip(nodeValue, preferLibSlot) {
    const info = cryptarchiaInfo(nodeValue)
    if (!info) {
        return 0
    }
    return preferLibSlot === true
        ? Number(info.lib_slot || info.slot || 0)
        : Number(info.slot || info.lib_slot || 0)
}

function slotWindow(anchorSlot, fallbackSlot, windowSize) {
    const fallback = Math.max(0, Number(fallbackSlot || 0))
    const anchor = Number(anchorSlot)
    const requested = Math.max(0, Number(anchorSlot === undefined || anchorSlot === null
        || !Number.isFinite(anchor) ? fallback : anchor))
    const slotTo = fallback > 0 ? Math.min(requested, fallback) : requested
    return {
        slotFrom: Math.max(0, slotTo - Math.max(0, Number(windowSize || 0))),
        slotTo: slotTo
    }
}

function liveSlotWindow(tipSlot, existingSlotTo, windowSize) {
    const slotTo = Number(tipSlot || 0) > 0
        ? Number(tipSlot || 0)
        : Math.max(0, Number(existingSlotTo || 0))
    const existingTo = Math.max(0, Number(existingSlotTo || 0))
    return {
        slotFrom: existingTo > 0 ? Math.min(existingTo, slotTo) : Math.max(0, slotTo - Math.max(0, Number(windowSize || 0))),
        slotTo: slotTo
    }
}
