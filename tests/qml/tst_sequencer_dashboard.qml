pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/zones/pages"
import "../../qml/theme"
import "fixtures"
import "fixtures/ZoneFixtureData.js" as FixtureData

TestCase {
    id: testRoot

    name: "SequencerDashboard"
    when: windowShown
    width: 1180
    height: 860

    Theme {
        id: theme
    }

    ZoneStateFixture {
        id: zoneState
    }

    QtObject {
        id: dashboardZoneState

        property alias activeZoneId: zoneState.activeZoneId
        property alias zoneDetail: zoneState.zoneDetail
        readonly property var l2: zoneState.l2
        property string requestedL2View: ""
        property string requestedDetailTab: ""
    }

    ListModel {
        id: registeredIdlModel
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
        property var idlInstructionFrozenArtifact: null
        property var idlInstructionConfirmation: null
        property var idlInstructionReceipt: null
        property var idlInstructionReceiptTarget: null
        property int reviseCount: 0
        property int planCount: 0

        function reset() {
            idlInstructionPlanValue = null
            idlInstructionPlanError = ""
            idlInstructionPlanPending = false
            idlInstructionPreviewValue = null
            idlInstructionError = ""
            idlInstructionPreviewPending = false
            idlInstructionSubmitPending = false
            idlInstructionFrozenArtifact = null
            idlInstructionConfirmation = null
            idlInstructionReceipt = null
            idlInstructionReceiptTarget = null
            reviseCount = 0
            planCount = 0
        }

        function reviseIdlInstructionDraft(entry, request, targetDisplay) {
            reviseCount += 1
            return true
        }

        function planIdlInstruction() {
            planCount += 1
            return planCount
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
            return true
        }
    }

    QtObject {
        id: appModel

        property var zoneInspection: dashboardZoneState
        property var registeredIdls: registeredIdlModel
        property var programExecution: programExecutionMock
        property string programTab: ""
        property string selectedView: ""

        function selectView(view) {
            selectedView = String(view || "")
        }

        function idlEntryAt(index) {
            return index >= 0 && index < registeredIdls.count
                ? registeredIdls.get(index) : null
        }

        function idlEntryForKey(key) {
            for (let index = 0; index < registeredIdls.count; ++index) {
                const entry = registeredIdls.get(index)
                if (String(entry.key || "") === String(key || "")) {
                    return entry
                }
            }
            return null
        }
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: 1180
        height: 860
        color: theme.background

        ScrollView {
            anchors.fill: parent
            leftPadding: theme.pageMargin
            rightPadding: theme.pageMargin
            topPadding: theme.gapLarge
            bottomPadding: theme.gapLarge
            contentWidth: availableWidth

            SequencerDashboardPage {
                id: page

                theme: theme
                model: appModel
                initialTab: dashboardZoneState.requestedDetailTab
                width: parent ? parent.width : 1100
            }
        }
    }

    function init() {
        page.currentTab = ""
        wait(0)
        zoneState.verification = "verified"
        zoneState.activeZoneId = FixtureData.identity("1")
        zoneState.zoneDetail = FixtureData.detailFor(zoneState.activeZoneId)
        zoneState.resetL2Fixture()
        dashboardZoneState.requestedL2View = ""
        dashboardZoneState.requestedDetailTab = ""
        zoneState.l2TransactionId = ""
        zoneState.l2TransactionRequestedSourceId = ""
        zoneState.clearTransactionOnBlockRefresh = false
        registeredIdlModel.clear()
        programExecutionMock.reset()
        appModel.programTab = ""
        appModel.selectedView = ""
        page.currentTab = "blocks"
        wait(0)
    }

    function test_accounts_show_only_provisional_snapshot_and_idl_decode() {
        page.currentTab = "accounts"
        tryVerify(function () {
            return findChild(page, "sequencerAccounts") !== null
        })
        const snapshot = findChild(page, "sequencerAccountSnapshot")
        verify(snapshot !== null)
        compare(snapshot.snapshot.source.source_role, "sequencer")
        verify(hasVisibleText(snapshot, "Provisional Snapshot"))
        verify(hasVisibleText(snapshot, "IDL Decode"))
        verify(!hasVisibleText(page, "Finalized Snapshot"))
        verify(findChild(page, "zoneL2FinalizedAccountSnapshot") === null)
    }

    function test_blocks_are_qualified_to_selected_sequencer() {
        tryVerify(function () {
            return findChild(page, "zoneL2Blocks") !== null
        })
        compare(zoneState.l2BlocksExactSourceId,
            zoneState.l2SequencerSourceId())
        verify(zoneState.l2BlockRows.length > 0)
        for (let i = 0; i < zoneState.l2BlockRows.length; ++i) {
            const observations = zoneState.l2BlockRows[i].observations
            compare(observations.length, 1)
            compare(observations[0].source_role, "sequencer")
            compare(observations[0].source_id,
                zoneState.l2SequencerSourceId())
        }
    }

    function test_page_excludes_indexer_and_l1_surfaces() {
        verify(!hasVisibleText(page, "Transfers"))
        verify(!hasVisibleText(page, "L1 Evidence"))
        verify(!hasVisibleText(page, "Historical Snapshot"))
        page.currentTab = "programs"
        tryVerify(function () {
            return findChild(page, "zoneL2Programs") !== null
        })
        verify(hasVisibleText(page, "Selected Sequencer"))
    }

    function test_interact_empty_registry_routes_to_idl_registration() {
        page.currentTab = "programs"
        let programs = null
        tryVerify(function () {
            programs = findChild(page, "zoneL2Programs")
            return programs !== null
        })
        programs.currentTool = "interact"

        let interaction = null
        let openIdls = null
        tryVerify(function () {
            interaction = findChild(programs, "zoneL2ProgramInteraction")
            openIdls = findChild(programs, "zoneProgramOpenIdlsButton")
            return interaction !== null && interaction.visible
                && openIdls !== null && openIdls.visible
        })
        compare(registeredIdlModel.count, 0)
        compare(programExecutionMock.planCount, 0)

        openIdls.clicked()

        compare(appModel.programTab, "idls")
        compare(appModel.selectedView, "programs")
        compare(programExecutionMock.planCount, 0)
    }

    function test_hidden_interaction_does_not_plan_registered_idl() {
        registeredIdlModel.append({
            key: "token-idl",
            name: "Token",
            programIdHex: FixtureData.identity("a"),
            programBinary: "",
            json: "{}"
        })
        page.currentTab = "programs"
        let programs = null
        tryVerify(function () {
            programs = findChild(page, "zoneL2Programs")
            return programs !== null && programs.currentTool === "programs"
        })
        compare(findChild(programs, "zoneL2ProgramInteraction"), null)
        compare(programExecutionMock.reviseCount, 0)
        compare(programExecutionMock.planCount, 0)

        programs.currentTool = "interact"
        tryVerify(function () {
            return findChild(programs, "zoneL2ProgramInteraction") !== null
                && programExecutionMock.reviseCount === 1
                && programExecutionMock.planCount === 1
        })
    }

    function test_transaction_readback_rejects_wrong_source_and_opens_exact_selected_source() {
        page.selectTab("programs")
        const transactionId = FixtureData.identity("e")
        const selectedSource = zoneState.l2SequencerSourceId()
        verify(selectedSource.length > 0)

        verify(!page.inspectSubmittedTransaction(transactionId, "wrong-source"))
        compare(page.currentTab, "programs")
        compare(dashboardZoneState.requestedL2View, "")
        compare(zoneState.l2TransactionId, "")
        compare(zoneState.l2TransactionRequestedSourceId, "")

        verify(page.inspectSubmittedTransaction(transactionId, selectedSource))
        compare(page.currentTab, "blocks")
        compare(dashboardZoneState.requestedL2View, "transaction")
        compare(zoneState.l2TransactionId, transactionId)
        compare(zoneState.l2TransactionRequestedSourceId, selectedSource)
        compare(dashboardZoneState.requestedDetailTab, "l2")
        compare(zoneState.l2TransactionDetail.source.source_id, selectedSource)

        tryVerify(function () {
            const detail = findChild(page, "zoneL2TransactionDetail")
            return detail !== null && detail.visible
        })
    }

    function test_external_submission_route_preserves_readback_before_blocks_loader() {
        page.selectTab("programs")
        compare(dashboardZoneState.requestedDetailTab, "programs")
        const transactionId = FixtureData.identity("f")
        const selectedSource = zoneState.l2SequencerSourceId()
        zoneState.l2BlocksLoaded = false
        zoneState.clearTransactionOnBlockRefresh = true
        const refreshCount = zoneState.l2RefreshCount

        verify(zoneState.openL2Transaction(transactionId, selectedSource) !== null)
        dashboardZoneState.requestedL2View = "transaction"
        dashboardZoneState.requestedDetailTab = "l2"

        tryCompare(page, "currentTab", "blocks")
        compare(zoneState.l2RefreshCount, refreshCount)
        compare(zoneState.l2TransactionId, transactionId)
        compare(zoneState.l2TransactionRequestedSourceId, selectedSource)
        verify(zoneState.l2TransactionDetail !== null)
        tryVerify(function () {
            const detail = findChild(page, "zoneL2TransactionDetail")
            return detail !== null && detail.visible
        })
    }

    function hasVisibleText(item, value) {
        if (!item) {
            return false
        }
        if (item.visible !== false && item.text !== undefined
                && String(item.text).indexOf(value) >= 0) {
            return true
        }
        const children = item.children || []
        for (let i = 0; i < children.length; ++i) {
            if (hasVisibleText(children[i], value)) {
                return true
            }
        }
        return false
    }
}
