import QtQml

QtObject {
    id: root

    required property var gateway

    property string domain: ""
    property string moduleName: ""
    property string effectiveSourceMode: "rest"
    property string restEndpoint: ""
    property bool usesRestEndpoint: false
    property bool mutatingDiagnosticsEnabled: false
    property bool sourceArgsIncludeMutatingFlag: false
    property string defaultLabel: qsTr("Runtime operation")
    property string busyError: qsTr("A runtime operation is already running.")
    property var terminalDetailProvider: null

    property string pendingMethod: ""
    property string pendingLabel: ""
    property var pendingArgs: []

    property alias startPending: operations.startPending
    property alias activeOperation: operations.activeOperation
    property alias activeOperationRevision: operations.activeOperationRevision
    property alias terminalOperationId: operations.terminalOperationId
    property alias operationLog: operations.operationLog
    property alias operationLogRevision: operations.operationLogRevision

    signal started(var operation)
    signal startFailed(var response)
    signal terminalOperation(var operation)

    property RuntimeOperationClient operationClient: RuntimeOperationClient {
        id: operations

        gateway: root.gateway
        domain: root.domain
        defaultLabel: root.defaultLabel
        busyError: root.busyError
        terminalDetailProvider: root.terminalDetailProvider

        onStarted: function (operation) {
            root.started(operation)
        }

        onStartFailed: function (response) {
            root.startFailed(response)
        }

        onTerminalOperation: function (operation) {
            root.terminalOperation(operation)
        }
    }

    function sourceArgs(extra) {
        const args = [
            effectiveSourceMode,
            usesRestEndpoint ? restEndpoint : ""
        ]
        if (sourceArgsIncludeMutatingFlag) {
            args.push(mutatingDiagnosticsEnabled === true)
        }
        return args.concat(extra || [])
    }

    function confirm(method, args, label, prependMutatingFlag) {
        pendingMethod = String(method || "")
        pendingArgs = prependMutatingFlag === true ? [mutatingDiagnosticsEnabled === true].concat(args || []) : (args || [])
        pendingLabel = String(label || "")
    }

    function clearPending() {
        pendingMethod = ""
        pendingArgs = []
        pendingLabel = ""
    }

    function runPending(callback) {
        if (!pendingMethod.length || typeof callback !== "function") {
            return null
        }
        const method = pendingMethod
        const args = pendingArgs
        const label = pendingLabel
        const response = callback(method, args, label)
        clearPending()
        return response
    }

    function operationRequest(method, args, label) {
        return {
            domain: domain,
            sourceMode: effectiveSourceMode,
            endpoint: restEndpoint,
            module: moduleName,
            method: String(method || ""),
            args: sourceArgs(args),
            mutatingEnabled: mutatingDiagnosticsEnabled === true,
            label: String(label || "")
        }
    }

    function startOperation(method, args, label, onResponse) {
        return operations.start(operationRequest(method, args, label), label, onResponse)
    }

    function pollOperation(showResult, onResponse) {
        return operations.poll(showResult === true, onResponse)
    }

    function cancelOperation(onResponse) {
        return operations.cancel(onResponse)
    }

    function appendOperation(label, response) {
        operations.appendOperation(label, response)
    }

    function appendTerminalOperation(operation, detail) {
        return operations.appendTerminalOperation(operation, detail)
    }

    function updateActiveOperation(value) {
        operations.updateActiveOperation(value)
    }

    function clearActiveOperation() {
        operations.clearActiveOperation()
    }

    function active() {
        return operations.active()
    }

    function known() {
        return operations.known()
    }

    function running() {
        return operations.running()
    }

    function busy() {
        return operations.busy()
    }

    function cancelable() {
        return operations.cancelable()
    }

    function terminal(operation) {
        return operations.terminal(operation)
    }

    function statusText() {
        return operations.statusText()
    }

    function tone() {
        return operations.tone()
    }

    function rows() {
        return operations.rows()
    }

    function operationPayload(value) {
        return operations.operationPayload(value)
    }

    function operationSummary(value) {
        return operations.operationSummary(value)
    }
}
