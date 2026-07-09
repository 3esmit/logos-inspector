.import "BackupImportPolicy.js" as BackupImportPolicy

function previewLocalSettingsImportPlan(state, backupCatalogId, options) {
    const importOptions = options && typeof options === "object" ? options : ({})
    const preview = state.model.previewLocalSettingsRestore(backupCatalogId, importOptions)
    if (!preview) {
        return null
    }
    return backupImportPlan(state, importOptions, preview, backupCatalogId)
}

function restoreLocalSettingsBackup(state, backupCatalogId, options) {
    const model = state.model
    const catalog = state.catalog
    const importOptions = options && typeof options === "object" ? options : ({})
    const preview = catalog.previewLocalRestore(backupCatalogId, model.walletProfile(), importOptions)
    if (!preview) {
        model.settingsBackupStatus = model.backupCatalogError
        return null
    }
    const plan = backupImportPlan(state, importOptions, preview, backupCatalogId)
    if (plan.selectedAreas.length === 0) {
        model.settingsBackupStatus = qsTr("Select at least one backup section to import.")
        return null
    }
    if (plan.blocked) {
        for (let i = 0; i < plan.decisions.length; ++i) {
            if (plan.decisions[i].action === "block") {
                recordBackupImportDecision(state, plan.decisions[i], qsTr("Blocked backup import while affected operation is running."))
            }
        }
        model.settingsBackupStatus = qsTr("Backup import blocked by running operation %1.")
            .arg(plan.blockedOperationLabel)
        return null
    }
    if (!stopBackupImportOperations(state, plan)) {
        return null
    }
    const summary = catalog.restoreLocal(backupCatalogId, model.walletProfile(), importOptions)
    if (!summary) {
        model.settingsBackupStatus = model.backupCatalogError.length ? model.backupCatalogError : qsTr("Local backup restore failed.")
        return null
    }
    const touchesLocalSettings = backupImportTouchesLocalSettings(plan.selectedAreas)
    if (touchesLocalSettings) {
        model.loadSettingsState()
        model.settingsBackupEncrypted = summary.encrypted === true
    }
    if (plan.selectedAreas.indexOf("idl_registry") >= 0) {
        model.loadIdlState()
    }
    if (plan.selectedAreas.indexOf("wallet_profile") >= 0) {
        model.loadWalletState()
    }
    if (touchesLocalSettings || plan.selectedAreas.indexOf("wallet_profile") >= 0) {
        model.loadCapabilityRegistry()
    }
    model.settingsBackupStatus = summary.encrypted === true
        ? qsTr("Imported encrypted backup: %1 IDLs and %2 favorites.")
            .arg(Number(summary.idl_count || 0))
            .arg(Number(summary.favorites || 0))
        : qsTr("Imported %1 IDLs and %2 favorites from local backup.")
            .arg(Number(summary.idl_count || 0))
            .arg(Number(summary.favorites || 0))
    model.appendOperationHistory({
        domain: "backup",
        method: "restoreLocalSettingsBackup",
        status: "applied_for_import",
        label: qsTr("Settings backup import"),
        operationClass: "backup",
        affectedInputs: backupImportAffectedInputs(plan.selectedAreas),
        restartPolicy: "safe_read_poll_only",
        confirmationRequired: true,
        importId: plan.importId,
        backupCatalogId: plan.backupCatalogId,
        reason: "backup_import_applied_for_import",
        provenance: ["backup_import_transaction", "backup_import_policy", "local_backup_catalog"],
        result: summary
    }, qsTr("Local backup import applied."))
    restartBackupImportOperations(state, plan)
    if (touchesLocalSettings) {
        model.saveSettingsState()
    }
    return summary
}

function backupImportPlan(state, options, summary, backupCatalogId) {
    const selectedAreas = selectedBackupImportAreas(options, summary)
    const decisions = []
    const operations = runningBackupImportOperations(state)
    let blocked = false
    let blockedLabel = ""
    const catalogId = String((summary && summary.backup_catalog_id) || backupCatalogId || "")
    const importId = backupImportId(catalogId)
    for (let i = 0; i < operations.length; ++i) {
        const decision = backupImportOperationDecision(state, operations[i], selectedAreas)
        if (!decision.affected) {
            continue
        }
        decision.importId = importId
        decision.backupCatalogId = catalogId
        decisions.push(decision)
        if (decision.action === "block") {
            blocked = true
            if (!blockedLabel.length) {
                blockedLabel = decision.label
            }
        }
    }
    const result = backupImportPlanBase(summary)
    result.selectedAreas = selectedAreas
    result.decisions = decisions
    result.operation_decisions = decisions
    result.blocked = blocked
    result.blockedOperationLabel = blockedLabel
    result.importId = importId
    result.backupCatalogId = catalogId
    result.summary = summary || {}
    result.import_plan = true
    return result
}

function backupImportId(backupCatalogId) {
    const catalogId = String(backupCatalogId || "unknown")
    return "backup_import:" + catalogId
}

function backupImportPlanBase(summary) {
    const result = ({})
    const source = summary && typeof summary === "object" ? summary : ({})
    const keys = Object.keys(source)
    for (let i = 0; i < keys.length; ++i) {
        result[keys[i]] = source[keys[i]]
    }
    return result
}

function runningBackupImportOperations(state) {
    const revision = state.model.runtimeOperationsRevision
    const values = state.model.runtimeOperations && typeof state.model.runtimeOperations === "object" ? state.model.runtimeOperations : ({})
    const keys = Object.keys(values)
    const rows = []
    for (let i = 0; i < keys.length; ++i) {
        const operation = values[keys[i]] || {}
        const status = String(operation.status || "")
        if (status === "running" || status === "canceling") {
            rows.push(operation)
        }
    }
    return rows
}

function backupImportOperationDecision(state, operation, selectedAreas) {
    return BackupImportPolicy.operationDecision(policyContext(state), operation, selectedAreas)
}

function backupImportOperationGate(state, operation, metadata) {
    return BackupImportPolicy.operationGate(policyContext(state), operation, metadata)
}

function backupImportCanRestartOperation(state, operation, metadata) {
    return BackupImportPolicy.canRestartOperation(policyContext(state), operation, metadata)
}

function backupImportTouchesLocalSettings(selectedAreas) {
    return BackupImportPolicy.touchesLocalSettings(selectedAreas)
}

function backupImportOperationWithRestart(decision, operation) {
    return BackupImportPolicy.operationWithRestart(decision, operation)
}

function backupImportMarkLetFinish(decision) {
    return BackupImportPolicy.markLetFinish(decision)
}

function backupImportStopState(state, decision, operation) {
    const value = backupImportOperationWithRestart(decision, operation)
    if (value && typeof value === "object") {
        state.model.updateRuntimeOperation(value)
    }
    const status = String(value && value.status ? value.status : "").toLowerCase()
    if (BackupImportPolicy.stoppedStatus(status)) {
        return { ok: true, operation: value }
    }
    if (BackupImportPolicy.terminalStatus(status)) {
        return {
            ok: false,
            operation: value,
            terminal: true,
            error: qsTr("Affected operation finished instead of stopping before backup import.")
        }
    }
    return null
}

function awaitBackupImportStoppedOperation(state, decision, initialOperation) {
    const operationId = String(decision && decision.operationId ? decision.operationId : "")
    let latest = backupImportOperationWithRestart(decision, initialOperation)
    let stopState = backupImportStopState(state, decision, latest)
    if (stopState !== null) {
        return stopState
    }
    if (!operationId.length) {
        return {
            ok: false,
            operation: latest,
            error: qsTr("Backup import could not identify an affected operation to stop.")
        }
    }
    for (let attempt = 0; attempt < 6; ++attempt) {
        const response = state.model.requestModule(state.model.inspectorModule, "runtimeOperationStatus", [operationId], qsTr("Runtime operation"), false, false)
        if (!response || !response.ok) {
            return {
                ok: false,
                operation: latest,
                error: response && response.error ? response.error : qsTr("Backup import could not check whether an affected operation stopped.")
            }
        }
        latest = backupImportOperationWithRestart(decision, response.value)
        stopState = backupImportStopState(state, decision, latest)
        if (stopState !== null) {
            return stopState
        }
    }
    return {
        ok: false,
        operation: latest,
        timeout: true,
        error: qsTr("Backup import timed out waiting for an affected operation to stop.")
    }
}

function stopBackupImportOperations(state, plan) {
    const decisions = plan && Array.isArray(plan.decisions) ? plan.decisions : []
    for (let i = 0; i < decisions.length; ++i) {
        const decision = decisions[i]
        if (decision.action === "let_finish") {
            recordBackupImportDecision(state, decision, qsTr("Left safe affected operation running during backup import."))
            continue
        }
        if (decision.action !== "stop") {
            continue
        }
        const response = state.model.callInspector("runtimeOperationCancel", [decision.operationId], qsTr("Cancel operation"))
        if (!response.ok) {
            if (decision.safeToLetFinish === true) {
                recordBackupImportDecision(state, backupImportMarkLetFinish(decision), qsTr("Stop failed; safe operation was left to finish."))
                continue
            }
            state.model.settingsBackupStatus = response.error || qsTr("Backup import could not stop a running operation.")
            recordBackupImportDecision(state, BackupImportPolicy.decisionWithAction(decision, "block", false), qsTr("Failed to stop affected operation before backup import."))
            return false
        }
        const stopped = awaitBackupImportStoppedOperation(state, decision, response.value || decision.operation)
        if (!stopped.ok) {
            if (stopped.operation && typeof stopped.operation === "object") {
                state.model.updateRuntimeOperation(stopped.operation)
            }
            if (decision.safeToLetFinish === true) {
                recordBackupImportDecision(state, backupImportMarkLetFinish(decision), stopped.error || qsTr("Safe operation was left to finish before backup import."))
                continue
            }
            state.model.settingsBackupStatus = stopped.error || qsTr("Backup import could not stop a running operation.")
            recordBackupImportDecision(state, BackupImportPolicy.decisionWithAction(decision, "block", false), stopped.error || qsTr("Affected operation did not stop before backup import."))
            return false
        }
        recordBackupImportDecision(state, decision, qsTr("Stopped affected operation before backup import."))
    }
    return true
}

function restartBackupImportOperations(state, plan) {
    const decisions = plan && Array.isArray(plan.decisions) ? plan.decisions : []
    for (let i = 0; i < decisions.length; ++i) {
        const decision = decisions[i]
        if (decision.action !== "stop") {
            continue
        }
        const request = decision.operation && decision.operation.restartRequest
        if (!request || typeof request !== "object" || decision.restartEligible !== true) {
            recordBackupImportDecision(state, BackupImportPolicy.decisionWithAction(decision, "skip_restart", false), qsTr("Manual rerun required after backup import."))
            continue
        }
        const metadata = state.operationHistory.operationMetadata(decision.operation || {})
        if (!backupImportCanRestartOperation(state, decision.operation, metadata)) {
            const skipped = BackupImportPolicy.decisionWithAction(decision, "skip_restart", false)
            skipped.restartGate = BackupImportPolicy.gateSummary(backupImportOperationGate(state, decision.operation, metadata))
            recordBackupImportDecision(state, skipped, qsTr("Skipped automatic restart because gates do not pass after import."))
            continue
        }
        state.model.runtimeOperationStart(request, false, function (response) {
            if (!response || !response.ok) {
                const failed = BackupImportPolicy.decisionWithAction(decision, "restart_failed", false)
                failed.previousOperationId = decision.operationId
                recordBackupImportDecision(state, failed, response && response.error ? response.error : qsTr("Safe read operation restart failed."))
                return
            }
            const restarted = BackupImportPolicy.decisionWithAction(decision, "restart", true)
            restarted.previousOperationId = decision.operationId
            restarted.restartOperationId = String(response.value && response.value.operationId ? response.value.operationId : "")
            if (restarted.restartOperationId.length) {
                restarted.operationId = restarted.restartOperationId
            }
            recordBackupImportDecision(state, restarted, qsTr("Restarted safe read operation after backup import."))
        })
    }
}

function recordBackupImportDecision(state, decision, detail) {
    const value = decision || {}
    const action = String(value.action || "")
    const status = BackupImportPolicy.actionStatus(action)
    const reason = BackupImportPolicy.actionReason(action)
    state.model.appendOperationHistory({
        domain: "backup",
        method: "settingsBackupImportPolicy",
        status: status,
        label: qsTr("Backup import policy"),
        operationId: value.operationId,
        previousOperationId: value.previousOperationId,
        restartOperationId: value.restartOperationId,
        operationClass: value.operationClass,
        affectedInputs: value.affectedInputs || [],
        restartPolicy: value.restartPolicy,
        confirmationRequired: false,
        importId: value.importId,
        backupCatalogId: value.backupCatalogId,
        reason: reason,
        provenance: ["backup_import_transaction", "backup_import_policy", "operation_history"],
        result: {
            action: action,
            status: status,
            reason: reason,
            import_id: value.importId,
            backup_catalog_id: value.backupCatalogId,
            operation_id: value.operationId,
            previous_operation_id: value.previousOperationId || null,
            restart_operation_id: value.restartOperationId || null,
            operation_class: value.operationClass,
            restart: value.restart === true,
            restart_eligible: value.restartEligible === true,
            restart_gate: value.restartGate || null,
            safe_to_let_finish: value.safeToLetFinish === true,
            provenance: ["backup_import_transaction", "backup_import_policy", "operation_history"]
        }
    }, detail)
}

function selectedBackupImportAreas(options, summary) {
    return BackupImportPolicy.selectedAreas(options, summary)
}

function backupImportAffectedInputs(selectedAreas) {
    return BackupImportPolicy.affectedInputs(selectedAreas)
}

function policyContext(state) {
    return {
        model: state.model,
        operationHistory: state.operationHistory
    }
}
