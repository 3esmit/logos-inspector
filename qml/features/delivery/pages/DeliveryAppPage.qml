pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    readonly property var deliveryState: root.model.deliveryApp
    readonly property var socialIdentityView: root.model.social.identitiesView()
    property bool deliveryConfirmationAccepted: false

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    Component.onCompleted: root.deliveryState.refreshManagedNodeState()
    onVisibleChanged: {
        if (visible) {
            root.deliveryState.refreshManagedNodeState()
        }
    }

    Connections {
        target: root.deliveryState

        function onSourceModeChanged() {
            root.deliveryState.refreshManagedNodeState()
        }
    }

    ListModel {
        id: deliveryTabs

        ListElement { value: "messages"; label: "Messages" }
        ListElement { value: "identity"; label: "Identity" }
        ListElement { value: "store"; label: "Store" }
        ListElement { value: "node"; label: "Node" }
        ListElement { value: "operations"; label: "Operations" }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Network / Delivery")
        title: qsTr("Delivery")
        layerLabel: qsTr("Network")
        subtitle: qsTr("Inspect Delivery health, node state, subscriptions, sends, and module events through the configured source.")
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: root.sourceBadges()
        Layout.fillWidth: true
    }

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        StatusChip {
            theme: root.theme
            label: qsTr("Source")
            value: root.model.sourceRouting.deliverySourceLabel()
            tone: root.deliveryDataSource() ? "success" : "warning"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Topic")
            value: root.topicShortText(root.deliveryState.activeTopic)
            tone: root.validContentTopic(root.deliveryState.activeTopic) ? "success" : "warning"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Network")
            value: root.model.messagingNetworkPreset
            tone: "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Events")
            value: root.model.deliveryModuleEventSummary()
            tone: root.model.deliveryConnectionStatus.length > 0 ? "success" : "neutral"
            Layout.fillWidth: true
        }

        StatusChip {
            theme: root.theme
            label: qsTr("Last")
            value: root.deliveryState.displayedLastOperation()
            tone: root.model.shell.resultIsError && root.model.shell.resultOwner === root.model.shell.currentView ? "error" : "neutral"
            Layout.fillWidth: true
        }
    }

    TabSwitch {
        theme: root.theme
        current: root.model.deliveryAppTab
        options: deliveryTabs
        Layout.fillWidth: true
        onSelected: value => root.model.deliveryAppTab = value
    }

    Loader {
        active: true
        sourceComponent: root.tabComponent(root.model.deliveryAppTab)
        Layout.fillWidth: true
    }

    Panel {
        visible: root.model.pageHasOutput("messaging")
        theme: root.theme
        title: root.model.shell.resultIsError ? qsTr("Operation error") : qsTr("Operation result")

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.shell.resultTitle
                color: root.model.shell.resultIsError ? root.theme.error : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.secondaryText
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Clear")
                Layout.preferredWidth: 84
                onClicked: root.model.shell.clearResult()
            }
        }

        TextArea {
            readOnly: true
            text: root.model.shell.resultText.length ? root.model.shell.resultText : qsTr("No response body.")
            wrapMode: TextArea.Wrap
            color: root.model.shell.resultIsError ? root.theme.warning : root.theme.text
            selectedTextColor: root.theme.selectedText
            selectionColor: root.theme.accent
            textFormat: Text.PlainText
            font.family: "monospace"
            font.pixelSize: root.theme.secondaryText
            leftPadding: 12
            rightPadding: 12
            topPadding: 10
            bottomPadding: 10
            Layout.fillWidth: true
            Layout.preferredHeight: 220

            background: Rectangle {
                color: root.model.shell.resultIsError ? root.theme.errorMuted : root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.model.shell.resultIsError ? root.theme.error : root.theme.outline
            }
        }
    }

    Component {
        id: messagesTab

        Panel {
            theme: root.theme
            title: qsTr("Messages")

            StatusMessage {
                visible: !root.deliveryMessageSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("Message source required")
                message: qsTr("Subscribe, unsubscribe, and send use Direct Waku REST or Delivery module source.")
                Layout.fillWidth: true
            }

            FieldRow {
                id: topicField

                theme: root.theme
                label: qsTr("Content topic")
                placeholderText: qsTr("/myapp/1/chat/proto")
                sourceText: root.deliveryState.activeTopic
                syncSourceText: true
                Layout.fillWidth: true
                onTextEdited: text => root.deliveryState.activeTopic = text
            }

            TextAreaField {
                id: payloadField

                theme: root.theme
                label: qsTr("Payload")
                rows: 5
                placeholderText: qsTr("message")
                Layout.fillWidth: true
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Subscribe")
                    primary: true
                    enabled: root.messageControlsEnabled(topicField.text)
                    Layout.preferredWidth: 124
                    onClicked: root.confirmDelivery("deliverySubscribe", [topicField.text.trim()], qsTr("Subscribe"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Unsubscribe")
                    enabled: root.messageControlsEnabled(topicField.text)
                    Layout.preferredWidth: 136
                    onClicked: root.confirmDelivery("deliveryUnsubscribe", [topicField.text.trim()], qsTr("Unsubscribe"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Send")
                    enabled: root.messageControlsEnabled(topicField.text) && payloadField.text.length > 0
                    Layout.preferredWidth: 104
                    onClicked: root.confirmDelivery("deliverySend", [topicField.text.trim(), payloadField.text], qsTr("Send message"))
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }

    ConfirmActionPopup {
        id: deliveryConfirm

        theme: root.theme
        title: root.deliveryState.nodeConfirmationTitle()
        message: root.deliveryState.nodeConfirmationMessage()
        confirmText: root.deliveryState.nodeConfirmationText()
        confirmEnabled: root.deliveryState.nodeConfirmationEnabled()
        onAccepted: {
            root.deliveryConfirmationAccepted = true
            root.deliveryState.runPendingNodeAction()
        }
        onClosed: Qt.callLater(function () {
            if (!root.deliveryConfirmationAccepted) {
                root.deliveryState.clearNodeConfirmation()
            }
            root.deliveryConfirmationAccepted = false
        })
    }

    Component {
        id: nodeTab

        Panel {
            theme: root.theme
            title: qsTr("Node")

            StatusMessage {
                visible: root.deliveryState.managedNodeLifecycleSource()
                theme: root.theme
                tone: root.deliveryState.managedNodeStatusTone()
                title: qsTr("Managed Messaging")
                message: root.deliveryState.managedNodeStatusText()
                Layout.fillWidth: true
            }

            StatusMessage {
                visible: !root.deliveryModuleSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("LogosCore CLI required")
                message: qsTr("Select LogosCore CLI in Messaging / Delivery Settings to create, start, or stop the managed Messaging node.")
                Layout.fillWidth: true
            }

            StatusMessage {
                visible: root.deliveryModuleSource()
                    && !root.deliveryState.managedNodeLifecycleSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("Host module lifecycle")
                message: qsTr("Create and Start call the host Delivery module. Stop is unavailable because the native stop callback is not a terminal shutdown boundary.")
                Layout.fillWidth: true
            }

            TextAreaField {
                id: nodeConfig

                visible: root.deliveryModuleSource()
                    && !root.deliveryState.managedNodeLifecycleSource()
                theme: root.theme
                label: qsTr("Config JSON")
                rows: 7
                text: root.defaultNodeConfig()
                Layout.fillWidth: true
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: root.deliveryState.nodeActionLabel("create")
                    primary: true
                    enabled: root.deliveryState.nodeActionEnabled("create")
                        && (root.deliveryState.managedNodeLifecycleSource()
                            || nodeConfig.text.trim().length > 0)
                    Layout.preferredWidth: 112
                    onClicked: {
                        if (root.deliveryState.confirmNodeAction("create", nodeConfig.text)) {
                            deliveryConfirm.open()
                        }
                    }
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Start")
                    enabled: root.deliveryState.nodeActionEnabled("start")
                    Layout.preferredWidth: 96
                    onClicked: {
                        if (root.deliveryState.confirmNodeAction("start", "")) {
                            deliveryConfirm.open()
                        }
                    }
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Stop")
                    visible: root.deliveryState.managedNodeLifecycleSource()
                    enabled: root.deliveryState.nodeActionEnabled("stop")
                    Layout.preferredWidth: 96
                    onClicked: {
                        if (root.deliveryState.confirmNodeAction("stop", "")) {
                            deliveryConfirm.open()
                        }
                    }
                }

                ActionButton {
                    visible: root.deliveryState.managedNodeLifecycleSource()
                    theme: root.theme
                    text: qsTr("Refresh")
                    enabled: !root.model.shell.busy
                        && (!root.deliveryState.managedNodes
                            || root.deliveryState.managedNodes.statusLoading !== true)
                    Layout.preferredWidth: 104
                    onClicked: root.deliveryState.refreshManagedNodeState()
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Settings")
                    enabled: !root.model.shell.busy
                    Layout.preferredWidth: 112
                    onClicked: root.model.openSettings("network", "messaging")
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }

    Component {
        id: identityTab

        Panel {
            theme: root.theme
            title: qsTr("Identity")

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                FieldRow {
                    id: identityName

                    theme: root.theme
                    label: qsTr("Display name")
                    placeholderText: qsTr("Pseudonym")
                    Layout.fillWidth: true
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true
                    Layout.alignment: Qt.AlignBottom

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Create")
                        primary: true
                        Layout.preferredWidth: 104
                        onClicked: {
                            root.model.social.createIdentity(identityName.text)
                            identityName.text = ""
                        }
                    }

                    ActionButton {
                        theme: root.theme
                        text: root.socialIdentityView.defaultMode === "manual" ? qsTr("Manual default") : qsTr("Per-topic default")
                        selected: root.socialIdentityView.defaultMode !== "manual"
                        Layout.preferredWidth: 172
                        onClicked: root.model.social.setIdentityDefaultMode(
                            root.socialIdentityView.defaultMode === "manual" ? "perConversation" : "manual")
                    }

                    Item {
                        Layout.fillWidth: true
                    }
                }
            }

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("Posting identity")
                message: root.identityStatusText()
                Layout.fillWidth: true
            }

            ColumnLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Repeater {
                    model: root.identityRows()

                    Frame {
                        id: identityFrame

                        required property var modelData

                        padding: root.theme.gap
                        Layout.fillWidth: true

                        background: Rectangle {
                            color: root.theme.field
                            radius: root.theme.radius
                            border.width: 1
                            border.color: root.model.social.selectedSocialIdentityKey === String(identityFrame.modelData.key || "") ? root.theme.accent : root.theme.outlineMuted
                        }

                        contentItem: RowLayout {
                            spacing: root.theme.gapSmall

                            ColumnLayout {
                                spacing: 2
                                Layout.fillWidth: true

                                Text {
                                    text: String(identityFrame.modelData.displayName || qsTr("Pseudonym"))
                                    color: root.theme.text
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    font.pixelSize: root.theme.primaryText
                                    font.weight: Font.DemiBold
                                    Layout.fillWidth: true
                                }

                                Text {
                                    text: String(identityFrame.modelData.localId || "")
                                    color: root.theme.textDim
                                    textFormat: Text.PlainText
                                    elide: Text.ElideMiddle
                                    font.family: "monospace"
                                    font.pixelSize: root.theme.labelText
                                    Layout.fillWidth: true
                                }
                            }

                            ActionButton {
                                theme: root.theme
                                text: root.model.social.selectedSocialIdentityKey === String(identityFrame.modelData.key || "") ? qsTr("Selected") : qsTr("Use")
                                selected: root.model.social.selectedSocialIdentityKey === String(identityFrame.modelData.key || "")
                                Layout.preferredWidth: 104
                                onClicked: root.model.social.selectIdentity(identityFrame.modelData.key)
                            }
                        }
                    }
                }
            }
        }
    }

    Component {
        id: storeTab

        Panel {
            theme: root.theme
            title: qsTr("Store")

            StatusMessage {
                visible: !root.deliveryRestSource()
                theme: root.theme
                tone: "warning"
                title: qsTr("REST source required")
                message: qsTr("Store queries use Direct Waku REST.")
                Layout.fillWidth: true
            }

            GridLayout {
                columns: root.width < 760 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                FieldRow {
                    id: storePeer

                    theme: root.theme
                    label: qsTr("Peer address")
                    placeholderText: qsTr("/ip4/127.0.0.1/tcp/60001/p2p/...")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: storeContentTopics

                    theme: root.theme
                    label: qsTr("Content topics")
                    placeholderText: qsTr("/myapp/1/chat/proto")
                    sourceText: root.deliveryState.activeTopic
                    syncSourceText: true
                    Layout.fillWidth: true
                    onTextEdited: text => root.deliveryState.activeTopic = text
                }

                FieldRow {
                    id: storePubsubTopic

                    theme: root.theme
                    label: qsTr("Pubsub topic")
                    placeholderText: qsTr("/waku/2/rs/16/32")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: storeCursor

                    theme: root.theme
                    label: qsTr("Cursor")
                    placeholderText: qsTr("optional")
                    Layout.fillWidth: true
                }

                FieldRow {
                    id: storePageSize

                    theme: root.theme
                    label: qsTr("Page size")
                    text: "20"
                    Layout.fillWidth: true
                }

                CheckBox {
                    id: storeIncludeData

                    text: qsTr("Include payloads")
                    checked: false
                    enabled: root.deliveryRestSource()
                    palette.text: root.theme.text
                    palette.windowText: enabled ? root.theme.text : root.theme.textDim
                    Layout.fillWidth: true
                }
            }

            RowLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                ActionButton {
                    theme: root.theme
                    text: qsTr("Query Store")
                    primary: true
                    enabled: !root.model.shell.busy && !root.activeDeliveryOperationBusy() && root.deliveryRestSource()
                    Layout.preferredWidth: 132
                    onClicked: root.runDelivery("deliveryStoreQuery", [
                        storePeer.text.trim(),
                        storeContentTopics.text.trim(),
                        storePubsubTopic.text.trim(),
                        storeCursor.text.trim(),
                        root.storePageSizeValue(storePageSize.text),
                        true,
                        storeIncludeData.checked
                    ], qsTr("Store query"))
                }

                Item {
                    Layout.fillWidth: true
                }
            }
        }
    }

    Component {
        id: operationsTab

        Panel {
            theme: root.theme
            title: qsTr("Operations")

            ColumnLayout {
                spacing: root.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: qsTr("Operations")
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.Medium
                    Layout.fillWidth: true
                }

                Repeater {
                    model: root.deliveryState.managedNodeLifecycleSource()
                        ? root.deliveryState.managedNodeOperationRows() : []

                    delegate: OperationHistoryRow {
                        required property var modelData

                        theme: root.theme
                        timeText: String(modelData.time || "")
                        labelText: String(modelData.label || "")
                        statusText: String(modelData.status || "")
                        detailText: String(modelData.detail || "")
                    }
                }

                Repeater {
                    model: root.deliveryState.operation.rows

                    delegate: OperationHistoryRow {
                        required property var modelData

                        theme: root.theme
                        timeText: String(modelData.time || "")
                        labelText: String(modelData.label || "")
                        statusText: String(modelData.status || "")
                        detailText: String(modelData.detail || "")
                    }
                }

                Text {
                    text: qsTr("Module events")
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: root.theme.secondaryText
                    font.weight: Font.Medium
                    Layout.fillWidth: true
                }

                Repeater {
                    model: root.model.deliveryModuleEventRows()

                    delegate: OperationHistoryRow {
                        required property var modelData

                        theme: root.theme
                        timeText: String(modelData.time || "")
                        labelText: String(modelData.label || "")
                        statusText: String(modelData.status || "")
                        detailText: String(modelData.detail || "")
                    }
                }
            }
        }
    }

    function tabComponent(tab) {
        switch (String(tab || "")) {
        case "identity":
            return identityTab
        case "store":
            return storeTab
        case "node":
            return nodeTab
        case "operations":
            return operationsTab
        default:
            return messagesTab
        }
    }

    function sourceBadges() {
        return root.deliveryState.sourceBadges()
    }

    function identityRows() {
        return root.socialIdentityView.rows
    }

    function identityStatusText() {
        if (root.socialIdentityView.defaultMode === "manual") {
            return qsTr("Manual mode reuses the selected pseudonym until changed.")
        }
        return qsTr("Per-topic mode creates a new pseudonym for each new conversation and reuses it for that topic.")
    }

    function deliveryModuleSource() {
        return root.deliveryState.deliveryModuleSource()
    }

    function deliveryRestSource() {
        return root.deliveryState.deliveryRestSource()
    }

    function deliveryMessageSource() {
        return root.deliveryState.deliveryMessageSource()
    }

    function deliveryDataSource() {
        return root.deliveryState.deliveryDataSource()
    }

    function confirmDelivery(method, args, label) {
        root.deliveryState.confirmDelivery(method, args, label)
        deliveryConfirm.open()
    }

    function runPendingDelivery() {
        return root.deliveryState.runPendingDelivery()
    }

    function runDelivery(method, args, label) {
        return root.deliveryState.runDelivery(method, args, label)
    }

    function activeDeliveryOperationBusy() {
        return root.deliveryState.operation.busy
    }

    function messageControlsEnabled(topic) {
        return root.deliveryState.messageControlsEnabled(topic)
    }

    function validContentTopic(topic) {
        return root.deliveryState.validContentTopic(topic)
    }

    function topicShortText(topic) {
        return root.deliveryState.topicShortText(topic)
    }

    function storePageSizeValue(value) {
        return root.deliveryState.storePageSizeValue(value)
    }

    function defaultNodeConfig() {
        return root.deliveryState.defaultNodeConfig()
    }

}
