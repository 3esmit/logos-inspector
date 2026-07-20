import QtQml

QtObject {
    id: root

    required property var l2Context
    readonly property var activeZoneContext: l2Context.activeZoneContext
    readonly property bool l2Applicable: l2Context.l2Applicable
    readonly property bool l2SourceConfigured: l2Context.l2SourceConfigured
    readonly property bool l2ReadEnabled: l2Context.l2ReadEnabled
    readonly property bool l2IndexerReadEnabled: l2Context.l2IndexerReadEnabled
    readonly property bool l2SequencerReadEnabled: l2Context.l2SequencerReadEnabled
    readonly property var appModel: l2Context.appModel || null
    readonly property int registeredIdlCount: appModel && appModel.registeredIdls
        ? Number(appModel.registeredIdls.count || 0) : 0

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
    property string l2BlocksExactSourceId: ""
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
    property int l2TransactionIdlRegistryRevision: 0
    property string l2TransactionTraceIdlProgramId: ""
    property int l2TransactionTraceIdlRegistryRevision: -1
    property bool l2TransactionTraceRedecodeQueued: false
    property string l2TransactionTraceError: ""
    property var l2TransactionTraceErrorDetails: null
    property bool l2BlocksInFlight: false
    property bool l2BlockDetailInFlight: false
    property bool l2TransactionDetailInFlight: false
    property bool l2TransactionTraceInFlight: false
    property bool l2SubmittedTransactionReadbackActive: false
    property bool l2SubmittedTransactionReadbackPending: false
    property int l2SubmittedTransactionReadbackAttempt: 0
    // Testnet currently produces roughly one block per minute.  Cover three
    // inclusion windows without making ordinary transaction searches poll.
    property int l2SubmittedTransactionReadbackMaxAttempts: 37
    property int l2SubmittedTransactionReadbackIntervalMs: 5000
    property var l2SubmittedTransactionReadbackContext: null
    property var l2SubmittedTransactionReceiptTraceInput: null
    property var l2SubmittedTransactionLocalDecode: null
    property string l2SubmittedTransactionLocalDecodeWarning: ""
    property string l2SubmittedTransactionLocalDecodeError: ""
    property bool l2SubmittedTransactionLocalDecodeInFlight: false
    property int l2SubmittedTransactionLocalDecodeRequestRevision: 0
    property bool l2SubmittedTransactionLocalDecodeQueued: false
    property int l2BlocksRequestRevision: 0
    property int l2BlockDetailRequestRevision: 0
    property int l2TransactionDetailRequestRevision: 0
    property int l2TransactionTraceRequestRevision: 0

    property Timer l2SubmittedTransactionReadbackTimer: Timer {
        interval: Math.max(1, root.l2SubmittedTransactionReadbackIntervalMs)
        repeat: false
        onTriggered: root.retrySubmittedL2TransactionReadback()
    }

    onRegisteredIdlCountChanged: {
        l2TransactionIdlRegistryRevision += 1
        resetL2SubmittedTransactionLocalDecodeResult()
        queueSubmittedTransactionLocalDecode()
        queueL2TransactionTraceRedecode()
    }

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
            l2BlocksExactSourceId = ""
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
        resetSubmittedL2TransactionReadback()
        resetSubmittedTransactionLocalDecodeState()
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

    function resetSubmittedL2TransactionReadback() {
        l2SubmittedTransactionReadbackTimer.stop()
        l2SubmittedTransactionReadbackActive = false
        l2SubmittedTransactionReadbackPending = false
        l2SubmittedTransactionReadbackAttempt = 0
        l2SubmittedTransactionReadbackContext = null
    }

    function finishSubmittedL2TransactionReadback() {
        l2SubmittedTransactionReadbackTimer.stop()
        l2SubmittedTransactionReadbackActive = false
        l2SubmittedTransactionReadbackPending = false
        l2SubmittedTransactionReadbackContext = null
    }

    function resetL2TransactionTraceState() {
        l2TransactionTraceRequestRevision += 1
        l2TransactionTraceInFlight = false
        l2TransactionTraceReport = null
        l2TransactionTrace = null
        l2TransactionTraceIdlProgramId = ""
        l2TransactionTraceIdlRegistryRevision = -1
        l2TransactionTraceError = ""
        l2TransactionTraceErrorDetails = null
    }

    function resetSubmittedTransactionLocalDecodeState() {
        resetL2SubmittedTransactionLocalDecodeResult()
        l2SubmittedTransactionReceiptTraceInput = null
    }

    function resetL2SubmittedTransactionLocalDecodeResult() {
        l2SubmittedTransactionLocalDecodeRequestRevision += 1
        l2SubmittedTransactionLocalDecodeInFlight = false
        l2SubmittedTransactionLocalDecode = null
        l2SubmittedTransactionLocalDecodeWarning = ""
        l2SubmittedTransactionLocalDecodeError = ""
    }

    function refreshL2Blocks() {
        return refreshL2BlocksForSource("")
    }

    function refreshL2BlocksForSource(exactSourceId) {
        resetL2BlocksState(true)
        resetL2BlockInspectionState()
        const sourceId = String(exactSourceId || "")
        l2BlocksExactSourceId = sourceId
        if (sourceId.length > 0 && (!l2Context.l2SequencerReadEnabled
                || sourceId !== l2Context.l2SequencerSourceId())) {
            l2BlocksLoaded = true
            l2BlocksError = qsTr("Selected Sequencer source is unavailable.")
            return null
        }
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
        refreshL2BlocksForSource(l2BlocksExactSourceId)
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
                limit: l2BlocksLimit,
                exact_source_id: l2BlocksExactSourceId.length > 0
                    ? l2BlocksExactSourceId : null
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
            if (l2BlocksExactSourceId.length > 0
                    && !l2BlocksReportMatchesExactSource(
                        report, page, l2BlocksExactSourceId)) {
                l2BlocksError = qsTr("L2 blocks returned data from another source.")
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

    function l2BlocksReportMatchesExactSource(report, page, exactSourceId) {
        const sourceId = String(exactSourceId || "")
        const route = report && report.route ? report.route : ({})
        const attempts = Array.isArray(route.attempts) ? route.attempts : []
        const heads = page && Array.isArray(page.source_heads) ? page.source_heads : []
        const rows = page && Array.isArray(page.rows) ? page.rows : []
        if (sourceId.length === 0
                || String(route.policy || "") !== "exact_source"
                || attempts.length !== 1
                || String(attempts[0].source_id || "") !== sourceId
                || String(attempts[0].source_role || "") !== "sequencer"
                || heads.length > 1
                || (rows.length > 0 && heads.length !== 1)) {
            return false
        }
        if (heads.length === 1
                && (String(heads[0].source_id || "") !== sourceId
                    || String(heads[0].source_role || "") !== "sequencer")) {
            return false
        }
        for (let i = 0; i < rows.length; ++i) {
            const observations = rows[i] && Array.isArray(rows[i].observations)
                ? rows[i].observations : []
            if (observations.length !== 1
                    || String(observations[0].source_id || "") !== sourceId
                    || String(observations[0].source_role || "") !== "sequencer") {
                return false
            }
        }
        return true
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
        return startL2TransactionReadback(transactionId, exactSourceId, false)
    }

    function openSubmittedL2Transaction(transactionId, exactSourceId,
            receiptTraceInput) {
        const sourceId = String(exactSourceId || "").trim()
        if (!l2Context.l2SequencerReadEnabled || sourceId.length === 0
                || sourceId !== l2Context.l2SequencerSourceId()) {
            return null
        }
        return startL2TransactionReadback(transactionId, sourceId, true,
            receiptTraceInput)
    }

    function startL2TransactionReadback(transactionId, exactSourceId, submitted,
            receiptTraceInput) {
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
        l2SubmittedTransactionReadbackActive = submitted === true
        l2SubmittedTransactionReadbackContext = submitted === true
            ? requestContext : null
        l2SubmittedTransactionReceiptTraceInput = submitted === true
            ? submittedReceiptTraceInputForRequest(normalizedId, sourceId,
                requestContext, receiptTraceInput) : null
        return requestL2TransactionDetail(normalizedId, sourceId,
            requestRevision, requestContext)
    }

    function requestL2TransactionDetail(normalizedId, sourceId,
            requestRevision, requestContext) {
        l2TransactionDetailInFlight = true
        l2SubmittedTransactionReadbackPending = false
        if (l2SubmittedTransactionReadbackActive) {
            l2SubmittedTransactionReadbackAttempt += 1
        }
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
                finishSubmittedL2TransactionReadback()
                return
            }
            l2TransactionDetailReport = response.value
            const outcome = response.value.data || ({})
            const kind = String(outcome.outcome || "")
            if (kind === "found" && outcome.value) {
                if (sourceId.length > 0 && String(outcome.value.source
                        && outcome.value.source.source_id || "") !== sourceId) {
                    l2TransactionDetailError = qsTr("L2 transaction returned different source provenance.")
                    finishSubmittedL2TransactionReadback()
                    return
                }
                l2TransactionDetail = outcome.value
                const source = outcome.value.source || ({})
                const returnedSourceId = String(source.source_id || sourceId)
                const localReceiptInput = submittedReceiptTraceInputForDetail(
                    normalizedId, returnedSourceId, requestContext, outcome.value)
                finishSubmittedL2TransactionReadback()
                requestL2TransactionTrace(normalizedId, returnedSourceId)
                if (localReceiptInput) {
                    requestSubmittedTransactionLocalDecode(normalizedId,
                        returnedSourceId, requestContext)
                }
                return
            }
            if (kind === "ambiguous") {
                l2TransactionCandidates = Array.isArray(outcome.candidates) ? outcome.candidates : []
                finishSubmittedL2TransactionReadback()
                return
            }
            if (kind === "not_found") {
                if (scheduleSubmittedL2TransactionReadbackRetry()) {
                    return
                }
                l2TransactionDetailError = qsTr("L2 transaction was not found in the Active Zone.")
                finishSubmittedL2TransactionReadback()
                return
            }
            l2TransactionDetailError = qsTr("L2 transaction returned an invalid outcome.")
            finishSubmittedL2TransactionReadback()
        })
    }

    function scheduleSubmittedL2TransactionReadbackRetry() {
        const maxAttempts = Math.max(1,
            Math.floor(Number(l2SubmittedTransactionReadbackMaxAttempts || 1)))
        if (!l2SubmittedTransactionReadbackActive
                || l2TransactionRequestedSourceId.length === 0
                || l2TransactionRequestedSourceId !== l2Context.l2SequencerSourceId()
                || l2SubmittedTransactionReadbackAttempt >= maxAttempts
                || !l2Context.l2RequestContextIsCurrent(
                    l2SubmittedTransactionReadbackContext)) {
            return false
        }
        l2SubmittedTransactionReadbackPending = true
        l2TransactionDetailInFlight = true
        l2SubmittedTransactionReadbackTimer.restart()
        return true
    }

    function retrySubmittedL2TransactionReadback() {
        if (!l2SubmittedTransactionReadbackActive
                || !l2SubmittedTransactionReadbackPending
                || !l2Context.l2ReadEnabled
                || !l2Context.l2RequestContextIsCurrent(
                    l2SubmittedTransactionReadbackContext)) {
            l2TransactionDetailInFlight = false
            finishSubmittedL2TransactionReadback()
            return null
        }
        return requestL2TransactionDetail(l2TransactionId,
            l2TransactionRequestedSourceId, l2TransactionDetailRequestRevision,
            l2SubmittedTransactionReadbackContext)
    }

    function resolveL2TransactionCandidate(candidate) {
        if (l2TransactionId.length === 0 || !candidate
                || String(candidate.source_id || "").length === 0) {
            return null
        }
        return openL2Transaction(l2TransactionId, String(candidate.source_id))
    }

    function requestL2TransactionTrace(transactionId, exactSourceId) {
        const normalizedId = String(transactionId || "").trim()
        if (!l2Context.l2ReadEnabled || normalizedId.length === 0) {
            return null
        }
        const programId = automaticL2TransactionIdlProgramId()
        resetL2TransactionTraceState()
        l2TransactionTraceIdlProgramId = programId
        l2TransactionTraceIdlRegistryRevision = l2TransactionIdlRegistryRevision
        l2TransactionTraceRequestRevision += 1
        const requestRevision = l2TransactionTraceRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const sourceId = String(exactSourceId || "")
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

    function submittedReceiptTraceInputForRequest(transactionId, sourceId,
            requestContext, value) {
        const input = value || ({})
        const normalizedId = String(transactionId || "").trim()
        const requestedSourceId = String(sourceId || "").trim()
        const inputContext = input.context || null
        const words = normalizedSubmittedInstructionWords(input.instructionWords)
        const accountIds = normalizedSubmittedAccountIds(input.accountIds)
        if (String(input.txHash || "") !== normalizedId
                || String(input.mode || "") !== "private"
                || !inputContext || !requestContext
                || !l2Context.sameFullL2Context(inputContext, requestContext)
                || !l2Context.l2RequestContextIsCurrent(inputContext)
                || !submittedReceiptTargetMatchesContext(input.target, inputContext,
                    requestedSourceId)
                || String(input.idlKey || "").length === 0
                || String(input.idlJson || "").length === 0
                || String(input.programIdHex || "").length === 0
                || words.length === 0 || accountIds === null) {
            return null
        }
        return frozenValue({
            txHash: normalizedId,
            mode: "private",
            target: input.target,
            context: inputContext,
            idlKey: String(input.idlKey),
            idlJson: String(input.idlJson),
            programIdHex: String(input.programIdHex).toLowerCase(),
            instructionWords: words,
            accountIds: accountIds,
            privateSyncPending: input.privateSyncPending === true
        })
    }

    function submittedReceiptTraceInputForDetail(transactionId, sourceId,
            requestContext, detail) {
        const input = l2SubmittedTransactionReceiptTraceInput
        const validInput = submittedReceiptTraceInputForRequest(transactionId,
            sourceId, requestContext, input)
        const transaction = detail && detail.transaction ? detail.transaction : null
        const source = detail && detail.source ? detail.source : null
        if (!validInput || !transaction || !source
                || String(transaction.hash || "") !== String(transactionId || "")
                || String(transaction.kind || "").toLowerCase()
                    !== "privacypreserving"
                || String(source.source_id || "") !== String(sourceId || "")
                || String(source.source_role || "") !== "sequencer"
                || Number(source.source_config_revision || 0)
                    !== Number(validInput.context.source_config_revision || 0)
                || localReceiptDecodeEntry(validInput) === null) {
            return null
        }
        return validInput
    }

    function submittedReceiptTargetMatchesContext(target, context, sourceId) {
        const actual = target || ({})
        const expected = context || ({})
        return l2Context.scopeKey(actual.network_scope)
                === l2Context.scopeKey(expected.network_scope)
            && String(actual.channel_id || "") === String(expected.channel_id || "")
            && String(actual.source_id || "") === String(sourceId || "")
            && String(expected.selected_sequencer_source_id || "")
                === String(sourceId || "")
            && Number(actual.source_config_revision || 0)
                === Number(expected.source_config_revision || 0)
            && Number(actual.context_revision || 0)
                === Number(expected.context_revision || 0)
    }

    function localReceiptDecodeEntry(input) {
        const model = appModel
        if (!model || typeof model.idlEntryForKey !== "function") {
            return null
        }
        const entry = model.idlEntryForKey(String(input && input.idlKey || ""))
        if (!entry || String(entry.json || "") !== String(input && input.idlJson || "")
                || String(entry.programIdHex || "").toLowerCase()
                    !== String(input && input.programIdHex || "").toLowerCase()) {
            return null
        }
        return entry
    }

    function requestSubmittedTransactionLocalDecode(transactionId, sourceId,
            requestContext) {
        const normalizedId = String(transactionId || "").trim()
        const input = submittedReceiptTraceInputForDetail(normalizedId, sourceId,
            requestContext, l2TransactionDetail)
        if (!input || !appModel || typeof appModel.decodeInstructionAsync !== "function") {
            return null
        }
        const ticket = l2SubmittedTransactionLocalDecodeRequestRevision + 1
        l2SubmittedTransactionLocalDecodeRequestRevision = ticket
        l2SubmittedTransactionLocalDecodeInFlight = true
        l2SubmittedTransactionLocalDecode = null
        l2SubmittedTransactionLocalDecodeWarning = ""
        l2SubmittedTransactionLocalDecodeError = ""
        return appModel.decodeInstructionAsync(input.programIdHex,
            input.instructionWords, input.idlJson, input.accountIds,
            function (response) {
                if (ticket !== l2SubmittedTransactionLocalDecodeRequestRevision) {
                    return
                }
                l2SubmittedTransactionLocalDecodeInFlight = false
                if (!submittedReceiptTraceInputForDetail(normalizedId, sourceId,
                        requestContext, l2TransactionDetail)) {
                    return
                }
                if (!response || response.ok !== true || !response.value
                        || typeof response.value !== "object") {
                    l2SubmittedTransactionLocalDecodeError = String(response
                        && response.error || qsTr("Local submission decode failed."))
                    return
                }
                l2SubmittedTransactionLocalDecode = frozenValue(response.value)
                l2SubmittedTransactionLocalDecodeWarning = String(response.value.decode_error || "")
            })
    }

    function queueSubmittedTransactionLocalDecode() {
        if (l2SubmittedTransactionLocalDecodeQueued) {
            return
        }
        l2SubmittedTransactionLocalDecodeQueued = true
        Qt.callLater(function () {
            l2SubmittedTransactionLocalDecodeQueued = false
            if (!l2TransactionDetail || l2TransactionId.length === 0) {
                return
            }
            const source = l2TransactionDetail.source || ({})
            requestSubmittedTransactionLocalDecode(l2TransactionId,
                String(source.source_id || ""), l2Context.l2RequestContext())
        })
    }

    function normalizedSubmittedInstructionWords(value) {
        if (!Array.isArray(value) || value.length === 0) {
            return []
        }
        const words = []
        for (let index = 0; index < value.length; ++index) {
            const word = Number(value[index])
            if (!Number.isFinite(word) || word < 0 || word > 4294967295
                    || Math.floor(word) !== word) {
                return []
            }
            words.push(word)
        }
        return words
    }

    function normalizedSubmittedAccountIds(value) {
        if (!Array.isArray(value)) {
            return null
        }
        const accountIds = []
        for (let index = 0; index < value.length; ++index) {
            const accountId = String(value[index] || "").trim()
            if (accountId.length === 0) {
                return null
            }
            accountIds.push(accountId)
        }
        return accountIds
    }

    function frozenValue(value) {
        return value === undefined || value === null
            ? null : JSON.parse(JSON.stringify(value))
    }

    function automaticL2TransactionIdlProgramId() {
        const model = appModel
        const transaction = l2TransactionDetail && l2TransactionDetail.transaction
            ? l2TransactionDetail.transaction : null
        const programId = String(transaction && transaction.program_id_hex || "").trim()
        if (!model || !programId.length || typeof model.idlEntriesForProgram !== "function") {
            return ""
        }
        const entries = model.idlEntriesForProgram(programId)
        return Array.isArray(entries) && entries.length > 0 ? programId : ""
    }

    function queueL2TransactionTraceRedecode() {
        if (l2TransactionTraceRedecodeQueued) {
            return
        }
        l2TransactionTraceRedecodeQueued = true
        Qt.callLater(function () {
            l2TransactionTraceRedecodeQueued = false
            reDecodeL2TransactionTrace()
        })
    }

    function reDecodeL2TransactionTrace() {
        if (!l2TransactionDetail || l2TransactionId.length === 0) {
            return null
        }
        const programId = automaticL2TransactionIdlProgramId()
        if (!programId.length && !l2TransactionTraceIdlProgramId.length) {
            return null
        }
        if (programId === l2TransactionTraceIdlProgramId
                && l2TransactionIdlRegistryRevision
                    === l2TransactionTraceIdlRegistryRevision) {
            return null
        }
        const source = l2TransactionDetail.source || ({})
        return requestL2TransactionTrace(l2TransactionId, String(source.source_id || ""))
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
