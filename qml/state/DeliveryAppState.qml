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
    property string pendingMethod: ""
    property string pendingLabel: ""
    property var pendingArgs: []
    property var activeOperation: null
    property int activeOperationRevision: 0
    property string terminalOperationId: ""
    property var operationLog: []
    property int operationLogRevision: 0

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
        const args = [
            effectiveSourceMode,
            usesRestEndpoint ? restEndpoint : "",
            mutatingDiagnosticsEnabled === true
        ]
        return args.concat(extra || [])
    }

    function confirmDelivery(method, args, label) {
        pendingMethod = String(method || "")
        pendingArgs = args || []
        pendingLabel = String(label || "")
    }

    function runPendingDelivery() {
        if (!pendingMethod.length) {
            return null
        }
        const response = runDelivery(pendingMethod, pendingArgs, pendingLabel)
        pendingMethod = ""
        pendingArgs = []
        pendingLabel = ""
        return response
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
        if (activeDeliveryOperationRunning()) {
            const blocked = {
                ok: false,
                text: "",
                error: qsTr("A delivery operation is already running.")
            }
            appendOperation(label, blocked)
            lastOperation = qsTr("Busy")
            return blocked
        }
        const request = {
            domain: "delivery",
            sourceMode: effectiveSourceMode,
            endpoint: restEndpoint,
            module: moduleName,
            method: String(method || ""),
            args: deliveryArgs(args),
            mutatingEnabled: mutatingDiagnosticsEnabled === true,
            label: String(label || "")
        }
        lastOperation = qsTr("Starting")
        gateway.startNodeOperation(request, false, function (response) {
            appendOperation(label, response)
            lastOperation = response && response.ok ? qsTr("Started") : qsTr("Error")
            if (response && response.ok) {
                terminalOperationId = ""
                updateActiveOperation(response.value)
                currentTab = "operations"
            }
        })
        return null
    }

    function pollDeliveryOperation(showResult) {
        const operation = activeOperation || null
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        if (!operationId.length) {
            return null
        }
        return gateway.nodeOperationStatus(operationId, showResult === true, function (response) {
            if (!response || !response.ok) {
                const failedOperation = {
                    operationId: operationId,
                    domain: "delivery",
                    method: String(operation && operation.method ? operation.method : ""),
                    status: "failed",
                    label: String(operation && operation.label ? operation.label : qsTr("Delivery operation")),
                    error: String((response && response.error) || qsTr("Delivery operation status failed."))
                }
                updateActiveOperation(failedOperation)
                appendTerminalDeliveryOperation(failedOperation)
                return
            }
            updateActiveOperation(response.value)
            if (activeDeliveryOperationTerminal(response.value)) {
                appendTerminalDeliveryOperation(response.value)
            }
        })
    }

    function appendTerminalDeliveryOperation(operation) {
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        if (!operationId.length || terminalOperationId === operationId) {
            return
        }
        terminalOperationId = operationId
        const ok = String(operation.status || "") === "completed"
        appendOperation(String(operation.label || qsTr("Delivery operation")), {
            ok: ok,
            value: operation.result || operation,
            error: String(operation.error || "")
        })
        gateway.appendOperationHistory(operation, operationSummary(operation))
        lastOperation = ok ? qsTr("Complete") : qsTr("Stopped")
    }

    function updateActiveOperation(value) {
        activeOperation = value || null
        activeOperationRevision += 1
    }

    function activeDeliveryOperationRunning() {
        const revision = activeOperationRevision
        const operation = activeOperation || null
        const status = String(operation && operation.status ? operation.status : "")
        return status === "running" || status === "canceling"
    }

    function activeDeliveryOperationTerminal(operation) {
        const status = String(operation && operation.status ? operation.status : "")
        return status === "completed" || status === "failed" || status === "canceled"
    }

    function appendOperation(label, response) {
        const rows = Array.isArray(operationLog) ? operationLog.slice(0) : []
        rows.unshift({
            time: timeText(),
            label: String(label || ""),
            status: response && response.ok ? qsTr("ok") : qsTr("error"),
            detail: response && response.ok ? operationSummary(response.value) : String((response && response.error) || "")
        })
        operationLog = rows.slice(0, 20)
        operationLogRevision += 1
    }

    function operationRows() {
        const revision = operationLogRevision
        if (operationLog.length > 0) {
            return operationLog
        }
        return [{
            time: "-",
            label: qsTr("No operations"),
            status: "-",
            detail: "-"
        }]
    }

    function operationPayload(value) {
        if (value && value.value && value.value.result && value.value.result.value !== undefined) {
            return value.value.result.value
        }
        if (value && value.result && value.result.value !== undefined) {
            return value.result.value
        }
        if (value && value.result !== undefined && value.result !== null) {
            return value.result
        }
        if (value && value.value !== undefined) {
            return value.value
        }
        return value
    }

    function operationSummary(value) {
        const payload = operationPayload(value)
        if (payload === undefined || payload === null) {
            return qsTr("No value")
        }
        if (typeof payload === "string") {
            return payload
        }
        if (typeof payload === "boolean") {
            return payload ? qsTr("true") : qsTr("false")
        }
        return BridgeHelpers.formatValue(payload).replace(/\s+/g, " ").slice(0, 180)
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
