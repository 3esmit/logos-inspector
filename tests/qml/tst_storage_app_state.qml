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

    QtObject {
        id: gate

        property var blocked: ({})

        function storageGate(action, options) {
            const inputs = options && Array.isArray(options.required_inputs) ? options.required_inputs : []
            for (let i = 0; i < inputs.length; ++i) {
                const input = inputs[i] || {}
                const value = input.value
                if (input.present !== true && (value === undefined || value === null || String(value).trim().length === 0)) {
                    return {
                        enabled: false,
                        status: "input_required",
                        missing: [{ dependency: String(input.key || ""), label: String(input.label || input.key || ""), status: "input_required", capability: String(input.key || ""), provenance: "input" }],
                        warnings: [],
                        provenance: ["input"]
                    }
                }
            }
            const dependency = String(blocked[String(action || "")] || "")
            if (dependency.length > 0) {
                return {
                    enabled: false,
                    status: "disabled",
                    missing: [{ dependency: dependency, label: dependency, status: "unavailable", capability: "storage", provenance: "test" }],
                    warnings: [],
                    provenance: ["test"]
                }
            }
            return {
                enabled: true,
                status: "enabled",
                missing: [],
                warnings: [],
                provenance: ["test"]
            }
        }
    }

    StorageAppState {
        id: state

        gateway: gateway
        gateFacade: gate
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

    StorageAppState {
        id: stateWithoutGate

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
        gate.blocked = ({})

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
        state.operationSession.reset()

        stateWithoutGate.busy = false
        stateWithoutGate.effectiveSourceMode = "rest"
        stateWithoutGate.sourceTargetKind = "rest_endpoint"
        stateWithoutGate.usesRestEndpoint = true
        stateWithoutGate.supportsMutatingDiagnostics = true
        stateWithoutGate.mutatingDiagnosticsEnabled = true
        stateWithoutGate.manifests = []
        stateWithoutGate.operationSession.reset()
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
        compare(gateway.lastArgs[0].adapter.source_mode, "rest")
        compare(gateway.lastArgs[0].adapter.inputs.rest_endpoint, "http://storage")
        compare(state.manifests.length, 1)
        compare(state.manifestRows()[0].cid, "z-cid")
        compare(state.lastOperation, "List")
        compare(state.operation.rows.length, 1)
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
        compare(gateway.lastArgs[0].adapter.source_mode, "rest")
        compare(gateway.lastArgs[0].payload.cid, "z-cid")
        compare(state.lastOperation, "Storage exists")
        compare(state.operation.rows[0].label, "Storage exists")
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
        compare(gateway.lastArgs[0].adapter.source_mode, "rest")
        compare(gateway.lastArgs[0].adapter.inputs.rest_endpoint, "http://storage")
        compare(gateway.lastArgs[0].mutating_enabled, true)
        compare(gateway.lastArgs[0].payload.path, "/tmp/file.bin")
        compare(state.operation.active.operationId, "storage-upload-1")
        compare(state.currentTab, "operations")
        compare(state.pendingOperation.method, "")
        verify(!state.operation.startPending)
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
        compare(state.operation.active.operationId, "storage-upload-ack")
        compare(state.operation.active.status, "running")
        compare(state.operation.active.externalSessionId, "session-1")
        compare(state.operation.active.requestId, "request-1")
        compare(state.operation.active.path, "/tmp/file.bin")
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

        compare(state.operation.active.status, "running")
        compare(state.operation.rows.length, 1)

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "session-1", requestId: "request-1", success: true, cid: "z-done" })
        ]))

        compare(state.operation.active.status, "completed")
        compare(state.operation.active.cid, "z-done")
        compare(gateway.history.length, 1)
        compare(state.operation.rows.length, 2)

        verify(!state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "session-1", requestId: "request-1", success: true, cid: "z-done" })
        ]))
        compare(gateway.history.length, 1)
        compare(state.operation.rows.length, 2)
    }

    function test_terminal_operation_sets_result_and_history_once() {
        state.operationSession.acceptUpdate({
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
        state.appendTerminalStorageOperation(state.operation.active)

        compare(state.operation.active.status, "completed")
        compare(gateway.history.length, 1)
        compare(gateway.history[0].operation.operationId, "storage-download-1")
        compare(gateway.resultTitle, "Download CID")
        verify(!gateway.resultIsError)
        compare(state.lastOperation, "Complete")
    }

    function test_busy_operation_rejects_second_start() {
        state.operationSession.acceptUpdate({
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
        compare(state.operation.rows[0].status, "error")
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

    function test_gate_blocks_manifest_refresh_without_clearing_stale_rows() {
        state.manifests = [
            { cid: "z-old", filename: "old.bin", datasetSize: 4 }
        ]
        gate.blocked = ({
            manifests: "storage.manifests.read"
        })

        const response = state.refreshManifests(true)

        verify(!response.ok)
        compare(gateway.callCount, 0)
        compare(state.lastOperation, "Blocked")
        compare(state.manifestRows()[0].cid, "z-old")
        compare(state.operation.rows[0].status, "error")
        verify(response.error.indexOf("storage.manifests.read") >= 0)
    }

    function test_gate_rechecks_before_starting_mutating_operation() {
        gate.blocked = ({
            upload: "storage.content.upload"
        })

        const response = state.startStorageOperation("storageUploadUrl", ["/tmp/file.bin", 65536], "Upload file")

        verify(!response.ok)
        compare(gateway.requestCount, 0)
        compare(state.lastOperation, "Blocked")
        verify(response.error.indexOf("storage.content.upload") >= 0)
    }

    function test_gate_reports_input_required_before_sync_call() {
        const response = state.runStorage("storageExists", [""], "Storage exists")

        verify(!response.ok)
        compare(gateway.callCount, 0)
        compare(response.error.indexOf("CID") >= 0, true)
        compare(state.lastOperation, "Blocked")
    }

    function test_missing_gate_facade_fails_closed() {
        const response = stateWithoutGate.refreshManifests(true)

        verify(!response.ok)
        compare(gateway.callCount, 0)
        compare(stateWithoutGate.lastOperation, "Blocked")
        verify(response.error.indexOf("storage") >= 0)
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
        compare(gateway.lastArgs[0].adapter.source_mode, "module")
        verify(gateway.lastArgs[0].adapter.inputs.rest_endpoint === undefined)
        compare(gateway.lastArgs.length, 1)
        compare(state.manifestRows()[0].cid, "z-module")
    }

    function test_storage_module_dispatch_ack_stays_running() {
        state.operationSession.acceptUpdate({
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

        compare(state.operation.active.status, "running")
        compare(state.operation.active.externalSessionId, "1")
        compare(state.operation.active.cancellable, false)
        compare(gateway.history.length, 0)
    }

    function test_storage_module_stale_dispatch_ack_does_not_reopen_terminal_operation() {
        state.operationSession.acceptUpdate({
            operationId: "op-1",
            domain: "storage",
            backend: "module",
            method: "storageUploadUrl",
            status: "completed",
            label: "Upload",
            cid: "z-done",
            result: { cid: "z-done" }
        })
        state.operationSession.terminalOperationId = "op-1"
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

        compare(state.operation.active.status, "completed")
        compare(state.operation.active.cid, "z-done")
        compare(state.operationSession.terminalOperationId, "op-1")
    }

    function test_storage_module_events_correlate_before_mutating_operation() {
        state.operationSession.acceptUpdate({
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
        compare(state.operation.active.bytesWritten, 0)

        verify(state.applyStorageModuleEvent("storageUploadProgress", [
            JSON.stringify({ sessionId: "1", bytes: 8, totalBytes: 16 })
        ]))

        compare(state.operation.active.status, "running")
        compare(state.operation.active.bytesWritten, 8)

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "1", success: true, cid: "z-done", bytes: 8 })
        ]))

        compare(state.operation.active.status, "completed")
        compare(state.operation.active.cid, "z-done")
        compare(state.lastOperation, "Complete")
        compare(gateway.history.length, 1)
        compare(state.operation.rows.length, 1)

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
