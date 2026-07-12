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

    property bool configureInFlight: false
    property bool statusInFlight: false
    property bool summaryInFlight: false
    property bool detailInFlight: false
    property bool controlInFlight: false
    property bool sourceMutationInFlight: false
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

    signal statusRefreshRequested()
    signal sourceMutationFinished(var response)

    onSourceDescriptorChanged: {
        if (started) {
            syncCatalogSource()
        }
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
        if (scopeChanged) {
            clearActiveZone()
            invalidateSummary(true)
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
        detailRequestRevision += 1
        detailInFlight = false
        zoneDetailReport = null
        zoneDetail = null
        detailError = ""
        detailStale = false
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
        return {
            config_revision: numericRevision(config && config.config_revision),
            selected_sequencer_source_id: config && config.selected_sequencer_source_id
                ? String(config.selected_sequencer_source_id)
                : null,
            sequencer_sources: config && Array.isArray(config.sequencer_sources)
                ? config.sequencer_sources
                : [],
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
