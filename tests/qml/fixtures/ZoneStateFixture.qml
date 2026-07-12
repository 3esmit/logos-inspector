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
    property var activeZoneContext: FixtureData.activeZoneContext(activeZoneId)
    property var zoneDetail: FixtureData.detailFor(activeZoneId)
    property var networkScope: FixtureData.networkScope()
    property string networkScopeKey: "genesis_id:" + FixtureData.identity("f")

    property int l2BlocksLimit: 25
    property var l2BlockRows: FixtureData.l2BlockRows()
    property string l2BlocksNextCursor: ""
    property bool l2BlocksHasMore: false
    property int l2BlocksDistinctCount: 3
    property var l2BlocksSourceHeads: [{
        source_id: "src_33333333333333333333333333333333",
        source_role: "indexer",
        block_id: 12842,
        block_hash: FixtureData.identity("d")
    }, {
        source_id: "src_11111111111111111111111111111111",
        source_role: "sequencer",
        block_id: 12844,
        block_hash: FixtureData.identity("b")
    }]
    property var l2BlocksRoute: FixtureData.l2RouteReport("lez.blocks", null).route
    property string l2BlocksRouteCompleteness: "all_configured"
    property var l2BlocksWarnings: []
    property string l2BlocksError: ""
    property var l2BlocksErrorDetails: null
    property bool l2BlocksLoaded: true
    property bool l2BlocksInFlight: false
    property int l2RefreshCount: 0

    property var l2BlockTarget: null
    property string l2BlockRequestedSourceId: ""
    property var l2BlockDetailReport: null
    property var l2BlockDetail: null
    property var l2BlockCandidates: []
    property string l2BlockDetailError: ""
    property var l2BlockDetailErrorDetails: null
    property bool l2BlockDetailInFlight: false

    property string l2TransactionId: ""
    property string l2TransactionRequestedSourceId: ""
    property var l2TransactionDetailReport: null
    property var l2TransactionDetail: null
    property var l2TransactionCandidates: []
    property string l2TransactionDetailError: ""
    property var l2TransactionDetailErrorDetails: null
    property bool l2TransactionDetailInFlight: false

    property var l2TransactionTraceReport: null
    property var l2TransactionTrace: null
    property string l2TransactionTraceError: ""
    property var l2TransactionTraceErrorDetails: null
    property bool l2TransactionTraceInFlight: false

    readonly property bool l2Applicable: activeZoneContext !== null
        && activeZoneContext.zone_kind === "sequencer_zone"
    readonly property bool l2SourceConfigured: l2Applicable
        && (String(activeZoneContext.indexer_source_id || "").length > 0
            || String(activeZoneContext.selected_sequencer_source_id || "").length > 0)
    readonly property bool l2ReadEnabled: verification === "verified"
        && l2Applicable && l2SourceConfigured

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

    onActiveZoneIdChanged: {
        activeZoneContext = FixtureData.activeZoneContext(activeZoneId)
    }

    function activateZone(channelId) {
        const target = String(channelId || "")
        const rows = zoneSummaries
        for (let i = 0; i < rows.length; ++i) {
            if (rows[i].channel_id === target) {
                activeZoneId = target
                zoneDetail = FixtureData.detailFor(target)
                resetL2Fixture()
                evidenceLoaded = false
                evidenceRows = []
                selectedEvidenceRow = null
                evidenceDetail = null
                return true
            }
        }
        return false
    }

    function resetL2Fixture() {
        l2BlockDetail = null
        l2BlockDetailReport = null
        l2BlockCandidates = []
        l2TransactionDetail = null
        l2TransactionDetailReport = null
        l2TransactionTrace = null
        l2TransactionTraceReport = null
        l2BlockRows = l2Applicable ? FixtureData.l2BlockRows() : []
        l2BlocksLoaded = l2Applicable
    }

    function refreshL2Blocks() {
        l2RefreshCount += 1
        l2BlockRows = l2Applicable ? FixtureData.l2BlockRows() : []
        l2BlocksLoaded = true
        l2BlocksError = l2ReadEnabled ? "" : l2AvailabilityMessage()
        return l2ReadEnabled ? l2RefreshCount : null
    }

    function loadMoreL2Blocks() {
        return null
    }

    function setL2BlocksLimit(limit) {
        l2BlocksLimit = Number(limit)
        refreshL2Blocks()
        return true
    }

    function openL2Block(value, exactSourceId) {
        const summary = value && value.summary ? value.summary : value
        l2BlockTarget = summary
        l2BlockRequestedSourceId = String(exactSourceId || "")
        l2BlockDetail = FixtureData.l2BlockDetail(summary, l2BlockRequestedSourceId)
        l2BlockDetailReport = FixtureData.l2RouteReport("lez.block_detail", l2BlockDetail.source)
        l2BlockCandidates = []
        return 1
    }

    function resolveL2BlockCandidate(candidate) {
        return openL2Block(l2BlockTarget, candidate && candidate.source_id)
    }

    function closeL2BlockDetail() {
        l2BlockDetail = null
        l2BlockDetailReport = null
        closeL2Transaction()
    }

    function openL2Transaction(transactionId, exactSourceId) {
        l2TransactionId = String(transactionId || "")
        l2TransactionRequestedSourceId = String(exactSourceId || "")
        l2TransactionDetail = FixtureData.l2TransactionDetail(
            l2TransactionId,
            l2TransactionRequestedSourceId
        )
        l2TransactionDetailReport = FixtureData.l2RouteReport(
            "lez.transaction",
            l2TransactionDetail.source
        )
        requestL2TransactionTrace(l2TransactionId,
            l2TransactionDetail.source.source_id, "")
        return 1
    }

    function resolveL2TransactionCandidate(candidate) {
        return openL2Transaction(l2TransactionId, candidate && candidate.source_id)
    }

    function requestL2TransactionTrace(transactionId, exactSourceId, idlProgramId) {
        l2TransactionTrace = FixtureData.l2TransactionTrace(transactionId, exactSourceId)
        l2TransactionTraceReport = FixtureData.l2RouteReport(
            "lez.transaction_trace",
            l2TransactionTrace.source
        )
        l2TransactionTraceError = ""
        return 1
    }

    function closeL2Transaction() {
        l2TransactionId = ""
        l2TransactionDetail = null
        l2TransactionDetailReport = null
        l2TransactionTrace = null
        l2TransactionTraceReport = null
    }

    function l2AvailabilityMessage() {
        return l2Applicable
            ? "Configure an Indexer or select a Sequencer source for this Zone."
            : "L2 reads do not apply to this Channel type."
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
