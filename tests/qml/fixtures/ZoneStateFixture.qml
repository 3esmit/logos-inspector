import QtQml
import "ZoneFixtureData.js" as FixtureData

QtObject {
    id: root

    property double sourceRevision: 3
    property double catalogRevision: 19
    property string verification: "verified"
    property var coverage: ({
        status: "complete",
        coverage_floor: 0,
        scanned_through_slot: 187085,
        observed_lib_slot: 187085,
        prefix_status: "complete",
        gap_count: 0
    })
    property var ingestion: ({
        worker_running: false,
        target_lib_slot: 187085,
        ingestion_cursor_slot: 187085,
        discovered_zone_count: 3
    })
    property string currentError: ""
    property string statusError: ""
    property string configureError: ""
    property string summaryError: ""
    property string detailError: ""
    property string sourceMutationError: ""
    property var sourceMutationWarning: null
    property bool controlInFlight: false
    property bool summaryInFlight: false
    property bool detailInFlight: false
    property bool sourceMutationInFlight: false
    property bool summaryStale: false
    property bool detailStale: false
    property var zoneSummaries: FixtureData.zones()
    property string activeZoneId: FixtureData.identity("1")
    property var zoneDetail: FixtureData.detailFor(activeZoneId)
    property var networkScope: FixtureData.networkScope()
    property string networkScopeKey: "genesis_id:" + FixtureData.identity("f")

    property string evidenceFilter: "all"
    property var evidenceRows: []
    property string evidenceNextCursor: ""
    property bool evidenceLoaded: false
    property bool evidenceInFlight: false
    property string evidenceError: ""
    property var selectedEvidenceRow: null
    property var evidenceDetail: null
    property bool evidenceDetailInFlight: false
    property string evidenceDetailError: ""
    property var evidencePayloadChunks: []
    property bool evidencePayloadDone: true
    property bool evidencePayloadInFlight: false
    property string evidencePayloadError: ""
    property var lastMutationRequest: null
    property string mutationFailure: ""
    property int retryCount: 0

    function activateZone(channelId) {
        const target = String(channelId || "")
        const rows = zoneSummaries
        for (let i = 0; i < rows.length; ++i) {
            if (rows[i].channel_id === target) {
                activeZoneId = target
                zoneDetail = FixtureData.detailFor(target)
                evidenceLoaded = false
                evidenceRows = []
                selectedEvidenceRow = null
                evidenceDetail = null
                return true
            }
        }
        return false
    }

    function retryCatalog() {
        retryCount += 1
    }

    function loadEvidence(filter) {
        evidenceFilter = String(filter || "all")
        const rows = FixtureData.evidenceRows(activeZoneId)
        evidenceRows = rows.filter(function (row) {
            return evidenceFilter === "all"
                || row.reference.evidence_kind === evidenceFilter
        })
        evidenceLoaded = true
        evidenceNextCursor = ""
        return true
    }

    function loadMoreEvidence() {
        return false
    }

    function openEvidence(row) {
        selectedEvidenceRow = row
        evidenceDetail = FixtureData.evidenceDetail(row)
        evidencePayloadDone = true
        return true
    }

    function closeEvidenceDetail() {
        selectedEvidenceRow = null
        evidenceDetail = null
    }

    function loadNextEvidencePayloadChunk() {
        evidencePayloadDone = true
        return false
    }

    function applyChannelSourceConfig(request, callback) {
        lastMutationRequest = request
        if (callback) {
            callback(mutationFailure.length > 0
                ? { ok: false, value: null, text: "", error: mutationFailure }
                : { ok: true, value: {}, text: "", error: "" })
        }
        return 1
    }
}
