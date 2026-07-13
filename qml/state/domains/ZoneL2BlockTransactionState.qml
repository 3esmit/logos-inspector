import QtQml

QtObject {
    id: root

    required property var l2Context
    readonly property bool l2Applicable: l2Context.l2Applicable
    readonly property bool l2SourceConfigured: l2Context.l2SourceConfigured
    readonly property bool l2ReadEnabled: l2Context.l2ReadEnabled
    readonly property bool l2IndexerReadEnabled: l2Context.l2IndexerReadEnabled
    readonly property bool l2SequencerReadEnabled: l2Context.l2SequencerReadEnabled

    function l2AvailabilityMessage() {
        return l2Context.l2AvailabilityMessage()
    }

    function l2IndexerSourceId() {
        return l2Context.l2IndexerSourceId()
    }

    function l2SequencerSourceId() {
        return l2Context.l2SequencerSourceId()
    }

    property int l2BlocksLimit: 25
    property var l2BlockRows: []
    property string l2BlocksNextCursor: ""
    property bool l2BlocksHasMore: false
    property int l2BlocksDistinctCount: 0
    property var l2BlocksSourceHeads: []
    property var l2BlocksRoute: null
    property string l2BlocksRouteCompleteness: ""
    property var l2BlocksWarnings: []
    property string l2BlocksError: ""
    property var l2BlocksErrorDetails: null
    property bool l2BlocksLoaded: false
    property var l2BlockTarget: null
    property string l2BlockRequestedSourceId: ""
    property var l2BlockDetailReport: null
    property var l2BlockDetail: null
    property var l2BlockCandidates: []
    property string l2BlockDetailError: ""
    property var l2BlockDetailErrorDetails: null
    property string l2TransactionId: ""
    property string l2TransactionRequestedSourceId: ""
    property var l2TransactionDetailReport: null
    property var l2TransactionDetail: null
    property var l2TransactionCandidates: []
    property string l2TransactionDetailError: ""
    property var l2TransactionDetailErrorDetails: null
    property var l2TransactionTraceReport: null
    property var l2TransactionTrace: null
    property string l2TransactionTraceError: ""
    property var l2TransactionTraceErrorDetails: null
    property bool l2BlocksInFlight: false
    property bool l2BlockDetailInFlight: false
    property bool l2TransactionDetailInFlight: false
    property bool l2TransactionTraceInFlight: false
    property int l2BlocksRequestRevision: 0
    property int l2BlockDetailRequestRevision: 0
    property int l2TransactionDetailRequestRevision: 0
    property int l2TransactionTraceRequestRevision: 0

    function resetL2BlocksState(clearRows) {
        l2BlocksRequestRevision += 1
        l2BlocksInFlight = false
        l2BlocksNextCursor = ""
        l2BlocksHasMore = false
        l2BlocksRoute = null
        l2BlocksRouteCompleteness = ""
        l2BlocksWarnings = []
        l2BlocksError = ""
        l2BlocksErrorDetails = null
        if (clearRows) {
            l2BlockRows = []
            l2BlocksDistinctCount = 0
            l2BlocksSourceHeads = []
            l2BlocksLoaded = false
        }
    }

    function resetL2BlockInspectionState() {
        l2BlockDetailRequestRevision += 1
        l2BlockDetailInFlight = false
        l2BlockTarget = null
        l2BlockRequestedSourceId = ""
        l2BlockDetailReport = null
        l2BlockDetail = null
        l2BlockCandidates = []
        l2BlockDetailError = ""
        l2BlockDetailErrorDetails = null
        resetL2TransactionInspectionState()
    }

    function resetL2TransactionInspectionState() {
        l2TransactionDetailRequestRevision += 1
        l2TransactionDetailInFlight = false
        l2TransactionId = ""
        l2TransactionRequestedSourceId = ""
        l2TransactionDetailReport = null
        l2TransactionDetail = null
        l2TransactionCandidates = []
        l2TransactionDetailError = ""
        l2TransactionDetailErrorDetails = null
        resetL2TransactionTraceState()
    }

    function resetL2TransactionTraceState() {
        l2TransactionTraceRequestRevision += 1
        l2TransactionTraceInFlight = false
        l2TransactionTraceReport = null
        l2TransactionTrace = null
        l2TransactionTraceError = ""
        l2TransactionTraceErrorDetails = null
    }

    function refreshL2Blocks() {
        resetL2BlocksState(true)
        resetL2BlockInspectionState()
        if (!l2Context.l2ReadEnabled) {
            l2BlocksLoaded = true
            l2BlocksError = l2Context.l2AvailabilityMessage()
            return null
        }
        return requestL2Blocks("", false)
    }

    function loadMoreL2Blocks() {
        if (!l2Context.l2ReadEnabled || l2BlocksInFlight || !l2BlocksHasMore
                || l2BlocksNextCursor.length === 0) {
            return null
        }
        return requestL2Blocks(l2BlocksNextCursor, true)
    }

    function setL2BlocksLimit(limit) {
        const next = Math.max(1, Math.min(50, Math.floor(Number(limit || 25))))
        if (next === l2BlocksLimit) {
            return false
        }
        l2BlocksLimit = next
        refreshL2Blocks()
        return true
    }

    function requestL2Blocks(cursor, append) {
        if (!l2Context.l2ReadEnabled || l2BlocksInFlight) {
            return null
        }
        l2BlocksRequestRevision += 1
        const requestRevision = l2BlocksRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const cursorText = String(cursor || "")
        l2BlocksInFlight = true
        l2BlocksError = ""
        l2BlocksErrorDetails = null
        return l2Context.dispatch("zoneL2Blocks", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                cursor: cursorText.length > 0 ? cursorText : null,
                limit: l2BlocksLimit
            }
        }, function (response) {
            if (requestRevision !== l2BlocksRequestRevision) {
                return
            }
            l2BlocksInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.blocks", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2BlocksError = l2Context.responseError(response, qsTr("L2 blocks could not be loaded."))
                    l2BlocksErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            const report = response.value
            const outcome = report.data || ({})
            if (String(outcome.outcome || "") === "not_found") {
                l2BlocksLoaded = true
                if (!append) {
                    l2BlockRows = []
                    l2BlocksDistinctCount = 0
                }
                applyL2BlocksReportMetadata(report, null, append)
                return
            }
            const page = outcome.value
            if (String(outcome.outcome || "") !== "found" || !page
                    || !Array.isArray(page.rows) || !Array.isArray(page.source_heads)) {
                l2BlocksError = qsTr("L2 blocks returned an invalid page.")
                return
            }
            l2BlockRows = append ? l2BlockRows.concat(page.rows) : page.rows
            l2BlocksDistinctCount = append
                ? l2BlocksDistinctCount + Number(page.distinct_block_ids || 0)
                : Number(page.distinct_block_ids || 0)
            l2BlocksSourceHeads = page.source_heads
            l2BlocksNextCursor = String(page.next_cursor || "")
            l2BlocksHasMore = page.has_more === true && l2BlocksNextCursor.length > 0
            l2BlocksLoaded = true
            applyL2BlocksReportMetadata(report, page, append)
        })
    }

    function applyL2BlocksReportMetadata(report, page, append) {
        l2BlocksRoute = report.route || null
        l2BlocksRouteCompleteness = String(report.route_completeness || "")
        const warnings = Array.isArray(report.warnings) ? report.warnings : []
        l2BlocksWarnings = append ? l2BlocksWarnings.concat(warnings) : warnings
        if (!page) {
            l2BlocksNextCursor = ""
            l2BlocksHasMore = false
            if (!append) {
                l2BlocksSourceHeads = []
            }
        }
    }

    function openL2Block(value, exactSourceId) {
        const target = l2BlockTargetFrom(value)
        if (!target) {
            return null
        }
        return requestL2BlockDetail(target, exactSourceId)
    }

    function resolveL2BlockCandidate(candidate) {
        if (!l2BlockTarget || !candidate || String(candidate.source_id || "").length === 0) {
            return null
        }
        return requestL2BlockDetail(l2BlockTarget, String(candidate.source_id))
    }

    function requestL2BlockDetail(target, exactSourceId) {
        if (!l2Context.l2ReadEnabled) {
            return null
        }
        resetL2BlockInspectionState()
        l2BlockTarget = target
        l2BlockDetailRequestRevision += 1
        const requestRevision = l2BlockDetailRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const sourceId = String(exactSourceId || "")
        l2BlockRequestedSourceId = sourceId
        l2BlockDetailInFlight = true
        return l2Context.dispatch("zoneL2BlockDetail", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                target: target,
                exact_source_id: sourceId.length > 0 ? sourceId : null
            }
        }, function (response) {
            if (requestRevision !== l2BlockDetailRequestRevision) {
                return
            }
            l2BlockDetailInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.block_detail", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2BlockDetailError = l2Context.responseError(response, qsTr("L2 block detail could not be loaded."))
                    l2BlockDetailErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            l2BlockDetailReport = response.value
            const outcome = response.value.data || ({})
            const kind = String(outcome.outcome || "")
            if (kind === "found" && outcome.value) {
                if (sourceId.length > 0 && String(outcome.value.source
                        && outcome.value.source.source_id || "") !== sourceId) {
                    l2BlockDetailError = qsTr("L2 block detail returned different source provenance.")
                    return
                }
                l2BlockDetail = outcome.value
                return
            }
            if (kind === "ambiguous") {
                l2BlockCandidates = Array.isArray(outcome.candidates) ? outcome.candidates : []
                return
            }
            if (kind === "not_found") {
                l2BlockDetailError = qsTr("L2 block was not found in the Active Zone.")
                return
            }
            l2BlockDetailError = qsTr("L2 block detail returned an invalid outcome.")
        })
    }

    function closeL2BlockDetail() {
        resetL2BlockInspectionState()
    }

    function openL2Transaction(transactionId, exactSourceId) {
        const normalizedId = String(transactionId || "").trim()
        if (!l2Context.l2ReadEnabled || normalizedId.length === 0) {
            return null
        }
        resetL2TransactionInspectionState()
        l2TransactionId = normalizedId
        l2TransactionDetailRequestRevision += 1
        const requestRevision = l2TransactionDetailRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const sourceId = String(exactSourceId || "")
        l2TransactionRequestedSourceId = sourceId
        l2TransactionDetailInFlight = true
        return l2Context.dispatch("zoneL2Transaction", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                transaction_id: normalizedId,
                exact_source_id: sourceId.length > 0 ? sourceId : null
            }
        }, function (response) {
            if (requestRevision !== l2TransactionDetailRequestRevision) {
                return
            }
            l2TransactionDetailInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.transaction", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2TransactionDetailError = l2Context.responseError(response, qsTr("L2 transaction could not be loaded."))
                    l2TransactionDetailErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            l2TransactionDetailReport = response.value
            const outcome = response.value.data || ({})
            const kind = String(outcome.outcome || "")
            if (kind === "found" && outcome.value) {
                if (sourceId.length > 0 && String(outcome.value.source
                        && outcome.value.source.source_id || "") !== sourceId) {
                    l2TransactionDetailError = qsTr("L2 transaction returned different source provenance.")
                    return
                }
                l2TransactionDetail = outcome.value
                const source = outcome.value.source || ({})
                const returnedSourceId = String(source.source_id || sourceId)
                requestL2TransactionTrace(normalizedId, returnedSourceId, "")
                return
            }
            if (kind === "ambiguous") {
                l2TransactionCandidates = Array.isArray(outcome.candidates) ? outcome.candidates : []
                return
            }
            if (kind === "not_found") {
                l2TransactionDetailError = qsTr("L2 transaction was not found in the Active Zone.")
                return
            }
            l2TransactionDetailError = qsTr("L2 transaction returned an invalid outcome.")
        })
    }

    function resolveL2TransactionCandidate(candidate) {
        if (l2TransactionId.length === 0 || !candidate
                || String(candidate.source_id || "").length === 0) {
            return null
        }
        return openL2Transaction(l2TransactionId, String(candidate.source_id))
    }

    function requestL2TransactionTrace(transactionId, exactSourceId, idlProgramId) {
        const normalizedId = String(transactionId || "").trim()
        if (!l2Context.l2ReadEnabled || normalizedId.length === 0) {
            return null
        }
        resetL2TransactionTraceState()
        l2TransactionTraceRequestRevision += 1
        const requestRevision = l2TransactionTraceRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const sourceId = String(exactSourceId || "")
        const programId = String(idlProgramId || "")
        l2TransactionTraceInFlight = true
        return l2Context.dispatch("zoneL2TransactionTrace", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                transaction_id: normalizedId,
                exact_source_id: sourceId.length > 0 ? sourceId : null,
                idl_program_id: programId.length > 0 ? programId : null
            }
        }, function (response) {
            if (requestRevision !== l2TransactionTraceRequestRevision) {
                return
            }
            l2TransactionTraceInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.transaction_trace", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2TransactionTraceError = l2Context.responseError(response, qsTr("Transaction trace could not be derived."))
                    l2TransactionTraceErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            l2TransactionTraceReport = response.value
            const outcome = response.value.data || ({})
            const kind = String(outcome.outcome || "")
            if (kind === "found" && outcome.value) {
                if (sourceId.length > 0 && String(outcome.value.source
                        && outcome.value.source.source_id || "") !== sourceId) {
                    l2TransactionTraceError = qsTr("Transaction trace returned different source provenance.")
                    return
                }
                l2TransactionTrace = outcome.value
            } else if (kind === "not_found") {
                l2TransactionTraceError = qsTr("Transaction trace source payload was not found.")
            } else if (kind === "ambiguous") {
                l2TransactionTraceError = qsTr("Transaction trace requires an exact source.")
            } else {
                l2TransactionTraceError = qsTr("Transaction trace returned an invalid outcome.")
            }
        })
    }

    function closeL2Transaction() {
        resetL2TransactionInspectionState()
    }

    function l2BlockTargetFrom(value) {
        if (!value || typeof value !== "object") {
            return null
        }
        const kind = String(value.kind || "")
        const blockId = Number(value.block_id)
        const blockHash = String(value.block_hash || "").trim()
        if (kind === "hash" && blockHash.length > 0) {
            return { kind: "hash", block_hash: blockHash }
        }
        if (kind === "id" && Number.isFinite(blockId) && blockId >= 0) {
            return { kind: "id", block_id: Math.floor(blockId) }
        }
        if (kind === "identity" && Number.isFinite(blockId) && blockId >= 0
                && blockHash.length > 0) {
            return { kind: "identity", block_id: Math.floor(blockId), block_hash: blockHash }
        }
        if (Number.isFinite(blockId) && blockId >= 0 && blockHash.length > 0) {
            return { kind: "identity", block_id: Math.floor(blockId), block_hash: blockHash }
        }
        if (blockHash.length > 0) {
            return { kind: "hash", block_hash: blockHash }
        }
        if (Number.isFinite(blockId) && blockId >= 0) {
            return { kind: "id", block_id: Math.floor(blockId) }
        }
        return null
    }

    function l2BlockEntityRef(detail) {
        const value = detail || l2BlockDetail
        const summary = value && value.summary ? value.summary : null
        if (!summary) {
            return null
        }
        return l2Context.l2EntityRef("block", "block:" + String(summary.block_id)
            + ":" + String(summary.block_hash || ""), value.source)
    }

    function l2TransactionEntityRef(detail) {
        const value = detail || l2TransactionDetail
        const transaction = value && value.transaction ? value.transaction : null
        return transaction ? l2Context.l2EntityRef("transaction", transaction.hash, value.source) : null
    }
}
