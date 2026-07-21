pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property var zoneState
    property bool interactionBlocked: false

    property string pendingAction: ""
    property string pendingChannelId: ""
    property bool actionConfirmationOpen: false
    property bool configurationOpen: false
    readonly property var node: root.zoneState.managedIndexerNode || ({})
    readonly property var runtime: root.zoneState.managedIndexerRuntime || ({})
    readonly property var availableActions: Array.isArray(root.node.available_actions)
        ? root.node.available_actions : []
    readonly property string runState: String(root.node.indexer_state
        || root.node.run_state || "not_initialized")
    readonly property string installedState: String(root.node.install_state || "needs_configuration")
    readonly property string managedChannelId: String(root.node.managed_channel_id || "")
    readonly property string selectedChannelId: String(root.zoneState.activeZoneId || "")
    readonly property var configurationPanel: configurationLoader.item || null
    readonly property bool hasDirtyDraft: root.configurationPanel !== null
        && root.configurationPanel.dirty === true
    readonly property bool configurationBusy: root.zoneState.managedIndexerConfigLoading === true
        || root.zoneState.managedIndexerConfigSaving === true
    readonly property bool actionInFlight: root.zoneState.managedIndexerControlInFlight === true
        || root.zoneState.managedIndexerRefreshInFlight === true
        || root.configurationBusy
    readonly property bool runtimeRunning: String(root.runtime.run_state || "") === "running"
    readonly property bool installed: root.installedState === "installed"
    readonly property bool canStart: !root.actionInFlight
        && !root.interactionBlocked
        && root.zoneState.managedIndexerStatusStale !== true
        && root.zoneState.verification === "verified"
        && root.availableActions.indexOf("start") >= 0
        && root.installed
        && (root.runState === "stopped" || root.runState === "not_initialized")
    readonly property bool canStop: !root.actionInFlight
        && !root.interactionBlocked
        && root.zoneState.managedIndexerStatusStale !== true
        && root.availableActions.indexOf("stop") >= 0 && root.runtimeRunning
        && root.managedChannelId.length > 0
    readonly property bool canConfigure: !root.actionInFlight
        && !root.interactionBlocked
        && root.zoneState.managedIndexerStatusStale !== true
        && (root.runState === "stopped" || root.runState === "not_initialized")

    objectName: "managedIndexerControl"
    spacing: root.theme.gapSmall
    Layout.fillWidth: true

    Component.onCompleted: root.zoneState.refreshManagedIndexer()

    Timer {
        objectName: "managedIndexerStatusRefreshTimer"

        interval: 2500
        repeat: true
        running: root.visible && !root.actionInFlight && !root.configurationOpen
            && !root.hasDirtyDraft && !root.actionConfirmationOpen
        onTriggered: root.zoneState.refreshManagedIndexer()
    }

    Text {
        text: qsTr("Managed Channel Indexer")
        color: root.theme.text
        textFormat: Text.PlainText
        font.pixelSize: root.theme.secondaryText
        font.weight: Font.DemiBold
        Layout.fillWidth: true
    }

    Text {
        text: qsTr("Each Channel uses an isolated Inspector-managed LogosCore runtime. The selected Sequencer source is recorded as its configuration binding; Indexer follows finalized Bedrock data.")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 1 : 3
        columnSpacing: root.theme.gapSmall
        rowSpacing: root.theme.gapSmall
        Layout.fillWidth: true

        StatusChip {
            objectName: "managedIndexerPackageStatus"
            theme: root.theme
            label: qsTr("Package")
            value: root.installed
                ? String(root.node.package_version || qsTr("Installed"))
                : qsTr("Not installed")
            detail: String(root.node.detail || "")
            tone: root.installed ? "success" : "warning"
            compact: true
            showIndicator: true
            Layout.fillWidth: true
        }

        StatusChip {
            objectName: "managedIndexerRuntimeStatus"
            theme: root.theme
            label: qsTr("LogosCore")
            value: root.statusLabel(String(root.runtime.run_state || "not configured"))
            detail: String(root.runtime.detail || "")
            tone: root.runtimeRunning ? "success" : "warning"
            compact: true
            showIndicator: true
            Layout.fillWidth: true
        }

        StatusChip {
            objectName: "managedIndexerRunStatus"
            theme: root.theme
            label: qsTr("Indexer")
            value: root.statusLabel(root.runState)
            detail: root.managedChannelId
            tone: root.runTone()
            compact: true
            showIndicator: true
            Layout.fillWidth: true
        }
    }

    Text {
        visible: root.managedChannelId.length > 0
            || String(root.node.indexer_head || "").length > 0
        text: {
            const facts = []
            if (root.managedChannelId.length > 0) {
                facts.push(qsTr("Channel %1").arg(root.managedChannelId))
            }
            const head = String(root.node.indexer_head || "")
            if (head.length > 0) {
                facts.push(qsTr("indexed block %1").arg(head))
            }
            return facts.join(" · ")
        }
        color: root.theme.textMuted
        textFormat: Text.PlainText
        wrapMode: Text.WrapAnywhere
        font.family: "monospace"
        font.pixelSize: root.theme.dataText
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: !root.installed
        theme: root.theme
        tone: "warning"
        title: qsTr("Indexer package required")
        message: qsTr("Install an exact lez_indexer_module version under System / Local Nodes, then start the managed LogosCore runtime.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.installed && !root.runtimeRunning
        theme: root.theme
        tone: "info"
        title: qsTr("Isolated runtime starts on demand")
        message: qsTr("Starting this Channel starts only its own Inspector-managed LogosCore runtime.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: String(root.node.indexer_error || "").length > 0
        theme: root.theme
        tone: "error"
        title: qsTr("Indexer reported an error")
        message: String(root.node.indexer_error || "")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: String(root.zoneState.managedIndexerError || "").length > 0
        theme: root.theme
        tone: "error"
        title: qsTr("Managed Indexer action failed")
        message: String(root.zoneState.managedIndexerError || "")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: String(root.zoneState.managedIndexerResult || "").length > 0
            && String(root.zoneState.managedIndexerError || "").length === 0
        theme: root.theme
        tone: "success"
        title: qsTr("Managed Indexer action accepted")
        message: String(root.zoneState.managedIndexerResult || "")
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            objectName: "refreshManagedIndexerButton"
            theme: root.theme
            text: qsTr("Refresh")
            enabled: !root.actionInFlight && !root.configurationOpen
            onClicked: root.zoneState.refreshManagedIndexer()
        }

        Item {
            Layout.fillWidth: true
        }

        ActionButton {
            objectName: "configureManagedIndexerButton"
            theme: root.theme
            text: root.configurationOpen ? qsTr("Close configuration") : qsTr("Configure")
            enabled: root.configurationOpen
                ? !root.configurationBusy && !root.hasDirtyDraft
                : root.canConfigure
            onClicked: {
                if (root.configurationOpen) {
                    root.closeConfiguration()
                } else {
                    root.configurationOpen = true
                }
            }
        }

        ActionButton {
            objectName: "stopManagedIndexerButton"
            theme: root.theme
            text: qsTr("Stop")
            enabled: root.canStop && !root.configurationOpen && !root.hasDirtyDraft
            onClicked: root.confirmAction("stop", root.managedChannelId)
        }

        ActionButton {
            objectName: "startManagedIndexerButton"
            theme: root.theme
            text: qsTr("Start for this Channel")
            primary: true
            enabled: root.canStart && !root.configurationOpen && !root.hasDirtyDraft
            onClicked: root.confirmAction("start", root.selectedChannelId)
        }
    }

    Loader {
        id: configurationLoader

        property var configurationTheme: root.theme
        property var configurationZoneState: root.zoneState

        active: root.configurationOpen
        asynchronous: false
        Layout.fillWidth: true
        sourceComponent: ChannelIndexerConfigurationPanel {
            theme: configurationLoader.configurationTheme
            zoneState: configurationLoader.configurationZoneState
        }
    }

    ConfirmActionPopup {
        id: actionPopup

        objectName: "managedIndexerActionConfirmation"
        theme: root.theme
        title: root.pendingAction === "start"
            ? qsTr("Start Channel Indexer") : qsTr("Stop Channel Indexer")
        message: root.pendingAction === "start"
            ? qsTr("Start an isolated lez_indexer_module runtime for Channel %1 using Bedrock %2?")
                .arg(root.pendingChannelId).arg(root.zoneState.bedrockEndpoint())
            : qsTr("Stop the isolated managed Indexer for Channel %1?")
                .arg(root.pendingChannelId)
        confirmText: root.pendingAction === "start" ? qsTr("Start") : qsTr("Stop")
        confirmEnabled: root.pendingAction.length > 0 && !root.actionInFlight
        onAccepted: {
            const action = root.pendingAction
            const channelId = root.pendingChannelId
            root.pendingAction = ""
            root.pendingChannelId = ""
            root.zoneState.runManagedIndexerAction(action, channelId)
        }
        onClosed: Qt.callLater(function () {
            root.actionConfirmationOpen = false
        })
    }

    function confirmAction(action, channelId) {
        actionConfirmationOpen = true
        pendingAction = String(action || "")
        pendingChannelId = String(channelId || "")
        actionPopup.open()
    }

    function closeConfiguration() {
        if (root.hasDirtyDraft) {
            root.zoneState.managedIndexerConfigError = qsTr("Save or undo Channel Indexer configuration changes before closing it.")
            return false
        }
        root.configurationOpen = false
        return true
    }

    function discardDraft() {
        if (root.configurationPanel && typeof root.configurationPanel.undoDraft === "function") {
            root.configurationPanel.undoDraft()
        }
        root.configurationOpen = false
    }

    function statusLabel(value) {
        const text = String(value || "unknown").replace(/_/g, " ")
        return text.length ? text[0].toUpperCase() + text.slice(1) : qsTr("Unknown")
    }

    function runTone() {
        if (runState === "running" || runState === "caught_up") {
            return "success"
        }
        if (runState === "starting" || runState === "syncing" || runState === "stopping") {
            return "warning"
        }
        if (runState === "failed" || runState === "error" || runState === "stalled") {
            return "error"
        }
        return "neutral"
    }
}
