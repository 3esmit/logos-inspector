pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../modules/controls"
import "../../../state"
import "../../../state/source_routing/SourceObservationProjection.js" as SourceObservation
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
        if (!root.report()) {
            root.refreshSource(false)
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Diagnostics / Storage")
        title: qsTr("Storage Diagnostics")
        layerLabel: qsTr("Diagnostics")
        subtitle: qsTr("%1 on %2, %3 s refresh window.")
            .arg(root.model.storageSourceLabel())
            .arg(root.model.storageNetworkPreset)
            .arg(root.model.storageRollingWindow)
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: root.sourceBadges()
        Layout.fillWidth: true
    }

    RowLayout {
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh source")
            primary: true
            enabled: !root.pending()
            Layout.preferredWidth: 162
            accessibleName: qsTr("Refresh Storage source")
            onClicked: root.refreshSource(true)
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Open settings")
            enabled: !root.pending()
            Layout.preferredWidth: 126
            accessibleName: qsTr("Open Storage settings")
            onClicked: root.model.openSettings("network", "storage")
        }

        Text {
            text: root.statusLine()
            color: root.statusColor()
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
                    value: root.healthText()
                    delta: root.freshnessText()
                    deltaColor: root.statusColor()
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Source")
                    value: root.sourceShortLabel()
                    delta: root.shortText(root.model.storageSourceTarget(), 34)
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Peers")
                    value: root.metricDisplay("storage.peer_count")
                    delta: root.identityEvidence()
                    deltaColor: root.metricTone("storage.peer_count")
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Capacity")
                    value: root.capacitySummary()
                    delta: root.valueSummary(root.probeValue("space"))
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Transfers")
                    value: root.transferSummary()
                    delta: qsTr("%1 failures in window").arg(root.metricDisplay("storage.failed_transfers_recent"))
                    deltaColor: root.transferFailureTone()
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Reliability")
                    value: root.reliabilityText()
                    delta: root.model.moduleLastError("storage") || root.sourceName()
                    deltaColor: root.reliabilityTone()
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
                    rows: root.healthRows()
                }

                StatusRowsPanel {
                    theme: root.theme
                    title: qsTr("Active operations")
                    rows: root.activeOperationRows()
                }

                StatusRowsPanel {
                    theme: root.theme
                    title: qsTr("Topology snapshot")
                    rows: root.topologyRows()
                }

                StatusRowsPanel {
                    theme: root.theme
                    title: qsTr("Capacity snapshot")
                    rows: root.capacityRows()
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
                rows: root.healthRows().concat(root.evidenceRows())
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
                rows: root.identityRows()
            }

            StatusRowsPanel {
                theme: root.theme
                title: qsTr("Peer boundaries")
                rows: root.topologyRows()
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
                rows: root.capacityRows().concat(root.repositoryRows())
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
                rows: root.transferRows()
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
                        enabled: root.model.storageCidProbe.length > 0 && !root.pending()
                        Layout.preferredWidth: 126
                        accessibleName: qsTr("Check local CID existence")
                        onClicked: root.refreshSource(true, true)
                    }
                }

                Repeater {
                    model: root.cidRows()

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
                rows: root.protocolRows()
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
            pending: root.pending() || !root.diagnosticsGateEnabled("storage")
            statusText: root.diagnosticsStatusText("storage", root.statusLine(), qsTr("Storage diagnostics"))
            guardedTitle: qsTr("Guarded diagnostics")
            permissionEnabled: root.model.storageMutatingDiagnosticsEnabled && root.diagnosticsGateEnabled("storage")
            permissionDisabledTitle: root.diagnosticsGateEnabled("storage") ? qsTr("Permission disabled") : qsTr("Diagnostics unavailable")
            guardedMessage: qsTr("Manifest fetch, provider lookup, download, connect, remove, upload, and lifecycle controls are not background-polled. They need backend adapters and per-action confirmation.")
            guardedActions: [
                { text: qsTr("Manifest fetch"), width: 142 },
                { text: qsTr("Provider lookup"), width: 148 },
                { text: qsTr("Download probe"), width: 142 }
            ]
            evidenceRows: root.evidenceRows()
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

    function pending() {
        return sourceSession.pending()
    }

    function report() {
        return sourceSession.report()
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
        return diagnosticsGateDetailText(gate, fallbackLabel)
    }

    function diagnosticsGateDetailText(gate, fallbackLabel) {
        return sourceSession.diagnosticsGateDetailText(gate, fallbackLabel)
    }

    function status() {
        return sourceSession.status()
    }

    function statusLine() {
        return sourceSession.statusLine()
    }

    function statusColor() {
        const status = root.status()
        if (!status.known) {
            return root.theme.textMuted
        }
        return status.ok ? root.theme.success : root.theme.error
    }

    function statusTone() {
        const status = root.status()
        if (!status.known) {
            return "neutral"
        }
        return status.ok ? "success" : "error"
    }

    function healthText() {
        if (root.pending()) {
            return qsTr("Working")
        }
        const status = root.status()
        if (!status.known) {
            return qsTr("Unknown")
        }
        if (!status.ok) {
            return qsTr("Problem")
        }
        return root.failedProbeCount() > 0 ? qsTr("Partial") : qsTr("Healthy")
    }

    function sourceName() {
        return sourceSession.sourceLabel()
    }

    function sourceShortLabel() {
        switch (root.storageSourceMode()) {
        case "rest":
            return qsTr("REST")
        case "metrics":
            return qsTr("Metrics")
        case "unsupported":
            return qsTr("Unsupported")
        default:
            return qsTr("Module")
        }
    }

    function freshnessText() {
        return sourceSession.freshnessText()
    }

    function freshnessCompactText() {
        return sourceSession.freshnessCompactText()
    }

    function sourceBadges() {
        return sourceSession.sourceBadges(root.model.storageNetworkPreset, qsTr("%1 s window").arg(root.model.storageRollingWindow))
    }

    function moduleInfoProbe() {
        return sourceSession.moduleInfoProbe()
    }

    function probeValue(method) {
        return sourceSession.probeValue(method)
    }

    function probe(method) {
        return sourceSession.probe(method)
    }

    function probeKnown(method) {
        return sourceSession.probeKnown(method)
    }

    function metricDisplay(key) {
        return sourceSession.metricDisplay(key)
    }

    function metricKnown(key) {
        return sourceSession.metricKnown(key)
    }

    function metricTone(key) {
        return sourceSession.metricTone(key)
    }

    function failedProbeCount() {
        return sourceSession.failedProbeCount()
    }

    function identityEvidence() {
        return SourceObservation.storageIdentityEvidence(root)
    }

    function sourceFactAvailable(key) {
        return sourceSession.sourceFactAvailable(key)
    }

    function sourceFactEvidence(key, fallback) {
        return sourceSession.sourceFactEvidence(key, fallback)
    }

    function capacitySummary() {
        return SourceObservation.storageCapacitySummary(root)
    }

    function transferSummary() {
        return SourceObservation.storageTransferSummary(root)
    }

    function reliabilityText() {
        return SourceObservation.storageReliabilityText(root)
    }

    function reliabilityTone() {
        return SourceObservation.storageReliabilityTone(root)
    }

    function transferFailureTone() {
        return SourceObservation.storageTransferFailureTone(root)
    }

    function healthRows() {
        return SourceObservation.storageHealthRows(root)
    }

    function activeOperationRows() {
        return SourceObservation.storageActiveOperationRows(root)
    }

    function topologyRows() {
        return SourceObservation.storageTopologyRows(root)
    }

    function capacityRows() {
        return SourceObservation.storageCapacityRows(root)
    }

    function repositoryRows() {
        return SourceObservation.storageRepositoryRows(root)
    }

    function transferRows() {
        return SourceObservation.storageTransferRows(root)
    }

    function activeDownloadRow() {
        return SourceObservation.storageActiveDownloadRow(root)
    }

    function activeStorageOperation() {
        const revision = root.model.storageActiveOperationRevision
        return root.model.storageActiveOperation || null
    }

    function activeStorageOperationDetail(operation) {
        return SourceObservation.storageActiveStorageOperationDetail(root, operation)
    }

    function cidRows() {
        return SourceObservation.storageCidRows(root)
    }

    function protocolRows() {
        return SourceObservation.storageProtocolRows(root)
    }

    function identityRows() {
        return SourceObservation.storageIdentityRows(root)
    }

    function evidenceRows() {
        return sourceSession.evidenceRows(root, qsTr("Refresh source to load probe evidence."))
    }

    function statusRow(label, state, evidence, tone) {
        return sourceSession.statusRow(label, state, evidence, tone)
    }

    function metricRow(label, key) {
        return SourceObservation.storageMetricRow(root, label, key)
    }

    function metricEvidence(key) {
        return SourceObservation.storageMetricEvidence(root, key)
    }

    function manifestCountRow() {
        return SourceObservation.storageManifestCountRow(root)
    }

    function spaceRow(label, keys) {
        return SourceObservation.storageSpaceRow(root, label, keys)
    }

    function protocolRow(label, protocolId, observed, evidence) {
        return SourceObservation.storageProtocolRow(label, protocolId, observed, evidence)
    }

    function probeRow(probe, fallbackLabel) {
        return sourceSession.probeRow(probe, fallbackLabel)
    }

    function detailRow(label, value) {
        return sourceSession.detailRow(label, value, [qsTr("Not queried"), qsTr("Not fetched"), qsTr("Idle")])
    }

    function pathDetailRow(label, value) {
        return SourceObservation.storagePathDetailRow(root, label, value)
    }

    function restMetricsState() {
        return SourceObservation.storageRestMetricsState(root)
    }

    function restMetricsEvidence() {
        return SourceObservation.storageRestMetricsEvidence(root)
    }

    function restMetricsTone() {
        return SourceObservation.storageRestMetricsTone(root)
    }

    function storageSourceMode() {
        return root.model.effectiveStorageSourceMode(root.model.storageSourceMode)
    }

    function metricsEndpointConfigured() {
        return String(root.model.storageMetricsUrl || "").trim().length > 0
    }

    function valueSummary(value) {
        return sourceSession.valueSummary(value)
    }

    function copyValue(value) {
        return sourceSession.copyValue(value)
    }

    function objectField(value, keys) {
        return SourceObservation.objectField(value, keys)
    }

    function shortText(value, maxLength) {
        return sourceSession.shortText(value, maxLength)
    }

}
