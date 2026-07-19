import QtQml
import "ZoneInspectionContract.js" as ZoneInspectionContract

QtObject {
    id: root

    required property var gateway
    property var appModel: null
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
    property var sourceEditorResetContext: null
    readonly property string activeZoneId: activeZoneContext
        ? String(activeZoneContext.channel_id || "")
        : ""
    property double contextRevision: 0
    property string requestedDetailTab: "overview"
    property string requestedL2View: "blocks"
    property var targetResolutionReport: null
    property var targetResolutionCandidates: []
    property string targetResolutionQuery: ""
    property string targetResolutionStatus: ""
    property string targetResolutionError: ""
    property var zoneDetailReport: null
    property var zoneDetail: null
    property bool detailStale: false
    property bool configureInFlight: false
    property bool statusInFlight: false
    property bool summaryInFlight: false
    property bool detailInFlight: false
    property bool controlInFlight: false
    property bool targetResolutionInFlight: false
    property int statusFailureCount: 0
    property bool automaticRetryPending: false
    property int automaticRetryAttempt: 0
    property bool startupAutoSelectionPending: true
    property string pendingZoneRestoreId: ""
    property string pendingZoneRestoreScopeKey: ""

    readonly property bool selectedSequencerReadEligible:
        selectedSequencerSourceReadEligible()

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
        : automaticRetryPending
            ? failureBackoffInterval(automaticRetryAttempt + 1)
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
    property int targetResolutionRequestRevision: 0

    readonly property ZoneL2InspectionState l2: ZoneL2InspectionState {
        gateway: root.gateway
        activeZoneContext: root.activeZoneContext
        verification: root.verification
        sequencerSourceReadEligible: root.selectedSequencerReadEligible
        appModel: root.appModel
    }
    readonly property ZoneEvidenceState evidence: ZoneEvidenceState {
        gateway: root.gateway
        activeZoneContext: root.activeZoneContext
        verification: root.verification
        sourceGeneration: root.sourceGeneration
        sourceRevision: root.sourceRevision
        networkScope: root.networkScope
        networkScopeKey: root.networkScopeKey
        catalogRevision: root.catalogRevision
    }
    readonly property ZoneSourceEditorState sourceEditor: ZoneSourceEditorState {
        gateway: root.gateway
        appModel: root.appModel
        sourceDescriptor: root.desiredSource
        activeZoneContext: root.activeZoneContext
        verification: root.verification
        networkScope: root.networkScope
        networkScopeKey: root.networkScopeKey
        sourceGeneration: root.sourceGeneration
        sourceRevision: root.sourceRevision

        onSourceMutationAccepted: function (report) {
            root.acceptSourceMutationReport(report)
        }
    }

    signal statusRefreshRequested()

    onSourceDescriptorChanged: {
        if (started) {
            syncCatalogSource()
        }
    }

    onActiveZoneContextChanged: {
        const preserveSourceWarning = sourceEditorResetContext !== null
            && activeZoneContext !== null
            && sameContextRoute(sourceEditorResetContext, activeZoneContext)
        sourceEditorResetContext = activeZoneContext
            ? copyObject(activeZoneContext) : null
        l2.resetL2InspectionState()
        sourceEditor.resetSourceEditorState(preserveSourceWarning)
        resetTargetResolution()
    }

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
            endpoint: endpoint,
            default_topology: String(value.default_topology || "") === "logos_testnet"
                ? "logos_testnet" : ""
        }
    }

    function sourceKey(value) {
        return value ? String(value.kind || "") + "\n" + String(value.endpoint || "")
            + "\n" + String(value.default_topology || "") : ""
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
        const source = {
            kind: desiredSource.kind,
            endpoint: desiredSource.endpoint
        }
        if (String(desiredSource.default_topology || "").length) {
            source.default_topology = desiredSource.default_topology
        }
        return dispatch("zoneCatalogConfigure", {
            source: source
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
        if (!statusPollingEnabled) {
            return false
        }
        if (statusInFlight || controlInFlight) {
            return false
        }
        if (automaticRetryPending) {
            automaticRetryPending = false
            automaticRetryAttempt = Math.min(4, automaticRetryAttempt + 1)
            return runCatalogControl("zoneCatalogRetry", true)
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
        const scopeChanged = networkScopeKey.length > 0
            && nextScopeKey.length > 0
            && nextScopeKey !== networkScopeKey
        const restoreScopeChanged = pendingZoneRestoreScopeKey.length > 0
            && nextScopeKey.length > 0
            && nextScopeKey !== pendingZoneRestoreScopeKey
        const catalogChanged = catalogStatus !== null
            && numericRevision(report.catalog_revision) !== catalogRevision
        if (scopeChanged || restoreScopeChanged) {
            pendingZoneRestoreId = ""
            pendingZoneRestoreScopeKey = ""
            startupAutoSelectionPending = true
            clearActiveZone()
            invalidateSummary(true)
        } else if (catalogChanged) {
            evidence.resetEvidenceState(true)
        }
        if (String(report.verification || "") !== "verified") {
            if (activeZoneId.length > 0) {
                pendingZoneRestoreId = activeZoneId
                pendingZoneRestoreScopeKey = networkScopeKey
            }
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

        if (currentError.length > 0 && ingestion.worker_running !== true) {
            automaticRetryPending = true
        } else {
            automaticRetryPending = false
        }
        if (verification === "verified" && currentError.length === 0) {
            automaticRetryAttempt = 0
        }

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
            selectStartupZone()
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
            startupAutoSelectionPending = false
            pendingZoneRestoreId = ""
            pendingZoneRestoreScopeKey = ""
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
            selectStartupZone()
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

    function activateZone(channelId, automatic) {
        const normalizedId = String(channelId || "")
        if (verification !== "verified" || networkScopeKey.length === 0) {
            return false
        }
        const row = zoneSummary(normalizedId)
        if (!row) {
            return false
        }
        if (automatic !== true) {
            startupAutoSelectionPending = false
            pendingZoneRestoreId = ""
            pendingZoneRestoreScopeKey = ""
        }
        if (activeZoneId === normalizedId) {
            reconcileDetail()
            return true
        }

        resetDetailState()
        contextRevision += 1
        activeZoneContext = contextFromSummary(row, contextRevision)
        detailStale = true
        reconcileDetail()
        return true
    }

    function selectStartupZone() {
        if (activeZoneId.length > 0) {
            startupAutoSelectionPending = false
            pendingZoneRestoreId = ""
            pendingZoneRestoreScopeKey = ""
            return false
        }
        if (pendingZoneRestoreId.length > 0) {
            const restoreId = pendingZoneRestoreId
            if (!zoneSummary(restoreId)) {
                return false
            }
            if (pendingZoneRestoreScopeKey.length > 0
                    && pendingZoneRestoreScopeKey !== networkScopeKey) {
                pendingZoneRestoreId = ""
                pendingZoneRestoreScopeKey = ""
                startupAutoSelectionPending = true
                return false
            }
            if (!activateZone(restoreId, true)) {
                return false
            }
            pendingZoneRestoreId = ""
            pendingZoneRestoreScopeKey = ""
            startupAutoSelectionPending = false
            return true
        }
        if (!startupAutoSelectionPending) {
            return false
        }
        const channelId = configuredSequencerZoneId(zoneSummaries)
        if (channelId.length === 0) {
            return false
        }
        if (!activateZone(channelId, true)) {
            return false
        }
        startupAutoSelectionPending = false
        return true
    }

    function configuredSequencerZoneId(rows) {
        const values = Array.isArray(rows) ? rows : []
        let candidate = ""
        for (let i = 0; i < values.length; ++i) {
            const row = values[i] || ({})
            const fields = row.active_zone_context_fields || ({})
            const channelId = String(row.channel_id || "")
            if (String(row.kind || "") !== "sequencer_zone"
                    || channelId.length === 0
                    || String(fields.selected_sequencer_source_id || "").length === 0
                    || numericRevision(fields.source_config_revision) === 0) {
                continue
            }
            if (candidate.length > 0) {
                return ""
            }
            candidate = channelId
        }
        return candidate
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
        evidence.resetEvidenceState(true)
        detailRequestRevision += 1
        detailInFlight = false
        zoneDetailReport = null
        zoneDetail = null
        detailError = ""
        detailStale = false
    }

    function resetTargetResolution() {
        targetResolutionRequestRevision += 1
        targetResolutionInFlight = false
        targetResolutionReport = null
        targetResolutionCandidates = []
        targetResolutionQuery = ""
        targetResolutionStatus = ""
        targetResolutionError = ""
    }

    function resolveTarget(query, callback) {
        const value = String(query || "").trim()
        if (value.length === 0) {
            return null
        }
        targetResolutionRequestRevision += 1
        const requestRevision = targetResolutionRequestRevision
        const requestContext = l2.l2RequestContext()
        const requestedContextRevision = numericRevision(contextRevision)
        targetResolutionInFlight = true
        targetResolutionQuery = value
        targetResolutionStatus = ""
        targetResolutionError = ""
        targetResolutionCandidates = []
        return dispatch("inspectionResolveTarget", {
            query: value,
            active_zone_context: requestContext,
            request_revision: requestRevision
        }, function (response) {
            if (requestRevision !== targetResolutionRequestRevision
                    || requestedContextRevision !== numericRevision(contextRevision)) {
                return
            }
            targetResolutionInFlight = false
            if (!response || response.ok !== true || !response.value
                    || String(response.value.report_kind || "")
                        !== "inspection.target_resolution"
                    || numericRevision(response.value.request_revision) !== requestRevision
                    || (requestContext && numericRevision(response.value.context_revision)
                        !== numericRevision(requestContext.context_revision))) {
                targetResolutionError = responseError(response,
                    qsTr("Search target could not be resolved."))
                if (typeof callback === "function") {
                    callback(null, targetResolutionError)
                }
                return
            }
            const report = response.value
            targetResolutionReport = report
            targetResolutionCandidates = Array.isArray(report.candidates)
                ? report.candidates.slice() : []
            targetResolutionStatus = String(report.status || "not_found")
            if (typeof callback === "function") {
                callback(report, "")
            }
        })
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

    function retryCatalog() {
        return runCatalogControl("zoneCatalogRetry", false)
    }

    function rebuildCatalog() {
        return runCatalogControl("zoneCatalogRebuild", false)
    }

    function runCatalogControl(method, automatic) {
        if (!catalogConfigured || controlInFlight) {
            return null
        }
        if (automatic !== true) {
            automaticRetryPending = false
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
                if (automatic === true) {
                    automaticRetryPending = true
                }
                return
            }
            const nextSourceRevision = numericRevision(response.value.source_revision)
            if (nextSourceRevision <= 0) {
                controlError = qsTr("Zone Catalog control returned an invalid source revision.")
                return
            }
            if (nextSourceRevision !== sourceRevision) {
                if (activeZoneId.length > 0) {
                    pendingZoneRestoreId = activeZoneId
                    pendingZoneRestoreScopeKey = networkScopeKey
                }
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
            numericRevision(activeZoneContext.context_revision)
        )
        if (sameContext(candidate, activeZoneContext)) {
            return false
        }
        contextRevision += 1
        candidate.context_revision = contextRevision
        activeZoneContext = candidate
        return true
    }

    function updateActiveContextFromDetail(detail) {
        if (!activeZoneContext || !detail || !detail.summary
                || String(detail.summary.channel_id || "") !== activeZoneId) {
            return false
        }
        const candidate = contextFromSummary(
            detail.summary,
            numericRevision(activeZoneContext.context_revision)
        )
        if (sameContext(candidate, activeZoneContext)) {
            return false
        }
        contextRevision += 1
        candidate.context_revision = contextRevision
        activeZoneContext = candidate
        return true
    }

    function updateActiveContextFromFields(fields) {
        if (!activeZoneContext || !fields || String(fields.channel_id || "") !== activeZoneId) {
            return false
        }
        const candidate = contextFromFields(
            fields,
            numericRevision(activeZoneContext.context_revision)
        )
        if (sameContext(candidate, activeZoneContext)) {
            return false
        }
        contextRevision += 1
        candidate.context_revision = contextRevision
        activeZoneContext = candidate
        return true
    }

    function contextFromSummary(row, revision) {
        return contextFromFields(row && row.active_zone_context_fields, revision)
    }

    function contextFromFields(fields, revision) {
        return {
            network_scope: fields ? fields.network_scope : null,
            channel_id: String(fields && fields.channel_id || ""),
            zone_kind: String(fields && fields.zone_kind || "unknown"),
            selected_sequencer_source_id: fields && fields.selected_sequencer_source_id
                ? String(fields.selected_sequencer_source_id)
                : null,
            indexer_source_id: fields && fields.indexer_source_id
                ? String(fields.indexer_source_id)
                : null,
            source_config_revision: numericRevision(fields && fields.source_config_revision),
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

    function selectedSequencerSourceReadEligible() {
        const sourceId = String(activeZoneContext
            && activeZoneContext.selected_sequencer_source_id || "")
        if (sourceId.length === 0 || !zoneDetail) {
            return false
        }

        const config = zoneDetail.channel_source_config || ({})
        const sources = Array.isArray(config.sequencer_sources)
            ? config.sequencer_sources : []
        let configured = null
        for (let i = 0; i < sources.length; ++i) {
            const source = sources[i] || ({})
            if (String(source.source_id || "") === sourceId) {
                configured = source
                break
            }
        }
        if (!configured) {
            return false
        }

        const observations = Array.isArray(zoneDetail.source_observations)
            ? zoneDetail.source_observations : []
        let observation = null
        for (let j = 0; j < observations.length; ++j) {
            const candidate = observations[j] || ({})
            if (String(candidate.source_id || "") === sourceId
                    && String(candidate.role || "") === "sequencer") {
                observation = candidate
                break
            }
        }
        const observedBinding = String(observation
            && observation.binding_state || "")
        const observedHealth = String(observation && observation.health || "")
        if (observedBinding === "channel_mismatch"
                || observedHealth === "channel_mismatch") {
            return false
        }
        if (observedBinding === "runtime_attested"
                || observedBinding === "runtime_evidence_matched"
                || observedBinding === "persisted_attested") {
            return true
        }

        const configuredBinding = String(configured.binding_state || "")
        return configuredBinding === "persisted_attested"
            || configuredBinding === "runtime_attested"
            || configuredBinding === "runtime_evidence_matched"
    }

    function acceptSourceMutationReport(report) {
        const config = report.config
        if (zoneDetail && typeof zoneDetail === "object") {
            const nextDetail = copyObject(zoneDetail)
            nextDetail.channel_source_config = sourceConfigSummary(config)
            nextDetail.source_observations = Array.isArray(report.observations)
                ? report.observations : []
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
        updateActiveContextFromFields(report.active_zone_context_fields)
        summaryStale = true
        statusRefreshRequested()
    }

    function sameContextRoute(left, right) {
        return ZoneInspectionContract.sameContextRoute(left, right)
    }

    function sameContext(left, right) {
        return ZoneInspectionContract.sameContext(left, right)
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
        automaticRetryPending = false
        automaticRetryAttempt = 0
        if (clearRows) {
            startupAutoSelectionPending = true
            pendingZoneRestoreId = ""
            pendingZoneRestoreScopeKey = ""
        }
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
        return ZoneInspectionContract.scopeKey(scope)
    }

    function failureBackoffInterval(count) {
        const intervals = [2000, 5000, 15000, 30000]
        const index = Math.max(0, Math.min(intervals.length - 1, Number(count || 1) - 1))
        return intervals[index]
    }

    function numericRevision(value) {
        return ZoneInspectionContract.numericRevision(value)
    }

    function copyObject(value) {
        return ZoneInspectionContract.copyObject(value)
    }

    function validReportResponse(response, reportKind) {
        return ZoneInspectionContract.validReportResponse(response, reportKind)
    }

    function responseError(response, fallback) {
        return ZoneInspectionContract.responseError(response, fallback)
    }

    function failedResponse(error) {
        return ZoneInspectionContract.failedResponse(error)
    }

    function dispatch(method, request, callback) {
        return ZoneInspectionContract.dispatch(gateway, method, request, callback)
    }
}
