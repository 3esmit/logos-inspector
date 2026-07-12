pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    property string initialView: "blocks"
    property string currentView: initialView

    signal configureSourcesRequested()

    objectName: "zoneL2Inspector"
    spacing: root.theme.gapLarge
    Layout.fillWidth: true

    onInitialViewChanged: root.currentView = root.initialView

    Component.onCompleted: root.ensureLoaded()

    Connections {
        target: root.zoneState

        function onActiveZoneContextChanged() {
            root.currentView = root.initialView
            Qt.callLater(root.ensureLoaded)
        }
    }

    StatusMessage {
        visible: !root.zoneState.l2ReadEnabled
        theme: root.theme
        tone: root.zoneState.l2Applicable ? "warning" : "info"
        title: root.zoneState.l2Applicable
            ? qsTr("L2 source required") : qsTr("L2 not applicable")
        message: root.zoneState.l2AvailabilityMessage()
        Layout.fillWidth: true
    }

    ActionButton {
        visible: root.zoneState.l2Applicable && !root.zoneState.l2SourceConfigured
        theme: root.theme
        text: qsTr("Open Sources")
        Layout.preferredWidth: 150
        onClicked: root.configureSourcesRequested()
    }

    ZoneL2Blocks {
        visible: root.zoneState.l2ReadEnabled && root.currentView === "blocks"
        theme: root.theme
        zoneState: root.zoneState
        Layout.fillWidth: true
        onBlockRequested: function (summary, exactSourceId) {
            root.currentView = "block"
            root.zoneState.openL2Block(summary, exactSourceId)
        }
        onTransactionRequested: function (transactionId, exactSourceId) {
            root.currentView = "transaction"
            root.zoneState.openL2Transaction(transactionId, exactSourceId)
        }
        onConfigureSourcesRequested: root.configureSourcesRequested()
    }

    ZoneL2BlockDetail {
        visible: root.zoneState.l2ReadEnabled && root.currentView === "block"
        theme: root.theme
        zoneState: root.zoneState
        Layout.fillWidth: true
        onBackRequested: {
            root.zoneState.closeL2BlockDetail()
            root.currentView = "blocks"
        }
        onTransactionRequested: function (transactionId, exactSourceId) {
            root.currentView = "transaction"
            root.zoneState.openL2Transaction(transactionId, exactSourceId)
        }
        onConfigureSourcesRequested: root.configureSourcesRequested()
    }

    ZoneL2TransactionDetail {
        visible: root.zoneState.l2ReadEnabled && root.currentView === "transaction"
        theme: root.theme
        zoneState: root.zoneState
        Layout.fillWidth: true
        onBackRequested: {
            root.zoneState.closeL2Transaction()
            root.currentView = root.zoneState.l2BlockDetail !== null ? "block" : "blocks"
        }
        onConfigureSourcesRequested: root.configureSourcesRequested()
    }

    function ensureLoaded() {
        if (root.zoneState.l2ReadEnabled && !root.zoneState.l2BlocksLoaded
                && !root.zoneState.l2BlocksInFlight) {
            root.zoneState.refreshL2Blocks()
        }
    }
}
