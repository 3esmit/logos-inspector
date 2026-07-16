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
        family: "storage"
    }
    readonly property var sourceView: sourceSession.view
    property string currentTab: "overview"

    width: parent ? parent.width : 900
    spacing: root.theme.gapLarge

    ListModel {
        id: storageTabs

        ListElement { value: "overview"; label: "Overview" }
        ListElement { value: "health"; label: "Health" }
        ListElement { value: "topology"; label: "Topology" }
        ListElement { value: "capacity"; label: "Capacity" }
        ListElement { value: "transfers"; label: "Transfers" }
        ListElement { value: "cids"; label: "CIDs" }
        ListElement { value: "protocols"; label: "Protocols" }
        ListElement { value: "diagnostics"; label: "Diagnostics" }
    }

    Component.onCompleted: {
        if (!root.sourceView.report) {
            root.refreshSource(false)
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Diagnostics / Storage")
        title: qsTr("Storage Diagnostics")
        layerLabel: qsTr("Diagnostics")
        subtitle: qsTr("%1 on %2, %3 s refresh window.")
            .arg(root.model.sourceRouting.storageSourceLabel())
            .arg(root.model.storageNetworkPreset)
            .arg(root.model.storageRollingWindow)
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
            accessibleName: qsTr("Refresh Storage source")
            onClicked: root.refreshSource(true)
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Open settings")
            enabled: !root.sourceView.pending
            Layout.preferredWidth: 126
            accessibleName: qsTr("Open Storage settings")
            onClicked: root.model.openSettings("network", "storage")
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
        options: storageTabs
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
                columns: root.width < 760 ? 2 : 6
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
                    label: qsTr("Capacity")
                    value: root.sourceView.capacitySummary
                    delta: root.sourceView.capacityEvidence
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Transfers")
                    value: root.sourceView.transferSummary
                    delta: qsTr("%1 failures in window").arg(root.sourceView.transferFailures)
                    deltaColor: root.sourceView.transferFailureColor
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Reliability")
                    value: root.sourceView.reliabilityText
                    delta: root.sourceView.reliabilityDetail
                    deltaColor: root.sourceView.reliabilityColor
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
                    title: qsTr("Active operations")
                    rows: root.sourceView.activeOperationRows
                }

                StatusRowsPanel {
                    theme: root.theme
                    title: qsTr("Topology snapshot")
                    rows: root.sourceView.topologyRows
                }

                StatusRowsPanel {
                    theme: root.theme
                    title: qsTr("Capacity snapshot")
                    rows: root.sourceView.capacityRows
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
                title: qsTr("Peer boundaries")
                rows: root.sourceView.topologyRows
            }
        }
    }

    Component {
        id: capacityTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Space and repository")
                rows: root.sourceView.capacityRows.concat(root.sourceView.repositoryRows)
            }
        }
    }

    Component {
        id: transfersTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Transfer counters")
                rows: root.sourceView.transferRows
            }
        }
    }

    Component {
        id: cidsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("CID lookup")

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    FieldRow {
                        theme: root.theme
                        label: qsTr("CID")
                        sourceText: root.model.storageCidProbe
                        syncSourceText: true
                        placeholderText: qsTr("Storage CID")
                        Layout.fillWidth: true
                        onTextEdited: text => root.model.storageCidProbe = String(text || "").trim()
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Local exists")
                        primary: true
                        enabled: root.model.storageCidProbe.length > 0 && !root.sourceView.pending
                        Layout.preferredWidth: 126
                        accessibleName: qsTr("Check local CID existence")
                        onClicked: root.refreshSource(true, true)
                    }
                }

                Repeater {
                    model: root.sourceView.cidRows

                    DetailRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        value: String(modelData.value || "")
                        copyText: String(modelData.copyText || "")
                        source: String(modelData.source || "")
                    }
                }
            }

            StatusMessage {
                theme: root.theme
                tone: "info"
                title: qsTr("Network diagnostics are explicit")
                message: qsTr("CID parsing and local exists checks are passive. Manifest fetch, provider lookup, and download probes stay idle until an explicit diagnostic action exists.")
                Layout.fillWidth: true
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
                labelWidth: 150
            }
        }
    }

    Component {
        id: diagnosticsTab

        DiagnosticsTab {
            theme: root.theme
            readTitle: qsTr("Read-only diagnostics")
            refreshActions: [
                { text: qsTr("Refresh status"), width: 140, accessibleName: qsTr("Refresh Storage status") }
            ]
            pending: root.sourceView.pending
            statusText: root.diagnosticsStatusText("storage", root.sourceView.statusLine, qsTr("Storage diagnostics"))
            guardedTitle: qsTr("Confirmed actions")
            permissionEnabled: root.diagnosticsGateEnabled("storage")
            permissionDisabledTitle: qsTr("Diagnostics unavailable")
            guardedMessage: qsTr("Manifest fetch, provider lookup, download, connect, remove, upload, and lifecycle controls are not background-polled. They need backend adapters and per-action confirmation.")
            guardedActions: [
                { text: qsTr("Manifest fetch"), width: 142 },
                { text: qsTr("Provider lookup"), width: 148 },
                { text: qsTr("Download probe"), width: 142 }
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
        case "capacity":
            return capacityTab
        case "transfers":
            return transfersTab
        case "cids":
            return cidsTab
        case "protocols":
            return protocolsTab
        case "diagnostics":
            return diagnosticsTab
        default:
            return overviewTab
        }
    }

    function refreshSource(showResult, includeCidProbe) {
        sourceSession.refresh(showResult, includeCidProbe)
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
