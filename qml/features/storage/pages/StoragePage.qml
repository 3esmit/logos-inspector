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
        const factEvidence = root.sourceFactEvidence("identity", "")
        if (factEvidence.length > 0 && factEvidence !== "not observed") {
            return factEvidence
        }
        const peerId = root.probeValue("peerId")
        if (peerId !== null) {
            return qsTr("peer id present")
        }
        const spr = root.probeValue("spr")
        if (spr !== null) {
            return qsTr("SPR present")
        }
        return root.sourceName()
    }

    function sourceFactAvailable(key) {
        return sourceSession.sourceFactAvailable(key)
    }

    function sourceFactEvidence(key, fallback) {
        return sourceSession.sourceFactEvidence(key, fallback)
    }

    function capacitySummary() {
        const factEvidence = root.sourceFactEvidence("space", "")
        if (factEvidence.length > 0 && factEvidence !== "not observed") {
            return factEvidence
        }
        const used = root.metricDisplay("storage.local_storage_used")
        if (used !== qsTr("n/a")) {
            return used
        }
        const space = root.probeValue("space")
        return space !== null ? root.valueSummary(space) : qsTr("n/a")
    }

    function transferSummary() {
        const uploads = root.metricDisplay("storage.active_uploads")
        const downloads = root.metricDisplay("storage.active_downloads")
        if (uploads === qsTr("n/a") && downloads === qsTr("n/a")) {
            return qsTr("n/a")
        }
        return qsTr("%1 upload requests / %2 download requests").arg(uploads).arg(downloads)
    }

    function reliabilityText() {
        if (root.failedProbeCount() > 0) {
            return qsTr("Degraded")
        }
        if (root.metricKnown("storage.failed_transfers_recent")) {
            return Number(root.model.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? qsTr("Recent failures") : qsTr("No failures")
        }
        return qsTr("Unknown")
    }

    function reliabilityTone() {
        if (root.failedProbeCount() > 0) {
            return root.theme.error
        }
        if (root.metricKnown("storage.failed_transfers_recent")) {
            return Number(root.model.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? root.theme.error : root.theme.success
        }
        return root.theme.textMuted
    }

    function transferFailureTone() {
        if (!root.metricKnown("storage.failed_transfers_recent")) {
            return root.theme.textMuted
        }
        return Number(root.model.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? root.theme.error : root.theme.success
    }

    function healthRows() {
        const status = root.status()
        const identityKnown = root.sourceFactAvailable("identity") || root.probeKnown("peerId") || root.probeKnown("spr")
        const debugKnown = root.sourceFactAvailable("debug") || root.probeKnown("debug")
        const spaceKnown = root.sourceFactAvailable("space") || root.probeKnown("space") || root.metricKnown("storage.local_storage_used")
        return [
            root.statusRow(qsTr("Source and lifecycle"), status.known ? (status.ok ? qsTr("reachable") : qsTr("problem")) : qsTr("unknown"), status.detail || qsTr("Not queried"), root.statusTone()),
            root.statusRow(qsTr("Identity"), identityKnown ? qsTr("present") : qsTr("unknown"), root.identityEvidence(), identityKnown ? "success" : "neutral"),
            root.statusRow(qsTr("REST and metrics access"), root.restMetricsState(), root.restMetricsEvidence(), root.restMetricsTone()),
            root.statusRow(qsTr("DHT / discovery"), debugKnown ? qsTr("observed") : qsTr("unknown"), debugKnown ? root.sourceFactEvidence("debug", root.valueSummary(root.probeValue("debug"))) : qsTr("Debug source unavailable."), debugKnown ? "success" : "neutral"),
            root.statusRow(qsTr("Connected peers"), root.metricKnown("storage.peer_count") ? qsTr("observed") : qsTr("unknown"), root.metricDisplay("storage.peer_count"), root.metricKnown("storage.peer_count") ? "success" : "neutral"),
            root.statusRow(qsTr("Repository and host disk"), spaceKnown ? qsTr("observed") : qsTr("unknown"), root.capacitySummary(), spaceKnown ? "success" : "neutral"),
            root.statusRow(qsTr("Recent transfer failures"), root.metricKnown("storage.failed_transfers_recent") ? root.metricDisplay("storage.failed_transfers_recent") : qsTr("unknown"), root.metricKnown("storage.failed_transfers_recent") ? qsTr("%1 s window").arg(root.model.storageRollingWindow) : qsTr("Metric not exposed by current source."), root.metricKnown("storage.failed_transfers_recent") ? (Number(root.model.dashboardMetricValue("storage.failed_transfers_recent")) > 0 ? "error" : "success") : "neutral"),
            root.statusRow(qsTr("Mix / private queries"), qsTr("not queried"), qsTr("No passive metric selected."), "neutral")
        ]
    }

    function activeOperationRows() {
        return [
            root.metricRow(qsTr("Upload requests"), "storage.active_uploads"),
            root.metricRow(qsTr("Download requests"), "storage.active_downloads"),
            root.metricRow(qsTr("Recent transfer failures"), "storage.failed_transfers_recent"),
            root.metricRow(qsTr("Historical transfer failures"), "storage.failed_transfers_total"),
            root.statusRow(qsTr("Provider lookup"), qsTr("idle"), qsTr("Explicit diagnostic only."), "neutral"),
            root.activeDownloadRow()
        ]
    }

    function topologyRows() {
        return [
            root.statusRow(qsTr("DHT routing table"), root.probeKnown("debug") ? qsTr("observed") : qsTr("unknown"), root.probeKnown("debug") ? root.valueSummary(root.probeValue("debug")) : qsTr("Current source has no DHT table."), root.probeKnown("debug") ? "success" : "neutral"),
            root.statusRow(qsTr("Connected peers"), root.metricKnown("storage.peer_count") ? root.metricDisplay("storage.peer_count") : qsTr("unknown"), root.metricKnown("storage.peer_count") ? qsTr("%1 s window").arg(root.model.storageRollingWindow) : qsTr("Metric not exposed by current source."), root.metricKnown("storage.peer_count") ? "success" : "neutral"),
            root.statusRow(qsTr("Providers for CID"), root.model.storageCidProbe.length > 0 ? qsTr("not queried") : qsTr("no CID"), root.model.storageCidProbe.length > 0 ? qsTr("Provider lookup is explicit.") : qsTr("Select a CID first."), "neutral"),
            root.statusRow(qsTr("Block exchange peers"), qsTr("unknown"), qsTr("Passive source does not expose transfer edges."), "neutral"),
            root.statusRow(qsTr("Mix proxies"), qsTr("unknown"), qsTr("Private-query topology is not exposed passively."), "neutral")
        ]
    }

    function capacityRows() {
        return [
            root.spaceRow(qsTr("Quota used"), ["quotaUsedBytes", "quota_used_bytes", "used", "usedBytes"]),
            root.spaceRow(qsTr("Quota reserved"), ["quotaReservedBytes", "quota_reserved_bytes", "reserved", "reservedBytes"]),
            root.spaceRow(qsTr("Quota max"), ["quotaMaxBytes", "quota_max_bytes", "max", "maxBytes"]),
            root.spaceRow(qsTr("Total blocks"), ["totalBlocks", "total_blocks", "blocks"]),
            root.metricRow(qsTr("Local storage used"), "storage.local_storage_used")
        ]
    }

    function repositoryRows() {
        const dataDir = root.probeKnown("dataDir") ? root.model.storageDisplayPath(root.valueSummary(root.probeValue("dataDir"))) : (root.model.storageDataDir.length > 0 ? root.model.storageDisplayPath(root.model.storageDataDir) : qsTr("No local path configured."))
        return [
            root.statusRow(qsTr("Data directory"), root.probeKnown("dataDir") || root.model.storageDataDir.length > 0 ? qsTr("configured") : qsTr("unknown"), dataDir, root.probeKnown("dataDir") || root.model.storageDataDir.length > 0 ? "success" : "neutral"),
            root.metricRow(qsTr("Shared files"), "storage.shared_files_count"),
            root.manifestCountRow()
        ]
    }

    function transferRows() {
        return [
            root.metricRow(qsTr("Upload requests"), "storage.active_uploads"),
            root.metricRow(qsTr("Download requests"), "storage.active_downloads"),
            root.metricRow(qsTr("Recent transfer failures"), "storage.failed_transfers_recent"),
            root.metricRow(qsTr("Historical transfer failures"), "storage.failed_transfers_total"),
            root.statusRow(qsTr("Upload diagnostics"), qsTr("disabled"), qsTr("Mutating diagnostics require explicit backend support."), root.model.storageMutatingDiagnosticsEnabled ? "warning" : "neutral"),
            root.activeDownloadRow()
        ]
    }

    function activeDownloadRow() {
        const operation = root.activeStorageOperation()
        const status = String(operation && operation.status ? operation.status : "")
        if (!operation || !status.length) {
            return root.statusRow(qsTr("Network download"), qsTr("idle"), qsTr("No active background download."), "success")
        }
        let tone = "neutral"
        if (status === "running" || status === "canceling") {
            tone = "warning"
        } else if (status === "completed") {
            tone = "success"
        } else if (status === "failed") {
            tone = "error"
        }
        return root.statusRow(qsTr("Network download"), status, root.activeStorageOperationDetail(operation), tone)
    }

    function activeStorageOperation() {
        const revision = root.model.storageActiveOperationRevision
        return root.model.storageActiveOperation || null
    }

    function activeStorageOperationDetail(operation) {
        const written = Number(operation && operation.bytesWritten ? operation.bytesWritten : 0)
        const total = Number(operation && operation.contentLength ? operation.contentLength : 0)
        const path = operation && operation.path ? root.shortText(operation.path, 42) : qsTr("n/a")
        if (Number.isFinite(total) && total > 0) {
            const percent = Math.min(100, Math.max(0, Math.floor((written / total) * 100)))
            return qsTr("%1 / %2 bytes (%3%) to %4")
                .arg(root.model.valueText(written))
                .arg(root.model.valueText(total))
                .arg(percent)
                .arg(path)
        }
        return qsTr("%1 bytes to %2").arg(root.model.valueText(written)).arg(path)
    }

    function cidRows() {
        const cid = String(root.model.storageCidProbe || "").trim()
        if (!cid.length) {
            return [
                root.detailRow(qsTr("Selected CID"), qsTr("n/a")),
                root.detailRow(qsTr("Network diagnostics"), qsTr("Not queried"))
            ]
        }
        const exists = root.probe("exists")
        return [
            root.detailRow(qsTr("Selected CID"), cid),
            root.detailRow(qsTr("Local exists"), exists ? (exists.ok ? root.valueSummary(exists.value) : String(exists.error || qsTr("problem"))) : qsTr("Not queried")),
            root.detailRow(qsTr("Manifest"), qsTr("Not fetched")),
            root.detailRow(qsTr("Providers"), qsTr("Not queried")),
            root.detailRow(qsTr("Transfer"), qsTr("Idle"))
        ]
    }

    function protocolRows() {
        return [
            root.protocolRow(qsTr("Store / RepoStore"), "repository", root.probeKnown("space") || root.probeKnown("manifests"), root.probeKnown("space") ? root.valueSummary(root.probeValue("space")) : root.valueSummary(root.probeValue("manifests"))),
            root.protocolRow(qsTr("Dataset / Manifest"), "storage-manifest", root.probeKnown("manifests"), root.valueSummary(root.probeValue("manifests"))),
            root.protocolRow(qsTr("Merkle verification"), "storage-root", false, qsTr("No passive verification source.")),
            root.protocolRow(qsTr("DHT discovery"), "libp2p/kad-dht", root.probeKnown("debug"), root.probeKnown("debug") ? root.valueSummary(root.probeValue("debug")) : qsTr("No DHT table.")),
            root.protocolRow(qsTr("Block exchange"), "storage/blockexchange", root.metricKnown("storage.active_downloads") || root.metricKnown("storage.active_uploads"), root.transferSummary()),
            root.protocolRow(qsTr("REST / C API"), "/api/storage/v1", root.storageSourceMode() === "rest", root.model.storageSourceTarget()),
            root.protocolRow(qsTr("Mix / private queries"), "private queries", false, qsTr("No passive signal."))
        ]
    }

    function identityRows() {
        return [
            root.detailRow(qsTr("Peer ID"), root.probeValue("peerId")),
            root.detailRow(qsTr("SPR"), root.probeValue("spr")),
            root.pathDetailRow(qsTr("Data directory"), root.probeValue("dataDir") || root.model.storageDataDir),
            root.detailRow(qsTr("Version"), root.probeValue("version") || root.probeValue("moduleVersion")),
            root.detailRow(qsTr("Network preset"), root.model.storageNetworkPreset),
            root.detailRow(qsTr("Source target"), root.model.storageSourceTarget())
        ]
    }

    function evidenceRows() {
        return sourceSession.evidenceRows(root, qsTr("Refresh source to load probe evidence."))
    }

    function statusRow(label, state, evidence, tone) {
        return sourceSession.statusRow(label, state, evidence, tone)
    }

    function metricRow(label, key) {
        const known = root.metricKnown(key)
        const tone = known && String(key || "") === "storage.failed_transfers_recent" && Number(root.model.dashboardMetricValue(key)) > 0 ? "error" : (known ? "success" : "neutral")
        return root.statusRow(label, known ? root.metricDisplay(key) : qsTr("n/a"), known ? root.metricEvidence(key) : qsTr("Metric not exposed by current source."), tone)
    }

    function metricEvidence(key) {
        switch (String(key || "")) {
        case "storage.active_uploads":
        case "storage.active_downloads":
        case "storage.failed_transfers_total":
            return qsTr("Counter total")
        default:
            return qsTr("%1 s window").arg(root.model.storageRollingWindow)
        }
    }

    function manifestCountRow() {
        const manifests = root.probeValue("manifests")
        if (Array.isArray(manifests)) {
            return root.statusRow(qsTr("Manifests"), qsTr("%1").arg(manifests.length), qsTr("Local manifest list"), "success")
        }
        return root.metricRow(qsTr("Manifests"), "storage.manifest_count")
    }

    function spaceRow(label, keys) {
        const value = root.objectField(root.probeValue("space"), keys)
        if (value !== null) {
            return root.statusRow(label, root.model.valueText(value), qsTr("space"), "success")
        }
        return root.statusRow(label, qsTr("n/a"), root.probeKnown("space") ? qsTr("Field not exposed by current space shape.") : qsTr("Space source unavailable."), "neutral")
    }

    function protocolRow(label, protocolId, observed, evidence) {
        return {
            label: label,
            protocolId: protocolId,
            state: observed ? qsTr("observed") : qsTr("unknown"),
            evidence: evidence === undefined || evidence === null || evidence === "" ? qsTr("No passive evidence") : String(evidence),
            tone: observed ? "success" : "neutral"
        }
    }

    function probeRow(probe, fallbackLabel) {
        return sourceSession.probeRow(probe, fallbackLabel)
    }

    function detailRow(label, value) {
        return sourceSession.detailRow(label, value, [qsTr("Not queried"), qsTr("Not fetched"), qsTr("Idle")])
    }

    function pathDetailRow(label, value) {
        const raw = root.valueSummary(value)
        const text = root.model.storageDisplayPath(raw)
        return {
            label: label,
            value: text.length ? text : qsTr("n/a"),
            copyText: root.model.storageLocalDiagnosticsEnabled ? root.copyValue(value) : "",
            source: root.sourceName()
        }
    }

    function restMetricsState() {
        const sourceMode = root.storageSourceMode()
        const metricsKnown = root.sourceFactAvailable("metrics")
        if (sourceMode === "module") {
            return metricsKnown || root.probeValue("collectMetrics") !== null ? qsTr("metrics") : qsTr("module")
        }
        if (sourceMode === "rest") {
            const metricsProbe = root.probe("collectMetrics")
            if (root.metricsEndpointConfigured() && metricsProbe && metricsProbe.ok === false) {
                return qsTr("metrics error")
            }
            if (root.metricsEndpointConfigured() && (!metricsProbe || metricsProbe.ok !== true)) {
                return root.status().ok ? qsTr("REST only") : qsTr("unknown")
            }
            return metricsKnown ? qsTr("REST + metrics") : (root.status().ok ? qsTr("reachable") : qsTr("unknown"))
        }
        if (sourceMode === "metrics") {
            return root.status().ok ? qsTr("scraping") : qsTr("unknown")
        }
        return qsTr("pending")
    }

    function restMetricsEvidence() {
        const sourceMode = root.storageSourceMode()
        const metricsEvidence = root.sourceFactEvidence("metrics", "")
        if (sourceMode === "module") {
            return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : qsTr("Module API")
        }
        if (sourceMode === "metrics") {
            return metricsEvidence.length > 0 && metricsEvidence !== "not observed" ? metricsEvidence : root.shortText(root.model.storageMetricsUrl, 48)
        }
        if (sourceMode === "rest" && root.metricsEndpointConfigured()) {
            const metricsProbe = root.probe("collectMetrics")
            if (metricsProbe && metricsProbe.ok === false && metricsProbe.error) {
                return qsTr("REST %1; metrics %2: %3")
                    .arg(root.shortText(root.model.storageRestUrl, 24))
                    .arg(root.shortText(root.model.storageMetricsUrl, 24))
                    .arg(root.shortText(metricsProbe.error, 36))
            }
            return qsTr("REST %1; metrics %2")
                .arg(root.shortText(root.model.storageRestUrl, 28))
                .arg(root.shortText(root.model.storageMetricsUrl, 28))
        }
        return root.shortText(root.model.storageRestUrl, 48)
    }

    function restMetricsTone() {
        const sourceMode = root.storageSourceMode()
        if (sourceMode === "c-library" || sourceMode === "local-os") {
            return "warning"
        }
        if (sourceMode === "rest") {
            const metricsProbe = root.probe("collectMetrics")
            if (root.metricsEndpointConfigured() && metricsProbe && metricsProbe.ok === false) {
                return "error"
            }
            if (root.metricsEndpointConfigured() && root.model.sourceCapabilityAvailable(root.report(), "metrics") === false) {
                return "warning"
            }
            if (root.metricsEndpointConfigured() && (!metricsProbe || metricsProbe.ok !== true)) {
                return "warning"
            }
        }
        return root.statusTone()
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
        if (value === undefined || value === null) {
            return null
        }
        if (typeof value !== "object") {
            return null
        }
        if (value.result !== undefined) {
            return root.objectField(value.result, keys)
        }
        if (value.value !== undefined) {
            return root.objectField(value.value, keys)
        }
        const wanted = Array.isArray(keys) ? keys : [keys]
        for (let i = 0; i < wanted.length; ++i) {
            const key = String(wanted[i] || "")
            if (value[key] !== undefined && value[key] !== null) {
                return value[key]
            }
        }
        return null
    }

    function shortText(value, maxLength) {
        return sourceSession.shortText(value, maxLength)
    }

}
