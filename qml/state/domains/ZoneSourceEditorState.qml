import QtQml
import "../ConfirmationPolicy.js" as ConfirmationPolicy
import "../OperationHistoryVocabulary.js" as OperationHistoryVocabulary
import "ZoneInspectionContract.js" as ZoneInspectionContract

QtObject {
    id: root

    required property var gateway
    required property var activeZoneContext
    required property string verification
    required property bool catalogSnapshotUsable
    property bool managedIndexerControlSnapshotUsable: false
    required property var networkScope
    required property string networkScopeKey
    required property int sourceGeneration
    required property double sourceRevision
    property var appModel: null
    property var sourceDescriptor: null
    readonly property int basecampLocalNodesRevision:
        Number(appModel && appModel.localNodesRevision || 0)

    readonly property string activeZoneId: activeZoneContext
        ? String(activeZoneContext.channel_id || "") : ""
    property string sourceMutationError: ""
    property var sourceMutationWarning: null
    property bool sourceMutationInFlight: false
    property int sourceMutationRequestRevision: 0

    property var managedIndexerReport: null
    property var managedIndexerNode: null
    property var managedIndexerRuntime: null
    property bool managedIndexerRefreshInFlight: false
    property bool managedIndexerControlInFlight: false
    property bool managedIndexerStatusStale: true
    property string managedIndexerError: ""
    property string managedIndexerResult: ""
    property int managedIndexerRequestRevision: 0
    property var managedIndexerRuntimeOperation: null
    property string managedIndexerRuntimeAction: ""
    property var managedIndexerRuntimeCallback: null
    property bool managedIndexerRuntimePollInFlight: false
    property int managedIndexerRuntimePollToken: 0
    property string managedIndexerRuntimePollError: ""
    readonly property bool managedIndexerOperationRunning:
        managedIndexerControlInFlight
        && managedIndexerRuntimeOperation !== null
        && OperationHistoryVocabulary.isRuntimeActiveStatus(
            managedIndexerRuntimeOperation.status)

    property var managedIndexerConfigSnapshot: null
    property var managedIndexerConfigValidation: null
    property string managedIndexerConfigValidationText: ""
    property string managedIndexerConfigError: ""
    property bool managedIndexerConfigLoading: false
    property bool managedIndexerConfigSaving: false
    property bool managedIndexerConfigValidationLoading: false
    property bool managedIndexerConfigDraftDirty: false
    property int managedIndexerConfigRequestRevision: 0

    signal sourceMutationFinished(var response)
    signal sourceMutationAccepted(var report)
    signal managedIndexerLifecycleChanged()

    function resetSourceEditorState(preserveMutationWarning) {
        const retainedWarning = preserveMutationWarning === true
            ? sourceMutationWarning : null
        sourceMutationRequestRevision += 1
        sourceMutationInFlight = false
        sourceMutationError = ""
        sourceMutationWarning = retainedWarning
        managedIndexerRequestRevision += 1
        managedIndexerRefreshInFlight = false
        managedIndexerControlInFlight = false
        clearManagedIndexerRuntimeOperation()
        managedIndexerStatusStale = true
        managedIndexerError = ""
        managedIndexerResult = ""
        clearManagedIndexerConfig()
    }

    function localNodeProfile() {
        const profile = String(appModel && appModel.networkProfile || "default")
            .trim().toLowerCase()
        return profile === "local" ? "local" : "default"
    }

    function bedrockEndpoint() {
        const source = sourceDescriptor || null
        if (!source) {
            return ""
        }
        if (String(source.kind || "") === "direct_http") {
            const endpoint = String(source.endpoint || "").trim()
            return isHttpEndpoint(endpoint) ? endpoint : ""
        }
        if (String(source.kind || "") === "logoscore_cli") {
            return managedBedrockEndpoint()
        }
        if (String(source.kind || "") !== "module" || !usesBasecampModules()) {
            return ""
        }
        return managedBedrockEndpoint()
    }

    function usesBasecampModules() {
        return appModel && typeof appModel.prefersBasecampModules === "function"
            && appModel.prefersBasecampModules() === true
    }

    function managedBedrockEndpoint() {
        // Include the revision in this binding so lifecycle refreshes replace a
        // stale endpoint as soon as the managed Bedrock report changes.
        const revision = basecampLocalNodesRevision
        const report = revision >= 0 && appModel && appModel.localNodesReport
            ? appModel.localNodesReport : null
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : []
        for (let i = 0; i < nodes.length; ++i) {
            const node = nodes[i] || ({})
            if (String(node.key || node.kind || "") !== "bedrock"
                    || ["inspector_managed", "basecamp_host", "local_attached"].indexOf(
                        String(node.ownership || "")) < 0
                    || String(node.run_state || "") !== "running") {
                continue
            }
            const endpoint = String(node.endpoint || "").trim()
            if (isHttpEndpoint(endpoint)) {
                return endpoint
            }
        }
        return ""
    }

    function isHttpEndpoint(endpoint) {
        return /^https?:\/\/[^\s/]+(?:\/|$)/i.test(String(endpoint || "").trim())
    }

    function managedIndexerConfigRequest(allowRetainedControlSnapshot) {
        if (!activeZoneContext || !networkScope || activeZoneId.length === 0) {
            managedIndexerConfigError = qsTr("A Zone context is required to configure this Channel Indexer.")
            return null
        }
        const controlSnapshotUsable = allowRetainedControlSnapshot === true
            ? managedIndexerControlSnapshotUsable === true
            : (verification === "verified" && catalogSnapshotUsable === true)
        if (!controlSnapshotUsable) {
            managedIndexerConfigError = qsTr("A current verified Zone catalog snapshot is required to configure Indexer.")
            return null
        }
        const sourceRevision = Number(activeZoneContext.source_config_revision || 0)
        const selectedSourceId = String(activeZoneContext.selected_sequencer_source_id || "").trim()
        if (!Number.isFinite(sourceRevision) || sourceRevision <= 0
                || selectedSourceId.length === 0) {
            managedIndexerConfigError = qsTr("A current selected Sequencer source is required.")
            return null
        }
        const endpoint = bedrockEndpoint()
        if (!endpoint.length) {
            managedIndexerConfigError = usesBasecampModules()
                && String(sourceDescriptor && sourceDescriptor.kind || "") === "module"
                ? qsTr("Basecamp-managed Bedrock must be running with a valid HTTP endpoint.")
                : qsTr("A Bedrock endpoint is required.")
            return null
        }
        return {
            network_scope: networkScope,
            channel_id: activeZoneId,
            bedrock_endpoint: endpoint,
            source_config_revision: sourceRevision,
            selected_sequencer_source_id: selectedSourceId
        }
    }

    function managedIndexerActionRequiresSourceConfiguration(action) {
        const actionKey = String(action || "")
        return actionKey === "start"
    }

    function managedIndexerConfigContextMatches(generation, scope, channelId, contextRevision) {
        return generation === sourceGeneration
            && scope === networkScopeKey
            && activeZoneContext !== null
            && channelId === activeZoneId
            && contextRevision === ZoneInspectionContract.numericRevision(
                activeZoneContext.context_revision)
    }

    function managedIndexerConfigSnapshotMatches(snapshot, request) {
        return snapshot && String(snapshot.channel_id || "") === String(request.channel_id || "")
            && ZoneInspectionContract.scopeKey(snapshot.network_scope) === networkScopeKey
            && Number(snapshot.source_config_revision || 0)
                === Number(request.source_config_revision || 0)
            && String(snapshot.selected_sequencer_source_id || "")
                === String(request.selected_sequencer_source_id || "")
    }

    function clearManagedIndexerConfig() {
        managedIndexerConfigRequestRevision += 1
        managedIndexerConfigSnapshot = null
        managedIndexerConfigValidation = null
        managedIndexerConfigValidationText = ""
        managedIndexerConfigError = ""
        managedIndexerConfigLoading = false
        managedIndexerConfigSaving = false
        managedIndexerConfigValidationLoading = false
        managedIndexerConfigDraftDirty = false
    }

    function setManagedIndexerConfigDraftDirty(dirty) {
        managedIndexerConfigDraftDirty = dirty === true
    }

    function loadManagedIndexerConfig(callback) {
        if (managedIndexerConfigLoading || managedIndexerConfigSaving
                || managedIndexerControlInFlight) {
            return null
        }
        if (!gateway || typeof gateway.request !== "function") {
            managedIndexerConfigError = qsTr("Inspector bridge is unavailable.")
            return null
        }
        const request = managedIndexerConfigRequest()
        if (!request) {
            return null
        }
        managedIndexerConfigRequestRevision += 1
        const requestRevision = managedIndexerConfigRequestRevision
        const generation = sourceGeneration
        const scope = networkScopeKey
        const channelId = activeZoneId
        const contextRevision = ZoneInspectionContract.numericRevision(
            activeZoneContext.context_revision)
        managedIndexerConfigLoading = true
        managedIndexerConfigError = ""
        return gateway.request("channelIndexerConfig", [
            localNodeProfile(), request
        ], function (response) {
            if (requestRevision !== managedIndexerConfigRequestRevision) {
                return
            }
            managedIndexerConfigLoading = false
            if (!managedIndexerConfigContextMatches(generation, scope, channelId,
                    contextRevision)) {
                return
            }
            if (!response || response.ok !== true || !response.value
                    || !managedIndexerConfigSnapshotMatches(response.value, request)) {
                managedIndexerConfigError = response && response.ok === true
                    ? qsTr("Channel Indexer configuration belongs to stale Zone state.")
                    : ZoneInspectionContract.responseError(response,
                        qsTr("Channel Indexer configuration failed to load."))
                if (callback) {
                    callback(response)
                }
                return
            }
            managedIndexerConfigSnapshot = response.value
            managedIndexerConfigValidation = null
            managedIndexerConfigValidationText = ""
            managedIndexerConfigError = ""
            if (callback) {
                callback(response)
            }
        })
    }

    function validateManagedIndexerConfig(text, callback) {
        if (managedIndexerConfigSaving || !gateway
                || typeof gateway.request !== "function") {
            return null
        }
        const request = managedIndexerConfigRequest()
        if (!request) {
            return null
        }
        const rawText = String(text || "")
        managedIndexerConfigRequestRevision += 1
        const requestRevision = managedIndexerConfigRequestRevision
        const generation = sourceGeneration
        const scope = networkScopeKey
        const channelId = activeZoneId
        const contextRevision = ZoneInspectionContract.numericRevision(
            activeZoneContext.context_revision)
        managedIndexerConfigValidationLoading = true
        managedIndexerConfigError = ""
        return gateway.request("channelIndexerConfigValidate", [
            localNodeProfile(), request, rawText
        ], function (response) {
            if (requestRevision !== managedIndexerConfigRequestRevision) {
                return
            }
            managedIndexerConfigValidationLoading = false
            if (!managedIndexerConfigContextMatches(generation, scope, channelId,
                    contextRevision)) {
                return
            }
            if (!response || response.ok !== true || !response.value) {
                managedIndexerConfigError = ZoneInspectionContract.responseError(response,
                    qsTr("Channel Indexer configuration validation failed."))
                if (callback) {
                    callback(response)
                }
                return
            }
            managedIndexerConfigValidation = response.value
            managedIndexerConfigValidationText = rawText
            managedIndexerConfigError = ""
            if (callback) {
                callback(response)
            }
        })
    }

    function saveManagedIndexerConfig(text, revision, callback) {
        if (managedIndexerConfigLoading || managedIndexerConfigSaving
                || managedIndexerControlInFlight || !gateway
                || typeof gateway.request !== "function") {
            return null
        }
        const request = managedIndexerConfigRequest()
        if (!request) {
            return null
        }
        const rawText = String(text || "")
        const expectedRevision = String(revision || "")
        if (!expectedRevision.length) {
            managedIndexerConfigError = qsTr("Reload Channel Indexer configuration before saving.")
            return null
        }
        managedIndexerConfigRequestRevision += 1
        const requestRevision = managedIndexerConfigRequestRevision
        const generation = sourceGeneration
        const scope = networkScopeKey
        const channelId = activeZoneId
        const contextRevision = ZoneInspectionContract.numericRevision(
            activeZoneContext.context_revision)
        managedIndexerConfigSaving = true
        managedIndexerConfigError = ""
        return gateway.request("channelIndexerConfigSave", [
            localNodeProfile(),
            request,
            rawText,
            expectedRevision,
            ConfirmationPolicy.token("local-node-action")
        ], function (response) {
            if (requestRevision !== managedIndexerConfigRequestRevision) {
                return
            }
            managedIndexerConfigSaving = false
            if (!managedIndexerConfigContextMatches(generation, scope, channelId,
                    contextRevision)) {
                return
            }
            if (!response || response.ok !== true || !response.value
                    || !managedIndexerConfigSnapshotMatches(response.value, request)) {
                managedIndexerConfigError = response && response.ok === true
                    ? qsTr("Saved Channel Indexer configuration belongs to stale Zone state.")
                    : ZoneInspectionContract.responseError(response,
                        qsTr("Channel Indexer configuration failed to save."))
                if (callback) {
                    callback(response)
                }
                return
            }
            managedIndexerConfigSnapshot = response.value
            managedIndexerConfigValidation = null
            managedIndexerConfigValidationText = ""
            managedIndexerConfigError = ""
            if (callback) {
                callback(response)
            }
        })
    }

    function acceptManagedIndexerReport(report) {
        managedIndexerReport = report || null
        managedIndexerRuntime = report && report.runtime ? report.runtime : null
        managedIndexerNode = null
        const nodes = report && Array.isArray(report.nodes) ? report.nodes : []
        for (let i = 0; i < nodes.length; ++i) {
            if (String(nodes[i] && (nodes[i].key || nodes[i].kind) || "") === "indexer") {
                managedIndexerNode = nodes[i]
                break
            }
        }
        managedIndexerStatusStale = false
    }

    function refreshManagedIndexer(callback) {
        if (managedIndexerRefreshInFlight || managedIndexerControlInFlight
                || managedIndexerConfigLoading || managedIndexerConfigSaving) {
            return null
        }
        if (!gateway || typeof gateway.request !== "function") {
            managedIndexerStatusStale = true
            managedIndexerError = qsTr("Inspector bridge is unavailable.")
            return null
        }
        if (!activeZoneContext || !networkScope || activeZoneId.length === 0) {
            managedIndexerStatusStale = true
            managedIndexerError = qsTr("A Zone context is required to inspect this Channel Indexer.")
            return null
        }
        managedIndexerRequestRevision += 1
        const requestRevision = managedIndexerRequestRevision
        managedIndexerRefreshInFlight = true
        return gateway.request("channelIndexerStatus", [
            localNodeProfile(), networkScope, activeZoneId
        ], function (response) {
            if (requestRevision !== managedIndexerRequestRevision) {
                return
            }
            managedIndexerRefreshInFlight = false
            if (!response || response.ok !== true || !response.value) {
                managedIndexerStatusStale = true
                managedIndexerError = ZoneInspectionContract.responseError(
                    response, qsTr("Managed Indexer status failed."))
                if (callback) {
                    callback(response)
                }
                return
            }
            acceptManagedIndexerReport(response.value)
            managedIndexerError = ""
            managedIndexerResult = ""
            if (callback) {
                callback(response)
            }
        })
    }

    function managedIndexerOperation(report, action) {
        const operations = report && Array.isArray(report.operations)
            ? report.operations : []
        for (let i = operations.length - 1; i >= 0; --i) {
            const operation = operations[i] || ({})
            if (String(operation.node || "") === "indexer"
                    && String(operation.action || "") === String(action || "")) {
                return operation
            }
        }
        return null
    }

    function clearManagedIndexerRuntimeOperation() {
        managedIndexerRuntimeOperation = null
        managedIndexerRuntimeAction = ""
        managedIndexerRuntimeCallback = null
        managedIndexerRuntimePollInFlight = false
        managedIndexerRuntimePollToken += 1
        managedIndexerRuntimePollError = ""
    }

    function managedIndexerRuntimeRequest(action, request) {
        return {
            domain: "localNodes",
            method: "channelIndexerAction",
            label: managedIndexerActionLabel(action),
            args: [
                localNodeProfile(),
                request,
                ConfirmationPolicy.token("local-node-action")
            ]
        }
    }

    function managedIndexerActionLabel(action) {
        switch (String(action || "")) {
        case "start":
            return qsTr("Start Channel Indexer")
        case "stop":
            return qsTr("Stop Channel Indexer")
        case "purge":
            return qsTr("Purge Channel Indexer")
        default:
            return qsTr("Channel Indexer action")
        }
    }

    function managedIndexerRuntimeMatches(operation, operationId) {
        const value = operation && typeof operation === "object" ? operation : null
        return value !== null
            && String(value.operationId || "").length > 0
            && String(value.operationId || "") === String(operationId || "")
            && String(value.domain || "") === "localNodes"
            && String(value.method || "") === "channelIndexerAction"
    }

    function managedIndexerRuntimeResponse(operation) {
        if (appModel && typeof appModel.runtimeOperationResponse === "function") {
            return appModel.runtimeOperationResponse(operation)
        }
        const successful = OperationHistoryVocabulary.isRuntimeSuccessfulTerminalStatus(
            operation && operation.status)
        return {
            ok: successful,
            value: operation && operation.result !== undefined
                && operation.result !== null ? operation.result : operation,
            text: "",
            error: successful ? "" : String(operation && operation.error || "")
        }
    }

    function finishManagedIndexerRuntimeOperation(operation) {
        const action = managedIndexerRuntimeAction
        const callback = managedIndexerRuntimeCallback
        clearManagedIndexerRuntimeOperation()
        const response = managedIndexerRuntimeResponse(operation)
        completeManagedIndexerAction(action, response, callback)
    }

    function completeManagedIndexerAction(action, response, callback) {
        managedIndexerControlInFlight = false
        managedIndexerRuntimePollError = ""
        if (!response || response.ok !== true || !response.value) {
            managedIndexerStatusStale = true
            managedIndexerError = ZoneInspectionContract.responseError(
                response, qsTr("Managed Indexer action failed."))
            if (callback) {
                callback(response)
            }
            return
        }
        acceptManagedIndexerReport(response.value)
        const operation = managedIndexerOperation(response.value, action)
        const status = String(operation && operation.status || "")
        const detail = String(operation && operation.detail || "")
        if (status === "failed" || status === "needs_configuration") {
            managedIndexerError = detail.length
                ? detail : qsTr("Managed Indexer action failed.")
        } else {
            managedIndexerError = ""
            managedIndexerResult = detail.length ? detail : status
            managedIndexerLifecycleChanged()
        }
        if (callback) {
            callback(response)
        }
    }

    function startManagedIndexerRuntimeOperation(action, request, requestRevision,
            callback) {
        if (!gateway || typeof gateway.startRuntimeOperation !== "function") {
            managedIndexerControlInFlight = false
            managedIndexerStatusStale = true
            managedIndexerError = qsTr("Runtime operation bridge is unavailable.")
            if (callback) {
                callback(ZoneInspectionContract.failedResponse(managedIndexerError))
            }
            return null
        }
        return gateway.startRuntimeOperation(
            managedIndexerRuntimeRequest(action, request),
            false,
            function (response) {
                if (requestRevision !== managedIndexerRequestRevision) {
                    return
                }
                if (!response || response.ok !== true || !response.value) {
                    completeManagedIndexerAction(action, response, callback)
                    return
                }
                const operation = response.value
                const operationId = String(operation && operation.operationId || "")
                if (!managedIndexerRuntimeMatches(operation, operationId)) {
                    completeManagedIndexerAction(action,
                        ZoneInspectionContract.failedResponse(qsTr("Managed Indexer action returned an invalid runtime operation.")),
                        callback)
                    return
                }
                if (OperationHistoryVocabulary.isRuntimeTerminalStatus(operation.status)) {
                    completeManagedIndexerAction(action,
                        managedIndexerRuntimeResponse(operation), callback)
                    return
                }
                if (!OperationHistoryVocabulary.isRuntimeActiveStatus(operation.status)) {
                    completeManagedIndexerAction(action,
                        ZoneInspectionContract.failedResponse(qsTr("Managed Indexer action returned an invalid runtime status.")),
                        callback)
                    return
                }
                managedIndexerRuntimeOperation = operation
                managedIndexerRuntimeAction = action
                managedIndexerRuntimeCallback = callback
            }
        )
    }

    function pollManagedIndexerOperation() {
        const operation = managedIndexerRuntimeOperation
        const operationId = String(operation && operation.operationId || "")
        if (!managedIndexerOperationRunning || managedIndexerRuntimePollInFlight
                || !operationId.length || !gateway
                || typeof gateway.runtimeOperationStatus !== "function") {
            return null
        }
        managedIndexerRuntimePollToken += 1
        const pollToken = managedIndexerRuntimePollToken
        const requestRevision = managedIndexerRequestRevision
        managedIndexerRuntimePollInFlight = true
        return gateway.runtimeOperationStatus(operationId, false, function (response) {
            if (requestRevision !== managedIndexerRequestRevision
                    || pollToken !== managedIndexerRuntimePollToken
                    || !managedIndexerRuntimeMatches(managedIndexerRuntimeOperation,
                        operationId)) {
                return
            }
            managedIndexerRuntimePollInFlight = false
            if (!response || response.ok !== true || !response.value) {
                managedIndexerRuntimePollError = ZoneInspectionContract.responseError(
                    response, qsTr("Managed Indexer action status is unavailable; retrying."))
                return
            }
            if (!managedIndexerRuntimeMatches(response.value, operationId)) {
                managedIndexerRuntimePollError = qsTr("Managed Indexer action status did not match the active operation; retrying.")
                return
            }
            if (!OperationHistoryVocabulary.runtimeSnapshotIsNewer(
                    managedIndexerRuntimeOperation, response.value)) {
                return
            }
            if (OperationHistoryVocabulary.isRuntimeTerminalStatus(response.value.status)) {
                finishManagedIndexerRuntimeOperation(response.value)
                return
            }
            if (OperationHistoryVocabulary.isRuntimeActiveStatus(response.value.status)) {
                managedIndexerRuntimeOperation = response.value
                managedIndexerRuntimePollError = ""
                return
            }
            managedIndexerRuntimePollError = qsTr("Managed Indexer action returned an invalid runtime status; retrying.")
        })
    }

    function runManagedIndexerAction(action, channelId, callback) {
        if (managedIndexerControlInFlight || managedIndexerRefreshInFlight
                || managedIndexerConfigLoading || managedIndexerConfigSaving) {
            return null
        }
        if (managedIndexerConfigDraftDirty) {
            managedIndexerError = qsTr("Save or undo Channel Indexer configuration changes before controlling Indexer.")
            return null
        }
        const actionKey = String(action || "")
        if (actionKey !== "start" && actionKey !== "stop" && actionKey !== "purge") {
            managedIndexerError = qsTr("Unsupported managed Indexer action.")
            return null
        }
        if (managedIndexerStatusStale) {
            managedIndexerError = qsTr("Refresh managed Indexer status before controlling it.")
            return null
        }
        const availableActions = managedIndexerNode
            && Array.isArray(managedIndexerNode.available_actions)
            ? managedIndexerNode.available_actions : []
        if (availableActions.indexOf(actionKey) < 0) {
            managedIndexerError = qsTr("Managed Indexer %1 is not currently available.")
                .arg(actionKey)
            return null
        }
        const requiresSourceConfiguration = managedIndexerActionRequiresSourceConfiguration(actionKey)
        const controlSnapshotUsable = requiresSourceConfiguration
            && managedIndexerControlSnapshotUsable === true
        if (requiresSourceConfiguration && (!activeZoneContext
                || (verification !== "verified" || catalogSnapshotUsable !== true)
                && !controlSnapshotUsable)) {
            managedIndexerError = actionKey === "purge"
                ? qsTr("A current verified Zone catalog snapshot is required to reset Indexer data.")
                : qsTr("A current verified Zone catalog snapshot is required to start Indexer.")
            return null
        }
        const targetChannel = String(channelId || activeZoneId).trim()
        if (!targetChannel.length) {
            managedIndexerError = qsTr("A Channel ID is required.")
            return null
        }
        const request = {
            action: actionKey,
            network_scope: networkScope,
            channel_id: targetChannel
        }
        if (requiresSourceConfiguration) {
            const configRequest = managedIndexerConfigRequest(true)
            if (!configRequest) {
                managedIndexerError = managedIndexerConfigError
                return null
            }
            if (targetChannel !== configRequest.channel_id) {
                managedIndexerError = actionKey === "purge"
                    ? qsTr("Reset Indexer data only for the active Zone.")
                    : qsTr("Start Indexer only for the active Zone.")
                return null
            }
            request.bedrock_endpoint = configRequest.bedrock_endpoint
            request.source_config_revision = configRequest.source_config_revision
            request.selected_sequencer_source_id = configRequest.selected_sequencer_source_id
        }
        managedIndexerRequestRevision += 1
        const requestRevision = managedIndexerRequestRevision
        managedIndexerControlInFlight = true
        managedIndexerError = ""
        managedIndexerResult = ""
        managedIndexerRuntimePollError = ""
        if (usesBasecampModules()) {
            return startManagedIndexerRuntimeOperation(actionKey, request,
                requestRevision, callback)
        }
        return gateway.request("channelIndexerAction", [
            localNodeProfile(),
            request,
            ConfirmationPolicy.token("local-node-action")
        ], function (response) {
            if (requestRevision !== managedIndexerRequestRevision) {
                return
            }
            completeManagedIndexerAction(actionKey, response, callback)
        })
    }

    function applyChannelSourceConfig(request, callback) {
        if (sourceMutationInFlight) {
            const busyResponse = ZoneInspectionContract.failedResponse(qsTr("Another Channel source edit is still running."))
            if (callback) {
                callback(busyResponse)
            }
            return null
        }
        if (!activeZoneContext || verification !== "verified"
                || catalogSnapshotUsable !== true) {
            const inactiveResponse = ZoneInspectionContract.failedResponse(qsTr("A current verified Zone catalog snapshot is required."))
            if (callback) {
                callback(inactiveResponse)
            }
            return null
        }

        const typedRequest = ZoneInspectionContract.copyObject(request)
        typedRequest.network_scope = networkScope
        typedRequest.channel_id = activeZoneId
        sourceMutationRequestRevision += 1
        const requestRevision = sourceMutationRequestRevision
        const generation = sourceGeneration
        const requestedContextRevision = ZoneInspectionContract.numericRevision(activeZoneContext.context_revision)
        const channelId = activeZoneId
        const scope = networkScopeKey
        sourceMutationInFlight = true
        sourceMutationError = ""
        sourceMutationWarning = null
        return ZoneInspectionContract.dispatch(gateway, "channelSourceConfigApply", typedRequest, function (response) {
            if (requestRevision !== sourceMutationRequestRevision) {
                return
            }
            sourceMutationInFlight = false
            if (generation !== sourceGeneration || scope !== networkScopeKey
                    || !activeZoneContext || channelId !== activeZoneId
                    || requestedContextRevision !== ZoneInspectionContract.numericRevision(activeZoneContext.context_revision)) {
                if (callback) {
                    callback(response)
                }
                return
            }
            if (!ZoneInspectionContract.validReportResponse(response, "zones.channel_source_config")) {
                sourceMutationError = ZoneInspectionContract.responseError(response, qsTr("Channel source update failed."))
                if (callback) {
                    callback(response)
                }
                sourceMutationFinished(response)
                return
            }
            const report = response.value
            if (ZoneInspectionContract.numericRevision(report.source_revision) !== sourceRevision
                    || !report.config
                    || String(report.config.channel_id || "") !== channelId
                    || ZoneInspectionContract.scopeKey(report.config.network_scope) !== networkScopeKey) {
                sourceMutationError = qsTr("Channel source update belongs to stale Zone state.")
                const staleResponse = ZoneInspectionContract.failedResponse(sourceMutationError)
                if (callback) {
                    callback(staleResponse)
                }
                sourceMutationFinished(staleResponse)
                return
            }

            const warning = report.attestation_warning || null
            sourceMutationAccepted(report)
            // Accepting a same-Zone config revision refreshes active context
            // synchronously and resets this state. Publish its receipt warning
            // after that refresh so the user can see the trust decision.
            sourceMutationWarning = warning
            if (callback) {
                callback(response)
            }
            sourceMutationFinished(response)
        })
    }

    function reloadChannelSourceConfig(callback) {
        if (sourceMutationInFlight) {
            const busyResponse = ZoneInspectionContract.failedResponse(qsTr("Another Channel source edit is still running."))
            if (callback) {
                callback(busyResponse)
            }
            return null
        }
        if (!activeZoneContext || verification !== "verified"
                || catalogSnapshotUsable !== true) {
            const inactiveResponse = ZoneInspectionContract.failedResponse(qsTr("A current verified Zone catalog snapshot is required."))
            if (callback) {
                callback(inactiveResponse)
            }
            return null
        }

        sourceMutationRequestRevision += 1
        const requestRevision = sourceMutationRequestRevision
        const generation = sourceGeneration
        const requestedContextRevision = ZoneInspectionContract.numericRevision(activeZoneContext.context_revision)
        const channelId = activeZoneId
        const scope = networkScopeKey
        sourceMutationInFlight = true
        sourceMutationError = ""
        return ZoneInspectionContract.dispatch(gateway, "channelSourceConfigCurrent", {
            network_scope: networkScope,
            channel_id: channelId
        }, function (response) {
            if (requestRevision !== sourceMutationRequestRevision) {
                return
            }
            sourceMutationInFlight = false
            if (generation !== sourceGeneration || scope !== networkScopeKey
                    || !activeZoneContext || channelId !== activeZoneId
                    || requestedContextRevision !== ZoneInspectionContract.numericRevision(activeZoneContext.context_revision)) {
                const staleResponse = ZoneInspectionContract.failedResponse(
                    qsTr("Channel source reload belongs to stale Zone state."))
                sourceMutationError = staleResponse.error
                if (callback) {
                    callback(staleResponse)
                }
                return
            }
            if (!ZoneInspectionContract.validReportResponse(response,
                    "zones.channel_source_config_current")) {
                sourceMutationError = ZoneInspectionContract.responseError(response,
                    qsTr("Channel source reload failed."))
                if (callback) {
                    callback(response)
                }
                return
            }
            const report = response.value
            if (ZoneInspectionContract.numericRevision(report.source_revision) !== sourceRevision
                    || String(report.channel_id || "") !== channelId
                    || ZoneInspectionContract.scopeKey(report.network_scope) !== networkScopeKey
                    || !report.config
                    || String(report.config.channel_id || "") !== channelId
                    || ZoneInspectionContract.scopeKey(report.config.network_scope) !== networkScopeKey) {
                sourceMutationError = qsTr("Channel source reload belongs to stale Zone state.")
                const staleResponse = ZoneInspectionContract.failedResponse(sourceMutationError)
                if (callback) {
                    callback(staleResponse)
                }
                return
            }
            sourceMutationError = ""
            if (callback) {
                callback(response)
            }
        })
    }

}
