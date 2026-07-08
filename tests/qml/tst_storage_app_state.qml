import QtQuick
import QtTest
import "../../qml/state"
import "fixtures"

TestCase {
    id: testRoot

    name: "StorageAppState"

    StateGatewayFixture {
        id: gateway
    }

    StorageAppState {
        id: state

        gateway: gateway
        busy: false
        effectiveSourceMode: "rest"
        sourceLabel: "Direct REST"
        sourceTarget: "http://storage"
        sourceTargetKind: "rest_endpoint"
        usesRestEndpoint: true
        supportsMutatingDiagnostics: true
        restEndpoint: "http://storage"
        moduleName: "storage_module"
        networkPreset: "logos.test"
        mutatingDiagnosticsEnabled: true
        currentView: "storage"
    }

    function init() {
        gateway.reset()

        state.busy = false
        state.effectiveSourceMode = "rest"
        state.sourceLabel = "Direct REST"
        state.sourceTarget = "http://storage"
        state.sourceTargetKind = "rest_endpoint"
        state.usesRestEndpoint = true
        state.supportsMutatingDiagnostics = true
        state.restEndpoint = "http://storage"
        state.networkPreset = "logos.test"
        state.mutatingDiagnosticsEnabled = true
        state.currentTab = "files"
        state.cidProbe = ""
        state.activeCid = ""
        state.resultTitle = ""
        state.resultText = ""
        state.resultIsError = false
        state.resultOwner = ""
        state.sourceReport = null
        state.manifests = []
        state.lastOperation = "None"
        state.pendingMethod = ""
        state.pendingLabel = ""
        state.pendingArgs = []
        state.terminalOperationId = ""
        state.operationStartPending = false
        state.activeOperation = null
        state.activeOperationRevision = 0
        state.operationLog = []
        state.operationLogRevision = 0
    }

    function test_refresh_manifests_updates_local_state() {
        gateway.callResponses = ({
            storageManifests: {
                ok: true,
                value: {
                    content: [
                        { cid: "z-cid", manifest: { filename: "file.bin", datasetSize: 12, blockSize: 4 } }
                    ]
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(true)

        compare(gateway.callCount, 1)
        compare(gateway.lastMethod, "storageManifests")
        compare(gateway.lastArgs[0], "rest")
        compare(gateway.lastArgs[1], "http://storage")
        compare(state.manifests.length, 1)
        compare(state.manifestRows()[0].cid, "z-cid")
        compare(state.lastOperation, "List")
        compare(state.operationLog.length, 1)
    }

    function test_run_storage_exists_uses_sync_call_and_log() {
        gateway.callResponses = ({
            storageExists: {
                ok: true,
                value: { exists: true },
                text: "OK",
                error: ""
            }
        })

        const response = state.runStorage("storageExists", ["z-cid"], "Storage exists")

        verify(response.ok)
        compare(gateway.lastMethod, "storageExists")
        compare(gateway.lastArgs[0], "rest")
        compare(gateway.lastArgs[2], "z-cid")
        compare(state.lastOperation, "Storage exists")
        compare(state.operationRows()[0].label, "Storage exists")
    }

    function test_pending_mutation_starts_node_operation() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-upload-1",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "running",
                    label: "Upload file",
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.confirmStorage("storageUploadUrl", ["/tmp/file.bin", 65536], "Upload file")
        state.runPendingStorage()

        compare(gateway.requestCount, 1)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].domain, "storage")
        compare(gateway.lastArgs[0].sourceMode, "rest")
        compare(gateway.lastArgs[0].args[0], "rest")
        compare(gateway.lastArgs[0].args[2], true)
        compare(gateway.lastArgs[0].args[3], "/tmp/file.bin")
        compare(state.activeOperation.operationId, "storage-upload-1")
        compare(state.currentTab, "operations")
        compare(state.pendingMethod, "")
        verify(!state.operationStartPending)
    }

    function test_start_dispatch_ack_becomes_running_module_operation() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-upload-ack",
                    domain: "storage",
                    backend: "module",
                    method: "storageUploadUrl",
                    status: "completed",
                    label: "Upload file",
                    result: {
                        dispatched: true,
                        sessionId: "session-1",
                        requestId: "request-1",
                        path: "/tmp/file.bin"
                    },
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.startStorageOperation("storageUploadUrl", ["/tmp/file.bin", 65536], "Upload file")

        compare(gateway.requestCount, 1)
        compare(state.activeOperation.operationId, "storage-upload-ack")
        compare(state.activeOperation.status, "running")
        compare(state.activeOperation.externalSessionId, "session-1")
        compare(state.activeOperation.requestId, "request-1")
        compare(state.activeOperation.path, "/tmp/file.bin")
        compare(state.lastOperation, "Running")
        compare(state.currentTab, "operations")
        compare(gateway.history.length, 0)
    }

    function test_storage_dispatch_ack_and_module_terminal_share_history() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-upload-ack",
                    domain: "storage",
                    backend: "module",
                    method: "storageUploadUrl",
                    status: "completed",
                    label: "Upload file",
                    result: {
                        dispatched: true,
                        sessionId: "session-1",
                        requestId: "request-1",
                        path: "/tmp/file.bin"
                    },
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.startStorageOperation("storageUploadUrl", ["/tmp/file.bin", 65536], "Upload file")

        compare(state.activeOperation.status, "running")
        compare(state.operationLog.length, 1)

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "session-1", requestId: "request-1", success: true, cid: "z-done" })
        ]))

        compare(state.activeOperation.status, "completed")
        compare(state.activeOperation.cid, "z-done")
        compare(gateway.history.length, 1)
        compare(state.operationLog.length, 2)

        verify(!state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "session-1", requestId: "request-1", success: true, cid: "z-done" })
        ]))
        compare(gateway.history.length, 1)
        compare(state.operationLog.length, 2)
    }

    function test_terminal_operation_sets_result_and_history_once() {
        state.updateActiveOperation({
            operationId: "storage-download-1",
            domain: "storage",
            method: "storageDownloadToUrl",
            status: "running",
            label: "Download CID",
            cid: "z-cid",
            path: "/tmp/file.bin",
            bytesWritten: 8,
            contentLength: 8
        })
        gateway.requestResponses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "storage-download-1",
                    domain: "storage",
                    method: "storageDownloadToUrl",
                    status: "completed",
                    label: "Download CID",
                    cid: "z-cid",
                    path: "/tmp/file.bin",
                    bytesWritten: 8,
                    contentLength: 8,
                    result: { cid: "z-cid", path: "/tmp/file.bin" }
                },
                text: "OK",
                error: ""
            }
        })

        state.pollStorageOperation(false)
        state.appendTerminalStorageOperation(state.activeOperation)

        compare(state.activeOperation.status, "completed")
        compare(gateway.history.length, 1)
        compare(gateway.history[0].operation.operationId, "storage-download-1")
        compare(gateway.resultTitle, "Download CID")
        verify(!gateway.resultIsError)
        compare(state.lastOperation, "Complete")
    }

    function test_busy_operation_rejects_second_start() {
        state.updateActiveOperation({
            operationId: "storage-download-1",
            domain: "storage",
            method: "storageDownloadToUrl",
            status: "running",
            label: "Download CID"
        })

        const response = state.startStorageOperation("storageRemove", ["z-cid"], "Remove CID")

        verify(!response.ok)
        compare(gateway.requestCount, 0)
        compare(state.lastOperation, "Busy")
        compare(state.operationRows()[0].status, "error")
    }

    function test_source_and_cid_helpers() {
        state.cidProbe = "z-old"
        compare(state.activeCid, "z-old")

        state.setCidProbe(" z-new ")

        compare(state.cidProbe, "z-new")
        compare(state.storageRestSource(), true)
        compare(state.storageMutatingSource(), true)
        compare(state.sourceBadges()[0], "Storage")

        state.openStorageSettings()
        compare(gateway.openedSection, "network")
        compare(gateway.openedSubSection, "storage")
    }

    function test_module_source_can_refresh_manifests() {
        state.effectiveSourceMode = "module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false
        gateway.callResponses = ({
            storageManifests: {
                ok: true,
                value: [
                    { cid: "z-module", filename: "module.bin", datasetSize: 7 }
                ],
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(true)

        compare(gateway.lastMethod, "storageManifests")
        compare(gateway.lastArgs[0], "module")
        compare(gateway.lastArgs[1], "")
        compare(state.manifestRows()[0].cid, "z-module")
    }

    function test_storage_module_dispatch_ack_stays_running() {
        state.updateActiveOperation({
            operationId: "op-1",
            domain: "storage",
            backend: "module",
            method: "storageUploadUrl",
            status: "running",
            label: "Upload"
        })
        gateway.requestResponses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "op-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageUploadUrl",
                    status: "completed",
                    label: "Upload",
                    result: { dispatched: true, sessionId: "1" },
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.pollStorageOperation(false)

        compare(state.activeOperation.status, "running")
        compare(state.activeOperation.externalSessionId, "1")
        compare(state.activeOperation.cancellable, false)
        compare(gateway.history.length, 0)
    }

    function test_storage_module_stale_dispatch_ack_does_not_reopen_terminal_operation() {
        state.updateActiveOperation({
            operationId: "op-1",
            domain: "storage",
            backend: "module",
            method: "storageUploadUrl",
            status: "completed",
            label: "Upload",
            cid: "z-done",
            result: { cid: "z-done" }
        })
        state.terminalOperationId = "op-1"
        gateway.requestResponses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "op-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageUploadUrl",
                    status: "completed",
                    label: "Upload",
                    result: { dispatched: true, sessionId: "1" },
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.pollStorageOperation(false)

        compare(state.activeOperation.status, "completed")
        compare(state.activeOperation.cid, "z-done")
        compare(state.terminalOperationId, "op-1")
    }

    function test_storage_module_events_correlate_before_mutating_operation() {
        state.updateActiveOperation({
            operationId: "op-1",
            domain: "storage",
            backend: "module",
            method: "storageUploadUrl",
            status: "running",
            label: "Upload",
            externalSessionId: "1",
            bytesWritten: 0,
            contentLength: 16
        })

        verify(!state.applyStorageModuleEvent("storageUploadProgress", [
            JSON.stringify({ sessionId: "2", bytes: 8 })
        ]))
        compare(state.activeOperation.bytesWritten, 0)

        verify(state.applyStorageModuleEvent("storageUploadProgress", [
            JSON.stringify({ sessionId: "1", bytes: 8, totalBytes: 16 })
        ]))

        compare(state.activeOperation.status, "running")
        compare(state.activeOperation.bytesWritten, 8)

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "1", success: true, cid: "z-done", bytes: 8 })
        ]))

        compare(state.activeOperation.status, "completed")
        compare(state.activeOperation.cid, "z-done")
        compare(state.lastOperation, "Complete")
        compare(gateway.history.length, 1)
        compare(state.operationLog.length, 1)

        verify(!state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "1", success: true, cid: "z-done", bytes: 8 })
        ]))
        compare(gateway.history.length, 1)
    }

    function test_storage_space_summary_uses_source_report_probe() {
        state.sourceReport = {
            probe_facts: [
                {
                    key: "space",
                    ok: true,
                    value: {
                        quotaUsedBytes: 75,
                        quotaMaxBytes: 100
                    }
                }
            ]
        }

        compare(state.storageSpaceSummary(), "75% used")
        compare(state.storageSpaceTone(), "warning")
    }
}
