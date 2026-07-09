import QtQml
import "../backup/BackupImportPolicy.js" as BackupImportPolicy
import "../backup/BackupImportTransaction.js" as BackupImportTransaction
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
        return BackupImportTransaction.previewLocalSettingsImportPlan(root, backupCatalogId, options)
    }

    function restoreLocalSettingsBackup(backupCatalogId, options) {
        return BackupImportTransaction.restoreLocalSettingsBackup(root, backupCatalogId, options)
    }

    function backupImportPlan(options, summary, backupCatalogId) {
        return BackupImportTransaction.backupImportPlan(root, options, summary, backupCatalogId)
    }

    function backupImportId(backupCatalogId) {
        return BackupImportTransaction.backupImportId(backupCatalogId)
    }

    function backupImportPlanBase(summary) {
        return BackupImportTransaction.backupImportPlanBase(summary)
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
        return BackupImportTransaction.backupImportOperationDecision(root, operation, selectedAreas)
    }

    function selectedBackupImportAreas(options, summary) {
        return BackupImportTransaction.selectedBackupImportAreas(options, summary)
    }

    function backupImportTouchesLocalSettings(selectedAreas) {
        return BackupImportTransaction.backupImportTouchesLocalSettings(selectedAreas)
    }

    function runningBackupImportOperations() {
        return BackupImportTransaction.runningBackupImportOperations(root)
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
        return BackupImportTransaction.backupImportOperationWithRestart(decision, operation)
    }

    function backupImportMarkLetFinish(decision) {
        return BackupImportTransaction.backupImportMarkLetFinish(decision)
    }

    function backupImportStopState(decision, operation) {
        return BackupImportTransaction.backupImportStopState(root, decision, operation)
    }

    function awaitBackupImportStoppedOperation(decision, initialOperation) {
        return BackupImportTransaction.awaitBackupImportStoppedOperation(root, decision, initialOperation)
    }

    function stopBackupImportOperations(plan) {
        return BackupImportTransaction.stopBackupImportOperations(root, plan)
    }

    function restartBackupImportOperations(plan) {
        return BackupImportTransaction.restartBackupImportOperations(root, plan)
    }

    function recordBackupImportDecision(decision, detail) {
        return BackupImportTransaction.recordBackupImportDecision(root, decision, detail)
    }

    function backupImportActionStatus(action) {
        return BackupImportPolicy.actionStatus(action)
    }

    function backupImportActionReason(action) {
        return BackupImportPolicy.actionReason(action)
    }

    function backupImportAffectedInputs(selectedAreas) {
        return BackupImportTransaction.backupImportAffectedInputs(selectedAreas)
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
