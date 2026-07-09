import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers
import "modules/ModuleEventUtils.js" as ModuleEventUtils

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
    property string moduleName: "delivery_module"
    property string networkPreset: "logos.test"
    property bool mutatingDiagnosticsEnabled: false
    property string currentTab: "messages"
    property string activeTopic: "/logos-inspector/1/chat/proto"

    property string lastOperation: qsTr("None")
    property alias pendingMethod: deliveryOperations.pendingMethod
    property alias pendingLabel: deliveryOperations.pendingLabel
    property alias pendingArgs: deliveryOperations.pendingArgs
    property alias activeOperation: deliveryOperations.activeOperation
    property alias activeOperationRevision: deliveryOperations.activeOperationRevision
    property alias terminalOperationId: deliveryOperations.terminalOperationId
    property alias operationLog: deliveryOperations.operationLog
    property alias operationLogRevision: deliveryOperations.operationLogRevision

    property SourceOperationFlow operationClient: SourceOperationFlow {
        id: deliveryOperations

        gateway: root.gateway
        domain: "delivery"
        moduleName: root.moduleName
        effectiveSourceMode: root.effectiveSourceMode
        restEndpoint: root.restEndpoint
        usesRestEndpoint: root.usesRestEndpoint
        mutatingDiagnosticsEnabled: root.mutatingDiagnosticsEnabled
        sourceArgsIncludeMutatingFlag: true
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

    function deliveryArgs(extra) {
        return deliveryOperations.sourceArgs(extra)
    }

    function confirmDelivery(method, args, label) {
        deliveryOperations.confirm(method, args, label, false)
    }

    function runPendingDelivery() {
        return deliveryOperations.runPending(function (method, args, label) {
            return root.runDelivery(method, args, label)
        })
    }

    function runDelivery(method, args, label) {
        if (String(method || "") !== "deliveryStoreQuery") {
            return startDeliveryOperation(method, args, label)
        }
        const response = gateway.call(method, deliveryArgs(args), label)
        appendOperation(label, response)
        lastOperation = response && response.ok ? String(label || "") : qsTr("Error")
        return response
    }

    function startDeliveryOperation(method, args, label) {
        lastOperation = qsTr("Starting")
        const started = deliveryOperations.startOperation(method, args, label, function (response) {
            lastOperation = response && response.ok ? qsTr("Started") : qsTr("Error")
            if (response && response.ok) {
                currentTab = "operations"
            }
        })
        if (started && started.ok === false) {
            lastOperation = String(started.error || "") === deliveryOperations.busyError ? qsTr("Busy") : qsTr("Error")
            return started
        }
        return null
    }

    function pollDeliveryOperation(showResult) {
        return deliveryOperations.pollOperation(showResult === true)
    }

    function appendTerminalDeliveryOperation(operation) {
        deliveryOperations.appendTerminalOperation(operation, operationSummary(operation))
    }

    function completeTerminalDeliveryOperation(operation) {
        const ok = String(operation.status || "") === "completed"
        lastOperation = ok ? qsTr("Complete") : qsTr("Stopped")
    }

    function updateActiveOperation(value) {
        deliveryOperations.updateActiveOperation(value)
    }

    function activeDeliveryOperationRunning() {
        return deliveryOperations.running()
    }

    function activeDeliveryOperationTerminal(operation) {
        return deliveryOperations.terminal(operation)
    }

    function appendOperation(label, response) {
        deliveryOperations.appendOperation(label, response)
    }

    function operationRows() {
        return deliveryOperations.rows()
    }

    function operationPayload(value) {
        return deliveryOperations.operationPayload(value)
    }

    function operationSummary(value) {
        return deliveryOperations.operationSummary(value)
    }

    function applyModuleEvent(eventName, args) {
        const name = String(eventName || "")
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
        const timestamp = ModuleEventUtils.fieldText(object, ["timestamp", "time"]) || String(values[3] || "")
        const topic = ModuleEventUtils.fieldText(object, ["contentTopic", "content_topic", "topic"]) || String(values[1] || "")
        const payload = object.payload !== undefined ? object.payload : values[2]
        const hash = ModuleEventUtils.fieldText(object, ["messageHash", "message_hash", "hash"]) || String(values[0] || "")
        const requestId = ModuleEventUtils.fieldText(object, ["requestId", "request_id"]) || String(values[0] || "")
        let error = ModuleEventUtils.fieldText(object, ["error", "message"])
        if (!error.length && eventName === "messageError") {
            error = String(values[2] || "")
        }
        let statusValue = ""
        if (eventName === "connectionStateChanged") {
            statusValue = ModuleEventUtils.fieldText(object, ["connectionStatus", "connection_status", "status"]) || String(values[0] || "")
        } else if (eventName === "nodeStarted" || eventName === "nodeStopped") {
            const ok = object.success !== undefined ? object.success : values[0]
            statusValue = (ok === true || String(ok).toLowerCase() === "true") ? qsTr("ok") : qsTr("error")
            const text = ModuleEventUtils.fieldText(object, ["message"]) || String(values[1] || "")
            if (text.length > 0) {
                statusValue += ": " + text
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
            status: moduleEventTone(eventName, error),
            statusValue: statusValue,
            detail: moduleEventDetail(eventName, requestId, hash, topic, error, payload)
        }
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

    function moduleEventDetail(eventName, requestId, hash, topic, error, payload) {
        if (eventName === "messageError") {
            return ModuleEventUtils.compactParts([requestId, ModuleEventUtils.shortText(hash, 20), error]).join(" / ")
        }
        if (eventName === "messageReceived") {
            return ModuleEventUtils.compactParts([ModuleEventUtils.shortText(topic, 44), ModuleEventUtils.shortText(hash, 20), ModuleEventUtils.payloadSummary(payload)]).join(" / ")
        }
        if (eventName === "connectionStateChanged") {
            return ModuleEventUtils.compactParts([requestId || hash]).join(" / ")
        }
        return ModuleEventUtils.compactParts([requestId, ModuleEventUtils.shortText(hash, 20), ModuleEventUtils.shortText(topic, 44)]).join(" / ")
    }

    function deliveryMessageEffect(record) {
        const topic = String(record.contentTopic || "")
        return {
            topic: topic,
            payload: record.payload,
            messageHash: String(record.messageHash || "")
        }
    }

    function moduleEventTone(eventName, error) {
        if (eventName === "messageError" || String(error || "").length > 0) {
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
        return !busy && !activeDeliveryOperationRunning() && deliveryMessageSource() && mutatingDiagnosticsEnabled && validContentTopic(topic)
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
