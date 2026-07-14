import QtQuick
import QtTest
import "../../qml/state"
import "fixtures"

TestCase {
    id: testRoot

    name: "DeliveryAppState"

    StateGatewayFixture {
        id: gateway
    }

    DeliveryAppState {
        id: state

        gateway: gateway
        effectiveSourceMode: "rest"
        sourceLabel: "Direct REST"
        sourceTarget: "http://delivery"
        sourceTargetKind: "rest_endpoint"
        usesRestEndpoint: true
        supportsMutatingDiagnostics: true
        restEndpoint: "http://delivery"
        moduleName: "delivery_module"
        networkPreset: "logos.test"
        mutatingDiagnosticsEnabled: true
    }

    function init() {
        gateway.reset()
        state.effectiveSourceMode = "rest"
        state.sourceTargetKind = "rest_endpoint"
        state.usesRestEndpoint = true
        state.supportsMutatingDiagnostics = true
        state.restEndpoint = "http://delivery"
        state.currentTab = "messages"
        state.deliveryModuleEvents = []
        state.deliveryModuleEventRevision = 0
        state.deliveryConnectionStatus = ""
        state.deliveryNodeStatus = ""
        state.operationSession.reset()
    }

    function test_store_query_uses_runtime_operation_and_projects_result() {
        state.currentTab = "store"
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-store-1",
                    domain: "delivery",
                    method: "deliveryStoreQuery",
                    status: "completed",
                    label: "Store query",
                    result: {
                        endpoint: "http://delivery",
                        pageSize: 25,
                        value: { messages: [{ payload: "0x01" }] }
                    },
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.runDelivery("deliveryStoreQuery", [
            "peer-1", "/logos/1/chat/proto", "/waku/2/rs/16/32", "cursor-1", 25, true, true
        ], "Store query")

        compare(gateway.callCount, 0)
        compare(gateway.requestCount, 1)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(gateway.lastArgs[0].domain, "delivery")
        compare(gateway.lastArgs[0].method, "deliveryStoreQuery")
        compare(gateway.lastArgs[0].adapter.source_mode, "rest")
        compare(gateway.lastArgs[0].adapter.inputs.rest_endpoint, "http://delivery")
        compare(gateway.lastArgs[0].payload.peer_addr, "peer-1")
        compare(gateway.lastArgs[0].payload.content_topics, "/logos/1/chat/proto")
        compare(gateway.lastArgs[0].payload.page_size, 25)
        compare(gateway.lastArgs[0].payload.include_data, true)
        compare(state.currentTab, "store")
        compare(state.lastOperation, "Complete")
        compare(gateway.resultTitle, "Store query")
        verify(!gateway.resultIsError)
        compare(gateway.resultValue.pageSize, 25)
        compare(gateway.history.length, 1)
    }

    function test_store_query_projects_polled_terminal_result() {
        state.currentTab = "store"
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-store-poll-1",
                    domain: "delivery",
                    method: "deliveryStoreQuery",
                    status: "running",
                    label: "Store query",
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.runDelivery("deliveryStoreQuery", ["", "/logos/1/chat/proto", "", "", 20, true, true], "Store query")

        compare(state.operation.active.status, "running")
        compare(gateway.resultTitle, "")
        gateway.requestResponses = ({
            runtimeOperationStatus: {
                ok: true,
                value: {
                    operationId: "delivery-store-poll-1",
                    domain: "delivery",
                    method: "deliveryStoreQuery",
                    status: "completed",
                    label: "Store query",
                    result: { value: { messages: [{ payload: "0x02" }] } },
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.pollDeliveryOperation(false)

        compare(gateway.lastMethod, "runtimeOperationStatus")
        compare(state.operation.active.status, "completed")
        compare(gateway.resultValue.value.messages[0].payload, "0x02")
        compare(state.currentTab, "store")
        compare(state.lastOperation, "Complete")
    }

    function test_source_change_rejects_stale_store_query_start() {
        state.currentTab = "store"
        gateway.deferRequests = true

        state.runDelivery("deliveryStoreQuery", ["", "/logos/1/chat/proto", "", "", 20, true, true], "Store query")
        state.effectiveSourceMode = "module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false

        verify(gateway.completeRequestAt(0, {
            ok: true,
            value: {
                operationId: "delivery-store-stale",
                domain: "delivery",
                method: "deliveryStoreQuery",
                status: "completed",
                label: "Store query",
                result: { value: { messages: [{ payload: "stale" }] } }
            },
            text: "OK",
            error: ""
        }))

        verify(state.operation.active === null)
        compare(gateway.resultTitle, "")
        compare(state.currentTab, "store")
    }

    function test_store_query_terminal_failure_projects_error() {
        state.currentTab = "store"
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-store-failed",
                    domain: "delivery",
                    method: "deliveryStoreQuery",
                    status: "failed",
                    label: "Store query",
                    error: "store unavailable",
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.runDelivery("deliveryStoreQuery", ["", "/logos/1/chat/proto", "", "", 20, true, true], "Store query")

        compare(gateway.resultTitle, "Store query")
        compare(gateway.resultText, "store unavailable")
        verify(gateway.resultIsError)
        compare(state.lastOperation, "Stopped")
        compare(state.currentTab, "store")
    }

    function test_start_pending_blocks_duplicate_delivery_action() {
        gateway.deferRequests = true

        state.runDelivery("deliveryStoreQuery", ["", "/logos/1/chat/proto", "", "", 20, true, true], "Store query")

        verify(state.operation.startPending)
        verify(!state.messageControlsEnabled("/logos/1/chat/proto"))
        const blocked = state.runDelivery("deliveryStoreQuery", ["", "/logos/1/chat/proto", "", "", 20, true, true], "Store query")

        verify(!blocked.ok)
        compare(gateway.requestCount, 1)
        compare(gateway.pendingRequests.length, 1)
        compare(state.lastOperation, "Busy")
    }

    function test_mutating_delivery_operation_still_selects_operations_tab() {
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-send-1",
                    domain: "delivery",
                    method: "deliverySend",
                    status: "running",
                    label: "Send",
                    cancellable: true
                },
                text: "OK",
                error: ""
            }
        })

        state.runDelivery("deliverySend", ["/logos/1/chat/proto", "hello"], "Send")

        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(state.currentTab, "operations")
        compare(state.operation.active.method, "deliverySend")
    }

    function test_message_received_event_returns_delivery_message_effect() {
        const topic = "/lez/account/account-1/comments"
        const payload = {
            kind: "comment",
            version: 1,
            identity: { display_name: "Peer" },
            body: "hello",
            created_at: "2026-07-07T00:00:00Z",
            conversation_id: topic
        }

        const effect = state.applyModuleEvent("messageReceived", [
            "hash-1",
            topic,
            JSON.stringify(payload),
            "1000"
        ])

        verify(effect.changed)
        verify(effect.deliveryMessage)
        compare(effect.deliveryMessage.topic, topic)
        compare(effect.deliveryMessage.messageHash, "hash-1")
        compare(effect.deliveryMessage.payload, JSON.stringify(payload))
        compare(state.moduleEventRows()[0].label, "messageReceived")
        compare(state.moduleEventRows()[0].status, "event")
        compare(gateway.lastMethod, "runtimeOperationModuleEvent")
        compare(gateway.lastArgs[0].moduleName, "delivery_module")
        compare(gateway.lastArgs[0].eventName, "messageReceived")
        compare(gateway.lastArgs[0].args[0], "hash-1")
    }

    function test_connection_event_returns_refresh_effect() {
        const effect = state.applyModuleEvent("connectionStateChanged", [
            JSON.stringify({
                connectionStatus: "connected",
                requestId: "connection-1"
            })
        ])

        verify(effect.changed)
        verify(effect.refreshMessagingConnection)
        verify(!effect.deliveryMessage)
        compare(state.moduleEventSummary(), "connected")
        compare(state.moduleEventRows()[0].label, "connectionStateChanged")
        compare(gateway.lastMethod, "runtimeOperationModuleEvent")
    }

    function test_operation_status_text_keeps_reconciled_terminal_state() {
        compare(state.operationStatusText({ status: "awaiting_external" }), "Waiting")
        compare(state.operationStatusText({ status: "completed" }), "Complete")
        compare(state.operationStatusText({ status: "dispatched" }), "Dispatched")
    }
}
