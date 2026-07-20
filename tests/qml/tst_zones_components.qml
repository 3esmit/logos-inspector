import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/zones/controls"
import "../../qml/features/zones/pages"
import "../../qml/state"
import "../../qml/state/domains"
import "../../qml/theme"
import "fixtures"
import "fixtures/ZoneFixtureData.js" as FixtureData

TestCase {
    id: testRoot

    name: "ZonesComponents"
    when: windowShown
    width: 1280
    height: 900
    readonly property Theme testTheme: theme

    Theme {
        id: theme
    }

    ZoneStateFixture {
        id: zoneState
    }

    ListModel {
        id: registeredIdlRegistry
    }

    FavoritesState {
        id: favoriteState
    }

    ZoneL2ProgramTransferState {
        id: isolatedProgramState

        l2Context: zoneState
    }

    QtObject {
        id: programExecutionMock

        property var idlInstructionPlanValue: null
        property string idlInstructionPlanError: ""
        property bool idlInstructionPlanPending: false
        property var idlInstructionPreviewValue: null
        property string idlInstructionError: ""
        property bool idlInstructionPreviewPending: false
        property bool idlInstructionSubmitPending: false
        property var idlInstructionConfirmation: null
        property var idlInstructionReceipt: null
        property var idlInstructionReceiptTarget: null
        property var idlInstructionFrozenArtifact: null

        function reviseIdlInstructionDraft(entry, request, targetDisplay) {
            return true
        }

        function planIdlInstruction() {
            return null
        }

        function previewIdlInstructionDraft() {
            return null
        }

        function idlInstructionPreviewCurrent() {
            return false
        }

        function beginIdlInstructionConfirmation() {
            return false
        }

        function cancelIdlInstructionConfirmation() {
            idlInstructionConfirmation = null
        }

        function confirmIdlInstruction(callback) {
            return null
        }

        function syncIdlInstructionContext(targetDisplay) {
            return false
        }
    }

    QtObject {
        id: shellMock

        property string resultText: ""
        property var resultValue: null
        property bool resultIsError: false
        property string resultOwner: ""
    }

    QtObject {
        id: appModel

        property var zoneInspection: zoneState
        property string selectedView: ""
        property string programTab: ""
        property var pendingInspectionEntityRef: null
        property alias registeredIdls: registeredIdlRegistry
        property var programExecution: programExecutionMock
        property var favoriteStore: favoriteState
        property var shell: shellMock
        property var openedInspectionCandidate: null
        property bool openInspectionCandidateSucceeds: true

        function idlEntryAt(index) {
            return null
        }

        function idlEntryForKey(key) {
            return null
        }

        function selectView(view) {
            selectedView = String(view || "")
        }

        function openInspectionCandidate(candidate, recordHistory) {
            openedInspectionCandidate = candidate
            return openInspectionCandidateSucceeds
        }
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: 1280
        height: 900
        color: theme.background

        ScrollView {
            anchors.fill: parent
            leftPadding: theme.pageMargin
            rightPadding: theme.pageMargin
            topPadding: theme.gapLarge
            bottomPadding: theme.gapLarge
            contentWidth: availableWidth

            ZonesPage {
                id: page

                theme: theme
                model: appModel
                width: parent ? parent.width : 1200
            }
        }
    }

    Component {
        id: factSectionFactory

        ZoneFactSection {}
    }

    Component {
        id: catalogStatusFactory

        ZoneCatalogStatus {}
    }

    Component {
        id: isolatedProgramsFactory

        ZoneL2Programs {}
    }

    Component {
        id: sourceRowFactory

        ChannelSourceRow {}
    }

    Component {
        id: sourcesSectionFactory

        ChannelSourcesSection {}
    }

    function init() {
        zoneState.verification = "verified"
        zoneState.coverage = {
            status: "complete",
            coverage_floor: 0,
            scanned_through_slot: 187085,
            observed_lib_slot: 187085,
            prefix_status: "complete",
            gap_count: 0
        }
        zoneState.currentError = ""
        zoneState.statusError = ""
        zoneState.configureError = ""
        zoneState.summaryStale = false
        zoneState.requestedDetailTab = "overview"
        zoneState.activeZoneId = FixtureData.identity("1")
        zoneState.zoneDetail = FixtureData.detailFor(zoneState.activeZoneId)
        zoneState.targetResolutionReport = null
        zoneState.targetResolutionCandidates = []
        zoneState.targetResolutionStatus = ""
        zoneState.targetResolutionError = ""
        shellMock.resultText = ""
        shellMock.resultValue = null
        shellMock.resultIsError = false
        shellMock.resultOwner = ""
        zoneState.resetL2Fixture()
        zoneState.l2BlocksError = ""
        zoneState.l2BlocksErrorDetails = null
        zoneState.l2RefreshCount = 0
        zoneState.evidenceLoaded = false
        zoneState.evidenceRows = []
        zoneState.selectedEvidenceRow = null
        zoneState.evidenceDetail = null
        zoneState.lastMutationRequest = null
        zoneState.mutationFailure = ""
        zoneState.sourceReloadConfig = null
        zoneState.sourceReloadFailure = ""
        zoneState.sourceReloadCount = 0
        zoneState.managedIndexerStatusStale = false
        zoneState.managedIndexerError = ""
        zoneState.managedIndexerResult = ""
        zoneState.closeL2Transaction()
        zoneState.l2TransactionRequestedSourceId = ""
        zoneState.appModel = appModel
        registeredIdlRegistry.clear()
        appModel.selectedView = ""
        appModel.programTab = ""
        appModel.pendingInspectionEntityRef = null
        appModel.openedInspectionCandidate = null
        appModel.openInspectionCandidateSucceeds = true
        favoriteState.clear()
        isolatedProgramState.l2ProgramsReport = zoneState.l2ProgramsReport
        isolatedProgramState.l2Programs = FixtureData.l2Programs()
        isolatedProgramState.l2ProgramsLoaded = true
        isolatedProgramState.l2ProgramsError = ""
        page.filter = "all"
        page.query = ""
        page.initialDetailTab = "overview"
        const detail = findChild(page, "zoneDetail")
        if (detail) {
            detail.discardSourceDraft()
            detail.currentTab = "overview"
        }
        wait(0)
    }

    function test_target_recovery_renders_actionable_guidance() {
        const message = "The configured L2 source is unavailable. Check Sources, then retry the search."
        const report = {
            report_kind: "inspection.target_resolution",
            status: "recovery",
            recovery: "retry",
            warnings: [{ code: "source_unavailable", recovery: "retry" }]
        }
        zoneState.targetResolutionReport = report
        zoneState.targetResolutionStatus = "recovery"
        shellMock.resultText = message
        shellMock.resultValue = report
        shellMock.resultIsError = true
        shellMock.resultOwner = "zones"

        const recovery = findChild(page, "inspectionTargetRecovery")
        verify(recovery !== null)
        tryCompare(recovery, "visible", true)
        compare(recovery.title, "Search needs attention")
        compare(recovery.message, message)
        compare(recovery.Accessible.name, "Search needs attention. " + message)

        shellMock.resultValue = { report_kind: "unrelated.result" }
        tryCompare(recovery, "visible", false)
    }

    function test_ambiguous_search_renders_ranked_source_choices_and_keeps_choices_after_selection() {
        const blockHash = "a".repeat(64)
        const canonicalKey = "block:42:" + blockHash
        const finalized = {
            entity_ref: {
                layer: "l2",
                network_scope: FixtureData.networkScope(),
                channel_id: FixtureData.identity("1"),
                zone_kind: "sequencer_zone",
                entity_kind: "block",
                canonical_key: canonicalKey,
                source: {
                    kind: "exact",
                    source_id: "idx-a",
                    source_role: "indexer"
                }
            },
            finality: "finalized"
        }
        const provisional = {
            entity_ref: {
                layer: "l2",
                network_scope: FixtureData.networkScope(),
                channel_id: FixtureData.identity("1"),
                zone_kind: "sequencer_zone",
                entity_kind: "block",
                canonical_key: canonicalKey,
                source: {
                    kind: "exact",
                    source_id: "seq-a",
                    source_role: "sequencer"
                }
            },
            finality: "provisional"
        }
        zoneState.targetResolutionReport = {
            report_kind: "inspection.target_resolution",
            status: "ambiguous",
            candidates: [finalized, provisional]
        }
        zoneState.targetResolutionCandidates = [finalized, provisional]
        zoneState.targetResolutionStatus = "ambiguous"
        wait(0)

        const table = findChild(page, "inspectionTargetCandidatesTable")
        verify(table !== null, "Object exists")
        compare(table.headerCells[0].text, "Priority")
        compare(table.headerCells[4].text, "Source / finality")

        const rows = page.targetCandidateRows()
        compare(rows.length, 2)
        compare(rows[0].cells[0].text, "1")
        compare(rows[0].cells[3].accessibleName,
            "Open result 1: " + canonicalKey)
        compare(rows[0].cells[4].text, "Indexer / Finalized / idx-a")
        compare(rows[1].cells[0].text, "2")
        compare(rows[1].cells[4].text, "Sequencer / Provisional / seq-a")

        appModel.openInspectionCandidateSucceeds = false
        table.cellActivated(0, 3, rows[0].cells[3], rows[0])
        compare(appModel.openedInspectionCandidate.entity_ref.source.source_id, "idx-a")
        compare(zoneState.targetResolutionCandidates.length, 2)

        appModel.openInspectionCandidateSucceeds = true
        table.cellActivated(1, 3, rows[1].cells[3], rows[1])
        compare(appModel.openedInspectionCandidate.entity_ref.source.source_id, "seq-a")
        compare(zoneState.targetResolutionCandidates.length, 2)
    }

    function test_catalog_status_exposes_complete_fact_and_error_text() {
        zoneState.verification = "source_behind"
        zoneState.coverage = {
            status: "partial",
            coverage_floor: 1008,
            scanned_through_slot: 691337,
            observed_lib_slot: 0,
            prefix_status: "unavailable",
            gap_count: 0
        }
        const errorText = "L1 source LIB 0 is behind catalog checkpoint 691337"
        zoneState.currentError = errorText
        const status = catalogStatusFactory.createObject(testWindow.contentItem, {
            theme: testRoot.testTheme,
            zoneState: zoneState,
            width: 900
        })
        verify(status !== null)
        try {
            const coverageFact = findChild(status, "zoneCatalogFact_1")
            const coverageValue = findChild(status, "zoneCatalogFactValue_1")
            const error = findChild(status, "zoneCatalogError")

            verify(coverageFact !== null)
            compare(coverageFact.Accessible.role, Accessible.StaticText)
            compare(coverageFact.Accessible.name,
                    "Coverage: Partial / prefix Unavailable")
            verify(coverageValue !== null)
            compare(coverageValue.text, "Partial / prefix Unavailable")
            tryCompare(coverageValue, "truncated", false)
            verify(error !== null)
            compare(error.Accessible.role, Accessible.StaticText)
            compare(error.Accessible.name, errorText)
        } finally {
            status.destroy()
        }
    }

    function test_catalog_status_bounds_multiline_error_accessibility() {
        const errorText = "first line\n" + "x".repeat(300)
        const normalized = errorText.replace(/\s+/g, " ").trim()
        const bounded = normalized.slice(0, 237) + "..."
        zoneState.currentError = errorText
        const status = catalogStatusFactory.createObject(testWindow.contentItem, {
            theme: testRoot.testTheme,
            zoneState: zoneState,
            width: 900
        })
        verify(status !== null)
        try {
            const error = findChild(status, "zoneCatalogError")

            verify(error !== null)
            compare(error.text, errorText)
            compare(error.Accessible.name, bounded)
            compare(error.Accessible.name.length, 240)
            verify(String(error.Accessible.name).indexOf("\n") < 0)
        } finally {
            status.destroy()
        }
    }

    function test_variant_d_hierarchy_full_identity_and_single_kind_tag() {
        const channelId = FixtureData.identity("1")
        const row = findChild(page, "zoneListRow_" + channelId)
        verify(row !== null)
        verify(row.visible)
        verify(hasVisibleText(row, channelId))
        compare(countNamed(row, "zoneKindChip"), 1)

        const catalogStatus = findChild(page, "zoneCatalogStatus")
        const compactStatus = findChild(page, "zoneCompactStatus")
        verify(catalogStatus !== null && catalogStatus.visible)
        verify(compactStatus !== null && compactStatus.visible)
        verify(hasVisibleText(compactStatus, "Settlement Link"))
        verify(hasVisibleText(page, "Channel Details"))
        verify(hasVisibleText(page, FixtureData.identity("a")))

        const cachedRow = findChild(page, "zoneListRow_" + FixtureData.identity("4"))
        verify(cachedRow !== null)
        verify(cachedRow.stale)
        verify(!cachedRow.interactive)
    }

    function test_zone_fact_section_exposes_heading_label_and_raw_value_semantics() {
        const channelId = FixtureData.identity("1")
        const section = factSectionFactory.createObject(testWindow.contentItem, {
            theme: testRoot.testTheme,
            title: "Channel Details",
            rows: [{
                label: "L1 Channel",
                value: channelId,
                copyable: true,
                monospace: true
            }],
            width: 600,
            height: 300
        })
        verify(section !== null)
        try {
            verify(hasAccessibleNode(section, "Channel Details", Accessible.Heading))
            verify(hasAccessibleNode(section, "L1 Channel", Accessible.StaticText))
            verify(hasAccessibleNode(section, channelId, Accessible.StaticText))
            verify(hasAccessibleNode(section, "Copy " + channelId, Accessible.Button))
        } finally {
            section.destroy()
        }
    }

    function test_configured_zone_channel_opens_sequencer_dashboard() {
        const channelId = FixtureData.identity("1")
        const row = findChild(page, "zoneListRow_" + channelId)
        const channelLink = findChild(page, "zoneChannelLink_" + channelId)
        verify(row !== null)
        verify(channelLink !== null)
        verify(channelLink.link)

        mouseClick(channelLink, channelLink.width / 2,
            channelLink.height / 2)

        tryCompare(appModel, "selectedView", "sequencerDashboard")
        compare(zoneState.activeZoneId, channelId)

        const dataLink = findChild(
            page, "zoneChannelLink_" + FixtureData.identity("8"))
        verify(dataLink !== null)
        verify(!dataLink.link)
    }

    function test_dirty_source_editor_guards_zone_change() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "channelSourcesSection") !== null
        })
        const sources = findChild(detail, "channelSourcesSection")
        sources.beginEditor("sequencer", null)
        tryVerify(function () {
            return findChild(sources, "channelSourceEditor") !== null
        })
        const endpoint = findChild(sources, "channelSourceEndpointField")
        verify(endpoint !== null)
        endpoint.text = "https://new-sequencer.example/"
        tryVerify(function () { return page.hasDirtyDraft })

        verify(!page.requestZoneActivation(FixtureData.identity("8")))
        compare(zoneState.activeZoneId, FixtureData.identity("1"))
        const guard = findChild(page, "zoneNavigationGuard")
        verify(guard !== null)
        tryCompare(guard, "opened", true)
        const confirmButton = findChild(guard.contentItem, "confirmButton")
        verify(confirmButton !== null)
        mouseClick(confirmButton, confirmButton.width / 2, confirmButton.height / 2)
        tryCompare(guard, "opened", false)
        compare(zoneState.activeZoneId, FixtureData.identity("8"))
    }

    function test_dirty_detail_component_survives_temporary_detail_reset() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "channelSourcesSection") !== null
        })
        const sources = findChild(detail, "channelSourcesSection")
        sources.beginEditor("sequencer", null)
        const endpoint = findChild(sources, "channelSourceEndpointField")
        endpoint.text = "https://draft.example/"
        tryVerify(function () { return page.hasDirtyDraft })
        zoneState.zoneDetail = null
        tryVerify(function () {
            return findChild(page, "zoneDetail") !== null && page.hasDirtyDraft
        })
    }

    function test_zone_detail_tab_survives_verified_catalog_refresh() {
        const original = findChild(page, "zoneDetail")
        verify(original !== null)
        verify(original.requestTab("transfers"))
        tryCompare(original, "currentTab", "transfers")
        tryCompare(zoneState, "requestedDetailTab", "transfers")
        page.initialDetailTab = zoneState.requestedDetailTab

        zoneState.zoneDetail = null
        tryVerify(function () {
            return findChild(page, "zoneDetail") === null
        })
        zoneState.zoneDetail = FixtureData.detailFor(zoneState.activeZoneId)

        let restored = null
        tryVerify(function () {
            restored = findChild(page, "zoneDetail")
            return restored !== null && restored !== original
        })
        compare(restored.currentTab, "transfers")
        verify(findChild(restored, "zoneL2Transfers") !== null)
    }

    function test_l1_evidence_viewer_renders_exact_payload_as_plain_text() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("evidence"))
        tryVerify(function () {
            return findChild(detail, "zoneEvidenceViewer") !== null
        })
        const viewer = findChild(detail, "zoneEvidenceViewer")
        tryCompare(zoneState, "evidenceLoaded", true)
        compare(zoneState.evidenceRows.length, 3)
        zoneState.openEvidence(zoneState.evidenceRows[2])
        tryVerify(function () {
            return hasVisibleText(viewer, "Raw inscription")
                && hasVisibleText(viewer, "Payload Integrity")
        })
        verify(hasVisibleText(viewer, "raw_data: archived payload\nchannel: " + FixtureData.identity("1")))
        viewer.payloadView = "hex"
        verify(viewer.payloadBody().length > 0)
    }

    function test_source_editor_submits_captured_revision_and_typed_target() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "channelSourcesSection") !== null
        })
        const sources = findChild(detail, "channelSourcesSection")
        sources.beginEditor("sequencer", null)
        tryVerify(function () {
            return findChild(sources, "channelSourceEditor") !== null
        })
        const endpoint = findChild(sources, "channelSourceEndpointField")
        const editor = findChild(sources, "channelSourceEditor")
        const save = findChild(sources, "channelSourceSaveButton")
        verify(!editor.remoteInsecureHttp("http://localhost:3040/"))
        verify(!editor.remoteInsecureHttp("http://source.localhost:3040/"))
        verify(!editor.remoteInsecureHttp("http://127.0.0.2:3040/"))
        verify(!editor.remoteInsecureHttp("http://[::1]:3040/"))
        verify(editor.remoteInsecureHttp("http://localhost.evil:3040/"))
        endpoint.text = "https://new-sequencer.example/"
        tryVerify(function () { return save.enabled })
        verify(editor.submit())

        verify(zoneState.lastMutationRequest !== null)
        compare(zoneState.lastMutationRequest.expected_config_revision, 7)
        compare(zoneState.lastMutationRequest.mutation.kind, "add_sequencer")
        compare(zoneState.lastMutationRequest.mutation.target.kind, "rpc")
        compare(zoneState.lastMutationRequest.mutation.target.endpoint, "https://new-sequencer.example/")
    }

    function test_degraded_source_row_uses_warning_tone_and_retains_diagnostic() {
        const row = sourceRowFactory.createObject(testWindow.contentItem, {
            theme: testRoot.testTheme,
            source: {
                source_id: "src_degraded",
                label: "Degraded Sequencer",
                target: {
                    kind: "rpc",
                    endpoint: "https://sequencer.example/"
                },
                binding_state: "runtime_attested"
            },
            observation: {
                source_id: "src_degraded",
                role: "sequencer",
                binding_state: "runtime_attested",
                health: "degraded",
                head_block_id: 42,
                head_block_hash: FixtureData.identity("a"),
                last_error: "health probe failed"
            },
            role: "sequencer",
            selected: true,
            width: 720
        })
        verify(row !== null)
        try {
            compare(row.tone, "warning")
            verify(hasVisibleText(row, "Degraded"))
            verify(hasVisibleText(row, "health probe failed"))
            verify(hasVisibleText(row, "Head 42"))
        } finally {
            row.destroy()
        }
    }

    function test_evidence_matched_source_row_uses_truthful_insecure_label() {
        const row = sourceRowFactory.createObject(testWindow.contentItem, {
            theme: testRoot.testTheme,
            source: {
                source_id: "src_evidence_matched",
                label: "Evidence-matched Sequencer",
                target: {
                    kind: "rpc",
                    endpoint: "http://sequencer.example/"
                },
                binding_state: "runtime_evidence_matched"
            },
            observation: {
                source_id: "src_evidence_matched",
                role: "sequencer",
                binding_state: "runtime_evidence_matched",
                health: "reachable",
                head_block_id: 73,
                head_block_hash: FixtureData.identity("b"),
                last_error: ""
            },
            role: "sequencer",
            selected: true,
            width: 720
        })
        verify(row !== null)
        try {
            const bindingLabel = findChild(row,
                "channelSourceBindingLabel_src_evidence_matched")
            verify(!!bindingLabel, "Object exists")
            compare(row.binding, "runtime_evidence_matched")
            compare(row.bindingLabel, qsTr("Evidence matched"))
            verify(row.insecureRemoteHttp)
            compare(bindingLabel.text, qsTr("%1 / Insecure HTTP").arg(
                qsTr("Evidence matched")))
        } finally {
            row.destroy()
        }
    }

    function test_persisted_evidence_match_keeps_legacy_identity_disclosure() {
        const channelId = FixtureData.identity("1")
        const detail = FixtureData.detailFor(channelId)
        detail.channel_source_config = {
            config_revision: 2,
            selected_sequencer_source_id: null,
            sequencer_sources: [{
                source_id: "src_evidence_matched",
                label: "Evidence-matched Sequencer",
                target: {
                    kind: "rpc",
                    endpoint: "https://sequencer.example/"
                },
                channel_attestation: {
                    state: "persisted_evidence_matched"
                }
            }],
            indexer_source: null
        }
        detail.source_observations = []
        zoneState.sourceMutationWarning = null
        const section = sourcesSectionFactory.createObject(testWindow.contentItem, {
            theme: testRoot.testTheme,
            zoneState: zoneState,
            detail: detail,
            width: 900
        })
        verify(section !== null)
        try {
            compare(section.config.sequencer_sources.length, 1)
            compare(section.config.sequencer_sources[0].channel_attestation.state,
                "persisted_evidence_matched")
            verify(section.hasPersistedLegacyIdentity)
            const disclosure = findChild(section, "channelSourceAttestationWarning")
            verify(disclosure !== null)
            verify(disclosure.visible)
            compare(disclosure.title, qsTr("Legacy Sequencer identity"))
            compare(disclosure.message,
                qsTr("Legacy Sequencer does not expose Channel identity. This user-selected mapping is enabled because its live block matches finalized L1 evidence for this Channel."))
            compare(disclosure.Accessible.role, Accessible.StaticText)
            compare(disclosure.Accessible.name,
                qsTr("%1. %2").arg(disclosure.title).arg(disclosure.message))
        } finally {
            section.destroy()
        }
    }

    function test_sequencer_editor_hides_unimplemented_module_mode() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "channelSourcesSection") !== null
        })
        const sources = findChild(detail, "channelSourcesSection")
        sources.beginEditor("sequencer", null)
        tryVerify(function () {
            return findChild(sources, "channelSourceEditor") !== null
        })
        const editor = findChild(sources, "channelSourceEditor")
        const save = findChild(sources, "channelSourceSaveButton")

        verify(!hasVisibleText(editor, "Module"))
        editor.targetKind = "module"
        verify(!editor.validDraft)
        verify(!save.enabled)
    }

    function test_module_source_uses_layer_owned_module_without_user_input() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "channelSourcesSection") !== null
        })
        const sources = findChild(detail, "channelSourcesSection")
        sources.beginEditor("indexer", null)
        tryVerify(function () {
            return findChild(sources, "channelSourceEditor") !== null
        })
        const editor = findChild(sources, "channelSourceEditor")
        const endpoint = findChild(sources, "channelSourceEndpointField")
        const moduleInfo = findChild(sources, "channelSourceModuleInfo")
        editor.targetKind = "module"

        tryVerify(function () { return editor.validDraft })
        verify(!endpoint.visible)
        verify(moduleInfo.visible)
        compare(editor.moduleDefault(), "lez_indexer_module")
        verify(editor.submit())
        compare(zoneState.lastMutationRequest.mutation.kind, "set_indexer")
        compare(zoneState.lastMutationRequest.mutation.target.kind, "module")
        compare(zoneState.lastMutationRequest.mutation.target.module_id, "lez_indexer_module")
    }

    function test_module_indexer_exposes_per_channel_lifecycle_control() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "managedIndexerControl") !== null
        })

        const control = findChild(detail, "managedIndexerControl")
        const start = findChild(control, "startManagedIndexerButton")
        const stop = findChild(control, "stopManagedIndexerButton")
        verify(control.visible)
        verify(start !== null && start.enabled)
        verify(stop !== null && !stop.enabled)
        verify(hasVisibleText(control,
            "Each Channel uses an isolated Inspector-managed LogosCore runtime. The selected Sequencer source is recorded as its configuration binding; Indexer follows finalized Bedrock data."))
        verify(hasVisibleText(control, "1.0.0"))

        compare(control.selectedChannelId, zoneState.activeZoneId)
    }

    function test_managed_indexer_actions_require_reported_availability() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "managedIndexerControl") !== null
        })

        const control = findChild(detail, "managedIndexerControl")
        const start = findChild(control, "startManagedIndexerButton")
        const stop = findChild(control, "stopManagedIndexerButton")
        verify(start !== null)
        verify(stop !== null)
        const initialNode = zoneState.managedIndexerNode
        const initialResult = zoneState.managedIndexerResult
        try {
            zoneState.managedIndexerNode = {
                key: "indexer",
                install_state: "installed",
                run_state: "stopped",
                indexer_state: "stopped",
                indexer_head: null,
                indexer_error: null,
                package_version: "1.0.0",
                managed_channel_id: null,
                available_actions: [],
                detail: "Ready"
            }
            tryVerify(function () {
                return !start.enabled && !stop.enabled
            })

            zoneState.managedIndexerNode = initialNode
            tryVerify(function () {
                return start.enabled && !stop.enabled
            })

            verify(zoneState.runManagedIndexerAction("start", zoneState.activeZoneId))
            tryCompare(control, "runState", "starting")
            tryVerify(function () {
                return !start.enabled && !stop.enabled
            })
        } finally {
            zoneState.managedIndexerNode = initialNode
            zoneState.managedIndexerResult = initialResult
        }
    }

    function test_managed_indexer_other_channel_does_not_block_this_channel_start() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "managedIndexerControl") !== null
        })

        const control = findChild(detail, "managedIndexerControl")
        const start = findChild(control, "startManagedIndexerButton")
        const initialNode = zoneState.managedIndexerNode
        const initialStale = zoneState.managedIndexerStatusStale
        try {
            zoneState.managedIndexerStatusStale = false
            zoneState.managedIndexerNode = {
                key: "indexer",
                install_state: "installed",
                run_state: "stopped",
                indexer_state: "stopped",
                managed_channel_id: "another-channel",
                available_actions: ["start"],
                detail: "Independent runtime available"
            }
            tryVerify(function () {
                return start.enabled
            })
        } finally {
            zoneState.managedIndexerNode = initialNode
            zoneState.managedIndexerStatusStale = initialStale
        }
    }

    function test_managed_indexer_stale_status_disables_lifecycle_actions() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "managedIndexerControl") !== null
        })

        const control = findChild(detail, "managedIndexerControl")
        const refresh = findChild(control, "refreshManagedIndexerButton")
        const start = findChild(control, "startManagedIndexerButton")
        const stop = findChild(control, "stopManagedIndexerButton")
        const initialNode = zoneState.managedIndexerNode
        const initialStale = zoneState.managedIndexerStatusStale
        try {
            zoneState.managedIndexerNode = {
                key: "indexer",
                install_state: "installed",
                run_state: "running",
                indexer_state: "caught_up",
                managed_channel_id: zoneState.activeZoneId,
                available_actions: ["stop"]
            }
            zoneState.managedIndexerStatusStale = false
            tryVerify(function () {
                return stop.enabled && !start.enabled
            })

            zoneState.managedIndexerStatusStale = true
            tryVerify(function () {
                return !start.enabled && !stop.enabled && refresh.enabled
            })
        } finally {
            zoneState.managedIndexerNode = initialNode
            zoneState.managedIndexerStatusStale = initialStale
        }
    }

    function test_managed_indexer_stop_remains_available_when_catalog_is_unverified() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "managedIndexerControl") !== null
        })

        const control = findChild(detail, "managedIndexerControl")
        const start = findChild(control, "startManagedIndexerButton")
        const stop = findChild(control, "stopManagedIndexerButton")
        const initialNode = zoneState.managedIndexerNode
        const initialStale = zoneState.managedIndexerStatusStale
        const initialVerification = zoneState.verification
        try {
            zoneState.verification = "empty"
            zoneState.managedIndexerStatusStale = false
            zoneState.managedIndexerNode = {
                key: "indexer",
                install_state: "installed",
                run_state: "stopped",
                indexer_state: "stopped",
                managed_channel_id: null,
                available_actions: ["start"]
            }
            tryVerify(function () {
                return !start.enabled && !stop.enabled
            })

            zoneState.managedIndexerNode = {
                key: "indexer",
                install_state: "installed",
                run_state: "running",
                indexer_state: "caught_up",
                managed_channel_id: zoneState.activeZoneId,
                available_actions: ["stop"]
            }
            tryVerify(function () {
                return stop.enabled && !start.enabled
            })
        } finally {
            zoneState.managedIndexerNode = initialNode
            zoneState.managedIndexerStatusStale = initialStale
            zoneState.verification = initialVerification
        }
    }

    function test_source_revision_conflict_keeps_unrebased_draft() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "channelSourcesSection") !== null
        })
        const sources = findChild(detail, "channelSourcesSection")
        sources.beginEditor("sequencer", null)
        tryVerify(function () {
            return findChild(sources, "channelSourceEditor") !== null
        })
        const editor = findChild(sources, "channelSourceEditor")
        const endpoint = findChild(sources, "channelSourceEndpointField")
        endpoint.text = "https://conflict.example/"
        zoneState.mutationFailure = "Channel source configuration revision conflict"

        verify(editor.submit())
        verify(editor.conflict)
        compare(endpoint.text, "https://conflict.example/")
        verify(sources.hasDirtyDraft)
    }

    function test_source_revision_conflict_reload_uses_current_persisted_revision() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("sources"))
        tryVerify(function () {
            return findChild(detail, "channelSourcesSection") !== null
        })
        const sources = findChild(detail, "channelSourcesSection")
        const originalConfig = zoneState.zoneDetail.channel_source_config
        const originalSource = originalConfig.sequencer_sources[0]
        sources.beginEditor("sequencer", originalSource)
        tryVerify(function () {
            return findChild(sources, "channelSourceEditor") !== null
        })
        const editor = findChild(sources, "channelSourceEditor")
        const endpoint = findChild(sources, "channelSourceEndpointField")
        endpoint.text = "https://stale-draft.example/"
        zoneState.mutationFailure = "Channel source configuration revision conflict"

        verify(editor.submit())
        verify(editor.conflict)
        compare(editor.expectedRevision, 7)
        compare(endpoint.text, "https://stale-draft.example/")

        zoneState.sourceReloadFailure = "Current source configuration is unavailable"
        sources.reloadDraft()
        compare(zoneState.sourceReloadCount, 1)
        verify(editor.conflict)
        compare(editor.expectedRevision, 7)
        compare(endpoint.text, "https://stale-draft.example/")
        verify(hasVisibleText(editor, "Current source configuration is unavailable"))

        const currentConfig = JSON.parse(JSON.stringify(originalConfig))
        currentConfig.config_revision = 8
        currentConfig.sequencer_sources[0].label = "Concurrent source revision"
        currentConfig.sequencer_sources[0].target.endpoint = "https://current.example/"
        zoneState.sourceReloadFailure = ""
        zoneState.sourceReloadConfig = currentConfig
        sources.reloadDraft()

        compare(zoneState.sourceReloadCount, 2)
        compare(editor.expectedRevision, 8)
        compare(editor.source.label, "Concurrent source revision")
        compare(endpoint.text, "https://current.example/")
        verify(!editor.conflict)
        verify(!editor.dirty)
        compare(zoneState.sourceMutationError, "")
    }

    function test_l2_tab_renders_conflicts_and_exact_block_provenance() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("l2"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Inspector") !== null
                && findChild(detail, "zoneL2Blocks") !== null
        })
        const inspector = findChild(detail, "zoneL2Inspector")
        const blocks = findChild(detail, "zoneL2Blocks")
        verify(hasVisibleText(blocks, "1 conflict ID"))
        verify(hasVisibleText(blocks, "Conflicting block observations"))
        verify(hasVisibleText(blocks, "Final + provisional"))

        const row = zoneState.l2BlockRows[0]
        const sourceId = row.observations[0].source_id
        blocks.blockRequested(row.summary, sourceId)
        tryCompare(inspector, "currentView", "block")
        const blockDetail = findChild(inspector, "zoneL2BlockDetail")
        verify(blockDetail !== null && blockDetail.visible)
        compare(zoneState.l2BlockRequestedSourceId, sourceId)
        verify(hasVisibleText(blockDetail, sourceId))
        verify(hasVisibleText(blockDetail, "Memory Cache"))
        verify(hasVisibleText(blockDetail, "Provisional"))
    }

    function test_l2_transaction_embeds_trace_with_same_source() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("l2"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Inspector") !== null
        })
        const inspector = findChild(detail, "zoneL2Inspector")
        const blocks = findChild(inspector, "zoneL2Blocks")
        const row = zoneState.l2BlockRows[0]
        blocks.blockRequested(row.summary, row.observations[0].source_id)
        const blockDetail = findChild(inspector, "zoneL2BlockDetail")
        const transaction = zoneState.l2BlockDetail.transactions[0]
        blockDetail.transactionRequested(transaction.hash, zoneState.l2BlockDetail.source.source_id)

        tryCompare(inspector, "currentView", "transaction")
        const transactionDetail = findChild(inspector, "zoneL2TransactionDetail")
        verify(transactionDetail !== null && transactionDetail.visible)
        compare(zoneState.l2TransactionDetail.source.source_id,
            zoneState.l2TransactionTrace.source.source_id)
        transactionDetail.currentTab = "trace"
        tryVerify(function () {
            return findChild(transactionDetail, "zoneL2TraceSummary") !== null
        })
        verify(hasVisibleText(transactionDetail, "Trace steps"))
        verify(hasVisibleText(transactionDetail, "Decoded Instruction"))
        verify(hasVisibleText(transactionDetail, "transfer"))
        verify(hasVisibleText(transactionDetail, "token"))
        verify(hasVisibleText(transactionDetail, "Account sender"))
        verify(hasVisibleText(transactionDetail, "Account recipient"))
        verify(hasVisibleText(transactionDetail, "Argument amount_to_transfer: u128"))
        verify(hasVisibleText(transactionDetail, "1234567"))
        verify(hasVisibleText(transactionDetail, "Content hash and signature checks"))
        verify(hasVisibleText(transactionDetail, "0. Parse transaction"))
        verify(hasVisibleText(transactionDetail,
            zoneState.l2TransactionDetail.source.source_id))
    }

    function test_private_l2_transaction_shows_local_submission_decode_separately() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("l2"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Inspector") !== null
        })
        const inspector = findChild(detail, "zoneL2Inspector")
        const blocks = findChild(inspector, "zoneL2Blocks")
        const row = zoneState.l2BlockRows[0]
        blocks.blockRequested(row.summary, row.observations[0].source_id)
        const blockDetail = findChild(inspector, "zoneL2BlockDetail")
        const transaction = zoneState.l2BlockDetail.transactions[0]
        blockDetail.transactionRequested(transaction.hash,
            zoneState.l2BlockDetail.source.source_id)

        tryCompare(inspector, "currentView", "transaction")
        const transactionDetail = findChild(inspector, "zoneL2TransactionDetail")
        verify(transactionDetail !== null && transactionDetail.visible)
        transactionDetail.currentTab = "trace"
        const remoteTrace = JSON.parse(JSON.stringify(zoneState.l2TransactionTrace))
        remoteTrace.trace.kind = "PrivacyPreserving"
        remoteTrace.trace.decoded_instruction = null
        zoneState.l2TransactionTrace = remoteTrace
        zoneState.l2SubmittedTransactionLocalDecode = {
            program_id: "ab".repeat(32),
            idl_name: "token",
            instruction: "transfer",
            variant_index: 0,
            accounts: [{ path: "sender", value: "Public/sender" }, {
                path: "recipient", value: "Private/recipient"
            }],
            args: [{ path: "amount", value: "1" }],
            remaining_words: []
        }
        zoneState.l2SubmittedTransactionLocalDecodeWarning = "invalid option tag 7"
        zoneState.l2SubmittedTransactionReceiptTraceInput = {
            privateSyncPending: true
        }

        tryVerify(function () {
            return hasVisibleText(transactionDetail,
                "Locally decoded submitted instruction")
        })
        verify(hasVisibleText(transactionDetail, "Local submission metadata"))
        verify(hasVisibleText(transactionDetail, "Private sync pending"))
        verify(hasVisibleText(transactionDetail,
            "Transaction submission is complete. After inclusion, use Read incoming in Local Wallet to update local private account state."))
        verify(hasVisibleText(transactionDetail, "Local submission decoded partially"))
        verify(hasVisibleText(transactionDetail, "invalid option tag 7"))
        verify(hasVisibleText(transactionDetail,
            "Privacy envelope does not expose program or instruction words. Decoded automatically from frozen local submission metadata held by this Inspector session and matched to this exact-source transaction."))
        verify(hasVisibleText(transactionDetail, "transfer"))
        verify(hasVisibleText(transactionDetail, "token"))
        verify(!hasVisibleText(transactionDetail, "Decoded Instruction"))
        verify(!hasVisibleText(transactionDetail, "Local submission decode unavailable"))
    }

    function test_l2_accounts_keep_snapshots_separate_and_activity_oldest_first() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("accounts"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Accounts") !== null
        })
        const accounts = findChild(detail, "zoneL2Accounts")
        const accountPosition = accounts.mapToItem(testWindow.contentItem, 0, 0)
        verify(accountPosition.y < testWindow.height)
        const finalized = findChild(accounts, "zoneL2FinalizedAccountSnapshot")
        const provisional = findChild(accounts, "zoneL2ProvisionalAccountSnapshot")
        verify(finalized !== null && provisional !== null)
        verify(finalized.snapshot !== provisional.snapshot)
        compare(finalized.snapshot.account.balance, "1240000")
        compare(provisional.snapshot.account.balance, "1242750")
        verify(hasVisibleText(finalized, "Finalized Snapshot"))
        verify(hasVisibleText(provisional, "Provisional Snapshot"))
        verify(hasVisibleText(provisional, "Sequencer head moved"))
        verify(hasVisibleText(finalized, "Indexer"))
        verify(hasVisibleText(provisional, "Sequencer"))
        verify(hasVisibleText(provisional, "IDL Decode"))
        verify(hasVisibleText(provisional, "Token Fixture"))
        verify(hasVisibleText(provisional, "TokenDefinition"))
        verify(hasVisibleText(provisional, "Pebble"))

        const activityRows = accounts.activityRows()
        compare(activityRows.length, 3)
        compare(activityRows[0].transactionId, FixtureData.identity("2"))
        compare(activityRows[2].transactionId, FixtureData.identity("5"))
        verify(hasVisibleText(accounts, "3 rows / oldest first"))
    }

    function test_l2_program_tools_show_selected_source_provenance() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("programs"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Programs") !== null
        })
        const programs = findChild(detail, "zoneL2Programs")
        verify(hasVisibleText(programs, "System Program"))
        verify(hasVisibleText(programs,
            "src_11111111111111111111111111111111"))
        compare(programs.programRows().length, 2)

        programs.currentTool = "proof"
        wait(0)
        verify(hasVisibleText(programs, "Proof Identity"))
        verify(hasVisibleText(programs, "42"))
        compare(programs.siblingRows().length, 3)

        programs.currentTool = "nonces"
        wait(0)
        compare(programs.nonceRows().length, 2)
        verify(findChild(programs, "zoneL2AccountNoncesTable") !== null)
    }

    function test_l2_program_favorite_uses_explicit_app_model() {
        const programs = isolatedProgramsFactory.createObject(
            testWindow.contentItem, {
                theme: testRoot.testTheme,
                zoneState: isolatedProgramState,
                appModel: appModel,
                zoneDetail: zoneState.zoneDetail,
                width: 1100
            })
        verify(programs !== null)
        try {
            const rows = programs.programRows()
            compare(rows.length, 2)
            verify(rows[0].favoriteEntry !== null)
            compare(rows[0].cells[3].text, "Add")

            const table = findChild(programs, "zoneL2ProgramsTable")
            verify(table !== null)
            table.cellActivated(0, 3, rows[0].cells[3], rows[0])
            compare(favoriteState.count("program"), 1)
            compare(programs.programRows()[0].cells[3].text, "Yes")

            const savedRows = programs.programRows()
            table.cellActivated(0, 3, savedRows[0].cells[3], savedRows[0])
            compare(favoriteState.count("program"), 0)
            compare(programs.programRows()[0].cells[3].text, "Add")
        } finally {
            programs.destroy()
        }
    }

    function test_l2_program_interact_empty_registry_opens_idl_registry() {
        const surface = openZoneProgramTools()
        surface.programs.currentTool = "interact"
        tryVerify(function () {
            const interaction = findChild(surface.programs,
                "zoneL2ProgramInteraction")
            const button = findChild(surface.programs,
                "zoneProgramOpenIdlsButton")
            return interaction !== null && button !== null && button.visible
        })

        const openButton = findChild(surface.programs,
            "zoneProgramOpenIdlsButton")
        openButton.clicked()

        compare(appModel.programTab, "idls")
        compare(appModel.selectedView, "programs")
    }

    function test_l2_program_transaction_rejects_wrong_source() {
        const surface = openZoneProgramTools()
        const transactionId = FixtureData.identity("e")
        const selectedSource = zoneState.l2SequencerSourceId()

        surface.programs.transactionRequested(transactionId,
            selectedSource + "-wrong")

        compare(surface.detail.currentTab, "programs")
        compare(zoneState.l2TransactionId, "")
        compare(zoneState.l2TransactionRequestedSourceId, "")
    }

    function test_l2_program_transaction_opens_exact_selected_source() {
        const surface = openZoneProgramTools()
        const transactionId = FixtureData.identity("e")
        const selectedSource = zoneState.l2SequencerSourceId()

        surface.programs.transactionRequested(transactionId, selectedSource)

        tryCompare(surface.detail, "currentTab", "l2")
        compare(zoneState.l2TransactionId, transactionId)
        compare(zoneState.l2TransactionRequestedSourceId, selectedSource)
        tryVerify(function () {
            const inspector = findChild(surface.detail, "zoneL2Inspector")
            return inspector !== null && inspector.currentView === "transaction"
        })
        compare(zoneState.l2TransactionDetail.source.source_id,
            selectedSource)
        compare(zoneState.l2TransactionTrace.source.source_id,
            selectedSource)
    }

    function test_l2_account_transaction_opens_exact_configured_indexer_source() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("accounts"))
        let accounts = null
        tryVerify(function () {
            accounts = findChild(detail, "zoneL2Accounts")
            return accounts !== null
        })
        const transactionId = FixtureData.identity("d")
        const indexerSource = zoneState.l2IndexerSourceId()
        verify(indexerSource.length > 0)

        accounts.transactionRequested(transactionId, indexerSource)

        tryCompare(detail, "currentTab", "l2")
        compare(zoneState.l2TransactionId, transactionId)
        compare(zoneState.l2TransactionRequestedSourceId, indexerSource)
        tryVerify(function () {
            const inspector = findChild(detail, "zoneL2Inspector")
            return inspector !== null && inspector.currentView === "transaction"
        })
        compare(zoneState.l2TransactionDetail.source.source_id,
            indexerSource)
        compare(zoneState.l2TransactionTrace.source.source_id,
            indexerSource)
    }

    function test_l2_transfers_show_page_local_window_and_both_evidence_kinds() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("transfers"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Transfers") !== null
        })
        const transfers = findChild(detail, "zoneL2Transfers")
        verify(hasVisibleText(transfers, "Finalized Window"))
        verify(hasVisibleText(transfers, transfers.windowLabel()))
        compare(zoneState.l2TransfersNewestBlock, 12840)
        compare(zoneState.l2TransfersOldestBlock, 12816)
        const rows = transfers.recipientRows()
        compare(rows.length, 2)
        compare(rows[0].recipient.received, "2750")
        compare(rows[0].recipient.outputs, 1)
        compare(rows[0].recipient.references, 2)
        compare(rows[0].recipient.source, "transfer_outputs_and_account_refs")

        transfers.selectedRecipient = rows[0].recipient
        wait(0)
        const evidenceRows = transfers.transferEvidenceRows()
        compare(evidenceRows.length, 2)
        compare(evidenceRows[0].cells[2].text, "Transfer output")
        compare(evidenceRows[1].cells[2].text, "Account reference")
        verify(findChild(transfers, "zoneL2TransferEvidenceTable") !== null)
    }

    function test_data_channel_l2_tab_is_explicitly_not_applicable() {
        appModel.pendingInspectionEntityRef = {
            canonical_key: "queued-account"
        }
        verify(page.requestZoneActivation(FixtureData.identity("8")))
        compare(appModel.pendingInspectionEntityRef, null)
        const detail = findChild(page, "zoneDetail")
        appModel.pendingInspectionEntityRef = {
            canonical_key: "queued-account"
        }
        verify(detail.requestTab("l2"))
        compare(appModel.pendingInspectionEntityRef, null)
        tryVerify(function () {
            return findChild(detail, "zoneL2Inspector") !== null
        })
        const inspector = findChild(detail, "zoneL2Inspector")
        verify(hasVisibleText(inspector, "L2 not applicable"))
        verify(hasVisibleText(inspector, "L2 reads do not apply to this Channel type."))
        compare(zoneState.l2RefreshCount, 0)
    }

    function openZoneProgramTools() {
        const detail = findChild(page, "zoneDetail")
        verify(detail !== null)
        verify(detail.requestTab("programs"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Programs") !== null
        })
        return {
            detail: detail,
            programs: findChild(detail, "zoneL2Programs")
        }
    }

    function countNamed(item, objectName) {
        if (!item) {
            return 0
        }
        let count = String(item.objectName || "") === objectName ? 1 : 0
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            count += countNamed(children[i], objectName)
        }
        return count
    }

    function hasVisibleText(item, expected) {
        if (!item) {
            return false
        }
        if (item.text !== undefined && String(item.text) === expected
                && item.visible && item.width > 0 && item.height > 0) {
            return true
        }
        if (item.contentItem && hasVisibleText(item.contentItem, expected)) {
            return true
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            if (hasVisibleText(children[i], expected)) {
                return true
            }
        }
        return false
    }

    function hasAccessibleNode(item, expectedName, expectedRole) {
        if (!item) {
            return false
        }
        if (item.Accessible
                && String(item.Accessible.name) === expectedName
                && item.Accessible.role === expectedRole
                && !item.Accessible.ignored
                && item.visible) {
            return true
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            if (hasAccessibleNode(children[i], expectedName,
                    expectedRole)) {
                return true
            }
        }
        return false
    }
}
