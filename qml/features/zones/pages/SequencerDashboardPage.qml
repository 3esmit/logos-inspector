pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"
import "../controls"

ColumnLayout {
    id: root

    required property Theme theme
    required property var model
    property string initialTab: "blocks"
    property string currentTab: root.normalizedTab(root.initialTab)
    readonly property var zoneState: root.model.zoneInspection
    readonly property var l2State: root.zoneState.l2
    readonly property string sequencerSourceId: root.l2State.l2SequencerSourceId()
    readonly property bool sequencerReady: root.l2State.l2SequencerReadEnabled

    objectName: "sequencerDashboardPage"
    width: parent ? parent.width : 1180
    spacing: root.theme.gapLarge

    onInitialTabChanged: root.currentTab = root.normalizedTab(root.initialTab)

    ListModel {
        id: sequencerTabs

        ListElement { value: "blocks"; label: "Blocks / Transactions" }
        ListElement { value: "accounts"; label: "Accounts" }
        ListElement { value: "programs"; label: "Programs" }
    }

    PageHeader {
        theme: root.theme
        layerLabel: qsTr("SEQUENCER")
        breadcrumb: qsTr("Zones / Selected Sequencer")
        title: qsTr("Sequencer")
        subtitle: root.sequencerReady
            ? root.sequencerSourceId
            : qsTr("No selected Sequencer source")

        ActionButton {
            theme: root.theme
            text: qsTr("Back to Zones")
            onClicked: root.model.selectView("zones")
        }
    }

    StatusMessage {
        visible: !root.sequencerReady
        theme: root.theme
        tone: root.l2State.l2Applicable ? "warning" : "info"
        title: root.l2State.l2Applicable
            ? qsTr("Sequencer source required") : qsTr("No active Sequencer Zone")
        message: root.l2State.l2Applicable
            ? qsTr("Select a Sequencer source in this Zone before opening Sequencer data.")
            : qsTr("Select a verified Sequencer Zone from Zones.")
        Layout.fillWidth: true
    }

    ActionButton {
        visible: !root.sequencerReady
        theme: root.theme
        text: qsTr("Open Zone Sources")
        Layout.preferredWidth: 176
        onClicked: root.openSources()
    }

    RowLayout {
        visible: root.sequencerReady
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Channel")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: root.theme.labelText
            font.weight: Font.DemiBold
        }

        LinkCell {
            theme: root.theme
            text: String(root.zoneState.activeZoneId || "")
            copyText: text
            copyable: true
            link: true
            tooltipText: qsTr("Open Zone")
            Layout.fillWidth: true
            onActivated: root.model.selectView("zones")
        }
    }

    TabSwitch {
        visible: root.sequencerReady
        theme: root.theme
        options: sequencerTabs
        current: root.currentTab
        compressTabs: false
        onSelected: function (value) {
            root.currentTab = root.normalizedTab(value)
        }
    }

    Loader {
        active: root.sequencerReady && root.currentTab === "blocks"
        asynchronous: false
        visible: active
        Layout.fillWidth: true
        Layout.preferredHeight: active ? implicitHeight : 0
        Layout.maximumHeight: active ? Number.POSITIVE_INFINITY : 0
        sourceComponent: ZoneL2Inspector {
            theme: root.theme
            zoneState: root.l2State.blocks
            initialView: String(root.zoneState.requestedL2View || "blocks")
            exactSourceId: root.sequencerSourceId
            onConfigureSourcesRequested: root.openSources()
        }
    }

    Loader {
        active: root.sequencerReady && root.currentTab === "accounts"
        asynchronous: false
        visible: active
        Layout.fillWidth: true
        Layout.preferredHeight: active ? implicitHeight : 0
        Layout.maximumHeight: active ? Number.POSITIVE_INFINITY : 0
        sourceComponent: SequencerAccounts {
            theme: root.theme
            zoneState: root.l2State.accounts
            onConfigureSourcesRequested: root.openSources()
        }
    }

    Loader {
        active: root.sequencerReady && root.currentTab === "programs"
        asynchronous: false
        visible: active
        Layout.fillWidth: true
        Layout.preferredHeight: active ? implicitHeight : 0
        Layout.maximumHeight: active ? Number.POSITIVE_INFINITY : 0
        sourceComponent: ZoneL2Programs {
            theme: root.theme
            zoneState: root.l2State.tools
            onConfigureSourcesRequested: root.openSources()
        }
    }

    function normalizedTab(value) {
        const tab = String(value || "")
        if (tab === "accounts" || tab === "programs") {
            return tab
        }
        return "blocks"
    }

    function openSources() {
        root.zoneState.requestedDetailTab = "sources"
        root.model.selectView("zones")
    }
}
