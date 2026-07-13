import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers
import "OperationHistoryVocabulary.js" as OperationHistoryVocabulary
import "source_operations/NodeOperationRequest.js" as NodeOperationRequest

QtObject {
    id: root

    required property var gateway

    required property string domain
    property var adapterInitialization: ({ source_mode: "", inputs: ({}) })
    property bool mutatingDiagnosticsEnabled: false
    property string defaultLabel: qsTr("Runtime operation")
    property string busyError: qsTr("A runtime operation is already running.")
    property var terminalDetailProvider: null

    property string pendingMethod: ""
    property string pendingLabel: ""
    property var pendingArgs: []
    property bool startPending: false
    property var activeOperation: null
    property int activeOperationRevision: 0
    property string terminalOperationId: ""
    property var operationLog: []
    property int operationLogRevision: 0
    property string lastOperation: qsTr("None")

    readonly property var confirmation: ({
        method: pendingMethod,
        label: pendingLabel,
        args: pendingArgs
    })
    readonly property var view: operationView()

    signal started(var operation)
    signal startFailed(var response)
    signal terminalOperation(var operation)

    function requestArgs(method, args) {
        return [NodeOperationRequest.envelope(
            adapterInitialization,
            NodeOperationRequest.payload(domain, method, args),
            mutatingDiagnosticsEnabled
        )]
    }

    function confirm(method, args, label) {
        pendingMethod = String(method || "")
        pendingArgs = args || []
        pendingLabel = String(label || "")
    }

    function clearConfirmation() {
        pendingMethod = ""
        pendingArgs = []
        pendingLabel = ""
    }

    function runConfirmed(callback) {
        if (!pendingMethod.length || typeof callback !== "function") {
            return null
        }
        const method = pendingMethod
        const args = pendingArgs
        const label = pendingLabel
        const response = callback(method, args, label)
        clearConfirmation()
        return response
    }

    function start(method, args, label, onResponse) {
        const operationLabel = String(label || defaultLabel)
        if (view.busy) {
            const blocked = { ok: false, text: "", error: busyError }
            appendResult(operationLabel, blocked)
            return blocked
        }

        const request = NodeOperationRequest.envelope(
            adapterInitialization,
            NodeOperationRequest.payload(domain, method, args),
            mutatingDiagnosticsEnabled
        )
        request.domain = domain
        request.method = String(method || "")
        request.label = operationLabel
        startPending = true
        const callback = function (response) {
            startPending = false
            appendResult(operationLabel, response)
            if (response && response.ok) {
                terminalOperationId = ""
                acceptUpdate(response.value)
                started(response.value)
            } else {
                startFailed(response)
            }
            if (onResponse) {
                onResponse(response)
            }
        }

        if (gateway && typeof gateway.startRuntimeOperation === "function") {
            return gateway.startRuntimeOperation(request, false, callback)
        }
        if (gateway && typeof gateway.startNodeOperation === "function") {
            return gateway.startNodeOperation(request, false, callback)
        }
        if (gateway && typeof gateway.request === "function") {
            return gateway.request("runtimeOperationStart", [request], operationLabel, false, callback)
        }

        const failed = {
            ok: false,
            text: "",
            error: qsTr("Runtime operation bridge is unavailable.")
        }
        callback(failed)
        return failed
    }

    function poll(showResult, onResponse) {
        const operation = activeOperation || null
        const operationId = String(operation && operation.operationId || "")
        if (!operationId.length) {
            return null
        }
        const callback = function (response) {
            if (onResponse && onResponse(response, operation) === true) {
                return
            }
            if (!response || !response.ok) {
                const failedOperation = {
                    operationId: operationId,
                    domain: String(operation && operation.domain || root.domain),
                    method: String(operation && operation.method || ""),
                    status: "failed",
                    label: String(operation && operation.label || defaultLabel),
                    error: String(response && response.error || qsTr("Runtime operation status failed."))
                }
                acceptUpdate(failedOperation)
                acceptTerminal(failedOperation)
                return
            }
            acceptUpdate(response.value)
            if (isTerminal(response.value)) {
                acceptTerminal(response.value)
            }
        }

        if (gateway && typeof gateway.runtimeOperationStatus === "function") {
            return gateway.runtimeOperationStatus(operationId, showResult === true, callback)
        }
        if (gateway && typeof gateway.nodeOperationStatus === "function") {
            return gateway.nodeOperationStatus(operationId, showResult === true, callback)
        }
        if (gateway && typeof gateway.request === "function") {
            return gateway.request("runtimeOperationStatus", [operationId], qsTr("Runtime operation"), showResult === true, callback)
        }
        return null
    }

    function cancel(onResponse) {
        const operationId = String(activeOperation && activeOperation.operationId || "")
        if (!operationId.length) {
            return null
        }
        const callback = function (response) {
            if (response && response.ok) {
                acceptUpdate(response.value)
            }
            appendResult(qsTr("Cancel operation"), response)
            if (onResponse) {
                onResponse(response)
            }
        }

        if (gateway && typeof gateway.runtimeOperationCancel === "function") {
            return gateway.runtimeOperationCancel(operationId, false, callback)
        }
        if (gateway && typeof gateway.nodeOperationCancel === "function") {
            return gateway.nodeOperationCancel(operationId, false, callback)
        }
        if (gateway && typeof gateway.request === "function") {
            return gateway.request("runtimeOperationCancel", [operationId], qsTr("Cancel operation"), false, callback)
        }
        return null
    }

    function acceptUpdate(value) {
        activeOperation = value || null
        activeOperationRevision += 1
    }

    function clearActive() {
        activeOperation = null
        activeOperationRevision += 1
    }

    function reset() {
        clearConfirmation()
        startPending = false
        activeOperation = null
        activeOperationRevision += 1
        terminalOperationId = ""
        operationLog = []
        operationLogRevision += 1
        lastOperation = qsTr("None")
    }

    function appendResult(label, response) {
        const rows = Array.isArray(operationLog) ? operationLog.slice(0) : []
        rows.unshift({
            time: timeText(),
            label: String(label || ""),
            status: response && response.ok ? qsTr("ok") : qsTr("error"),
            detail: response && response.ok ? summary(response.value) : String(response && response.error || "")
        })
        operationLog = rows.slice(0, 20)
        operationLogRevision += 1
    }

    function acceptTerminal(operation, detail) {
        const operationId = String(operation && operation.operationId || "")
        if (!operationId.length || terminalOperationId === operationId) {
            return false
        }
        terminalOperationId = operationId
        const ok = String(operation.status || "") === "completed"
        appendResult(String(operation.label || defaultLabel), {
            ok: ok,
            value: operation.result || operation,
            error: String(operation.error || "")
        })
        if (gateway && typeof gateway.appendOperationHistory === "function") {
            gateway.appendOperationHistory(operation, terminalDetail(operation, detail))
        }
        terminalOperation(operation)
        return true
    }

    function operationView() {
        const activeRevision = activeOperationRevision
        const logRevision = operationLogRevision
        const operation = activeOperation || null
        const status = String(operation && operation.status || "")
        const running = status === "running" || status === "canceling"
        return {
            activeRevision: activeRevision,
            logRevision: logRevision,
            active: operation,
            startPending: startPending,
            known: operation !== null && String(operation.operationId || "").length > 0,
            running: running,
            busy: startPending || running,
            cancelable: running && operation && operation.cancellable === true,
            terminal: isTerminal(operation),
            statusText: OperationHistoryVocabulary.runtimeStatusText(operation, defaultLabel),
            tone: OperationHistoryVocabulary.runtimeTone(operation),
            rows: operationRows()
        }
    }

    function isTerminal(operation) {
        return OperationHistoryVocabulary.isRuntimeTerminalStatus(operation && operation.status)
    }

    function terminalDetail(operation, detail) {
        if (detail !== undefined) {
            return String(detail || "")
        }
        if (terminalDetailProvider && typeof terminalDetailProvider === "function") {
            return String(terminalDetailProvider(operation) || "")
        }
        return summary(operation)
    }

    function operationRows() {
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

    function payload(value) {
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

    function summary(value) {
        const result = payload(value)
        if (result === undefined || result === null) {
            return qsTr("No value")
        }
        if (typeof result === "string") {
            return result
        }
        if (typeof result === "boolean") {
            return result ? qsTr("true") : qsTr("false")
        }
        return BridgeHelpers.formatValue(result).replace(/\s+/g, " ").slice(0, 180)
    }

    function timeText() {
        return Qt.formatTime(new Date(), "HH:mm:ss")
    }
}
