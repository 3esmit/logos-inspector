import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers

QtObject {
    id: root

    required property var gateway

    property bool busy: false
    property string sourceMode: "auto"
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
