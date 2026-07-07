import QtQuick
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "StorageAppState"

    QtObject {
        id: gateway

        property int callCount: 0
        property int requestCount: 0
        property string lastMethod: ""
        property var lastArgs: []
        property string lastLabel: ""
        property bool lastShowResult: false
        property var calls: []
        property var requests: []
        property var callResponses: ({})
        property var requestResponses: ({})
        property string resultTitle: ""
        property string resultText: ""
        property bool resultIsError: false
        property var resultValue: null
        property var history: []
        property string openedSection: ""
        property string openedSubSection: ""

        function call(method, args, label) {
            callCount += 1
            lastMethod = String(method || "")
            lastArgs = args || []
            lastLabel = String(label || "")
            calls = calls.concat([{ method: lastMethod, args: lastArgs, label: lastLabel }])
            return callResponses[lastMethod] !== undefined ? callResponses[lastMethod] : {
                ok: true,
                value: {},
                text: "OK",
                error: ""
            }
        }

        function request(method, args, label, showResult, callback) {
            requestCount += 1
            lastMethod = String(method || "")
            lastArgs = args || []
            lastLabel = String(label || "")
            lastShowResult = showResult === true
            requests = requests.concat([{ method: lastMethod, args: lastArgs, label: lastLabel, showResult: lastShowResult }])
            const response = requestResponses[lastMethod] !== undefined ? requestResponses[lastMethod] : {
                ok: true,
                value: {},
                text: "OK",
                error: ""
            }
            callback(response)
            return response
        }

        function setResult(title, text, isError, value) {
            resultTitle = String(title || "")
            resultText = String(text || "")
            resultIsError = isError === true
            resultValue = value === undefined ? null : value
        }

        function clearResult() {
            resultTitle = ""
            resultText = ""
            resultIsError = false
            resultValue = null
        }

        function appendOperationHistory(operation, detail) {
            history = history.concat([{ operation: operation, detail: String(detail || "") }])
        }

        function openSettings(section, subSection) {
            openedSection = String(section || "")
            openedSubSection = String(subSection || "")
        }

        function valueText(value) {
            return String(value)
        }
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
        gateway.callCount = 0
        gateway.requestCount = 0
        gateway.lastMethod = ""
        gateway.lastArgs = []
        gateway.lastLabel = ""
        gateway.lastShowResult = false
        gateway.calls = []
        gateway.requests = []
        gateway.callResponses = ({})
        gateway.requestResponses = ({})
        gateway.resultTitle = ""
        gateway.resultText = ""
        gateway.resultIsError = false
        gateway.resultValue = null
        gateway.history = []
        gateway.openedSection = ""
        gateway.openedSubSection = ""

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
            nodeOperationStart: {
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
        compare(gateway.lastMethod, "nodeOperationStart")
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
            nodeOperationStatus: {
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

    function test_storage_module_events_update_active_operation() {
        state.applyStorageModuleEvent("storageUploadProgress", [
            JSON.stringify({ sessionId: "1", bytes: 8 })
        ])

        compare(state.activeOperation.status, "running")
        compare(state.activeOperation.externalSessionId, "1")
        compare(state.activeOperation.bytesWritten, 8)

        state.applyStorageModuleEvent("storageUploadDone", [
            JSON.stringify({ sessionId: "1", success: true, cid: "z-done", bytes: 8 })
        ])

        compare(state.activeOperation.status, "completed")
        compare(state.activeOperation.cid, "z-done")
        compare(state.lastOperation, "Complete")
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
