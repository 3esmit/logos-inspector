import QtQml
import "ZoneInspectionContract.js" as ZoneInspectionContract

QtObject {
    id: root

    required property var gateway
    required property var activeZoneContext
    required property string verification
    required property int sourceGeneration
    required property double sourceRevision
    required property var networkScope
    required property string networkScopeKey
    required property double catalogRevision

    readonly property string activeZoneId: activeZoneContext
        ? String(activeZoneContext.channel_id || "") : ""

    property string evidenceError: ""
    property string evidenceDetailError: ""
    property string evidencePayloadError: ""

    property string evidenceFilter: "all"
    property var evidenceRows: []
    property string evidenceNextCursor: ""
    property bool evidenceLoaded: false
    property var evidencePageContext: null
    property var selectedEvidenceRow: null
    property var evidenceDetail: null
    property var evidencePayloadChunks: []
    property double evidencePayloadOffset: 0
    property bool evidencePayloadDone: true

    property bool evidenceInFlight: false
    property bool evidenceDetailInFlight: false
    property bool evidencePayloadInFlight: false
    property int evidenceRequestRevision: 0
    property int evidenceDetailRequestRevision: 0
    property int evidencePayloadRequestRevision: 0

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

    function scopeKey(scope) {
        return ZoneInspectionContract.scopeKey(scope)
    }

    function loadEvidence(filter) {
        const normalizedFilter = normalizedEvidenceFilter(filter)
        if (!activeZoneContext || verification !== "verified") {
            evidenceError = qsTr("A verified active Zone is required.")
            return false
        }
        if (evidenceInFlight) {
            return false
        }
        resetEvidenceState(true)
        evidenceFilter = normalizedFilter
        evidencePageContext = {
            source_generation: sourceGeneration,
            source_revision: sourceRevision,
            network_scope: networkScope,
            network_scope_key: networkScopeKey,
            catalog_revision: catalogRevision,
            channel_id: activeZoneId,
            context_revision: numericRevision(activeZoneContext.context_revision),
            filter: normalizedFilter
        }
        evidenceLoaded = false
        evidenceError = ""
        requestEvidencePage("")
        return true
    }

    function loadMoreEvidence() {
        if (!evidencePageContext || evidenceInFlight || evidenceNextCursor.length === 0) {
            return false
        }
        requestEvidencePage(evidenceNextCursor)
        return true
    }

    function requestEvidencePage(cursor) {
        const pageContext = evidencePageContext
        if (!pageContext || evidenceInFlight) {
            return null
        }
        evidenceRequestRevision += 1
        const requestRevision = evidenceRequestRevision
        const cursorText = String(cursor || "")
        evidenceInFlight = true
        evidenceError = ""
        return dispatch("zoneEvidencePage", {
            source_revision: pageContext.source_revision,
            network_scope: pageContext.network_scope,
            catalog_revision: pageContext.catalog_revision,
            channel_id: pageContext.channel_id,
            filter: pageContext.filter,
            cursor: cursorText.length > 0 ? cursorText : null,
            limit: 25
        }, function (response) {
            if (requestRevision !== evidenceRequestRevision) {
                return
            }
            evidenceInFlight = false
            if (!evidenceContextIsCurrent(pageContext)) {
                return
            }
            if (!validReportResponse(response, "zones.evidence_page")) {
                evidenceError = responseError(response, qsTr("L1 evidence failed."))
                return
            }
            const report = response.value
            if (numericRevision(report.source_revision) !== pageContext.source_revision
                    || scopeKey(report.network_scope) !== pageContext.network_scope_key
                    || numericRevision(report.catalog_revision) !== pageContext.catalog_revision
                    || String(report.channel_id || "") !== pageContext.channel_id
                    || String(report.filter || "") !== pageContext.filter
                    || !Array.isArray(report.rows)) {
                evidenceError = qsTr("L1 evidence belongs to stale Zone state.")
                return
            }
            evidenceRows = cursorText.length > 0
                ? appendUniqueEvidenceRows(evidenceRows, report.rows)
                : report.rows.slice()
            evidenceNextCursor = String(report.next_cursor || "")
            evidenceLoaded = true
        })
    }

    function openEvidence(row) {
        if (!row || !row.reference || !activeZoneContext
                || verification !== "verified" || evidenceDetailInFlight) {
            return false
        }
        resetEvidenceDetail(true)
        evidenceDetailRequestRevision += 1
        const requestRevision = evidenceDetailRequestRevision
        const generation = sourceGeneration
        const requestedContextRevision = numericRevision(activeZoneContext.context_revision)
        const channelId = activeZoneId
        const evidenceId = String(row.reference.evidence_id || "")
        selectedEvidenceRow = row
        evidenceDetailInFlight = true
        evidenceDetailError = ""
        dispatch("zoneEvidenceDetail", {
            source_revision: sourceRevision,
            network_scope: networkScope,
            catalog_revision: catalogRevision,
            channel_id: channelId,
            reference: row.reference
        }, function (response) {
            if (requestRevision !== evidenceDetailRequestRevision) {
                return
            }
            evidenceDetailInFlight = false
            if (generation !== sourceGeneration || !activeZoneContext
                    || channelId !== activeZoneId
                    || requestedContextRevision !== numericRevision(activeZoneContext.context_revision)) {
                return
            }
            if (!validReportResponse(response, "zones.evidence_detail")) {
                evidenceDetailError = responseError(response, qsTr("L1 evidence detail failed."))
                return
            }
            const report = response.value
            if (numericRevision(report.source_revision) !== sourceRevision
                    || scopeKey(report.network_scope) !== networkScopeKey
                    || numericRevision(report.catalog_revision) !== catalogRevision
                    || String(report.channel_id || "") !== channelId
                    || String(report.row && report.row.reference && report.row.reference.evidence_id || "") !== evidenceId) {
                evidenceDetailError = qsTr("L1 evidence detail belongs to stale Zone state.")
                return
            }
            evidenceDetail = report
            evidencePayloadChunks = []
            evidencePayloadOffset = 0
            evidencePayloadDone = !(report.payload && String(report.payload.session_id || "").length > 0)
            evidenceDetailError = ""
        })
        return true
    }

    function loadNextEvidencePayloadChunk() {
        const payload = evidenceDetail && evidenceDetail.payload ? evidenceDetail.payload : null
        const row = evidenceDetail && evidenceDetail.row ? evidenceDetail.row : null
        const reference = row && row.reference ? row.reference : null
        const sessionId = String(payload && payload.session_id || "")
        if (!activeZoneContext || !reference || sessionId.length === 0
                || evidencePayloadDone || evidencePayloadInFlight) {
            return false
        }
        evidencePayloadRequestRevision += 1
        const requestRevision = evidencePayloadRequestRevision
        const generation = sourceGeneration
        const requestedContextRevision = numericRevision(activeZoneContext.context_revision)
        const channelId = activeZoneId
        const evidenceId = String(reference.evidence_id || "")
        const offset = evidencePayloadOffset
        evidencePayloadInFlight = true
        evidencePayloadError = ""
        dispatch("zoneEvidencePayloadChunk", {
            source_revision: sourceRevision,
            network_scope: networkScope,
            channel_id: channelId,
            evidence_id: evidenceId,
            session_id: sessionId,
            offset: offset,
            limit: 65536
        }, function (response) {
            if (requestRevision !== evidencePayloadRequestRevision) {
                return
            }
            evidencePayloadInFlight = false
            if (generation !== sourceGeneration || !activeZoneContext
                    || channelId !== activeZoneId
                    || requestedContextRevision !== numericRevision(activeZoneContext.context_revision)) {
                return
            }
            if (!validReportResponse(response, "zones.evidence_payload_chunk")) {
                evidencePayloadError = responseError(response, qsTr("Evidence payload chunk failed."))
                return
            }
            const report = response.value
            if (String(report.session_id || "") !== sessionId
                    || String(report.evidence_id || "") !== evidenceId
                    || numericRevision(report.offset) !== numericRevision(offset)
                    || numericRevision(report.next_offset) <= numericRevision(offset)) {
                evidencePayloadError = qsTr("Evidence payload chunk is out of sequence.")
                return
            }
            evidencePayloadChunks = evidencePayloadChunks.concat([{
                offset: numericRevision(report.offset),
                next_offset: numericRevision(report.next_offset),
                text: report.text === null || report.text === undefined ? "" : String(report.text),
                base64: report.base64 === null || report.base64 === undefined ? "" : String(report.base64)
            }])
            evidencePayloadOffset = numericRevision(report.next_offset)
            evidencePayloadDone = report.done === true
        })
        return true
    }

    function closeEvidenceDetail() {
        resetEvidenceDetail(true)
    }

    function resetEvidenceState(releasePayload) {
        resetEvidenceDetail(releasePayload)
        evidenceRequestRevision += 1
        evidenceInFlight = false
        evidenceRows = []
        evidenceNextCursor = ""
        evidenceLoaded = false
        evidencePageContext = null
        evidenceError = ""
    }

    function resetEvidenceDetail(releasePayload) {
        if (releasePayload) {
            releaseEvidencePayloadSession()
        }
        evidenceDetailRequestRevision += 1
        evidencePayloadRequestRevision += 1
        evidenceDetailInFlight = false
        evidencePayloadInFlight = false
        selectedEvidenceRow = null
        evidenceDetail = null
        evidencePayloadChunks = []
        evidencePayloadOffset = 0
        evidencePayloadDone = true
        evidenceDetailError = ""
        evidencePayloadError = ""
    }

    function releaseEvidencePayloadSession() {
        const report = evidenceDetail
        const payload = report && report.payload ? report.payload : null
        const row = report && report.row ? report.row : null
        const reference = row && row.reference ? row.reference : null
        const sessionId = String(payload && payload.session_id || "")
        if (!reference || sessionId.length === 0 || !networkScope) {
            return false
        }
        dispatch("zoneEvidencePayloadRelease", {
            source_revision: numericRevision(report.source_revision),
            network_scope: report.network_scope || networkScope,
            channel_id: String(report.channel_id || reference.channel_id || ""),
            evidence_id: String(reference.evidence_id || ""),
            session_id: sessionId
        }, function (_response) {})
        return true
    }

    function evidenceContextIsCurrent(pageContext) {
        return pageContext
            && pageContext.source_generation === sourceGeneration
            && pageContext.source_revision === sourceRevision
            && pageContext.network_scope_key === networkScopeKey
            && activeZoneContext
            && pageContext.channel_id === activeZoneId
            && pageContext.context_revision === numericRevision(activeZoneContext.context_revision)
    }

    function appendUniqueEvidenceRows(existing, additions) {
        const rows = []
        const seen = ({})
        const values = (Array.isArray(existing) ? existing : []).concat(
            Array.isArray(additions) ? additions : []
        )
        for (let i = 0; i < values.length; ++i) {
            const evidenceId = String(values[i] && values[i].reference
                && values[i].reference.evidence_id || "")
            if (evidenceId.length > 0 && seen[evidenceId] !== true) {
                seen[evidenceId] = true
                rows.push(values[i])
            }
        }
        return rows
    }

    function normalizedEvidenceFilter(filter) {
        const value = String(filter || "all")
        return value === "channel_configuration"
                || value === "channel_operation"
                || value === "raw_inscription"
            ? value
            : "all"
    }

}
