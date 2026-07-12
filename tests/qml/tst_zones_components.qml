import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml/features/zones/pages"
import "../../qml/theme"
import "fixtures"
import "fixtures/ZoneFixtureData.js" as FixtureData

TestCase {
    id: testRoot

    name: "ZonesComponents"
    when: windowShown
    width: 1280
    height: 900

    Theme {
        id: theme
    }

    ZoneStateFixture {
        id: zoneState
    }

    QtObject {
        id: appModel

        property var zoneInspection: zoneState
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

    function init() {
        zoneState.verification = "verified"
        zoneState.summaryStale = false
        zoneState.activeZoneId = FixtureData.identity("1")
        zoneState.zoneDetail = FixtureData.detailFor(zoneState.activeZoneId)
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
        page.filter = "all"
        page.query = ""
        const detail = findChild(page, "zoneDetail")
        if (detail) {
            detail.discardSourceDraft()
            detail.currentTab = "overview"
        }
        wait(0)
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
        verify(hasVisibleText(transactionDetail, "Content hash and signature checks"))
        verify(hasVisibleText(transactionDetail, "0. Parse transaction"))
        verify(hasVisibleText(transactionDetail,
            zoneState.l2TransactionDetail.source.source_id))
    }

    function test_l2_accounts_keep_snapshots_separate_and_activity_oldest_first() {
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("accounts"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Accounts") !== null
        })
        const accounts = findChild(detail, "zoneL2Accounts")
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
        verify(page.requestZoneActivation(FixtureData.identity("8")))
        const detail = findChild(page, "zoneDetail")
        verify(detail.requestTab("l2"))
        tryVerify(function () {
            return findChild(detail, "zoneL2Inspector") !== null
        })
        const inspector = findChild(detail, "zoneL2Inspector")
        verify(hasVisibleText(inspector, "L2 not applicable"))
        verify(hasVisibleText(inspector, "L2 reads do not apply to this Channel type."))
        compare(zoneState.l2RefreshCount, 0)
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
}
