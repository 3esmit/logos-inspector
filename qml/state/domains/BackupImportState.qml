import QtQml
import "../settings/SettingsProfile.js" as SettingsProfile

QtObject {
    id: root

    required property var model
    required property var catalog
    required property var operationHistory
    readonly property string backupCatalogError: String(catalog.error || "")

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

    function previewLocalSettingsImportPlan(backupCatalogId, options) {
        return catalog.previewImport(
            backupCatalogId,
            model.walletProfile(),
            options && typeof options === "object" ? options : ({})
        )
    }

    function restoreLocalSettingsBackup(backupCatalogId, options) {
        const result = catalog.applyImport(
            backupCatalogId,
            model.walletProfile(),
            options && typeof options === "object" ? options : ({})
        )
        if (!result) {
            model.settingsBackupStatus = backupCatalogError.length
                ? backupCatalogError
                : qsTr("Local backup restore failed.")
            return null
        }

        recordOperationEvents(result)
        if (result.applied !== true) {
            const blockedLabel = String(result.blockedOperationLabel || "")
            model.settingsBackupStatus = blockedLabel.length
                ? qsTr("Backup import blocked by running operation %1.").arg(blockedLabel)
                : qsTr("Backup import is blocked by an affected running operation.")
            return null
        }

        const selectedAreas = Array.isArray(result.selectedAreas) ? result.selectedAreas : []
        const touchesSettings = selectedAreas.indexOf("settings") >= 0
            || selectedAreas.indexOf("favorites") >= 0
        if (touchesSettings) {
            model.loadSettingsState()
            model.settingsBackupEncrypted = result.encrypted === true
        }
        if (selectedAreas.indexOf("idl_registry") >= 0) {
            model.loadIdlState()
        }
        if (selectedAreas.indexOf("wallet_profile") >= 0) {
            model.loadWalletState()
            model.checkLocalWalletProfile(false)
        }
        if (touchesSettings || selectedAreas.indexOf("wallet_profile") >= 0) {
            model.loadCapabilityRegistry()
        }

        operationHistory.append({
            domain: "backup",
            method: "settingsBackupImportApply",
            status: "applied_for_import",
            label: qsTr("Settings backup import"),
            operationClass: "backup",
            affectedInputs: affectedInputs(selectedAreas),
            restartPolicy: "safe_read_polling",
            confirmationRequired: true,
            importId: String(result.importId || ""),
            backupCatalogId: String(result.backupCatalogId || backupCatalogId || ""),
            reason: "backup_import_applied_for_import",
            provenance: ["backup_import_coordinator", "runtime_operation_registry", "local_backup_catalog"],
            result: result
        }, qsTr("Local backup import applied."))

        model.settingsBackupStatus = result.encrypted === true
            ? qsTr("Imported encrypted backup: %1 IDLs and %2 favorites.")
                .arg(Number(result.idl_count || 0))
                .arg(Number(result.favorites || 0))
            : qsTr("Imported %1 IDLs and %2 favorites from local backup.")
                .arg(Number(result.idl_count || 0))
                .arg(Number(result.favorites || 0))
        return result
    }

    function recordOperationEvents(result) {
        const events = result && Array.isArray(result.operation_events)
            ? result.operation_events : []
        for (let i = 0; i < events.length; ++i) {
            const event = events[i] || {}
            if (event.operation && typeof event.operation === "object") {
                model.updateRuntimeOperation(event.operation)
            }
            operationHistory.append(event, String(event.detail || ""))
        }
    }

    function backupImportDecisionSummaryText(decision) {
        const value = decision || {}
        return qsTr("%1: %2")
            .arg(String(value.label || value.operationId || qsTr("operation")))
            .arg(decisionActionLabel(value.action, value.restart === true))
    }

    function decisionActionLabel(action, restart) {
        switch (String(action || "")) {
        case "stop":
            return restart ? qsTr("will stop and restart") : qsTr("will stop")
        case "block":
            return qsTr("blocks import")
        case "restart":
            return qsTr("restarted")
        case "restart_failed":
            return qsTr("restart failed")
        default:
            return qsTr("not affected")
        }
    }

    function affectedInputs(selectedAreas) {
        const areas = Array.isArray(selectedAreas) ? selectedAreas : []
        return areas.map(function (area) {
            return { key: "backup_area", value: String(area || "") }
        })
    }

    function uploadBackupCatalogEntry(backupCatalogId, onComplete) {
        if (!model.settingsBackupAvailable()) {
            model.settingsBackupStatus = qsTr("Storage upload capability is required.")
            return false
        }
        let completed = false
        const admitted = catalog.uploadLocal(backupCatalogId, function (response) {
            completed = true
            if (typeof onComplete === "function") {
                onComplete(response)
                return
            }
            applyBackupUploadResponse(response)
        })
        if (admitted && !completed && typeof onComplete !== "function") {
            model.settingsBackupStatus = qsTr("Backup upload started.")
        }
        return admitted
    }

    function applyBackupUploadResponse(response) {
        if (!response || response.ok !== true) {
            model.settingsBackupStatus = String(response && response.error
                || backupCatalogError || qsTr("Backup upload failed."))
            return false
        }
        const cid = String(response.value && response.value.cid || "").trim()
        if (!cid.length) {
            model.settingsBackupStatus = qsTr("Backup upload returned no CID.")
            return false
        }
        model.settingsBackupCid = cid
        model.settingsRestoreCid = cid
        model.settingsBackupStatus = qsTr("Backup uploaded as %1.").arg(cid)
        return true
    }

    function backupCatalogRows() {
        return catalog.rows()
    }

    function recordSettingsBackupCatalogEntry(encrypted, cid) {
        const label = model.settingsBackupEncrypted
            ? qsTr("Encrypted settings backup") : qsTr("Settings backup")
        const entry = model.createLocalSettingsBackup(
            label, encrypted === true, model.settingsBackupContents)
        if (!entry || !String(entry.backup_catalog_id || "").length || !String(cid || "").length) {
            return entry
        }
        return model.attachBackupRemote(entry.backup_catalog_id, cid, "logos_storage") || entry
    }
}
