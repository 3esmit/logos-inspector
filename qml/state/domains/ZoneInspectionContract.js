function scopeKey(scope) {
    if (!scope || typeof scope !== "object") {
        return ""
    }
    const kind = String(scope.kind || "")
    if (kind === "genesis_id") {
        return kind + ":" + String(scope.genesis_id || "")
    }
    if (kind === "finalized_anchor") {
        return kind
            + ":" + String(scope.genesis_time || "")
            + ":" + String(scope.block_slot === undefined ? "" : scope.block_slot)
            + ":" + String(scope.block_id || "")
            + ":" + String(scope.parent_id || "")
    }
    return JSON.stringify(scope)
}

function numericRevision(value) {
    const revision = Number(value || 0)
    return Number.isFinite(revision) && revision >= 0 ? revision : 0
}

function copyObject(value) {
    const result = {}
    if (!value || typeof value !== "object") {
        return result
    }
    for (const key in value) {
        result[key] = value[key]
    }
    return result
}

function validReportResponse(response, reportKind) {
    return response && response.ok === true
        && response.value && typeof response.value === "object"
        && String(response.value.report_kind || "") === String(reportKind || "")
        && Number(response.value.schema_version || 0) === 1
}

function responseError(response, fallback) {
    return response && String(response.error || "").length > 0
        ? String(response.error)
        : String(fallback || "")
}

function failedResponse(error) {
    return {
        ok: false,
        value: null,
        text: "",
        error: String(error || "")
    }
}

function sameContextRoute(left, right) {
    return left && right
        && scopeKey(left.network_scope) === scopeKey(right.network_scope)
        && String(left.channel_id || "") === String(right.channel_id || "")
        && String(left.zone_kind || "") === String(right.zone_kind || "")
        && String(left.selected_sequencer_source_id || "") === String(right.selected_sequencer_source_id || "")
        && String(left.indexer_source_id || "") === String(right.indexer_source_id || "")
}

function sameContext(left, right) {
    return sameContextRoute(left, right)
        && numericRevision(left.source_config_revision) === numericRevision(right.source_config_revision)
}

function dispatch(gateway, method, request, callback) {
    if (!gateway || typeof gateway.request !== "function") {
        callback(failedResponse(qsTr("Inspector bridge is unavailable.")))
        return null
    }
    try {
        return gateway.request(String(method || ""), [request || {}], callback)
    } catch (error) {
        callback(failedResponse(String(error)))
        return null
    }
}
