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

    function uploadOperation(id, status, cursor, result) {
        return {
            operationId: id,
            domain: "storage",
            method: "storageUploadBackupCatalogEntry",
            label: "Backup upload",
            status: status,
            eventCursor: cursor,
            result: result === undefined ? null : result,
            error: status === "failed" ? "upload failed" : ""
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

    function test_upload_local_starts_runtime_operation_and_projects_terminal_result() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: uploadOperation("upload-1", "completed", 1, {
                    cid: "z-cid",
                    backup_catalog_id: "backup-1",
                    catalog_entry: {
                        backup_catalog_id: "backup-1",
                        remote: { cid: "z-cid" }
                    }
                })
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
                value: uploadOperation("upload-1", "completed", 2, {
                    cid: "z-polled",
                    backup_catalog_id: "backup-1",
                    catalog_entry: {
                        backup_catalog_id: "backup-1",
                        remote: { cid: "z-polled" }
                    }
                })
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
                value: uploadOperation("upload-1", "completed", 1, {
                    cid: "z-wrong",
                    backup_catalog_id: "backup-2",
                    catalog_entry: {
                        backup_catalog_id: "backup-2",
                        remote: { cid: "z-wrong" }
                    }
                })
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
            value: uploadOperation("upload-1", "completed", 1, {
                cid: "z-first",
                backup_catalog_id: "backup-1",
                catalog_entry: { backup_catalog_id: "backup-1", remote: { cid: "z-first" } }
            })
        })
        verify(first !== null)
        verify(first.ok)
    }

    function test_source_invalidation_rejects_late_poll_completion() {
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
        verify(completion !== null)
        verify(!completion.ok)
        verify(catalog.error.indexOf("Storage source changed") >= 0)
        verify(catalog.revision > 0)

        gateway.completeRequestAt(0, {
            ok: true,
            value: uploadOperation("upload-1", "completed", 2, {
                cid: "z-late",
                backup_catalog_id: "backup-1",
                catalog_entry: { backup_catalog_id: "backup-1", remote: { cid: "z-late" } }
            })
        })
        compare(catalog.rows().length, 0)
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
        const terminal = uploadOperation("upload-once", "completed", 1, {
            cid: "z-once",
            backup_catalog_id: "backup-1",
            catalog_entry: {
                backup_catalog_id: "backup-1",
                remote: { cid: "z-once" }
            }
        })
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
                value: uploadOperation("upload-a", "completed", 1, {
                    cid: "z-a",
                    backup_catalog_id: "backup-a",
                    catalog_entry: {
                        backup_catalog_id: "backup-a",
                        remote: { cid: "z-a" }
                    }
                })
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
            value: uploadOperation("upload-b", "running", 1, null)
        })
        verify(catalog.uploadRunning)
        catalog.pollUpload()
        compare(gateway.pendingRequests.length, 1)
        compare(gateway.pendingRequests[0].method, "runtimeOperationStatus")
        compare(gateway.requests[gateway.requests.length - 1].args[0], "upload-b")
        gateway.completeRequestAt(0, {
            ok: true,
            value: uploadOperation("upload-b", "completed", 2, {
                cid: "z-b",
                backup_catalog_id: "backup-b",
                catalog_entry: {
                    backup_catalog_id: "backup-b",
                    remote: { cid: "z-b" }
                }
            })
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
