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

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

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
            value: root.model.deliverySourceLabel()
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
            value: root.deliveryState.lastOperation
            tone: root.model.resultIsError && root.model.resultOwner === root.model.currentView ? "error" : "neutral"
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
        title: root.model.resultIsError ? qsTr("Operation error") : qsTr("Operation result")

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.resultTitle
                color: root.model.resultIsError ? root.theme.error : root.theme.textMuted
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
                onClicked: root.model.clearResult()
            }
        }

        TextArea {
            readOnly: true
            text: root.model.resultText.length ? root.model.resultText : qsTr("No response body.")
            wrapMode: TextArea.Wrap
            color: root.model.resultIsError ? root.theme.warning : root.theme.text
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
                color: root.model.resultIsError ? root.theme.errorMuted : root.theme.field
                radius: root.theme.radius
                border.width: 1
                border.color: root.model.resultIsError ? root.theme.error : root.theme.outline
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

            StatusMessage {
                visible: root.deliveryMessageSource() && !root.model.messagingMutatingDiagnosticsEnabled
                theme: root.theme
                tone: "warning"
                title: qsTr("Mutating diagnostics off")
                message: qsTr("Enable mutating diagnostics in Settings before subscribe, unsubscribe, or send.")
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
        title: root.deliveryState.pendingLabel
        message: qsTr("This will call the configured Delivery source and may change node relay state.")
        confirmText: root.deliveryState.pendingLabel
        confirmEnabled: root.deliveryState.pendingMethod.length > 0
        onAccepted: root.runPendingDelivery()
    }

    Component {
        id: nodeTab

        Panel {
            theme: root.theme
            title: qsTr("Node")

            TextAreaField {
                id: nodeConfig

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
                    text: qsTr("Create")
                    primary: true
                    enabled: !root.model.busy && !root.activeDeliveryOperationRunning() && root.deliveryModuleSource() && nodeConfig.text.trim().length > 0
                    Layout.preferredWidth: 112
                    onClicked: root.confirmDelivery("deliveryCreateNode", [nodeConfig.text.trim()], qsTr("Create node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Start")
                    enabled: !root.model.busy && !root.activeDeliveryOperationRunning() && root.deliveryModuleSource()
                    Layout.preferredWidth: 96
                    onClicked: root.confirmDelivery("deliveryStart", [], qsTr("Start node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Stop")
                    enabled: !root.model.busy && !root.activeDeliveryOperationRunning() && root.deliveryModuleSource()
                    Layout.preferredWidth: 96
                    onClicked: root.confirmDelivery("deliveryStop", [], qsTr("Stop node"))
                }

                ActionButton {
                    theme: root.theme
                    text: qsTr("Settings")
                    enabled: !root.model.busy
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
                            root.model.createSocialIdentity(identityName.text)
                            identityName.text = ""
                        }
                    }

                    ActionButton {
                        theme: root.theme
                        text: root.model.socialIdentityDefaultMode === "manual" ? qsTr("Manual default") : qsTr("Per-topic default")
                        selected: root.model.socialIdentityDefaultMode !== "manual"
                        Layout.preferredWidth: 172
                        onClicked: root.model.setSocialIdentityDefaultMode(root.model.socialIdentityDefaultMode === "manual" ? "perConversation" : "manual")
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
                            border.color: root.model.selectedSocialIdentityKey === String(identityFrame.modelData.key || "") ? root.theme.accent : root.theme.outlineMuted
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
                                text: root.model.selectedSocialIdentityKey === String(identityFrame.modelData.key || "") ? qsTr("Selected") : qsTr("Use")
                                selected: root.model.selectedSocialIdentityKey === String(identityFrame.modelData.key || "")
                                Layout.preferredWidth: 104
                                onClicked: root.model.selectSocialIdentity(identityFrame.modelData.key)
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
                    enabled: !root.model.busy && root.deliveryRestSource()
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
                    model: root.deliveryState.operationRows()

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
        const revision = root.model.socialIdentityRevision
        return root.model.socialIdentityRows()
    }

    function identityStatusText() {
        if (root.model.socialIdentityDefaultMode === "manual") {
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

    function activeDeliveryOperationRunning() {
        return root.deliveryState.activeDeliveryOperationRunning()
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
