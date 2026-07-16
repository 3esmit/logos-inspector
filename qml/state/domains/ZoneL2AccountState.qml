import QtQml

QtObject {
    id: root

    required property var l2Context
    readonly property bool l2Applicable: l2Context.l2Applicable
    readonly property bool l2SourceConfigured: l2Context.l2SourceConfigured
    readonly property bool l2ReadEnabled: l2Context.l2ReadEnabled
    readonly property bool l2IndexerReadEnabled: l2Context.l2IndexerReadEnabled
    readonly property bool l2SequencerReadEnabled: l2Context.l2SequencerReadEnabled
    readonly property var appModel: l2Context.appModel || null
    readonly property int registeredIdlCount: appModel && appModel.registeredIdls
        ? Number(appModel.registeredIdls.count || 0) : 0
    readonly property string l2AccountDecodeCandidateRevision: String(registeredIdlCount)
        + ":" + String(appModel ? Number(appModel.accountIdlSelectionRevision || 0) : 0)
        + ":" + String(appModel && appModel.social
            ? Number(appModel.social.sharedIdlRevision || 0) : 0)

    function l2AvailabilityMessage() {
        return l2Context.l2AvailabilityMessage()
    }

    function l2IndexerSourceId() {
        return l2Context.l2IndexerSourceId()
    }

    function l2SequencerSourceId() {
        return l2Context.l2SequencerSourceId()
    }

    property string l2AccountId: ""
    property var l2AccountFinalizedReport: null
    property var l2AccountFinalized: null
    property string l2AccountFinalizedError: ""
    property var l2AccountFinalizedErrorDetails: null
    property var l2AccountFinalizedDecode: null
    property string l2AccountFinalizedDecodeError: ""
    property bool l2AccountFinalizedDecodeInFlight: false
    property var l2AccountProvisionalReport: null
    property var l2AccountProvisional: null
    property string l2AccountProvisionalError: ""
    property var l2AccountProvisionalErrorDetails: null
    property var l2AccountProvisionalDecode: null
    property string l2AccountProvisionalDecodeError: ""
    property bool l2AccountProvisionalDecodeInFlight: false
    property var l2AccountHistoricalTarget: null
    property var l2AccountHistoricalReport: null
    property var l2AccountHistorical: null
    property string l2AccountHistoricalError: ""
    property var l2AccountHistoricalErrorDetails: null
    property var l2AccountHistoricalDecode: null
    property string l2AccountHistoricalDecodeError: ""
    property bool l2AccountHistoricalDecodeInFlight: false
    property int l2AccountActivityLimit: 25
    property var l2AccountActivityReport: null
    property string l2AccountActivityCanonicalId: ""
    property var l2AccountActivityRows: []
    property string l2AccountActivityNextCursor: ""
    property bool l2AccountActivityHasMore: false
    property bool l2AccountActivityLoaded: false
    property string l2AccountActivityError: ""
    property var l2AccountActivityErrorDetails: null
    property bool l2AccountFinalizedInFlight: false
    property bool l2AccountProvisionalInFlight: false
    property bool l2AccountHistoricalInFlight: false
    property bool l2AccountActivityInFlight: false
    property int l2AccountFinalizedRequestRevision: 0
    property int l2AccountProvisionalRequestRevision: 0
    property int l2AccountHistoricalRequestRevision: 0
    property int l2AccountActivityRequestRevision: 0
    property int l2AccountFinalizedDecodeRequestRevision: 0
    property int l2AccountProvisionalDecodeRequestRevision: 0
    property int l2AccountHistoricalDecodeRequestRevision: 0

    onL2AccountDecodeCandidateRevisionChanged: reDecodeL2AccountSnapshots()

    function resetL2AccountState(clearAccount) {
        resetL2CurrentAccountSnapshots()
        resetL2HistoricalAccountState()
        resetL2AccountActivityState(true)
        if (clearAccount) {
            l2AccountId = ""
        }
    }

    function resetL2CurrentAccountSnapshots() {
        l2AccountFinalizedRequestRevision += 1
        l2AccountProvisionalRequestRevision += 1
        l2AccountFinalizedInFlight = false
        l2AccountProvisionalInFlight = false
        l2AccountFinalizedReport = null
        l2AccountFinalized = null
        l2AccountFinalizedError = ""
        l2AccountFinalizedErrorDetails = null
        resetL2AccountSnapshotDecode("finalized")
        l2AccountProvisionalReport = null
        l2AccountProvisional = null
        l2AccountProvisionalError = ""
        l2AccountProvisionalErrorDetails = null
        resetL2AccountSnapshotDecode("provisional")
    }

    function resetL2HistoricalAccountState() {
        l2AccountHistoricalRequestRevision += 1
        l2AccountHistoricalInFlight = false
        l2AccountHistoricalTarget = null
        l2AccountHistoricalReport = null
        l2AccountHistorical = null
        l2AccountHistoricalError = ""
        l2AccountHistoricalErrorDetails = null
        resetL2AccountSnapshotDecode("historical")
    }

    function resetL2AccountActivityState(clearRows) {
        l2AccountActivityRequestRevision += 1
        l2AccountActivityInFlight = false
        l2AccountActivityReport = null
        l2AccountActivityNextCursor = ""
        l2AccountActivityHasMore = false
        l2AccountActivityError = ""
        l2AccountActivityErrorDetails = null
        if (clearRows) {
            l2AccountActivityCanonicalId = ""
            l2AccountActivityRows = []
            l2AccountActivityLoaded = false
        }
    }

    function inspectL2Account(accountId) {
        const normalizedId = String(accountId || "").trim()
        if (!l2Context.l2ReadEnabled || normalizedId.length === 0) {
            return false
        }
        resetL2AccountState(true)
        l2AccountId = normalizedId
        let dispatched = false
        if (l2Context.l2IndexerReadEnabled) {
            requestL2AccountSnapshot("finalized", { kind: "finalized" }, l2Context.l2IndexerSourceId())
            requestL2AccountActivity("", false)
            dispatched = true
        } else {
            l2AccountFinalizedError = qsTr("Configure an Indexer for finalized account state.")
            l2AccountActivityError = qsTr("Configure an Indexer for account activity.")
            l2AccountActivityLoaded = true
        }
        if (l2Context.l2SequencerReadEnabled) {
            requestL2AccountSnapshot("provisional", { kind: "provisional" }, l2Context.l2SequencerSourceId())
            dispatched = true
        } else {
            l2AccountProvisionalError = qsTr("Select a Sequencer source for provisional account state.")
        }
        return dispatched
    }

    function inspectL2SequencerAccount(accountId) {
        const normalizedId = String(accountId || "").trim()
        if (!l2Context.l2SequencerReadEnabled || normalizedId.length === 0) {
            return false
        }
        resetL2AccountState(true)
        l2AccountId = normalizedId
        requestL2AccountSnapshot("provisional", { kind: "provisional" },
            l2Context.l2SequencerSourceId())
        l2AccountActivityLoaded = true
        return true
    }

    function inspectL2AccountReference(accountId, source) {
        const qualifier = source && typeof source === "object" ? source : ({ kind: "policy" })
        if (String(qualifier.kind || "policy") !== "exact") {
            return inspectL2Account(accountId)
        }
        const normalizedId = String(accountId || "").trim()
        const sourceId = String(qualifier.source_id || "")
        const sourceRole = String(qualifier.source_role || "")
        if (!l2Context.l2ReadEnabled || normalizedId.length === 0 || sourceId.length === 0) {
            return false
        }
        resetL2AccountState(true)
        l2AccountId = normalizedId
        if (sourceRole === "indexer" && sourceId === l2Context.l2IndexerSourceId()) {
            requestL2AccountSnapshot("finalized", { kind: "finalized" }, sourceId)
            requestL2AccountActivity("", false)
            l2AccountProvisionalError = qsTr("Exact reference is qualified to the Indexer source.")
            return true
        }
        if (sourceRole === "sequencer" && sourceId === l2Context.l2SequencerSourceId()) {
            requestL2AccountSnapshot("provisional", { kind: "provisional" }, sourceId)
            l2AccountFinalizedError = qsTr("Exact reference is qualified to the Sequencer source.")
            l2AccountActivityLoaded = true
            return true
        }
        resetL2AccountState(true)
        return false
    }

    function refreshL2AccountSnapshots() {
        if (l2AccountId.length === 0 || !l2Context.l2ReadEnabled) {
            return false
        }
        resetL2CurrentAccountSnapshots()
        let dispatched = false
        if (l2Context.l2IndexerReadEnabled) {
            requestL2AccountSnapshot("finalized", { kind: "finalized" }, l2Context.l2IndexerSourceId())
            dispatched = true
        } else {
            l2AccountFinalizedError = qsTr("Configure an Indexer for finalized account state.")
        }
        if (l2Context.l2SequencerReadEnabled) {
            requestL2AccountSnapshot("provisional", { kind: "provisional" }, l2Context.l2SequencerSourceId())
            dispatched = true
        } else {
            l2AccountProvisionalError = qsTr("Select a Sequencer source for provisional account state.")
        }
        return dispatched
    }

    function refreshL2SequencerAccount() {
        if (l2AccountId.length === 0 || !l2Context.l2SequencerReadEnabled) {
            return false
        }
        resetL2CurrentAccountSnapshots()
        requestL2AccountSnapshot("provisional", { kind: "provisional" },
            l2Context.l2SequencerSourceId())
        return true
    }

    function requestL2HistoricalAccount(blockId, blockHash) {
        const normalizedBlockId = Number(blockId)
        const normalizedBlockHash = String(blockHash || "").trim()
        resetL2HistoricalAccountState()
        if (l2AccountId.length === 0) {
            l2AccountHistoricalError = qsTr("Inspect an account before requesting historical state.")
            return null
        }
        if (!Number.isFinite(normalizedBlockId) || normalizedBlockId < 0
                || Math.floor(normalizedBlockId) !== normalizedBlockId
                || normalizedBlockHash.length === 0) {
            l2AccountHistoricalError = qsTr("Historical state requires an exact block ID and hash.")
            return null
        }
        if (!l2Context.l2IndexerReadEnabled) {
            l2AccountHistoricalError = qsTr("Configure an Indexer for historical account state.")
            return null
        }
        l2AccountHistoricalTarget = {
            block_id: normalizedBlockId,
            block_hash: normalizedBlockHash
        }
        return requestL2AccountSnapshot("historical", {
            kind: "historical",
            block_id: normalizedBlockId,
            block_hash: normalizedBlockHash
        }, l2Context.l2IndexerSourceId())
    }

    function requestL2AccountSnapshot(kind, snapshot, exactSourceId) {
        if (!l2Context.l2ReadEnabled || l2AccountId.length === 0) {
            return null
        }
        const requestRevision = beginL2AccountSnapshotRequest(kind)
        if (requestRevision < 0) {
            return null
        }
        const requestContext = l2Context.l2RequestContext()
        const sourceId = String(exactSourceId || "")
        return l2Context.dispatch("zoneL2Account", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                account_id: l2AccountId,
                snapshot: snapshot,
                exact_source_id: sourceId.length > 0 ? sourceId : null
            }
        }, function (response) {
            if (requestRevision !== l2AccountSnapshotRevision(kind)) {
                return
            }
            setL2AccountSnapshotInFlight(kind, false)
            if (!l2Context.l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.account", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    setL2AccountSnapshotError(kind,
                        l2Context.responseError(response, l2AccountSnapshotFailureText(kind)),
                        response && response.error_details ? response.error_details : null)
                }
                return
            }
            const report = response.value
            setL2AccountSnapshotReport(kind, report)
            const outcome = report.data || ({})
            const outcomeKind = String(outcome.outcome || "")
            if (outcomeKind === "found" && outcome.value) {
                const expectedRole = kind === "provisional" ? "sequencer" : "indexer"
                if (!l2Context.validL2SingleSourceValue(outcome.value, sourceId, expectedRole)
                        || !outcome.value.account
                        || String(outcome.value.account.account_id || "").length === 0
                        || !validL2AccountAnchor(kind, outcome.value)) {
                    setL2AccountSnapshotError(kind,
                        qsTr("Account snapshot returned invalid source or account data."), null)
                    return
                }
                setL2AccountSnapshotValue(kind, outcome.value)
                return
            }
            if (outcomeKind === "not_found") {
                setL2AccountSnapshotError(kind, l2AccountSnapshotNotFoundText(kind), null)
                return
            }
            if (outcomeKind === "ambiguous") {
                setL2AccountSnapshotError(kind,
                    qsTr("Account snapshot requires an exact source."), null)
                return
            }
            setL2AccountSnapshotError(kind,
                qsTr("Account snapshot returned an invalid outcome."), null)
        })
    }

    function beginL2AccountSnapshotRequest(kind) {
        if (kind === "finalized") {
            l2AccountFinalizedRequestRevision += 1
            l2AccountFinalizedInFlight = true
            l2AccountFinalizedReport = null
            l2AccountFinalized = null
            l2AccountFinalizedError = ""
            l2AccountFinalizedErrorDetails = null
            resetL2AccountSnapshotDecode(kind)
            return l2AccountFinalizedRequestRevision
        }
        if (kind === "provisional") {
            l2AccountProvisionalRequestRevision += 1
            l2AccountProvisionalInFlight = true
            l2AccountProvisionalReport = null
            l2AccountProvisional = null
            l2AccountProvisionalError = ""
            l2AccountProvisionalErrorDetails = null
            resetL2AccountSnapshotDecode(kind)
            return l2AccountProvisionalRequestRevision
        }
        if (kind === "historical") {
            l2AccountHistoricalRequestRevision += 1
            l2AccountHistoricalInFlight = true
            l2AccountHistoricalReport = null
            l2AccountHistorical = null
            l2AccountHistoricalError = ""
            l2AccountHistoricalErrorDetails = null
            resetL2AccountSnapshotDecode(kind)
            return l2AccountHistoricalRequestRevision
        }
        return -1
    }

    function l2AccountSnapshotRevision(kind) {
        if (kind === "finalized") {
            return l2AccountFinalizedRequestRevision
        }
        if (kind === "provisional") {
            return l2AccountProvisionalRequestRevision
        }
        return kind === "historical" ? l2AccountHistoricalRequestRevision : -1
    }

    function setL2AccountSnapshotInFlight(kind, value) {
        if (kind === "finalized") {
            l2AccountFinalizedInFlight = value
        } else if (kind === "provisional") {
            l2AccountProvisionalInFlight = value
        } else if (kind === "historical") {
            l2AccountHistoricalInFlight = value
        }
    }

    function setL2AccountSnapshotReport(kind, report) {
        if (kind === "finalized") {
            l2AccountFinalizedReport = report
        } else if (kind === "provisional") {
            l2AccountProvisionalReport = report
        } else if (kind === "historical") {
            l2AccountHistoricalReport = report
        }
    }

    function setL2AccountSnapshotValue(kind, value) {
        if (kind === "finalized") {
            l2AccountFinalized = value
        } else if (kind === "provisional") {
            l2AccountProvisional = value
        } else if (kind === "historical") {
            l2AccountHistorical = value
        }
        decodeL2AccountSnapshot(kind)
    }

    function l2AccountSnapshotValue(kind) {
        if (kind === "finalized") {
            return l2AccountFinalized
        }
        if (kind === "provisional") {
            return l2AccountProvisional
        }
        return kind === "historical" ? l2AccountHistorical : null
    }

    function resetL2AccountSnapshotDecode(kind) {
        if (kind === "finalized") {
            l2AccountFinalizedDecodeRequestRevision += 1
            l2AccountFinalizedDecodeInFlight = false
            l2AccountFinalizedDecode = null
            l2AccountFinalizedDecodeError = ""
        } else if (kind === "provisional") {
            l2AccountProvisionalDecodeRequestRevision += 1
            l2AccountProvisionalDecodeInFlight = false
            l2AccountProvisionalDecode = null
            l2AccountProvisionalDecodeError = ""
        } else if (kind === "historical") {
            l2AccountHistoricalDecodeRequestRevision += 1
            l2AccountHistoricalDecodeInFlight = false
            l2AccountHistoricalDecode = null
            l2AccountHistoricalDecodeError = ""
        }
    }

    function l2AccountDecodeRevision(kind) {
        if (kind === "finalized") {
            return l2AccountFinalizedDecodeRequestRevision
        }
        if (kind === "provisional") {
            return l2AccountProvisionalDecodeRequestRevision
        }
        return kind === "historical" ? l2AccountHistoricalDecodeRequestRevision : -1
    }

    function setL2AccountDecodeInFlight(kind, value) {
        if (kind === "finalized") {
            l2AccountFinalizedDecodeInFlight = value
        } else if (kind === "provisional") {
            l2AccountProvisionalDecodeInFlight = value
        } else if (kind === "historical") {
            l2AccountHistoricalDecodeInFlight = value
        }
    }

    function setL2AccountSnapshotDecode(kind, value) {
        if (kind === "finalized") {
            l2AccountFinalizedDecode = value
        } else if (kind === "provisional") {
            l2AccountProvisionalDecode = value
        } else if (kind === "historical") {
            l2AccountHistoricalDecode = value
        }
    }

    function setL2AccountSnapshotDecodeError(kind, message) {
        if (kind === "finalized") {
            l2AccountFinalizedDecodeError = String(message || "")
        } else if (kind === "provisional") {
            l2AccountProvisionalDecodeError = String(message || "")
        } else if (kind === "historical") {
            l2AccountHistoricalDecodeError = String(message || "")
        }
    }

    function snapshotMatchesDecodeInput(kind, accountId, ownerProgramId, dataHex) {
        const snapshot = l2AccountSnapshotValue(kind)
        const account = snapshot && snapshot.account ? snapshot.account : null
        return account !== null
            && String(account.account_id || account.account_id_base58 || "") === accountId
            && String(account.owner_program_hex || "") === ownerProgramId
            && String(account.data_hex || "") === dataHex
    }

    function decodeL2AccountSnapshot(kind) {
        resetL2AccountSnapshotDecode(kind)
        const model = appModel
        const snapshot = l2AccountSnapshotValue(kind)
        const account = snapshot && snapshot.account ? snapshot.account : null
        if (!model || !account || typeof model.accountDecodeCandidates !== "function"
                || typeof model.programDecodeCandidatePayload !== "function"
                || typeof model.selectAccountDecodeSessionAsync !== "function") {
            return null
        }
        const accountId = String(account.account_id || account.account_id_base58 || "")
        const ownerProgramId = String(account.owner_program_hex || "")
        const dataHex = String(account.data_hex || "")
        if (accountId.length === 0 || ownerProgramId.length === 0 || dataHex.length === 0) {
            return null
        }
        const candidates = model.accountDecodeCandidates(accountId, ownerProgramId)
        if (!Array.isArray(candidates) || candidates.length === 0) {
            return null
        }
        const candidatePayload = model.programDecodeCandidatePayload(candidates)
        if (!Array.isArray(candidatePayload) || candidatePayload.length === 0) {
            return null
        }
        const requestRevision = l2AccountDecodeRevision(kind)
        setL2AccountDecodeInFlight(kind, true)
        return model.selectAccountDecodeSessionAsync(dataHex, accountId, ownerProgramId,
            candidatePayload, function (response) {
                if (requestRevision !== l2AccountDecodeRevision(kind)) {
                    return
                }
                setL2AccountDecodeInFlight(kind, false)
                if (!snapshotMatchesDecodeInput(kind, accountId, ownerProgramId, dataHex)) {
                    return
                }
                const session = response && response.ok === true && response.value
                    ? response.value : null
                const selected = session && session.selected ? session.selected : null
                if (selected && selected.report) {
                    setL2AccountSnapshotDecode(kind, {
                        evidence: selected.evidence || ({}),
                        report: selected.report
                    })
                    return
                }
                setL2AccountSnapshotDecodeError(kind,
                    String(session && (session.firstError || session.first_error)
                        || response && response.error || qsTr("Registered IDL did not decode this account.")))
            })
    }

    function reDecodeL2AccountSnapshots() {
        if (l2AccountFinalized !== null) {
            decodeL2AccountSnapshot("finalized")
        }
        if (l2AccountProvisional !== null) {
            decodeL2AccountSnapshot("provisional")
        }
        if (l2AccountHistorical !== null) {
            decodeL2AccountSnapshot("historical")
        }
    }

    function setL2AccountSnapshotError(kind, message, details) {
        if (kind === "finalized") {
            l2AccountFinalizedError = String(message || "")
            l2AccountFinalizedErrorDetails = details
        } else if (kind === "provisional") {
            l2AccountProvisionalError = String(message || "")
            l2AccountProvisionalErrorDetails = details
        } else if (kind === "historical") {
            l2AccountHistoricalError = String(message || "")
            l2AccountHistoricalErrorDetails = details
        }
    }

    function l2AccountSnapshotFailureText(kind) {
        if (kind === "finalized") {
            return qsTr("Finalized account snapshot could not be loaded.")
        }
        if (kind === "provisional") {
            return qsTr("Provisional account snapshot could not be loaded.")
        }
        return qsTr("Historical account snapshot could not be loaded.")
    }

    function l2AccountSnapshotNotFoundText(kind) {
        if (kind === "finalized") {
            return qsTr("Finalized account snapshot was not found.")
        }
        if (kind === "provisional") {
            return qsTr("Provisional account snapshot was not found.")
        }
        return qsTr("Historical account snapshot was not found at the exact block.")
    }

    function refreshL2AccountActivity() {
        if (l2AccountId.length === 0 || !l2Context.l2IndexerReadEnabled) {
            return false
        }
        resetL2AccountActivityState(true)
        return requestL2AccountActivity("", false) !== null
    }

    function loadMoreL2AccountActivity() {
        if (!l2Context.l2IndexerReadEnabled || l2AccountActivityInFlight
                || !l2AccountActivityHasMore
                || l2AccountActivityNextCursor.length === 0) {
            return false
        }
        return requestL2AccountActivity(l2AccountActivityNextCursor, true) !== null
    }

    function setL2AccountActivityLimit(limit) {
        const next = Math.max(1, Math.min(50, Math.floor(Number(limit || 25))))
        if (next === l2AccountActivityLimit) {
            return false
        }
        l2AccountActivityLimit = next
        if (l2AccountId.length > 0) {
            refreshL2AccountActivity()
        }
        return true
    }

    function requestL2AccountActivity(cursor, append) {
        if (!l2Context.l2IndexerReadEnabled || l2AccountActivityInFlight
                || l2AccountId.length === 0) {
            return null
        }
        l2AccountActivityRequestRevision += 1
        const requestRevision = l2AccountActivityRequestRevision
        const requestContext = l2Context.l2RequestContext()
        const cursorText = String(cursor || "")
        const requestedAccountId = l2AccountId
        l2AccountActivityInFlight = true
        l2AccountActivityError = ""
        l2AccountActivityErrorDetails = null
        return l2Context.dispatch("zoneL2AccountActivity", {
            context: requestContext,
            request_revision: requestRevision,
            query: {
                account_id: requestedAccountId,
                cursor: cursorText.length > 0 ? cursorText : null,
                limit: l2AccountActivityLimit,
                order: "oldest_first"
            }
        }, function (response) {
            if (requestRevision !== l2AccountActivityRequestRevision) {
                return
            }
            l2AccountActivityInFlight = false
            if (!l2Context.l2RequestContextIsCurrent(requestContext)
                    || requestedAccountId !== l2AccountId) {
                return
            }
            if (!l2Context.validL2ReportResponse(response, "lez.account_activity", requestRevision)) {
                if (l2Context.acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2AccountActivityError = l2Context.responseError(response,
                        qsTr("Account activity could not be loaded."))
                    l2AccountActivityErrorDetails = response && response.error_details
                        ? response.error_details : null
                }
                return
            }
            const report = response.value
            const outcome = report.data || ({})
            if (String(outcome.outcome || "") !== "found" || !outcome.value
                    || !Array.isArray(outcome.value.rows)
                    || String(outcome.value.order || "") !== "oldest_first"
                    || String(outcome.value.account_id || "").length === 0) {
                l2AccountActivityError = qsTr("Account activity returned an invalid page.")
                return
            }
            const page = outcome.value
            const canonicalId = String(page.account_id)
            if (append && canonicalId !== l2AccountActivityCanonicalId) {
                l2AccountActivityError = qsTr("Account activity cursor belongs to another account.")
                return
            }
            l2AccountActivityReport = report
            l2AccountActivityCanonicalId = canonicalId
            l2AccountActivityRows = append
                ? l2AccountActivityRows.concat(page.rows) : page.rows.slice()
            l2AccountActivityNextCursor = String(page.next_cursor || "")
            l2AccountActivityHasMore = page.has_more === true
                && l2AccountActivityNextCursor.length > 0
            l2AccountActivityLoaded = true
        })
    }

    function validL2AccountAnchor(kind, value) {
        const anchor = value && value.anchor ? value.anchor : null
        const anchorState = String(value && value.anchor_state || "")
        if (!anchor || !Number.isFinite(Number(anchor.block_id))
                || String(anchor.block_hash || "").length === 0) {
            return false
        }
        if (kind !== "provisional") {
            if (anchorState !== "exact" || value.after_anchor !== null) {
                return false
            }
            if (kind === "historical" && l2AccountHistoricalTarget) {
                return Number(anchor.block_id)
                        === Number(l2AccountHistoricalTarget.block_id)
                    && String(anchor.block_hash || "")
                        === String(l2AccountHistoricalTarget.block_hash || "")
            }
            return true
        }
        if (anchorState === "exact") {
            return value.after_anchor === null
        }
        return anchorState === "moving" && value.after_anchor
            && Number.isFinite(Number(value.after_anchor.block_id))
            && String(value.after_anchor.block_hash || "").length > 0
    }

    function l2AccountEntityRef(snapshot) {
        const value = snapshot || l2AccountFinalized || l2AccountProvisional
        const account = value && value.account ? value.account : null
        return account ? l2Context.l2EntityRef("account", account.account_id
            || account.account_id_base58 || account.account_id_hex, value.source) : null
    }
}
