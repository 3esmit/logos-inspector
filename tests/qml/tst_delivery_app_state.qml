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

    QtObject {
        id: deliveryGateFixture

        property var blockedActions: []

        function deliveryGate(action, options) {
            const key = String(action || "")
            const required = options && Array.isArray(options.required_inputs)
                ? options.required_inputs : []
            for (let index = 0; index < required.length; ++index) {
                const input = required[index] || {}
                if (String(input.value || "").trim().length === 0) {
                    return {
                        enabled: false,
                        status: "input_required",
                        missing: [{
                            dependency: String(input.key || "input"),
                            label: String(input.label || "Input"),
                            status: "input_required",
                            capability: String(input.key || "input"),
                            provenance: "input"
                        }],
                        warnings: [],
                        provenance: ["input"]
                    }
                }
            }
            const blocked = blockedActions.indexOf(key) >= 0
            if (!blocked) {
                return {
                    enabled: true,
                    status: "available",
                    missing: [],
                    warnings: [],
                    provenance: ["test"]
                }
            }
            return {
                enabled: false,
                status: "unavailable",
                missing: [{
                    dependency: "delivery.store.query",
                    label: "Delivery Store",
                    status: "unavailable",
                    capability: "delivery.store.query",
                    provenance: "test"
                }],
                warnings: [],
                provenance: ["test"]
            }
        }
    }

    QtObject {
        id: managedNodesFixture

        property bool busy: false
        property bool statusLoading: false
        property string error: ""
        property var report: null
        property var operations: []
        property int revision: 0
        property string pendingAction: ""
        property string pendingNode: ""
        property int runCount: 0
        property int refreshCount: 0

        function nodeByKind(kind) {
            const nodes = report && Array.isArray(report.nodes) ? report.nodes : []
            for (let i = 0; i < nodes.length; ++i) {
                if (String(nodes[i].key || "") === String(kind || "")) {
                    return nodes[i]
                }
            }
            return null
        }

        function actionAvailable(kind, action) {
            const node = nodeByKind(kind)
            const actions = node && Array.isArray(node.available_actions)
                ? node.available_actions : []
            return actions.indexOf(String(action || "")) >= 0
        }

        function beginNodeAction(action, node) {
            pendingAction = String(action || "")
            pendingNode = String(node || "")
        }

        function clearActionDraft() {
            pendingAction = ""
            pendingNode = ""
        }

        function actionLabel(action) {
            switch (String(action || "")) {
            case "initialize": return "Initialize"
            case "start": return "Start"
            case "stop": return "Stop"
            default: return "Action"
            }
        }

        function actionDraftTitle() {
            return actionLabel(pendingAction) + " Messaging"
        }

        function actionDraftMessage() {
            if (pendingAction === "stop") {
                return "Unload Delivery and require Initialize before Start."
            }
            return actionLabel(pendingAction) + " the managed Messaging context."
        }

        function runPendingAction() {
            runCount += 1
            clearActionDraft()
            return { started: true }
        }

        function refresh(showResult) {
            refreshCount += 1
            return { requested: true, showResult: showResult === true }
        }
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
        managedNodes: managedNodesFixture
        gateFacade: deliveryGateFixture
    }

    function init() {
        gateway.reset()
        managedNodesFixture.busy = false
        managedNodesFixture.statusLoading = false
        managedNodesFixture.error = ""
        managedNodesFixture.report = null
        managedNodesFixture.operations = []
        managedNodesFixture.revision = 0
        managedNodesFixture.pendingAction = ""
        managedNodesFixture.pendingNode = ""
        managedNodesFixture.runCount = 0
        managedNodesFixture.refreshCount = 0
        deliveryGateFixture.blockedActions = []
        state.sourceMode = "rest"
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
        state.lastOperationOwner = "delivery"
        state.managedNodeBaselineOperationFingerprint = ""
        state.adapterInitialization = Qt.binding(function() {
            return ({
                source_mode: state.effectiveSourceMode,
                inputs: state.usesRestEndpoint
                    ? ({ rest_endpoint: state.restEndpoint }) : ({})
            })
        })
    }

    function setManagedMessaging(actions, runState) {
        managedNodesFixture.report = {
            nodes: [{
                key: "messaging",
                run_state: runState,
                available_actions: actions
            }]
        }
        managedNodesFixture.revision += 1
    }

    function useManagedDeliverySource() {
        state.sourceMode = "logoscore_cli"
        state.effectiveSourceMode = "module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false
    }

    function test_managed_running_node_exposes_only_verified_stop_workflow() {
        useManagedDeliverySource()
        setManagedMessaging(["stop"], "running")

        verify(!state.nodeActionAvailable("create"))
        verify(!state.nodeActionAvailable("start"))
        verify(state.nodeActionAvailable("stop"))
        verify(state.nodeActionEnabled("stop"))
        compare(state.nodeActionLabel("create"), "Initialize")

        verify(state.confirmNodeAction("stop", ""))
        verify(state.managedNodeConfirmationPending)
        compare(managedNodesFixture.pendingAction, "stop")
        compare(managedNodesFixture.pendingNode, "messaging")
        compare(state.nodeConfirmationTitle(), "Stop Messaging")
        compare(
            state.nodeConfirmationMessage(),
            "Unload Delivery and require Initialize before Start.")
        compare(state.nodeConfirmationText(), "Stop")
        verify(state.nodeConfirmationEnabled())

        const started = state.runPendingNodeAction()
        verify(started.started)
        compare(managedNodesFixture.runCount, 1)
        compare(gateway.requestCount, 0)
        compare(state.currentTab, "operations")
        verify(!state.managedNodeConfirmationPending)
    }

    function test_managed_stopped_node_maps_create_to_initialize() {
        useManagedDeliverySource()
        setManagedMessaging(["initialize"], "stopped")

        verify(state.nodeActionAvailable("create"))
        verify(!state.nodeActionAvailable("start"))
        verify(!state.nodeActionAvailable("stop"))
        verify(state.confirmNodeAction("create", "ignored raw config"))
        compare(managedNodesFixture.pendingAction, "initialize")
        compare(state.nodeConfirmationTitle(), "Initialize Messaging")
        compare(state.nodeConfirmationText(), "Initialize")
    }

    function test_managed_initialized_node_exposes_only_start() {
        useManagedDeliverySource()
        setManagedMessaging(["start"], "initialized")

        verify(!state.nodeActionAvailable("create"))
        verify(state.nodeActionAvailable("start"))
        verify(!state.nodeActionAvailable("stop"))
        verify(state.nodeActionEnabled("start"))
    }

    function test_host_module_never_exposes_nonterminal_native_stop() {
        state.sourceMode = "module"
        state.effectiveSourceMode = "module"
        state.sourceTargetKind = "module"
        state.usesRestEndpoint = false

        verify(state.nodeActionAvailable("create"))
        verify(state.nodeActionAvailable("start"))
        verify(!state.nodeActionAvailable("stop"))
        verify(!state.confirmNodeAction("stop", ""))
        compare(state.pendingOperation.method, "")

        verify(state.confirmNodeAction("start", ""))
        compare(state.pendingOperation.method, "deliveryStart")
        compare(state.nodeConfirmationTitle(), "Start node")
        state.clearNodeConfirmation()
    }

    function test_managed_operation_rows_filter_and_project_messaging_lifecycle() {
        useManagedDeliverySource()
        managedNodesFixture.operations = [{
            node: "storage",
            action: "stop",
            status: "stopped",
            detail: "storage stopped",
            timestamp_millis: 1000
        }, {
            node: "messaging",
            action: "stop",
            status: "stopped",
            detail: "Delivery unloaded",
            timestamp_millis: 2000
        }]
        managedNodesFixture.revision += 1

        const rows = state.managedNodeOperationRows()
        compare(rows.length, 1)
        compare(rows[0].label, "Stop Messaging")
        compare(rows[0].status, "stopped")
        compare(rows[0].detail, "Delivery unloaded")
        state.lastOperationOwner = "managed"
        compare(state.displayedLastOperation(), "Stop Messaging: stopped")
    }

    function test_canceled_managed_confirmation_cannot_replace_message_send() {
        useManagedDeliverySource()
        setManagedMessaging(["stop"], "running")
        verify(state.confirmNodeAction("stop", ""))

        state.clearNodeConfirmation()
        verify(!state.managedNodeConfirmationPending)
        compare(managedNodesFixture.pendingAction, "")

        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-send-after-cancel",
                    domain: "delivery",
                    method: "deliverySend",
                    status: "running",
                    label: "Send message",
                    cancellable: true
                },
                text: "OK",
                error: ""
            }
        })
        state.confirmDelivery("deliverySend", ["/logos/1/chat/proto", "hello"], "Send message")
        state.runPendingNodeAction()

        compare(managedNodesFixture.runCount, 0)
        compare(gateway.lastMethod, "runtimeOperationStart")
        compare(state.operation.active.method, "deliverySend")
    }

    function test_managed_confirmation_revalidates_lifecycle_before_accept() {
        useManagedDeliverySource()
        setManagedMessaging(["stop"], "running")
        verify(state.confirmNodeAction("stop", ""))

        setManagedMessaging(["start"], "initialized")

        verify(!state.nodeConfirmationEnabled())
        const response = state.runPendingNodeAction()
        verify(!response.ok)
        compare(managedNodesFixture.runCount, 0)
        verify(gateway.resultIsError)
        compare(gateway.resultTitle, "Messaging lifecycle")
    }

    function test_delivery_operation_supersedes_historical_managed_operation() {
        useManagedDeliverySource()
        managedNodesFixture.operations = [{
            node: "messaging",
            action: "start",
            status: "running",
            detail: "Delivery started",
            timestamp_millis: 1000
        }]
        managedNodesFixture.revision += 1
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-send-newer",
                    domain: "delivery",
                    method: "deliverySend",
                    status: "running",
                    label: "Send message",
                    cancellable: true
                },
                text: "OK",
                error: ""
            }
        })

        state.runDelivery("deliverySend", ["/logos/1/chat/proto", "hello"], "Send message")

        compare(state.lastOperationOwner, "delivery")
        compare(state.displayedLastOperation(), "Started")
    }

    function test_managed_action_ignores_older_history_until_new_outcome_arrives() {
        useManagedDeliverySource()
        setManagedMessaging(["stop"], "running")
        managedNodesFixture.operations = [{
            node: "messaging",
            action: "start",
            status: "running",
            detail: "old",
            timestamp_millis: 1000
        }]
        managedNodesFixture.revision += 1

        verify(state.confirmNodeAction("stop", ""))
        state.runPendingNodeAction()
        compare(state.displayedLastOperation(), "Starting")

        managedNodesFixture.operations = managedNodesFixture.operations.concat([{
            node: "messaging",
            action: "stop",
            status: "stopped",
            detail: "new",
            timestamp_millis: 2000
        }])
        managedNodesFixture.revision += 1
        compare(state.displayedLastOperation(), "Stop Messaging: stopped")
    }

    function test_managed_status_failure_is_actionable() {
        useManagedDeliverySource()
        managedNodesFixture.error = "logoscore unavailable"

        compare(
            state.managedNodeStatusText(),
            "Messaging lifecycle status failed: logoscore unavailable")
        compare(state.managedNodeStatusTone(), "error")
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

    function test_store_query_is_blocked_by_delivery_capability_gate() {
        deliveryGateFixture.blockedActions = ["store_query"]

        const response = state.runDelivery("deliveryStoreQuery", [
            "", "/logos/1/chat/proto", "", "", 20, true, true
        ], "Store query")

        verify(!response.ok)
        compare(gateway.requestCount, 0)
        compare(state.lastOperation, "Blocked")
        verify(gateway.resultIsError)
        verify(gateway.resultText.indexOf("delivery.store.query") >= 0)
    }

    function test_cli_store_query_requires_provider_or_uses_configured_provider() {
        useManagedDeliverySource()
        state.adapterInitialization = ({
            source_mode: "logoscore_cli",
            inputs: {}
        })

        const blocked = state.runDelivery("deliveryStoreQuery", [
            "", "/logos/1/chat/proto", "", "", 20, true, true
        ], "Store query")

        verify(!blocked.ok)
        compare(gateway.requestCount, 0)
        verify(gateway.resultText.indexOf("Store provider multiaddress") >= 0)

        state.adapterInitialization = ({
            source_mode: "logoscore_cli",
            inputs: {
                store_peer_addr: "/dns4/provider.example/tcp/30303/p2p/peer"
            }
        })
        gateway.requestResponses = ({
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "delivery-cli-store-1",
                    domain: "delivery",
                    method: "deliveryStoreQuery",
                    status: "completed",
                    label: "Store query",
                    result: { messages: [] },
                    cancellable: false
                },
                text: "OK",
                error: ""
            }
        })

        state.runDelivery("deliveryStoreQuery", [
            "", "/logos/1/chat/proto", "", "", 20, true, true
        ], "Store query")

        compare(gateway.requestCount, 1)
        compare(gateway.lastArgs[0].adapter.source_mode, "logoscore_cli")
        compare(gateway.lastArgs[0].adapter.inputs.store_peer_addr,
                "/dns4/provider.example/tcp/30303/p2p/peer")
        compare(gateway.lastArgs[0].payload.peer_addr, "")
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

    function test_watcher_module_event_renders_structured_detail() {
        const effect = state.applyModuleEvent("moduleReady", [{
            simulated: true,
            source: "poll",
            status: "loaded",
            timestamp: 1784363339000
        }], false)

        verify(effect.changed)
        const row = state.moduleEventRows()[0]
        compare(row.label, "moduleReady")
        compare(row.status, "ok")
        compare(row.detail, "poll / loaded")
        verify(row.detail.indexOf("[object Object]") < 0)

        state.applyModuleEvent("moduleUnavailable", [{
            node: "messaging-" + "n".repeat(80),
            network_id: "logos.test-" + "i".repeat(80),
            source: { internal: "hidden" },
            status: "unavailable\n" + "x".repeat(220),
            message: ["hidden"]
        }], false)
        const boundedRow = state.moduleEventRows()[0]
        verify(boundedRow.detail.length <= 180)
        verify(boundedRow.detail.indexOf("\n") < 0)
        verify(boundedRow.detail.indexOf("[object Object]") < 0)
        verify(boundedRow.detail.indexOf("hidden") < 0)
    }

    function test_native_event_signatures_map_fields_and_nanosecond_time() {
        const timestampMs = 1784363339000
        const timestampNs = String(timestampMs) + "000000"
        const expectedTime = Qt.formatTime(new Date(timestampMs), "HH:mm:ss")

        const timestampVariants = [
            String(timestampMs / 1000),
            String(timestampMs),
            String(timestampMs) + "000",
            timestampNs
        ]
        for (let i = 0; i < timestampVariants.length; ++i) {
            state.applyModuleEvent("connectionStateChanged", [
                "unit-" + String(i), timestampVariants[i]
            ], false)
            compare(state.moduleEventRows()[0].time, expectedTime)
        }

        state.applyModuleEvent("connectionStateChanged", ["connected", timestampNs], false)
        compare(state.moduleEventRows()[0].detail, "connected")
        compare(state.moduleEventRows()[0].time, expectedTime)
        compare(state.deliveryConnectionStatus, "connected")

        state.applyModuleEvent("messageSent", ["request-1", "hash-1", timestampNs], false)
        compare(state.moduleEventRows()[0].detail, "request-1 / hash-1")
        compare(state.moduleEventRows()[0].time, expectedTime)

        state.applyModuleEvent("messagePropagated", ["request-2", "hash-2", timestampNs], false)
        compare(state.moduleEventRows()[0].detail, "request-2 / hash-2")
        compare(state.moduleEventRows()[0].time, expectedTime)

        state.applyModuleEvent("messageError", ["request-3", "hash-3", "rejected", timestampNs], false)
        compare(state.moduleEventRows()[0].detail, "request-3 / hash-3 / rejected")
        compare(state.moduleEventRows()[0].status, "error")
        compare(state.moduleEventRows()[0].time, expectedTime)

        const effect = state.applyModuleEvent("messageReceived", [
            "hash-4", "/logos/1/chat/proto", "hello", timestampNs
        ], false)
        compare(state.moduleEventRows()[0].detail, "/logos/1/chat/proto / hash-4 / hello")
        compare(state.moduleEventRows()[0].time, expectedTime)
        compare(effect.deliveryMessage.messageHash, "hash-4")
        compare(effect.deliveryMessage.topic, "/logos/1/chat/proto")
        compare(effect.deliveryMessage.payload, "hello")

        state.applyModuleEvent("nodeStarted", [true, "started", timestampNs], false)
        compare(state.moduleEventRows()[0].detail, "started")
        compare(state.moduleEventRows()[0].status, "ok")
        compare(state.moduleEventRows()[0].time, expectedTime)

        state.applyModuleEvent("nodeStopped", [true, "stopped", timestampNs], false)
        compare(state.moduleEventRows()[0].detail, "stopped")
        compare(state.moduleEventRows()[0].status, "ok")
        compare(state.moduleEventRows()[0].time, expectedTime)

        state.applyModuleEvent("connectionStateChanged", [
            "connected\n" + "c".repeat(220), timestampNs
        ], false)
        verify(state.deliveryConnectionStatus.length <= 180)
        verify(state.deliveryConnectionStatus.indexOf("\n") < 0)
        verify(state.moduleEventRows()[0].detail.length <= 180)
        verify(state.moduleEventRows()[0].detail.indexOf("\n") < 0)

        state.applyModuleEvent("messageError", [
            "request\n" + "r".repeat(220),
            "hash-5",
            "failure\n" + "e".repeat(300),
            timestampNs
        ], false)
        verify(state.moduleEventRows()[0].detail.length <= 180)
        verify(state.moduleEventRows()[0].detail.indexOf("\n") < 0)
    }

    function test_successful_structured_node_event_keeps_success_tone() {
        const effect = state.applyModuleEvent("nodeStarted", [{
            node: "messaging",
            network_id: "logos.test",
            success: true,
            simulated: true,
            source: "poll",
            message: "Messaging health is READY",
            timestamp: 1784363339000
        }], false)

        verify(effect.refreshMessagingConnection)
        compare(state.deliveryNodeStatus, "ok: Messaging health is READY")
        const row = state.moduleEventRows()[0]
        compare(row.status, "ok")
        compare(row.detail, "messaging / logos.test / poll / Messaging health is READY")

        state.applyModuleEvent("nodeStopped", [{
            node: "messaging",
            success: false,
            source: "poll",
            message: "Messaging stop failed",
            timestamp: 1784363339000
        }], false)
        compare(state.deliveryNodeStatus, "error: Messaging stop failed")
        compare(state.moduleEventRows()[0].status, "error")

        state.applyModuleEvent("nodeStarted", [
            false,
            "Messaging unavailable\n" + "u".repeat(220),
            "1784363339000000000"
        ], false)
        verify(state.deliveryNodeStatus.length <= 180)
        verify(state.deliveryNodeStatus.indexOf("\n") < 0)
        verify(state.deliveryNodeStatus.indexOf("error: Messaging unavailable") === 0)
        compare(state.moduleEventRows()[0].status, "error")
        verify(state.moduleEventRows()[0].detail.indexOf("Messaging unavailable") === 0)
        verify(state.moduleEventRows()[0].detail.length <= 180)
        verify(state.moduleEventRows()[0].detail.indexOf("\n") < 0)
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
