pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../theme"
import "../ZonePresentation.js" as Presentation

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    property string initialTab: "overview"
    property bool sourceEditorInitiallyOpen: false
    property string currentTab: initialTab
    property string pendingTab: ""
    readonly property var detail: root.zoneState.zoneDetail
    readonly property var zone: root.detail && root.detail.summary ? root.detail.summary : ({})
    readonly property bool hasDirtyDraft: sourceLoader.section !== null
        && sourceLoader.section.hasDirtyDraft

    objectName: "zoneDetail"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    ListModel {
        id: detailTabs

        ListElement { value: "overview"; label: "Overview" }
        ListElement { value: "sources"; label: "Sources" }
        ListElement { value: "evidence"; label: "L1 Evidence" }
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ColumnLayout {
            spacing: root.theme.gapTiny
            Layout.fillWidth: true

            Text {
                text: Presentation.title(root.zone)
                color: root.theme.text
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.panelTitleText
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("%1 / last L1 slot %2")
                    .arg(Presentation.words(root.zone.activity_state))
                    .arg(Presentation.numberText(root.zone.activity_detail
                        && root.zone.activity_detail.last_l1_slot))
                color: root.theme.textMuted
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.dataText
                Layout.fillWidth: true
            }
        }

        ZoneKindChip {
            theme: root.theme
            label: Presentation.kindLabel(root.zone.kind)
            tone: Presentation.stateTone(root.zone, root.zoneState.detailStale)
        }
    }

    ZoneCompactStatus {
        theme: root.theme
        zone: root.zone
        stale: root.zoneState.detailStale
        Layout.fillWidth: true
    }

    Text {
        visible: root.zoneState.detailStale
        text: qsTr("Detail is refreshing against current catalog facts")
        color: root.theme.warning
        textFormat: Text.PlainText
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    TabSwitch {
        theme: root.theme
        options: detailTabs
        current: root.currentTab
        onSelected: function (value) {
            root.requestTab(value)
        }
    }

    Loader {
        active: root.currentTab === "overview"
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: ZoneOverview {
            theme: root.theme
            detail: root.detail
        }
    }

    Loader {
        id: sourceLoader

        readonly property ChannelSourcesSection section: item as ChannelSourcesSection

        active: root.currentTab === "sources"
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: ChannelSourcesSection {
            theme: root.theme
            zoneState: root.zoneState
            detail: root.detail
        }
        onLoaded: {
            if (root.sourceEditorInitiallyOpen && section) {
                section.beginEditor("sequencer", null)
            }
        }
    }

    Loader {
        active: root.currentTab === "evidence"
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: ZoneEvidenceViewer {
            theme: root.theme
            zoneState: root.zoneState
            detail: root.detail
        }
    }

    ConfirmActionPopup {
        id: tabGuardPopup

        theme: root.theme
        title: qsTr("Discard source draft")
        message: qsTr("Discard unsaved Channel source changes?")
        confirmText: qsTr("Discard")
        onAccepted: {
            root.discardSourceDraft()
            root.currentTab = root.pendingTab
            root.pendingTab = ""
        }
    }

    function requestTab(value) {
        const nextTab = String(value || "overview")
        if (nextTab === currentTab) {
            return true
        }
        if (hasDirtyDraft) {
            pendingTab = nextTab
            tabGuardPopup.open()
            return false
        }
        currentTab = nextTab
        return true
    }

    function discardSourceDraft() {
        if (sourceLoader.section) {
            sourceLoader.section.discardDraft()
        }
    }
}
