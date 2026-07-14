import QtQuick
import QtTest
import "../../qml/state/domains" as Domains
import "fixtures"

TestCase {
    id: testRoot

    name: "BackupCatalogState"

    StateGatewayFixture {
        id: gateway
    }

    Domains.BackupCatalogState {
        id: catalog

        gateway: gateway
        storageAdapterInitialization: ({
            source_mode: "rest",
            inputs: { rest_endpoint: "http://storage" }
        })
        storageMutatingDiagnosticsEnabled: true
    }

    QtObject {
        id: immediateUploadModel

        property string settingsBackupStatus: ""
        property string settingsBackupCid: ""
        property string settingsRestoreCid: ""

        function settingsBackupAvailable() {
            return true
        }
    }

    QtObject {
        id: immediateUploadCatalog

        property string error: ""
        property var response: ({ ok: false, value: null, text: "", error: "upload failed" })

        function uploadLocal(backupCatalogId, callback) {
            callback(response)
            return true
        }
    }

    QtObject {
        id: unusedOperationHistory
    }

    Domains.BackupImportState {
        id: backupImport

        model: immediateUploadModel
        catalog: immediateUploadCatalog
        operationHistory: unusedOperationHistory
    }

    function init() {
        catalog.invalidateUpload("")
        catalog.invalidateDownload("")
        gateway.reset()
        catalog.entries = []
        catalog.loaded = false
        catalog.error = ""
        catalog.revision = 0
        catalog.storageAdapterInitialization = ({
            source_mode: "rest",
            inputs: { rest_endpoint: "http://storage" }
        })
        catalog.storageMutatingDiagnosticsEnabled = true
        immediateUploadModel.settingsBackupStatus = ""
        immediateUploadModel.settingsBackupCid = ""
        immediateUploadModel.settingsRestoreCid = ""
        immediateUploadCatalog.error = ""
    }

    function uploadOperation(id, status, cursor, result, catalogId) {
        const expectedCatalogId = catalogId === undefined
            ? String(result && result.backup_catalog_id || "backup-1")
            : String(catalogId)
        return {
            operationId: id,
            domain: "storage",
            method: "storageUploadBackupCatalogEntry",
            backend: "rest",
            label: "Backup upload",
            status: status,
            eventCursor: cursor,
            context: {
                source: "rest",
                endpoint: "http://storage",
                mutatingEnabled: true,
                backupCatalogId: expectedCatalogId
            },
            result: result === undefined ? null : result,
            error: status === "failed" ? "upload failed" : ""
        }
    }

    function uploadResult(catalogId, cid, payloadId) {
        const expectedCatalogId = String(catalogId || "backup-1")
        const expectedCid = String(cid || "z-cid")
        return {
            cid: expectedCid,
            bytes: 128,
            endpoint: "http://storage",
            backup_catalog_id: expectedCatalogId,
            catalog_entry: {
                backup_catalog_id: expectedCatalogId,
                payload_id: String(payloadId || "sha256:upload"),
                encrypted: false,
                remote: {
                    cid: expectedCid,
                    provider: "logos_storage"
                }
            }
        }
    }

    function downloadResult(cid, catalogId, payloadId) {
        const expectedCid = cid === undefined ? "cid-download" : String(cid)
        const expectedCatalogId = catalogId === undefined ? "backup-download" : String(catalogId)
        const expectedPayloadId = payloadId === undefined ? "sha256:download" : String(payloadId)
        return {
            downloaded: true,
            restored: false,
            cid: expectedCid,
            backup_catalog_id: expectedCatalogId,
            payload_id: expectedPayloadId,
            catalog_entry: {
                backup_catalog_id: expectedCatalogId,
                payload_id: expectedPayloadId,
                encrypted: false,
                remote: {
                    cid: expectedCid,
                    provider: "logos_storage"
                }
            },
            bytes: 128,
            endpoint: "http://storage",
            source: "network",
            encrypted: false
        }
    }

    function downloadOperation(id, status, cursor, cid, result) {
        const expectedCid = String(cid || "cid-download")
        return {
            operationId: String(id || "download-1"),
            domain: "storage",
            method: "storageDownloadBackupCatalogEntry",
            backend: "rest",
            backend: "rest",
            label: "Backup download",
            status: String(status || "completed"),
            eventCursor: Number(cursor || 1),
            context: {
                source: "rest",
                endpoint: "http://storage",
                cid: expectedCid,
                downloadScope: "network"
            },
            result: result === undefined ? null : result,
            error: status === "failed" ? "download failed" : ""
        }
    }

    function test_load_reads_catalog_entries() {
        gateway.callResponses = ({
            loadBackupCatalog: {
                ok: true,
                value: {
                    version: 1,
                    entries: [{ backup_catalog_id: "backup-1" }]
                },
                text: "OK",
                error: ""
            }
        })

        verify(catalog.load())

        compare(gateway.lastMethod, "loadBackupCatalog")
        verify(catalog.loaded)
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].backup_catalog_id, "backup-1")
    }

    function test_create_local_upserts_entry() {
        gateway.callResponses = ({
            createLocalSettingsBackup: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-1",
                    payload_id: "sha256:abc",
                    backup_version_label: "Manual"
                },
                text: "OK",
                error: ""
            }
        })

        const entry = catalog.createLocal("Manual", false, { wallet_home: "/tmp/wallet" })

        verify(entry !== null)
        compare(gateway.lastArgs[0], "Manual")
        compare(gateway.lastArgs[1], false)
        compare(gateway.lastArgs[2].wallet_home, "/tmp/wallet")
        verify(gateway.lastArgs[3] !== undefined)
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].backup_version_label, "Manual")
    }

    function test_create_local_allows_backend_default_for_empty_label() {
        gateway.callResponses = ({
            createLocalSettingsBackup: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-default",
                    payload_id: "sha256:def",
                    backup_version_label: "1720000000",
                    created_at: "1720000000"
                },
                text: "OK",
                error: ""
            }
        })

        const entry = catalog.createLocal("", false, {}, {})

        verify(entry !== null)
        compare(gateway.lastArgs[0], "")
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].backup_version_label, "1720000000")
    }

    function test_attach_remote_replaces_existing_entry_without_duplicate() {
        catalog.entries = [{
            backup_catalog_id: "backup-1",
            payload_id: "sha256:abc"
        }]
        gateway.callResponses = ({
            attachBackupRemote: {
                ok: true,
                value: {
                    backup_catalog_id: "backup-1",
                    payload_id: "sha256:abc",
                    remote: { cid: "z-cid" }
                },
                text: "OK",
                error: ""
            }
        })

        const entry = catalog.attachRemote("backup-1", "z-cid", "logos_storage")

        verify(entry !== null)
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].remote.cid, "z-cid")
    }

    function test_apply_import_calls_transaction_method() {
        gateway.callResponses = ({
            settingsBackupImportApply: {
                ok: true,
                value: {
                    restored: true,
                    backup_catalog_id: "backup-1",
                    favorites: 2,
                    idl_count: 1
                },
                text: "OK",
                error: ""
            }
        })

        const summary = catalog.applyImport("backup-1", { wallet_home: "/tmp/wallet" }, { favorites: "merge" })

        verify(summary !== null)
        compare(gateway.lastMethod, "settingsBackupImportApply")
        compare(gateway.lastArgs[0], "backup-1")
        compare(gateway.lastArgs[1].wallet_home, "/tmp/wallet")
        compare(gateway.lastArgs[2].favorites, "merge")
        compare(summary.favorites, 2)
    }

    function test_preview_import_calls_transaction_method() {
        gateway.callResponses = ({
            settingsBackupImportPreview: {
                ok: true,
                value: {
                    import_plan: true,
                    blocked: false,
                    selectedAreas: ["settings"]
                },
                text: "OK",
                error: ""
            }
        })

        const plan = catalog.previewImport("backup-1", {}, { settings: "replace" })

        verify(plan !== null)
        compare(gateway.lastMethod, "settingsBackupImportPreview")
        compare(gateway.lastArgs[0], "backup-1")
        compare(gateway.lastArgs[2].settings, "replace")
    }

    function test_download_remote_starts_canonical_operation_and_projects_terminal_catalog_entry() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: downloadOperation(
                    "download-1", "completed", 1, "cid-download",
                    downloadResult("cid-download", "backup-download", "sha256:download"))
            }
        })
        let callbackCount = 0
        let completion = null

        verify(catalog.downloadRemote("cid-download", function (response) {
            callbackCount += 1
            completion = response
        }))

        compare(gateway.lastMethod, "runtimeOperationStart")
        const request = gateway.lastArgs[0]
        compare(request.domain, "storage")
        compare(request.method, "storageDownloadBackupCatalogEntry")
        compare(request.adapter.source_mode, "rest")
        compare(request.adapter.inputs.rest_endpoint, "http://storage")
        compare(request.mutating_enabled, false)
        compare(request.payload.cid, "cid-download")
        compare(request.payload.local_only, false)
        compare(callbackCount, 1)
        verify(completion.ok)
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].backup_catalog_id, "backup-download")
        compare(catalog.rows()[0].remote.cid, "cid-download")
        compare(gateway.history.length, 1)
        compare(catalog.pollDownload(), null)
        compare(callbackCount, 1)
    }

    function test_download_remote_polls_running_operation_to_terminal_once() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: downloadOperation("download-1", "running", 1, "cid-polled", null)
            },
            runtimeOperationStatus: {
                ok: true,
                value: downloadOperation(
                    "download-1", "completed", 2, "cid-polled",
                    downloadResult("cid-polled", "backup-polled", "sha256:polled"))
            }
        })
        let completion = null

        verify(catalog.downloadRemote("cid-polled", function (response) {
            completion = response
        }))
        verify(catalog.downloadRunning)
        catalog.pollDownload()

        verify(completion !== null)
        verify(completion.ok)
        verify(!catalog.downloadRunning)
        compare(catalog.rows().length, 1)
        compare(catalog.rows()[0].backup_catalog_id, "backup-polled")
        compare(gateway.requestCount, 2)
        compare(gateway.history.length, 1)
    }

    function test_catalog_transfer_admission_serializes_downloads_and_uploads() {
        gateway.deferRequests = true
        let first = null
        let duplicate = null
        let upload = null

        verify(catalog.downloadRemote("cid-first", function (response) {
            first = response
        }))
        verify(!catalog.downloadRemote("cid-second", function (response) {
            duplicate = response
        }))
        verify(!catalog.uploadLocal("backup-upload", function (response) {
            upload = response
        }))

        compare(gateway.requestCount, 1)
        verify(duplicate !== null)
        verify(!duplicate.ok)
        verify(upload !== null)
        verify(!upload.ok)
        gateway.completeRequestAt(0, {
            ok: true,
            value: downloadOperation(
                "download-first", "completed", 1, "cid-first",
                downloadResult("cid-first", "backup-first", "sha256:first"))
        })
        verify(first !== null)
        verify(first.ok)
        compare(catalog.rows().length, 1)
    }

    function test_catalog_mutators_are_rejected_at_state_seam_during_transfer() {
        gateway.deferRequests = true
        verify(catalog.downloadRemote("cid-active", function () {}))
        const callsBeforeMutations = gateway.callCount

        compare(catalog.createLocal("Blocked", false, {}, {}), null)
        compare(catalog.attachRemote("backup-active", "cid-other", "logos_storage"), null)
        compare(catalog.applyImport("backup-active", {}, {}), null)

        compare(gateway.callCount, callsBeforeMutations)
        verify(catalog.transferRunning)
        verify(catalog.error.indexOf("blocked while a backup catalog transfer") >= 0)
        gateway.completeRequestAt(0, {
            ok: true,
            value: downloadOperation(
                "download-active", "completed", 1, "cid-active",
                downloadResult("cid-active", "backup-active", "sha256:active"))
        })
        verify(!catalog.transferRunning)
    }

    function test_download_rejects_foreign_identity_context_and_result_shapes() {
        const wrongDomain = downloadOperation(
            "wrong-domain", "completed", 1, "cid-test",
            downloadResult("cid-test", "backup-test", "sha256:test"))
        wrongDomain.domain = "delivery"
        const wrongMethod = downloadOperation(
            "wrong-method", "completed", 1, "cid-test",
            downloadResult("cid-test", "backup-test", "sha256:test"))
        wrongMethod.method = "storageDownloadManifest"
        const wrongBackend = downloadOperation(
            "wrong-backend", "completed", 1, "cid-test",
            downloadResult("cid-test", "backup-test", "sha256:test"))
        wrongBackend.backend = "module"
        const wrongContext = downloadOperation(
            "wrong-context", "completed", 1, "cid-other",
            downloadResult("cid-test", "backup-test", "sha256:test"))
        const wrongCid = downloadOperation(
            "wrong-cid", "completed", 1, "cid-test",
            downloadResult("cid-other", "backup-test", "sha256:test"))
        const missingCatalog = downloadOperation(
            "missing-catalog", "completed", 1, "cid-test",
            downloadResult("cid-test", "", "sha256:test"))
        const wrongPayload = downloadResult("cid-test", "backup-test", "sha256:test")
        wrongPayload.catalog_entry.payload_id = "sha256:other"
        const mismatchedPayload = downloadOperation(
            "wrong-payload", "completed", 1, "cid-test", wrongPayload)
        const wrongRemote = downloadResult("cid-test", "backup-test", "sha256:test")
        wrongRemote.catalog_entry.remote.cid = "cid-other"
        const mismatchedRemote = downloadOperation(
            "wrong-remote", "completed", 1, "cid-test", wrongRemote)
        const wrongScope = downloadResult("cid-test", "backup-test", "sha256:test")
        wrongScope.source = "local"
        const mismatchedScope = downloadOperation(
            "wrong-scope", "completed", 1, "cid-test", wrongScope)
        const wrongEndpoint = downloadResult("cid-test", "backup-test", "sha256:test")
        wrongEndpoint.endpoint = "http://other-storage"
        const mismatchedEndpoint = downloadOperation(
            "wrong-endpoint", "completed", 1, "cid-test", wrongEndpoint)
        const wrongEncryption = downloadResult("cid-test", "backup-test", "sha256:test")
        wrongEncryption.encrypted = true
        const mismatchedEncryption = downloadOperation(
            "wrong-encryption", "completed", 1, "cid-test", wrongEncryption)
        const wrongProvider = downloadResult("cid-test", "backup-test", "sha256:test")
        wrongProvider.catalog_entry.remote.provider = "other_storage"
        const mismatchedProvider = downloadOperation(
            "wrong-provider", "completed", 1, "cid-test", wrongProvider)
        const unknownStatus = downloadOperation("unknown", "mystery", 1, "cid-test", null)
        const cases = [
            wrongDomain,
            wrongMethod,
            wrongBackend,
            wrongContext,
            wrongCid,
            missingCatalog,
            mismatchedPayload,
            mismatchedRemote,
            mismatchedScope,
            mismatchedEndpoint,
            mismatchedEncryption,
            mismatchedProvider,
            unknownStatus
        ]

        for (let i = 0; i < cases.length; ++i) {
            gateway.requestResponses = ({
                runtimeOperationStart: { ok: true, value: cases[i] }
            })
            let completion = null
            verify(catalog.downloadRemote("cid-test", function (response) {
                completion = response
            }))
            verify(completion !== null)
            verify(!completion.ok)
            verify(!catalog.downloadRunning)
            compare(catalog.rows().length, 0)
        }
        compare(gateway.history.length, 0)
    }

    function test_download_rejects_foreign_endpoint_when_admitted_context_has_none() {
        catalog.storageAdapterInitialization = ({
            source_mode: "logoscore_cli",
            inputs: ({})
        })
        const operation = downloadOperation(
            "download-no-endpoint", "running", 1, "cid-no-endpoint", null)
        operation.backend = "logoscore_cli"
        operation.context.source = "logoscore_cli"
        operation.context.endpoint = "http://foreign-storage"
        gateway.requestResponses = ({
            runtimeOperationStart: { ok: true, value: operation }
        })
        let completion = null

        verify(catalog.downloadRemote("cid-no-endpoint", function (response) {
            completion = response
        }))

        verify(completion !== null)
        verify(!completion.ok)
        verify(completion.error.indexOf("different request context") >= 0)
        verify(!catalog.transferRunning)
    }

    function test_download_terminal_failures_complete_once_without_projection() {
        const statuses = ["failed", "canceled", "timed_out"]
        let callbackCount = 0

        for (let i = 0; i < statuses.length; ++i) {
            gateway.requestResponses = ({
                runtimeOperationStart: {
                    ok: true,
                    value: downloadOperation(
                        "download-" + statuses[i], statuses[i], 1, "cid-failure", null)
                }
            })
            let completion = null
            verify(catalog.downloadRemote("cid-failure", function (response) {
                callbackCount += 1
                completion = response
            }))
            verify(completion !== null)
            verify(!completion.ok)
            compare(catalog.rows().length, 0)
        }

        compare(callbackCount, statuses.length)
        compare(gateway.history.length, statuses.length)
    }

    function test_download_source_change_retains_running_transfer_until_old_terminal() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: downloadOperation("download-1", "running", 1, "cid-late", null)
            }
        })
        let callbackCount = 0
        let completion = null
        verify(catalog.downloadRemote("cid-late", function (response) {
            callbackCount += 1
            completion = response
        }))
        gateway.deferRequests = true
        catalog.pollDownload()
        catalog.storageAdapterInitialization = ({
            source_mode: "rest",
            inputs: { rest_endpoint: "http://other-storage" }
        })

        compare(callbackCount, 0)
        verify(completion === null)
        verify(catalog.downloadRunning)
        verify(catalog.transferRunning)
        verify(!catalog.invalidateDownload("must remain tracked"))
        const requestsBeforeBlockedTransfer = gateway.requestCount
        verify(!catalog.uploadLocal("backup-blocked", function () {}))
        compare(gateway.requestCount, requestsBeforeBlockedTransfer)
        gateway.completeRequestAt(0, {
            ok: true,
            value: downloadOperation(
                "download-1", "completed", 2, "cid-late",
                downloadResult("cid-late", "backup-late", "sha256:late"))
        })
        compare(callbackCount, 1)
        verify(completion !== null)
        verify(completion.ok)
        verify(!catalog.transferRunning)
        compare(catalog.rows().length, 1)
        compare(gateway.history.length, 1)
    }

    function test_download_source_change_retains_pending_start_and_serializes_new_transfer() {
        gateway.deferRequests = true
        let callbackCount = 0
        let completion = null
        verify(catalog.downloadRemote("cid-start", function (response) {
            callbackCount += 1
            completion = response
        }))
        catalog.storageAdapterInitialization = ({
            source_mode: "rest",
            inputs: { rest_endpoint: "http://replacement" }
        })

        compare(callbackCount, 0)
        verify(completion === null)
        verify(catalog.transferRunning)
        verify(!catalog.downloadRemote("cid-replacement", function () {}))
        verify(!catalog.uploadLocal("backup-replacement", function () {}))
        compare(gateway.requestCount, 1)
        gateway.completeRequestAt(0, {
            ok: true,
            value: downloadOperation(
                "download-start", "completed", 1, "cid-start",
                downloadResult("cid-start", "backup-start", "sha256:start"))
        })
        compare(callbackCount, 1)
        verify(completion !== null)
        verify(completion.ok)
        verify(!catalog.transferRunning)
        compare(catalog.rows().length, 1)
        compare(gateway.history.length, 1)
    }

    function test_download_only_records_catalog_and_never_calls_import_methods() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: downloadOperation(
                    "download-only", "completed", 1, "cid-only",
                    downloadResult("cid-only", "backup-only", "sha256:only"))
            }
        })

        verify(catalog.downloadRemote("cid-only", function () {}))

        compare(catalog.rows().length, 1)
        verify(!gateway.calls.some(function (call) {
            return call.method === "settingsBackupImportPreview"
                || call.method === "settingsBackupImportApply"
                || call.method === "loadBackupCatalog"
        }))
    }

    function test_upload_local_starts_runtime_operation_and_projects_terminal_result() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation(
                    "upload-1", "completed", 1,
                    uploadResult("backup-1", "z-cid", "sha256:upload-1"))
            }
        })
        let completion = null

        verify(catalog.uploadLocal("backup-1", function (response) {
            completion = response
        }))

        compare(gateway.lastMethod, "runtimeOperationStart")
        const request = gateway.lastArgs[0]
        compare(request.domain, "storage")
        compare(request.method, "storageUploadBackupCatalogEntry")
        compare(request.adapter.source_mode, "rest")
        compare(request.adapter.inputs.rest_endpoint, "http://storage")
        compare(request.payload.backup_catalog_id, "backup-1")
        compare(request.payload.block_size, 65536)
        verify(completion !== null)
        verify(completion.ok)
        compare(completion.value.cid, "z-cid")
        compare(catalog.rows()[0].remote.cid, "z-cid")
        compare(gateway.history.length, 1)
    }

    function test_upload_local_polls_one_running_operation_to_completion() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation("upload-1", "running", 1, null)
            },
            runtimeOperationStatus: {
                ok: true,
                value: uploadOperation(
                    "upload-1", "completed", 2,
                    uploadResult("backup-1", "z-polled", "sha256:polled"))
            }
        })
        let completion = null

        verify(catalog.uploadLocal("backup-1", function (response) {
            completion = response
        }))
        verify(catalog.uploadRunning)
        catalog.pollUpload()

        verify(completion !== null)
        verify(completion.ok)
        compare(completion.value.cid, "z-polled")
        verify(!catalog.uploadRunning)
        compare(gateway.requestCount, 2)
    }

    function test_upload_local_rejects_dispatched_or_foreign_terminal_identity() {
        const responses = []
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation("upload-1", "dispatched", 1, null)
            }
        })
        verify(catalog.uploadLocal("backup-1", function (response) {
            responses.push(response)
        }))
        verify(!responses[0].ok)

        const foreign = uploadOperation("upload-2", "completed", 1, { cid: "wrong" })
        foreign.domain = "delivery"
        gateway.requestResponses = ({
            runtimeOperationStart: { ok: true, value: foreign }
        })
        verify(catalog.uploadLocal("backup-2", function (response) {
            responses.push(response)
        }))
        verify(!responses[1].ok)
        compare(catalog.rows().length, 0)
        compare(gateway.history.length, 0)
    }

    function test_upload_rejects_mismatched_backup_result_identity() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation(
                    "upload-1", "completed", 1,
                    uploadResult("backup-2", "z-wrong", "sha256:wrong"),
                    "backup-1")
            }
        })
        let completion = null

        verify(catalog.uploadLocal("backup-1", function (response) {
            completion = response
        }))

        verify(completion !== null)
        verify(!completion.ok)
        verify(completion.error.indexOf("different backup catalog ID") >= 0)
        compare(catalog.rows().length, 0)
        compare(gateway.history.length, 0)
    }

    function test_upload_rejects_foreign_context_and_transfer_metadata() {
        const valid = uploadResult("backup-1", "z-valid", "sha256:valid")
        const wrongBackend = uploadOperation(
            "upload-wrong-backend", "completed", 1, valid)
        wrongBackend.backend = "logoscore_cli"
        const wrongEndpointContext = uploadOperation(
            "upload-wrong-context-endpoint", "completed", 1, valid)
        wrongEndpointContext.context.endpoint = "http://other-storage"
        const wrongCatalogContext = uploadOperation(
            "upload-wrong-context-catalog", "completed", 1, valid)
        wrongCatalogContext.context.backupCatalogId = "backup-2"
        const wrongMutatingContext = uploadOperation(
            "upload-wrong-context-mutating", "completed", 1, valid)
        wrongMutatingContext.context.mutatingEnabled = false
        const wrongResultEndpointValue = uploadResult(
            "backup-1", "z-endpoint", "sha256:endpoint")
        wrongResultEndpointValue.endpoint = "http://other-storage"
        const wrongResultEndpoint = uploadOperation(
            "upload-wrong-result-endpoint", "completed", 1, wrongResultEndpointValue)
        const missingPayloadValue = uploadResult(
            "backup-1", "z-payload", "sha256:payload")
        missingPayloadValue.catalog_entry.payload_id = ""
        const missingPayload = uploadOperation(
            "upload-missing-payload", "completed", 1, missingPayloadValue)
        const wrongProviderValue = uploadResult(
            "backup-1", "z-provider", "sha256:provider")
        wrongProviderValue.catalog_entry.remote.provider = "other_storage"
        const wrongProvider = uploadOperation(
            "upload-wrong-provider", "completed", 1, wrongProviderValue)
        const wrongBytesValue = uploadResult("backup-1", "z-bytes", "sha256:bytes")
        wrongBytesValue.bytes = -1
        const wrongBytes = uploadOperation(
            "upload-wrong-bytes", "completed", 1, wrongBytesValue)
        const cases = [
            wrongBackend,
            wrongEndpointContext,
            wrongCatalogContext,
            wrongMutatingContext,
            wrongResultEndpoint,
            missingPayload,
            wrongProvider,
            wrongBytes
        ]

        for (let i = 0; i < cases.length; ++i) {
            gateway.requestResponses = ({
                runtimeOperationStart: { ok: true, value: cases[i] }
            })
            let completion = null
            verify(catalog.uploadLocal("backup-1", function (response) {
                completion = response
            }))
            verify(completion !== null)
            verify(!completion.ok)
            verify(!catalog.transferRunning)
            compare(catalog.rows().length, 0)
        }
        compare(gateway.history.length, 0)
    }

    function test_upload_rejects_unknown_status_and_malformed_completion() {
        const cases = [
            uploadOperation("upload-unknown", "mystery", 1, null),
            uploadOperation("upload-empty", "completed", 1, null),
            uploadOperation("upload-blank", "completed", 1, {
                cid: "",
                backup_catalog_id: "backup-1",
                catalog_entry: {
                    backup_catalog_id: "backup-1",
                    remote: { cid: "" }
                }
            })
        ]

        for (let i = 0; i < cases.length; ++i) {
            gateway.requestResponses = ({
                runtimeOperationStart: { ok: true, value: cases[i] }
            })
            let completion = null
            verify(catalog.uploadLocal("backup-1", function (response) {
                completion = response
            }))
            verify(completion !== null)
            verify(!completion.ok)
            verify(!catalog.uploadRunning)
            compare(catalog.rows().length, 0)
        }
        compare(gateway.history.length, 0)
    }

    function test_terminal_failure_completes_callback_once_without_upsert() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation("upload-failed", "failed", 1, null)
            }
        })
        let callbackCount = 0
        let completion = null

        verify(catalog.uploadLocal("backup-1", function (response) {
            callbackCount += 1
            completion = response
        }))

        compare(callbackCount, 1)
        verify(completion !== null)
        verify(!completion.ok)
        compare(catalog.rows().length, 0)
        compare(gateway.history.length, 1)
    }

    function test_duplicate_upload_is_rejected_without_second_start() {
        gateway.deferRequests = true
        let first = null
        let second = null
        verify(catalog.uploadLocal("backup-1", function (response) {
            first = response
        }))
        verify(!catalog.uploadLocal("backup-2", function (response) {
            second = response
        }))

        compare(gateway.requestCount, 1)
        verify(second !== null)
        verify(!second.ok)
        gateway.completeRequestAt(0, {
            ok: true,
            value: uploadOperation(
                "upload-1", "completed", 1,
                uploadResult("backup-1", "z-first", "sha256:first"))
        })
        verify(first !== null)
        verify(first.ok)
    }

    function test_upload_source_change_retains_running_transfer_until_old_terminal() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation("upload-1", "running", 1, null)
            }
        })
        let completion = null
        verify(catalog.uploadLocal("backup-1", function (response) {
            completion = response
        }))
        gateway.deferRequests = true
        catalog.pollUpload()
        catalog.storageAdapterInitialization = ({
            source_mode: "rest",
            inputs: { rest_endpoint: "http://other-storage" }
        })
        verify(completion === null)
        verify(catalog.uploadRunning)
        verify(catalog.transferRunning)
        verify(!catalog.invalidateUpload("must remain tracked"))
        const requestsBeforeBlockedTransfer = gateway.requestCount
        verify(!catalog.downloadRemote("cid-blocked", function () {}))
        compare(gateway.requestCount, requestsBeforeBlockedTransfer)

        gateway.completeRequestAt(0, {
            ok: true,
            value: uploadOperation(
                "upload-1", "completed", 2,
                uploadResult("backup-1", "z-late", "sha256:late"))
        })
        verify(completion !== null)
        verify(completion.ok)
        verify(!catalog.transferRunning)
        compare(catalog.rows().length, 1)
        compare(gateway.history.length, 1)
    }

    function test_unknown_polled_status_fails_closed() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation("upload-1", "running", 1, null)
            },
            runtimeOperationStatus: {
                ok: true,
                value: uploadOperation("upload-1", "mystery", 2, null)
            }
        })
        let completion = null

        verify(catalog.uploadLocal("backup-1", function (response) {
            completion = response
        }))
        verify(catalog.uploadRunning)
        catalog.pollUpload()

        verify(completion !== null)
        verify(!completion.ok)
        verify(!catalog.uploadRunning)
        compare(gateway.history.length, 0)
    }

    function test_successful_projection_is_applied_once() {
        const terminal = uploadOperation(
            "upload-once", "completed", 1,
            uploadResult("backup-1", "z-once", "sha256:once"))
        gateway.requestResponses = ({
            runtimeOperationStart: { ok: true, value: terminal }
        })
        let callbackCount = 0

        verify(catalog.uploadLocal("backup-1", function () {
            callbackCount += 1
        }))
        const revision = catalog.revision
        compare(callbackCount, 1)
        compare(catalog.rows().length, 1)
        compare(gateway.history.length, 1)
        compare(catalog.pollUpload(), null)
        compare(callbackCount, 1)
        compare(catalog.revision, revision)
        compare(gateway.history.length, 1)
    }

    function test_deferred_second_start_never_polls_prior_terminal_identity() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation(
                    "upload-a", "completed", 1,
                    uploadResult("backup-a", "z-a", "sha256:a"))
            }
        })
        verify(catalog.uploadLocal("backup-a", function () {}))

        gateway.deferRequests = true
        let secondCompletion = null
        verify(catalog.uploadLocal("backup-b", function (response) {
            secondCompletion = response
        }))
        compare(gateway.pendingRequests.length, 1)
        const requestsBeforePoll = gateway.requestCount

        compare(catalog.pollUpload(), null)
        compare(gateway.requestCount, requestsBeforePoll)

        gateway.completeRequestAt(0, {
            ok: true,
            value: uploadOperation("upload-b", "running", 1, null, "backup-b")
        })
        verify(catalog.uploadRunning)
        catalog.pollUpload()
        compare(gateway.pendingRequests.length, 1)
        compare(gateway.pendingRequests[0].method, "runtimeOperationStatus")
        compare(gateway.requests[gateway.requests.length - 1].args[0], "upload-b")
        gateway.completeRequestAt(0, {
            ok: true,
            value: uploadOperation(
                "upload-b", "completed", 2,
                uploadResult("backup-b", "z-b", "sha256:b"))
        })

        verify(secondCompletion !== null)
        verify(secondCompletion.ok)
        compare(secondCompletion.value.backup_catalog_id, "backup-b")
    }

    function test_callbackless_import_keeps_inline_terminal_status() {
        immediateUploadCatalog.response = ({
            ok: true,
            value: { cid: "z-inline" },
            text: "",
            error: ""
        })

        verify(backupImport.uploadBackupCatalogEntry("backup-1"))

        compare(immediateUploadModel.settingsBackupCid, "z-inline")
        compare(immediateUploadModel.settingsRestoreCid, "z-inline")
        compare(immediateUploadModel.settingsBackupStatus, "Backup uploaded as z-inline.")
    }
}
