import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers
import "OperationHistoryVocabulary.js" as OperationHistoryVocabulary

QtObject {
    id: root

    required property var gateway

    property string domain: ""
    property string defaultLabel: qsTr("Runtime operation")
    property string busyError: qsTr("A runtime operation is already running.")
    property bool startPending: false
    property var activeOperation: null
    property int activeOperationRevision: 0
    property string terminalOperationId: ""
    property var operationLog: []
    property int operationLogRevision: 0
    property var terminalDetailProvider: null

    signal started(var operation)
    signal startFailed(var response)
    signal terminalOperation(var operation)

    function start(request, label, onResponse) {
        const operationLabel = String(label || (request && request.label) || defaultLabel)
        if (busy()) {
            const blocked = {
                ok: false,
                text: "",
                error: busyError
            }
            appendOperation(operationLabel, blocked)
            return blocked
        }

        startPending = true
        const callback = function (response) {
            startPending = false
            appendOperation(operationLabel, response)
            if (response && response.ok) {
                terminalOperationId = ""
                updateActiveOperation(response.value)
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
        const operation = active()
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
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
                    domain: String(operation && operation.domain ? operation.domain : root.domain),
                    method: String(operation && operation.method ? operation.method : ""),
                    status: "failed",
                    label: String(operation && operation.label ? operation.label : defaultLabel),
                    error: String((response && response.error) || qsTr("Runtime operation status failed."))
                }
                updateActiveOperation(failedOperation)
                appendTerminalOperation(failedOperation)
                return
            }
            updateActiveOperation(response.value)
            if (terminal(response.value)) {
                appendTerminalOperation(response.value)
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
        const operation = active()
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        if (!operationId.length) {
            return null
        }
        const callback = function (response) {
            if (response && response.ok) {
                updateActiveOperation(response.value)
            }
            appendOperation(qsTr("Cancel operation"), response)
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

    function updateActiveOperation(value) {
        activeOperation = value || null
        activeOperationRevision += 1
    }

    function clearActiveOperation() {
        activeOperation = null
        activeOperationRevision += 1
    }

    function active() {
        const revision = activeOperationRevision
        return activeOperation || null
    }

    function known() {
        const operation = active()
        return operation && String(operation.operationId || "").length > 0
    }

    function running() {
        const operation = active()
        const status = String(operation && operation.status ? operation.status : "")
        return status === "running" || status === "canceling"
    }

    function busy() {
        return startPending || running()
    }

    function cancelable() {
        const operation = active()
        const status = String(operation && operation.status ? operation.status : "")
        return (status === "running" || status === "canceling") && operation && operation.cancellable === true
    }

    function terminal(operation) {
        return OperationHistoryVocabulary.isRuntimeTerminalStatus(operation && operation.status)
    }

    function statusText() {
        const operation = active()
        const status = String(operation && operation.status ? operation.status : "")
        switch (status) {
        case "running":
            return String(operation && operation.label ? operation.label : qsTr("Running"))
        case "canceling":
            return qsTr("Canceling")
        case "completed":
            return qsTr("Complete")
        case "failed":
            return qsTr("Failed")
        case "canceled":
            return qsTr("Canceled")
        default:
            return qsTr("Idle")
        }
    }

    function tone() {
        const operation = active()
        const status = String(operation && operation.status ? operation.status : "")
        if (status === "completed") {
            return "success"
        }
        if (status === "failed") {
            return "error"
        }
        if (status === "running" || status === "canceling") {
            return "warning"
        }
        return "neutral"
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

    function appendTerminalOperation(operation, detail) {
        const operationId = String(operation && operation.operationId ? operation.operationId : "")
        if (!operationId.length || terminalOperationId === operationId) {
            return false
        }
        terminalOperationId = operationId
        const ok = String(operation.status || "") === "completed"
        appendOperation(String(operation.label || defaultLabel), {
            ok: ok,
            value: operation.result || operation,
            error: String(operation.error || "")
        })
        if (gateway && typeof gateway.appendOperationHistory === "function") {
            const detailText = terminalDetail(operation, detail)
            gateway.appendOperationHistory(operation, detailText)
        }
        terminalOperation(operation)
        return true
    }

    function terminalDetail(operation, detail) {
        if (detail !== undefined) {
            return String(detail || "")
        }
        if (terminalDetailProvider && typeof terminalDetailProvider === "function") {
            return String(terminalDetailProvider(operation) || "")
        }
        return operationSummary(operation)
    }

    function rows() {
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

    function timeText() {
        return Qt.formatTime(new Date(), "HH:mm:ss")
    }
}
