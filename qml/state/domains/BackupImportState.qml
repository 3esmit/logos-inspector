import QtQml
import "../settings/SettingsProfile.js" as SettingsProfile

QtObject {
    id: root

    required property var model
    required property var catalog
    required property var operationHistory
    readonly property string backupCatalogError: String(catalog.error || "")
    readonly property bool running: catalog.importRunning === true

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
        const catalogId = String(backupCatalogId || "").trim()
        let completed = false
        const admitted = catalog.applyImport(
            catalogId,
            model.walletProfile(),
            options && typeof options === "object" ? options : ({}),
            function (response) {
                completed = true
                completeLocalSettingsImport(response, catalogId)
            }
        )
        if (!admitted && !completed) {
            model.settingsBackupStatus = backupCatalogError.length
                ? backupCatalogError
                : qsTr("Local backup restore failed.")
        }
        if (admitted && !completed) {
            model.settingsBackupStatus = qsTr("Backup import started.")
        }
        return admitted
    }

    function completeLocalSettingsImport(response, requestedCatalogId) {
        if (!response || response.ok !== true || !response.value) {
            model.settingsBackupStatus = String(response && response.error
                || backupCatalogError || qsTr("Local backup restore failed."))
            return false
        }

        const result = response.value
        if (String(result.backupCatalogId || "") !== String(requestedCatalogId || "")) {
            model.settingsBackupStatus = qsTr("Backup import returned a different backup catalog ID.")
            return false
        }
        recordOperationEvents(result)
        const phase = String(result.phase || "")
        const outcome = String(result.outcome || "")
        if (phase !== "Applied" || outcome !== "applied") {
            const blockedLabel = String(result.blockedOperationLabel || "")
            if (outcome === "blocked") {
                model.settingsBackupStatus = blockedLabel.length
                    ? qsTr("Backup import blocked by running operation %1.").arg(blockedLabel)
                    : qsTr("Backup import is blocked by an affected running operation.")
            } else if (outcome === "rolled_back") {
                model.settingsBackupStatus = qsTr("Backup import failed; prior local state was restored.")
            } else if (outcome === "recovery_required") {
                model.settingsBackupStatus = qsTr("Backup import requires local state recovery before further changes.")
            } else {
                model.settingsBackupStatus = qsTr("Backup import returned an unsupported terminal outcome.")
            }
            return false
        }

        const appliedAreas = Array.isArray(result.appliedAreas) ? result.appliedAreas : []
        const settingsApplied = appliedAreas.indexOf("settings") >= 0
        const favoritesApplied = appliedAreas.indexOf("favorites") >= 0
        const walletApplied = appliedAreas.indexOf("wallet_profile") >= 0
        if (settingsApplied || favoritesApplied) {
            model.loadSettingsState()
        }
        if (appliedAreas.indexOf("idl_registry") >= 0) {
            model.loadIdlState()
        }
        if (walletApplied) {
            model.loadWalletState()
            model.checkLocalWalletProfile(false)
        }
        if (settingsApplied || walletApplied) {
            model.loadCapabilityRegistry()
        }

        model.settingsBackupStatus = result.encrypted === true
            ? qsTr("Imported encrypted backup: %1 IDLs and %2 favorites.")
                .arg(Number(result.idl_count || 0))
                .arg(Number(result.favorites || 0))
            : qsTr("Imported %1 IDLs and %2 favorites from local backup.")
                .arg(Number(result.idl_count || 0))
                .arg(Number(result.favorites || 0))
        return true
    }

    function recordOperationEvents(result) {
        const events = result && Array.isArray(result.operationEvents)
            ? result.operationEvents : []
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
