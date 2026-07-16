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
        id: appModel

        property var zoneInspection: zoneState
        property string selectedView: ""

        function selectView(view) {
            selectedView = String(view || "")
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
                width: parent ? parent.width : 1100
            }
        }
    }

    function init() {
        zoneState.verification = "verified"
        zoneState.activeZoneId = FixtureData.identity("1")
        zoneState.zoneDetail = FixtureData.detailFor(zoneState.activeZoneId)
        zoneState.resetL2Fixture()
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
