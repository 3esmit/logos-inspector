pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../modules/controls"
import "../../../state"
import "../../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    SourceInspectionSession {
        id: sourceSession
        model: root.model
        theme: root.theme
        family: "delivery"
    }
    readonly property var sourceView: sourceSession.view
    readonly property string currentTab: root.model.deliveryDiagnosticsTab

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    ListModel {
        id: deliveryTabs

        ListElement { value: "overview"; label: "Overview" }
        ListElement { value: "health"; label: "Health" }
        ListElement { value: "topology"; label: "Topology" }
        ListElement { value: "throughput"; label: "Throughput" }
        ListElement { value: "protocols"; label: "Protocols" }
        ListElement { value: "topics"; label: "Topics" }
        ListElement { value: "store"; label: "Store" }
        ListElement { value: "diagnostics"; label: "Diagnostics" }
    }

    Component.onCompleted: {
        if (!root.sourceView.report) {
            root.refreshSource(false)
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Diagnostics / Delivery")
        title: qsTr("Delivery Diagnostics")
        layerLabel: qsTr("Diagnostics")
        subtitle: qsTr("%1 on %2, %3 s rolling window.")
            .arg(root.model.sourceRouting.deliverySourceLabel())
            .arg(root.model.messagingNetworkPreset)
            .arg(root.model.messagingRollingWindow)
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: root.sourceView.sourceBadges
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh source")
            primary: true
            enabled: !root.sourceView.pending
            Layout.preferredWidth: 162
            accessibleName: qsTr("Refresh Delivery source")
            onClicked: root.refreshSource(true)
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Open settings")
            enabled: !root.sourceView.pending
            Layout.preferredWidth: 126
            accessibleName: qsTr("Open Delivery settings")
            onClicked: root.model.openSettings("network", "messaging")
        }

        Text {
            text: root.sourceView.statusLine
            color: root.sourceView.statusColor
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.secondaryText
            font.weight: Font.Medium
            Layout.fillWidth: true
        }
    }

    TabSwitch {
        theme: root.theme
        current: root.currentTab
        options: deliveryTabs
        Layout.fillWidth: true
        onSelected: value => root.model.deliveryDiagnosticsTab = value
    }

    Loader {
        active: true
        asynchronous: true
        sourceComponent: root.tabComponent(root.currentTab)
        Layout.fillWidth: true
    }

    Component {
        id: overviewTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            GridLayout {
                columns: root.width < 520 ? 1 : (root.width < 798 ? 2 : 5)
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Health")
                    value: root.sourceView.healthText
                    delta: root.sourceView.freshnessText
                    deltaColor: root.sourceView.statusColor
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Source")
                    value: root.sourceView.sourceShortLabel
                    delta: root.sourceView.sourceTargetShort
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Peers")
                    value: root.sourceView.peerCount
                    delta: root.sourceView.identityEvidence
                    deltaColor: root.sourceView.peerColor
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Messages")
                    value: root.sourceView.messageCount
                    delta: qsTr("waku_node_messages_total")
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Errors")
                    value: root.sourceView.errorCount
                    delta: root.sourceView.errorDetail
                    deltaColor: root.sourceView.errorColor
                }
            }

            GridLayout {
                columns: root.width < 980 ? 1 : 2
                columnSpacing: root.theme.gap
                rowSpacing: root.theme.gap
                Layout.fillWidth: true

                StatusRowsPanel {
                    theme: root.theme
                    title: qsTr("Live degradation")
                    rows: root.sourceView.healthRows
                }

                StatusRowsPanel {
                    theme: root.theme
                    title: qsTr("Protocol readiness")
                    rows: root.sourceView.protocolRows
                }
            }
        }
    }

    Component {
        id: healthTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Diagnostic checklist")
                rows: root.sourceView.healthRows.concat(root.sourceView.evidenceRows)
            }
        }
    }

    Component {
        id: topologyTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            DetailRowsPanel {
                theme: root.theme
                title: qsTr("Local node identity")
                rows: root.sourceView.identityRows
            }

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Topology boundaries")
                rows: root.sourceView.topologyRows
            }
        }
    }

    Component {
        id: throughputTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Metrics and rolling-window activity")
                rows: root.sourceView.throughputRows
            }
        }
    }

    Component {
        id: protocolsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            ProtocolRowsPanel {
                theme: root.theme
                title: qsTr("Protocols")
                rows: root.sourceView.protocolRows
            }
        }
    }

    Component {
        id: topicsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Topics")
                rows: root.sourceView.topicRows
            }

            DetailRowsPanel {
                visible: root.sourceView.topicDetailRows.length > 0
                theme: root.theme
                title: qsTr("Observed content-topic activity")
                rows: root.sourceView.topicDetailRows
            }

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("Read-only observations")
                message: qsTr("Opening this screen never changes subscriptions. Current topic observations are source-reported; no search or filter action is performed.")
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: storeTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Store")
                rows: root.sourceView.storeRows
            }
        }
    }

    Component {
        id: diagnosticsTab

        DiagnosticsTab {
            theme: root.theme
            readTitle: qsTr("Read-only diagnostics")
            refreshActions: [
                { text: qsTr("Refresh status"), width: 140, accessibleName: qsTr("Refresh Delivery status") }
            ]
            pending: root.sourceView.pending
            statusText: root.diagnosticsStatusText("delivery", root.sourceView.statusLine, qsTr("Delivery diagnostics"))
            guardedTitle: qsTr("Delivery workflows")
            permissionEnabled: root.diagnosticsGateEnabled("delivery")
            permissionEnabledTitle: qsTr("Delivery tools available")
            permissionEnabledTone: "success"
            permissionDisabledTitle: qsTr("Diagnostics unavailable")
            guardedMessage: qsTr("Open the Delivery workspace for confirmed subscribe, unsubscribe, send, and Store-query workflows. Store support remains source-specific. Peer ping is unavailable because current Delivery adapters expose no peer-ping operation.")
            guardedActions: [
                {
                    action: "messages",
                    text: qsTr("Open message tools"),
                    accessibleName: qsTr("Open Delivery message tools"),
                    width: 174,
                    enabled: true
                },
                {
                    action: "store",
                    text: qsTr("Open Store tools"),
                    accessibleName: qsTr("Open Delivery Store tools"),
                    width: 158,
                    enabled: true
                }
            ]
            evidenceRows: root.sourceView.evidenceRows
            onRefreshRequested: root.refreshSource(true)
            onGuardedActionRequested: action => root.openDeliveryWorkflow(action)
        }
    }

    function tabComponent(tab) {
        switch (tab) {
        case "health":
            return healthTab
        case "topology":
            return topologyTab
        case "throughput":
            return throughputTab
        case "protocols":
            return protocolsTab
        case "topics":
            return topicsTab
        case "store":
            return storeTab
        case "diagnostics":
            return diagnosticsTab
        default:
            return overviewTab
        }
    }

    function refreshSource(showResult) {
        sourceSession.refresh(showResult)
    }

    function openDeliveryWorkflow(action) {
        const tab = String(action || "") === "store" ? "store" : "messages"
        root.model.pushNavigationHistory()
        root.model.deliveryAppTab = tab
        root.model.selectView("messaging", false)
    }

    function diagnosticsGate(action) {
        return root.model.diagnosticsGate(action)
    }

    function diagnosticsGateEnabled(action) {
        return diagnosticsGate(action).enabled === true
    }

    function diagnosticsStatusText(action, currentStatus, fallbackLabel) {
        const gate = diagnosticsGate(action)
        if (gate.enabled === true) {
            return currentStatus
        }
        return sourceSession.diagnosticsGateDetailText(gate, fallbackLabel)
    }
}
