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
    property string pendingDownloadCid: ""
    property var downloadCompletion: null
    property int downloadGeneration: 0
    readonly property bool downloadRunning: downloadOperations.view.busy
    property var activeTransferIdentity: null
    readonly property bool transferRunning: activeTransferIdentity !== null
        || uploadRunning || downloadRunning

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

    property State.SourceOperationSession downloadSession: State.SourceOperationSession {
        id: downloadOperations

        gateway: root.gateway
        domain: "storage"
        adapterInitialization: root.storageAdapterInitialization
        mutatingDiagnosticsEnabled: false
        defaultLabel: qsTr("Backup download")
        busyError: qsTr("A backup download is already running.")
        operationValidator: function (operation) {
            return root.downloadOperationProblem(operation).length === 0
        }

        onTerminalOperation: function (operation) {
            root.completeDownload(operation)
        }
    }

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
        if (!catalogMutationAllowed(qsTr("Local backup creation"))) {
            return null
        }
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
        if (!catalogMutationAllowed(qsTr("Remote backup metadata update"))) {
            return null
        }
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
        if (!catalogMutationAllowed(qsTr("Local backup restore"))) {
            return null
        }
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

    function catalogMutationAllowed(label) {
        if (!transferRunning) {
            return true
        }
        error = qsTr("%1 is blocked while a backup catalog transfer is running.")
            .arg(String(label || qsTr("Backup catalog mutation")))
        revision += 1
        return false
    }

    function beginTransferIdentity(kind, method, backupCatalogId, cid, downloadScope, mutatingEnabled) {
        const initialization = storageAdapterInitialization || ({})
        const inputs = initialization.inputs && typeof initialization.inputs === "object"
                ? initialization.inputs : ({})
        const identity = {
            kind: String(kind || ""),
            domain: "storage",
            method: String(method || ""),
            source: String(initialization.source_mode || "").trim(),
            endpoint: String(inputs.rest_endpoint || "").trim(),
            mutatingEnabled: mutatingEnabled === true,
            backupCatalogId: String(backupCatalogId || "").trim(),
            cid: String(cid || "").trim(),
            downloadScope: String(downloadScope || "")
        }
        activeTransferIdentity = identity
        return identity
    }

    function transferIdentity(kind) {
        const identity = activeTransferIdentity
                && typeof activeTransferIdentity === "object"
                && !Array.isArray(activeTransferIdentity)
            ? activeTransferIdentity : null
        return identity && String(identity.kind || "") === String(kind || "")
            ? identity : null
    }

    function releaseTransferIdentity(kind) {
        if (transferIdentity(kind)) {
            activeTransferIdentity = null
        }
    }

    function transferLabel(kind) {
        return String(kind || "") === "upload"
            ? qsTr("Backup upload") : qsTr("Backup download")
    }

    function transferOperationContextProblem(kind, operation) {
        const label = transferLabel(kind)
        const identity = transferIdentity(kind)
        const value = operation && typeof operation === "object" ? operation : null
        if (!identity) {
            return qsTr("%1 has no admitted request identity.").arg(label)
        }
        if (!value || !String(value.operationId || "").length) {
            return qsTr("%1 returned no operation identity.").arg(label)
        }
        if (String(value.domain || "") !== identity.domain
                || String(value.method || "") !== identity.method
                || String(value.backend || "").trim() !== identity.source) {
            return qsTr("%1 returned invalid operation identity.").arg(label)
        }
        const context = value.context && typeof value.context === "object"
                && !Array.isArray(value.context) ? value.context : null
        if (!context
                || String(context.source || "").trim() !== identity.source
                || String(context.endpoint || "").trim() !== identity.endpoint
                || (context.mutatingEnabled === true) !== identity.mutatingEnabled) {
            return qsTr("%1 returned different request context.").arg(label)
        }
        if (String(kind || "") === "upload") {
            return String(context.backupCatalogId || "").trim() === identity.backupCatalogId
                ? "" : qsTr("Backup upload returned different request context.")
        }
        return String(context.cid || "").trim() === identity.cid
                && String(context.downloadScope || "") === identity.downloadScope
            ? "" : qsTr("Backup download returned different request context.")
    }

    function expectedTransferResultEndpoint(identity) {
        const value = identity || ({})
        if (String(value.endpoint || "").length) {
            return String(value.endpoint)
        }
        switch (String(value.source || "")) {
        case "module":
            return "module storage_module"
        case "logoscore_cli":
            return "logoscore call storage_module"
        default:
            return ""
        }
    }

    function downloadRemote(cid, onComplete) {
        const requestedCid = String(cid || "").trim()
        if (!requestedCid.length) {
            const missing = downloadFailure(qsTr("Backup CID is required."))
            applyDownloadFailure(missing)
            if (typeof onComplete === "function") {
                onComplete(missing)
            }
            return false
        }
        if (transferRunning) {
            const busy = downloadFailure(qsTr("A backup catalog transfer is already running."))
            applyDownloadFailure(busy)
            if (typeof onComplete === "function") {
                onComplete(busy)
            }
            return false
        }

        downloadGeneration += 1
        const generation = downloadGeneration
        pendingDownloadCid = requestedCid
        downloadCompletion = typeof onComplete === "function" ? onComplete : null
        error = ""
        beginTransferIdentity(
            "download",
            "storageDownloadBackupCatalogEntry",
            "",
            requestedCid,
            "network",
            false
        )
        const started = downloadOperations.start(
            "storageDownloadBackupCatalogEntry",
            [requestedCid, false],
            qsTr("Backup download"),
            function (response, operation) {
                if (generation !== downloadGeneration || pendingDownloadCid !== requestedCid) {
                    return
                }
                if (!response || response.ok !== true) {
                    finishDownload(downloadFailure(String(response && response.error || qsTr("Backup download failed."))))
                    downloadOperations.clearActive()
                    return
                }
                const problem = downloadOperationProblem(operation)
                if (problem.length) {
                    finishDownload(downloadFailure(problem))
                    downloadOperations.clearActive()
                    return
                }
                if (!validDownloadOperation(operation)) {
                    finishDownload(downloadFailure(qsTr("Backup download returned invalid operation identity.")))
                    downloadOperations.clearActive()
                }
            }
        )
        return !(started && started.ok === false) && started !== null && started !== false
    }

    function pollDownload() {
        if (!downloadOperations.view.running) {
            return null
        }
        return downloadOperations.poll(false, function (response) {
            if (!response || response.ok !== true) {
                return false
            }
            const problem = downloadOperationProblem(response.value)
            if (problem.length) {
                finishDownload(downloadFailure(problem))
                downloadOperations.clearActive()
                return true
            }
            if (!validDownloadOperation(response.value)) {
                finishDownload(downloadFailure(qsTr("Backup download status returned invalid operation identity.")))
                downloadOperations.clearActive()
                return true
            }
            return false
        })
    }

    function completeDownload(operation) {
        if (!validDownloadOperation(operation)) {
            finishDownload(downloadFailure(qsTr("Backup download returned invalid operation identity.")))
            return
        }
        if (String(operation.status || "") !== "completed") {
            finishDownload(downloadFailure(String(operation.error || qsTr("Backup download did not complete."))))
            return
        }
        const value = operation.result
        const problem = downloadResultProblem(value)
        if (problem.length) {
            finishDownload(downloadFailure(problem))
            return
        }
        upsertEntry(value.catalog_entry)
        finishDownload({
            ok: true,
            value: value,
            text: "",
            error: ""
        })
    }

    function validDownloadOperation(operation) {
        const value = operation && typeof operation === "object" ? operation : null
        const activeId = String(downloadOperations.view.active && downloadOperations.view.active.operationId || "")
        return value !== null
            && String(value.operationId || "").length > 0
            && String(value.operationId) === activeId
            && String(value.domain || "") === "storage"
            && String(value.method || "") === "storageDownloadBackupCatalogEntry"
    }

    function downloadOperationProblem(operation) {
        const value = operation && typeof operation === "object" ? operation : null
        const contextProblem = transferOperationContextProblem("download", value)
        if (contextProblem.length) {
            return contextProblem
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
            return qsTr("Backup download returned unsupported status %1.").arg(status || qsTr("unknown"))
        }
        if (status !== "completed") {
            return ""
        }
        return downloadResultProblem(value.result)
    }

    function downloadResultProblem(result) {
        const value = result && typeof result === "object" && !Array.isArray(result)
            ? result : null
        if (!value || value.downloaded !== true || value.restored !== false) {
            return qsTr("Backup download returned invalid completion state.")
        }
        const identity = transferIdentity("download")
        if (!identity) {
            return qsTr("Backup download has no admitted request identity.")
        }
        const expectedCid = String(identity.cid || "")
        if (!expectedCid.length || String(value.cid || "").trim() !== expectedCid) {
            return qsTr("Backup download returned a different CID.")
        }
        const catalogId = String(value.backup_catalog_id || "").trim()
        const payloadId = String(value.payload_id || "").trim()
        if (!catalogId.length || !payloadId.length) {
            return qsTr("Backup download returned incomplete catalog identity.")
        }
        const entry = value.catalog_entry && typeof value.catalog_entry === "object"
                && !Array.isArray(value.catalog_entry) ? value.catalog_entry : null
        if (!entry
                || String(entry.backup_catalog_id || "").trim() !== catalogId
                || String(entry.payload_id || "").trim() !== payloadId) {
            return qsTr("Backup download returned invalid catalog metadata.")
        }
        const remote = entry.remote && typeof entry.remote === "object"
                && !Array.isArray(entry.remote) ? entry.remote : null
        if (!remote
                || String(remote.cid || "").trim() !== expectedCid
                || String(remote.provider || "") !== "logos_storage") {
            return qsTr("Backup download returned mismatched catalog CID metadata.")
        }
        if (String(value.source || "") !== String(identity.downloadScope || "")) {
            return qsTr("Backup download returned a different download scope.")
        }
        const expectedEndpoint = expectedTransferResultEndpoint(identity)
        if (String(value.endpoint || "").trim() !== expectedEndpoint) {
            return qsTr("Backup download returned a different endpoint.")
        }
        if (typeof value.bytes !== "number" || !Number.isInteger(value.bytes) || value.bytes < 0
                || typeof value.encrypted !== "boolean"
                || typeof entry.encrypted !== "boolean"
                || entry.encrypted !== value.encrypted) {
            return qsTr("Backup download returned invalid payload metadata.")
        }
        return ""
    }

    function finishDownload(response) {
        const callback = downloadCompletion
        downloadCompletion = null
        pendingDownloadCid = ""
        releaseTransferIdentity("download")
        if (response && response.ok === true) {
            error = ""
        } else {
            applyDownloadFailure(response || downloadFailure(qsTr("Backup download failed.")))
        }
        if (typeof callback === "function") {
            callback(response)
        }
    }

    function applyDownloadFailure(response) {
        error = String(response && response.error || qsTr("Backup download failed."))
        revision += 1
    }

    function downloadFailure(message) {
        return {
            ok: false,
            value: null,
            text: "",
            error: String(message || qsTr("Backup download failed."))
        }
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
        if (transferRunning) {
            const busy = uploadFailure(qsTr("A backup catalog transfer is already running."))
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
        beginTransferIdentity(
            "upload",
            "storageUploadBackupCatalogEntry",
            catalogId,
            "",
            "",
            storageMutatingDiagnosticsEnabled
        )
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
        const contextProblem = transferOperationContextProblem("upload", value)
        if (contextProblem.length) {
            return contextProblem
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
        const identity = transferIdentity("upload")
        if (!identity) {
            return qsTr("Backup upload has no admitted request identity.")
        }
        const expectedId = String(identity.backupCatalogId || "")
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
        if (!entry
                || String(entry.backup_catalog_id || "").trim() !== expectedId
                || !String(entry.payload_id || "").trim().length
                || typeof entry.encrypted !== "boolean") {
            return qsTr("Backup upload returned invalid catalog metadata.")
        }
        const remote = entry.remote && typeof entry.remote === "object"
                && !Array.isArray(entry.remote) ? entry.remote : null
        if (!remote
                || String(remote.cid || "").trim() !== cid
                || String(remote.provider || "") !== "logos_storage") {
            return qsTr("Backup upload returned mismatched catalog CID metadata.")
        }
        if (String(value.endpoint || "").trim() !== expectedTransferResultEndpoint(identity)
                || typeof value.bytes !== "number"
                || !Number.isInteger(value.bytes)
                || value.bytes < 0) {
            return qsTr("Backup upload returned invalid transfer metadata.")
        }
        return ""
    }

    function finishUpload(response) {
        const callback = uploadCompletion
        uploadCompletion = null
        pendingUploadCatalogId = ""
        releaseTransferIdentity("upload")
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
        if (uploadRunning || transferIdentity("upload")) {
            return false
        }
        uploadGeneration += 1
        const callback = uploadCompletion
        const wasPending = pendingUploadCatalogId.length > 0
        uploadCompletion = null
        pendingUploadCatalogId = ""
        uploadOperations.clearActive()
        releaseTransferIdentity("upload")
        if (wasPending) {
            const failure = uploadFailure(String(reason || qsTr("Backup upload was invalidated.")))
            applyUploadFailure(failure)
            if (typeof callback === "function") {
                callback(failure)
            }
        }
        return true
    }

    function invalidateDownload(reason) {
        if (downloadRunning || transferIdentity("download")) {
            return false
        }
        downloadGeneration += 1
        const callback = downloadCompletion
        const wasPending = pendingDownloadCid.length > 0
        downloadCompletion = null
        pendingDownloadCid = ""
        downloadOperations.clearActive()
        releaseTransferIdentity("download")
        if (wasPending) {
            const failure = downloadFailure(String(reason || qsTr("Backup download was invalidated.")))
            applyDownloadFailure(failure)
            if (typeof callback === "function") {
                callback(failure)
            }
        }
        return true
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
