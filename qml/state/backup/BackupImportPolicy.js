function enabledGate(provenance) {
    return {
        enabled: true,
        status: "enabled",
        missing: [],
        warnings: [],
        provenance: [String(provenance || "backup_import_policy")]
    }
}

function disabledGate(status, dependency, label, provenance) {
    return {
        enabled: false,
        status: String(status || "disabled"),
        missing: [{
            dependency: String(dependency || ""),
            label: String(label || dependency || ""),
            status: String(status || "disabled"),
            provenance: String(provenance || "backup_import_policy")
        }],
        warnings: [],
        provenance: [String(provenance || "backup_import_policy")]
    }
}

function gateSummary(gate) {
    const value = gate && typeof gate === "object" ? gate : enabledGate("backup_import_policy")
    const missing = Array.isArray(value.missing) ? value.missing : []
    return {
        enabled: value.enabled === true,
        status: String(value.status || (value.enabled === true ? "enabled" : "disabled")),
        missing: missing,
        warnings: Array.isArray(value.warnings) ? value.warnings : [],
        provenance: Array.isArray(value.provenance) ? value.provenance : []
    }
}

function safeReadOperation(metadata) {
    const operationClass = String(metadata && metadata.operationClass ? metadata.operationClass : "")
    const restartPolicy = String(metadata && metadata.restartPolicy ? metadata.restartPolicy : "")
    return operationClass === "read_poll" || restartPolicy === "safe_read_polling"
}

function restartRequest(operation) {
    const request = operation && operation.restartRequest
    return request && typeof request === "object" ? request : null
}

function operationGate(context, operation, metadata) {
    const model = context.model
    const value = operation || {}
    const domain = String(value.domain || "").toLowerCase()
    const method = String(value.method || value.label || "").toLowerCase()
    if (domain === "storage" || method.indexOf("storage") >= 0) {
        if (method.indexOf("manifest") >= 0 || method.indexOf("list") >= 0) {
            return model.storageGate("manifests")
        }
        if (method.indexOf("exists") >= 0 || method.indexOf("probe") >= 0) {
            return model.storageGate("exists")
        }
        if (method.indexOf("read") >= 0 || method.indexOf("cid") >= 0) {
            return model.storageGate("read_by_cid")
        }
        return model.storageGate("")
    }
    if (domain === "delivery" || method.indexOf("delivery") >= 0) {
        if (method.indexOf("store") >= 0 || method.indexOf("query") >= 0 || method.indexOf("read") >= 0) {
            return model.deliveryGate("store_query")
        }
        if (method.indexOf("subscribe") >= 0) {
            return model.deliveryGate("subscribe")
        }
        return model.deliveryGate("")
    }
    if (domain === "wallet" || method.indexOf("wallet") >= 0) {
        return model.walletGate("")
    }
    if (domain === "program" || method.indexOf("decode") >= 0 || method.indexOf("idl") >= 0) {
        return model.programDecodeGate()
    }
    if (domain === "backup") {
        return disabledGate("manual_required", "backup", qsTr("Backup operation"), "operation_history")
    }
    return enabledGate("operation_history")
}

function canRestartOperation(context, operation, metadata) {
    const gate = operationGate(context, operation, metadata)
    return restartRequest(operation) !== null
        && safeReadOperation(metadata)
        && gate.enabled === true
}

function decisionWithAction(decision, action, restart) {
    const source = decision || {}
    return {
        operation: source.operation || {},
        operationId: String(source.operationId || ""),
        label: String(source.label || ""),
        operationClass: String(source.operationClass || ""),
        affectedInputs: source.affectedInputs || [],
        restartPolicy: String(source.restartPolicy || ""),
        action: String(action || source.action || ""),
        affected: source.affected === true,
        restart: restart === undefined ? source.restart === true : restart === true,
        restartEligible: source.restartEligible === true,
        restartGate: source.restartGate || null,
        safeToLetFinish: source.safeToLetFinish === true,
        previousOperationId: String(source.previousOperationId || source.previous_operation_id || ""),
        restartOperationId: String(source.restartOperationId || source.restart_operation_id || ""),
        importId: String(source.importId || ""),
        backupCatalogId: String(source.backupCatalogId || "")
    }
}

function decisionActionLabel(decision) {
    const value = decision || {}
    switch (String(value.action || "")) {
    case "stop":
        return value.restart === true
            ? qsTr("will stop and restart if gates still pass")
            : qsTr("will stop; manual rerun required")
    case "let_finish":
        return qsTr("safe to let finish")
    case "restart":
        return qsTr("restarted")
    case "block":
        return qsTr("blocks import")
    case "skip_restart":
        return qsTr("manual rerun required")
    case "restart_failed":
        return qsTr("restart failed")
    default:
        return qsTr("not affected")
    }
}

function decisionGateText(decision) {
    const gate = decision && decision.restartGate ? decision.restartGate : null
    if (!gate || gate.enabled === true) {
        return ""
    }
    const missing = Array.isArray(gate.missing) ? gate.missing : []
    if (missing.length > 0) {
        return String(missing[0].label || missing[0].dependency || gate.status || "")
    }
    return String(gate.status || "")
}

function decisionSummaryText(decision) {
    const value = decision || {}
    const gateText = decisionGateText(value)
    const base = qsTr("%1: %2").arg(String(value.label || value.operationId || qsTr("operation"))).arg(decisionActionLabel(value))
    return gateText.length ? qsTr("%1 (%2)").arg(base).arg(gateText) : base
}

function operationDecision(context, operation, selectedAreas) {
    const metadata = context.operationHistory.operationMetadata(operation || {})
    const operationClass = String(metadata.operationClass || "unknown")
    const restartPolicy = String(metadata.restartPolicy || "")
    const affected = operationAffected(context, operation, selectedAreas)
    const operationId = String(operation && operation.operationId ? operation.operationId : "")
    const status = String(operation && operation.status ? operation.status : "")
    const canCancel = operation && operation.cancellable === true && status === "running"
    const safeToLetFinish = safeReadOperation(metadata)
    const restartEligible = canCancel && restartRequest(operation) !== null && safeToLetFinish
    const restartGate = restartEligible ? gateSummary(operationGate(context, operation, metadata)) : null
    let action = "ignore"
    if (affected) {
        action = operationConflictsWithImport(context, operation, metadata)
            ? "block"
            : (canCancel ? "stop" : (safeToLetFinish ? "let_finish" : "block"))
    }
    return {
        selectedAreas: selectedAreas,
        operation: operation || {},
        operationId: operationId,
        label: String(operation && (operation.label || operation.method) ? (operation.label || operation.method) : operationId),
        operationClass: operationClass,
        affectedInputs: metadata.affectedInputs || [],
        restartPolicy: restartPolicy,
        action: action,
        affected: affected,
        restart: restartEligible,
        restartEligible: restartEligible,
        restartGate: restartGate,
        safeToLetFinish: safeToLetFinish
    }
}

function selectedAreas(options, summary) {
    const selected = []
    const value = options && typeof options === "object" ? options : ({})
    const areas = ["settings", "favorites", "idl_registry", "wallet_profile"]
    for (let i = 0; i < areas.length; ++i) {
        const area = areas[i]
        const mode = String(value[area] || "").trim().toLowerCase()
        if (mode.length && mode !== "skip" && mode !== "none" && mode !== "not_import" && mode !== "not import") {
            selected.push(area)
        }
    }
    if (selected.length > 0 || !summary || typeof summary !== "object") {
        return selected
    }
    const applied = Array.isArray(summary.applied_areas) ? summary.applied_areas : []
    for (let i = 0; i < applied.length; ++i) {
        selected.push(String(applied[i] || ""))
    }
    return selected
}

function touchesLocalSettings(selectedAreas) {
    const areas = Array.isArray(selectedAreas) ? selectedAreas : []
    return areas.indexOf("settings") >= 0 || areas.indexOf("favorites") >= 0
}

function operationAffected(context, operation, selectedAreas) {
    const areas = Array.isArray(selectedAreas) ? selectedAreas : []
    const metadata = context.operationHistory.operationMetadata(operation || {})
    if (areas.length > 0 && operationConflictsWithImport(context, operation, metadata)) {
        return true
    }
    for (let i = 0; i < areas.length; ++i) {
        if (operationAffectsArea(context, operation, areas[i], metadata)) {
            return true
        }
    }
    return false
}

function operationConflictsWithImport(context, operation, metadata) {
    const value = operation || {}
    const domain = String(value.domain || "").toLowerCase()
    const method = String(value.method || value.label || "").toLowerCase()
    const info = metadata || context.operationHistory.operationMetadata(value)
    const operationClass = String(info.operationClass || "").toLowerCase()
    return domain === "backup"
        || operationClass === "backup"
        || method.indexOf("backup") >= 0
        || method.indexOf("restore") >= 0
        || method.indexOf("import") >= 0
        || method.indexOf("export") >= 0
        || method.indexOf("decrypt") >= 0
}

function operationAffectsArea(context, operation, area, metadata) {
    if (metadataAffectsArea(metadata, area)) {
        return true
    }
    const domain = String(operation && operation.domain ? operation.domain : "").toLowerCase()
    const method = String(operation && operation.method ? operation.method : "").toLowerCase()
    switch (String(area || "")) {
    case "settings":
        return domain !== "backup"
    case "favorites":
        return domain === "favorites" || method.indexOf("favorite") >= 0
    case "idl_registry":
        return method.indexOf("idl") >= 0
            || method.indexOf("decode") >= 0
            || method.indexOf("instruction") >= 0
            || method.indexOf("account") >= 0
            || domain === "program"
    case "wallet_profile":
        return domain === "wallet"
            || method.indexOf("wallet") >= 0
            || method.indexOf("sign") >= 0
            || method.indexOf("submit") >= 0
            || method.indexOf("deploy") >= 0
    default:
        return false
    }
}

function metadataAffectsArea(metadata, area) {
    const wanted = canonicalArea(area)
    if (!wanted.length) {
        return false
    }
    const inputs = metadata && Array.isArray(metadata.affectedInputs) ? metadata.affectedInputs : []
    for (let i = 0; i < inputs.length; ++i) {
        const input = inputs[i] || {}
        const key = canonicalArea(input.key)
        const value = canonicalArea(input.value)
        if (key === wanted || value === wanted) {
            return true
        }
    }
    return false
}

function canonicalArea(value) {
    const text = String(value || "").trim().toLowerCase().replace(/[- ]/g, "_")
    switch (text) {
    case "favorite":
        return "favorites"
    case "idl":
    case "idls":
        return "idl_registry"
    case "wallet":
    case "wallet_profile_state":
        return "wallet_profile"
    case "app_settings":
    case "local_settings":
    case "settings_profile":
        return "settings"
    default:
        return text
    }
}

function stoppedStatus(status) {
    const value = String(status || "").toLowerCase()
    return value === "canceled" || value === "cancelled" || value === "stopped"
}

function terminalStatus(status) {
    const value = String(status || "").toLowerCase()
    return stoppedStatus(value) || value === "completed" || value === "failed"
}

function operationWithRestart(decision, operation) {
    const value = operation || (decision ? decision.operation : null)
    const request = decision && decision.operation ? decision.operation.restartRequest : undefined
    if (!value || typeof value !== "object" || request === undefined || value.restartRequest !== undefined || value.restart_request !== undefined) {
        return value
    }
    const next = {}
    for (const key in value) {
        next[key] = value[key]
    }
    next.restartRequest = request
    return next
}

function markLetFinish(decision) {
    if (decision && typeof decision === "object") {
        decision.action = "let_finish"
        decision.restart = false
        decision.restartEligible = false
        decision.restartGate = null
    }
    return decisionWithAction(decision, "let_finish", false)
}

function actionStatus(action) {
    switch (String(action || "")) {
    case "stop":
        return "stopped_for_import"
    case "let_finish":
        return "let_finish_for_import"
    case "block":
        return "blocked_for_import"
    case "skip_restart":
        return "restart_skipped_for_import"
    case "restart":
        return "restarted_after_import"
    case "restart_failed":
        return "restart_failed_after_import"
    default:
        return "ignored"
    }
}

function actionReason(action) {
    switch (String(action || "")) {
    case "stop":
        return "affected_operation_stopped_for_import"
    case "let_finish":
        return "safe_operation_left_running_for_import"
    case "block":
        return "affected_operation_blocked_for_import"
    case "skip_restart":
        return "restart_not_safe_for_import"
    case "restart":
        return "safe_operation_restarted_after_import"
    case "restart_failed":
        return "safe_operation_restart_failed_after_import"
    default:
        return "not_applicable"
    }
}

function affectedInputs(selectedAreas) {
    const rows = []
    const areas = Array.isArray(selectedAreas) ? selectedAreas : []
    for (let i = 0; i < areas.length; ++i) {
        rows.push({ key: "backup_area", value: String(areas[i] || "") })
    }
    return rows
}
