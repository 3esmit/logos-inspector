import QtQml
import "ZoneInspectionContract.js" as ZoneInspectionContract

QtObject {
    id: root

    required property var gateway
    required property var activeZoneContext
    required property string verification
    property var appModel: null
    readonly property bool l2Applicable: activeZoneContext !== null
        && String(activeZoneContext.zone_kind || "") === "sequencer_zone"
    readonly property bool l2SourceConfigured: activeZoneContext !== null
        && (String(activeZoneContext.indexer_source_id || "").length > 0
            || String(activeZoneContext.selected_sequencer_source_id || "").length > 0)
    readonly property bool l2ReadEnabled: verification === "verified"
        && l2Applicable && l2SourceConfigured
    readonly property bool l2IndexerReadEnabled: l2ReadEnabled
        && String(activeZoneContext && activeZoneContext.indexer_source_id || "").length > 0
    readonly property bool l2SequencerReadEnabled: l2ReadEnabled
        && String(activeZoneContext
            && activeZoneContext.selected_sequencer_source_id || "").length > 0

    readonly property ZoneL2BlockTransactionState blocks: ZoneL2BlockTransactionState {
        l2Context: root
    }
    readonly property ZoneL2AccountState accounts: ZoneL2AccountState {
        l2Context: root
    }
    readonly property ZoneL2ProgramTransferState tools: ZoneL2ProgramTransferState {
        l2Context: root
    }

    function dispatch(method, request, callback) {
        return ZoneInspectionContract.dispatch(gateway, method, request, callback)
    }

    function numericRevision(value) {
        return ZoneInspectionContract.numericRevision(value)
    }

    function responseError(response, fallback) {
        return ZoneInspectionContract.responseError(response, fallback)
    }

    function validReportResponse(response, reportKind) {
        return ZoneInspectionContract.validReportResponse(response, reportKind)
    }

    function sameContext(left, right) {
        return ZoneInspectionContract.sameContext(left, right)
    }

    function scopeKey(scope) {
        return ZoneInspectionContract.scopeKey(scope)
    }

    function resetL2InspectionState() {
        blocks.resetL2BlocksState(true)
        blocks.resetL2BlockInspectionState()
        accounts.resetL2AccountState(true)
        tools.resetL2ProgramsState()
        tools.resetL2CommitmentProofState()
        tools.resetL2AccountNoncesState()
        tools.resetL2TransfersState(true)
    }

    function l2IndexerSourceId() {
        return String(activeZoneContext && activeZoneContext.indexer_source_id || "")
    }

    function l2SequencerSourceId() {
        return String(activeZoneContext
            && activeZoneContext.selected_sequencer_source_id || "")
    }

    function validL2SingleSourceValue(value, exactSourceId, expectedRole) {
        const source = value && value.source ? value.source : null
        if (!source || String(source.source_role || "") !== String(expectedRole || "")) {
            return false
        }
        const sourceId = String(exactSourceId || "")
        return sourceId.length === 0 || String(source.source_id || "") === sourceId
    }

    function l2RequestContext() {
        if (!activeZoneContext) {
            return null
        }
        return {
            network_scope: activeZoneContext.network_scope,
            channel_id: String(activeZoneContext.channel_id || ""),
            zone_kind: String(activeZoneContext.zone_kind || "unknown"),
            selected_sequencer_source_id: activeZoneContext.selected_sequencer_source_id
                ? String(activeZoneContext.selected_sequencer_source_id) : null,
            indexer_source_id: activeZoneContext.indexer_source_id
                ? String(activeZoneContext.indexer_source_id) : null,
            source_config_revision: numericRevision(activeZoneContext.source_config_revision),
            context_revision: numericRevision(activeZoneContext.context_revision)
        }
    }

    function l2EntityRef(entityKind, canonicalKey, sourceObservation) {
        if (!activeZoneContext) {
            return null
        }
        const key = String(canonicalKey || "").trim()
        if (key.length === 0) {
            return null
        }
        const sourceId = String(sourceObservation && sourceObservation.source_id || "")
        const sourceRole = String(sourceObservation && sourceObservation.source_role || "")
        return {
            network_scope: activeZoneContext.network_scope,
            channel_id: String(activeZoneContext.channel_id || ""),
            zone_kind: String(activeZoneContext.zone_kind || "unknown"),
            entity_kind: String(entityKind || ""),
            canonical_key: key,
            source: sourceId.length > 0 && sourceRole.length > 0 ? {
                kind: "exact",
                source_id: sourceId,
                source_role: sourceRole
            } : { kind: "policy" }
        }
    }

    function l2RequestContextIsCurrent(context) {
        return activeZoneContext !== null && sameFullL2Context(context, activeZoneContext)
    }

    function sameFullL2Context(left, right) {
        return sameContext(left, right)
            && scopeKey(left && left.network_scope) === scopeKey(right && right.network_scope)
            && numericRevision(left && left.context_revision)
                === numericRevision(right && right.context_revision)
    }

    function validL2ReportResponse(response, reportKind, requestRevision) {
        return validReportResponse(response, reportKind)
            && numericRevision(response.value.request_revision) === requestRevision
            && l2RequestContextIsCurrent(response.value.context)
    }

    function acceptedL2Failure(response, requestContext, requestRevision) {
        if (!response || response.ok !== false) {
            return false
        }
        const details = response && response.error_details
            && typeof response.error_details === "object"
            ? response.error_details : null
        if (!details) {
            return true
        }
        return String(details.report_kind || "") === "lez.read_error"
            && Number(details.schema_version || 0) === 1
            && numericRevision(details.request_revision) === requestRevision
            && sameFullL2Context(details.context, requestContext)
            && l2RequestContextIsCurrent(details.context)
    }

    function l2AvailabilityMessage() {
        if (!activeZoneContext) {
            return qsTr("Select a verified Zone to inspect L2 data.")
        }
        if (!l2Applicable) {
            return qsTr("L2 reads do not apply to this Channel type.")
        }
        if (!l2SourceConfigured) {
            return qsTr("Configure an Indexer or select a Sequencer source for this Zone.")
        }
        return qsTr("Zone verification is required before reading L2 data.")
    }

    function l2Capability(sourceRole) {
        const role = String(sourceRole || "")
        if (!activeZoneContext) {
            return capability(false, "input_required", "select_zone",
                qsTr("Select an Active Zone."))
        }
        if (verification !== "verified") {
            return capability(false, "disabled", "refresh_context",
                qsTr("Zone catalog verification is required."))
        }
        if (!l2Applicable) {
            return capability(false, "disabled", "none",
                qsTr("Active Zone has no Sequencer L2."))
        }
        if (role === "indexer" && !l2IndexerReadEnabled) {
            return capability(false, "input_required", "configure_source",
                qsTr("Configure this Channel's Indexer."))
        }
        if (role === "sequencer" && !l2SequencerReadEnabled) {
            return capability(false, "input_required", "select_source",
                qsTr("Select this Channel's Sequencer."))
        }
        if (!l2ReadEnabled) {
            return capability(false, "input_required", "configure_source",
                qsTr("Configure an L2 source owned by this Channel."))
        }
        return capability(true, "enabled", "none", "")
    }

    function collaborationCapability() {
        const read = l2Capability("")
        if (!read.enabled) {
            return read
        }
        const scope = activeZoneContext && activeZoneContext.network_scope
        if (!scope || String(scope.kind || "") !== "genesis_id") {
            return capability(false, "disabled", "refresh_context",
                qsTr("Verified genesis network identity is required for Zone collaboration."))
        }
        return capability(true, "enabled", "none", "")
    }

    function capability(enabled, status, recovery, reason) {
        return {
            enabled: enabled === true,
            status: String(status || "disabled"),
            recovery: String(recovery || "none"),
            reason: String(reason || ""),
            provenance: ["active_zone_context"]
        }
    }
}
