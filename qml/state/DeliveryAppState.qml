import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers
import "modules/ModuleEventUtils.js" as ModuleEventUtils
import "source_operations/SourceOperationCommandCatalog.js" as SourceOperationCommandCatalog

QtObject {
    id: root

    required property var gateway

    property bool busy: false
    property string sourceMode: "rest"
    property string effectiveSourceMode: "rest"
    property string sourceLabel: ""
    property string sourceTarget: ""
    property string sourceTargetKind: "none"
    property bool usesRestEndpoint: false
    property bool supportsMutatingDiagnostics: false
    property string restEndpoint: ""
    property var adapterInitialization: ({
        source_mode: effectiveSourceMode,
        inputs: usesRestEndpoint ? ({ rest_endpoint: restEndpoint }) : ({})
    })
    property string moduleName: "delivery_module"
    property string networkPreset: "logos.test"
    property bool mutatingDiagnosticsEnabled: true
    property var managedNodes: null
    property bool managedNodeConfirmationPending: false
    property string lastOperationOwner: "delivery"
    property string managedNodeBaselineOperationFingerprint: ""
    property string currentTab: "messages"
    property string activeTopic: "/logos-inspector/1/chat/proto"

    property alias lastOperation: deliveryOperations.lastOperation
    readonly property var pendingOperation: deliveryOperations.confirmation
    readonly property var operation: deliveryOperations.view

    property SourceOperationSession operationSession: SourceOperationSession {
        id: deliveryOperations

        gateway: root.gateway
        domain: "delivery"
        adapterInitialization: root.adapterInitialization
        mutatingDiagnosticsEnabled: root.mutatingDiagnosticsEnabled
        defaultLabel: qsTr("Delivery operation")
        busyError: qsTr("A delivery operation is already running.")

        onTerminalOperation: function (operation) {
            root.completeTerminalDeliveryOperation(operation)
        }
    }

    property var deliveryModuleEvents: []
    property int deliveryModuleEventRevision: 0
    property string deliveryConnectionStatus: ""
    property string deliveryNodeStatus: ""

    onAdapterInitializationChanged: {
        invalidateSourceRequests()
    }

    onSourceModeChanged: {
        clearNodeConfirmation()
    }

    function invalidateSourceRequests() {
        deliveryOperations.clearActive()
    }

    function sourceBadges() {
        return [qsTr("Delivery"), sourceLabel, sourceTarget, networkPreset]
    }

    function deliveryModuleSource() {
        return sourceTargetKind === "module"
    }

    function deliveryRestSource() {
        return sourceTargetKind === "rest_endpoint" && supportsMutatingDiagnostics
    }

    function deliveryMessageSource() {
        return supportsMutatingDiagnostics
    }

    function deliveryDataSource() {
        return sourceTargetKind !== "none"
    }

    function managedNodeLifecycleSource() {
        return sourceMode === "logoscore_cli"
    }

    function managedNodeActionKey(action) {
        return String(action || "") === "create" ? "initialize" : String(action || "")
    }

    function managedNodeActionAvailable(action) {
        if (!managedNodeLifecycleSource() || !managedNodes
                || typeof managedNodes.actionAvailable !== "function") {
            return false
        }
        return managedNodes.actionAvailable("messaging", managedNodeActionKey(action))
    }

    function nodeActionAvailable(action) {
        if (!deliveryModuleSource()) {
            return false
        }
        if (managedNodeLifecycleSource()) {
            return managedNodeActionAvailable(action)
        }
        // The installed Delivery stop callback is not a terminal lifecycle
        // boundary. LogosCore CLI uses the verified managed unload workflow;
        // a host module cannot safely expose direct Stop here.
        return String(action || "") !== "stop"
    }

    function nodeActionEnabled(action) {
        return !busy && !deliveryOperations.view.busy
            && nodeActionAvailable(action)
            && (!managedNodeLifecycleSource()
                || !managedNodes || managedNodes.busy !== true)
    }

    function nodeActionLabel(action) {
        if (managedNodeLifecycleSource() && String(action || "") === "create") {
            return qsTr("Initialize")
        }
        switch (String(action || "")) {
        case "create":
            return qsTr("Create")
        case "start":
            return qsTr("Start")
        case "stop":
            return qsTr("Stop")
        default:
            return qsTr("Node action")
        }
    }

    function confirmNodeAction(action, config) {
        const key = String(action || "")
        if (!nodeActionEnabled(key)) {
            return false
        }
        clearNodeConfirmation()
        if (managedNodeLifecycleSource()) {
            if (!managedNodes || typeof managedNodes.beginNodeAction !== "function") {
                return false
            }
            managedNodes.beginNodeAction(managedNodeActionKey(key), "messaging")
            managedNodeConfirmationPending = true
            return true
        }
        const methods = {
            create: "deliveryCreateNode",
            start: "deliveryStart"
        }
        const labels = {
            create: qsTr("Create node"),
            start: qsTr("Start node")
        }
        const method = String(methods[key] || "")
        if (!method.length) {
            return false
        }
        confirmDelivery(method, key === "create" ? [String(config || "").trim()] : [], labels[key])
        return true
    }

    function nodeConfirmationTitle() {
        if (managedNodeConfirmationPending && managedNodes
                && typeof managedNodes.actionDraftTitle === "function") {
            return managedNodes.actionDraftTitle()
        }
        return pendingOperation.label
    }

    function nodeConfirmationMessage() {
        if (managedNodeConfirmationPending && managedNodes
                && typeof managedNodes.actionDraftMessage === "function") {
            return managedNodes.actionDraftMessage()
        }
        return qsTr("This will call the configured Delivery source and may change node relay state.")
    }

    function nodeConfirmationText() {
        if (managedNodeConfirmationPending && managedNodes
                && typeof managedNodes.actionLabel === "function") {
            return managedNodes.actionLabel(managedNodes.pendingAction)
        }
        return pendingOperation.label
    }

    function nodeConfirmationEnabled() {
        if (managedNodeConfirmationPending) {
            return pendingManagedNodeActionAvailable()
        }
        return pendingOperation.method.length > 0
    }

    function pendingManagedNodeActionAvailable() {
        if (!managedNodeConfirmationPending || !managedNodeLifecycleSource()
                || !managedNodes || managedNodes.busy === true
                || String(managedNodes.pendingNode || "") !== "messaging") {
            return false
        }
        const action = String(managedNodes.pendingAction || "")
        return action.length > 0
            && typeof managedNodes.actionAvailable === "function"
            && managedNodes.actionAvailable("messaging", action)
    }

    function runPendingNodeAction() {
        if (!managedNodeConfirmationPending) {
            return runPendingDelivery()
        }
        if (!pendingManagedNodeActionAvailable()) {
            const error = qsTr("Messaging lifecycle changed. Refresh status and choose an available action.")
            clearNodeConfirmation()
            lastOperationOwner = "managed"
            lastOperation = qsTr("Error")
            gateway.setResult(qsTr("Messaging lifecycle"), error, true, null)
            return { ok: false, error: error }
        }
        const rows = managedNodeOperationRows()
        managedNodeBaselineOperationFingerprint = rows.length > 0
            ? String(rows[0].fingerprint || "") : ""
        managedNodeConfirmationPending = false
        if (!managedNodes || typeof managedNodes.runPendingAction !== "function") {
            return null
        }
        lastOperationOwner = "managed"
        lastOperation = qsTr("Starting")
        currentTab = "operations"
        return managedNodes.runPendingAction()
    }

    function clearNodeConfirmation() {
        if (managedNodeConfirmationPending && managedNodes
                && typeof managedNodes.clearActionDraft === "function") {
            managedNodes.clearActionDraft()
        }
        managedNodeConfirmationPending = false
        deliveryOperations.clearConfirmation()
    }

    function refreshManagedNodeState() {
        if (managedNodeLifecycleSource() && managedNodes
                && typeof managedNodes.refresh === "function") {
            return managedNodes.refresh(false)
        }
        return null
    }

    function managedNodeRecord() {
        if (!managedNodes || typeof managedNodes.nodeByKind !== "function") {
            return null
        }
        return managedNodes.nodeByKind("messaging")
    }

    function managedNodeStatusText() {
        const error = managedNodes ? String(managedNodes.error || "") : ""
        if (error.length > 0) {
            return qsTr("Messaging lifecycle status failed: %1").arg(error)
        }
        const node = managedNodeRecord()
        if (!node) {
            return managedNodes && managedNodes.statusLoading === true
                ? qsTr("Loading Inspector-managed Messaging state.")
                : qsTr("Messaging lifecycle status is unavailable. Select Refresh to try again.")
        }
        const runState = String(node.run_state || "unknown").replace(/_/g, " ")
        const available = ["create", "start", "stop"].filter(function (action) {
            return managedNodeActionAvailable(action)
        }).map(function (action) {
            return nodeActionLabel(action)
        })
        if (available.length > 0) {
            return qsTr("Messaging is %1. Available: %2.").arg(runState).arg(available.join(", "))
        }
        return qsTr("Messaging is %1. Refresh Local Nodes for lifecycle status.").arg(runState)
    }

    function managedNodeStatusTone() {
        if (managedNodes && String(managedNodes.error || "").length > 0) {
            return "error"
        }
        return managedNodeRecord() ? "info" : "warning"
    }

    function managedNodeOperationFingerprint(row) {
        const value = row || {}
        return [
            String(value.action || ""),
            String(value.node || ""),
            String(value.status || ""),
            String(value.timestamp_millis || value.time || ""),
            String(value.detail || "")
        ].join("\u001f")
    }

    function managedNodeOperationRows() {
        const revision = managedNodes && Number(managedNodes.revision || 0)
        const rows = managedNodes && Array.isArray(managedNodes.operations)
            ? managedNodes.operations : []
        const result = []
        for (let i = rows.length - 1; i >= 0; --i) {
            const row = rows[i] || {}
            if (String(row.node || "") !== "messaging") {
                continue
            }
            const millis = Number(row.timestamp_millis || row.time || 0)
            const action = String(row.action || "")
            const label = managedNodes && typeof managedNodes.actionLabel === "function"
                ? managedNodes.actionLabel(action) : action
            result.push({
                time: millis > 0 ? Qt.formatTime(new Date(millis), "HH:mm:ss") : String(row.time || "-"),
                label: qsTr("%1 Messaging").arg(label),
                status: String(row.status || "-"),
                detail: String(row.detail || "-"),
                fingerprint: managedNodeOperationFingerprint(row)
            })
        }
        return result
    }

    function displayedLastOperation() {
        const rows = lastOperationOwner === "managed" ? managedNodeOperationRows() : []
        return rows.length > 0
                && String(rows[0].fingerprint || "") !== managedNodeBaselineOperationFingerprint
            ? qsTr("%1: %2").arg(rows[0].label).arg(rows[0].status)
            : lastOperation
    }

    function deliveryArgs(method, extra) {
        return deliveryOperations.requestArgs(method, extra)
    }

    function confirmDelivery(method, args, label) {
        clearNodeConfirmation()
        deliveryOperations.confirm(method, args, label)
    }

    function runPendingDelivery() {
        return deliveryOperations.runConfirmed(function (method, args, label) {
            return root.runDelivery(method, args, label)
        })
    }

    function runDelivery(method, args, label) {
        const command = SourceOperationCommandCatalog.deliveryCommand(method, args)
        return startDeliveryOperation(command.method, args, label)
    }

    function startDeliveryOperation(method, args, label) {
        const storeQuery = String(method || "") === "deliveryStoreQuery"
        lastOperationOwner = "delivery"
        lastOperation = qsTr("Starting")
        const started = deliveryOperations.start(method, args, label, function (response, operation) {
            if (response && response.ok) {
                if (!deliveryOperations.isTerminal(operation)) {
                    lastOperation = operationStatusText(operation || response.value)
                }
                if (!storeQuery) {
                    currentTab = "operations"
                }
            } else {
                lastOperation = qsTr("Error")
            }
            if ((!response || !response.ok) && storeQuery) {
                gateway.setResult(
                    String(label || qsTr("Delivery Store query")),
                    String(response && response.error || qsTr("Delivery Store query failed.")),
                    true,
                    null
                )
            }
        })
        if (started && started.ok === false) {
            lastOperation = String(started.error || "") === deliveryOperations.busyError ? qsTr("Busy") : qsTr("Error")
            return started
        }
        return null
    }

    function pollDeliveryOperation(showResult) {
        return deliveryOperations.poll(showResult === true)
    }

    function completeTerminalDeliveryOperation(operation) {
        const status = String(operation && operation.status || "")
        if (String(operation && operation.method || "") === "deliveryStoreQuery") {
            setDeliveryStoreQueryResult(operation)
        }
        lastOperation = status === "dispatched"
            ? qsTr("Dispatched")
            : (status === "completed" ? qsTr("Complete") : qsTr("Stopped"))
    }

    function setDeliveryStoreQueryResult(operation) {
        const label = String(operation && operation.label || qsTr("Delivery Store query"))
        if (String(operation && operation.status || "") === "completed") {
            const value = operation && operation.result !== undefined && operation.result !== null
                ? operation.result
                : operation
            gateway.setResult(label, BridgeHelpers.formatValue(value), false, value)
        } else {
            gateway.setResult(
                label,
                String(operation && operation.error || qsTr("Delivery Store query failed.")),
                true,
                null
            )
        }
    }

    function operationStatusText(operation) {
        switch (String(operation && operation.status || "")) {
        case "awaiting_external":
            return qsTr("Waiting")
        case "running":
        case "canceling":
            return qsTr("Started")
        case "completed":
            return qsTr("Complete")
        case "dispatched":
            return qsTr("Dispatched")
        default:
            return qsTr("Started")
        }
    }

    function applyModuleEvent(eventName, args, forwardRuntimeEvent) {
        const name = String(eventName || "")
        const event = args && args.__moduleEventEnvelope === true ? args : {
            moduleName: moduleName,
            eventName: name,
            args: Array.isArray(args) ? args : (args === undefined || args === null ? [] : [args])
        }
        if (forwardRuntimeEvent !== false) {
            deliveryOperations.ingestModuleEvent(event)
        }
        const record = deliveryEventRecord(name, args)
        appendModuleEvent(record)
        const effect = {
            changed: true,
            refreshMessagingConnection: false,
            deliveryMessage: null
        }
        if (name === "connectionStateChanged") {
            deliveryConnectionStatus = record.statusValue
            effect.refreshMessagingConnection = true
        } else if (name === "nodeStarted" || name === "nodeStopped") {
            deliveryNodeStatus = record.statusValue
            effect.refreshMessagingConnection = true
        } else if (name === "messageReceived") {
            effect.deliveryMessage = deliveryMessageEffect(record)
        }
        return effect
    }

    function deliveryEventRecord(eventName, args) {
        const values = ModuleEventUtils.eventValues(args)
        const object = ModuleEventUtils.eventObject(args)
        const timestamp = ModuleEventUtils.fieldText(object, ["timestamp", "time"]) || positionalEventTimestamp(eventName, values)
        const topic = ModuleEventUtils.fieldText(object, ["contentTopic", "content_topic", "topic"]) || positionalEventTopic(eventName, values)
        const payload = object.payload !== undefined ? object.payload : positionalEventPayload(eventName, values)
        const hash = ModuleEventUtils.fieldText(object, ["messageHash", "message_hash", "hash"]) || positionalEventHash(eventName, values)
        const requestId = ModuleEventUtils.fieldText(object, ["requestId", "request_id"]) || positionalEventRequestId(eventName, values)
        const lifecycleMessage = ModuleEventUtils.singleLineText(
            ModuleEventUtils.fieldText(object, ["message"]) || positionalLifecycleMessage(eventName, values),
            160
        )
        let error = ModuleEventUtils.fieldText(object, ["error"])
        if (!error.length && eventName === "messageError") {
            error = lifecycleMessage || ModuleEventUtils.scalarText(values[2])
        }
        error = ModuleEventUtils.singleLineText(error, 160)
        let statusValue = ""
        if (eventName === "connectionStateChanged") {
            statusValue = ModuleEventUtils.singleLineText(
                ModuleEventUtils.fieldText(object, ["connectionStatus", "connection_status", "status"]) || ModuleEventUtils.scalarText(values[0]),
                160
            )
        } else if (eventName === "nodeStarted" || eventName === "nodeStopped") {
            const ok = object.success !== undefined ? object.success : values[0]
            statusValue = (ok === true || String(ok).toLowerCase() === "true") ? qsTr("ok") : qsTr("error")
            if (lifecycleMessage.length > 0) {
                statusValue += ": " + lifecycleMessage
            }
        }
        return {
            eventName: eventName,
            time: ModuleEventUtils.eventTimeText(timestamp),
            contentTopic: topic,
            payload: payload,
            messageHash: hash,
            requestId: requestId,
            error: error,
            status: moduleEventTone(eventName, error, statusValue),
            statusValue: statusValue,
            detail: moduleEventDetail(eventName, requestId, hash, topic, error, payload, object, statusValue, lifecycleMessage)
        }
    }

    function positionalEventTimestamp(eventName, values) {
        if (eventName === "connectionStateChanged") {
            return ModuleEventUtils.scalarText(values[1])
        }
        if (eventName === "messageSent" || eventName === "messagePropagated") {
            return ModuleEventUtils.scalarText(values[2])
        }
        if (eventName === "nodeStarted" || eventName === "nodeStopped") {
            return ModuleEventUtils.scalarText(values[2])
        }
        if (eventName === "messageReceived" || eventName === "messageError") {
            return ModuleEventUtils.scalarText(values[3])
        }
        return ""
    }

    function positionalEventTopic(eventName, values) {
        return eventName === "messageReceived" ? ModuleEventUtils.scalarText(values[1]) : ""
    }

    function positionalEventPayload(eventName, values) {
        return eventName === "messageReceived" ? values[2] : undefined
    }

    function positionalEventHash(eventName, values) {
        if (eventName === "messageReceived") {
            return ModuleEventUtils.scalarText(values[0])
        }
        if (eventName === "messageSent" || eventName === "messagePropagated" || eventName === "messageError") {
            return ModuleEventUtils.scalarText(values[1])
        }
        return ""
    }

    function positionalEventRequestId(eventName, values) {
        if (eventName === "messageSent" || eventName === "messagePropagated" || eventName === "messageError") {
            return ModuleEventUtils.scalarText(values[0])
        }
        return ""
    }

    function positionalLifecycleMessage(eventName, values) {
        if (eventName === "nodeStarted" || eventName === "nodeStopped") {
            return ModuleEventUtils.scalarText(values[1])
        }
        return ""
    }

    function appendModuleEvent(record) {
        const rows = Array.isArray(deliveryModuleEvents) ? deliveryModuleEvents.slice(0) : []
        rows.unshift({
            time: record.time,
            label: String(record.eventName || ""),
            status: String(record.status || ""),
            detail: String(record.detail || "")
        })
        deliveryModuleEvents = rows.slice(0, 50)
        deliveryModuleEventRevision += 1
    }

    function moduleEventDetail(eventName, requestId, hash, topic, error, payload, object, statusValue, lifecycleMessage) {
        if (eventName === "messageError") {
            return boundedEventDetail([requestId, ModuleEventUtils.shortText(hash, 20), error])
        }
        if (eventName === "messageReceived") {
            return boundedEventDetail([ModuleEventUtils.shortText(topic, 44), ModuleEventUtils.shortText(hash, 20), ModuleEventUtils.payloadSummary(payload)])
        }
        if (eventName === "connectionStateChanged") {
            return boundedEventDetail([statusValue])
        }
        if (eventName === "messageSent" || eventName === "messagePropagated") {
            return boundedEventDetail([requestId, ModuleEventUtils.shortText(hash, 20)])
        }
        const structured = boundedEventDetail([
            ModuleEventUtils.fieldText(object, ["node"]),
            ModuleEventUtils.fieldText(object, ["networkId", "network_id"]),
            ModuleEventUtils.fieldText(object, ["source"]),
            ModuleEventUtils.fieldText(object, ["status"]),
            ModuleEventUtils.fieldText(object, ["message"])
        ])
        if (structured.length > 0) {
            return structured
        }
        if (eventName === "nodeStarted" || eventName === "nodeStopped") {
            return boundedEventDetail([lifecycleMessage])
        }
        return boundedEventDetail([requestId, ModuleEventUtils.shortText(hash, 20), ModuleEventUtils.shortText(topic, 44)])
    }

    function boundedEventDetail(parts) {
        const values = ModuleEventUtils.compactParts(parts)
        const normalized = []
        for (let i = 0; i < values.length; ++i) {
            normalized.push(ModuleEventUtils.singleLineText(values[i], 120))
        }
        return ModuleEventUtils.singleLineText(normalized.join(" / "), 180)
    }

    function deliveryMessageEffect(record) {
        const topic = String(record.contentTopic || "")
        return {
            topic: topic,
            payload: record.payload,
            messageHash: String(record.messageHash || "")
        }
    }

    function moduleEventTone(eventName, error, statusValue) {
        const failedLifecycle = (eventName === "nodeStarted" || eventName === "nodeStopped")
            && String(statusValue || "").indexOf(qsTr("error")) === 0
        if (eventName === "messageError" || eventName === "moduleUnavailable"
                || eventName === "nodeUnavailable" || failedLifecycle
                || String(error || "").length > 0) {
            return qsTr("error")
        }
        if (eventName === "messageReceived" || eventName === "messagePropagated") {
            return qsTr("event")
        }
        return qsTr("ok")
    }

    function moduleEventRows() {
        const revision = deliveryModuleEventRevision
        const rows = Array.isArray(deliveryModuleEvents) ? deliveryModuleEvents : []
        if (rows.length > 0) {
            return rows
        }
        return [{
            time: "-",
            label: qsTr("No module events"),
            status: "-",
            detail: "-"
        }]
    }

    function moduleEventSummary() {
        const revision = deliveryModuleEventRevision
        if (deliveryConnectionStatus.length > 0) {
            return deliveryConnectionStatus
        }
        if (deliveryNodeStatus.length > 0) {
            return deliveryNodeStatus
        }
        return qsTr("No events")
    }

    function messageControlsEnabled(topic) {
        return !busy && !deliveryOperations.view.busy && deliveryMessageSource()
            && validContentTopic(topic)
    }

    function validContentTopic(topic) {
        const value = String(topic || "").trim()
        return /^\/[^/]+\/[^/]+\/[^/]+\/[^/]+$/.test(value)
    }

    function topicShortText(topic) {
        const value = String(topic || "").trim()
        if (!value.length) {
            return qsTr("Required")
        }
        if (value.length <= 28) {
            return value
        }
        return value.slice(0, 12) + "..." + value.slice(value.length - 12)
    }

    function storePageSizeValue(value) {
        const parsed = Number(String(value || "").trim())
        if (!Number.isFinite(parsed)) {
            return 20
        }
        return Math.max(1, Math.min(100, Math.floor(parsed)))
    }

    function defaultNodeConfig() {
        return BridgeHelpers.formatValue({
            logLevel: "INFO",
            mode: "Core",
            preset: networkPreset || "logos.test"
        })
    }

    function timeText() {
        return Qt.formatTime(new Date(), "HH:mm:ss")
    }
}
