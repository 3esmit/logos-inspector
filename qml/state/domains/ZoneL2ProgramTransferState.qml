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

    property var l2ProgramsReport: null
    property var l2Programs: []
    property bool l2ProgramsLoaded: false
    property string l2ProgramsError: ""
    property var l2ProgramsErrorDetails: null
    property string l2CommitmentHex: ""
    property var l2CommitmentProofReport: null
    property var l2CommitmentProof: null
    property bool l2CommitmentProofLoaded: false
    property string l2CommitmentProofError: ""
    property var l2CommitmentProofErrorDetails: null
    property var l2NonceAccountIds: []
    property var l2AccountNoncesReport: null
    property var l2AccountNonces: []
    property bool l2AccountNoncesLoaded: false
    property string l2AccountNoncesError: ""
    property var l2AccountNoncesErrorDetails: null
    property int l2TransfersLimit: 25
    property var l2TransfersReport: null
    property var l2TransferRecipients: []
    property string l2TransfersNextCursor: ""
    property bool l2TransfersHasMore: false
    property var l2TransfersNewestBlock: null
    property var l2TransfersOldestBlock: null
    property int l2TransfersScannedBlocks: 0
    property bool l2TransfersFinalized: false
    property bool l2TransfersLoaded: false
    property var l2TransfersHistory: []
    property string l2TransfersError: ""
    property var l2TransfersErrorDetails: null
    property bool l2ProgramsInFlight: false
    property bool l2CommitmentProofInFlight: false
    property bool l2AccountNoncesInFlight: false
    property bool l2TransfersInFlight: false
    property int l2ProgramsRequestRevision: 0
    property int l2CommitmentProofRequestRevision: 0
    property int l2AccountNoncesRequestRevision: 0
    property int l2TransfersRequestRevision: 0

    function resetL2ProgramsState() {
        l2ProgramsRequestRevision += 1
        l2ProgramsInFlight = false
        l2ProgramsReport = null
        l2Programs = []
        l2ProgramsLoaded = false
        l2ProgramsError = ""
        l2ProgramsErrorDetails = null
    }

    function resetL2CommitmentProofState() {
        l2CommitmentProofRequestRevision += 1
        l2CommitmentProofInFlight = false
        l2CommitmentHex = ""
        l2CommitmentProofReport = null
        l2CommitmentProof = null
        l2CommitmentProofLoaded = false
        l2CommitmentProofError = ""
        l2CommitmentProofErrorDetails = null
    }

    function resetL2AccountNoncesState() {
        l2AccountNoncesRequestRevision += 1
        l2AccountNoncesInFlight = false
        l2NonceAccountIds = []
        l2AccountNoncesReport = null
        l2AccountNonces = []
        l2AccountNoncesLoaded = false
        l2AccountNoncesError = ""
        l2AccountNoncesErrorDetails = null
    }

    function resetL2TransfersState(clearHistory) {
        l2TransfersRequestRevision += 1
        l2TransfersInFlight = false
        l2TransfersReport = null
        l2TransferRecipients = []
        l2TransfersNextCursor = ""
        l2TransfersHasMore = false
        l2TransfersNewestBlock = null
        l2TransfersOldestBlock = null
        l2TransfersScannedBlocks = 0
        l2TransfersFinalized = false
        l2TransfersLoaded = false
        l2TransfersError = ""
        l2TransfersErrorDetails = null
        if (clearHistory) {
            l2TransfersHistory = []
        }
    }

    function refreshL2Programs() {
        resetL2ProgramsState()
        if (!l2Context.l2SequencerReadEnabled) {
            l2ProgramsLoaded = true
            l2ProgramsError = qsTr("Select a Sequencer source to inspect known programs.")
            return null
        }
        l2ProgramsRequestRevision += 1
        const requestRevision = l2ProgramsRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const sourceId = l2Context.l2SequencerSourceId()
        l2ProgramsInFlight = true
        return l2Context.dispatch("zoneL2Programs", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                exact_source_id: sourceId
            }
        }, function (response) {
            if (requestRevision !== l2ProgramsRequestRevision) {
                return
            }
            l2ProgramsInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.programs", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2ProgramsError = l2Context.responseError(response,
                        qsTr("Known programs could not be loaded."))
                    l2ProgramsErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            const report = response.value
            const outcome = report.data || ({})
            if (String(outcome.outcome || "") !== "found" || !outcome.value
                    || !Array.isArray(outcome.value.programs)
                    || !l2Context.validL2SingleSourceValue(outcome.value, sourceId, "sequencer")) {
                l2ProgramsError = qsTr("Known programs returned invalid source data.")
                return
            }
            l2ProgramsReport = report
            l2Programs = outcome.value.programs.slice()
            l2ProgramsLoaded = true
        })
    }

    function requestL2CommitmentProof(commitmentHex) {
        resetL2CommitmentProofState()
        const normalizedCommitment = String(commitmentHex || "").trim()
        if (normalizedCommitment.length === 0) {
            l2CommitmentProofError = qsTr("Enter a commitment hash.")
            return null
        }
        if (!l2Context.l2SequencerReadEnabled) {
            l2CommitmentProofError = qsTr("Select a Sequencer source to inspect a commitment proof.")
            return null
        }
        l2CommitmentHex = normalizedCommitment
        l2CommitmentProofRequestRevision += 1
        const requestRevision = l2CommitmentProofRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const sourceId = l2Context.l2SequencerSourceId()
        l2CommitmentProofInFlight = true
        return l2Context.dispatch("zoneL2CommitmentProof", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                commitment_hex: normalizedCommitment,
                exact_source_id: sourceId
            }
        }, function (response) {
            if (requestRevision !== l2CommitmentProofRequestRevision) {
                return
            }
            l2CommitmentProofInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)
                    || normalizedCommitment !== l2CommitmentHex) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.commitment_proof", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2CommitmentProofError = l2Context.responseError(response,
                        qsTr("Commitment proof could not be loaded."))
                    l2CommitmentProofErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            const report = response.value
            const outcome = report.data || ({})
            const outcomeKind = String(outcome.outcome || "")
            l2CommitmentProofReport = report
            l2CommitmentProofLoaded = true
            if (outcomeKind === "not_found") {
                return
            }
            if (outcomeKind !== "found" || !outcome.value
                    || !Array.isArray(outcome.value.sibling_hashes)
                    || String(outcome.value.commitment_hex || "") !== normalizedCommitment
                    || !l2Context.validL2SingleSourceValue(outcome.value, sourceId, "sequencer")) {
                l2CommitmentProofError = qsTr("Commitment proof returned invalid source data.")
                return
            }
            l2CommitmentProof = outcome.value
        })
    }

    function requestL2AccountNonces(accountIds) {
        resetL2AccountNoncesState()
        const normalizedIds = normalizedL2AccountIds(accountIds)
        if (normalizedIds.length === 0) {
            l2AccountNoncesError = qsTr("Enter at least one account ID.")
            return null
        }
        if (normalizedIds.length > 100) {
            l2AccountNoncesError = qsTr("At most 100 account IDs can be requested.")
            return null
        }
        if (!l2Context.l2SequencerReadEnabled) {
            l2AccountNoncesError = qsTr("Select a Sequencer source to inspect account nonces.")
            return null
        }
        l2NonceAccountIds = normalizedIds
        l2AccountNoncesRequestRevision += 1
        const requestRevision = l2AccountNoncesRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const sourceId = l2Context.l2SequencerSourceId()
        l2AccountNoncesInFlight = true
        return l2Context.dispatch("zoneL2AccountNonces", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                account_ids: normalizedIds,
                exact_source_id: sourceId
            }
        }, function (response) {
            if (requestRevision !== l2AccountNoncesRequestRevision) {
                return
            }
            l2AccountNoncesInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.account_nonces", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2AccountNoncesError = l2Context.responseError(response,
                        qsTr("Account nonces could not be loaded."))
                    l2AccountNoncesErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            const report = response.value
            const outcome = report.data || ({})
            if (String(outcome.outcome || "") !== "found" || !outcome.value
                    || !Array.isArray(outcome.value.rows)
                    || outcome.value.rows.length !== normalizedIds.length
                    || !l2Context.validL2SingleSourceValue(outcome.value, sourceId, "sequencer")) {
                l2AccountNoncesError = qsTr("Account nonces returned invalid source data.")
                return
            }
            l2AccountNoncesReport = report
            l2AccountNonces = outcome.value.rows.slice()
            l2AccountNoncesLoaded = true
        })
    }

    function normalizedL2AccountIds(accountIds) {
        const values = Array.isArray(accountIds) ? accountIds : []
        const result = []
        for (let i = 0; i < values.length; ++i) {
            const value = String(values[i] || "").trim()
            if (value.length > 0) {
                result.push(value)
            }
        }
        return result
    }

    function refreshL2Transfers() {
        resetL2TransfersState(true)
        if (!l2Context.l2IndexerReadEnabled) {
            l2TransfersLoaded = true
            l2TransfersError = qsTr("Configure an Indexer to inspect finalized transfer windows.")
            return null
        }
        return requestL2Transfers("", false)
    }

    function loadOlderL2Transfers() {
        if (!l2Context.l2IndexerReadEnabled || l2TransfersInFlight || !l2TransfersHasMore
                || l2TransfersNextCursor.length === 0) {
            return null
        }
        return requestL2Transfers(l2TransfersNextCursor, true)
    }

    function loadNewerL2Transfers() {
        if (l2TransfersInFlight || l2TransfersHistory.length === 0) {
            return false
        }
        l2TransfersRequestRevision += 1
        const history = l2TransfersHistory.slice()
        const page = history.pop()
        l2TransfersHistory = history
        restoreL2TransfersPage(page)
        l2TransfersError = ""
        l2TransfersErrorDetails = null
        return true
    }

    function setL2TransfersLimit(limit) {
        const next = Math.max(1, Math.min(50, Math.floor(Number(limit || 25))))
        if (next === l2TransfersLimit) {
            return false
        }
        l2TransfersLimit = next
        refreshL2Transfers()
        return true
    }

    function requestL2Transfers(cursor, older) {
        if (!l2Context.l2IndexerReadEnabled || l2TransfersInFlight) {
            return null
        }
        l2TransfersRequestRevision += 1
        const requestRevision = l2TransfersRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const cursorText = String(cursor || "")
        const previousPage = older ? currentL2TransfersPage() : null
        l2TransfersInFlight = true
        l2TransfersError = ""
        l2TransfersErrorDetails = null
        return l2Context.dispatch("zoneL2Transfers", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                cursor: cursorText.length > 0 ? cursorText : null,
                block_limit: l2TransfersLimit
            }
        }, function (response) {
            if (requestRevision !== l2TransfersRequestRevision) {
                return
            }
            l2TransfersInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.transfers", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2TransfersError = l2Context.responseError(response,
                        qsTr("Transfer window could not be loaded."))
                    l2TransfersErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            const report = response.value
            const outcome = report.data || ({})
            if (String(outcome.outcome || "") !== "found" || !outcome.value
                    || !Array.isArray(outcome.value.recipients)
                    || outcome.value.finalized !== true) {
                l2TransfersError = qsTr("Transfer window returned invalid finalized data.")
                return
            }
            if (older && previousPage) {
                l2TransfersHistory = l2TransfersHistory.concat([previousPage])
            }
            applyL2TransfersPage(report, outcome.value)
        })
    }

    function currentL2TransfersPage() {
        return {
            report: l2TransfersReport,
            recipients: l2TransferRecipients.slice(),
            next_cursor: l2TransfersNextCursor,
            has_more: l2TransfersHasMore,
            newest_block: l2TransfersNewestBlock,
            oldest_block: l2TransfersOldestBlock,
            scanned_blocks: l2TransfersScannedBlocks,
            finalized: l2TransfersFinalized,
            loaded: l2TransfersLoaded
        }
    }

    function applyL2TransfersPage(report, page) {
        l2TransfersReport = report
        l2TransferRecipients = page.recipients.slice()
        l2TransfersNextCursor = String(page.next_cursor || "")
        l2TransfersHasMore = page.has_more === true
            && l2TransfersNextCursor.length > 0
        l2TransfersNewestBlock = page.newest_block === undefined
            ? null : page.newest_block
        l2TransfersOldestBlock = page.oldest_block === undefined
            ? null : page.oldest_block
        l2TransfersScannedBlocks = Number(page.scanned_blocks || 0)
        l2TransfersFinalized = page.finalized === true
        l2TransfersLoaded = true
    }

    function restoreL2TransfersPage(page) {
        l2TransfersReport = page.report || null
        l2TransferRecipients = Array.isArray(page.recipients)
            ? page.recipients.slice() : []
        l2TransfersNextCursor = String(page.next_cursor || "")
        l2TransfersHasMore = page.has_more === true
        l2TransfersNewestBlock = page.newest_block === undefined
            ? null : page.newest_block
        l2TransfersOldestBlock = page.oldest_block === undefined
            ? null : page.oldest_block
        l2TransfersScannedBlocks = Number(page.scanned_blocks || 0)
        l2TransfersFinalized = page.finalized === true
        l2TransfersLoaded = page.loaded === true
    }

    function l2ProgramEntityRef(program) {
        const value = program || null
        const source = l2ProgramsReport && l2ProgramsReport.data
            && l2ProgramsReport.data.value ? l2ProgramsReport.data.value.source : null
        return value ? l2Context.l2EntityRef("program", value.hex || value.base58, source) : null
    }
}
