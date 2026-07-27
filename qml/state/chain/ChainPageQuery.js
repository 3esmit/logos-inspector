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

function nodeSyncState(nodeValue) {
    const info = cryptarchiaInfo(nodeValue)
    if (!info || typeof info !== "object") {
        return "unknown"
    }
    const mode = String(info.mode || info.sync_state || info.syncState || "")
        .trim().toLowerCase()
    const compactMode = mode.replace(/[^a-z0-9]/g, "")
    if (mode.indexOf("sync") >= 0 || mode.indexOf("catch") >= 0
            || mode.indexOf("start") >= 0 || compactMode.indexOf("initialblock") >= 0
            || compactMode.indexOf("bootstrap") >= 0) {
        return "syncing"
    }
    const slot = Number(info.slot)
    const libSlot = Number(info.lib_slot)
    return mode.length > 0 || (Number.isFinite(slot) && slot > 0)
        || (Number.isFinite(libSlot) && libSlot > 0) ? "ready" : "unknown"
}

function explorerWindowForSource(source, requestedWindow) {
    const requested = Math.max(0, Number(requestedWindow || 0))
    const mode = String(source || "").trim().toLowerCase()
    if (mode === "module" || mode === "logoscore_cli") {
        // Module get_blocks falls back to a live parent walk. Its fixed bound
        // is 500 blocks, so the inclusive lower endpoint must be at most 499
        // slots behind the tip.
        return Math.min(requested, 499)
    }
    return requested
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
    const windowFrom = Math.max(0, slotTo - Math.max(0, Number(windowSize || 0)))
    return {
        slotFrom: existingTo > 0
            ? Math.max(windowFrom, Math.min(existingTo, slotTo))
            : windowFrom,
        slotTo: slotTo
    }
}
