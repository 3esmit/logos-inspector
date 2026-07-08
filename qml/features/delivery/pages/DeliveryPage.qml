pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../../../components"
import "../../modules/controls"
import "../../../state"
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
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
            pending: root.pending()
            statusText: root.statusLine()
            guardedTitle: qsTr("Mutating diagnostics")
            permissionEnabled: root.model.messagingMutatingDiagnosticsEnabled
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
        root.model.queryNetworkConnection("messaging", showResult === true)
    }

    function pending() {
        return root.model.networkConnectionIsPending("messaging")
    }

    function report() {
        return root.model.moduleReport("messaging")
    }

    function status() {
        return root.model.networkConnectionState("messaging")
    }

    function statusLine() {
        if (root.pending()) {
            return qsTr("Refreshing %1").arg(root.model.deliverySourceLabel())
        }
        const status = root.status()
        if (!status.known) {
            return qsTr("Not queried")
        }
        return qsTr("%1, checked %2").arg(status.detail || status.text).arg(status.checkedAt || qsTr("now"))
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
        return root.model.deliverySourceLabel()
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
        const status = root.status()
        if (!status.known) {
            return qsTr("No source check")
        }
        return status.checkedAt && status.checkedAt.length ? qsTr("Updated %1").arg(status.checkedAt) : qsTr("Updated")
    }

    function freshnessCompactText() {
        const status = root.status()
        if (!status.known) {
            return qsTr("not queried")
        }
        return status.checkedAt && status.checkedAt.length ? status.checkedAt : qsTr("updated")
    }

    function sourceBadges() {
        const rows = [
            root.model.deliverySourceLabel(),
            root.shortText(root.model.deliverySourceTarget(), 42),
            root.model.normalizedMessagingNetworkPreset(root.model.messagingNetworkPreset),
            qsTr("%1 s window").arg(root.model.messagingRollingWindow)
        ]
        const status = root.status()
        rows.push(status.known ? root.freshnessText() : qsTr("not queried"))
        rows.push(status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"))
        return rows
    }

    function moduleInfoProbe() {
        const report = root.report()
        return report && report.module_info ? report.module_info : null
    }

    function probeValue(method) {
        return root.model.moduleProbeValue("messaging", method)
    }

    function probe(method) {
        return root.model.sourceProbeFact(root.report(), method) || root.model.moduleProbe("messaging", method)
    }

    function probeOk(method) {
        const probe = root.probe(method)
        return probe ? probe.ok === true : false
    }

    function metricDisplay(key) {
        const value = root.model.dashboardMetricValue(key)
        return value === null || value === undefined ? qsTr("n/a") : root.model.valueText(value)
    }

    function metricTone(key) {
        const value = Number(root.model.dashboardMetricValue(key))
        if (!Number.isFinite(value)) {
            return root.theme.textMuted
        }
        if (key.indexOf("error") >= 0 || key.indexOf("failed") >= 0) {
            return value > 0 ? root.theme.error : root.theme.success
        }
        return value > 0 ? root.theme.success : root.theme.textMuted
    }

    function failedProbeCount() {
        let failed = 0
        const report = root.report()
        if (!report) {
            return failed
        }
        if (report.module_info && report.module_info.ok === false) {
            failed += 1
        }
        const probes = Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            if (probes[i] && probes[i].ok === false) {
                failed += 1
            }
        }
        return failed
    }

    function identityEvidence() {
        const factEvidence = root.sourceFactEvidence("identity", "")
        if (factEvidence.length > 0 && factEvidence !== "not observed") {
            return factEvidence
        }
        const peerId = root.identityValue("peerId")
        if (peerId !== null) {
            return qsTr("peer id present")
        }
        const addresses = root.identityValue("listenAddresses")
        if (addresses !== null) {
            return qsTr("addresses present")
        }
        return root.sourceName()
    }

    function sourceFactAvailable(key) {
        return root.model.sourceCapabilityAvailable(root.report(), key) === true
    }

    function sourceFactEvidence(key, fallback) {
        const evidence = root.model.sourceCapabilityEvidence(root.report(), key)
        return evidence.length > 0 ? evidence : fallback
    }

    function sourceFactObservedState(key, fallbackKnown) {
        return root.sourceFactAvailable(key) || fallbackKnown ? qsTr("observed") : qsTr("unknown")
    }

    function sourceFactObservedTone(key, fallbackKnown) {
        return root.sourceFactAvailable(key) || fallbackKnown ? "success" : "neutral"
    }

    function healthRows() {
        const status = root.status()
        const identity = root.identityValue("peerId") || root.identityValue("enrUri") || root.identityValue("listenAddresses")
        const nodeHealth = root.probeValue("nodeHealth")
        const connectionStatus = root.probeValue("connectionStatus")
        const nodeTone = root.combinedHealthTone(nodeHealth, connectionStatus)
        const relayKnown = root.metricKnown("messaging.pubsub_peers")
        const storeKnown = root.metricKnown("messaging.store_peers")
        const filterKnown = root.metricKnown("messaging.filter_peers")
        const lightpushKnown = root.metricKnown("messaging.lightpush_peers")
        const discovered = root.networkMonitorPeerCount()
        const discoveryKnown = discovered !== null
        return [
            root.statusRow(qsTr("Source and lifecycle"), status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"), status.detail || qsTr("Not queried"), root.statusTone()),
            root.statusRow(qsTr("Identity"), root.sourceFactAvailable("identity") || identity !== null ? qsTr("present") : qsTr("unknown"), root.identityEvidence(), root.sourceFactAvailable("identity") || identity !== null ? "success" : "neutral"),
            root.statusRow(qsTr("Node health"), nodeHealth !== null ? root.valueSummary(nodeHealth) : qsTr("unknown"), root.valueSummary(connectionStatus), nodeTone),
            root.statusRow(qsTr("Preset, cluster, shards"), root.model.messagingNetworkPreset.length ? qsTr("configured") : qsTr("unknown"), root.model.normalizedMessagingNetworkPreset(root.model.messagingNetworkPreset) || qsTr("No preset"), root.model.messagingNetworkPreset.length ? "success" : "neutral"),
            root.statusRow(qsTr("REST and metrics access"), root.restMetricsState(), root.restMetricsEvidence(), root.restMetricsTone()),
            root.statusRow(qsTr("Relay"), root.sourceFactObservedState("relay", relayKnown), relayKnown ? root.metricDisplay("messaging.pubsub_peers") : root.sourceFactEvidence("relay", qsTr("No relay fact.")), root.sourceFactObservedTone("relay", relayKnown)),
            root.statusRow(qsTr("Store"), root.sourceFactObservedState("store", storeKnown), storeKnown ? root.metricDisplay("messaging.store_peers") : root.sourceFactEvidence("store", qsTr("No Store fact.")), root.sourceFactObservedTone("store", storeKnown)),
            root.statusRow(qsTr("Filter"), root.sourceFactObservedState("filter", filterKnown), filterKnown ? root.metricDisplay("messaging.filter_peers") : root.sourceFactEvidence("filter", qsTr("No Filter fact.")), root.sourceFactObservedTone("filter", filterKnown)),
            root.statusRow(qsTr("Lightpush"), root.sourceFactObservedState("lightpush", lightpushKnown), lightpushKnown ? root.metricDisplay("messaging.lightpush_peers") : root.sourceFactEvidence("lightpush", qsTr("No Lightpush fact.")), root.sourceFactObservedTone("lightpush", lightpushKnown)),
            root.statusRow(qsTr("Discovery"), root.sourceFactObservedState("network_monitor", discoveryKnown), discoveryKnown ? qsTr("%1 peer(s)").arg(discovered) : root.sourceFactEvidence("network_monitor", qsTr("No network monitor peer snapshot.")), root.sourceFactObservedTone("network_monitor", discoveryKnown)),
            root.statusRow(qsTr("RLN / spam protection"), qsTr("unknown"), qsTr("No passive metric selected"), "neutral")
        ]
    }

    function evidenceRows() {
        const rows = []
        const info = root.moduleInfoProbe()
        if (info) {
            rows.push(root.probeRow(info, qsTr("Source check")))
        }
        const report = root.report()
        const probes = report && Array.isArray(report.probes) ? report.probes : []
        for (let i = 0; i < probes.length; ++i) {
            rows.push(root.probeRow(probes[i], qsTr("Probe")))
        }
        if (rows.length === 0) {
            rows.push(root.statusRow(qsTr("Probe evidence"), qsTr("empty"), qsTr("Refresh source to load probe evidence."), "neutral"))
        }
        return rows
    }

    function topologyRows() {
        const discovered = root.networkMonitorPeerCount()
        const topics = root.networkMonitorTopicCount()
        const servicePeers = root.servicePeerCount()
        return [
            root.statusRow(qsTr("Local connected peers"), root.metricKnown("messaging.peer_count") ? qsTr("observed") : qsTr("unknown"), root.metricDisplay("messaging.peer_count"), root.metricKnown("messaging.peer_count") ? "success" : "neutral"),
            root.statusRow(qsTr("Relay mesh peers"), root.metricKnown("messaging.pubsub_peers") ? qsTr("observed") : qsTr("unknown"), root.metricDisplay("messaging.pubsub_peers"), root.metricKnown("messaging.pubsub_peers") ? "success" : "neutral"),
            root.statusRow(qsTr("Discovery peers"), discovered !== null ? qsTr("observed") : qsTr("unknown"), discovered !== null ? qsTr("%1 peer(s)").arg(discovered) : qsTr("No network monitor peer snapshot."), discovered !== null ? "success" : "neutral"),
            root.statusRow(qsTr("Service peers"), servicePeers !== null ? qsTr("observed") : qsTr("unknown"), servicePeers !== null ? qsTr("%1 service peer(s)").arg(servicePeers) : qsTr("No Store/Filter/Lightpush peer metrics."), servicePeers !== null ? "success" : "neutral"),
            root.statusRow(qsTr("Content topics"), topics !== null ? qsTr("observed") : qsTr("unknown"), topics !== null ? qsTr("%1 topic(s)").arg(topics) : qsTr("No network monitor topic snapshot."), topics !== null ? "success" : "neutral")
        ]
    }

    function throughputRows() {
        return [
            root.metricRow(qsTr("Peer count"), "messaging.peer_count"),
            root.metricRow(qsTr("Pubsub peers"), "messaging.pubsub_peers"),
            root.metricRow(qsTr("Network ingress"), "messaging.network_ingress_recent"),
            root.metricRow(qsTr("Network egress"), "messaging.network_egress_recent"),
            root.metricRow(qsTr("Relay ingress"), "messaging.relay_ingress_recent"),
            root.metricRow(qsTr("Relay egress"), "messaging.relay_egress_recent"),
            root.metricRow(qsTr("Service ingress"), "messaging.service_ingress_recent"),
            root.metricRow(qsTr("Service egress"), "messaging.service_egress_recent"),
            root.metricRow(qsTr("Sent events"), "messaging.message_sent_events_recent"),
            root.metricRow(qsTr("Propagated events"), "messaging.message_propagated_events_recent"),
            root.metricRow(qsTr("Messages in window"), "messaging.message_received_events_recent"),
            root.metricRow(qsTr("Errors in window"), "messaging.message_error_events_recent"),
            root.metricRow(qsTr("Store peers"), "messaging.store_peers"),
            root.metricRow(qsTr("Filter peers"), "messaging.filter_peers"),
            root.metricRow(qsTr("Lightpush peers"), "messaging.lightpush_peers")
        ]
    }

    function protocolRows() {
        const healthRows = root.protocolHealthRows()
        if (healthRows.length > 0) {
            return healthRows
        }
        return [
            root.protocolRow(qsTr("Relay"), "/vac/waku/relay/2.0.0", "messaging.pubsub_peers"),
            root.protocolRow(qsTr("Store"), "/vac/waku/store/3.0.0", "messaging.store_peers"),
            root.protocolRow(qsTr("Filter"), "/vac/waku/filter/2.0.0-beta1", "messaging.filter_peers"),
            root.protocolRow(qsTr("Lightpush"), "/vac/waku/lightpush/2.0.0-beta1", "messaging.lightpush_peers"),
            root.protocolRow(qsTr("Peer exchange"), "/vac/waku/peer-exchange/2.0.0-alpha1", ""),
            root.protocolRow(qsTr("Metadata"), "/vac/waku/metadata/1.0.0", "Version"),
            root.protocolRow(qsTr("Discv5"), "discv5", ""),
            root.protocolRow(qsTr("RLN relay"), "/vac/waku/rln-relay/2.0.0", "")
        ]
    }

    function protocolHealthRows() {
        const value = root.probeValue("protocolsHealth")
        if (!value || typeof value !== "object") {
            return []
        }
        const rows = []
        if (Array.isArray(value)) {
            for (let i = 0; i < value.length; ++i) {
                const item = root.protocolHealthEntry(value[i])
                if (item) {
                    rows.push(root.statusRow(root.protocolLabel(item.protocol), root.valueSummary(item.health), item.detail, root.healthValueTone(item.health)))
                }
            }
            return rows
        }
        if (value.protocol !== undefined || value.name !== undefined || value.health !== undefined || value.status !== undefined) {
            const single = root.protocolHealthEntry(value)
            rows.push(root.statusRow(root.protocolLabel(single.protocol), root.valueSummary(single.health), single.detail, root.healthValueTone(single.health)))
            return rows
        }
        const keys = Object.keys(value).sort()
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            if (key === "desc" || key === "description") {
                continue
            }
            const state = value[key]
            rows.push(root.statusRow(root.protocolLabel(key), root.valueSummary(state), key, root.healthValueTone(state)))
        }
        return rows
    }

    function protocolHealthEntry(item) {
        if (!item || typeof item !== "object" || Array.isArray(item)) {
            return null
        }
        const explicitProtocol = item.protocol !== undefined ? item.protocol : item.name
        const explicitHealth = item.health !== undefined ? item.health : item.status
        if (explicitProtocol !== undefined || explicitHealth !== undefined) {
            const protocol = explicitProtocol !== undefined ? explicitProtocol : qsTr("Protocol")
            return {
                protocol: protocol,
                health: explicitHealth,
                detail: root.protocolHealthDetail(protocol, item.desc !== undefined ? item.desc : item.description)
            }
        }
        const keys = Object.keys(item).filter(key => key !== "desc" && key !== "description").sort()
        if (!keys.length) {
            return null
        }
        const protocolKey = keys[0]
        return {
            protocol: protocolKey,
            health: item[protocolKey],
            detail: root.protocolHealthDetail(protocolKey, item.desc !== undefined ? item.desc : item.description)
        }
    }

    function protocolHealthDetail(protocol, description) {
        const detail = root.valueSummary(description)
        if (!detail.length || detail === qsTr("unknown")) {
            return String(protocol || "")
        }
        return "%1: %2".arg(String(protocol || "")).arg(detail)
    }

    function protocolLabel(key) {
        const text = String(key || "")
        const normalized = text.toLowerCase()
        if (normalized.indexOf("lightpush") >= 0) {
            return qsTr("Lightpush")
        }
        if (normalized.indexOf("filter") >= 0) {
            return qsTr("Filter")
        }
        if (normalized.indexOf("store") >= 0) {
            return qsTr("Store")
        }
        if (normalized.indexOf("relay") >= 0) {
            return qsTr("Relay")
        }
        if (normalized.indexOf("metadata") >= 0) {
            return qsTr("Metadata")
        }
        if (normalized.indexOf("peer") >= 0) {
            return qsTr("Peer exchange")
        }
        return text.length ? text : qsTr("Protocol")
    }

    function healthValueTone(value) {
        if (value === undefined || value === null) {
            return "neutral"
        }
        return root.model.deliveryHealthValueOk(value, false) ? "success" : "error"
    }

    function combinedHealthTone(left, right) {
        const leftTone = root.healthValueTone(left)
        const rightTone = root.healthValueTone(right)
        if (leftTone === "error" || rightTone === "error") {
            return "error"
        }
        if (leftTone === "success" || rightTone === "success") {
            return "success"
        }
        return "neutral"
    }

    function topicRows() {
        const topics = root.networkMonitorTopicCount()
        return [
            root.metricRow(qsTr("Pubsub peers"), "messaging.pubsub_peers"),
            root.metricRow(qsTr("Content topics"), "messaging.content_topics"),
            root.statusRow(qsTr("Topic-to-shard mapping"), topics !== null ? qsTr("observed") : qsTr("unknown"), topics !== null ? qsTr("%1 content topic(s)").arg(topics) : qsTr("Requires topic metadata or network monitor source."), topics !== null ? "success" : "neutral"),
            root.metricRow(qsTr("Store query pressure"), "messaging.store_query_requests_recent"),
            root.metricRow(qsTr("Filter query pressure"), "messaging.filter_requests_recent")
        ]
    }

    function storeRows() {
        return [
            root.protocolStatusRow(qsTr("Store mounted state"), "Store", "messaging.store_peers"),
            root.metricRow(qsTr("Store peers"), "messaging.store_peers"),
            root.metricRow(qsTr("Stored messages"), "messaging.store_messages"),
            root.metricRow(qsTr("Store query rate"), "messaging.store_query_requests_recent"),
            root.metricRow(qsTr("Store errors"), "messaging.store_errors_recent"),
            root.statusRow(qsTr("Manual query"), qsTr("available"), qsTr("Network / Delivery Store tab uses includeData=false by default."), "success"),
            root.statusRow(qsTr("Payload viewing"), qsTr("disabled"), qsTr("Payload bytes stay hidden unless a future query opts in."), "success")
        ]
    }

    function identityRows() {
        return [
            root.detailRow(qsTr("Peer ID"), root.identityValue("peerId")),
            root.detailRow(qsTr("ENR"), root.identityValue("enrUri")),
            root.detailRow(qsTr("Multiaddresses"), root.identityValue("listenAddresses")),
            root.detailRow(qsTr("Protocol health"), root.probeValue("protocolsHealth")),
            root.detailRow(qsTr("Version"), root.probeValue("Version") || root.probeValue("version"))
        ]
    }

    function identityValue(kind) {
        switch (kind) {
        case "peerId":
            return root.probeValue("peerId") || root.probeValue("MyPeerId")
        case "enrUri":
            return root.probeValue("enrUri") || root.probeValue("MyENR")
        case "listenAddresses":
            return root.probeValue("listenAddresses") || root.probeValue("MyMultiaddresses")
        default:
            return null
        }
    }

    function probeRows() {
        return root.evidenceRows()
    }

    function statusRow(label, state, evidence, tone) {
        return {
            label: label,
            state: state,
            evidence: evidence,
            source: root.sourceName(),
            freshness: root.freshnessCompactText(),
            tone: tone
        }
    }

    function metricRow(label, key) {
        const known = root.metricKnown(key)
        return root.statusRow(label, known ? root.metricDisplay(key) : qsTr("n/a"), known ? root.metricEvidence(key) : qsTr("Metric not exposed by current source."), known ? "success" : "neutral")
    }

    function metricEvidence(key) {
        return root.model.dashboardMetricUsesWindow(key) ? qsTr("%1 s window").arg(root.model.messagingRollingWindow) : qsTr("OpenMetrics value")
    }

    function protocolStatusRow(label, protocol, metricKey) {
        const rows = root.protocolHealthRows()
        const needle = String(protocol || "").toLowerCase()
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (String(row.label || "").toLowerCase().indexOf(needle) >= 0) {
                return root.statusRow(label, row.state, row.evidence, row.tone)
            }
        }
        return root.statusRow(label, root.metricKnown(metricKey) ? qsTr("observed") : qsTr("unknown"), root.metricKnown(metricKey) ? root.metricDisplay(metricKey) : qsTr("No protocol health or peer metric."), root.metricKnown(metricKey) ? "success" : "neutral")
    }

    function protocolRow(label, protocolId, signalKey) {
        let known = false
        let evidence = qsTr("No passive evidence")
        if (signalKey.indexOf("messaging.") === 0) {
            known = root.metricKnown(signalKey)
            evidence = root.metricDisplay(signalKey)
        } else if (signalKey.length > 0) {
            known = root.probeValue(signalKey) !== null
            evidence = root.valueSummary(root.probeValue(signalKey))
        }
        const row = root.statusRow(label, known ? qsTr("observed") : qsTr("unknown"), evidence, known ? "success" : "neutral")
        row.protocolId = protocolId
        return row
    }

    function probeRow(probe, fallbackLabel) {
        const ok = probe && probe.ok === true
        return root.statusRow(
            String(probe && probe.label ? probe.label : fallbackLabel),
            ok ? qsTr("ok") : qsTr("problem"),
            ok ? root.valueSummary(probe.value) : String(probe && probe.error ? probe.error : qsTr("No response")),
            ok ? "success" : "error"
        )
    }

    function detailRow(label, value) {
        const text = root.valueSummary(value)
        return {
            label: label,
            value: text,
            copyText: text === "-" || text === qsTr("n/a") ? "" : root.copyValue(value),
            source: root.sourceName()
        }
    }

    function metricKnown(key) {
        const value = root.model.dashboardMetricValue(key)
        return value !== null && value !== undefined
    }

    function restMetricsState() {
        const sourceMode = root.deliverySourceMode()
        const metricsKnown = root.sourceFactAvailable("metrics")
        if (sourceMode === "module") {
            return metricsKnown || root.moduleMetricsText().length > 0 ? qsTr("metrics") : qsTr("module")
        }
        if (sourceMode === "rest") {
            return metricsKnown ? qsTr("REST + metrics") : (root.status().ok ? qsTr("reachable") : qsTr("unknown"))
        }
        if (sourceMode === "metrics") {
            return root.status().ok ? qsTr("scraping") : qsTr("unknown")
        }
        if (sourceMode === "network-monitor") {
            return metricsKnown ? qsTr("monitor + metrics") : (root.status().ok ? qsTr("monitor") : qsTr("unknown"))
        }
        return qsTr("pending")
    }

    function restMetricsEvidence() {
        const sourceMode = root.deliverySourceMode()
        const metricsEvidence = root.sourceFactEvidence("metrics", "")
        if (sourceMode === "module") {
            return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : qsTr("Module API")
        }
        if (sourceMode === "metrics") {
            return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : root.shortText(root.model.messagingMetricsUrl, 48)
        }
        if (sourceMode === "network-monitor") {
            return qsTr("%1; metrics %2")
                .arg(root.shortText(root.model.deliverySourceTarget(), 24))
                .arg(root.shortText(root.model.messagingMetricsUrl, 24))
        }
        return root.shortText(root.model.messagingRestUrl, 48)
    }

    function moduleMetricsText() {
        const value = root.probeValue("collectOpenMetricsText")
        return typeof value === "string" ? value.trim() : ""
    }

    function restMetricsTone() {
        const sourceMode = root.deliverySourceMode()
        if (sourceMode === "unsupported") {
            return "warning"
        }
        if (root.model.sourceCapabilityAvailable(root.report(), "metrics") === false && (sourceMode === "metrics" || sourceMode === "network-monitor")) {
            return "warning"
        }
        return root.statusTone()
    }

    function deliverySourceMode() {
        return root.model.effectiveMessagingSourceMode(root.model.messagingSourceMode)
    }

    function networkMonitorPeerCount() {
        return root.countValue(root.probeValue("allPeersInfo"))
    }

    function networkMonitorTopicCount() {
        const value = root.probeValue("contentTopics")
        const count = root.countValue(value)
        if (count !== null) {
            return count
        }
        const metric = root.model.dashboardMetricValue("messaging.content_topics")
        return metric === null || metric === undefined ? null : Number(metric)
    }

    function servicePeerCount() {
        let total = 0
        let found = false
        const keys = ["messaging.store_peers", "messaging.filter_peers", "messaging.lightpush_peers"]
        for (let i = 0; i < keys.length; ++i) {
            const value = Number(root.model.dashboardMetricValue(keys[i]))
            if (Number.isFinite(value)) {
                total += value
                found = true
            }
        }
        return found ? total : null
    }

    function countValue(value) {
        return UiFormat.countValue(value, {
            scalarValue: root.model.scalarValue,
            nestedKeys: ["peers", "allPeers", "all_peers", "contentTopics", "content_topics", "topics", "items", "value", "result"]
        })
    }

    function valueSummary(value) {
        return UiFormat.valueSummary(value, {
            emptyText: qsTr("n/a"),
            emptyArrayText: qsTr("empty"),
            shortArrayLimit: 3,
            unwrapKeys: ["result", "value"],
            objectSummary: "fields"
        })
    }

    function copyValue(value) {
        return UiFormat.copyValue(value)
    }

    function shortText(value, maxLength) {
        return UiFormat.shortText(value, {
            emptyText: qsTr("n/a"),
            limit: maxLength || 32,
            minimum: 12,
            tailLength: 5
        })
    }

}
