import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers
import "OperationHistoryVocabulary.js" as OperationHistoryVocabulary
import "source_operations/NodeOperationRequest.js" as NodeOperationRequest

QtObject {
    id: root

    required property var gateway

    required property string domain
    property var adapterInitialization: ({ source_mode: "", inputs: ({}) })
    property bool mutatingDiagnosticsEnabled: true
    property string defaultLabel: qsTr("Runtime operation")
    property string busyError: qsTr("A runtime operation is already running.")
    property var terminalDetailProvider: null
    property var operationValidator: null

    property string pendingMethod: ""
    property string pendingLabel: ""
    property var pendingArgs: []
    property bool startPending: false
    property bool pollPending: false
    property int callbackEpoch: 0
    property int nextPollToken: 0
    property int activePollToken: 0
    property string activePollOperationId: ""
    property bool cancelPending: false
    property string pendingCancelOperationId: ""
    property var pendingStartOperations: ({})
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
            NodeOperationRequest.payload(domain, method, args)
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
            NodeOperationRequest.payload(domain, method, args)
        )
        request.domain = domain
        request.method = String(method || "")
        request.label = operationLabel
        invalidateInFlightRequests()
        const requestEpoch = callbackEpoch
        startPending = true
        const callback = function (response) {
            if (requestEpoch !== callbackEpoch) {
                return
            }
            startPending = false
            appendResult(operationLabel, response)
            let projectedOperation = null
            if (response && response.ok) {
                terminalOperationId = ""
                projectedOperation = reconcileStartOperation(response.value)
                if (acceptUpdate(projectedOperation)) {
                    started(projectedOperation)
                    if (isTerminal(projectedOperation)) {
                        acceptTerminal(projectedOperation)
                    }
                }
            } else {
                pendingStartOperations = ({})
                startFailed(response)
            }
            if (onResponse) {
                onResponse(response, projectedOperation)
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
        if (!operationId.length || startPending || !view.running || pollPending) {
            return null
        }
        nextPollToken += 1
        const pollToken = nextPollToken
        const requestEpoch = callbackEpoch
        activePollToken = pollToken
        activePollOperationId = operationId
        pollPending = true
        const callback = function (response) {
            if (!releasePoll(requestEpoch, pollToken, operationId)
                    || !isCurrentOperationId(operationId)) {
                return
            }
            if (onResponse && onResponse(response, operation) === true) {
                return
            }
            if (!response || !response.ok) {
                appendResult(qsTr("Runtime operation status"), response || {
                    ok: false,
                    error: qsTr("Runtime operation status failed.")
                })
                return
            }
            if (!matchesOperationId(response.value, operationId)) {
                return
            }
            if (acceptUpdate(response.value) && isTerminal(response.value)) {
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
        releasePoll(requestEpoch, pollToken, operationId)
        return null
    }

    function cancel(onResponse) {
        const operationId = String(activeOperation && activeOperation.operationId || "")
        if (!operationId.length || !canCancelActiveOperation()) {
            return null
        }
        const requestEpoch = callbackEpoch
        cancelPending = true
        pendingCancelOperationId = operationId
        const callback = function (response) {
            if (requestEpoch !== callbackEpoch
                    || pendingCancelOperationId !== operationId) {
                return
            }
            cancelPending = false
            pendingCancelOperationId = ""
            if (response && response.ok && isCurrentOperationId(operationId)
                    && matchesOperationId(response.value, operationId)) {
                if (acceptUpdate(response.value) && isTerminal(response.value)) {
                    acceptTerminal(response.value)
                }
            }
            appendResult(qsTr("Cancel operation"), response)
            if (onResponse) {
                onResponse(response)
            }
        }

        try {
            if (gateway && typeof gateway.runtimeOperationCancel === "function") {
                return gateway.runtimeOperationCancel(operationId, false, callback)
            }
            if (gateway && typeof gateway.nodeOperationCancel === "function") {
                return gateway.nodeOperationCancel(operationId, false, callback)
            }
            if (gateway && typeof gateway.request === "function") {
                return gateway.request("runtimeOperationCancel", [operationId], qsTr("Cancel operation"), false, callback)
            }
        } catch (error) {
            cancelPending = false
            pendingCancelOperationId = ""
            throw error
        }
        cancelPending = false
        pendingCancelOperationId = ""
        return null
    }

    function ingestModuleEvent(event, onResponse) {
        const envelope = moduleEventEnvelope(event)
        if (!envelope.moduleName.length || !envelope.eventName.length) {
            return null
        }
        const expectedOperationId = startPending
            ? ""
            : String(activeOperation && activeOperation.operationId || "")
        const requestEpoch = callbackEpoch
        const callback = function (response) {
            if (requestEpoch !== callbackEpoch) {
                return
            }
            const operation = response && response.ok && response.value
                ? response.value.operation || null
                : null
            const currentOperationId = String(activeOperation && activeOperation.operationId || "")
            let applied = false
            if (operation && currentOperationId.length
                    && matchesOperationId(operation, currentOperationId)
                    && (!expectedOperationId.length || expectedOperationId === currentOperationId)) {
                applied = acceptUpdate(operation)
                if (applied && isTerminal(operation)) {
                    acceptTerminal(operation)
                }
            } else if (operation && startPending) {
                rememberPendingStartOperation(operation)
            }
            if (onResponse) {
                onResponse(response, operation, applied)
            }
        }

        if (gateway && typeof gateway.runtimeOperationModuleEvent === "function") {
            return gateway.runtimeOperationModuleEvent(envelope, false, callback)
        }
        if (gateway && typeof gateway.request === "function") {
            return gateway.request(
                "runtimeOperationModuleEvent",
                [envelope],
                qsTr("Runtime module event"),
                false,
                callback
            )
        }
        return null
    }

    function acceptUpdate(value) {
        const operation = value || null
        if (!acceptsOperation(operation)) {
            return false
        }
        const operationId = String(operation && operation.operationId || "")
        if (!operationId.length) {
            return false
        }
        const currentId = String(activeOperation && activeOperation.operationId || "")
        if (terminalOperationId === operationId
                || (currentId === operationId
                    && !OperationHistoryVocabulary.runtimeSnapshotIsNewer(activeOperation, operation))) {
            return false
        }
        activeOperation = operation
        activeOperationRevision += 1
        return true
    }

    function clearActive() {
        invalidateInFlightRequests()
        activeOperation = null
        activeOperationRevision += 1
    }

    function reset() {
        clearConfirmation()
        invalidateInFlightRequests()
        activeOperation = null
        activeOperationRevision += 1
        terminalOperationId = ""
        operationLog = []
        operationLogRevision += 1
        lastOperation = qsTr("None")
    }

    function invalidateInFlightRequests() {
        callbackEpoch += 1
        startPending = false
        pollPending = false
        activePollToken = 0
        activePollOperationId = ""
        cancelPending = false
        pendingCancelOperationId = ""
        pendingStartOperations = ({})
    }

    function rememberPendingStartOperation(operation) {
        if (!acceptsOperation(operation)) {
            return false
        }
        const operationId = String(operation && operation.operationId || "")
        if (!operationId.length) {
            return false
        }
        const pending = pendingStartOperations && typeof pendingStartOperations === "object"
            ? pendingStartOperations
            : ({})
        const current = pending[operationId] || null
        if (!OperationHistoryVocabulary.runtimeSnapshotIsNewer(current, operation)) {
            return false
        }
        const next = ({})
        const keys = Object.keys(pending)
        for (let i = 0; i < keys.length; ++i) {
            next[keys[i]] = pending[keys[i]]
        }
        next[operationId] = operation
        pendingStartOperations = next
        return true
    }

    function acceptsOperation(operation) {
        return typeof operationValidator !== "function"
            || operationValidator(operation) === true
    }

    function reconcileStartOperation(operation) {
        const startedOperation = operation || null
        const operationId = String(startedOperation && startedOperation.operationId || "")
        const pending = operationId.length && pendingStartOperations
            ? pendingStartOperations[operationId] || null
            : null
        pendingStartOperations = ({})
        if (!startedOperation || !pending) {
            return startedOperation
        }
        return OperationHistoryVocabulary.runtimeSnapshotIsNewer(startedOperation, pending)
            ? pending
            : startedOperation
    }

    function releasePoll(requestEpoch, pollToken, operationId) {
        if (requestEpoch !== callbackEpoch
                || activePollToken !== pollToken
                || activePollOperationId !== operationId) {
            return false
        }
        pollPending = false
        activePollToken = 0
        activePollOperationId = ""
        return true
    }

    function appendResult(label, response) {
        const rows = Array.isArray(operationLog) ? operationLog.slice(0) : []
        const explicitStatus = String(response && response.status || "").trim()
        rows.unshift({
            time: timeText(),
            label: String(label || ""),
            status: explicitStatus.length > 0
                ? explicitStatus
                : (response && response.ok ? qsTr("ok") : qsTr("error")),
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
        const ok = OperationHistoryVocabulary.isRuntimeSuccessfulTerminalStatus(operation.status)
        const canceled = String(operation.status || "") === "canceled"
        appendResult(String(operation.label || defaultLabel), {
            ok: ok,
            status: canceled ? qsTr("canceled") : "",
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
        const running = OperationHistoryVocabulary.isRuntimeActiveStatus(status)
        return {
            activeRevision: activeRevision,
            logRevision: logRevision,
            active: operation,
            startPending: startPending,
            pollPending: pollPending,
            known: operation !== null && String(operation.operationId || "").length > 0,
            running: running,
            busy: startPending || running,
            cancelable: canCancelActiveOperation(),
            terminal: isTerminal(operation),
            statusText: OperationHistoryVocabulary.runtimeStatusText(operation, defaultLabel),
            tone: OperationHistoryVocabulary.runtimeTone(operation),
            rows: operationRows()
        }
    }

    function isTerminal(operation) {
        return OperationHistoryVocabulary.isRuntimeTerminalStatus(operation && operation.status)
    }

    function canCancelActiveOperation() {
        const operation = activeOperation || null
        const status = String(operation && operation.status || "")
        return operation !== null
            && !cancelPending
            && status !== "canceling"
            && OperationHistoryVocabulary.isRuntimeActiveStatus(status)
            && operation.cancellable === true
    }

    function matchesOperationId(operation, expectedOperationId) {
        const operationId = String(operation && operation.operationId || "")
        return operationId.length > 0 && operationId === String(expectedOperationId || "")
    }

    function isCurrentOperationId(expectedOperationId) {
        return String(activeOperation && activeOperation.operationId || "")
            === String(expectedOperationId || "")
    }

    function moduleEventEnvelope(event) {
        const value = event && typeof event === "object" ? event : ({})
        return {
            moduleName: String(value.moduleName || ""),
            eventName: String(value.eventName || ""),
            args: Array.isArray(value.args) ? value.args.slice(0) : []
        }
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
