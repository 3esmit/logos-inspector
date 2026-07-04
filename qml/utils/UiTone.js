.pragma library

function toneColor(theme, tone) {
    if (tone === "success") {
        return theme.success
    }
    if (tone === "warning") {
        return theme.warning
    }
    if (tone === "error") {
        return theme.error
    }
    return theme.textDim
}

function toneFill(theme, tone) {
    if (tone === "success") {
        return theme.successMuted
    }
    if (tone === "warning") {
        return theme.warningMuted
    }
    if (tone === "error") {
        return theme.errorMuted
    }
    return theme.field
}

function toneBorder(theme, tone) {
    if (tone === "success" || tone === "warning" || tone === "error") {
        return toneColor(theme, tone)
    }
    return theme.outlineMuted
}

function statusTone(value) {
    const text = String(value || "")
    if (text === "ok" || text === "up" || text === "ready" || text === "finalized" || text === "confirmed" || text === "success") {
        return "success"
    }
    if (text === "error" || text === "down" || text === "failed") {
        return "error"
    }
    if (text === "warning" || text === "degraded" || text === "unknown" || text === "pending") {
        return "warning"
    }
    return "neutral"
}
