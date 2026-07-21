import QtQml
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "SourceOperationSession"

    QtObject {
        id: gateway

        property int requestCount: 0
        property string lastMethod: ""
        property var lastArgs: []
        property var responses: ({})
        property var history: []
        property bool holdStatusCallbacks: false
        property var statusCallbacks: []
        property bool holdStartCallbacks: false
        property var startCallbacks: []
        property bool holdModuleEventCallbacks: false
        property var moduleEventCallbacks: []
        property bool holdCancelCallbacks: false
        property var cancelCallbacks: []

        function reset() {
            requestCount = 0
            lastMethod = ""
            lastArgs = []
            responses = ({})
            history = []
            holdStatusCallbacks = false
            statusCallbacks = []
            holdStartCallbacks = false
            startCallbacks = []
            holdModuleEventCallbacks = false
            moduleEventCallbacks = []
            holdCancelCallbacks = false
            cancelCallbacks = []
        }

        function request(method, args, label, showResult, callback) {
            requestCount += 1
            lastMethod = String(method || "")
            lastArgs = args || []
            const response = responses[lastMethod]
            if (lastMethod === "runtimeOperationStart" && holdStartCallbacks) {
                startCallbacks = startCallbacks.concat([callback])
                return response
            }
            if (lastMethod === "runtimeOperationStatus" && holdStatusCallbacks) {
                statusCallbacks = statusCallbacks.concat([callback])
                return response
            }
            if (lastMethod === "runtimeOperationModuleEvent" && holdModuleEventCallbacks) {
                moduleEventCallbacks = moduleEventCallbacks.concat([callback])
                return response
            }
            if (lastMethod === "runtimeOperationCancel" && holdCancelCallbacks) {
                cancelCallbacks = cancelCallbacks.concat([callback])
                return response
            }
            if (callback) {
                callback(response)
            }
            return response
        }

        function completeStatusResponse(response) {
            if (statusCallbacks.length === 0) {
                return false
            }
            const callback = statusCallbacks[0]
            statusCallbacks = statusCallbacks.slice(1)
            if (callback) {
                callback(response)
            }
            return true
        }

        function completeStartResponse(response) {
            if (startCallbacks.length === 0) {
                return false
            }
            const callback = startCallbacks[0]
            startCallbacks = startCallbacks.slice(1)
            if (callback) {
                callback(response)
            }
            return true
        }

        function completeModuleEventResponse(response) {
            if (moduleEventCallbacks.length === 0) {
                return false
            }
            const callback = moduleEventCallbacks[0]
            moduleEventCallbacks = moduleEventCallbacks.slice(1)
            if (callback) {
                callback(response)
            }
            return true
        }

        function completeCancelResponse(response) {
            if (cancelCallbacks.length === 0) {
                return false
            }
            const callback = cancelCallbacks[0]
            cancelCallbacks = cancelCallbacks.slice(1)
            if (callback) {
                callback(response)
            }
            return true
        }

        function appendOperationHistory(operation, detail) {
            history = history.concat([{
                operation: operation,
                detail: String(detail || "")
            }])
        }
    }

    SourceOperationSession {
        id: storageSession

        gateway: gateway
        domain: "storage"
        adapterInitialization: ({
            source_mode: "rest",
            inputs: ({ rest_endpoint: "http://storage" })
        })
        mutatingDiagnosticsEnabled: true
        defaultLabel: "Storage operation"
    }

    SourceOperationSession {
        id: deliverySession

        gateway: gateway
        domain: "delivery"
        adapterInitialization: ({
            source_mode: "module",
            inputs: ({ module_name: "delivery_module" })
        })
        defaultLabel: "Delivery operation"
    }

    function init() {
        gateway.reset()
        storageSession.reset()
        deliverySession.reset()
    }

    function assertAdapterContract(session, domain, method, args, expectedPayload) {
        const operationId = domain + "-operation-1"
        gateway.responses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: operationId,
                    domain: domain,
                    method: method,
                    status: "running",
                    label: "Run operation",
                    cancellable: true
                },
                text: "OK",
                error: ""
            },
            runtimeOperationCancel: {
                ok: true,
                value: {
                    operationId: operationId,
                    domain: domain,
                    method: method,
                    status: "canceling",
                    label: "Run operation",
                    cancellable: true
                },
                text: "OK",
                error: ""
            },
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: operationId,
                    domain: domain,
                    method: method,
                    status: "completed",
                    label: "Run operation",
                    result: { accepted: true }
                },
                text: "OK",
                error: ""
            }
        })

        session.confirm(method, args, "Run operation")
        compare(session.confirmation.method, method)
        session.runConfirmed(function (pendingMethod, pendingArgs, label) {
            return session.start(pendingMethod, pendingArgs, label)
        })

        compare(session.confirmation.method, "")
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].domain, domain)
        compare(gateway.lastArgs[0].method, method)
        compare(gateway.lastArgs[0].adapter.source_mode, session.adapterInitialization.source_mode)
        compare(JSON.stringify(gateway.lastArgs[0].payload), JSON.stringify(expectedPayload))
        compare(gateway.lastArgs[0].mutating_enabled, true)
        verify(session.view.running)
        verify(session.view.cancelable)

        const startCount = gateway.requestCount
        const blocked = session.start(method, args, "Duplicate")
        verify(!blocked.ok)
        compare(gateway.requestCount, startCount)

        session.cancel()
        compare(gateway.lastMethod, "runtimeOperationCancel")
        compare(gateway.lastArgs[0], operationId)
        compare(session.view.active.status, "canceling")
        verify(!session.view.cancelable)

        const cancelCount = gateway.requestCount
        verify(session.cancel() === null)
        compare(gateway.requestCount, cancelCount)

        session.poll(false)
        compare(gateway.lastMethod, "runtimeOperationStatus")
        verify(session.view.terminal)
        compare(session.view.active.status, "completed")
        compare(gateway.history.length, 1)

        session.poll(false)
        compare(gateway.history.length, 1)
        verify(session.view.rows.length >= 4)
    }

    function test_storage_adapter_satisfies_operation_session_seam() {
        assertAdapterContract(
            storageSession,
            "storage",
            "storageUploadUrl",
            ["/tmp/file.bin", 32768],
            { path: "/tmp/file.bin", block_size: 32768 }
        )
    }

    function test_delivery_adapter_satisfies_operation_session_seam() {
        assertAdapterContract(
            deliverySession,
            "delivery",
            "deliverySend",
            ["/logos/1/chat/proto", "hello"],
            { topic: "/logos/1/chat/proto", payload: "hello" }
        )
    }

    function test_terminal_start_response_records_one_operation_row() {
        gateway.responses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-subscribe-1",
                    domain: "delivery",
                    method: "deliverySubscribe",
                    status: "completed",
                    label: "Subscribe",
                    result: { subscribed: true }
                }
            }
        })

        deliverySession.start(
            "deliverySubscribe",
            ["/logos-inspector/1/chat/proto"],
            "Subscribe"
        )

        compare(deliverySession.operationLog.length, 1)
        compare(deliverySession.operationLog[0].label, "Subscribe")
        compare(deliverySession.operationLog[0].status, "ok")
        compare(gateway.history.length, 1)
    }

    function test_cancel_is_single_flight_before_async_response() {
        storageSession.acceptUpdate({
            operationId: "async-cancel-operation",
            domain: "storage",
            method: "storageDownloadToUrl",
            status: "running",
            cancellable: true
        })
        gateway.holdCancelCallbacks = true
        gateway.responses = ({
            runtimeOperationCancel: {
                ok: true,
                value: {
                    operationId: "async-cancel-operation",
                    domain: "storage",
                    method: "storageDownloadToUrl",
                    status: "canceling",
                    cancellable: true
                }
            }
        })

        storageSession.cancel()
        verify(!storageSession.view.cancelable)
        const requestCount = gateway.requestCount

        verify(storageSession.cancel() === null)
        compare(gateway.requestCount, requestCount)

        verify(gateway.completeCancelResponse(
            gateway.responses.runtimeOperationCancel))
        compare(storageSession.activeOperation.status, "canceling")
        verify(!storageSession.view.cancelable)
    }

    function test_awaiting_external_is_active_and_dispatched_is_successful_terminal() {
        storageSession.acceptUpdate({
            operationId: "storage-operation-1",
            domain: "storage",
            method: "storageUploadUrl",
            status: "awaiting_external",
            moduleSessionId: "session-1"
        })

        verify(storageSession.view.running)
        verify(storageSession.view.busy)
        verify(!storageSession.view.terminal)

        verify(storageSession.acceptUpdate({
            operationId: "storage-operation-2",
            domain: "storage",
            method: "storageRemove",
            status: "dispatched",
            acknowledgement: { dispatched: true }
        }))
        verify(storageSession.acceptTerminal(storageSession.activeOperation))

        verify(storageSession.view.terminal)
        verify(!storageSession.view.running)
        compare(gateway.history.length, 1)
        compare(storageSession.operationLog[0].status, "ok")
    }

    function test_poll_transport_error_does_not_fabricate_terminal_state() {
        storageSession.acceptUpdate({
            operationId: "storage-operation-1",
            domain: "storage",
            method: "storageUploadUrl",
            status: "awaiting_external"
        })
        gateway.responses = ({
            runtimeOperationStatus: {
                ok: false,
                value: null,
                text: "",
                error: "status transport unavailable"
            }
        })

        storageSession.poll(false)

        compare(storageSession.activeOperation.status, "awaiting_external")
        verify(!storageSession.view.terminal)
        compare(gateway.history.length, 0)
        compare(storageSession.operationLog[0].status, "error")
    }

    function test_poll_rejects_wrong_operation_identity_and_terminal_regression() {
        storageSession.acceptUpdate({
            operationId: "storage-operation-1",
            domain: "storage",
            method: "storageUploadUrl",
            status: "completed",
            result: { cid: "cid-complete" }
        })
        storageSession.acceptTerminal(storageSession.activeOperation)
        gateway.responses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "storage-operation-2",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external"
                }
            }
        })

        storageSession.poll(false)

        compare(storageSession.activeOperation.operationId, "storage-operation-1")
        compare(storageSession.activeOperation.status, "completed")
        compare(storageSession.activeOperation.result.cid, "cid-complete")
        compare(gateway.history.length, 1)
    }

    function test_only_one_status_poll_can_be_in_flight() {
        storageSession.acceptUpdate({
            operationId: "storage-operation-1",
            domain: "storage",
            method: "storageUploadUrl",
            status: "awaiting_external"
        })
        gateway.holdStatusCallbacks = true
        gateway.responses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "storage-operation-1",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external"
                }
            }
        })

        verify(storageSession.poll(false) !== null)
        compare(storageSession.poll(false), null)
        compare(gateway.requestCount, 1)
        verify(storageSession.view.pollPending)

        verify(gateway.completeStatusResponse(gateway.responses.runtimeOperationStatus))
        verify(!storageSession.view.pollPending)
    }

    function test_clear_active_invalidates_stale_start_response() {
        gateway.holdStartCallbacks = true
        gateway.responses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "stale-operation",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external"
                }
            }
        })

        storageSession.start("storageUploadUrl", ["/tmp/file.bin"], "Upload")
        verify(storageSession.view.startPending)
        storageSession.clearActive()
        verify(!storageSession.view.startPending)

        verify(gateway.completeStartResponse(gateway.responses.runtimeOperationStart))
        verify(storageSession.activeOperation === null)
        compare(storageSession.operationLog.length, 0)
    }

    function test_stale_poll_cannot_clear_new_operation_poll_guard() {
        storageSession.acceptUpdate({
            operationId: "old-operation",
            domain: "storage",
            method: "storageUploadUrl",
            status: "awaiting_external"
        })
        gateway.holdStatusCallbacks = true
        gateway.responses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "old-operation",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "completed"
                }
            }
        })
        storageSession.poll(false)
        storageSession.clearActive()
        storageSession.acceptUpdate({
            operationId: "new-operation",
            domain: "storage",
            method: "storageUploadUrl",
            status: "awaiting_external"
        })
        gateway.responses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "new-operation",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external"
                }
            }
        })
        storageSession.poll(false)
        verify(storageSession.view.pollPending)

        verify(gateway.completeStatusResponse({
            ok: true,
            value: {
                operationId: "old-operation",
                status: "completed"
            }
        }))
        verify(storageSession.view.pollPending)
        compare(storageSession.activeOperation.operationId, "new-operation")

        verify(gateway.completeStatusResponse(gateway.responses.runtimeOperationStatus))
        verify(!storageSession.view.pollPending)
        compare(storageSession.activeOperation.operationId, "new-operation")
    }

    function test_stale_poll_cannot_regress_newer_cancel_snapshot() {
        storageSession.acceptUpdate({
            operationId: "ordered-operation",
            domain: "storage",
            method: "storageUploadUrl",
            status: "awaiting_external",
            eventCursor: 1,
            cancellable: true
        })
        gateway.holdStatusCallbacks = true
        gateway.responses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "ordered-operation",
                    status: "awaiting_external",
                    eventCursor: 2
                }
            },
            runtimeOperationCancel: {
                ok: true,
                value: {
                    operationId: "ordered-operation",
                    status: "canceling",
                    eventCursor: 3
                }
            }
        })

        storageSession.poll(false)
        storageSession.cancel()
        compare(storageSession.activeOperation.status, "canceling")
        compare(storageSession.activeOperation.eventCursor, 3)

        verify(gateway.completeStatusResponse(gateway.responses.runtimeOperationStatus))
        compare(storageSession.activeOperation.status, "canceling")
        compare(storageSession.activeOperation.eventCursor, 3)
        verify(!storageSession.view.pollPending)
    }

    function test_module_terminal_event_before_start_response_remains_authoritative() {
        gateway.holdStartCallbacks = true
        let projectedStart = null
        gateway.responses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "early-terminal-operation",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external",
                    eventCursor: 1
                }
            },
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "applied",
                    operation: {
                        operationId: "early-terminal-operation",
                        domain: "storage",
                        method: "storageUploadUrl",
                        status: "completed",
                        eventCursor: 3,
                        result: { cid: "cid-complete" }
                    }
                }
            }
        })

        storageSession.start("storageUploadUrl", ["/tmp/file.bin"], "Upload", function (response, operation) {
            projectedStart = operation
        })
        verify(storageSession.view.startPending)
        storageSession.ingestModuleEvent({
            moduleName: "storage_module",
            eventName: "uploadCompleted",
            args: ["cid-complete"]
        })
        verify(storageSession.activeOperation === null)

        verify(gateway.completeStartResponse(gateway.responses.runtimeOperationStart))
        compare(storageSession.activeOperation.status, "completed")
        compare(storageSession.activeOperation.eventCursor, 3)
        compare(storageSession.activeOperation.result.cid, "cid-complete")
        compare(projectedStart.status, "completed")
        compare(projectedStart.eventCursor, 3)
        verify(storageSession.view.terminal)
        compare(gateway.history.length, 1)
    }

    function test_module_terminal_event_after_start_response_remains_authoritative() {
        gateway.holdStartCallbacks = true
        gateway.holdModuleEventCallbacks = true
        gateway.responses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "late-terminal-operation",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external",
                    eventCursor: 1
                }
            },
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "applied",
                    operation: {
                        operationId: "late-terminal-operation",
                        domain: "storage",
                        method: "storageUploadUrl",
                        status: "completed",
                        eventCursor: 3,
                        result: { cid: "cid-late" }
                    }
                }
            }
        })

        storageSession.start("storageUploadUrl", ["/tmp/file.bin"], "Upload")
        storageSession.ingestModuleEvent({
            moduleName: "storage_module",
            eventName: "storageUploadDone",
            args: ["cid-late"]
        })

        verify(gateway.completeStartResponse(gateway.responses.runtimeOperationStart))
        compare(storageSession.activeOperation.status, "awaiting_external")
        verify(gateway.completeModuleEventResponse(gateway.responses.runtimeOperationModuleEvent))
        compare(storageSession.activeOperation.status, "completed")
        compare(storageSession.activeOperation.result.cid, "cid-late")
        compare(gateway.history.length, 1)
    }

    function test_unrelated_event_cannot_poison_pending_start_reconciliation() {
        gateway.holdStartCallbacks = true
        gateway.responses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "wanted-operation",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "awaiting_external",
                    eventCursor: 1
                }
            },
            runtimeOperationModuleEvent: {
                ok: true,
                value: {
                    disposition: "stale",
                    operation: {
                        operationId: "unrelated-operation",
                        status: "completed",
                        eventCursor: 8
                    }
                }
            }
        })

        storageSession.start("storageUploadUrl", ["/tmp/file.bin"], "Upload")
        storageSession.ingestModuleEvent({
            moduleName: "storage_module",
            eventName: "storageUploadDone",
            args: ["unrelated"]
        })
        gateway.responses.runtimeOperationModuleEvent = {
            ok: true,
            value: {
                disposition: "applied",
                operation: {
                    operationId: "wanted-operation",
                    domain: "storage",
                    method: "storageUploadUrl",
                    status: "completed",
                    eventCursor: 3,
                    result: { cid: "cid-wanted" }
                }
            }
        }
        storageSession.ingestModuleEvent({
            moduleName: "storage_module",
            eventName: "storageUploadDone",
            args: ["wanted"]
        })

        verify(gateway.completeStartResponse(gateway.responses.runtimeOperationStart))
        compare(storageSession.activeOperation.operationId, "wanted-operation")
        compare(storageSession.activeOperation.status, "completed")
        compare(storageSession.activeOperation.result.cid, "cid-wanted")
        compare(gateway.history.length, 1)
    }
}
