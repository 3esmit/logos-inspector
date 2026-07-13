import QtQml

QtObject {
    id: root

    required property var gateway

    property var entries: []
    property bool loaded: false
    property string error: ""
    property int revision: 0

    function load() {
        const response = gateway.call("loadBackupCatalog", [], qsTr("Backup catalog"))
        if (response && response.ok === true && response.value && typeof response.value === "object") {
            entries = Array.isArray(response.value.entries) ? response.value.entries : []
            loaded = true
            error = ""
            revision += 1
            return true
        }
        loaded = true
        error = String(response && response.error ? response.error : qsTr("Backup catalog is not readable."))
        entries = []
        revision += 1
        return false
    }

    function createLocal(label, encrypted, walletProfile, contents) {
        const response = gateway.call("createLocalSettingsBackup", [String(label || ""), encrypted === true, walletProfile || {}, contents || {}], qsTr("Local backup"))
        if (response && response.ok === true && response.value) {
            upsertEntry(response.value)
            error = ""
            return response.value
        }
        error = String(response && response.error ? response.error : qsTr("Local backup failed."))
        revision += 1
        return null
    }

    function attachRemote(backupCatalogId, cid, provider) {
        const response = gateway.call("attachBackupRemote", [String(backupCatalogId || ""), String(cid || ""), String(provider || "logos_storage")], qsTr("Backup catalog"))
        if (response && response.ok === true && response.value) {
            upsertEntry(response.value)
            error = ""
            return response.value
        }
        error = String(response && response.error ? response.error : qsTr("Remote backup metadata was not saved."))
        revision += 1
        return null
    }

    function previewImport(backupCatalogId, walletProfile, options) {
        const response = gateway.call("settingsBackupImportPreview", [String(backupCatalogId || ""), walletProfile || {}, options || {}], qsTr("Backup import plan"))
        if (response && response.ok === true && response.value) {
            error = ""
            return response.value
        }
        error = String(response && response.error ? response.error : qsTr("Backup import plan failed."))
        revision += 1
        return null
    }

    function applyImport(backupCatalogId, walletProfile, options) {
        const response = gateway.call("settingsBackupImportApply", [String(backupCatalogId || ""), walletProfile || {}, options || {}], qsTr("Local backup restore"))
        if (response && response.ok === true && response.value) {
            error = ""
            revision += 1
            return response.value
        }
        error = String(response && response.error ? response.error : qsTr("Local backup restore failed."))
        revision += 1
        return null
    }

    function uploadLocal(backupCatalogId, storageRequest) {
        const request = storageRequest && typeof storageRequest === "object" ? storageRequest : ({})
        request.payload = {
            backup_catalog_id: String(backupCatalogId || ""),
            block_size: 65536
        }
        const response = gateway.call("storageUploadBackupCatalogEntry", [request], qsTr("Backup upload"))
        if (response && response.ok === true && response.value) {
            if (response.value.catalog_entry) {
                upsertEntry(response.value.catalog_entry)
            }
            error = ""
            return response.value
        }
        error = String(response && response.error ? response.error : qsTr("Backup upload failed."))
        revision += 1
        return null
    }

    function upsertEntry(entry) {
        const value = entry || {}
        const id = String(value.backup_catalog_id || "")
        if (!id.length) {
            return
        }
        const rows = Array.isArray(entries) ? entries.slice(0) : []
        let replaced = false
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i] && rows[i].backup_catalog_id ? rows[i].backup_catalog_id : "") === id) {
                rows[i] = value
                replaced = true
                break
            }
        }
        if (!replaced) {
            rows.unshift(value)
        }
        entries = rows
        loaded = true
        revision += 1
    }

    function rows() {
        const currentRevision = revision
        const rows = Array.isArray(entries) ? entries.slice(0) : []
        rows.sort(function (left, right) {
            return String(right && right.created_at ? right.created_at : "").localeCompare(String(left && left.created_at ? left.created_at : ""))
        })
        return rows
    }
}
