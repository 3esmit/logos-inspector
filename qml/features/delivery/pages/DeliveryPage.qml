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
    property string currentTab: "overview"

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
        onSelected: value => root.currentTab = value
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
                title: qsTr("Rolling-window rates")
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

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("Passive only")
                message: qsTr("Search and filters on this screen never subscribe to topics in the background.")
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
            pending: root.sourceView.pending || !root.diagnosticsGateEnabled("delivery")
            statusText: root.diagnosticsStatusText("delivery", root.sourceView.statusLine, qsTr("Delivery diagnostics"))
            guardedTitle: qsTr("Mutating diagnostics")
            permissionEnabled: root.model.messagingMutatingDiagnosticsEnabled && root.diagnosticsGateEnabled("delivery")
            permissionDisabledTitle: root.diagnosticsGateEnabled("delivery") ? qsTr("Permission disabled") : qsTr("Diagnostics unavailable")
            guardedMessage: qsTr("Dial, publish, subscribe, and lightpush probes are not auto-run. They need backend adapters and per-action confirmation.")
            guardedActions: [
                { text: qsTr("Ping peer"), width: 118 },
                { text: qsTr("Store query"), width: 122 },
                { text: qsTr("Lightpush test"), width: 136 }
            ]
            evidenceRows: root.sourceView.evidenceRows
            onRefreshRequested: root.refreshSource(true)
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
