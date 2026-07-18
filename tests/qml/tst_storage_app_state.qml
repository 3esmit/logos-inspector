import QtQuick
import QtTest
import "../../qml/state"
import "../../qml/state/modules/StorageModuleEvents.js" as StorageModuleEvents
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
        property bool manifestsLoading: false
        property int revision: 0

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
            if (String(action || "") === "manifests" && manifestsLoading) {
                return {
                    enabled: false,
                    status: "loading",
                    missing: [{ dependency: "storage.manifests.read", label: "Storage manifests", status: "loading", capability: "storage", provenance: "test" }],
                    warnings: [],
                    provenance: ["test"]
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
        gate.manifestsLoading = false
        gate.revision = 0

        state.busy = false
        state.currentView = ""
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
        state.manifestRequestGeneration = 0
        state.manifestBootstrapGeneration = 0
        state.manifestRefreshDeferred = false
        state.manifestObservationPending = false
        state.manifestDeferredShowLog = false
        state.manifestBusyDeferred = false
        state.manifestBusyShowLog = false
        state.diagnosticRequestGeneration = 0
        state.manifestRefreshContext = null
        state.operationSession.reset()
        state.currentView = "storage"

        stateWithoutGate.busy = false
        stateWithoutGate.currentView = ""
        stateWithoutGate.effectiveSourceMode = "rest"
        stateWithoutGate.sourceTargetKind = "rest_endpoint"
        stateWithoutGate.usesRestEndpoint = true
        stateWithoutGate.supportsMutatingDiagnostics = true
        stateWithoutGate.mutatingDiagnosticsEnabled = true
        stateWithoutGate.manifests = []
        stateWithoutGate.manifestRequestGeneration = 0
        stateWithoutGate.manifestBootstrapGeneration = 0
        stateWithoutGate.manifestRefreshDeferred = false
        stateWithoutGate.manifestObservationPending = false
        stateWithoutGate.manifestDeferredShowLog = false
        stateWithoutGate.manifestBusyDeferred = false
        stateWithoutGate.manifestBusyShowLog = false
        stateWithoutGate.diagnosticRequestGeneration = 0
        stateWithoutGate.manifestRefreshContext = null
        stateWithoutGate.operationSession.reset()
    }

    function test_refresh_manifests_updates_local_state() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-1",
                    domain: "storage",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: {
                        content: [
                            { cid: "z-cid", manifest: { filename: "file.bin", datasetSize: 12, blockSize: 4 } }
                        ]
                    },
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(true)

        compare(gateway.requestCount, 1)
        compare(gateway.callCount, 0)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].method, "storageManifests")
        compare(gateway.lastArgs[0].domain, "storage")
        compare(gateway.lastArgs[0].adapter.source_mode, "rest")
        compare(gateway.lastArgs[0].adapter.inputs.rest_endpoint, "http://storage")
        compare(state.manifests.length, 1)
        compare(state.manifestRows()[0].cid, "z-cid")
        compare(state.lastOperation, "List")
        compare(state.operation.rows.length, 2)
    }

    function test_loading_manifest_gate_observes_source_then_retries_once() {
        gate.manifestsLoading = true
        gateway.deferStorageObservations = true
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-bootstrap-1",
                    domain: "storage",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: [{ cid: "z-bootstrap", filename: "bootstrap.bin", datasetSize: 13 }],
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(false)
        state.refreshManifests(false)

        compare(gateway.storageObservationCount, 1)
        compare(gateway.requestCount, 0)
        compare(state.lastOperation, "Loading")

        verify(gateway.completeStorageObservationAt(0, {
            ok: true,
            value: { health: { ready: true } },
            text: "OK",
            error: ""
        }))

        compare(gateway.requestCount, 0)
        verify(state.manifestRefreshDeferred)
        verify(!state.manifestObservationPending)

        gate.manifestsLoading = false
        gate.revision += 1

        compare(gateway.requestCount, 1)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(state.manifestRows()[0].cid, "z-bootstrap")
        compare(state.lastOperation, "List")
    }

    function test_busy_initial_manifest_refresh_retries_when_idle() {
        state.busy = true
        gate.manifestsLoading = true
        gateway.deferStorageObservations = true
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-after-busy-1",
                    domain: "storage",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: [{ cid: "z-after-busy", filename: "after-busy.bin", datasetSize: 19 }],
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(false)

        verify(state.manifestBusyDeferred)
        compare(state.lastOperation, "Waiting")
        compare(gateway.storageObservationCount, 0)
        compare(gateway.requestCount, 0)

        state.busy = false

        verify(!state.manifestBusyDeferred)
        compare(gateway.storageObservationCount, 1)
        compare(state.lastOperation, "Loading")

        gate.manifestsLoading = false
        verify(gateway.completeStorageObservationAt(0, {
            ok: true,
            value: { health: { ready: true } },
            text: "OK",
            error: ""
        }))

        compare(gateway.requestCount, 1)
        compare(state.manifestRows()[0].cid, "z-after-busy")
        compare(state.lastOperation, "List")
    }

    function test_manifest_bootstrap_replaces_observation_after_source_change() {
        gate.manifestsLoading = true
        gateway.deferStorageObservations = true
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-new-source-1",
                    domain: "storage",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: [{ cid: "z-new-source", filename: "new-source.bin", datasetSize: 23 }],
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(false)

        compare(gateway.storageObservationCount, 1)
        verify(state.manifestObservationPending)

        state.restEndpoint = "http://storage-new"
        wait(0)

        compare(gateway.storageObservationCount, 2)
        verify(state.manifestObservationPending)

        gate.manifestsLoading = false
        verify(gateway.completeStorageObservationAt(0, {
            ok: true,
            value: { health: { ready: true } },
            text: "OK",
            error: ""
        }))

        compare(gateway.requestCount, 0)
        verify(state.manifestRefreshDeferred)
        verify(state.manifestObservationPending)

        verify(gateway.completeStorageObservationAt(0, {
            ok: true,
            value: { health: { ready: true } },
            text: "OK",
            error: ""
        }))

        compare(gateway.requestCount, 1)
        compare(state.manifestRows()[0].cid, "z-new-source")
        compare(state.lastOperation, "List")
    }

    function test_silent_manifest_refresh_rechecks_disabled_configured_source() {
        gate.blocked = ({ manifests: "storage.manifests.read" })
        gateway.deferStorageObservations = true
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-recheck-1",
                    domain: "storage",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: [{ cid: "z-rechecked", filename: "rechecked.bin", datasetSize: 17 }],
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(false)

        compare(gateway.storageObservationCount, 1)
        compare(gateway.requestCount, 0)
        compare(state.lastOperation, "Loading")

        gate.blocked = ({})
        verify(gateway.completeStorageObservationAt(0, {
            ok: true,
            value: { health: { ready: true } },
            text: "OK",
            error: ""
        }))

        compare(gateway.requestCount, 1)
        compare(state.manifestRows()[0].cid, "z-rechecked")
        compare(state.lastOperation, "List")
    }

    function test_operation_status_text_keeps_reconciled_terminal_state() {
        compare(state.operationStatusText({ status: "awaiting_external" }), "Waiting")
        compare(state.operationStatusText({ status: "completed" }), "Complete")
        compare(state.operationStatusText({ status: "dispatched" }), "Dispatched")
        verify(state.terminalRefreshesStorageObservations({
            method: "storageUploadUrl",
            status: "completed"
        }))
        verify(state.terminalRefreshesStorageObservations({
            method: "storageRemove",
            status: "completed"
        }))
        verify(!state.terminalRefreshesStorageObservations({
            method: "storageRemove",
            status: "dispatched"
        }))
        verify(StorageModuleEvents.rawEventInvalidatesStorageObservations("storageDownloadDone"))
        verify(StorageModuleEvents.rawEventInvalidatesStorageObservations("storageRemoveDone"))
        verify(!StorageModuleEvents.rawEventInvalidatesStorageObservations("storageUploadDone"))
    }

    function test_remove_event_defers_manifest_refresh_until_terminal() {
        state.effectiveSourceMode = "logoscore_cli"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false
        state.operationSession.acceptUpdate({
            operationId: "storage-remove-race-1",
            domain: "storage",
            method: "storageRemove",
            status: "running",
            label: "Remove CID",
            cid: "cid-remove-race"
        })

        compare(state.refreshManifests(false), null)
        verify(state.manifestBusyDeferred)
        compare(gateway.requestCount, 0)
        compare(state.operationSession.operationLog.length, 0)

        gateway.storageRefreshCallback = function () {
            return state.refreshManifests(false)
        }
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-after-remove-1",
                    domain: "storage",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: [],
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })
        verify(state.operationSession.acceptUpdate({
            operationId: "storage-remove-race-1",
            domain: "storage",
            method: "storageRemove",
            status: "completed",
            label: "Remove CID",
            cid: "cid-remove-race",
            result: {
                success: true,
                cid: "cid-remove-race",
                completion: "storageRemoveDone"
            }
        }))
        verify(state.appendTerminalStorageOperation(state.operation.active))

        compare(gateway.storageRefreshCount, 1)
        compare(gateway.lastStorageRefreshCid, "cid-remove-race")
        compare(gateway.requestCount, 1)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].method, "storageManifests")
        verify(!state.manifestBusyDeferred)
    }

    function test_manifest_refresh_projects_terminal_poll_result() {
        state.effectiveSourceMode = "module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-poll-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageManifests",
                    status: "awaiting_external",
                    label: "Storage manifests",
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(false)

        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(state.operation.active.operationId, "storage-manifests-poll-1")
        compare(state.operation.active.status, "awaiting_external")
        compare(state.manifests.length, 0)

        gateway.requestResponses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "storage-manifests-poll-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: [{ cid: "z-polled", filename: "polled.bin", datasetSize: 9 }],
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.pollStorageOperation(false)

        compare(gateway.lastMethod, "runtimeOperationStatus")
        compare(state.operation.active.status, "completed")
        compare(state.manifestRows()[0].cid, "z-polled")
        compare(state.lastOperation, "List")
        compare(gateway.history.length, 1)
        compare(gateway.history[0].detail, "1 manifests")
        compare(gateway.resultTitle, "")
    }

    function test_run_storage_exists_uses_async_request_and_log() {
        gateway.requestResponses = ({
            storageExists: {
                ok: true,
                value: { exists: true },
                text: "OK",
                error: ""
            }
        })

        state.runStorage("storageExists", ["z-cid"], "Storage exists")

        compare(gateway.requestCount, 1)
        compare(gateway.lastMethod, "storageExists")
        compare(gateway.lastArgs[0].adapter.source_mode, "rest")
        compare(gateway.lastArgs[0].payload.cid, "z-cid")
        verify(gateway.lastShowResult)
        compare(state.lastOperation, "Storage exists")
        compare(state.operation.rows[0].label, "Storage exists")
    }

    function test_storage_exists_ignores_reverse_order_response() {
        gateway.deferRequests = true

        state.runStorage("storageExists", ["z-old"], "Old exists")
        state.runStorage("storageExists", ["z-new"], "New exists")

        compare(gateway.pendingRequests.length, 2)
        verify(typeof gateway.requests[0].acceptResponse === "function")
        verify(typeof gateway.requests[1].acceptResponse === "function")
        verify(gateway.completeRequestAt(1, {
            ok: true,
            value: { exists: true },
            text: "OK",
            error: ""
        }))
        compare(state.lastOperation, "New exists")
        compare(state.operation.rows.length, 1)
        compare(state.operation.rows[0].label, "New exists")

        verify(gateway.completeRequestAt(0, {
            ok: false,
            value: null,
            text: "",
            error: "stale failure"
        }))
        compare(state.lastOperation, "New exists")
        compare(state.operation.rows.length, 1)
        compare(gateway.rejectedResponseCount, 1)
    }

    function test_storage_exists_rejects_response_after_source_change() {
        gateway.deferRequests = true

        state.runStorage("storageExists", ["z-old-source"], "Old source exists")

        compare(gateway.pendingRequests.length, 1)
        verify(typeof gateway.requests[0].acceptResponse === "function")
        state.currentView = ""
        state.restEndpoint = "http://storage-new"
        wait(0)

        verify(gateway.completeRequestAt(0, {
            ok: true,
            value: { exists: true },
            text: "true",
            error: ""
        }))

        compare(gateway.rejectedResponseCount, 1)
        compare(state.lastOperation, "None")
        compare(state.operation.rows[0].label, "No operations")
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

    function test_start_accepted_operation_preserves_authoritative_awaiting_state() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-upload-ack",
                    clientRequestId: "client-1",
                    bridgeCallbackId: 7,
                    moduleSessionId: "session-1",
                    moduleRequestId: "request-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageUploadUrl",
                    status: "awaiting_external",
                    label: "Upload file",
                    acknowledgement: {
                        dispatched: true,
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
        compare(state.operation.active.status, "awaiting_external")
        compare(state.operation.active.clientRequestId, "client-1")
        compare(state.operation.active.bridgeCallbackId, 7)
        compare(state.operation.active.moduleSessionId, "session-1")
        compare(state.operation.active.moduleRequestId, "request-1")
        verify(state.operation.running)
        compare(state.lastOperation, "Waiting")
        compare(state.currentTab, "operations")
        compare(gateway.history.length, 0)
    }

    function test_cli_upload_event_completion_is_terminal_once() {
        state.effectiveSourceMode = "logoscore_cli"
        state.sourceLabel = "LogosCore CLI"
        state.sourceTarget = "storage_module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-upload-cli-1",
                    domain: "storage",
                    backend: "logoscore_cli",
                    method: "storageUploadUrl",
                    status: "completed",
                    label: "Upload file",
                    path: "/tmp/file.bin",
                    result: {
                        success: true,
                        sessionId: "session-cli-1",
                        cid: "z-uploaded",
                        completion: "storageUploadDone"
                    },
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.startStorageOperation("storageUploadUrl", ["/tmp/file.bin", 65536], "Upload file")

        compare(gateway.requestCount, 1)
        compare(state.operation.active.status, "completed")
        compare(state.operation.active.result.cid, "z-uploaded")
        compare(state.lastOperation, "Complete")
        compare(state.currentTab, "operations")
        verify(!state.operation.running)
        compare(gateway.history.length, 1)
        compare(gateway.history[0].operation.operationId, "storage-upload-cli-1")
        compare(gateway.resultTitle, "Upload file")
        compare(gateway.resultOwner, "storage")
        compare(gateway.resultValue.cid, "z-uploaded")
        compare(gateway.resultValue.sessionId, "session-cli-1")
        compare(gateway.resultValue.completion, "storageUploadDone")
        verify(!gateway.resultIsError)
    }

    function test_module_event_projects_only_authoritative_backend_operation() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-upload-ack",
                    moduleSessionId: "session-1",
                    moduleRequestId: "request-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageUploadUrl",
                    status: "awaiting_external",
                    label: "Upload file",
                    acknowledgement: { dispatched: true },
                    error: ""
                },
                text: "OK",
                error: ""
            },
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "applied",
                    operation: {
                        operationId: "storage-upload-ack",
                        moduleSessionId: "session-1",
                        moduleRequestId: "request-1",
                        domain: "storage",
                        backend: "module",
                        method: "storageUploadUrl",
                        status: "completed",
                        label: "Upload file",
                        result: { cid: "z-done" },
                        cid: "z-done",
                        error: ""
                    }
                },
                text: "OK",
                error: ""
            }
        })

        state.startStorageOperation("storageUploadUrl", ["/tmp/file.bin", 65536], "Upload file")

        compare(state.operation.active.status, "awaiting_external")
        compare(state.operation.rows.length, 1)

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "session-1", requestId: "request-1", success: true, cid: "z-done" })
        ]))

        compare(state.operation.active.status, "completed")
        compare(state.operation.active.cid, "z-done")
        compare(gateway.history.length, 1)
        compare(state.operation.rows.length, 2)
        compare(gateway.lastMethod, "runtimeOperationModuleEvent")
        compare(gateway.lastArgs[0].moduleName, "storage_module")
        compare(gateway.lastArgs[0].eventName, "storageUploadDone")

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "session-1", requestId: "request-1", success: true, cid: "z-done" })
        ]) !== null)
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

    function test_canceled_operation_is_neutral_and_explicit() {
        state.operationSession.acceptUpdate({
            operationId: "storage-download-canceled-1",
            domain: "storage",
            method: "storageDownloadToUrl",
            status: "canceled",
            label: "Download file",
            cid: "z-canceled",
            path: "/tmp/canceled.bin",
            error: "storage operation cancellation confirmed"
        })

        verify(state.appendTerminalStorageOperation(state.operation.active))

        compare(state.lastOperation, "Stopped")
        compare(state.operation.rows[0].status, "canceled")
        compare(gateway.resultTitle, "Download file")
        compare(gateway.resultValue.status, "canceled")
        verify(gateway.resultText.indexOf("Canceled") >= 0)
        verify(!gateway.resultIsError)
    }

    function test_manifest_fetch_completes_with_exact_result() {
        const manifest = {
            cid: "z-manifest",
            treeCid: "z-tree",
            datasetSize: 42,
            blockSize: 65536,
            filename: "manifest.json",
            mimetype: "application/json"
        }
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifest-1",
                    domain: "storage",
                    method: "storageDownloadManifest",
                    status: "completed",
                    label: "Fetch manifest",
                    cid: "z-manifest",
                    result: manifest,
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.runStorage(
            "storageDownloadManifest", ["z-manifest"], "Fetch manifest")

        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].method, "storageDownloadManifest")
        compare(gateway.lastArgs[0].payload.cid, "z-manifest")
        compare(state.currentTab, "operations")
        compare(state.lastOperation, "Complete")
        compare(state.operation.active.status, "completed")
        compare(state.operation.rows.length, 2)
        compare(gateway.resultTitle, "Fetch manifest")
        compare(gateway.resultOwner, "storage")
        compare(gateway.resultValue.cid, "z-manifest")
        compare(gateway.resultValue.treeCid, "z-tree")
        verify(gateway.resultText.indexOf("manifest.json") >= 0)
        verify(!gateway.resultIsError)
    }

    function test_cache_dispatch_shows_acknowledgement_without_claiming_completion() {
        const acknowledgement = {
            adapter: "logoscore_cli",
            cid: "z-cache",
            dispatched: true,
            method: "fetch",
            module: "storage_module",
            value: null
        }
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-cache-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageFetch",
                    status: "dispatched",
                    terminalReason: "completion_unobservable",
                    label: "Cache CID",
                    cid: "z-cache",
                    result: null,
                    acknowledgement: acknowledgement,
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.confirmStorage("storageFetch", ["z-cache"], "Cache CID")
        state.runPendingStorage()

        compare(gateway.requestCount, 1)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].method, "storageFetch")
        compare(gateway.lastArgs[0].payload.cid, "z-cache")
        compare(state.lastOperation, "Dispatched")
        compare(state.operation.active.status, "dispatched")
        compare(gateway.resultTitle, "Cache CID")
        compare(gateway.resultOwner, "storage")
        compare(gateway.resultValue.cid, "z-cache")
        compare(gateway.resultValue.dispatched, true)
        compare(gateway.resultValue.method, "fetch")
        compare(gateway.resultValue.module, "storage_module")
        verify(gateway.resultValue.operationId === undefined)
        verify(gateway.resultText.indexOf("z-cache") >= 0)
        verify(gateway.resultText.indexOf("operationId") < 0)
        verify(gateway.resultText.indexOf("completion_unobservable") < 0)
        verify(!gateway.resultIsError)
    }

    function test_failed_manifest_fetch_keeps_terminal_status_and_owner() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifest-failed",
                    domain: "storage",
                    method: "storageDownloadManifest",
                    status: "failed",
                    label: "Fetch manifest",
                    cid: "z-missing",
                    error: "manifest lookup failed"
                },
                text: "OK",
                error: ""
            }
        })

        state.runStorage(
            "storageDownloadManifest", ["z-missing"], "Fetch manifest")

        compare(state.currentTab, "operations")
        compare(state.lastOperation, "Stopped")
        compare(state.operation.active.status, "failed")
        compare(gateway.resultTitle, "Fetch manifest")
        compare(gateway.resultText, "manifest lookup failed")
        compare(gateway.resultOwner, "storage")
        verify(gateway.resultIsError)
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
        compare(gateway.requestCount, 0)
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

    function test_gate_reports_input_required_before_async_request() {
        const response = state.runStorage("storageExists", [""], "Storage exists")

        verify(!response.ok)
        compare(gateway.requestCount, 0)
        compare(response.error.indexOf("CID") >= 0, true)
        compare(state.lastOperation, "Blocked")
    }

    function test_missing_gate_facade_fails_closed() {
        const response = stateWithoutGate.refreshManifests(true)

        verify(!response.ok)
        compare(gateway.requestCount, 0)
        compare(stateWithoutGate.lastOperation, "Blocked")
        verify(response.error.indexOf("storage") >= 0)
    }

    function test_module_source_can_refresh_manifests() {
        state.effectiveSourceMode = "module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "storage-manifests-module-1",
                    domain: "storage",
                    backend: "module",
                    method: "storageManifests",
                    status: "completed",
                    label: "Storage manifests",
                    result: [
                        { cid: "z-module", filename: "module.bin", datasetSize: 7 }
                    ],
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.refreshManifests(true)

        compare(gateway.callCount, 0)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].method, "storageManifests")
        compare(gateway.lastArgs[0].adapter.source_mode, "module")
        verify(gateway.lastArgs[0].adapter.inputs.rest_endpoint === undefined)
        compare(Object.keys(gateway.lastArgs[0].payload).length, 0)
        compare(state.manifestRows()[0].cid, "z-module")
    }

    function test_manifest_refresh_ignores_stale_start_after_adapter_change() {
        gateway.deferRequests = true

        state.refreshManifests(false)
        state.effectiveSourceMode = "module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false
        state.refreshManifests(false)

        compare(gateway.pendingRequests.length, 2)
        verify(gateway.completeRequestAt(1, {
            ok: true,
            value: {
                operationId: "storage-manifests-new",
                domain: "storage",
                backend: "module",
                method: "storageManifests",
                status: "completed",
                label: "Storage manifests",
                result: [{ cid: "z-new", filename: "new.bin", datasetSize: 8 }]
            },
            text: "OK",
            error: ""
        }))
        compare(state.manifestRows()[0].cid, "z-new")
        verify(gateway.completeRequestAt(0, {
            ok: true,
            value: {
                operationId: "storage-manifests-old",
                domain: "storage",
                method: "storageManifests",
                status: "completed",
                label: "Storage manifests",
                result: [{ cid: "z-old", filename: "old.bin", datasetSize: 4 }]
            },
            text: "OK",
            error: ""
        }))
        compare(state.manifestRows()[0].cid, "z-new")
    }

    function test_busy_manifest_refresh_preserves_in_flight_context() {
        gateway.deferRequests = true

        state.refreshManifests(false)
        const generation = state.manifestRefreshContext.generation

        const blocked = state.refreshManifests(true)

        verify(!blocked.ok)
        compare(gateway.pendingRequests.length, 1)
        compare(state.manifestRefreshContext.generation, generation)
        compare(state.manifestRefreshContext.showLog, false)
        compare(state.lastOperation, "Busy")

        verify(gateway.completeRequestAt(0, {
            ok: true,
            value: {
                operationId: "storage-manifests-original",
                domain: "storage",
                method: "storageManifests",
                status: "completed",
                label: "Storage manifests",
                result: [{ cid: "z-original", filename: "original.bin", datasetSize: 5 }]
            },
            text: "OK",
            error: ""
        }))
        compare(state.manifestRows()[0].cid, "z-original")
        compare(state.lastOperation, "List")
        verify(state.manifestRefreshContext === null)
    }

    function test_legacy_completed_dispatch_envelope_is_never_reopened() {
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
                    cancellable: false,
                    error: ""
                },
                text: "OK",
                error: ""
            }
        })

        state.pollStorageOperation(false)

        compare(state.operation.active.status, "completed")
        verify(state.operation.active.externalSessionId === undefined)
        compare(state.operation.active.cancellable, false)
        compare(gateway.history.length, 1)
    }

    function test_stale_nonterminal_backend_record_does_not_reopen_terminal_operation() {
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
                    status: "awaiting_external",
                    label: "Upload",
                    moduleSessionId: "1",
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

    function test_raw_module_event_requires_authoritative_backend_projection() {
        state.operationSession.acceptUpdate({
            operationId: "op-1",
            domain: "storage",
            backend: "module",
            method: "storageUploadUrl",
            status: "awaiting_external",
            label: "Upload",
            moduleSessionId: "1",
            bytesWritten: 0,
            contentLength: 16
        })
        gateway.requestResponses = ({
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "uncorrelated",
                    operation: null
                },
                text: "OK",
                error: ""
            }
        })

        verify(state.applyStorageModuleEvent("storageUploadProgress", [
            JSON.stringify({ sessionId: "2", bytes: 8 })
        ]) !== null)
        compare(state.operation.active.bytesWritten, 0)
        compare(state.operation.active.status, "awaiting_external")

        gateway.requestResponses = ({
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "applied",
                    operation: {
                        operationId: "op-1",
                        domain: "storage",
                        backend: "module",
                        method: "storageUploadUrl",
                        status: "awaiting_external",
                        label: "Upload",
                        moduleSessionId: "1",
                        bytesWritten: 8,
                        contentLength: 16,
                        progress: 0.5
                    }
                },
                text: "OK",
                error: ""
            }
        })

        verify(state.applyStorageModuleEvent("storageUploadProgress", [
            JSON.stringify({ sessionId: "1", bytes: 8, totalBytes: 16 })
        ]) !== null)

        compare(state.operation.active.status, "awaiting_external")
        compare(state.operation.active.bytesWritten, 8)

        gateway.requestResponses = ({
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "applied",
                    operation: {
                        operationId: "op-1",
                        domain: "storage",
                        backend: "module",
                        method: "storageUploadUrl",
                        status: "completed",
                        label: "Upload",
                        moduleSessionId: "1",
                        bytesWritten: 8,
                        contentLength: 16,
                        progress: 1,
                        cid: "z-done",
                        result: { cid: "z-done", bytes: 8 }
                    }
                },
                text: "OK",
                error: ""
            }
        })

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "1", success: true, cid: "z-done", bytes: 8 })
        ]) !== null)

        compare(state.operation.active.status, "completed")
        compare(state.operation.active.cid, "z-done")
        compare(state.lastOperation, "Complete")
        compare(gateway.history.length, 1)
        compare(state.operation.rows.length, 1)

        verify(state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "1", success: true, cid: "z-done", bytes: 8 })
        ]) !== null)
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
