import QtQml
import "ZoneFixtureData.js" as FixtureData

QtObject {
    id: root

    property var appModel: null
    readonly property var l2: root
    readonly property var blocks: root
    readonly property var accounts: root
    readonly property var tools: root
    readonly property var evidence: root
    readonly property var sourceEditor: root

    property double sourceRevision: 3
    property double catalogRevision: 19
    property string requestedDetailTab: "overview"
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
    property var readiness: null
    property string currentError: ""
    property string statusError: ""
    property string configureError: ""
    property bool catalogSourceUnavailable: false
    property string summaryError: ""
    property string detailError: ""
    property string sourceMutationError: ""
    property var sourceMutationWarning: null
    property bool controlInFlight: false
    property bool summaryInFlight: false
    property bool detailInFlight: false
    property bool sourceMutationInFlight: false
    property bool managedIndexerRefreshInFlight: false
    property bool managedIndexerControlInFlight: false
    property bool managedIndexerStatusStale: false
    property string managedIndexerError: ""
    property string managedIndexerResult: ""
    property int managedIndexerRefreshCount: 0
    property var managedIndexerConfigSnapshot: null
    property bool managedIndexerConfigLoading: false
    property bool managedIndexerConfigSaving: false
    property bool managedIndexerConfigValidationLoading: false
    property var managedIndexerConfigValidation: null
    property string managedIndexerConfigValidationText: ""
    property string managedIndexerConfigError: ""
    property bool managedIndexerConfigDraftDirty: false
    property var managedIndexerRuntime: ({
        ownership: "inspector_managed",
        run_state: "running",
        detail: "Fixture LogosCore runtime"
    })
    property var managedIndexerNode: ({
        key: "indexer",
        install_state: "installed",
        run_state: "stopped",
        indexer_state: "stopped",
        indexer_head: null,
        indexer_error: null,
        package_version: "1.0.0",
        managed_channel_id: null,
        available_actions: ["start"],
        detail: "Ready"
    })
    property bool summaryStale: false
    property bool detailStale: false
    property var zoneSummaries: FixtureData.zones()
    property string activeZoneId: FixtureData.identity("1")
    property var activeZoneContext: FixtureData.activeZoneContext(activeZoneId)
    property var zoneDetail: FixtureData.detailFor(activeZoneId)
    property var networkScope: FixtureData.networkScope()
    property string networkScopeKey: "genesis_id:" + FixtureData.identity("f")
    property var targetResolutionReport: null
    property var targetResolutionCandidates: []
    property string targetResolutionStatus: ""
    property string targetResolutionError: ""

    property int l2BlocksLimit: 25
    property string l2BlocksExactSourceId: ""
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
    property var l2SubmittedTransactionReceiptTraceInput: null
    property var l2SubmittedTransactionLocalDecode: null
    property string l2SubmittedTransactionLocalDecodeWarning: ""
    property string l2SubmittedTransactionLocalDecodeError: ""

    property string l2AccountId: FixtureData.l2AccountId()
    property var l2AccountFinalizedReport: FixtureData.l2RouteReport(
        "lez.account", FixtureData.l2AccountSnapshot("finalized").source)
    property var l2AccountFinalized: FixtureData.l2AccountSnapshot("finalized")
    property string l2AccountFinalizedError: ""
    property var l2AccountFinalizedErrorDetails: null
    property bool l2AccountFinalizedInFlight: false
    property var l2AccountFinalizedDecode: null
    property string l2AccountFinalizedDecodeError: ""
    property bool l2AccountFinalizedDecodeInFlight: false
    property var l2AccountProvisionalReport: FixtureData.l2RouteReport(
        "lez.account", FixtureData.l2AccountSnapshot("provisional").source)
    property var l2AccountProvisional: FixtureData.l2AccountSnapshot("provisional")
    property string l2AccountProvisionalError: ""
    property var l2AccountProvisionalErrorDetails: null
    property bool l2AccountProvisionalInFlight: false
    property var l2AccountProvisionalDecode: FixtureData.l2AccountDecode()
    property string l2AccountProvisionalDecodeError: ""
    property bool l2AccountProvisionalDecodeInFlight: false
    property var l2AccountHistoricalTarget: ({
        block_id: 12790,
        block_hash: FixtureData.identity("0")
    })
    property var l2AccountHistoricalReport: FixtureData.l2RouteReport(
        "lez.account", FixtureData.l2AccountSnapshot("historical").source)
    property var l2AccountHistorical: FixtureData.l2AccountSnapshot("historical")
    property string l2AccountHistoricalError: ""
    property var l2AccountHistoricalErrorDetails: null
    property bool l2AccountHistoricalInFlight: false
    property var l2AccountHistoricalDecode: null
    property string l2AccountHistoricalDecodeError: ""
    property bool l2AccountHistoricalDecodeInFlight: false
    property int l2AccountActivityLimit: 25
    property var l2AccountActivityReport: FixtureData.l2FoundReport(
        "lez.account_activity", {
            account_id: FixtureData.l2AccountId(),
            order: "oldest_first",
            rows: FixtureData.l2AccountActivityRows(),
            next_cursor: null,
            has_more: false
        }, FixtureData.l2AccountSnapshot("finalized").source)
    property string l2AccountActivityCanonicalId: FixtureData.l2AccountId()
    property var l2AccountActivityRows: FixtureData.l2AccountActivityRows()
    property string l2AccountActivityNextCursor: ""
    property bool l2AccountActivityHasMore: false
    property bool l2AccountActivityLoaded: true
    property string l2AccountActivityError: ""
    property var l2AccountActivityErrorDetails: null
    property bool l2AccountActivityInFlight: false

    property var l2ProgramsReport: FixtureData.l2FoundReport(
        "lez.programs", {
            programs: FixtureData.l2Programs(),
            source: FixtureData.l2AccountSnapshot("provisional").source
        }, FixtureData.l2AccountSnapshot("provisional").source)
    property var l2Programs: FixtureData.l2Programs()
    property bool l2ProgramsLoaded: true
    property string l2ProgramsError: ""
    property var l2ProgramsErrorDetails: null
    property bool l2ProgramsInFlight: false
    property string l2CommitmentHex: FixtureData.identity("c")
    property var l2CommitmentProofReport: FixtureData.l2FoundReport(
        "lez.commitment_proof", FixtureData.l2CommitmentProof(),
        FixtureData.l2CommitmentProof().source)
    property var l2CommitmentProof: FixtureData.l2CommitmentProof()
    property bool l2CommitmentProofLoaded: true
    property string l2CommitmentProofError: ""
    property var l2CommitmentProofErrorDetails: null
    property bool l2CommitmentProofInFlight: false
    property var l2NonceAccountIds: [FixtureData.l2AccountId(), FixtureData.identity("8")]
    property var l2AccountNoncesReport: FixtureData.l2FoundReport(
        "lez.account_nonces", {
            rows: FixtureData.l2AccountNonces(),
            source: FixtureData.l2AccountSnapshot("provisional").source
        }, FixtureData.l2AccountSnapshot("provisional").source)
    property var l2AccountNonces: FixtureData.l2AccountNonces()
    property bool l2AccountNoncesLoaded: true
    property string l2AccountNoncesError: ""
    property var l2AccountNoncesErrorDetails: null
    property bool l2AccountNoncesInFlight: false

    property int l2TransfersLimit: 25
    property var l2TransfersReport: FixtureData.l2FoundReport(
        "lez.transfers", {
            recipients: FixtureData.l2TransferRecipients(),
            next_cursor: null,
            has_more: false,
            newest_block: 12840,
            oldest_block: 12816,
            scanned_blocks: 25,
            finalized: true
        }, FixtureData.l2AccountSnapshot("finalized").source)
    property var l2TransferRecipients: FixtureData.l2TransferRecipients()
    property string l2TransfersNextCursor: ""
    property bool l2TransfersHasMore: false
    property var l2TransfersNewestBlock: 12840
    property var l2TransfersOldestBlock: 12816
    property int l2TransfersScannedBlocks: 25
    property bool l2TransfersFinalized: true
    property bool l2TransfersLoaded: true
    property var l2TransfersHistory: []
    property string l2TransfersError: ""
    property var l2TransfersErrorDetails: null
    property bool l2TransfersInFlight: false

    readonly property bool l2Applicable: activeZoneContext !== null
        && activeZoneContext.zone_kind === "sequencer_zone"
    readonly property bool l2SourceConfigured: l2Applicable
        && (String(activeZoneContext.indexer_source_id || "").length > 0
            || String(activeZoneContext.selected_sequencer_source_id || "").length > 0)
    readonly property bool l2ReadEnabled: verification === "verified"
        && l2Applicable && l2SourceConfigured
    property bool sequencerSourceReadEligible: true
    readonly property bool l2SequencerConfigured: l2Applicable
        && String(activeZoneContext
            && activeZoneContext.selected_sequencer_source_id || "").length > 0
    readonly property bool l2IndexerReadEnabled: l2ReadEnabled
        && String(activeZoneContext && activeZoneContext.indexer_source_id || "").length > 0
    readonly property bool l2SequencerReadEnabled: l2ReadEnabled
        && l2SequencerConfigured && sequencerSourceReadEligible

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
    property var sourceReloadConfig: null
    property string sourceReloadFailure: ""
    property int sourceReloadCount: 0
    property int retryCount: 0
    property bool clearTransactionOnBlockRefresh: false

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
        l2SubmittedTransactionReceiptTraceInput = null
        l2SubmittedTransactionLocalDecode = null
        l2SubmittedTransactionLocalDecodeWarning = ""
        l2SubmittedTransactionLocalDecodeError = ""
        l2BlocksExactSourceId = ""
        l2BlockRows = l2Applicable ? FixtureData.l2BlockRows() : []
        l2BlocksLoaded = l2Applicable
        l2AccountId = l2Applicable ? FixtureData.l2AccountId() : ""
        l2AccountFinalized = l2Applicable
            ? FixtureData.l2AccountSnapshot("finalized") : null
        l2AccountProvisional = l2Applicable
            ? FixtureData.l2AccountSnapshot("provisional") : null
        l2AccountHistorical = l2Applicable
            ? FixtureData.l2AccountSnapshot("historical") : null
        l2AccountFinalizedDecode = null
        l2AccountProvisionalDecode = l2Applicable ? FixtureData.l2AccountDecode() : null
        l2AccountHistoricalDecode = null
        l2AccountActivityRows = l2Applicable
            ? FixtureData.l2AccountActivityRows() : []
        l2AccountActivityLoaded = l2Applicable
        l2Programs = l2Applicable ? FixtureData.l2Programs() : []
        l2ProgramsLoaded = l2Applicable
        l2CommitmentProof = l2Applicable ? FixtureData.l2CommitmentProof() : null
        l2CommitmentProofLoaded = l2Applicable
        l2AccountNonces = l2Applicable ? FixtureData.l2AccountNonces() : []
        l2AccountNoncesLoaded = l2Applicable
        l2TransferRecipients = l2Applicable
            ? FixtureData.l2TransferRecipients() : []
        l2TransfersLoaded = l2Applicable
        l2TransfersFinalized = l2Applicable
        l2TransfersHistory = []
    }

    function bedrockEndpoint() {
        return "http://127.0.0.1:8080/"
    }

    function refreshManagedIndexer() {
        managedIndexerRefreshCount += 1
        return true
    }

    function runManagedIndexerAction(action, channelId) {
        const actionKey = String(action || "")
        const targetChannel = String(channelId || activeZoneId)
        if (actionKey === "start") {
            managedIndexerNode = {
                key: "indexer",
                install_state: "installed",
                run_state: "starting",
                indexer_state: "starting",
                indexer_head: "0",
                indexer_error: null,
                package_version: "1.0.0",
                managed_channel_id: targetChannel,
                available_actions: [],
                detail: "Starting"
            }
            managedIndexerResult = "Indexer start accepted"
            return true
        }
        if (actionKey === "stop") {
            managedIndexerNode = {
                key: "indexer",
                install_state: "installed",
                run_state: "stopping",
                indexer_state: "stopping",
                indexer_head: null,
                indexer_error: null,
                package_version: "1.0.0",
                managed_channel_id: targetChannel,
                available_actions: [],
                detail: "Stopping"
            }
            managedIndexerResult = "Indexer stop accepted"
            return true
        }
        managedIndexerError = "Unsupported action"
        return false
    }

    function setManagedIndexerConfigDraftDirty(dirty) {
        managedIndexerConfigDraftDirty = dirty === true
    }

    function clearManagedIndexerConfig() {
        managedIndexerConfigSnapshot = null
        managedIndexerConfigValidation = null
        managedIndexerConfigValidationText = ""
        managedIndexerConfigError = ""
        managedIndexerConfigLoading = false
        managedIndexerConfigSaving = false
        managedIndexerConfigValidationLoading = false
        managedIndexerConfigDraftDirty = false
    }

    function channelIndexerConfigText() {
        return JSON.stringify({
            channel_id: activeZoneId,
            bedrock_config: { addr: bedrockEndpoint() },
            consensus_info_polling_interval: "1s",
            allow_chain_reset: false
        }, null, 2)
    }

    function channelIndexerConfigFields(text) {
        let value = null
        try {
            value = JSON.parse(String(text || ""))
        } catch (error) {
            return []
        }
        return [{
            path: "/channel_id",
            label: "Zone channel ID",
            section: "Protocol",
            kind: "string",
            value: String(value.channel_id || ""),
            required: true,
            editable: false
        }, {
            path: "/bedrock_config/addr",
            label: "Bedrock API URL",
            section: "API",
            kind: "string",
            value: String(value.bedrock_config && value.bedrock_config.addr || ""),
            required: true,
            editable: false
        }, {
            path: "/consensus_info_polling_interval",
            label: "Consensus polling interval",
            section: "Protocol",
            kind: "string",
            value: String(value.consensus_info_polling_interval || ""),
            required: true,
            editable: true
        }, {
            path: "/allow_chain_reset",
            label: "Allow automatic chain reset",
            section: "Recovery",
            kind: "boolean",
            value: value.allow_chain_reset === true,
            required: false,
            editable: true
        }]
    }

    function channelIndexerConfigSnapshotFor(revision, text) {
        const rawText = String(text || channelIndexerConfigText())
        return {
            profile: "default",
            network_scope: networkScope,
            channel_id: activeZoneId,
            source_config_revision: Number(activeZoneContext
                && activeZoneContext.source_config_revision || 0),
            selected_sequencer_source_id: String(activeZoneContext
                && activeZoneContext.selected_sequencer_source_id || ""),
            node_label: "Channel Indexer",
            config_path: "/tmp/channel-indexers/" + activeZoneId + "/indexer-config.json",
            config_role: "Zone-owned Indexer",
            format: "json",
            raw_text: rawText,
            revision: String(revision || "fixture-revision-1"),
            editable: true,
            blocked_reason: null,
            validation_scope: "JSON syntax, Zone identity, Bedrock source, and supported Indexer fields",
            common_fields: channelIndexerConfigFields(rawText),
            protected_fields: [
                "Zone channel ID (derived from the selected Zone)",
                "Bedrock API URL (derived from the active Bedrock source)"
            ]
        }
    }

    function loadManagedIndexerConfig() {
        managedIndexerConfigLoading = true
        managedIndexerConfigError = ""
        managedIndexerConfigSnapshot = channelIndexerConfigSnapshotFor(
            managedIndexerConfigSnapshot
                ? managedIndexerConfigSnapshot.revision : "fixture-revision-1",
            managedIndexerConfigSnapshot ? managedIndexerConfigSnapshot.raw_text : "")
        managedIndexerConfigLoading = false
        validateManagedIndexerConfig(managedIndexerConfigSnapshot.raw_text)
        return true
    }

    function validateManagedIndexerConfig(text) {
        const rawText = String(text || "")
        managedIndexerConfigValidationLoading = true
        let valid = false
        let errorMessage = ""
        let fields = []
        try {
            const value = JSON.parse(rawText)
            if (!value || typeof value !== "object" || Array.isArray(value)) {
                errorMessage = "Configuration must be a JSON object."
            } else if (String(value.channel_id || "") !== activeZoneId) {
                errorMessage = "Zone channel ID is derived from the active Zone."
            } else if (String(value.bedrock_config && value.bedrock_config.addr || "")
                    !== bedrockEndpoint()) {
                errorMessage = "Bedrock API URL is derived from the active Bedrock source."
            } else if (String(value.consensus_info_polling_interval || "").length === 0) {
                errorMessage = "Consensus polling interval is required."
            } else {
                valid = true
                fields = channelIndexerConfigFields(rawText)
            }
        } catch (error) {
            errorMessage = String(error && error.message || error)
        }
        managedIndexerConfigValidation = {
            valid: valid,
            error: errorMessage,
            common_fields: fields
        }
        managedIndexerConfigValidationText = rawText
        managedIndexerConfigValidationLoading = false
        return managedIndexerConfigValidation
    }

    function saveManagedIndexerConfig(text, revision) {
        const rawText = String(text || "")
        if (!managedIndexerConfigSnapshot
                || String(revision || "") !== String(managedIndexerConfigSnapshot.revision || "")) {
            managedIndexerConfigError = "Configuration changed; reload it before saving."
            return false
        }
        const validation = validateManagedIndexerConfig(rawText)
        if (!validation.valid) {
            managedIndexerConfigError = validation.error
            return false
        }
        managedIndexerConfigSaving = true
        managedIndexerConfigSnapshot = channelIndexerConfigSnapshotFor(
            "fixture-revision-" + (Number(String(revision).split("-").pop()) + 1 || 2),
            rawText)
        managedIndexerConfigValidation = null
        managedIndexerConfigValidationText = ""
        managedIndexerConfigError = ""
        managedIndexerConfigSaving = false
        return true
    }

    function refreshL2Blocks() {
        l2BlocksExactSourceId = ""
        l2RefreshCount += 1
        l2BlockRows = l2Applicable ? FixtureData.l2BlockRows() : []
        l2BlocksLoaded = true
        l2BlocksError = l2ReadEnabled ? "" : l2AvailabilityMessage()
        return l2ReadEnabled ? l2RefreshCount : null
    }

    function refreshL2BlocksForSource(exactSourceId) {
        if (clearTransactionOnBlockRefresh) {
            closeL2Transaction()
        }
        l2BlocksExactSourceId = String(exactSourceId || "")
        l2RefreshCount += 1
        const rows = FixtureData.l2BlockRows()
        l2BlockRows = rows.map(function (row) {
            const observations = Array.isArray(row.observations)
                ? row.observations.filter(function (observation) {
                    return String(observation.source_id || "")
                        === l2BlocksExactSourceId
                        && String(observation.source_role || "") === "sequencer"
                }) : []
            return {
                summary: row.summary,
                observations: observations
            }
        }).filter(function (row) {
            return row.observations.length > 0
        })
        l2BlocksLoaded = true
        l2BlocksError = l2SequencerReadEnabled ? ""
            : "Select a Sequencer source for this Zone."
        return l2SequencerReadEnabled ? l2RefreshCount : null
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
            l2TransactionDetail.source.source_id)
        return 1
    }

    function resolveL2TransactionCandidate(candidate) {
        return openL2Transaction(l2TransactionId, candidate && candidate.source_id)
    }

    function requestL2TransactionTrace(transactionId, exactSourceId) {
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
        l2SubmittedTransactionReceiptTraceInput = null
        l2SubmittedTransactionLocalDecode = null
        l2SubmittedTransactionLocalDecodeWarning = ""
        l2SubmittedTransactionLocalDecodeError = ""
    }

    function inspectL2Account(accountId) {
        l2AccountId = String(accountId || "")
        l2AccountFinalized = FixtureData.l2AccountSnapshot("finalized")
        l2AccountProvisional = FixtureData.l2AccountSnapshot("provisional")
        l2AccountActivityRows = FixtureData.l2AccountActivityRows()
        l2AccountActivityLoaded = true
        return true
    }

    function inspectL2SequencerAccount(accountId) {
        l2AccountId = String(accountId || "")
        l2AccountFinalized = null
        l2AccountProvisional = FixtureData.l2AccountSnapshot("provisional")
        l2AccountActivityRows = []
        l2AccountActivityLoaded = true
        return true
    }

    function refreshL2AccountSnapshots() {
        l2AccountFinalized = FixtureData.l2AccountSnapshot("finalized")
        l2AccountProvisional = FixtureData.l2AccountSnapshot("provisional")
        return true
    }

    function refreshL2SequencerAccount() {
        l2AccountFinalized = null
        l2AccountProvisional = FixtureData.l2AccountSnapshot("provisional")
        return true
    }

    function requestL2HistoricalAccount(blockId, blockHash) {
        l2AccountHistoricalTarget = {
            block_id: Number(blockId),
            block_hash: String(blockHash || "")
        }
        l2AccountHistorical = FixtureData.l2AccountSnapshot("historical")
        return 1
    }

    function refreshL2AccountActivity() {
        l2AccountActivityRows = FixtureData.l2AccountActivityRows()
        l2AccountActivityLoaded = true
        return true
    }

    function loadMoreL2AccountActivity() {
        return false
    }

    function setL2AccountActivityLimit(limit) {
        l2AccountActivityLimit = Number(limit)
        return true
    }

    function refreshL2Programs() {
        l2Programs = FixtureData.l2Programs()
        l2ProgramsLoaded = true
        return 1
    }

    function requestL2CommitmentProof(commitmentHex) {
        l2CommitmentHex = String(commitmentHex || "")
        l2CommitmentProof = FixtureData.l2CommitmentProof()
        l2CommitmentProofLoaded = true
        return 1
    }

    function requestL2AccountNonces(accountIds) {
        l2NonceAccountIds = accountIds
        l2AccountNonces = FixtureData.l2AccountNonces()
        l2AccountNoncesLoaded = true
        return 1
    }

    function refreshL2Transfers() {
        l2TransferRecipients = FixtureData.l2TransferRecipients()
        l2TransfersLoaded = true
        l2TransfersFinalized = true
        l2TransfersHistory = []
        return 1
    }

    function loadOlderL2Transfers() {
        return null
    }

    function loadNewerL2Transfers() {
        return false
    }

    function setL2TransfersLimit(limit) {
        l2TransfersLimit = Number(limit)
        return true
    }

    function l2IndexerSourceId() {
        return String(activeZoneContext && activeZoneContext.indexer_source_id || "")
    }

    function l2SequencerSourceId() {
        return String(activeZoneContext
            && activeZoneContext.selected_sequencer_source_id || "")
    }

    function l2EntityRef(entityKind, canonicalKey, sourceObservation) {
        const source = sourceObservation || ({})
        return {
            network_scope: activeZoneContext.network_scope,
            channel_id: String(activeZoneContext.channel_id || ""),
            zone_kind: String(activeZoneContext.zone_kind || "unknown"),
            entity_kind: String(entityKind || ""),
            canonical_key: String(canonicalKey || ""),
            source: String(source.source_id || "").length > 0
                && String(source.source_role || "").length > 0 ? {
                    kind: "exact",
                    source_id: String(source.source_id),
                    source_role: String(source.source_role)
                } : { kind: "policy" }
        }
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

    function reloadChannelSourceConfig(callback) {
        sourceReloadCount += 1
        sourceMutationError = ""
        const response = sourceReloadFailure.length > 0
            ? { ok: false, value: null, text: "", error: sourceReloadFailure }
            : {
                ok: true,
                value: {
                    report_kind: "zones.channel_source_config_current",
                    schema_version: 1,
                    source_revision: sourceRevision,
                    network_scope: networkScope,
                    channel_id: activeZoneId,
                    config: sourceReloadConfig || zoneDetail.channel_source_config
                },
                text: "",
                error: ""
            }
        sourceMutationError = response.ok ? "" : response.error
        if (callback) {
            callback(response)
        }
        return 1
    }
}
