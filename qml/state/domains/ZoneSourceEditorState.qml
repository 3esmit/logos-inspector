import QtQml
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

    readonly property string activeZoneId: activeZoneContext
        ? String(activeZoneContext.channel_id || "") : ""

    property string sourceMutationError: ""
    property var sourceMutationWarning: null
    property bool sourceMutationInFlight: false
    property int sourceMutationRequestRevision: 0

    signal sourceMutationFinished(var response)
    signal sourceMutationAccepted(var report)

    function resetSourceEditorState() {
        sourceMutationRequestRevision += 1
        sourceMutationInFlight = false
        sourceMutationError = ""
        sourceMutationWarning = null
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

            sourceMutationWarning = report.attestation_warning || null
            sourceMutationAccepted(report)
            if (callback) {
                callback(response)
            }
            sourceMutationFinished(response)
        })
    }

}
