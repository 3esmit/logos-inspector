.import "SourcePolicyCatalog.generated.js" as Generated

function fallbackPolicy() {
    return Generated.sourcePolicy()
}

function sourceModes(family) {
    const modes = fallbackPolicy().source_modes[String(family || "")]
    return Array.isArray(modes) ? modes : []
}

function defaultValue(key, fallback) {
    const value = fallbackPolicy().defaults[String(key || "")]
    return value !== undefined ? String(value || "") : String(fallback || "")
}
