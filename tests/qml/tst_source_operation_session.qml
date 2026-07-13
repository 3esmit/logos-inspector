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

        function reset() {
            requestCount = 0
            lastMethod = ""
            lastArgs = []
            responses = ({})
            history = []
        }

        function request(method, args, label, showResult, callback) {
            requestCount += 1
            lastMethod = String(method || "")
            lastArgs = args || []
            const response = responses[lastMethod]
            if (callback) {
                callback(response)
            }
            return response
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
                    cancellable: false
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
        compare(gateway.lastArgs[0].mutating_enabled, session.mutatingDiagnosticsEnabled)
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
}
