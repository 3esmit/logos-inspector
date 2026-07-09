import QtQml
import "../backup/BackupImportPolicy.js" as BackupImportPolicy
import "../settings/SettingsProfile.js" as SettingsProfile

QtObject {
    id: root

    required property var model
    required property var catalog
    required property var operationHistory

    function defaultSettingsBackupContents() {
        return SettingsProfile.defaultBackupContents()
    }

    function normalizedBackupContents(contents) {
        return SettingsProfile.normalizedBackupContents(contents)
    }

    function backupContentsSelected(contents) {
        return SettingsProfile.backupContentsSelected(contents || model.settingsBackupContents)
    }

    function setSettingsBackupContent(area, enabled) {
        model.settingsBackupContents = SettingsProfile.updatedBackupContents(model.settingsBackupContents, area, enabled)
    }

    function policyContext() {
        return {
            model: model,
            operationHistory: operationHistory
        }
    }

    function previewLocalSettingsImportPlan(backupCatalogId, options) {
        const importOptions = options && typeof options === "object" ? options : ({})
        const preview = model.previewLocalSettingsRestore(backupCatalogId, importOptions)
        if (!preview) {
            return null
        }
        return backupImportPlan(importOptions, preview, backupCatalogId)
    }

    function restoreLocalSettingsBackup(backupCatalogId, options) {
        const importOptions = options && typeof options === "object" ? options : ({})
        const preview = catalog.previewLocalRestore(backupCatalogId, model.walletProfile(), importOptions)
        if (!preview) {
            model.settingsBackupStatus = model.backupCatalogError
            return null
        }
        const plan = backupImportPlan(importOptions, preview, backupCatalogId)
        if (plan.selectedAreas.length === 0) {
            model.settingsBackupStatus = qsTr("Select at least one backup section to import.")
            return null
        }
        if (plan.blocked) {
            for (let i = 0; i < plan.decisions.length; ++i) {
                if (plan.decisions[i].action === "block") {
                    recordBackupImportDecision(plan.decisions[i], qsTr("Blocked backup import while affected operation is running."))
                }
            }
            model.settingsBackupStatus = qsTr("Backup import blocked by running operation %1.")
                .arg(plan.blockedOperationLabel)
            return null
        }
        if (!stopBackupImportOperations(plan)) {
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
            provenance: ["backup_import_policy", "local_backup_catalog"],
            result: summary
        }, qsTr("Local backup import applied."))
        restartBackupImportOperations(plan)
        if (touchesLocalSettings) {
            model.saveSettingsState()
        }
        return summary
    }

    function backupImportPlan(options, summary, backupCatalogId) {
        const selectedAreas = selectedBackupImportAreas(options, summary)
        const decisions = []
        const operations = runningBackupImportOperations()
        let blocked = false
        let blockedLabel = ""
        const catalogId = String((summary && summary.backup_catalog_id) || backupCatalogId || "")
        const importId = backupImportId(catalogId)
        for (let i = 0; i < operations.length; ++i) {
            const decision = backupImportOperationDecision(operations[i], selectedAreas)
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

    function backupImportEnabledGate(provenance) {
        return BackupImportPolicy.enabledGate(provenance)
    }

    function backupImportDisabledGate(status, dependency, label, provenance) {
        return BackupImportPolicy.disabledGate(status, dependency, label, provenance)
    }

    function backupImportGateSummary(gate) {
        return BackupImportPolicy.gateSummary(gate)
    }

    function backupImportSafeReadOperation(metadata) {
        return BackupImportPolicy.safeReadOperation(metadata)
    }

    function backupImportRestartRequest(operation) {
        return BackupImportPolicy.restartRequest(operation)
    }

    function backupImportOperationGate(operation, metadata) {
        return BackupImportPolicy.operationGate(policyContext(), operation, metadata)
    }

    function backupImportCanRestartOperation(operation, metadata) {
        return BackupImportPolicy.canRestartOperation(policyContext(), operation, metadata)
    }

    function backupImportDecisionWithAction(decision, action, restart) {
        return BackupImportPolicy.decisionWithAction(decision, action, restart)
    }

    function backupImportDecisionActionLabel(decision) {
        return BackupImportPolicy.decisionActionLabel(decision)
    }

    function backupImportDecisionGateText(decision) {
        return BackupImportPolicy.decisionGateText(decision)
    }

    function backupImportDecisionSummaryText(decision) {
        return BackupImportPolicy.decisionSummaryText(decision)
    }

    function backupImportOperationDecision(operation, selectedAreas) {
        return BackupImportPolicy.operationDecision(policyContext(), operation, selectedAreas)
    }

    function selectedBackupImportAreas(options, summary) {
        return BackupImportPolicy.selectedAreas(options, summary)
    }

    function backupImportTouchesLocalSettings(selectedAreas) {
        return BackupImportPolicy.touchesLocalSettings(selectedAreas)
    }

    function runningBackupImportOperations() {
        const revision = model.runtimeOperationsRevision
        const values = model.runtimeOperations && typeof model.runtimeOperations === "object" ? model.runtimeOperations : ({})
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

    function backupImportOperationAffected(operation, selectedAreas) {
        return BackupImportPolicy.operationAffected(policyContext(), operation, selectedAreas)
    }

    function backupImportOperationConflictsWithImport(operation, metadata) {
        return BackupImportPolicy.operationConflictsWithImport(policyContext(), operation, metadata)
    }

    function backupImportOperationAffectsArea(operation, area, metadata) {
        return BackupImportPolicy.operationAffectsArea(policyContext(), operation, area, metadata)
    }

    function backupImportMetadataAffectsArea(metadata, area) {
        return BackupImportPolicy.metadataAffectsArea(metadata, area)
    }

    function backupImportCanonicalArea(value) {
        return BackupImportPolicy.canonicalArea(value)
    }

    function backupImportStoppedStatus(status) {
        return BackupImportPolicy.stoppedStatus(status)
    }

    function backupImportTerminalStatus(status) {
        return BackupImportPolicy.terminalStatus(status)
    }

    function backupImportOperationWithRestart(decision, operation) {
        return BackupImportPolicy.operationWithRestart(decision, operation)
    }

    function backupImportMarkLetFinish(decision) {
        return BackupImportPolicy.markLetFinish(decision)
    }

    function backupImportStopState(decision, operation) {
        const value = backupImportOperationWithRestart(decision, operation)
        if (value && typeof value === "object") {
            model.updateRuntimeOperation(value)
        }
        const status = String(value && value.status ? value.status : "").toLowerCase()
        if (backupImportStoppedStatus(status)) {
            return { ok: true, operation: value }
        }
        if (backupImportTerminalStatus(status)) {
            return {
                ok: false,
                operation: value,
                terminal: true,
                error: qsTr("Affected operation finished instead of stopping before backup import.")
            }
        }
        return null
    }

    function awaitBackupImportStoppedOperation(decision, initialOperation) {
        const operationId = String(decision && decision.operationId ? decision.operationId : "")
        let latest = backupImportOperationWithRestart(decision, initialOperation)
        let state = backupImportStopState(decision, latest)
        if (state !== null) {
            return state
        }
        if (!operationId.length) {
            return {
                ok: false,
                operation: latest,
                error: qsTr("Backup import could not identify an affected operation to stop.")
            }
        }
        for (let attempt = 0; attempt < 6; ++attempt) {
            const response = model.requestModule(model.inspectorModule, "runtimeOperationStatus", [operationId], qsTr("Runtime operation"), false, false)
            if (!response || !response.ok) {
                return {
                    ok: false,
                    operation: latest,
                    error: response && response.error ? response.error : qsTr("Backup import could not check whether an affected operation stopped.")
                }
            }
            latest = backupImportOperationWithRestart(decision, response.value)
            state = backupImportStopState(decision, latest)
            if (state !== null) {
                return state
            }
        }
        return {
            ok: false,
            operation: latest,
            timeout: true,
            error: qsTr("Backup import timed out waiting for an affected operation to stop.")
        }
    }

    function stopBackupImportOperations(plan) {
        const decisions = plan && Array.isArray(plan.decisions) ? plan.decisions : []
        for (let i = 0; i < decisions.length; ++i) {
            const decision = decisions[i]
            if (decision.action === "let_finish") {
                recordBackupImportDecision(decision, qsTr("Left safe affected operation running during backup import."))
                continue
            }
            if (decision.action !== "stop") {
                continue
            }
            const response = model.callInspector("runtimeOperationCancel", [decision.operationId], qsTr("Cancel operation"))
            if (!response.ok) {
                if (decision.safeToLetFinish === true) {
                    recordBackupImportDecision(backupImportMarkLetFinish(decision), qsTr("Stop failed; safe operation was left to finish."))
                    continue
                }
                model.settingsBackupStatus = response.error || qsTr("Backup import could not stop a running operation.")
                recordBackupImportDecision(backupImportDecisionWithAction(decision, "block", false), qsTr("Failed to stop affected operation before backup import."))
                return false
            }
            const stopped = awaitBackupImportStoppedOperation(decision, response.value || decision.operation)
            if (!stopped.ok) {
                if (stopped.operation && typeof stopped.operation === "object") {
                    model.updateRuntimeOperation(stopped.operation)
                }
                if (decision.safeToLetFinish === true) {
                    recordBackupImportDecision(backupImportMarkLetFinish(decision), stopped.error || qsTr("Safe operation was left to finish before backup import."))
                    continue
                }
                model.settingsBackupStatus = stopped.error || qsTr("Backup import could not stop a running operation.")
                recordBackupImportDecision(backupImportDecisionWithAction(decision, "block", false), stopped.error || qsTr("Affected operation did not stop before backup import."))
                return false
            }
            recordBackupImportDecision(decision, qsTr("Stopped affected operation before backup import."))
        }
        return true
    }

    function restartBackupImportOperations(plan) {
        const decisions = plan && Array.isArray(plan.decisions) ? plan.decisions : []
        for (let i = 0; i < decisions.length; ++i) {
            const decision = decisions[i]
            if (decision.action !== "stop") {
                continue
            }
            const request = decision.operation && decision.operation.restartRequest
            if (!request || typeof request !== "object" || decision.restartEligible !== true) {
                recordBackupImportDecision(backupImportDecisionWithAction(decision, "skip_restart", false), qsTr("Manual rerun required after backup import."))
                continue
            }
            const metadata = operationHistory.operationMetadata(decision.operation || {})
            if (!backupImportCanRestartOperation(decision.operation, metadata)) {
                const skipped = backupImportDecisionWithAction(decision, "skip_restart", false)
                skipped.restartGate = backupImportGateSummary(backupImportOperationGate(decision.operation, metadata))
                recordBackupImportDecision(skipped, qsTr("Skipped automatic restart because gates do not pass after import."))
                continue
            }
            model.runtimeOperationStart(request, false, function (response) {
                if (!response || !response.ok) {
                    const failed = backupImportDecisionWithAction(decision, "restart_failed", false)
                    failed.previousOperationId = decision.operationId
                    recordBackupImportDecision(failed, response && response.error ? response.error : qsTr("Safe read operation restart failed."))
                    return
                }
                const restarted = backupImportDecisionWithAction(decision, "restart", true)
                restarted.previousOperationId = decision.operationId
                restarted.restartOperationId = String(response.value && response.value.operationId ? response.value.operationId : "")
                if (restarted.restartOperationId.length) {
                    restarted.operationId = restarted.restartOperationId
                }
                recordBackupImportDecision(restarted, qsTr("Restarted safe read operation after backup import."))
            })
        }
    }

    function recordBackupImportDecision(decision, detail) {
        const value = decision || {}
        const action = String(value.action || "")
        const status = backupImportActionStatus(action)
        const reason = backupImportActionReason(action)
        model.appendOperationHistory({
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
            provenance: ["backup_import_policy", "operation_history"],
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
                provenance: ["backup_import_policy", "operation_history"]
            }
        }, detail)
    }

    function backupImportActionStatus(action) {
        return BackupImportPolicy.actionStatus(action)
    }

    function backupImportActionReason(action) {
        return BackupImportPolicy.actionReason(action)
    }

    function backupImportAffectedInputs(selectedAreas) {
        return BackupImportPolicy.affectedInputs(selectedAreas)
    }

    function uploadBackupCatalogEntry(backupCatalogId) {
        if (!model.settingsBackupAvailable()) {
            model.settingsBackupStatus = qsTr("Storage upload capability is required.")
            return null
        }
        return catalog.uploadLocal(backupCatalogId, [
            model.effectiveStorageSourceMode(model.storageSourceMode),
            model.configuredStorageRestUrl(),
            model.storageMutatingDiagnosticsEnabled === true
        ])
    }

    function backupCatalogRows() {
        return catalog.rows()
    }

    function recordSettingsBackupCatalogEntry(encrypted, cid) {
        const entry = model.createLocalSettingsBackup(model.settingsBackupEncrypted ? qsTr("Encrypted settings backup") : qsTr("Settings backup"), encrypted === true, model.settingsBackupContents)
        if (!entry || !String(entry.backup_catalog_id || "").length || !String(cid || "").length) {
            return entry
        }
        return model.attachBackupRemote(entry.backup_catalog_id, cid, "logos_storage") || entry
    }
}
