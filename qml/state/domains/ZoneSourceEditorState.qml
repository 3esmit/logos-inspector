import QtQml
import "../ConfirmationPolicy.js" as ConfirmationPolicy
import "ZoneInspectionContract.js" as ZoneInspectionContract

QtObject {
    id: root

    required property var gateway
    required property var activeZoneContext
    required property string verification
    required property var networkScope
    required property string networkScopeKey
    required property int sourceGeneration
    required property double sourceRevision
    property var appModel: null
    property var sourceDescriptor: null

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

    signal sourceMutationFinished(var response)
    signal sourceMutationAccepted(var report)

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
        managedIndexerStatusStale = true
        managedIndexerError = ""
        managedIndexerResult = ""
    }

    function localNodeProfile() {
        const profile = String(appModel && appModel.networkProfile || "default")
            .trim().toLowerCase()
        return profile === "local" ? "local" : "default"
    }

    function bedrockEndpoint() {
        const source = sourceDescriptor || null
        if (!source || String(source.kind || "") !== "direct_http") {
            return ""
        }
        return String(source.endpoint || "").trim()
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
        if (managedIndexerRefreshInFlight || managedIndexerControlInFlight) {
            return null
        }
        if (!gateway || typeof gateway.request !== "function") {
            managedIndexerStatusStale = true
            managedIndexerError = qsTr("Inspector bridge is unavailable.")
            return null
        }
        managedIndexerRequestRevision += 1
        const requestRevision = managedIndexerRequestRevision
        managedIndexerRefreshInFlight = true
        return gateway.request("localNodesStatus", [localNodeProfile()], function (response) {
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

    function runManagedIndexerAction(action, channelId, callback) {
        if (managedIndexerControlInFlight || managedIndexerRefreshInFlight) {
            return null
        }
        const actionKey = String(action || "")
        if (actionKey !== "start" && actionKey !== "stop") {
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
        if (actionKey === "start"
                && (!activeZoneContext || verification !== "verified")) {
            managedIndexerError = qsTr("A verified active Zone is required to start Indexer.")
            return null
        }
        const targetChannel = String(channelId || activeZoneId).trim()
        if (!targetChannel.length) {
            managedIndexerError = qsTr("A Channel ID is required.")
            return null
        }
        const request = {
            action: actionKey,
            node: "indexer",
            channel_id: targetChannel
        }
        if (actionKey === "start") {
            const endpoint = bedrockEndpoint()
            if (!endpoint.length) {
                managedIndexerError = qsTr("A Bedrock endpoint is required.")
                return null
            }
            request.bedrock_endpoint = endpoint
        }
        managedIndexerRequestRevision += 1
        const requestRevision = managedIndexerRequestRevision
        managedIndexerControlInFlight = true
        managedIndexerError = ""
        managedIndexerResult = ""
        return gateway.request("localNodesAction", [
            localNodeProfile(),
            request,
            ConfirmationPolicy.token("local-node-action")
        ], function (response) {
            if (requestRevision !== managedIndexerRequestRevision) {
                return
            }
            managedIndexerControlInFlight = false
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
            const operation = managedIndexerOperation(response.value, actionKey)
            const status = String(operation && operation.status || "")
            const detail = String(operation && operation.detail || "")
            if (status === "failed" || status === "needs_configuration") {
                managedIndexerError = detail.length
                    ? detail : qsTr("Managed Indexer action failed.")
            } else {
                managedIndexerError = ""
                managedIndexerResult = detail.length ? detail : status
            }
            if (callback) {
                callback(response)
            }
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
        if (!activeZoneContext || verification !== "verified") {
            const inactiveResponse = ZoneInspectionContract.failedResponse(qsTr("A verified active Zone is required."))
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

}
