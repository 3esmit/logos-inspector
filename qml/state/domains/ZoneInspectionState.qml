import QtQml

QtObject {
    id: root

    required property var gateway
    property var sourceDescriptor: null
    property bool started: false

    property bool catalogConfigured: false
    property double sourceRevision: 0
    property var catalogStatus: null
    property string verification: "empty"
    property var coverage: ({})
    property var ingestion: ({})
    property string currentError: ""
    property string configureError: ""
    property string statusError: ""
    property string summaryError: ""
    property string detailError: ""
    property string controlError: ""
    property string sourceMutationError: ""
    property var sourceMutationWarning: null
    property string evidenceError: ""
    property string evidenceDetailError: ""
    property string evidencePayloadError: ""

    property var networkScope: null
    property string networkScopeKey: ""
    property double catalogRevision: 0
    property double sourceConfigEpoch: 0
    property double observationRevision: 0
    property double summaryRevision: 0
    property var zoneSummaries: []
    property bool summaryLoaded: false
    property bool summaryStale: false

    property var activeZoneContext: null
    readonly property string activeZoneId: activeZoneContext
        ? String(activeZoneContext.channel_id || "")
        : ""
    property double contextRevision: 0
    property var zoneDetailReport: null
    property var zoneDetail: null
    property bool detailStale: false
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

    property string l2AccountId: ""
    property var l2AccountFinalizedReport: null
    property var l2AccountFinalized: null
    property string l2AccountFinalizedError: ""
    property var l2AccountFinalizedErrorDetails: null
    property var l2AccountProvisionalReport: null
    property var l2AccountProvisional: null
    property string l2AccountProvisionalError: ""
    property var l2AccountProvisionalErrorDetails: null
    property var l2AccountHistoricalTarget: null
    property var l2AccountHistoricalReport: null
    property var l2AccountHistorical: null
    property string l2AccountHistoricalError: ""
    property var l2AccountHistoricalErrorDetails: null
    property int l2AccountActivityLimit: 25
    property var l2AccountActivityReport: null
    property string l2AccountActivityCanonicalId: ""
    property var l2AccountActivityRows: []
    property string l2AccountActivityNextCursor: ""
    property bool l2AccountActivityHasMore: false
    property bool l2AccountActivityLoaded: false
    property string l2AccountActivityError: ""
    property var l2AccountActivityErrorDetails: null

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

    property bool configureInFlight: false
    property bool statusInFlight: false
    property bool summaryInFlight: false
    property bool detailInFlight: false
    property bool controlInFlight: false
    property bool sourceMutationInFlight: false
    property bool evidenceInFlight: false
    property bool evidenceDetailInFlight: false
    property bool evidencePayloadInFlight: false
    property bool l2BlocksInFlight: false
    property bool l2BlockDetailInFlight: false
    property bool l2TransactionDetailInFlight: false
    property bool l2TransactionTraceInFlight: false
    property bool l2AccountFinalizedInFlight: false
    property bool l2AccountProvisionalInFlight: false
    property bool l2AccountHistoricalInFlight: false
    property bool l2AccountActivityInFlight: false
    property bool l2ProgramsInFlight: false
    property bool l2CommitmentProofInFlight: false
    property bool l2AccountNoncesInFlight: false
    property bool l2TransfersInFlight: false
    property int statusFailureCount: 0

    readonly property bool statusPollingEnabled: started
        && catalogConfigured
        && desiredSourceKey.length > 0
    readonly property bool catalogBusy: controlInFlight
        || verification !== "verified"
        || String(coverage && coverage.status || "") === "rebuilding"
        || (ingestion && ingestion.worker_running === true)
        || currentError.length > 0
    readonly property int statusPollInterval: statusFailureCount > 0
        ? failureBackoffInterval(statusFailureCount)
        : (catalogBusy ? 1000 : 5000)

    property var desiredSource: null
    property string desiredSourceKey: ""
    property int sourceGeneration: 0
    property int configureRequestRevision: 0
    property int statusRequestRevision: 0
    property int activeStatusRequestRevision: 0
    property int statusAcceptanceRevision: 0
    property int summaryRequestRevision: 0
    property var summaryAssembly: null
    property double summarySourceRevision: 0
    property string summaryNetworkScopeKey: ""
    property double summaryCatalogRevision: 0
    property double summarySourceConfigEpoch: 0
    property double summaryObservationRevision: 0
    property int detailRequestRevision: 0
    property int controlRequestRevision: 0
    property int sourceMutationRequestRevision: 0
    property int evidenceRequestRevision: 0
    property int evidenceDetailRequestRevision: 0
    property int evidencePayloadRequestRevision: 0
    property int l2BlocksRequestRevision: 0
    property int l2BlockDetailRequestRevision: 0
    property int l2TransactionDetailRequestRevision: 0
    property int l2TransactionTraceRequestRevision: 0
    property int l2AccountFinalizedRequestRevision: 0
    property int l2AccountProvisionalRequestRevision: 0
    property int l2AccountHistoricalRequestRevision: 0
    property int l2AccountActivityRequestRevision: 0
    property int l2ProgramsRequestRevision: 0
    property int l2CommitmentProofRequestRevision: 0
    property int l2AccountNoncesRequestRevision: 0
    property int l2TransfersRequestRevision: 0

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

    signal statusRefreshRequested()
    signal sourceMutationFinished(var response)

    onSourceDescriptorChanged: {
        if (started) {
            syncCatalogSource()
        }
    }

    onActiveZoneContextChanged: resetL2InspectionState()

    function start() {
        if (started) {
            appResumed()
            return
        }
        started = true
        syncCatalogSource()
    }

    function stop() {
        if (!started) {
            return
        }
        started = false
        sourceGeneration += 1
        catalogConfigured = false
        sourceRevision = 0
        invalidateCatalogState(true)
    }

    function appResumed() {
        if (!started) {
            return false
        }
        syncCatalogSource()
        if (catalogConfigured) {
            statusRefreshRequested()
            return true
        }
        beginConfigure()
        return false
    }

    function normalizedSource(value) {
        if (!value || typeof value !== "object") {
            return null
        }
        const kind = String(value.kind || "")
        const endpoint = String(value.endpoint || "").trim()
        if (kind !== "direct_http" || endpoint.length === 0) {
            return null
        }
        return {
            kind: kind,
            endpoint: endpoint
        }
    }

    function sourceKey(value) {
        return value ? String(value.kind || "") + "\n" + String(value.endpoint || "") : ""
    }

    function syncCatalogSource() {
        const nextSource = normalizedSource(sourceDescriptor)
        const nextKey = sourceKey(nextSource)
        if (nextKey === desiredSourceKey) {
            if (started && nextSource && !catalogConfigured && !configureInFlight) {
                beginConfigure()
            }
            return false
        }

        desiredSource = nextSource
        desiredSourceKey = nextKey
        sourceGeneration += 1
        catalogConfigured = false
        sourceRevision = 0
        configureError = ""
        invalidateCatalogState(true)
        if (started && nextSource) {
            beginConfigure()
        }
        return true
    }

    function beginConfigure() {
        if (!started || !desiredSource || configureInFlight) {
            return null
        }

        configureInFlight = true
        configureRequestRevision += 1
        const requestRevision = configureRequestRevision
        const generation = sourceGeneration
        const key = desiredSourceKey
        return dispatch("zoneCatalogConfigure", {
            source: desiredSource
        }, function (response) {
            if (requestRevision !== configureRequestRevision) {
                return
            }
            configureInFlight = false
            if (generation !== sourceGeneration || key !== desiredSourceKey) {
                if (started && desiredSource) {
                    Qt.callLater(root.beginConfigure)
                }
                return
            }
            if (!validReportResponse(response, "zones.catalog_configured")) {
                configureError = responseError(response, qsTr("Zone Catalog configuration failed."))
                catalogConfigured = false
                return
            }
            const revision = numericRevision(response.value.source_revision)
            if (revision <= 0) {
                configureError = qsTr("Zone Catalog configuration returned an invalid source revision.")
                catalogConfigured = false
                return
            }
            sourceRevision = revision
            catalogConfigured = true
            configureError = ""
            statusFailureCount = 0
            statusRefreshRequested()
        })
    }

    function pollStatus() {
        if (!statusPollingEnabled || statusInFlight) {
            return false
        }

        statusInFlight = true
        statusRequestRevision += 1
        activeStatusRequestRevision = statusRequestRevision
        const requestRevision = activeStatusRequestRevision
        const generation = sourceGeneration
        const requestedSourceRevision = sourceRevision
        dispatch("zoneCatalogStatus", {}, function (response) {
            if (requestRevision !== activeStatusRequestRevision) {
                return
            }
            statusInFlight = false
            if (generation !== sourceGeneration
                    || requestedSourceRevision !== sourceRevision
                    || !statusPollingEnabled) {
                if (statusPollingEnabled) {
                    statusRefreshRequested()
                }
                return
            }
            if (!validReportResponse(response, "zones.catalog_status")) {
                recordStatusFailure(responseError(response, qsTr("Zone Catalog status failed.")))
                return
            }
            const report = response.value
            if (numericRevision(report.source_revision) !== sourceRevision) {
                recordStatusFailure(qsTr("Zone Catalog status belongs to another source revision."))
                return
            }
            statusFailureCount = 0
            statusError = ""
            acceptStatus(report)
        })
        return true
    }

    function recordStatusFailure(error) {
        statusFailureCount = Math.min(4, statusFailureCount + 1)
        statusError = String(error || "")
    }

    function acceptStatus(report) {
        const nextScope = report.network_scope || null
        const nextScopeKey = scopeKey(nextScope)
        const scopeChanged = networkScopeKey.length > 0 && nextScopeKey !== networkScopeKey
        const catalogChanged = catalogStatus !== null
            && numericRevision(report.catalog_revision) !== catalogRevision
        if (scopeChanged) {
            clearActiveZone()
            invalidateSummary(true)
        } else if (catalogChanged) {
            resetEvidenceState(true)
        }
        if (String(report.verification || "") !== "verified") {
            clearActiveZone()
        }

        networkScope = nextScope
        networkScopeKey = nextScopeKey
        catalogRevision = numericRevision(report.catalog_revision)
        sourceConfigEpoch = numericRevision(report.source_config_epoch)
        observationRevision = numericRevision(report.observation_revision)
        verification = String(report.verification || "empty")
        coverage = report.coverage && typeof report.coverage === "object" ? report.coverage : ({})
        ingestion = report.ingestion && typeof report.ingestion === "object" ? report.ingestion : ({})
        currentError = String(report.current_error || "")
        catalogStatus = report
        statusAcceptanceRevision += 1

        if (verification === "verified") {
            reconcileSummaries()
        } else {
            summaryStale = summaryLoaded
        }
    }

    function reconcileSummaries() {
        if (!catalogStatus || verification !== "verified") {
            return false
        }
        if (summaryMatchesStatus()) {
            summaryStale = false
            reconcileDetail()
            return true
        }
        if (summaryInFlight) {
            summaryStale = summaryLoaded
            return false
        }
        if (summaryLoaded
                && summarySourceRevision === sourceRevision
                && summaryNetworkScopeKey === networkScopeKey
                && numericRevision(catalogStatus.summary_revision) < summaryRevision) {
            statusRefreshRequested()
            return false
        }
        beginSummaryAssembly()
        return false
    }

    function beginSummaryAssembly() {
        if (!catalogStatus || summaryInFlight || verification !== "verified") {
            return null
        }

        summaryRequestRevision += 1
        const sameIdentity = summaryLoaded
            && summarySourceRevision === sourceRevision
            && summaryNetworkScopeKey === networkScopeKey
        summaryAssembly = {
            request_revision: summaryRequestRevision,
            source_generation: sourceGeneration,
            source_revision: sourceRevision,
            network_scope: networkScope,
            network_scope_key: networkScopeKey,
            after_summary_revision: sameIdentity ? summaryRevision : null,
            base_summary_revision: sameIdentity ? summaryRevision : 0,
            target_summary_revision: numericRevision(catalogStatus.summary_revision),
            kind: "",
            report: null,
            rows: [],
            upserts: [],
            removed_zone_ids: [],
            seen_cursors: ({})
        }
        summaryInFlight = true
        summaryStale = summaryLoaded
        summaryError = ""
        return requestSummaryPage("")
    }

    function requestSummaryPage(cursor) {
        const assembly = summaryAssembly
        if (!assembly || assembly.request_revision !== summaryRequestRevision) {
            return null
        }
        const cursorText = String(cursor || "")
        return dispatch("zonesSummary", {
            source_revision: assembly.source_revision,
            network_scope: assembly.network_scope,
            after_summary_revision: assembly.after_summary_revision,
            cursor: cursorText.length > 0 ? cursorText : null,
            limit: 200
        }, function (response) {
            handleSummaryResponse(assembly.request_revision, response)
        })
    }

    function handleSummaryResponse(requestRevision, response) {
        const assembly = summaryAssembly
        if (!assembly || requestRevision !== summaryRequestRevision
                || requestRevision !== assembly.request_revision) {
            return
        }
        if (assembly.source_generation !== sourceGeneration
                || assembly.source_revision !== sourceRevision
                || assembly.network_scope_key !== networkScopeKey) {
            failSummary("")
            return
        }
        if (!validReportResponse(response, "zones.summary")) {
            failSummary(responseError(response, qsTr("Zone summaries failed.")))
            return
        }

        const report = response.value
        if (!summaryReportMatchesAssembly(report, assembly)) {
            failSummary(qsTr("Zone summaries failed revision validation."))
            return
        }
        const changes = report.changes
        const kind = String(changes && changes.kind || "")
        if (assembly.kind.length === 0) {
            if (kind !== "reset" && kind !== "delta") {
                failSummary(qsTr("Zone summaries returned an unknown change set."))
                return
            }
            if (numericRevision(report.summary_revision) < assembly.target_summary_revision) {
                failSummary(qsTr("Zone summaries returned an older revision."))
                return
            }
            assembly.kind = kind
            assembly.report = report
        } else if (kind !== assembly.kind || !sameSummarySnapshot(report, assembly.report)) {
            failSummary(qsTr("Zone summary pages do not belong to one snapshot."))
            return
        }

        if (!appendSummaryChanges(assembly, changes)) {
            failSummary(qsTr("Zone summaries returned malformed rows."))
            return
        }
        const nextCursor = String(report.next_cursor || "")
        if (nextCursor.length > 0) {
            if (assembly.seen_cursors[nextCursor] === true) {
                failSummary(qsTr("Zone summaries repeated a page cursor."))
                return
            }
            assembly.seen_cursors[nextCursor] = true
            requestSummaryPage(nextCursor)
            return
        }
        commitSummaryAssembly(assembly)
    }

    function appendSummaryChanges(assembly, changes) {
        const rows = assembly.kind === "reset" ? changes.rows : changes.upserts
        if (!Array.isArray(rows)) {
            return false
        }
        for (let i = 0; i < rows.length; ++i) {
            if (!rows[i] || typeof rows[i] !== "object"
                    || String(rows[i].channel_id || "").length === 0) {
                return false
            }
            if (assembly.kind === "reset") {
                assembly.rows.push(rows[i])
            } else {
                assembly.upserts.push(rows[i])
            }
        }
        if (assembly.kind === "delta") {
            if (!Array.isArray(changes.removed_zone_ids)) {
                return false
            }
            for (let j = 0; j < changes.removed_zone_ids.length; ++j) {
                const channelId = String(changes.removed_zone_ids[j] || "")
                if (channelId.length === 0) {
                    return false
                }
                assembly.removed_zone_ids.push(channelId)
            }
        }
        return true
    }

    function commitSummaryAssembly(assembly) {
        if (!assembly || assembly.request_revision !== summaryRequestRevision
                || assembly.source_generation !== sourceGeneration) {
            return
        }
        let rows = []
        if (assembly.kind === "reset") {
            rows = rowsFromMap(rowsByChannel(assembly.rows))
        } else {
            if (assembly.base_summary_revision !== summaryRevision
                    || summarySourceRevision !== assembly.source_revision
                    || summaryNetworkScopeKey !== assembly.network_scope_key) {
                failSummary(qsTr("Zone summary delta no longer matches the visible model."))
                return
            }
            const byChannel = rowsByChannel(zoneSummaries)
            for (let i = 0; i < assembly.removed_zone_ids.length; ++i) {
                delete byChannel[assembly.removed_zone_ids[i]]
            }
            for (let j = 0; j < assembly.upserts.length; ++j) {
                const row = assembly.upserts[j]
                byChannel[String(row.channel_id)] = row
            }
            rows = rowsFromMap(byChannel)
        }

        const report = assembly.report
        if (activeZoneId.length > 0) {
            const nextActiveRow = rowFromRows(rows, activeZoneId)
            if (!nextActiveRow) {
                clearActiveZone()
            } else {
                updateActiveContextFromSummary(nextActiveRow)
                detailStale = true
            }
        }

        summaryLoaded = true
        summarySourceRevision = numericRevision(report.source_revision)
        summaryNetworkScopeKey = scopeKey(report.network_scope)
        summaryCatalogRevision = numericRevision(report.catalog_revision)
        summarySourceConfigEpoch = numericRevision(report.source_config_epoch)
        summaryObservationRevision = numericRevision(report.observation_revision)
        summaryRevision = numericRevision(report.summary_revision)
        summaryInFlight = false
        summaryAssembly = null
        summaryError = ""
        zoneSummaries = rows

        summaryStale = !summaryMatchesStatus()
        if (summaryStale) {
            reconcileSummaries()
        } else {
            reconcileDetail()
        }
    }

    function failSummary(error) {
        summaryRequestRevision += 1
        summaryInFlight = false
        summaryAssembly = null
        summaryStale = summaryLoaded
        if (String(error || "").length > 0) {
            summaryError = String(error)
        }
    }

    function activateZone(channelId) {
        const normalizedId = String(channelId || "")
        if (verification !== "verified" || networkScopeKey.length === 0) {
            return false
        }
        const row = zoneSummary(normalizedId)
        if (!row) {
            return false
        }
        if (activeZoneId === normalizedId) {
            reconcileDetail()
            return true
        }

        resetDetailState()
        contextRevision += 1
        activeZoneContext = contextFromSummary(row, 0, contextRevision)
        detailStale = true
        reconcileDetail()
        return true
    }

    function clearActiveZone() {
        if (!activeZoneContext && !zoneDetailReport && !detailInFlight) {
            return false
        }
        resetDetailState()
        contextRevision += 1
        activeZoneContext = null
        return true
    }

    function resetDetailState() {
        resetEvidenceState(true)
        detailRequestRevision += 1
        detailInFlight = false
        zoneDetailReport = null
        zoneDetail = null
        detailError = ""
        detailStale = false
    }

    function resetL2InspectionState() {
        resetL2BlocksState(true)
        resetL2BlockInspectionState()
        resetL2AccountState(true)
        resetL2ProgramsState()
        resetL2CommitmentProofState()
        resetL2AccountNoncesState()
        resetL2TransfersState(true)
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
        l2AccountProvisionalReport = null
        l2AccountProvisional = null
        l2AccountProvisionalError = ""
        l2AccountProvisionalErrorDetails = null
    }

    function resetL2HistoricalAccountState() {
        l2AccountHistoricalRequestRevision += 1
        l2AccountHistoricalInFlight = false
        l2AccountHistoricalTarget = null
        l2AccountHistoricalReport = null
        l2AccountHistorical = null
        l2AccountHistoricalError = ""
        l2AccountHistoricalErrorDetails = null
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

    function refreshL2Blocks() {
        resetL2BlocksState(true)
        resetL2BlockInspectionState()
        if (!l2ReadEnabled) {
            l2BlocksLoaded = true
            l2BlocksError = l2AvailabilityMessage()
            return null
        }
        return requestL2Blocks("", false)
    }

    function loadMoreL2Blocks() {
        if (!l2ReadEnabled || l2BlocksInFlight || !l2BlocksHasMore
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
        if (!l2ReadEnabled || l2BlocksInFlight) {
            return null
        }
        l2BlocksRequestRevision += 1
        const requestRevision = l2BlocksRequestRevision
        const requestContext = l2RequestContext()
        const cursorText = String(cursor || "")
        l2BlocksInFlight = true
        l2BlocksError = ""
        l2BlocksErrorDetails = null
        return dispatch("zoneL2Blocks", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.blocks", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2BlocksError = responseError(response, qsTr("L2 blocks could not be loaded."))
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
        if (!l2ReadEnabled) {
            return null
        }
        resetL2BlockInspectionState()
        l2BlockTarget = target
        l2BlockDetailRequestRevision += 1
        const requestRevision = l2BlockDetailRequestRevision
        const requestContext = l2RequestContext()
        const sourceId = String(exactSourceId || "")
        l2BlockRequestedSourceId = sourceId
        l2BlockDetailInFlight = true
        return dispatch("zoneL2BlockDetail", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.block_detail", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2BlockDetailError = responseError(response, qsTr("L2 block detail could not be loaded."))
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
        if (!l2ReadEnabled || normalizedId.length === 0) {
            return null
        }
        resetL2TransactionInspectionState()
        l2TransactionId = normalizedId
        l2TransactionDetailRequestRevision += 1
        const requestRevision = l2TransactionDetailRequestRevision
        const requestContext = l2RequestContext()
        const sourceId = String(exactSourceId || "")
        l2TransactionRequestedSourceId = sourceId
        l2TransactionDetailInFlight = true
        return dispatch("zoneL2Transaction", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.transaction", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2TransactionDetailError = responseError(response, qsTr("L2 transaction could not be loaded."))
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
        if (!l2ReadEnabled || normalizedId.length === 0) {
            return null
        }
        resetL2TransactionTraceState()
        l2TransactionTraceRequestRevision += 1
        const requestRevision = l2TransactionTraceRequestRevision
        const requestContext = l2RequestContext()
        const sourceId = String(exactSourceId || "")
        const programId = String(idlProgramId || "")
        l2TransactionTraceInFlight = true
        return dispatch("zoneL2TransactionTrace", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.transaction_trace", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2TransactionTraceError = responseError(response, qsTr("Transaction trace could not be derived."))
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

    function inspectL2Account(accountId) {
        const normalizedId = String(accountId || "").trim()
        if (!l2ReadEnabled || normalizedId.length === 0) {
            return false
        }
        resetL2AccountState(true)
        l2AccountId = normalizedId
        let dispatched = false
        if (l2IndexerReadEnabled) {
            requestL2AccountSnapshot("finalized", { kind: "finalized" }, l2IndexerSourceId())
            requestL2AccountActivity("", false)
            dispatched = true
        } else {
            l2AccountFinalizedError = qsTr("Configure an Indexer for finalized account state.")
            l2AccountActivityError = qsTr("Configure an Indexer for account activity.")
            l2AccountActivityLoaded = true
        }
        if (l2SequencerReadEnabled) {
            requestL2AccountSnapshot("provisional", { kind: "provisional" }, l2SequencerSourceId())
            dispatched = true
        } else {
            l2AccountProvisionalError = qsTr("Select a Sequencer source for provisional account state.")
        }
        return dispatched
    }

    function refreshL2AccountSnapshots() {
        if (l2AccountId.length === 0 || !l2ReadEnabled) {
            return false
        }
        resetL2CurrentAccountSnapshots()
        let dispatched = false
        if (l2IndexerReadEnabled) {
            requestL2AccountSnapshot("finalized", { kind: "finalized" }, l2IndexerSourceId())
            dispatched = true
        } else {
            l2AccountFinalizedError = qsTr("Configure an Indexer for finalized account state.")
        }
        if (l2SequencerReadEnabled) {
            requestL2AccountSnapshot("provisional", { kind: "provisional" }, l2SequencerSourceId())
            dispatched = true
        } else {
            l2AccountProvisionalError = qsTr("Select a Sequencer source for provisional account state.")
        }
        return dispatched
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
        if (!l2IndexerReadEnabled) {
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
        }, l2IndexerSourceId())
    }

    function requestL2AccountSnapshot(kind, snapshot, exactSourceId) {
        if (!l2ReadEnabled || l2AccountId.length === 0) {
            return null
        }
        const requestRevision = beginL2AccountSnapshotRequest(kind)
        if (requestRevision < 0) {
            return null
        }
        const requestContext = l2RequestContext()
        const sourceId = String(exactSourceId || "")
        return dispatch("zoneL2Account", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.account", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    setL2AccountSnapshotError(kind,
                        responseError(response, l2AccountSnapshotFailureText(kind)),
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
                if (!validL2SingleSourceValue(outcome.value, sourceId, expectedRole)
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
            return l2AccountFinalizedRequestRevision
        }
        if (kind === "provisional") {
            l2AccountProvisionalRequestRevision += 1
            l2AccountProvisionalInFlight = true
            l2AccountProvisionalReport = null
            l2AccountProvisional = null
            l2AccountProvisionalError = ""
            l2AccountProvisionalErrorDetails = null
            return l2AccountProvisionalRequestRevision
        }
        if (kind === "historical") {
            l2AccountHistoricalRequestRevision += 1
            l2AccountHistoricalInFlight = true
            l2AccountHistoricalReport = null
            l2AccountHistorical = null
            l2AccountHistoricalError = ""
            l2AccountHistoricalErrorDetails = null
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
        if (l2AccountId.length === 0 || !l2IndexerReadEnabled) {
            return false
        }
        resetL2AccountActivityState(true)
        return requestL2AccountActivity("", false) !== null
    }

    function loadMoreL2AccountActivity() {
        if (!l2IndexerReadEnabled || l2AccountActivityInFlight
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
        if (!l2IndexerReadEnabled || l2AccountActivityInFlight
                || l2AccountId.length === 0) {
            return null
        }
        l2AccountActivityRequestRevision += 1
        const requestRevision = l2AccountActivityRequestRevision
        const requestContext = l2RequestContext()
        const cursorText = String(cursor || "")
        const requestedAccountId = l2AccountId
        l2AccountActivityInFlight = true
        l2AccountActivityError = ""
        l2AccountActivityErrorDetails = null
        return dispatch("zoneL2AccountActivity", {
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
            if (!l2RequestContextIsCurrent(requestContext)
                    || requestedAccountId !== l2AccountId) {
                return
            }
            if (!validL2ReportResponse(response, "lez.account_activity", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2AccountActivityError = responseError(response,
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

    function refreshL2Programs() {
        resetL2ProgramsState()
        if (!l2SequencerReadEnabled) {
            l2ProgramsLoaded = true
            l2ProgramsError = qsTr("Select a Sequencer source to inspect known programs.")
            return null
        }
        l2ProgramsRequestRevision += 1
        const requestRevision = l2ProgramsRequestRevision
        const requestContext = l2RequestContext()
        const sourceId = l2SequencerSourceId()
        l2ProgramsInFlight = true
        return dispatch("zoneL2Programs", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.programs", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2ProgramsError = responseError(response,
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
                    || !validL2SingleSourceValue(outcome.value, sourceId, "sequencer")) {
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
        if (!l2SequencerReadEnabled) {
            l2CommitmentProofError = qsTr("Select a Sequencer source to inspect a commitment proof.")
            return null
        }
        l2CommitmentHex = normalizedCommitment
        l2CommitmentProofRequestRevision += 1
        const requestRevision = l2CommitmentProofRequestRevision
        const requestContext = l2RequestContext()
        const sourceId = l2SequencerSourceId()
        l2CommitmentProofInFlight = true
        return dispatch("zoneL2CommitmentProof", {
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
            if (!l2RequestContextIsCurrent(requestContext)
                    || normalizedCommitment !== l2CommitmentHex) {
                return
            }
            if (!validL2ReportResponse(response, "lez.commitment_proof", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2CommitmentProofError = responseError(response,
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
                    || !validL2SingleSourceValue(outcome.value, sourceId, "sequencer")) {
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
        if (!l2SequencerReadEnabled) {
            l2AccountNoncesError = qsTr("Select a Sequencer source to inspect account nonces.")
            return null
        }
        l2NonceAccountIds = normalizedIds
        l2AccountNoncesRequestRevision += 1
        const requestRevision = l2AccountNoncesRequestRevision
        const requestContext = l2RequestContext()
        const sourceId = l2SequencerSourceId()
        l2AccountNoncesInFlight = true
        return dispatch("zoneL2AccountNonces", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.account_nonces", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2AccountNoncesError = responseError(response,
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
                    || !validL2SingleSourceValue(outcome.value, sourceId, "sequencer")) {
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
        if (!l2IndexerReadEnabled) {
            l2TransfersLoaded = true
            l2TransfersError = qsTr("Configure an Indexer to inspect finalized transfer windows.")
            return null
        }
        return requestL2Transfers("", false)
    }

    function loadOlderL2Transfers() {
        if (!l2IndexerReadEnabled || l2TransfersInFlight || !l2TransfersHasMore
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
        if (!l2IndexerReadEnabled || l2TransfersInFlight) {
            return null
        }
        l2TransfersRequestRevision += 1
        const requestRevision = l2TransfersRequestRevision
        const requestContext = l2RequestContext()
        const cursorText = String(cursor || "")
        const previousPage = older ? currentL2TransfersPage() : null
        l2TransfersInFlight = true
        l2TransfersError = ""
        l2TransfersErrorDetails = null
        return dispatch("zoneL2Transfers", {
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
            if (!l2RequestContextIsCurrent(requestContext)) {
                return
            }
            if (!validL2ReportResponse(response, "lez.transfers", requestRevision)) {
                if (acceptedL2Failure(response, requestContext, requestRevision)) {
                    l2TransfersError = responseError(response,
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

    function reconcileDetail() {
        if (!activeZoneContext || verification !== "verified"
                || !summaryMatchesStatus()) {
            return false
        }
        if (detailMatchesStatus()) {
            detailStale = false
            return true
        }
        detailStale = zoneDetail !== null
        if (!detailInFlight) {
            fetchActiveZoneDetail()
        }
        return false
    }

    function fetchActiveZoneDetail() {
        if (!activeZoneContext || !catalogStatus || detailInFlight
                || verification !== "verified" || !summaryMatchesStatus()) {
            return null
        }

        detailRequestRevision += 1
        const requestRevision = detailRequestRevision
        const generation = sourceGeneration
        const requestedContextRevision = numericRevision(activeZoneContext.context_revision)
        const channelId = activeZoneId
        detailInFlight = true
        detailError = ""
        return dispatch("zoneDetail", {
            source_revision: sourceRevision,
            network_scope: networkScope,
            catalog_revision: catalogRevision,
            summary_revision: summaryRevision,
            observation_revision: observationRevision,
            channel_id: channelId
        }, function (response) {
            if (requestRevision !== detailRequestRevision) {
                return
            }
            detailInFlight = false
            if (generation !== sourceGeneration || !activeZoneContext
                    || channelId !== activeZoneId
                    || requestedContextRevision !== numericRevision(activeZoneContext.context_revision)) {
                return
            }
            if (!validReportResponse(response, "zones.zone_detail")) {
                detailError = responseError(response, qsTr("Zone detail failed."))
                detailStale = zoneDetail !== null
                return
            }
            const report = response.value
            if (!detailReportMatchesCurrent(report, channelId)) {
                reconcileDetail()
                return
            }
            zoneDetailReport = report
            zoneDetail = report.detail
            detailError = ""
            detailStale = false
            updateActiveContextFromDetail(report.detail)
        })
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

    function applyChannelSourceConfig(request, callback) {
        if (sourceMutationInFlight) {
            const busyResponse = failedResponse(qsTr("Another Channel source edit is still running."))
            if (callback) {
                callback(busyResponse)
            }
            return null
        }
        if (!activeZoneContext || verification !== "verified") {
            const inactiveResponse = failedResponse(qsTr("A verified active Zone is required."))
            if (callback) {
                callback(inactiveResponse)
            }
            return null
        }

        const typedRequest = copyObject(request)
        typedRequest.network_scope = networkScope
        typedRequest.channel_id = activeZoneId
        sourceMutationRequestRevision += 1
        const requestRevision = sourceMutationRequestRevision
        const generation = sourceGeneration
        const requestedContextRevision = numericRevision(activeZoneContext.context_revision)
        const channelId = activeZoneId
        const scope = networkScopeKey
        sourceMutationInFlight = true
        sourceMutationError = ""
        sourceMutationWarning = null
        return dispatch("channelSourceConfigApply", typedRequest, function (response) {
            if (requestRevision !== sourceMutationRequestRevision) {
                return
            }
            sourceMutationInFlight = false
            if (generation !== sourceGeneration || scope !== networkScopeKey
                    || !activeZoneContext || channelId !== activeZoneId
                    || requestedContextRevision !== numericRevision(activeZoneContext.context_revision)) {
                if (callback) {
                    callback(response)
                }
                return
            }
            if (!validReportResponse(response, "zones.channel_source_config")) {
                sourceMutationError = responseError(response, qsTr("Channel source update failed."))
                if (callback) {
                    callback(response)
                }
                sourceMutationFinished(response)
                return
            }
            const report = response.value
            if (numericRevision(report.source_revision) !== sourceRevision
                    || !report.config
                    || String(report.config.channel_id || "") !== channelId
                    || scopeKey(report.config.network_scope) !== networkScopeKey) {
                sourceMutationError = qsTr("Channel source update belongs to stale Zone state.")
                const staleResponse = failedResponse(sourceMutationError)
                if (callback) {
                    callback(staleResponse)
                }
                sourceMutationFinished(staleResponse)
                return
            }

            applySourceMutationReport(report)
            sourceMutationWarning = report.attestation_warning || null
            summaryStale = true
            statusRefreshRequested()
            if (callback) {
                callback(response)
            }
            sourceMutationFinished(response)
        })
    }

    function applySourceMutationReport(report) {
        const config = report.config
        if (zoneDetail && typeof zoneDetail === "object") {
            const nextDetail = copyObject(zoneDetail)
            nextDetail.channel_source_config = sourceConfigSummary(config)
            nextDetail.source_observations = Array.isArray(report.observations) ? report.observations : []
            nextDetail.source_agreement = report.agreement || ({})
            zoneDetail = nextDetail
            zoneDetailReport = {
                report_kind: "zones.zone_detail",
                schema_version: Number(report.schema_version || 1),
                source_revision: numericRevision(report.source_revision),
                network_scope: networkScope,
                catalog_revision: numericRevision(report.catalog_revision),
                source_config_epoch: numericRevision(report.source_config_epoch),
                observation_revision: numericRevision(report.observation_revision),
                summary_revision: numericRevision(report.summary_revision),
                detail: nextDetail
            }
        }
        updateActiveContextFromConfig(config)
    }

    function retryCatalog() {
        return runCatalogControl("zoneCatalogRetry")
    }

    function rebuildCatalog() {
        return runCatalogControl("zoneCatalogRebuild")
    }

    function runCatalogControl(method) {
        if (!catalogConfigured || controlInFlight) {
            return null
        }
        controlRequestRevision += 1
        const requestRevision = controlRequestRevision
        const generation = sourceGeneration
        const requestedSourceRevision = sourceRevision
        controlInFlight = true
        controlError = ""
        return dispatch(method, {
            source_revision: requestedSourceRevision
        }, function (response) {
            if (requestRevision !== controlRequestRevision) {
                return
            }
            controlInFlight = false
            if (generation !== sourceGeneration || requestedSourceRevision !== sourceRevision) {
                return
            }
            if (!validReportResponse(response, "zones.catalog_control")) {
                controlError = responseError(response, qsTr("Zone Catalog control failed."))
                return
            }
            const nextSourceRevision = numericRevision(response.value.source_revision)
            if (nextSourceRevision <= 0) {
                controlError = qsTr("Zone Catalog control returned an invalid source revision.")
                return
            }
            if (nextSourceRevision !== sourceRevision) {
                sourceGeneration += 1
                sourceRevision = nextSourceRevision
                clearActiveZone()
                invalidateSummary(false)
            }
            statusRefreshRequested()
        })
    }

    function updateActiveContextFromSummary(row) {
        if (!activeZoneContext || String(row.channel_id || "") !== activeZoneId) {
            return false
        }
        const candidate = contextFromSummary(
            row,
            numericRevision(activeZoneContext.source_config_revision),
            numericRevision(activeZoneContext.context_revision)
        )
        if (sameContextRoute(candidate, activeZoneContext)) {
            return false
        }
        contextRevision += 1
        candidate.context_revision = contextRevision
        candidate.source_config_revision = 0
        activeZoneContext = candidate
        return true
    }

    function updateActiveContextFromDetail(detail) {
        if (!activeZoneContext || !detail || !detail.summary
                || String(detail.summary.channel_id || "") !== activeZoneId) {
            return false
        }
        const config = detail.channel_source_config || ({})
        const candidate = contextFromConfig(detail.summary, config, numericRevision(activeZoneContext.context_revision))
        if (sameContext(candidate, activeZoneContext)) {
            return false
        }
        contextRevision += 1
        candidate.context_revision = contextRevision
        activeZoneContext = candidate
        return true
    }

    function updateActiveContextFromConfig(config) {
        if (!activeZoneContext || !config || String(config.channel_id || "") !== activeZoneId) {
            return false
        }
        const row = zoneSummary(activeZoneId)
        if (!row) {
            return false
        }
        const candidate = contextFromConfig(row, sourceConfigSummary(config), numericRevision(activeZoneContext.context_revision))
        if (sameContext(candidate, activeZoneContext)) {
            return false
        }
        contextRevision += 1
        candidate.context_revision = contextRevision
        activeZoneContext = candidate
        return true
    }

    function contextFromSummary(row, configRevision, revision) {
        const settlement = row && row.settlement_link ? row.settlement_link : ({})
        const l2 = row && row.l2_zone ? row.l2_zone : ({})
        return {
            network_scope: networkScope,
            channel_id: String(row && row.channel_id || ""),
            zone_kind: String(row && row.kind || "unknown"),
            selected_sequencer_source_id: settlement.selected_sequencer_source_id
                || l2.selected_source_id
                || null,
            indexer_source_id: settlement.indexer_source_id || null,
            source_config_revision: numericRevision(configRevision),
            context_revision: numericRevision(revision)
        }
    }

    function contextFromConfig(row, config, revision) {
        const indexer = config && config.indexer_source ? config.indexer_source : null
        return {
            network_scope: networkScope,
            channel_id: String(row && row.channel_id || ""),
            zone_kind: String(row && row.kind || "unknown"),
            selected_sequencer_source_id: config && config.selected_sequencer_source_id
                ? String(config.selected_sequencer_source_id)
                : null,
            indexer_source_id: indexer && indexer.source_id ? String(indexer.source_id) : null,
            source_config_revision: numericRevision(config && config.config_revision),
            context_revision: numericRevision(revision)
        }
    }

    function sourceConfigSummary(config) {
        const sequencers = []
        const configuredSequencers = config && Array.isArray(config.sequencer_sources)
            ? config.sequencer_sources
            : []
        for (let i = 0; i < configuredSequencers.length; ++i) {
            const source = configuredSequencers[i] || ({})
            const attestation = source.channel_attestation || ({})
            sequencers.push({
                source_id: String(source.source_id || ""),
                label: source.label === undefined ? null : source.label,
                target: source.target || ({}),
                binding_state: String(source.binding_state || attestation.state || "pending")
            })
        }
        return {
            config_revision: numericRevision(config && config.config_revision),
            selected_sequencer_source_id: config && config.selected_sequencer_source_id
                ? String(config.selected_sequencer_source_id)
                : null,
            sequencer_sources: sequencers,
            indexer_source: config && config.indexer_source ? config.indexer_source : null
        }
    }

    function sameContextRoute(left, right) {
        return left && right
            && scopeKey(left.network_scope) === scopeKey(right.network_scope)
            && String(left.channel_id || "") === String(right.channel_id || "")
            && String(left.zone_kind || "") === String(right.zone_kind || "")
            && String(left.selected_sequencer_source_id || "") === String(right.selected_sequencer_source_id || "")
            && String(left.indexer_source_id || "") === String(right.indexer_source_id || "")
    }

    function sameContext(left, right) {
        return sameContextRoute(left, right)
            && numericRevision(left.source_config_revision) === numericRevision(right.source_config_revision)
    }

    function summaryMatchesStatus() {
        return verification === "verified" && summaryLoaded && catalogStatus
            && summarySourceRevision === sourceRevision
            && summaryNetworkScopeKey === networkScopeKey
            && summaryCatalogRevision === catalogRevision
            && summarySourceConfigEpoch === sourceConfigEpoch
            && summaryObservationRevision === observationRevision
            && summaryRevision === numericRevision(catalogStatus.summary_revision)
    }

    function detailMatchesStatus() {
        if (!zoneDetailReport || !activeZoneContext || !zoneDetail) {
            return false
        }
        return numericRevision(zoneDetailReport.source_revision) === sourceRevision
            && scopeKey(zoneDetailReport.network_scope) === networkScopeKey
            && numericRevision(zoneDetailReport.catalog_revision) === catalogRevision
            && numericRevision(zoneDetailReport.source_config_epoch) === sourceConfigEpoch
            && numericRevision(zoneDetailReport.observation_revision) === observationRevision
            && numericRevision(zoneDetailReport.summary_revision) === summaryRevision
            && String(zoneDetail.summary && zoneDetail.summary.channel_id || "") === activeZoneId
    }

    function summaryReportMatchesAssembly(report, assembly) {
        return numericRevision(report.source_revision) === assembly.source_revision
            && scopeKey(report.network_scope) === assembly.network_scope_key
            && report.changes && typeof report.changes === "object"
    }

    function sameSummarySnapshot(left, right) {
        return numericRevision(left.source_revision) === numericRevision(right.source_revision)
            && scopeKey(left.network_scope) === scopeKey(right.network_scope)
            && numericRevision(left.catalog_revision) === numericRevision(right.catalog_revision)
            && numericRevision(left.source_config_epoch) === numericRevision(right.source_config_epoch)
            && numericRevision(left.observation_revision) === numericRevision(right.observation_revision)
            && numericRevision(left.summary_revision) === numericRevision(right.summary_revision)
    }

    function detailReportMatchesCurrent(report, channelId) {
        return report && report.detail
            && numericRevision(report.source_revision) === sourceRevision
            && scopeKey(report.network_scope) === networkScopeKey
            && numericRevision(report.catalog_revision) === catalogRevision
            && numericRevision(report.source_config_epoch) === sourceConfigEpoch
            && numericRevision(report.observation_revision) === observationRevision
            && numericRevision(report.summary_revision) === summaryRevision
            && String(report.detail.summary && report.detail.summary.channel_id || "") === channelId
    }

    function zoneSummary(channelId) {
        return rowFromRows(zoneSummaries, channelId)
    }

    function rowFromRows(rows, channelId) {
        const target = String(channelId || "")
        const values = Array.isArray(rows) ? rows : []
        for (let i = 0; i < values.length; ++i) {
            if (String(values[i].channel_id || "") === target) {
                return values[i]
            }
        }
        return null
    }

    function rowsByChannel(rows) {
        const result = {}
        const values = Array.isArray(rows) ? rows : []
        for (let i = 0; i < values.length; ++i) {
            const channelId = String(values[i] && values[i].channel_id || "")
            if (channelId.length > 0) {
                result[channelId] = values[i]
            }
        }
        return result
    }

    function rowsFromMap(rows) {
        const keys = Object.keys(rows).sort()
        const result = []
        for (let i = 0; i < keys.length; ++i) {
            result.push(rows[keys[i]])
        }
        return result
    }

    function invalidateCatalogState(clearRows) {
        catalogStatus = null
        verification = "empty"
        coverage = ({})
        ingestion = ({})
        currentError = ""
        statusError = ""
        statusFailureCount = 0
        networkScope = null
        networkScopeKey = ""
        catalogRevision = 0
        sourceConfigEpoch = 0
        observationRevision = 0
        clearActiveZone()
        invalidateSummary(clearRows)
    }

    function invalidateSummary(clearRows) {
        summaryRequestRevision += 1
        summaryInFlight = false
        summaryAssembly = null
        summaryError = ""
        if (clearRows) {
            zoneSummaries = []
            summaryLoaded = false
            summaryStale = false
            summaryRevision = 0
            summarySourceRevision = 0
            summaryNetworkScopeKey = ""
            summaryCatalogRevision = 0
            summarySourceConfigEpoch = 0
            summaryObservationRevision = 0
        } else {
            summaryStale = summaryLoaded
        }
    }

    function scopeKey(scope) {
        if (!scope || typeof scope !== "object") {
            return ""
        }
        const kind = String(scope.kind || "")
        if (kind === "genesis_id") {
            return kind + ":" + String(scope.genesis_id || "")
        }
        if (kind === "finalized_anchor") {
            return kind
                + ":" + String(scope.genesis_time || "")
                + ":" + String(scope.block_slot === undefined ? "" : scope.block_slot)
                + ":" + String(scope.block_id || "")
                + ":" + String(scope.parent_id || "")
        }
        return JSON.stringify(scope)
    }

    function failureBackoffInterval(count) {
        const intervals = [2000, 5000, 15000, 30000]
        const index = Math.max(0, Math.min(intervals.length - 1, Number(count || 1) - 1))
        return intervals[index]
    }

    function numericRevision(value) {
        const revision = Number(value || 0)
        return Number.isFinite(revision) && revision >= 0 ? revision : 0
    }

    function copyObject(value) {
        const result = {}
        if (!value || typeof value !== "object") {
            return result
        }
        for (const key in value) {
            result[key] = value[key]
        }
        return result
    }

    function validReportResponse(response, reportKind) {
        return response && response.ok === true
            && response.value && typeof response.value === "object"
            && String(response.value.report_kind || "") === String(reportKind || "")
            && Number(response.value.schema_version || 0) === 1
    }

    function responseError(response, fallback) {
        return response && String(response.error || "").length > 0
            ? String(response.error)
            : String(fallback || "")
    }

    function failedResponse(error) {
        return {
            ok: false,
            value: null,
            text: "",
            error: String(error || "")
        }
    }

    function dispatch(method, request, callback) {
        if (!gateway || typeof gateway.request !== "function") {
            callback(failedResponse(qsTr("Inspector bridge is unavailable.")))
            return null
        }
        try {
            return gateway.request(String(method || ""), [request || {}], callback)
        } catch (error) {
            callback(failedResponse(String(error)))
            return null
        }
    }
}
