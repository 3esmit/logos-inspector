import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers
import "storage/StorageTransfer.js" as StorageTransfer
import "source_operations/SourceOperationCommandCatalog.js" as SourceOperationCommandCatalog

QtObject {
    id: root

    required property var gateway
    property var gateFacade: null

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
    property string moduleName: "storage_module"
    property string networkPreset: "logos.test"
    property bool mutatingDiagnosticsEnabled: false
    property string currentView: ""
    property string currentTab: "files"
    property string cidProbe: ""

    property string resultTitle: ""
    property string resultText: ""
    property bool resultIsError: false
    property string resultOwner: ""
    property var sourceReport: null

    property var manifests: []
    property alias lastOperation: storageOperations.lastOperation
    property string activeCid: cidProbe
    readonly property var pendingOperation: storageOperations.confirmation
    readonly property var operation: storageOperations.view

    property SourceOperationSession operationSession: SourceOperationSession {
        id: storageOperations

        gateway: root.gateway
        domain: "storage"
        adapterInitialization: root.adapterInitialization
        mutatingDiagnosticsEnabled: root.mutatingDiagnosticsEnabled
        defaultLabel: qsTr("Storage operation")
        busyError: qsTr("A storage operation is already running.")
        terminalDetailProvider: function (operation) {
            return root.activeStorageDetailText()
        }

        onTerminalOperation: function (operation) {
            root.completeTerminalStorageOperation(operation)
        }
    }

    onCidProbeChanged: {
        if (activeCid !== cidProbe) {
            activeCid = cidProbe
        }
    }

    function resultVisible() {
        return resultOwner === "storage" && (resultText.length > 0 || resultTitle.length > 0)
    }

    function clearResult() {
        gateway.clearResult()
    }

    function openStorageSettings() {
        gateway.openSettings("network", "storage")
    }

    function sourceBadges() {
        return [qsTr("Storage"), sourceLabel, shortText(sourceTarget, 42), networkPreset]
    }

    function storageRestSource() {
        return sourceTargetKind === "rest_endpoint"
    }

    function storageMutatingSource() {
        return supportsMutatingDiagnostics && mutatingDiagnosticsEnabled === true
    }

    function storageDataSource() {
        return usesRestEndpoint || effectiveSourceMode === "module"
    }

    function storageActionGate(action, requiredInputs) {
        const options = {
            required_inputs: Array.isArray(requiredInputs) ? requiredInputs : []
        }
        if (gateFacade && typeof gateFacade.storageGate === "function") {
            return gateFacade.storageGate(action, options)
        }
        return {
            enabled: false,
            status: "disabled",
            missing: [{ dependency: "storage", label: qsTr("Storage capability"), status: "unavailable", capability: "storage", provenance: "capability_registry" }],
            warnings: [],
            provenance: ["capability_registry"]
        }
    }

    function storageActionEnabled(action, requiredInputs) {
        return storageActionGate(action, requiredInputs).enabled === true
    }

    function storageActionProblem(action, requiredInputs) {
        const gate = storageActionGate(action, requiredInputs)
        return gate.enabled ? "" : gateDetailText(gate)
    }

    function storageArgs(method, extra) {
        return storageOperations.requestArgs(method, extra)
    }

    function setCidProbe(value) {
        cidProbe = String(value || "").trim()
    }

    function refreshManifests(showLog) {
        if (busy) {
            return null
        }
        const gate = storageActionGate("manifests", [])
        if (!gate.enabled) {
            return blockedStorageResponse(qsTr("List files"), gate, showLog === true)
        }
        const response = gateway.call("storageManifests", storageArgs("storageManifests", []), qsTr("Storage manifests"))
        if (showLog) {
            storageOperations.appendResult(qsTr("List files"), response)
        }
        if (response.ok) {
            manifests = manifestArray(response.value)
            lastOperation = qsTr("List")
        } else if (showLog) {
            lastOperation = qsTr("Error")
        }
        return response
    }

    function runStorage(method, args, label) {
        const command = SourceOperationCommandCatalog.storageCommand(method, args)
        const gate = storageActionGate(command.action, command.requiredInputs)
        if (!gate.enabled) {
            return blockedStorageResponse(label, gate, true)
        }
        if (command.runtime) {
            return startStorageOperation(method, args, label)
        }
        const response = gateway.call(method, storageArgs(method, args), label)
        storageOperations.appendResult(label, response)
        lastOperation = response.ok ? label : qsTr("Error")
        return response
    }

    function confirmStorage(method, args, label) {
        storageOperations.confirm(method, args, label)
    }

    function clearPendingStorage() {
        storageOperations.clearConfirmation()
    }

    function runPendingStorage() {
        return storageOperations.runConfirmed(function (method, args, label) {
            return root.startStorageOperation(method, args, label)
        })
    }

    function startStorageOperation(method, args, label) {
        const command = SourceOperationCommandCatalog.storageCommand(method, args)
        const gate = storageActionGate(command.action, command.requiredInputs)
        if (!gate.enabled) {
            return blockedStorageResponse(label, gate, true)
        }
        lastOperation = qsTr("Starting")
        const started = storageOperations.start(method, args, label, function (response, operation) {
            if (response && response.ok) {
                lastOperation = operationStatusText(operation || response.value)
                currentTab = "operations"
            } else {
                lastOperation = qsTr("Error")
                gateway.setResult(String(label || qsTr("Storage operation")), String((response && response.error) || qsTr("Storage operation failed.")), true, null)
            }
        })
        if (started && started.ok === false) {
            lastOperation = String(started.error || "") === storageOperations.busyError ? qsTr("Busy") : qsTr("Error")
            return started
        }
        return null
    }

    function pollStorageOperation(showResult) {
        return storageOperations.poll(showResult === true)
    }

    function cancelStorageOperation() {
        return storageOperations.cancel()
    }

    function appendTerminalStorageOperation(operation) {
        return storageOperations.acceptTerminal(operation, activeStorageDetailText())
    }

    function completeTerminalStorageOperation(operation) {
        const ok = isSuccessfulTerminal(operation)
        setStorageOperationResult(operation)
        if (terminalRefreshesStorageObservations(operation)
                && gateway && typeof gateway.refreshStorageObservations === "function") {
            gateway.refreshStorageObservations()
        }
        lastOperation = String(operation && operation.status || "") === "dispatched"
            ? qsTr("Dispatched")
            : (ok ? qsTr("Complete") : qsTr("Stopped"))
    }

    function setStorageOperationResult(operation) {
        const label = String(operation && operation.label ? operation.label : qsTr("Storage operation"))
        const ok = isSuccessfulTerminal(operation)
        if (ok) {
            const value = operation && operation.result !== undefined && operation.result !== null ? operation.result : operation
            gateway.setResult(label, BridgeHelpers.formatValue(value), false, value)
        } else {
            gateway.setResult(label, String(operation && operation.error ? operation.error : qsTr("Storage operation failed.")), true, null)
        }
    }

    function blockedStorageResponse(label, gate, logResponse) {
        const response = {
            ok: false,
            value: null,
            text: "",
            error: gateDetailText(gate)
        }
        lastOperation = qsTr("Blocked")
        if (logResponse) {
            storageOperations.appendResult(String(label || qsTr("Storage operation")), response)
        }
        return response
    }

    function gateDetailText(gate) {
        return SourceOperationCommandCatalog.gateDetailText(gate, qsTr("Storage capability"))
    }

    function storageActionForMethod(method) {
        return SourceOperationCommandCatalog.storageActionForMethod(method)
    }

    function requiredInputsForStorageAction(action, args) {
        return SourceOperationCommandCatalog.storageRequiredInputs(action, args)
    }

    function activeStorageDetailText() {
        const operation = storageOperations.view.active
        if (!operation) {
            return qsTr("No active operation.")
        }
        const detail = [
            shortText(operation.cid, 28),
            activeStorageProgressText(),
            shortText(operation.path, 48)
        ].filter(value => String(value || "").length > 0)
        if (operation.error) {
            detail.push(String(operation.error))
        }
        return detail.join(" / ")
    }

    function applyStorageModuleEvent(eventName, args, onResponse) {
        const event = args && args.__moduleEventEnvelope === true ? args : {
            moduleName: moduleName,
            eventName: String(eventName || ""),
            args: Array.isArray(args) ? args : (args === undefined || args === null ? [] : [args])
        }
        return storageOperations.ingestModuleEvent(event, onResponse)
    }

    function storageSpaceSummary() {
        return StorageTransfer.spaceSummary(root)
    }

    function storageSpaceTone() {
        return StorageTransfer.spaceTone(root)
    }

    function storageSpaceValue() {
        return StorageTransfer.spaceValue(root)
    }

    function activeStorageProgressText() {
        const operation = storageOperations.view.active
        if (!operation) {
            return ""
        }
        const written = Number(operation.bytesWritten || 0)
        const total = Number(operation.contentLength || 0)
        if (Number.isFinite(total) && total > 0) {
            const percent = Math.min(100, Math.max(0, Math.floor((written / total) * 100)))
            return qsTr("%1 / %2 bytes (%3%)").arg(valueText(written)).arg(valueText(total)).arg(percent)
        }
        return qsTr("%1 bytes").arg(valueText(written))
    }

    function manifestArray(value) {
        const payload = storageOperations.payload(value)
        if (Array.isArray(payload)) {
            return payload
        }
        if (payload && Array.isArray(payload.content)) {
            return payload.content
        }
        if (payload && Array.isArray(payload.manifests)) {
            return payload.manifests
        }
        if (payload && Array.isArray(payload.value)) {
            return payload.value
        }
        return []
    }

    function manifestRows() {
        if (manifests.length === 0) {
            return [{
                cid: "",
                name: qsTr("No local manifests"),
                detail: qsTr(""),
                size: "-",
                mime: "-"
            }]
        }
        return manifests.map(function (manifest) {
            const row = manifest || {}
            const metadata = row.manifest || {}
            const cid = String(row.cid || row.CID || row.id || "")
            const name = String(metadata.filename || row.filename || row.name || row.path || cid || qsTr("Untitled"))
            const size = metadata.datasetSize || row.datasetSize || row.size || row.bytes || row.totalSize || "-"
            const blockSize = metadata.blockSize || row.blockSize || row.block_size || ""
            return {
                cid: cid,
                name: name,
                detail: blockSize ? qsTr("block %1").arg(blockSize) : String(metadata.treeCid || row.treeCid || row.tree_cid || ""),
                size: String(size),
                mime: String(metadata.mimetype || row.mimetype || row.mimeType || row.contentType || "-")
            }
        })
    }

    function chunkSizeValue(text) {
        const parsed = Number(String(text || "").trim())
        if (!isFinite(parsed) || parsed <= 0) {
            return 65536
        }
        return Math.floor(parsed)
    }

    function shortText(value, max) {
        const text = String(value || "")
        const limit = Math.max(8, Number(max || 42))
        if (text.length <= limit) {
            return text
        }
        return text.slice(0, Math.max(3, limit - 1)) + "..."
    }

    function valueText(value) {
        return gateway.valueText(value)
    }

    function isSuccessfulTerminal(operation) {
        const status = String(operation && operation.status || "")
        return status === "completed" || status === "dispatched"
    }

    function terminalRefreshesStorageObservations(operation) {
        return String(operation && operation.status || "") === "completed"
            && String(operation && operation.method || "") === "storageUploadUrl"
    }

    function operationStatusText(operation) {
        switch (String(operation && operation.status || "")) {
        case "awaiting_external":
            return qsTr("Waiting")
        case "running":
        case "canceling":
            return qsTr("Running")
        case "completed":
            return qsTr("Complete")
        case "dispatched":
            return qsTr("Dispatched")
        default:
            return qsTr("Started")
        }
    }

    function timeText() {
        return Qt.formatTime(new Date(), "HH:mm:ss")
    }
}
