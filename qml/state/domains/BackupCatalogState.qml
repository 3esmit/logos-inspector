import QtQml
import ".." as State

QtObject {
    id: root

    required property var gateway
    property var storageAdapterInitialization: ({ source_mode: "", inputs: ({}) })
    property bool storageMutatingDiagnosticsEnabled: false

    property var entries: []
    property bool loaded: false
    property string error: ""
    property int revision: 0
    property string pendingUploadCatalogId: ""
    property var uploadCompletion: null
    property int uploadGeneration: 0
    readonly property bool uploadRunning: uploadOperations.view.busy

    property State.SourceOperationSession uploadSession: State.SourceOperationSession {
        id: uploadOperations

        gateway: root.gateway
        domain: "storage"
        adapterInitialization: root.storageAdapterInitialization
        mutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        defaultLabel: qsTr("Backup upload")
        busyError: qsTr("A backup upload is already running.")
        operationValidator: function (operation) {
            return root.uploadOperationProblem(operation).length === 0
        }

        onTerminalOperation: function (operation) {
            root.completeUpload(operation)
        }
    }

    onStorageAdapterInitializationChanged: invalidateUpload(qsTr("Storage source changed during backup upload."))
    onStorageMutatingDiagnosticsEnabledChanged: invalidateUpload(qsTr("Storage upload policy changed during backup upload."))

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

    function uploadLocal(backupCatalogId, onComplete) {
        const catalogId = String(backupCatalogId || "").trim()
        if (!catalogId.length) {
            const missing = uploadFailure(qsTr("Backup catalog ID is required."))
            applyUploadFailure(missing)
            if (typeof onComplete === "function") {
                onComplete(missing)
            }
            return false
        }
        if (uploadRunning) {
            const busy = uploadFailure(uploadOperations.busyError)
            applyUploadFailure(busy)
            if (typeof onComplete === "function") {
                onComplete(busy)
            }
            return false
        }

        uploadGeneration += 1
        const generation = uploadGeneration
        pendingUploadCatalogId = catalogId
        uploadCompletion = typeof onComplete === "function" ? onComplete : null
        error = ""
        const started = uploadOperations.start(
            "storageUploadBackupCatalogEntry",
            [catalogId, 65536],
            qsTr("Backup upload"),
            function (response, operation) {
                if (generation !== uploadGeneration || pendingUploadCatalogId !== catalogId) {
                    return
                }
                if (!response || response.ok !== true) {
                    finishUpload(uploadFailure(String(response && response.error || qsTr("Backup upload failed."))))
                    uploadOperations.clearActive()
                    return
                }
                const problem = uploadOperationProblem(operation)
                if (problem.length) {
                    finishUpload(uploadFailure(problem))
                    uploadOperations.clearActive()
                    return
                }
                if (!validUploadOperation(operation)) {
                    finishUpload(uploadFailure(qsTr("Backup upload returned invalid operation identity.")))
                    uploadOperations.clearActive()
                }
            }
        )
        return !(started && started.ok === false) && started !== null && started !== false
    }

    function pollUpload() {
        if (!uploadOperations.view.running) {
            return null
        }
        return uploadOperations.poll(false, function (response) {
            if (!response || response.ok !== true) {
                return false
            }
            const problem = uploadOperationProblem(response.value)
            if (problem.length) {
                finishUpload(uploadFailure(problem))
                uploadOperations.clearActive()
                return true
            }
            if (!validUploadOperation(response.value)) {
                finishUpload(uploadFailure(qsTr("Backup upload status returned invalid operation identity.")))
                uploadOperations.clearActive()
                return true
            }
            return false
        })
    }

    function completeUpload(operation) {
        if (!validUploadOperation(operation)) {
            finishUpload(uploadFailure(qsTr("Backup upload returned invalid operation identity.")))
            return
        }
        if (String(operation.status || "") !== "completed") {
            finishUpload(uploadFailure(String(operation.error || qsTr("Backup upload did not complete."))))
            return
        }
        const value = operation.result
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            finishUpload(uploadFailure(qsTr("Backup upload returned no result.")))
            return
        }
        const cid = String(value.cid || "").trim()
        if (!cid.length) {
            finishUpload(uploadFailure(qsTr("Backup upload returned no CID.")))
            return
        }
        upsertEntry(value.catalog_entry)
        finishUpload({
            ok: true,
            value: value,
            text: "",
            error: ""
        })
    }

    function validUploadOperation(operation) {
        const value = operation && typeof operation === "object" ? operation : null
        const activeId = String(uploadOperations.view.active && uploadOperations.view.active.operationId || "")
        return value !== null
            && String(value.operationId || "").length > 0
            && String(value.operationId) === activeId
            && String(value.domain || "") === "storage"
            && String(value.method || "") === "storageUploadBackupCatalogEntry"
    }

    function uploadOperationProblem(operation) {
        const value = operation && typeof operation === "object" ? operation : null
        if (!value || !String(value.operationId || "").length) {
            return qsTr("Backup upload returned no operation identity.")
        }
        if (String(value.domain || "") !== "storage"
                || String(value.method || "") !== "storageUploadBackupCatalogEntry") {
            return qsTr("Backup upload returned invalid operation identity.")
        }
        const status = String(value.status || "")
        const knownStatus = status === "running"
            || status === "awaiting_external"
            || status === "canceling"
            || status === "completed"
            || status === "failed"
            || status === "canceled"
            || status === "timed_out"
        if (!knownStatus) {
            return qsTr("Backup upload returned unsupported status %1.").arg(status || qsTr("unknown"))
        }
        if (status !== "completed") {
            return ""
        }
        return uploadResultProblem(value.result)
    }

    function uploadResultProblem(result) {
        const value = result && typeof result === "object" && !Array.isArray(result)
            ? result : null
        if (!value) {
            return qsTr("Backup upload returned no result.")
        }
        const expectedId = String(pendingUploadCatalogId || "").trim()
        const resultId = String(value.backup_catalog_id || "").trim()
        if (!expectedId.length || resultId !== expectedId) {
            return qsTr("Backup upload returned a different backup catalog ID.")
        }
        const cid = String(value.cid || "").trim()
        if (!cid.length) {
            return qsTr("Backup upload returned no CID.")
        }
        const entry = value.catalog_entry && typeof value.catalog_entry === "object"
                && !Array.isArray(value.catalog_entry) ? value.catalog_entry : null
        if (!entry || String(entry.backup_catalog_id || "").trim() !== expectedId) {
            return qsTr("Backup upload returned invalid catalog metadata.")
        }
        if (String(entry.remote && entry.remote.cid || "").trim() !== cid) {
            return qsTr("Backup upload returned mismatched catalog CID metadata.")
        }
        return ""
    }

    function finishUpload(response) {
        const callback = uploadCompletion
        uploadCompletion = null
        pendingUploadCatalogId = ""
        if (response && response.ok === true) {
            error = ""
        } else {
            applyUploadFailure(response || uploadFailure(qsTr("Backup upload failed.")))
        }
        if (typeof callback === "function") {
            callback(response)
        }
    }

    function applyUploadFailure(response) {
        error = String(response && response.error || qsTr("Backup upload failed."))
        revision += 1
    }

    function uploadFailure(message) {
        return {
            ok: false,
            value: null,
            text: "",
            error: String(message || qsTr("Backup upload failed."))
        }
    }

    function invalidateUpload(reason) {
        uploadGeneration += 1
        const callback = uploadCompletion
        const wasPending = pendingUploadCatalogId.length > 0
        uploadCompletion = null
        pendingUploadCatalogId = ""
        uploadOperations.clearActive()
        if (wasPending) {
            const failure = uploadFailure(String(reason || qsTr("Backup upload was invalidated.")))
            applyUploadFailure(failure)
            if (typeof callback === "function") {
                callback(failure)
            }
        }
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
