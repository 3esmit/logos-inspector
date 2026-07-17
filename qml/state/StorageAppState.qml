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
    property bool mutatingDiagnosticsEnabled: true
    property string currentView: ""
    property string currentTab: "files"
    property string cidProbe: ""

    property string resultTitle: ""
    property string resultText: ""
    property bool resultIsError: false
    property string resultOwner: ""
    property var sourceReport: null

    property var manifests: []
    property int manifestRequestGeneration: 0
    property int manifestBootstrapGeneration: 0
    property bool manifestRefreshDeferred: false
    property bool manifestObservationPending: false
    property bool manifestDeferredShowLog: false
    property bool manifestBusyDeferred: false
    property bool manifestBusyShowLog: false
    property int diagnosticRequestGeneration: 0
    property var manifestRefreshContext: null
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
            return root.storageOperationDetail(operation)
        }

        onTerminalOperation: function (operation) {
            root.completeTerminalStorageOperation(operation)
        }
    }

    property Connections capabilityGateConnections: Connections {
        target: root.gateFacade
        enabled: target !== null

        function onRevisionChanged() {
            root.retryDeferredManifestRefresh()
        }
    }

    onCidProbeChanged: {
        if (activeCid !== cidProbe) {
            activeCid = cidProbe
        }
    }

    onAdapterInitializationChanged: {
        const reloadVisiblePage = currentView === "storage"
        invalidateSourceRequests()
        if (reloadVisiblePage) {
            const reloadGeneration = manifestBootstrapGeneration
            Qt.callLater(function () {
                if (root.currentView === "storage"
                        && reloadGeneration === root.manifestBootstrapGeneration) {
                    root.refreshManifests(false)
                }
            })
        }
    }

    onBusyChanged: {
        if (!busy) {
            if (manifestBusyDeferred) {
                const showBusyLog = manifestBusyShowLog
                manifestBusyDeferred = false
                manifestBusyShowLog = false
                refreshManifests(showBusyLog)
            } else {
                retryDeferredManifestRefresh()
            }
        }
    }

    function invalidateSourceRequests() {
        manifestRequestGeneration += 1
        manifestBootstrapGeneration += 1
        manifestRefreshDeferred = false
        manifestObservationPending = false
        manifestDeferredShowLog = false
        manifestBusyDeferred = false
        manifestBusyShowLog = false
        diagnosticRequestGeneration += 1
        manifestRefreshContext = null
        storageOperations.clearActive()
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
        return supportsMutatingDiagnostics
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
            manifestBusyDeferred = true
            manifestBusyShowLog = manifestBusyShowLog || showLog === true
            lastOperation = qsTr("Waiting")
            return null
        }
        const gate = storageActionGate("manifests", [])
        if (!gate.enabled) {
            const gateStatus = String(gate.status || "")
            if (gateStatus === "loading"
                    || (showLog !== true && gateStatus === "disabled"
                        && storageDataSource())) {
                return deferManifestRefresh(showLog === true)
            }
            return blockedStorageResponse(qsTr("List files"), gate, showLog === true)
        }
        manifestRefreshDeferred = false
        manifestDeferredShowLog = false
        manifestBusyDeferred = false
        manifestBusyShowLog = false
        if (storageOperations.view.busy) {
            const blocked = storageOperations.start("storageManifests", [], qsTr("Storage manifests"))
            lastOperation = qsTr("Busy")
            return blocked
        }
        manifestRequestGeneration += 1
        const requestGeneration = manifestRequestGeneration
        manifestRefreshContext = {
            generation: requestGeneration,
            showLog: showLog === true,
            operationId: ""
        }
        const started = storageOperations.start("storageManifests", [], qsTr("Storage manifests"), function (response, operation) {
            const context = manifestRefreshContext
            if (!context || context.generation !== requestGeneration) {
                return
            }
            if (!response || !response.ok) {
                manifestRefreshContext = null
                lastOperation = qsTr("Error")
                return
            }
            if (storageOperations.isTerminal(operation)) {
                return
            }
            manifestRefreshContext = {
                generation: requestGeneration,
                showLog: context.showLog,
                operationId: String(operation && operation.operationId || "")
            }
            lastOperation = operationStatusText(operation)
        })
        if (started && started.ok === false) {
            manifestRefreshContext = null
            lastOperation = String(started.error || "") === storageOperations.busyError ? qsTr("Busy") : qsTr("Error")
            return started
        }
        return null
    }

    function deferManifestRefresh(showLog) {
        manifestRefreshDeferred = true
        manifestDeferredShowLog = manifestDeferredShowLog || showLog === true
        lastOperation = qsTr("Loading")
        if (manifestObservationPending) {
            return null
        }
        if (!gateway || typeof gateway.observeStorage !== "function") {
            const showDeferredLog = manifestDeferredShowLog
            manifestRefreshDeferred = false
            manifestDeferredShowLog = false
            const gate = storageActionGate("manifests", [])
            return blockedStorageResponse(
                qsTr("List files"), gate, showDeferredLog)
        }

        manifestBootstrapGeneration += 1
        const generation = manifestBootstrapGeneration
        manifestObservationPending = true
        return gateway.observeStorage(function (response) {
            if (generation !== manifestBootstrapGeneration) {
                return
            }
            manifestObservationPending = false
            if (!response || response.ok !== true) {
                const showDeferredLog = manifestDeferredShowLog
                manifestRefreshDeferred = false
                manifestDeferredShowLog = false
                lastOperation = qsTr("Error")
                if (showDeferredLog) {
                    storageOperations.appendResult(qsTr("Storage source"), {
                        ok: false,
                        value: null,
                        text: "",
                        error: String(response && response.error
                            ? response.error : qsTr("Storage source observation failed."))
                    })
                }
                return
            }
            retryDeferredManifestRefresh()
        })
    }

    function retryDeferredManifestRefresh() {
        if (!manifestRefreshDeferred || manifestObservationPending || busy) {
            return null
        }
        const gate = storageActionGate("manifests", [])
        if (!gate.enabled) {
            if (String(gate.status || "") === "loading") {
                return null
            }
            const showDeferredLog = manifestDeferredShowLog
            manifestRefreshDeferred = false
            manifestDeferredShowLog = false
            return blockedStorageResponse(
                qsTr("List files"), gate, showDeferredLog)
        }
        const showDeferredLog = manifestDeferredShowLog
        manifestRefreshDeferred = false
        manifestDeferredShowLog = false
        return refreshManifests(showDeferredLog)
    }

    function runStorage(method, args, label) {
        const command = SourceOperationCommandCatalog.storageCommand(method, args)
        if (!command.runtime) {
            diagnosticRequestGeneration += 1
        }
        const requestGeneration = diagnosticRequestGeneration
        const gate = storageActionGate(command.action, command.requiredInputs)
        if (!gate.enabled) {
            return blockedStorageResponse(label, gate, true)
        }
        if (command.runtime) {
            return startStorageOperation(method, args, label)
        }
        const acceptResponse = function () {
            return requestGeneration === diagnosticRequestGeneration
        }
        return gateway.request(method, storageArgs(method, args), label, true, function (response) {
            if (requestGeneration !== diagnosticRequestGeneration) {
                return
            }
            storageOperations.appendResult(label, response)
            lastOperation = response && response.ok ? label : qsTr("Error")
        }, acceptResponse)
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
                if (!storageOperations.isTerminal(operation)) {
                    lastOperation = operationStatusText(operation || response.value)
                }
                currentTab = "operations"
            } else {
                lastOperation = qsTr("Error")
                gateway.setResult(String(label || qsTr("Storage operation")), String((response && response.error) || qsTr("Storage operation failed.")), true, null, "storage")
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
        if (String(operation && operation.method || "") === "storageManifests") {
            completeManifestRefresh(operation)
            return
        }
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

    function completeManifestRefresh(operation) {
        const context = manifestRefreshContext
        const operationId = String(operation && operation.operationId || "")
        if (!context || (String(context.operationId || "").length > 0
                && String(context.operationId) !== operationId)) {
            return
        }
        manifestRefreshContext = null
        if (isSuccessfulTerminal(operation)) {
            const value = operation && operation.result !== undefined && operation.result !== null
                ? operation.result
                : operation
            manifests = manifestArray(value)
            lastOperation = qsTr("List")
        } else {
            lastOperation = qsTr("Error")
        }
    }

    function setStorageOperationResult(operation) {
        const label = String(operation && operation.label ? operation.label : qsTr("Storage operation"))
        const ok = isSuccessfulTerminal(operation)
        if (ok) {
            const value = operation && operation.result !== undefined && operation.result !== null ? operation.result : operation
            gateway.setResult(label, BridgeHelpers.formatValue(value), false, value, "storage")
        } else {
            gateway.setResult(label, String(operation && operation.error ? operation.error : qsTr("Storage operation failed.")), true, null, "storage")
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

    function storageOperationDetail(operation) {
        if (String(operation && operation.method || "") === "storageManifests") {
            if (operation && operation.error) {
                return String(operation.error)
            }
            return qsTr("%1 manifests").arg(manifestArray(operation && operation.result).length)
        }
        return activeStorageDetailText()
    }

    function applyStorageModuleEvent(eventName, args, onResponse, forwardRuntimeEvent) {
        const event = args && args.__moduleEventEnvelope === true ? args : {
            moduleName: moduleName,
            eventName: String(eventName || ""),
            args: Array.isArray(args) ? args : (args === undefined || args === null ? [] : [args])
        }
        if (forwardRuntimeEvent === false) {
            return true
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
