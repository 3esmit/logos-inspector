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
        family: "delivery"
    }
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
        if (!root.report()) {
            root.refreshSource(false)
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Diagnostics / Delivery")
        title: qsTr("Delivery Diagnostics")
        layerLabel: qsTr("Diagnostics")
        subtitle: qsTr("%1 on %2, %3 s rolling window.")
            .arg(root.model.deliverySourceLabel())
            .arg(root.model.messagingNetworkPreset)
            .arg(root.model.messagingRollingWindow)
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
            accessibleName: qsTr("Refresh Delivery source")
            onClicked: root.refreshSource(true)
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Open settings")
            enabled: !root.pending()
            Layout.preferredWidth: 126
            accessibleName: qsTr("Open Delivery settings")
            onClicked: root.model.openSettings("network", "messaging")
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
                    value: root.healthText()
                    delta: root.freshnessText()
                    deltaColor: root.statusColor()
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Source")
                    value: root.sourceShortLabel()
                    delta: root.shortText(root.model.deliverySourceTarget(), 34)
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Peers")
                    value: root.metricDisplay("messaging.peer_count")
                    delta: root.identityEvidence()
                    deltaColor: root.metricTone("messaging.peer_count")
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Messages")
                    value: root.metricDisplay("messaging.message_received_events_recent")
                    delta: qsTr("waku_node_messages_total")
                }

                MetricCard {
                    theme: root.theme
                    compact: true
                    label: qsTr("Errors")
                    value: root.metricDisplay("messaging.message_error_events_recent")
                    delta: root.model.moduleLastError("messaging") || root.sourceName()
                    deltaColor: root.metricTone("messaging.message_error_events_recent")
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
                    title: qsTr("Protocol readiness")
                    rows: root.protocolRows()
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
                title: qsTr("Topology boundaries")
                rows: root.topologyRows()
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
                rows: root.throughputRows()
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
                rows: root.topicRows()
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
                rows: root.storeRows()
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
            pending: root.pending() || !root.diagnosticsGateEnabled("delivery")
            statusText: root.diagnosticsStatusText("delivery", root.statusLine(), qsTr("Delivery diagnostics"))
            guardedTitle: qsTr("Mutating diagnostics")
            permissionEnabled: root.model.messagingMutatingDiagnosticsEnabled && root.diagnosticsGateEnabled("delivery")
            permissionDisabledTitle: root.diagnosticsGateEnabled("delivery") ? qsTr("Permission disabled") : qsTr("Diagnostics unavailable")
            guardedMessage: qsTr("Dial, publish, subscribe, and lightpush probes are not auto-run. They need backend adapters and per-action confirmation.")
            guardedActions: [
                { text: qsTr("Ping peer"), width: 118 },
                { text: qsTr("Store query"), width: 122 },
                { text: qsTr("Lightpush test"), width: 136 }
            ]
            evidenceRows: root.probeRows()
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
        switch (root.deliverySourceMode()) {
        case "rest":
            return qsTr("REST")
        case "metrics":
            return qsTr("Metrics")
        case "network-monitor":
            return qsTr("Monitor")
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
        return sourceSession.sourceBadges(root.model.normalizedMessagingNetworkPreset(root.model.messagingNetworkPreset), qsTr("%1 s window").arg(root.model.messagingRollingWindow))
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

    function probeOk(method) {
        const probe = root.probe(method)
        return probe ? probe.ok === true : false
    }

    function metricDisplay(key) {
        return sourceSession.metricDisplay(key)
    }

    function metricTone(key) {
        return sourceSession.metricTone(key)
    }

    function failedProbeCount() {
        return sourceSession.failedProbeCount()
    }

    function identityEvidence() {
        return SourceObservation.deliveryIdentityEvidence(root)
    }

    function sourceFactAvailable(key) {
        return sourceSession.sourceFactAvailable(key)
    }

    function sourceFactEvidence(key, fallback) {
        return sourceSession.sourceFactEvidence(key, fallback)
    }

    function sourceFactObservedState(key, fallbackKnown) {
        return SourceObservation.deliverySourceFactObservedState(root, key, fallbackKnown)
    }

    function sourceFactObservedTone(key, fallbackKnown) {
        return SourceObservation.deliverySourceFactObservedTone(root, key, fallbackKnown)
    }

    function healthRows() {
        return SourceObservation.deliveryHealthRows(root)
    }

    function evidenceRows() {
        return sourceSession.evidenceRows(root, qsTr("Refresh source to load probe evidence."))
    }

    function topologyRows() {
        return SourceObservation.deliveryTopologyRows(root)
    }

    function throughputRows() {
        return SourceObservation.deliveryThroughputRows(root)
    }

    function protocolRows() {
        return SourceObservation.deliveryProtocolRows(root)
    }

    function protocolHealthRows() {
        return SourceObservation.deliveryProtocolHealthRows(root)
    }

    function protocolHealthEntry(item) {
        return SourceObservation.deliveryProtocolHealthEntry(root, item)
    }

    function protocolHealthDetail(protocol, description) {
        return SourceObservation.deliveryProtocolHealthDetail(root, protocol, description)
    }

    function protocolLabel(key) {
        return SourceObservation.deliveryProtocolLabel(key)
    }

    function healthValueTone(value) {
        return SourceObservation.deliveryHealthValueTone(root, value)
    }

    function combinedHealthTone(left, right) {
        return SourceObservation.deliveryCombinedHealthTone(root, left, right)
    }

    function topicRows() {
        return SourceObservation.deliveryTopicRows(root)
    }

    function storeRows() {
        return SourceObservation.deliveryStoreRows(root)
    }

    function identityRows() {
        return SourceObservation.deliveryIdentityRows(root)
    }

    function identityValue(kind) {
        return SourceObservation.deliveryIdentityValue(root, kind)
    }

    function probeRows() {
        return root.evidenceRows()
    }

    function statusRow(label, state, evidence, tone) {
        return sourceSession.statusRow(label, state, evidence, tone)
    }

    function metricRow(label, key) {
        return SourceObservation.deliveryMetricRow(root, label, key)
    }

    function metricEvidence(key) {
        return SourceObservation.deliveryMetricEvidence(root, key)
    }

    function protocolStatusRow(label, protocol, metricKey) {
        return SourceObservation.deliveryProtocolStatusRow(root, label, protocol, metricKey)
    }

    function protocolRow(label, protocolId, signalKey) {
        return SourceObservation.deliveryProtocolRow(root, label, protocolId, signalKey)
    }

    function probeRow(probe, fallbackLabel) {
        return sourceSession.probeRow(probe, fallbackLabel)
    }

    function detailRow(label, value) {
        return sourceSession.detailRow(label, value)
    }

    function metricKnown(key) {
        return sourceSession.metricKnown(key)
    }

    function restMetricsState() {
        return SourceObservation.deliveryRestMetricsState(root)
    }

    function restMetricsEvidence() {
        return SourceObservation.deliveryRestMetricsEvidence(root)
    }

    function moduleMetricsText() {
        return SourceObservation.deliveryModuleMetricsText(root)
    }

    function restMetricsTone() {
        return SourceObservation.deliveryRestMetricsTone(root)
    }

    function deliverySourceMode() {
        return root.model.effectiveMessagingSourceMode(root.model.messagingSourceMode)
    }

    function networkMonitorPeerCount() {
        return SourceObservation.deliveryNetworkMonitorPeerCount(root)
    }

    function networkMonitorTopicCount() {
        return SourceObservation.deliveryNetworkMonitorTopicCount(root)
    }

    function servicePeerCount() {
        return SourceObservation.deliveryServicePeerCount(root)
    }

    function countValue(value) {
        return SourceObservation.deliveryCountValue(root, value)
    }

    function valueSummary(value) {
        return sourceSession.valueSummary(value)
    }

    function copyValue(value) {
        return sourceSession.copyValue(value)
    }

    function shortText(value, maxLength) {
        return sourceSession.shortText(value, maxLength)
    }

}
