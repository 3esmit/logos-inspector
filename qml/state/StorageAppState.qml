import QtQml
import "../services/BridgeHelpers.js" as BridgeHelpers
import "appmodel/StorageTransfer.js" as StorageTransfer

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
    property string lastOperation: qsTr("None")
    property string activeCid: cidProbe
    property string pendingMethod: ""
    property string pendingLabel: ""
    property var pendingArgs: []
    property alias terminalOperationId: storageOperations.terminalOperationId
    property alias operationStartPending: storageOperations.startPending
    property alias activeOperation: storageOperations.activeOperation
    property alias activeOperationRevision: storageOperations.activeOperationRevision
    property alias operationLog: storageOperations.operationLog
    property alias operationLogRevision: storageOperations.operationLogRevision

    property NodeOperationClient operationClient: NodeOperationClient {
        id: storageOperations

        gateway: root.gateway
        domain: "storage"
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

    function storageArgs(extra) {
        const args = [
            effectiveSourceMode,
            usesRestEndpoint ? restEndpoint : ""
        ]
        return args.concat(extra || [])
    }

    function setCidProbe(value) {
        cidProbe = String(value || "").trim()
    }

    function refreshManifests(showLog) {
        if (busy || !storageDataSource()) {
            return null
        }
        const response = gateway.call("storageManifests", storageArgs([]), qsTr("Storage manifests"))
        if (showLog) {
            appendOperation(qsTr("List files"), response)
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
        if (String(method || "") !== "storageExists") {
            return startStorageOperation(method, args, label)
        }
        const response = gateway.call(method, storageArgs(args), label)
        appendOperation(label, response)
        lastOperation = response.ok ? label : qsTr("Error")
        return response
    }

    function confirmStorage(method, args, label) {
        pendingMethod = String(method || "")
        pendingArgs = [mutatingDiagnosticsEnabled === true].concat(args || [])
        pendingLabel = String(label || "")
    }

    function clearPendingStorage() {
        pendingMethod = ""
        pendingArgs = []
        pendingLabel = ""
    }

    function runPendingStorage() {
        if (!pendingMethod.length) {
            return null
        }
        const response = startStorageOperation(pendingMethod, pendingArgs, pendingLabel)
        clearPendingStorage()
        return response
    }

    function startStorageOperation(method, args, label) {
        const request = {
            domain: "storage",
            sourceMode: effectiveSourceMode,
            endpoint: restEndpoint,
            module: moduleName,
            method: String(method || ""),
            args: storageArgs(args),
            mutatingEnabled: mutatingDiagnosticsEnabled === true,
            label: String(label || "")
        }
        lastOperation = qsTr("Starting")
        const started = storageOperations.start(request, label, function (response) {
            lastOperation = response && response.ok ? qsTr("Started") : qsTr("Error")
            if (response && response.ok) {
                currentTab = "operations"
            } else {
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
        return storageOperations.poll(showResult === true, function (response) {
            if (response && response.ok && StorageTransfer.applyDispatchAck(root, response.value)) {
                return true
            }
            return false
        })
    }

    function cancelStorageOperation() {
        return storageOperations.cancel()
    }

    function appendTerminalStorageOperation(operation) {
        storageOperations.appendTerminalOperation(operation, activeStorageDetailText())
    }

    function completeTerminalStorageOperation(operation) {
        const ok = String(operation.status || "") === "completed"
        setStorageOperationResult(operation)
        lastOperation = ok ? qsTr("Complete") : qsTr("Stopped")
    }

    function setStorageOperationResult(operation) {
        const label = String(operation && operation.label ? operation.label : qsTr("Storage operation"))
        const ok = String(operation && operation.status ? operation.status : "") === "completed"
        if (ok) {
            const value = operation && operation.result !== undefined && operation.result !== null ? operation.result : operation
            gateway.setResult(label, BridgeHelpers.formatValue(value), false, value)
        } else {
            gateway.setResult(label, String(operation && operation.error ? operation.error : qsTr("Storage operation failed.")), true, null)
        }
    }

    function appendOperation(label, response) {
        storageOperations.appendOperation(label, response)
    }

    function updateActiveOperation(value) {
        storageOperations.updateActiveOperation(value)
    }

    function clearActiveOperation() {
        storageOperations.clearActiveOperation()
    }

    function activeStorageOperation() {
        return storageOperations.active()
    }

    function activeStorageOperationKnown() {
        return storageOperations.known()
    }

    function activeStorageOperationRunning() {
        return storageOperations.running()
    }

    function storageOperationBusy() {
        return storageOperations.busy()
    }

    function activeStorageOperationCancelable() {
        return storageOperations.cancelable()
    }

    function activeStorageOperationTerminal(operation) {
        return storageOperations.terminal(operation)
    }

    function activeStorageStatusText() {
        return storageOperations.statusText()
    }

    function activeStorageTone() {
        return storageOperations.tone()
    }

    function activeStorageDetailText() {
        const operation = activeStorageOperation()
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

    function applyStorageModuleEvent(eventName, args) {
        return StorageTransfer.applyModuleEvent(root, eventName, args)
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
        const operation = activeStorageOperation()
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

    function operationPayload(value) {
        return storageOperations.operationPayload(value)
    }

    function manifestArray(value) {
        const payload = operationPayload(value)
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

    function operationRows() {
        return storageOperations.rows()
    }

    function operationSummary(value) {
        return storageOperations.operationSummary(value)
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

    function timeText() {
        return Qt.formatTime(new Date(), "HH:mm:ss")
    }
}
