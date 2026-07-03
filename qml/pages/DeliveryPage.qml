pragma ComponentBehavior: Bound

import QtQuick
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"

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
        breadcrumb: qsTr("Home / Delivery")
        title: qsTr("Delivery")
        layerLabel: qsTr("Module / Network")
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
                columns: root.width < 760 ? 2 : 5
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
                    delta: qsTr("%1 sent").arg(root.metricDisplay("messaging.message_sent_events_recent"))
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

                Panel {
                    theme: root.theme
                    title: qsTr("Live degradation")

                    Repeater {
                        model: root.healthRows()

                        StatusRow {
                            required property var modelData

                            theme: root.theme
                            label: String(modelData.label || "")
                            stateText: String(modelData.state || "")
                            evidence: String(modelData.evidence || "")
                            source: String(modelData.source || "")
                            freshness: String(modelData.freshness || "")
                            tone: String(modelData.tone || "neutral")
                        }
                    }
                }

                Panel {
                    theme: root.theme
                    title: qsTr("Protocol readiness")

                    Repeater {
                        model: root.protocolRows()

                        StatusRow {
                            required property var modelData

                            theme: root.theme
                            label: String(modelData.label || "")
                            stateText: String(modelData.state || "")
                            evidence: String(modelData.evidence || "")
                            source: String(modelData.source || "")
                            freshness: String(modelData.freshness || "")
                            tone: String(modelData.tone || "neutral")
                        }
                    }
                }
            }
        }
    }

    Component {
        id: healthTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Diagnostic checklist")

                Repeater {
                    model: root.healthRows().concat(root.evidenceRows())

                    StatusRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        stateText: String(modelData.state || "")
                        evidence: String(modelData.evidence || "")
                        source: String(modelData.source || "")
                        freshness: String(modelData.freshness || "")
                        tone: String(modelData.tone || "neutral")
                    }
                }
            }
        }
    }

    Component {
        id: topologyTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Local node identity")

                Repeater {
                    model: root.identityRows()

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

            Panel {
                theme: root.theme
                title: qsTr("Topology boundaries")

                Repeater {
                    model: root.topologyRows()

                    StatusRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        stateText: String(modelData.state || "")
                        evidence: String(modelData.evidence || "")
                        source: String(modelData.source || "")
                        freshness: String(modelData.freshness || "")
                        tone: String(modelData.tone || "neutral")
                    }
                }
            }
        }
    }

    Component {
        id: throughputTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Rolling-window rates")

                Repeater {
                    model: root.throughputRows()

                    StatusRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        stateText: String(modelData.state || "")
                        evidence: String(modelData.evidence || "")
                        source: String(modelData.source || "")
                        freshness: String(modelData.freshness || "")
                        tone: String(modelData.tone || "neutral")
                    }
                }
            }
        }
    }

    Component {
        id: protocolsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Protocols")

                Repeater {
                    model: root.protocolRows()

                    ProtocolRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        protocolId: String(modelData.protocolId || "")
                        stateText: String(modelData.state || "")
                        evidence: String(modelData.evidence || "")
                        tone: String(modelData.tone || "neutral")
                    }
                }
            }
        }
    }

    Component {
        id: topicsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Topics")

                Repeater {
                    model: root.topicRows()

                    StatusRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        stateText: String(modelData.state || "")
                        evidence: String(modelData.evidence || "")
                        source: String(modelData.source || "")
                        freshness: String(modelData.freshness || "")
                        tone: String(modelData.tone || "neutral")
                    }
                }
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

            Panel {
                theme: root.theme
                title: qsTr("Store")

                Repeater {
                    model: root.storeRows()

                    StatusRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        stateText: String(modelData.state || "")
                        evidence: String(modelData.evidence || "")
                        source: String(modelData.source || "")
                        freshness: String(modelData.freshness || "")
                        tone: String(modelData.tone || "neutral")
                    }
                }
            }
        }
    }

    Component {
        id: diagnosticsTab

        ColumnLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Panel {
                theme: root.theme
                title: qsTr("Read-only diagnostics")

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Refresh node info")
                        enabled: !root.pending()
                        Layout.preferredWidth: 150
                        accessibleName: qsTr("Refresh Delivery node information")
                        onClicked: root.refreshSource(true)
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Refresh metrics")
                        enabled: !root.pending()
                        Layout.preferredWidth: 140
                        accessibleName: qsTr("Refresh Delivery metrics")
                        onClicked: root.refreshSource(true)
                    }

                    Text {
                        text: root.statusLine()
                        color: root.theme.textMuted
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        font.pixelSize: root.theme.secondaryText
                        Layout.fillWidth: true
                    }
                }
            }

            Panel {
                theme: root.theme
                title: qsTr("Mutating diagnostics")

                StatusMessage {
                    theme: root.theme
                    tone: root.model.messagingMutatingDiagnosticsEnabled ? "warning" : "info"
                    title: root.model.messagingMutatingDiagnosticsEnabled ? qsTr("Permission enabled") : qsTr("Permission disabled")
                    message: qsTr("Dial, publish, subscribe, and lightpush probes are not auto-run. They need backend adapters and per-action confirmation.")
                    Layout.fillWidth: true
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Ping peer")
                        enabled: false
                        Layout.preferredWidth: 118
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Store query")
                        enabled: false
                        Layout.preferredWidth: 122
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Lightpush test")
                        enabled: false
                        Layout.preferredWidth: 136
                    }

                    Text {
                        text: qsTr("Adapters pending")
                        color: root.theme.textDim
                        textFormat: Text.PlainText
                        font.pixelSize: root.theme.dataText
                        Layout.fillWidth: true
                    }
                }
            }

            Panel {
                theme: root.theme
                title: qsTr("Probe evidence")

                Repeater {
                    model: root.probeRows()

                    StatusRow {
                        required property var modelData

                        theme: root.theme
                        label: String(modelData.label || "")
                        stateText: String(modelData.state || "")
                        evidence: String(modelData.evidence || "")
                        source: String(modelData.source || "")
                        freshness: String(modelData.freshness || "")
                        tone: String(modelData.tone || "neutral")
                    }
                }
            }
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
        switch (String(root.model.messagingSourceMode || "module")) {
        case "rest":
            return qsTr("REST")
        case "metrics":
            return qsTr("Metrics")
        case "network-monitor":
            return qsTr("Monitor")
        case "discovery-crawler":
            return qsTr("Crawler")
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
            root.model.messagingNetworkPreset,
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
        return root.model.moduleProbe("messaging", method)
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
        const peerId = root.probeValue("MyPeerId")
        if (peerId !== null) {
            return qsTr("peer id present")
        }
        const addresses = root.probeValue("MyMultiaddresses")
        if (addresses !== null) {
            return qsTr("addresses present")
        }
        return root.sourceName()
    }

    function healthRows() {
        const status = root.status()
        return [
            root.statusRow(qsTr("Source and lifecycle"), status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"), status.detail || qsTr("Not queried"), root.statusTone()),
            root.statusRow(qsTr("Identity"), root.probeValue("MyPeerId") !== null ? qsTr("present") : qsTr("unknown"), root.valueSummary(root.probeValue("MyPeerId")), root.probeValue("MyPeerId") !== null ? "success" : "neutral"),
            root.statusRow(qsTr("Preset, cluster, shards"), root.model.messagingNetworkPreset.length ? qsTr("configured") : qsTr("unknown"), root.model.messagingNetworkPreset || qsTr("No preset"), root.model.messagingNetworkPreset.length ? "success" : "neutral"),
            root.statusRow(qsTr("REST and metrics access"), root.restMetricsState(), root.restMetricsEvidence(), root.restMetricsTone()),
            root.statusRow(qsTr("Relay"), root.metricKnown("messaging.message_received_events_recent") ? qsTr("observed") : qsTr("unknown"), root.metricDisplay("messaging.message_received_events_recent"), root.metricKnown("messaging.message_received_events_recent") ? "success" : "neutral"),
            root.statusRow(qsTr("Store"), qsTr("not queried"), qsTr("Manual query only, includeData=false"), "neutral"),
            root.statusRow(qsTr("Filter"), root.metricKnown("messaging.active_subscriptions") ? qsTr("observed") : qsTr("unknown"), root.metricDisplay("messaging.active_subscriptions"), root.metricKnown("messaging.active_subscriptions") ? "success" : "neutral"),
            root.statusRow(qsTr("Lightpush"), root.metricKnown("messaging.publish_latency_ms") ? qsTr("observed") : qsTr("unknown"), root.metricDisplay("messaging.publish_latency_ms"), root.metricKnown("messaging.publish_latency_ms") ? "success" : "neutral"),
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
        return [
            root.statusRow(qsTr("Local connected peers"), root.metricKnown("messaging.peer_count") ? qsTr("observed") : qsTr("unknown"), root.metricDisplay("messaging.peer_count"), root.metricKnown("messaging.peer_count") ? "success" : "neutral"),
            root.statusRow(qsTr("Relay mesh peers"), qsTr("unknown"), qsTr("Current passive source does not expose mesh edges."), "neutral"),
            root.statusRow(qsTr("Discovery peers"), qsTr("unknown"), qsTr("Discovery crawler source is pending."), "neutral"),
            root.statusRow(qsTr("Service peers"), qsTr("unknown"), qsTr("Protocol-specific service peers not exposed by passive source."), "neutral")
        ]
    }

    function throughputRows() {
        return [
            root.metricRow(qsTr("Peer count"), "messaging.peer_count"),
            root.metricRow(qsTr("Outbound queue"), "messaging.outbound_queue"),
            root.metricRow(qsTr("Sent events"), "messaging.message_sent_events_recent"),
            root.metricRow(qsTr("Propagated events"), "messaging.message_propagated_events_recent"),
            root.metricRow(qsTr("Received events"), "messaging.message_received_events_recent"),
            root.metricRow(qsTr("Error events"), "messaging.message_error_events_recent"),
            root.metricRow(qsTr("Publish latency"), "messaging.publish_latency_ms"),
            root.metricRow(qsTr("Receive latency"), "messaging.receive_latency_ms")
        ]
    }

    function protocolRows() {
        return [
            root.protocolRow(qsTr("Relay"), "/vac/waku/relay/2.0.0", "messaging.message_received_events_recent"),
            root.protocolRow(qsTr("Store"), "/vac/waku/store/3.0.0", ""),
            root.protocolRow(qsTr("Filter"), "/vac/waku/filter/2.0.0-beta1", "messaging.active_subscriptions"),
            root.protocolRow(qsTr("Lightpush"), "/vac/waku/lightpush/2.0.0-beta1", "messaging.publish_latency_ms"),
            root.protocolRow(qsTr("Peer exchange"), "/vac/waku/peer-exchange/2.0.0-alpha1", ""),
            root.protocolRow(qsTr("Metadata"), "/vac/waku/metadata/1.0.0", "Version"),
            root.protocolRow(qsTr("Discv5"), "discv5", ""),
            root.protocolRow(qsTr("RLN relay"), "/vac/waku/rln-relay/2.0.0", ""),
            root.protocolRow(qsTr("Mix"), "mix", "MyMixPubKey")
        ]
    }

    function topicRows() {
        return [
            root.metricRow(qsTr("Pubsub topics"), "messaging.active_subscriptions"),
            root.metricRow(qsTr("Content topics"), "messaging.content_topics"),
            root.statusRow(qsTr("Topic-to-shard mapping"), qsTr("unknown"), qsTr("Requires topic metadata or network monitor source."), "neutral"),
            root.statusRow(qsTr("Query pressure"), qsTr("unknown"), qsTr("Store/filter query pressure not available from passive source."), "neutral")
        ]
    }

    function storeRows() {
        return [
            root.statusRow(qsTr("Store mounted state"), qsTr("unknown"), qsTr("No Store admin or metrics source selected."), "neutral"),
            root.statusRow(qsTr("Store peers"), qsTr("unknown"), qsTr("Service peer list unavailable."), "neutral"),
            root.statusRow(qsTr("Manual query"), qsTr("not queried"), qsTr("Default query must use includeData=false."), "neutral"),
            root.statusRow(qsTr("Payload viewing"), qsTr("disabled"), qsTr("Payload bytes stay hidden unless a future query opts in."), "success")
        ]
    }

    function identityRows() {
        return [
            root.detailRow(qsTr("Peer ID"), root.probeValue("MyPeerId")),
            root.detailRow(qsTr("ENR"), root.probeValue("MyENR")),
            root.detailRow(qsTr("Multiaddresses"), root.probeValue("MyMultiaddresses")),
            root.detailRow(qsTr("Bound ports"), root.probeValue("MyBoundPorts")),
            root.detailRow(qsTr("Mix public key"), root.probeValue("MyMixPubKey")),
            root.detailRow(qsTr("Version"), root.probeValue("Version") || root.probeValue("version"))
        ]
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
        return root.statusRow(label, known ? root.metricDisplay(key) : qsTr("n/a"), known ? qsTr("%1 s window").arg(root.model.messagingRollingWindow) : qsTr("Metric not exposed by current source."), known ? "success" : "neutral")
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
        if (root.model.messagingSourceMode === "module") {
            return root.probeValue("collectOpenMetricsText") !== null ? qsTr("metrics") : qsTr("module")
        }
        if (root.model.messagingSourceMode === "rest") {
            return root.status().ok ? qsTr("reachable") : qsTr("unknown")
        }
        if (root.model.messagingSourceMode === "metrics") {
            return root.status().ok ? qsTr("scraping") : qsTr("unknown")
        }
        return qsTr("pending")
    }

    function restMetricsEvidence() {
        if (root.model.messagingSourceMode === "module") {
            return root.probeValue("collectOpenMetricsText") !== null ? qsTr("OpenMetrics text available") : qsTr("Module API")
        }
        if (root.model.messagingSourceMode === "metrics") {
            return root.shortText(root.model.messagingMetricsUrl, 48)
        }
        return root.shortText(root.model.messagingRestUrl, 48)
    }

    function restMetricsTone() {
        if (root.model.messagingSourceMode === "network-monitor" || root.model.messagingSourceMode === "discovery-crawler") {
            return "warning"
        }
        return root.statusTone()
    }

    function valueSummary(value) {
        if (value === undefined || value === null || value === "") {
            return qsTr("n/a")
        }
        if (Array.isArray(value)) {
            if (value.length === 0) {
                return qsTr("empty")
            }
            if (value.length <= 3) {
                return value.map(function (item) { return String(item) }).join(", ")
            }
            return qsTr("%1 item(s)").arg(value.length)
        }
        if (typeof value === "object") {
            if (value.result !== undefined) {
                return root.valueSummary(value.result)
            }
            if (value.value !== undefined) {
                return root.valueSummary(value.value)
            }
            return qsTr("%1 field(s)").arg(Object.keys(value).length)
        }
        return String(value)
    }

    function copyValue(value) {
        if (value === undefined || value === null) {
            return ""
        }
        if (typeof value === "object") {
            return JSON.stringify(value, null, 2)
        }
        return String(value)
    }

    function shortText(value, maxLength) {
        const text = String(value || "")
        const limit = Math.max(12, Number(maxLength || 32))
        if (text.length <= limit) {
            return text.length ? text : qsTr("n/a")
        }
        return text.slice(0, Math.max(4, limit - 8)) + "..." + text.slice(-5)
    }

    component StatusRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string stateText: ""
        property string evidence: ""
        property string source: ""
        property string freshness: ""
        property string tone: "neutral"

        Layout.fillWidth: true
        implicitHeight: Math.max(48, rowGrid.implicitHeight + rowRoot.theme.gapSmall * 2)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: rowRoot.theme.gapSmall
            anchors.rightMargin: rowRoot.theme.gapSmall
            columns: root.width < 760 ? 2 : 4
            columnSpacing: rowRoot.theme.gap
            rowSpacing: 2

            RowLayout {
                spacing: rowRoot.theme.gapSmall
                Layout.preferredWidth: root.width < 760 ? 0 : 212
                Layout.fillWidth: root.width < 760

                Rectangle {
                    radius: 4
                    color: rowRoot.toneColor()
                    Layout.preferredWidth: 8
                    Layout.preferredHeight: 8
                    Layout.alignment: Qt.AlignVCenter
                    Accessible.ignored: true
                }

                Text {
                    text: rowRoot.label
                    color: rowRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: rowRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            Text {
                text: rowRoot.stateText
                color: rowRoot.toneColor()
                textFormat: Text.PlainText
                font.pixelSize: rowRoot.theme.dataText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.preferredWidth: root.width < 760 ? 110 : 118
            }

            Text {
                text: rowRoot.evidence
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.family: "monospace"
                font.pixelSize: rowRoot.theme.dataText
                elide: Text.ElideRight
                Layout.columnSpan: root.width < 760 ? 2 : 1
                Layout.fillWidth: true
            }

            Text {
                visible: root.width >= 760
                text: qsTr("%1 / %2").arg(rowRoot.source).arg(rowRoot.freshness)
                color: rowRoot.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: rowRoot.theme.dataText
                elide: Text.ElideRight
                Layout.preferredWidth: 192
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: rowRoot.theme.outlineMuted
            Accessible.ignored: true
        }

        function toneColor() {
            if (rowRoot.tone === "success") {
                return rowRoot.theme.success
            }
            if (rowRoot.tone === "warning") {
                return rowRoot.theme.warning
            }
            if (rowRoot.tone === "error") {
                return rowRoot.theme.error
            }
            return rowRoot.theme.textDim
        }
    }

    component DetailRow: Item {
        id: detailRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string copyText: ""
        property string source: ""

        Layout.fillWidth: true
        implicitHeight: Math.max(48, rowGrid.implicitHeight + detailRoot.theme.gapSmall * 2)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: detailRoot.theme.gapSmall
            anchors.rightMargin: detailRoot.theme.gapSmall
            columns: root.width < 720 ? 1 : 3
            columnSpacing: detailRoot.theme.gap
            rowSpacing: 2

            Text {
                text: detailRoot.label
                color: detailRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: detailRoot.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.preferredWidth: root.width < 720 ? 0 : 150
                Layout.fillWidth: root.width < 720
            }

            LinkCell {
                theme: detailRoot.theme
                text: detailRoot.value
                copyable: detailRoot.copyText.length > 0
                copyText: detailRoot.copyText
                link: false
                wrap: root.width < 720
                Layout.fillWidth: true
            }

            Text {
                visible: root.width >= 720
                text: detailRoot.source
                color: detailRoot.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: detailRoot.theme.dataText
                elide: Text.ElideRight
                Layout.preferredWidth: 180
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: detailRoot.theme.outlineMuted
            Accessible.ignored: true
        }
    }

    component ProtocolRow: Item {
        id: protocolRoot

        required property Theme theme
        property string label: ""
        property string protocolId: ""
        property string stateText: ""
        property string evidence: ""
        property string tone: "neutral"

        Layout.fillWidth: true
        implicitHeight: Math.max(52, rowGrid.implicitHeight + protocolRoot.theme.gapSmall * 2)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: protocolRoot.theme.gapSmall
            anchors.rightMargin: protocolRoot.theme.gapSmall
            columns: root.width < 780 ? 2 : 4
            columnSpacing: protocolRoot.theme.gap
            rowSpacing: 2

            Text {
                text: protocolRoot.label
                color: protocolRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: protocolRoot.theme.secondaryText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.preferredWidth: root.width < 780 ? 0 : 132
                Layout.fillWidth: root.width < 780
            }

            LinkCell {
                theme: protocolRoot.theme
                text: protocolRoot.protocolId
                copyable: protocolRoot.protocolId.length > 0
                copyText: protocolRoot.protocolId
                link: false
                Layout.fillWidth: true
            }

            Text {
                text: protocolRoot.stateText
                color: protocolRoot.toneColor()
                textFormat: Text.PlainText
                font.pixelSize: protocolRoot.theme.dataText
                font.weight: Font.DemiBold
                elide: Text.ElideRight
                Layout.preferredWidth: root.width < 780 ? 0 : 110
                Layout.fillWidth: root.width < 780
            }

            Text {
                text: protocolRoot.evidence
                color: protocolRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: protocolRoot.theme.dataText
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: protocolRoot.theme.outlineMuted
            Accessible.ignored: true
        }

        function toneColor() {
            if (protocolRoot.tone === "success") {
                return protocolRoot.theme.success
            }
            if (protocolRoot.tone === "warning") {
                return protocolRoot.theme.warning
            }
            if (protocolRoot.tone === "error") {
                return protocolRoot.theme.error
            }
            return protocolRoot.theme.textDim
        }
    }
}
