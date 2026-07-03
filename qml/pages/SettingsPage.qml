pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"

ColumnLayout {
    id: settingsRoot

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    ListModel {
        id: settingsSections

        ListElement { value: "general"; label: "General" }
        ListElement { value: "network"; label: "Network" }
        ListElement { value: "ui"; label: "User Interface" }
    }

    ListModel {
        id: networkSections

        ListElement { value: "blockchain"; label: "Blockchain" }
        ListElement { value: "indexer"; label: "Indexer" }
        ListElement { value: "execution"; label: "Execution Zone" }
        ListElement { value: "messaging"; label: "Messaging / Delivery" }
        ListElement { value: "storage"; label: "Storage" }
    }

    ListModel {
        id: uiSections

        ListElement { value: "footer"; label: "Footer" }
        ListElement { value: "dashboard"; label: "Dashboard" }
    }

    ListModel {
        id: profileOptions

        ListElement {
            key: "default"
            label: "Testnet"
            summary: "Public LEZ, local indexer and node defaults"
        }
        ListElement {
            key: "local"
            label: "Local sequencer"
            summary: "Local sequencer, indexer, and node"
        }
        ListElement {
            key: "custom"
            label: "Custom"
            summary: "Manual endpoint override"
        }
    }

    ListModel {
        id: deliverySourceOptions

        ListElement {
            key: "module"
            label: "Basecamp module"
            summary: "Local logoscore delivery_module bridge"
        }
        ListElement {
            key: "rest"
            label: "Direct Waku REST"
            summary: "Read-only health, info, version, and optional metrics"
        }
        ListElement {
            key: "metrics"
            label: "Metrics only"
            summary: "Scrape a Prometheus/OpenMetrics endpoint"
        }
        ListElement {
            key: "network-monitor"
            label: "Network monitor"
            summary: "Fleet monitor source; adapter pending"
        }
        ListElement {
            key: "discovery-crawler"
            label: "Discovery crawler"
            summary: "Preset and bootnode crawl; adapter pending"
        }
    }

    ListModel {
        id: storageSourceOptions

        ListElement {
            key: "module"
            label: "Basecamp module"
            summary: "Local logoscore storage_module bridge"
        }
        ListElement {
            key: "rest"
            label: "Standalone REST"
            summary: "Read-only space, identity, local data, debug, and metrics"
        }
        ListElement {
            key: "metrics"
            label: "Metrics only"
            summary: "Scrape a Prometheus/OpenMetrics endpoint"
        }
        ListElement {
            key: "c-library"
            label: "C library"
            summary: "Embedded storage library source; adapter pending"
        }
        ListElement {
            key: "local-os"
            label: "Local OS diagnostics"
            summary: "Process, data directory, and port checks; adapter pending"
        }
    }

    PageHeader {
        theme: settingsRoot.theme
        breadcrumb: qsTr("Home / Settings")
        title: qsTr("Settings")
        layerLabel: qsTr("System")
        subtitle: qsTr("Configure profiles, network connections, footer status fields, and dashboard graphs.")
        Layout.fillWidth: true
    }

    TabSwitch {
        theme: settingsRoot.theme
        current: settingsRoot.model.settingsSection
        options: settingsSections
        Layout.fillWidth: true
        onSelected: value => settingsRoot.model.settingsSection = value
    }

    Loader {
        active: true
        asynchronous: true
        sourceComponent: settingsRoot.sectionComponent(settingsRoot.model.settingsSection)
        Layout.fillWidth: true
    }

    Component {
        id: generalSection

        ColumnLayout {
            spacing: settingsRoot.theme.gap
            Layout.fillWidth: true

            GridLayout {
                columns: settingsRoot.width < 760 ? 2 : 4
                columnSpacing: settingsRoot.theme.gap
                rowSpacing: settingsRoot.theme.gap
                Layout.fillWidth: true

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Profile")
                    value: settingsRoot.profileLabel(settingsRoot.model.networkProfile)
                    delta: settingsRoot.profileSummary(settingsRoot.model.networkProfile)
                    deltaColor: settingsRoot.model.networkProfile === "custom" ? settingsRoot.theme.warning : settingsRoot.theme.textMuted
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Blockchain")
                    value: settingsRoot.connectionStatusText("blockchain")
                    delta: settingsRoot.shortEndpoint(settingsRoot.model.nodeUrl)
                    deltaColor: settingsRoot.connectionStatusColor("blockchain")
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Execution Zone")
                    value: settingsRoot.connectionStatusText("execution")
                    delta: settingsRoot.shortEndpoint(settingsRoot.model.sequencerUrl)
                    deltaColor: settingsRoot.connectionStatusColor("execution")
                }

                MetricCard {
                    theme: settingsRoot.theme
                    compact: true
                    label: qsTr("Indexer")
                    value: settingsRoot.connectionStatusText("indexer")
                    delta: settingsRoot.shortEndpoint(settingsRoot.model.indexerUrl)
                    deltaColor: settingsRoot.connectionStatusColor("indexer")
                }
            }

            Panel {
                theme: settingsRoot.theme
                title: qsTr("General")

                ColumnLayout {
                    spacing: settingsRoot.theme.gapSmall
                    Layout.fillWidth: true

                    Text {
                        text: qsTr("Network profile")
                        color: settingsRoot.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: settingsRoot.theme.secondaryText
                        font.weight: Font.Medium
                        Layout.fillWidth: true
                    }

                    ProfileComboBox {
                        theme: settingsRoot.theme
                        options: profileOptions
                        currentIndex: settingsRoot.profileIndexFor(settingsRoot.model.networkProfile)
                        Layout.fillWidth: true
                        onProfileActivated: index => settingsRoot.applyProfileIndex(index)
                    }

                    StatusMessage {
                        theme: settingsRoot.theme
                        tone: settingsRoot.model.networkProfile === "custom" ? "warning" : "info"
                        title: settingsRoot.profileLabel(settingsRoot.model.networkProfile)
                        message: settingsRoot.profileDetail()
                        Layout.fillWidth: true
                    }
                }
            }
        }
    }

    Component {
        id: networkSection

        ColumnLayout {
            spacing: settingsRoot.theme.gap
            Layout.fillWidth: true

            TabSwitch {
                theme: settingsRoot.theme
                current: settingsRoot.model.settingsNetworkSection
                options: networkSections
                Layout.fillWidth: true
                onSelected: value => settingsRoot.model.settingsNetworkSection = value
            }

            Loader {
                active: true
                asynchronous: true
                sourceComponent: settingsRoot.networkComponent(settingsRoot.model.settingsNetworkSection)
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: blockchainNetwork

        NetworkConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Bedrock Blockchain")
            subtitle: qsTr("RPC connection used for node health, consensus, blocks, and channel scans.")
            kind: "blockchain"
            connectionType: qsTr("RPC")
            endpointLabel: qsTr("RPC URL")
            endpoint: settingsRoot.model.nodeUrl
            primaryFieldVisible: true
            moduleFieldVisible: false
            refreshRate: settingsRoot.model.blockchainRefreshRate
            statusText: settingsRoot.connectionStatusText("blockchain")
            statusDetail: settingsRoot.connectionStatusDetail("blockchain")
            statusColor: settingsRoot.connectionStatusColor("blockchain")
            onEndpointEdited: value => settingsRoot.updateNodeUrl(value)
            onRefreshRateEdited: value => settingsRoot.model.setNetworkConnectionRate("blockchain", value)
            onQueryClicked: settingsRoot.model.queryNetworkConnection("blockchain", true)
        }
    }

    Component {
        id: indexerNetwork

        NetworkConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Indexer")
            subtitle: qsTr("RPC connection used for finalized head, block lookup, transfer activity, and transaction history.")
            kind: "indexer"
            connectionType: qsTr("RPC")
            endpointLabel: qsTr("RPC URL")
            endpoint: settingsRoot.model.indexerUrl
            primaryFieldVisible: true
            moduleFieldVisible: false
            refreshRate: settingsRoot.model.indexerRefreshRate
            statusText: settingsRoot.connectionStatusText("indexer")
            statusDetail: settingsRoot.connectionStatusDetail("indexer")
            statusColor: settingsRoot.connectionStatusColor("indexer")
            onEndpointEdited: value => settingsRoot.updateIndexerUrl(value)
            onRefreshRateEdited: value => settingsRoot.model.setNetworkConnectionRate("indexer", value)
            onQueryClicked: settingsRoot.model.queryNetworkConnection("indexer", true)
        }
    }

    Component {
        id: executionNetwork

        NetworkConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Logos Execution Zone")
            subtitle: qsTr("Sequencer RPC used for LEZ blocks, accounts, transactions, and SPEL program inspection.")
            kind: "execution"
            connectionType: qsTr("RPC")
            endpointLabel: qsTr("RPC URL")
            endpoint: settingsRoot.model.sequencerUrl
            primaryFieldVisible: true
            moduleFieldVisible: false
            refreshRate: settingsRoot.model.executionRefreshRate
            statusText: settingsRoot.connectionStatusText("execution")
            statusDetail: settingsRoot.connectionStatusDetail("execution")
            statusColor: settingsRoot.connectionStatusColor("execution")
            onEndpointEdited: value => settingsRoot.updateSequencerUrl(value)
            onRefreshRateEdited: value => settingsRoot.model.setNetworkConnectionRate("execution", value)
            onQueryClicked: settingsRoot.model.queryNetworkConnection("execution", true)
        }
    }

    Component {
        id: messagingNetwork

        DeliveryConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Messaging / Delivery")
            subtitle: qsTr("Configure the Delivery inspection source. Probes here are read-only status checks.")
            statusText: settingsRoot.connectionStatusText("messaging")
            statusDetail: settingsRoot.connectionStatusDetail("messaging")
            statusColor: settingsRoot.connectionStatusColor("messaging")
            sourceOptions: deliverySourceOptions
            onQueryClicked: settingsRoot.model.queryNetworkConnection("messaging", true)
        }
    }

    Component {
        id: storageNetwork

        StorageConnectionPanel {
            theme: settingsRoot.theme
            title: qsTr("Storage")
            subtitle: qsTr("Configure the Storage inspection source. Safe checks only query identity, space, local manifests, metrics, and optional local exists.")
            statusText: settingsRoot.connectionStatusText("storage")
            statusDetail: settingsRoot.connectionStatusDetail("storage")
            statusColor: settingsRoot.connectionStatusColor("storage")
            sourceOptions: storageSourceOptions
            onQueryClicked: settingsRoot.model.queryNetworkConnection("storage", true)
        }
    }

    Component {
        id: uiSection

        ColumnLayout {
            spacing: settingsRoot.theme.gap
            Layout.fillWidth: true

            TabSwitch {
                theme: settingsRoot.theme
                current: settingsRoot.model.settingsUiSection
                options: uiSections
                Layout.fillWidth: true
                onSelected: value => settingsRoot.model.settingsUiSection = value
            }

            Loader {
                active: true
                asynchronous: true
                sourceComponent: settingsRoot.uiComponent(settingsRoot.model.settingsUiSection)
                Layout.fillWidth: true
            }
        }
    }

    Component {
        id: footerSettings

        FieldSelector {
            theme: settingsRoot.theme
            title: qsTr("Footer fields")
            description: qsTr("Choose concise status fields for the persistent footer. The footer groups network context on the left and health/action fields on the right.")
            groups: settingsRoot.footerFieldGroups()
            mode: "footer"
        }
    }

    Component {
        id: dashboardSettings

        FieldSelector {
            theme: settingsRoot.theme
            title: qsTr("Dashboard graphs")
            description: qsTr("Choose the live graph tiles shown above dashboard tables.")
            groups: settingsRoot.dashboardGraphGroups()
            mode: "dashboard"
        }
    }

    function sectionComponent(section) {
        switch (section) {
        case "network":
            return networkSection
        case "ui":
            return uiSection
        default:
            return generalSection
        }
    }

    function networkComponent(section) {
        switch (section) {
        case "indexer":
            return indexerNetwork
        case "execution":
            return executionNetwork
        case "messaging":
            return messagingNetwork
        case "storage":
            return storageNetwork
        default:
            return blockchainNetwork
        }
    }

    function uiComponent(section) {
        return section === "dashboard" ? dashboardSettings : footerSettings
    }

    function connectionStatus(kind) {
        return settingsRoot.model.networkConnectionState(kind)
    }

    function connectionStatusText(kind) {
        const status = settingsRoot.connectionStatus(kind)
        if (!status.known) {
            return qsTr("Unknown")
        }
        return status.ok ? qsTr("OK") : qsTr("Error")
    }

    function connectionStatusDetail(kind) {
        const status = settingsRoot.connectionStatus(kind)
        if (!status.known) {
            return qsTr("Not queried. Auto refresh runs every %1 seconds.").arg(settingsRoot.model.networkConnectionRate(kind))
        }
        const checked = status.checkedAt && status.checkedAt.length ? qsTr(" at %1").arg(status.checkedAt) : ""
        return qsTr("%1%2").arg(status.detail || "").arg(checked)
    }

    function connectionStatusColor(kind) {
        const status = settingsRoot.connectionStatus(kind)
        if (!status.known) {
            return settingsRoot.theme.textMuted
        }
        return status.ok ? settingsRoot.theme.success : settingsRoot.theme.warning
    }

    function updateSequencerUrl(value) {
        settingsRoot.model.sequencerUrl = String(value || "").trim()
        settingsRoot.syncProfileFromEndpoints()
    }

    function updateIndexerUrl(value) {
        settingsRoot.model.indexerUrl = String(value || "").trim()
        settingsRoot.syncProfileFromEndpoints()
    }

    function updateNodeUrl(value) {
        settingsRoot.model.nodeUrl = String(value || "").trim()
        settingsRoot.syncProfileFromEndpoints()
    }

    function syncProfileFromEndpoints() {
        settingsRoot.model.networkProfile = settingsRoot.inferProfile(settingsRoot.model.sequencerUrl, settingsRoot.model.indexerUrl, settingsRoot.model.nodeUrl)
    }

    function applyProfileIndex(index) {
        if (index === 2) {
            settingsRoot.syncProfileFromEndpoints()
            return
        }
        settingsRoot.model.applyProfile(index)
    }

    function deliverySourceIndexFor(value) {
        const source = String(value || "module")
        for (let i = 0; i < deliverySourceOptions.count; ++i) {
            if (deliverySourceOptions.get(i).key === source) {
                return i
            }
        }
        return 0
    }

    function deliverySourceModeAt(index) {
        if (index < 0 || index >= deliverySourceOptions.count) {
            return "module"
        }
        return deliverySourceOptions.get(index).key
    }

    function storageSourceIndexFor(value) {
        const source = String(value || "module")
        for (let i = 0; i < storageSourceOptions.count; ++i) {
            if (storageSourceOptions.get(i).key === source) {
                return i
            }
        }
        return 0
    }

    function storageSourceModeAt(index) {
        if (index < 0 || index >= storageSourceOptions.count) {
            return "module"
        }
        return storageSourceOptions.get(index).key
    }

    function profileIndexFor(value) {
        if (value === "local") {
            return 1
        }
        if (value === "custom") {
            return 2
        }
        return 0
    }

    function inferProfile(sequencer, indexer, node) {
        const seq = settingsRoot.normalizeEndpoint(sequencer)
        const idx = settingsRoot.normalizeEndpoint(indexer)
        const nod = settingsRoot.normalizeEndpoint(node)
        const testnetSeq = settingsRoot.normalizeEndpoint("https://testnet.lez.logos.co/")
        const localSeq = settingsRoot.normalizeEndpoint("http://127.0.0.1:3040/")
        const localIndexer = settingsRoot.normalizeEndpoint("http://127.0.0.1:8779/")
        const localNode = settingsRoot.normalizeEndpoint("http://127.0.0.1:8080/")

        if (seq === localSeq && idx === localIndexer && nod === localNode) {
            return "local"
        }
        if (seq === testnetSeq && idx === localIndexer && nod === localNode) {
            return "default"
        }
        return "custom"
    }

    function profileLabel(value) {
        if (value === "local") {
            return qsTr("Local")
        }
        if (value === "custom") {
            return qsTr("Custom")
        }
        return qsTr("Testnet")
    }

    function profileSummary(value) {
        if (value === "local") {
            return qsTr("All endpoints local")
        }
        if (value === "custom") {
            return qsTr("Manual endpoints")
        }
        return qsTr("Default testnet")
    }

    function profileDetail() {
        return qsTr("%1 / %2 / %3")
            .arg(settingsRoot.shortEndpoint(settingsRoot.model.sequencerUrl))
            .arg(settingsRoot.shortEndpoint(settingsRoot.model.indexerUrl))
            .arg(settingsRoot.shortEndpoint(settingsRoot.model.nodeUrl))
    }

    function normalizeEndpoint(value) {
        return String(value || "").trim().replace(/\/+$/, "")
    }

    function shortEndpoint(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("Not configured")
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "")
    }

    function footerFieldGroups() {
        return [
            { title: qsTr("Network"), fields: [
                { key: "network.network", label: qsTr("network"), detail: qsTr("testnet, mainnet, local, or custom") },
                { key: "network.chain_id", label: qsTr("chain_id"), detail: qsTr("Bedrock chain identifier") },
                { key: "network.zone_id", label: qsTr("zone_id"), detail: qsTr("Execution zone identifier") },
                { key: "network.channel_id", label: qsTr("channel_id"), detail: qsTr("Active delivery channel identifier") },
                { key: "network.report_time", label: qsTr("report_time"), detail: qsTr("Last local report timestamp") }
            ] },
            { title: qsTr("Bedrock Blockchain"), fields: [
                { key: "bedrock.node_health", label: qsTr("node_health"), detail: qsTr("ok, degraded, or down") },
                { key: "bedrock.peer_count", label: qsTr("peer_count"), detail: qsTr("Connected Bedrock peers") },
                { key: "bedrock.sync_state", label: qsTr("sync_state"), detail: qsTr("synced, syncing, or stalled") },
                { key: "bedrock.tip_height", label: qsTr("tip_height"), detail: qsTr("Current tip height") },
                { key: "bedrock.tip_hash", label: qsTr("tip_hash"), detail: qsTr("Current tip hash") },
                { key: "bedrock.lib_height", label: qsTr("lib_height"), detail: qsTr("Last irreversible block height") },
                { key: "bedrock.lib_hash", label: qsTr("lib_hash"), detail: qsTr("Last irreversible block hash") },
                { key: "bedrock.tip_minus_lib", label: qsTr("tip_minus_lib"), detail: qsTr("Distance from tip to LIB") },
                { key: "bedrock.last_tip_time", label: qsTr("last_tip_time"), detail: qsTr("Last tip observation time") },
                { key: "bedrock.last_lib_time", label: qsTr("last_lib_time"), detail: qsTr("Last LIB observation time") },
                { key: "bedrock.finality_lag_seconds", label: qsTr("finality_lag_seconds"), detail: qsTr("Approximate finality lag") }
            ] },
            { title: qsTr("LEZ Sequencer"), fields: [
                { key: "lez.rpc_health", label: qsTr("rpc_health"), detail: qsTr("Sequencer RPC availability") },
                { key: "lez.sequencer_version", label: qsTr("sequencer_version"), detail: qsTr("Sequencer version") },
                { key: "lez.last_lez_block_id", label: qsTr("last_lez_block_id"), detail: qsTr("Latest LEZ block id") },
                { key: "lez.last_lez_block_hash", label: qsTr("last_lez_block_hash"), detail: qsTr("Latest LEZ block hash") },
                { key: "lez.last_lez_block_time", label: qsTr("last_lez_block_time"), detail: qsTr("Latest LEZ block time") },
                { key: "lez.pending_tx_count", label: qsTr("pending_tx_count"), detail: qsTr("Pending sequencer transactions") },
                { key: "lez.mempool_tx_count", label: qsTr("mempool_tx_count"), detail: qsTr("Mempool transaction count") },
                { key: "lez.rejected_tx_count_recent", label: qsTr("rejected_tx_count_recent"), detail: qsTr("Recent rejected transactions") },
                { key: "lez.blocks_produced_recent", label: qsTr("blocks_produced_recent"), detail: qsTr("Recent LEZ blocks produced") },
                { key: "lez.publish_to_bedrock_status", label: qsTr("publish_to_bedrock_status"), detail: qsTr("Bedrock publish state") },
                { key: "lez.last_published_channel_update", label: qsTr("last_published_channel_update"), detail: qsTr("Last channel update publication") },
                { key: "lez.last_finalized_callback_height", label: qsTr("last_finalized_callback_height"), detail: qsTr("Last finalized callback height") },
                { key: "lez.pending_blocks_count", label: qsTr("pending_blocks_count"), detail: qsTr("Pending LEZ blocks") }
            ] },
            { title: qsTr("Indexer"), fields: [
                { key: "indexer.rpc_health", label: qsTr("rpc_health"), detail: qsTr("Indexer RPC availability") },
                { key: "indexer.indexer_version", label: qsTr("indexer_version"), detail: qsTr("Indexer version") },
                { key: "indexer.indexed_finalized_height", label: qsTr("indexed_finalized_height"), detail: qsTr("Indexed finalized height") },
                { key: "indexer.indexed_finalized_hash", label: qsTr("indexed_finalized_hash"), detail: qsTr("Indexed finalized hash") },
                { key: "indexer.indexed_channel_message", label: qsTr("indexed_channel_message"), detail: qsTr("Indexed channel message") },
                { key: "indexer.indexer_lag_vs_sequencer_head", label: qsTr("indexer_lag_vs_sequencer_head"), detail: qsTr("Indexer lag versus sequencer") },
                { key: "indexer.last_indexed_time", label: qsTr("last_indexed_time"), detail: qsTr("Last indexed timestamp") },
                { key: "indexer.db_health", label: qsTr("db_health"), detail: qsTr("Database health") },
                { key: "indexer.ingestion_status", label: qsTr("ingestion_status"), detail: qsTr("running, stalled, or backfilling") }
            ] },
            { title: qsTr("Storage"), fields: [
                { key: "storage.module", label: qsTr("module"), detail: qsTr("loaded, running, or stopped") },
                { key: "storage.network", label: qsTr("network"), detail: qsTr("Storage preset or network name") },
                { key: "storage.node_reachable", label: qsTr("node_reachable"), detail: qsTr("Storage node reachability") },
                { key: "storage.nat_mode", label: qsTr("nat_mode"), detail: qsTr("upnp, port-forward, or manual") },
                { key: "storage.udp_discovery_port", label: qsTr("udp_discovery_port"), detail: qsTr("UDP discovery port state") },
                { key: "storage.tcp_transfer_port", label: qsTr("tcp_transfer_port"), detail: qsTr("TCP transfer port state") },
                { key: "storage.peer_count", label: qsTr("peer_count"), detail: qsTr("Storage peers") },
                { key: "storage.dht_connected", label: qsTr("dht_connected"), detail: qsTr("DHT connectivity") },
                { key: "storage.shared_files_count", label: qsTr("shared_files_count"), detail: qsTr("Shared files") },
                { key: "storage.manifest_count", label: qsTr("manifest_count"), detail: qsTr("Manifest count") },
                { key: "storage.local_storage_used", label: qsTr("local_storage_used"), detail: qsTr("Local storage usage") },
                { key: "storage.active_uploads", label: qsTr("upload_requests_total"), detail: qsTr("Upload request counter total") },
                { key: "storage.active_downloads", label: qsTr("download_requests_total"), detail: qsTr("Download request counter total") },
                { key: "storage.failed_transfers_recent", label: qsTr("transfer_failures_total"), detail: qsTr("Transfer failure counter total") },
                { key: "storage.cid_fetch_test", label: qsTr("cid_fetch_test"), detail: qsTr("CID fetch probe result") },
                { key: "storage.last_error", label: qsTr("last_error"), detail: qsTr("Latest storage error") }
            ] },
            { title: qsTr("Messaging / Delivery"), fields: [
                { key: "messaging.module", label: qsTr("module"), detail: qsTr("loaded, running, or stopped") },
                { key: "messaging.connection_state", label: qsTr("connection_state"), detail: qsTr("connected, disconnected, or connecting") },
                { key: "messaging.peer_count", label: qsTr("peer_count"), detail: qsTr("Delivery peers") },
                { key: "messaging.active_subscriptions", label: qsTr("active_subscriptions"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.content_topics", label: qsTr("content_topics"), detail: qsTr("Subscribed content topics") },
                { key: "messaging.outbound_queue", label: qsTr("outbound_queue"), detail: qsTr("Outbound message queue") },
                { key: "messaging.message_sent_events_recent", label: qsTr("message_sent_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_propagated_events_recent", label: qsTr("message_propagated_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_received_events_recent", label: qsTr("waku_node_messages_total"), detail: qsTr("Delivery message counter total") },
                { key: "messaging.message_error_events_recent", label: qsTr("waku_node_errors_total"), detail: qsTr("Delivery error counter total") },
                { key: "messaging.publish_latency_ms", label: qsTr("publish_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.receive_latency_ms", label: qsTr("receive_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.last_error", label: qsTr("last_error"), detail: qsTr("Latest Delivery error") }
            ] },
            { title: qsTr("Overall"), fields: [
                { key: "overall.status", label: qsTr("status"), detail: qsTr("healthy, degraded, or down") },
                { key: "overall.main_risk", label: qsTr("main_risk"), detail: qsTr("Most important current risk") },
                { key: "overall.operator_action", label: qsTr("operator_action"), detail: qsTr("Suggested operator action") }
            ] }
        ]
    }

    function dashboardGraphGroups() {
        return [
            { title: qsTr("Bedrock Blockchain"), fields: [
                { key: "bedrock.peer_count", label: qsTr("peer_count"), detail: qsTr("Connected Bedrock peers") },
                { key: "bedrock.tip_minus_lib", label: qsTr("tip_minus_lib"), detail: qsTr("Tip to LIB distance") },
                { key: "bedrock.finality_lag_seconds", label: qsTr("finality_lag_seconds"), detail: qsTr("Finality lag in seconds") }
            ] },
            { title: qsTr("LEZ Sequencer"), fields: [
                { key: "lez.pending_tx_count", label: qsTr("pending_tx_count"), detail: qsTr("Pending sequencer transactions") },
                { key: "lez.mempool_tx_count", label: qsTr("mempool_tx_count"), detail: qsTr("Mempool transaction count") },
                { key: "lez.rejected_tx_count_recent", label: qsTr("rejected_tx_count_recent"), detail: qsTr("Recent rejected transactions") },
                { key: "lez.blocks_produced_recent", label: qsTr("blocks_produced_recent"), detail: qsTr("Recent produced blocks") },
                { key: "lez.pending_blocks_count", label: qsTr("pending_blocks_count"), detail: qsTr("Pending LEZ blocks") }
            ] },
            { title: qsTr("Indexer"), fields: [
                { key: "indexer.indexer_lag_vs_sequencer_head", label: qsTr("indexer_lag_vs_sequencer_head"), detail: qsTr("Indexer lag versus sequencer head") }
            ] },
            { title: qsTr("Storage"), fields: [
                { key: "storage.peer_count", label: qsTr("peer_count"), detail: qsTr("Storage peers") },
                { key: "storage.shared_files_count", label: qsTr("shared_files_count"), detail: qsTr("Shared files") },
                { key: "storage.manifest_count", label: qsTr("manifest_count"), detail: qsTr("Manifests") },
                { key: "storage.local_storage_used", label: qsTr("local_storage_used"), detail: qsTr("Local storage usage") },
                { key: "storage.active_uploads", label: qsTr("upload_requests_total"), detail: qsTr("Upload request counter total") },
                { key: "storage.active_downloads", label: qsTr("download_requests_total"), detail: qsTr("Download request counter total") },
                { key: "storage.failed_transfers_recent", label: qsTr("transfer_failures_total"), detail: qsTr("Transfer failure counter total") }
            ] },
            { title: qsTr("Messaging / Delivery"), fields: [
                { key: "messaging.peer_count", label: qsTr("peer_count"), detail: qsTr("Delivery peers") },
                { key: "messaging.active_subscriptions", label: qsTr("active_subscriptions"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.content_topics", label: qsTr("content_topics"), detail: qsTr("Content topics") },
                { key: "messaging.outbound_queue", label: qsTr("outbound_queue"), detail: qsTr("Outbound queue") },
                { key: "messaging.message_sent_events_recent", label: qsTr("message_sent_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_propagated_events_recent", label: qsTr("message_propagated_events_recent"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.message_received_events_recent", label: qsTr("waku_node_messages_total"), detail: qsTr("Delivery message counter total") },
                { key: "messaging.message_error_events_recent", label: qsTr("waku_node_errors_total"), detail: qsTr("Delivery error counter total") },
                { key: "messaging.publish_latency_ms", label: qsTr("publish_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") },
                { key: "messaging.receive_latency_ms", label: qsTr("receive_latency_ms"), detail: qsTr("Not exposed by current Delivery metrics") }
            ] }
        ]
    }

    component NetworkConnectionPanel: Panel {
        id: panelRoot

        property string kind: ""
        property string subtitle: ""
        property string connectionType: ""
        property string endpointLabel: qsTr("URL")
        property string endpoint: ""
        property string moduleName: ""
        property bool primaryFieldVisible: true
        property bool moduleFieldVisible: false
        property bool auxiliaryFieldVisible: false
        property string auxiliaryLabel: ""
        property string auxiliaryText: ""
        property string auxiliaryPlaceholder: ""
        property int refreshRate: 30
        property string statusText: qsTr("Unknown")
        property string statusDetail: ""
        property color statusColor: theme.textMuted
        signal endpointEdited(string value)
        signal auxiliaryEdited(string value)
        signal refreshRateEdited(int value)
        signal queryClicked()

        RowLayout {
            spacing: panelRoot.theme.gap
            Layout.fillWidth: true

            Text {
                text: panelRoot.subtitle
                color: panelRoot.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: panelRoot.theme.secondaryText
                Layout.fillWidth: true
            }

            StatusPill {
                theme: panelRoot.theme
                text: panelRoot.statusText
                colorToken: panelRoot.statusColor
            }
        }

        GridLayout {
            columns: settingsRoot.width < 760 ? 1 : 2
            columnSpacing: panelRoot.theme.gap
            rowSpacing: panelRoot.theme.gap
            Layout.fillWidth: true

            InfoField {
                theme: panelRoot.theme
                label: qsTr("Connection")
                value: panelRoot.connectionType
            }

            RefreshRateField {
                theme: panelRoot.theme
                value: panelRoot.refreshRate
                onRateEdited: value => panelRoot.refreshRateEdited(value)
            }

            FieldRow {
                visible: panelRoot.primaryFieldVisible
                theme: panelRoot.theme
                label: panelRoot.endpointLabel
                text: panelRoot.endpoint
                placeholderText: qsTr("Endpoint URL")
                onTextChanged: panelRoot.endpointEdited(text)
            }

            InfoField {
                visible: panelRoot.moduleFieldVisible
                theme: panelRoot.theme
                label: qsTr("Module bridge")
                value: panelRoot.moduleName
            }

            FieldRow {
                visible: panelRoot.auxiliaryFieldVisible
                theme: panelRoot.theme
                label: panelRoot.auxiliaryLabel
                text: panelRoot.auxiliaryText
                placeholderText: panelRoot.auxiliaryPlaceholder
                onTextChanged: panelRoot.auxiliaryEdited(text)
            }
        }

        RowLayout {
            spacing: panelRoot.theme.gapSmall
            Layout.fillWidth: true

            ActionButton {
                theme: panelRoot.theme
                text: qsTr("Query status")
                primary: true
                enabled: !settingsRoot.model.busy
                Layout.preferredWidth: 132
                accessibleName: qsTr("Query %1 status").arg(panelRoot.title)
                onClicked: panelRoot.queryClicked()
            }

            Text {
                text: panelRoot.statusDetail
                color: panelRoot.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: panelRoot.theme.dataText
                Layout.fillWidth: true
            }
        }
    }

    component DeliveryConnectionPanel: Panel {
        id: deliveryRoot

        property string subtitle: ""
        property string statusText: qsTr("Unknown")
        property string statusDetail: ""
        property color statusColor: theme.textMuted
        property ListModel sourceOptions
        signal queryClicked()

        RowLayout {
            spacing: deliveryRoot.theme.gap
            Layout.fillWidth: true

            Text {
                text: deliveryRoot.subtitle
                color: deliveryRoot.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: deliveryRoot.theme.secondaryText
                Layout.fillWidth: true
            }

            StatusPill {
                theme: deliveryRoot.theme
                text: deliveryRoot.statusText
                colorToken: deliveryRoot.statusColor
            }
        }

        GridLayout {
            columns: settingsRoot.width < 760 ? 1 : 2
            columnSpacing: deliveryRoot.theme.gap
            rowSpacing: deliveryRoot.theme.gap
            Layout.fillWidth: true

            ComboField {
                theme: deliveryRoot.theme
                label: qsTr("Source mode")
                accessibleName: qsTr("Delivery source mode")
                options: deliveryRoot.sourceOptions
                currentIndex: settingsRoot.deliverySourceIndexFor(settingsRoot.model.messagingSourceMode)
                onActivated: index => settingsRoot.model.messagingSourceMode = settingsRoot.deliverySourceModeAt(index)
            }

            InfoField {
                theme: deliveryRoot.theme
                label: qsTr("Module API")
                value: settingsRoot.model.deliveryModule
            }

            FieldRow {
                theme: deliveryRoot.theme
                label: qsTr("Waku REST URL")
                text: settingsRoot.model.messagingRestUrl
                placeholderText: qsTr("http://127.0.0.1:8645")
                onTextChanged: settingsRoot.model.messagingRestUrl = String(text || "").trim()
            }

            FieldRow {
                theme: deliveryRoot.theme
                label: qsTr("Metrics URL")
                text: settingsRoot.model.messagingMetricsUrl
                placeholderText: qsTr("http://127.0.0.1:8008/metrics")
                onTextChanged: settingsRoot.model.messagingMetricsUrl = String(text || "").trim()
            }

            FieldRow {
                theme: deliveryRoot.theme
                label: qsTr("Network preset")
                text: settingsRoot.model.messagingNetworkPreset
                placeholderText: qsTr("logos.test")
                onTextChanged: settingsRoot.model.messagingNetworkPreset = settingsRoot.model.normalizedMessagingNetworkPreset(text)
            }

            FieldRow {
                theme: deliveryRoot.theme
                label: qsTr("Node info id")
                text: settingsRoot.model.messagingNodeInfoId
                placeholderText: qsTr("Optional getNodeInfo id")
                onTextChanged: settingsRoot.model.messagingNodeInfoId = String(text || "").trim()
            }

            RefreshRateField {
                theme: deliveryRoot.theme
                value: settingsRoot.model.messagingRefreshRate
                onRateEdited: value => settingsRoot.model.setNetworkConnectionRate("messaging", value)
            }

            SecondsField {
                theme: deliveryRoot.theme
                label: qsTr("Rolling window")
                value: settingsRoot.model.messagingRollingWindow
                onValueEdited: value => settingsRoot.model.messagingRollingWindow = value
            }
        }

        Flow {
            spacing: deliveryRoot.theme.gapSmall
            Layout.fillWidth: true

            SafetyToggle {
                theme: deliveryRoot.theme
                text: qsTr("Admin REST")
                detail: qsTr("Allows privileged read-only admin endpoints when a future adapter uses them.")
                checked: settingsRoot.model.messagingAdminRestEnabled
                onToggled: settingsRoot.model.messagingAdminRestEnabled = checked
            }

            SafetyToggle {
                theme: deliveryRoot.theme
                text: qsTr("Mutating diagnostics")
                detail: qsTr("Allows future publish, subscribe, dial, and lightpush probes after per-action confirmation.")
                checked: settingsRoot.model.messagingMutatingDiagnosticsEnabled
                onToggled: settingsRoot.model.messagingMutatingDiagnosticsEnabled = checked
            }
        }

        StatusMessage {
            visible: settingsRoot.model.messagingSourceMode === "network-monitor" || settingsRoot.model.messagingSourceMode === "discovery-crawler"
            theme: deliveryRoot.theme
            tone: "warning"
            title: qsTr("Adapter pending")
            message: qsTr("This source profile is saved for layout and future wiring. Query status reports it as unavailable until the backend adapter exists.")
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: deliveryRoot.theme.gapSmall
            Layout.fillWidth: true

            ActionButton {
                theme: deliveryRoot.theme
                text: qsTr("Query status")
                primary: true
                enabled: !settingsRoot.model.busy
                Layout.preferredWidth: 132
                accessibleName: qsTr("Query Delivery status")
                onClicked: deliveryRoot.queryClicked()
            }

            Text {
                text: deliveryRoot.statusDetail
                color: deliveryRoot.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: deliveryRoot.theme.dataText
                Layout.fillWidth: true
            }
        }
    }

    component StorageConnectionPanel: Panel {
        id: storageRoot

        property string subtitle: ""
        property string statusText: qsTr("Unknown")
        property string statusDetail: ""
        property color statusColor: theme.textMuted
        property ListModel sourceOptions
        signal queryClicked()

        RowLayout {
            spacing: storageRoot.theme.gap
            Layout.fillWidth: true

            Text {
                text: storageRoot.subtitle
                color: storageRoot.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: storageRoot.theme.secondaryText
                Layout.fillWidth: true
            }

            StatusPill {
                theme: storageRoot.theme
                text: storageRoot.statusText
                colorToken: storageRoot.statusColor
            }
        }

        GridLayout {
            columns: settingsRoot.width < 760 ? 1 : 2
            columnSpacing: storageRoot.theme.gap
            rowSpacing: storageRoot.theme.gap
            Layout.fillWidth: true

            ComboField {
                theme: storageRoot.theme
                label: qsTr("Source mode")
                accessibleName: qsTr("Storage source mode")
                options: storageRoot.sourceOptions
                currentIndex: settingsRoot.storageSourceIndexFor(settingsRoot.model.storageSourceMode)
                onActivated: index => settingsRoot.model.storageSourceMode = settingsRoot.storageSourceModeAt(index)
            }

            InfoField {
                theme: storageRoot.theme
                label: qsTr("Module API")
                value: settingsRoot.model.storageModule
            }

            FieldRow {
                theme: storageRoot.theme
                label: qsTr("REST URL")
                text: settingsRoot.model.storageRestUrl
                placeholderText: qsTr("http://127.0.0.1:8080/api/storage/v1")
                onTextChanged: settingsRoot.model.storageRestUrl = String(text || "").trim()
            }

            FieldRow {
                theme: storageRoot.theme
                label: qsTr("Metrics URL")
                text: settingsRoot.model.storageMetricsUrl
                placeholderText: qsTr("http://127.0.0.1:8008/metrics")
                onTextChanged: settingsRoot.model.storageMetricsUrl = String(text || "").trim()
            }

            FieldRow {
                theme: storageRoot.theme
                label: qsTr("Network preset")
                text: settingsRoot.model.storageNetworkPreset
                placeholderText: qsTr("logos.test")
                onTextChanged: settingsRoot.model.storageNetworkPreset = String(text || "").trim()
            }

            FieldRow {
                theme: storageRoot.theme
                label: qsTr("Data directory")
                text: settingsRoot.model.storageDataDir
                placeholderText: qsTr("Optional local diagnostics path")
                onTextChanged: settingsRoot.model.storageDataDir = String(text || "").trim()
            }

            FieldRow {
                theme: storageRoot.theme
                label: qsTr("CID local exists")
                text: settingsRoot.model.storageCidProbe
                placeholderText: qsTr("Optional CID")
                onTextChanged: settingsRoot.model.storageCidProbe = String(text || "").trim()
            }

            RefreshRateField {
                theme: storageRoot.theme
                value: settingsRoot.model.storageRefreshRate
                onRateEdited: value => settingsRoot.model.setNetworkConnectionRate("storage", value)
            }

            SecondsField {
                theme: storageRoot.theme
                label: qsTr("Rolling window")
                value: settingsRoot.model.storageRollingWindow
                onValueEdited: value => settingsRoot.model.storageRollingWindow = value
            }
        }

        Flow {
            spacing: storageRoot.theme.gapSmall
            Layout.fillWidth: true

            SafetyToggle {
                theme: storageRoot.theme
                text: qsTr("Local OS diagnostics")
                detail: qsTr("Allows future process, disk, and port checks from the local machine.")
                checked: settingsRoot.model.storageLocalDiagnosticsEnabled
                onToggled: settingsRoot.model.storageLocalDiagnosticsEnabled = checked
            }

            SafetyToggle {
                theme: storageRoot.theme
                text: qsTr("Privileged debug")
                detail: qsTr("Allows future privileged debug endpoints after source-specific confirmation.")
                checked: settingsRoot.model.storagePrivilegedDebugEnabled
                onToggled: settingsRoot.model.storagePrivilegedDebugEnabled = checked
            }

            SafetyToggle {
                theme: storageRoot.theme
                text: qsTr("Mutating diagnostics")
                detail: qsTr("Allows future upload, download, connect, remove, and lifecycle probes after per-action confirmation.")
                checked: settingsRoot.model.storageMutatingDiagnosticsEnabled
                onToggled: settingsRoot.model.storageMutatingDiagnosticsEnabled = checked
            }
        }

        StatusMessage {
            visible: settingsRoot.model.storageSourceMode === "c-library" || settingsRoot.model.storageSourceMode === "local-os"
            theme: storageRoot.theme
            tone: "warning"
            title: qsTr("Adapter pending")
            message: qsTr("This source profile is saved for layout and future wiring. Query status reports it as unavailable until the backend adapter exists.")
            Layout.fillWidth: true
        }

        RowLayout {
            spacing: storageRoot.theme.gapSmall
            Layout.fillWidth: true

            ActionButton {
                theme: storageRoot.theme
                text: qsTr("Query status")
                primary: true
                enabled: !settingsRoot.model.busy
                Layout.preferredWidth: 132
                accessibleName: qsTr("Query Storage status")
                onClicked: storageRoot.queryClicked()
            }

            Text {
                text: storageRoot.statusDetail
                color: storageRoot.theme.textMuted
                textFormat: Text.PlainText
                wrapMode: Text.Wrap
                font.pixelSize: storageRoot.theme.dataText
                Layout.fillWidth: true
            }
        }
    }

    component FieldSelector: Panel {
        id: selectorRoot

        property string description: ""
        property var groups: []
        property string mode: "footer"

        Text {
            text: selectorRoot.description
            color: selectorRoot.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: selectorRoot.theme.secondaryText
            Layout.fillWidth: true
        }

        Repeater {
            model: selectorRoot.groups

            ColumnLayout {
                id: fieldGroupRoot

                required property var modelData

                spacing: selectorRoot.theme.gapSmall
                Layout.fillWidth: true

                Text {
                    text: String(fieldGroupRoot.modelData.title || "")
                    color: selectorRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: selectorRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    Layout.fillWidth: true
                }

                Flow {
                    spacing: selectorRoot.theme.gapSmall
                    Layout.fillWidth: true

                    Repeater {
                        model: fieldGroupRoot.modelData.fields || []

                        FieldToggle {
                            required property var modelData

                            theme: selectorRoot.theme
                            fieldKey: String(modelData.key || "")
                            label: String(modelData.label || "")
                            detail: String(modelData.detail || "")
                            checked: selectorRoot.mode === "dashboard"
                                ? settingsRoot.model.dashboardGraphEnabled(fieldKey)
                                : settingsRoot.model.footerFieldEnabled(fieldKey)
                            onToggled: {
                                if (selectorRoot.mode === "dashboard") {
                                    settingsRoot.model.setDashboardGraphEnabled(fieldKey, checked)
                                } else {
                                    settingsRoot.model.setFooterFieldEnabled(fieldKey, checked)
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    component FieldToggle: CheckBox {
        id: toggleRoot

        required property Theme theme
        property string fieldKey: ""
        property string label: ""
        property string detail: ""

        text: toggleRoot.label
        hoverEnabled: true
        implicitWidth: 236
        implicitHeight: 34

        indicator: Rectangle {
            x: 0
            y: (toggleRoot.height - height) / 2
            width: 18
            height: 18
            radius: 4
            color: toggleRoot.checked ? toggleRoot.theme.accent : toggleRoot.theme.field
            border.width: toggleRoot.activeFocus ? 2 : 1
            border.color: toggleRoot.checked ? toggleRoot.theme.accentHover : toggleRoot.theme.outline

            Rectangle {
                visible: toggleRoot.checked
                anchors.centerIn: parent
                width: 8
                height: 8
                radius: 2
                color: toggleRoot.theme.selectedText
            }
        }

        contentItem: Text {
            text: toggleRoot.text
            color: toggleRoot.enabled ? toggleRoot.theme.text : toggleRoot.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: toggleRoot.theme.dataText
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            leftPadding: 26
        }

        ToolTip.visible: hovered && toggleRoot.detail.length > 0
        ToolTip.text: toggleRoot.detail
        Accessible.role: Accessible.CheckBox
        Accessible.name: toggleRoot.label
    }

    component RefreshRateField: ColumnLayout {
        id: rateRoot

        required property Theme theme
        property int value: 30
        signal rateEdited(int value)

        spacing: 6
        Layout.fillWidth: true

        Text {
            text: qsTr("Auto refresh")
            color: rateRoot.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: rateRoot.theme.secondaryText
            font.weight: Font.Medium
            Layout.fillWidth: true
        }

        SpinBox {
            id: refreshSpin

            from: 0
            to: 3600
            stepSize: 5
            value: rateRoot.value
            editable: true
            hoverEnabled: true
            Layout.fillWidth: true
            Layout.preferredHeight: rateRoot.theme.controlHeight
            textFromValue: function (value, locale) {
                return value === 0 ? qsTr("Off") : qsTr("%1 s").arg(Number(value).toLocaleString(locale, "f", 0))
            }
            valueFromText: function (text, locale) {
                const parsed = Number(String(text || "").replace(/[^0-9]/g, ""))
                return Number.isFinite(parsed) ? parsed : 0
            }
            onValueModified: rateRoot.rateEdited(value)

            contentItem: TextInput {
                text: refreshSpin.textFromValue(refreshSpin.value, refreshSpin.locale)
                color: rateRoot.theme.text
                selectionColor: rateRoot.theme.accent
                selectedTextColor: rateRoot.theme.selectedText
                font.pixelSize: rateRoot.theme.primaryText
                horizontalAlignment: Qt.AlignHCenter
                verticalAlignment: Qt.AlignVCenter
                readOnly: !refreshSpin.editable
                validator: refreshSpin.validator
                inputMethodHints: Qt.ImhDigitsOnly
            }

            background: Rectangle {
                radius: rateRoot.theme.radius
                color: refreshSpin.hovered || refreshSpin.activeFocus ? rateRoot.theme.surfaceRaised : rateRoot.theme.field
                border.width: refreshSpin.activeFocus ? 2 : 1
                border.color: refreshSpin.activeFocus ? rateRoot.theme.accent : rateRoot.theme.outlineMuted
            }
        }
    }

    component SecondsField: ColumnLayout {
        id: secondsRoot

        required property Theme theme
        property string label: ""
        property int value: 120
        signal valueEdited(int value)

        spacing: 6
        Layout.fillWidth: true

        Text {
            text: secondsRoot.label
            color: secondsRoot.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: secondsRoot.theme.secondaryText
            font.weight: Font.Medium
            Layout.fillWidth: true
        }

        SpinBox {
            id: secondsSpin

            from: 5
            to: 3600
            stepSize: 5
            value: secondsRoot.value
            editable: true
            hoverEnabled: true
            Layout.fillWidth: true
            Layout.preferredHeight: secondsRoot.theme.controlHeight
            textFromValue: function (value, locale) {
                return qsTr("%1 s").arg(Number(value).toLocaleString(locale, "f", 0))
            }
            valueFromText: function (text, locale) {
                const parsed = Number(String(text || "").replace(/[^0-9]/g, ""))
                return Number.isFinite(parsed) ? parsed : secondsRoot.value
            }
            onValueModified: secondsRoot.valueEdited(value)

            contentItem: TextInput {
                text: secondsSpin.textFromValue(secondsSpin.value, secondsSpin.locale)
                color: secondsRoot.theme.text
                selectionColor: secondsRoot.theme.accent
                selectedTextColor: secondsRoot.theme.selectedText
                font.pixelSize: secondsRoot.theme.primaryText
                horizontalAlignment: Qt.AlignHCenter
                verticalAlignment: Qt.AlignVCenter
                readOnly: !secondsSpin.editable
                validator: secondsSpin.validator
                inputMethodHints: Qt.ImhDigitsOnly
            }

            background: Rectangle {
                radius: secondsRoot.theme.radius
                color: secondsSpin.hovered || secondsSpin.activeFocus ? secondsRoot.theme.surfaceRaised : secondsRoot.theme.field
                border.width: secondsSpin.activeFocus ? 2 : 1
                border.color: secondsSpin.activeFocus ? secondsRoot.theme.accent : secondsRoot.theme.outlineMuted
            }
        }
    }

    component InfoField: ColumnLayout {
        id: infoRoot

        required property Theme theme
        property string label: ""
        property string value: ""

        spacing: 6
        Layout.fillWidth: true

        Text {
            text: infoRoot.label
            color: infoRoot.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: infoRoot.theme.secondaryText
            font.weight: Font.Medium
            Layout.fillWidth: true
        }

        Rectangle {
            color: infoRoot.theme.field
            radius: infoRoot.theme.radius
            border.width: 1
            border.color: infoRoot.theme.outlineMuted
            Layout.fillWidth: true
            Layout.preferredHeight: infoRoot.theme.controlHeight

            Text {
                anchors.fill: parent
                anchors.leftMargin: 12
                anchors.rightMargin: 12
                text: infoRoot.value.length ? infoRoot.value : "-"
                color: infoRoot.theme.text
                textFormat: Text.PlainText
                elide: Text.ElideRight
                verticalAlignment: Text.AlignVCenter
                font.family: "monospace"
                font.pixelSize: infoRoot.theme.primaryText
            }
        }
    }

    component ProfileComboBox: ComboBox {
        id: comboRoot

        required property Theme theme
        property ListModel options
        property string accessibleName: qsTr("Network profile")
        signal profileActivated(int index)

        model: comboRoot.options
        textRole: "label"
        valueRole: "key"
        hoverEnabled: true
        implicitHeight: comboRoot.theme.controlHeight
        Accessible.role: Accessible.ComboBox
        Accessible.name: comboRoot.accessibleName
        onActivated: index => comboRoot.profileActivated(index)

        contentItem: Text {
            text: comboRoot.displayText
            color: comboRoot.enabled ? comboRoot.theme.text : comboRoot.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: comboRoot.theme.primaryText
            font.weight: Font.Medium
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            leftPadding: 12
            rightPadding: 36
        }

        indicator: Text {
            x: comboRoot.width - width - 14
            y: (comboRoot.height - height) / 2
            text: "v"
            color: comboRoot.enabled ? comboRoot.theme.textMuted : comboRoot.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: comboRoot.theme.secondaryText
            font.weight: Font.DemiBold
        }

        background: Rectangle {
            radius: comboRoot.theme.radius
            color: comboRoot.hovered || comboRoot.activeFocus ? comboRoot.theme.surfaceRaised : comboRoot.theme.field
            border.width: comboRoot.activeFocus ? 2 : 1
            border.color: comboRoot.activeFocus ? comboRoot.theme.accent : comboRoot.theme.outlineMuted
        }

        delegate: ItemDelegate {
            id: delegateRoot

            required property int index
            required property string label
            required property string summary

            width: comboRoot.width
            implicitHeight: 54
            hoverEnabled: true
            highlighted: comboRoot.highlightedIndex === index

            contentItem: ColumnLayout {
                spacing: comboRoot.theme.gapTiny

                Text {
                    text: delegateRoot.label
                    color: delegateRoot.highlighted ? comboRoot.theme.selectedText : comboRoot.theme.text
                    textFormat: Text.PlainText
                    font.pixelSize: comboRoot.theme.secondaryText
                    font.weight: Font.DemiBold
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }

                Text {
                    text: delegateRoot.summary
                    color: delegateRoot.highlighted ? comboRoot.theme.selectedText : comboRoot.theme.textMuted
                    textFormat: Text.PlainText
                    font.pixelSize: comboRoot.theme.dataText
                    elide: Text.ElideRight
                    Layout.fillWidth: true
                }
            }

            background: Rectangle {
                color: delegateRoot.highlighted ? comboRoot.theme.accent : (delegateRoot.hovered ? comboRoot.theme.hover : "transparent")
                radius: comboRoot.theme.radius
            }
        }

        popup: Popup {
            y: comboRoot.height + comboRoot.theme.gapTiny
            width: comboRoot.width
            implicitHeight: Math.min(contentItem.implicitHeight + 8, 296)
            padding: 4

            contentItem: ListView {
                clip: true
                implicitHeight: contentHeight
                model: comboRoot.popup.visible ? comboRoot.delegateModel : null
                currentIndex: comboRoot.highlightedIndex
            }

            background: Rectangle {
                color: comboRoot.theme.surfaceRaised
                radius: comboRoot.theme.radius
                border.width: 1
                border.color: comboRoot.theme.outline
            }
        }
    }

    component ComboField: ColumnLayout {
        id: comboFieldRoot

        required property Theme theme
        property string label: ""
        property string accessibleName: label
        property ListModel options
        property int currentIndex: 0
        signal activated(int index)

        spacing: 6
        Layout.fillWidth: true

        Text {
            text: comboFieldRoot.label
            color: comboFieldRoot.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: comboFieldRoot.theme.secondaryText
            font.weight: Font.Medium
            Layout.fillWidth: true
        }

        ProfileComboBox {
            theme: comboFieldRoot.theme
            options: comboFieldRoot.options
            accessibleName: comboFieldRoot.accessibleName
            currentIndex: comboFieldRoot.currentIndex
            Layout.fillWidth: true
            onProfileActivated: index => comboFieldRoot.activated(index)
        }
    }

    component SafetyToggle: CheckBox {
        id: safetyRoot

        required property Theme theme
        property string detail: ""

        hoverEnabled: true
        implicitWidth: 220
        implicitHeight: 34

        indicator: Rectangle {
            x: 0
            y: (safetyRoot.height - height) / 2
            width: 18
            height: 18
            radius: 4
            color: safetyRoot.checked ? safetyRoot.theme.accent : safetyRoot.theme.field
            border.width: safetyRoot.activeFocus ? 2 : 1
            border.color: safetyRoot.checked ? safetyRoot.theme.accentHover : safetyRoot.theme.outline

            Rectangle {
                visible: safetyRoot.checked
                anchors.centerIn: parent
                width: 8
                height: 8
                radius: 2
                color: safetyRoot.theme.selectedText
            }
        }

        contentItem: Text {
            text: safetyRoot.text
            color: safetyRoot.enabled ? safetyRoot.theme.text : safetyRoot.theme.textDim
            textFormat: Text.PlainText
            font.pixelSize: safetyRoot.theme.dataText
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
            leftPadding: 26
        }

        ToolTip.visible: hovered && safetyRoot.detail.length > 0
        ToolTip.text: safetyRoot.detail
        Accessible.role: Accessible.CheckBox
        Accessible.name: safetyRoot.text
    }

    component StatusPill: Rectangle {
        id: pillRoot

        required property Theme theme
        property string text: ""
        property color colorToken: theme.textMuted

        radius: pillRoot.theme.radius
        color: pillRoot.colorToken === pillRoot.theme.success ? pillRoot.theme.successMuted : (pillRoot.colorToken === pillRoot.theme.warning ? pillRoot.theme.warningMuted : pillRoot.theme.field)
        border.width: 1
        border.color: pillRoot.colorToken
        implicitWidth: pillText.implicitWidth + 18
        implicitHeight: 26

        Text {
            id: pillText

            anchors.centerIn: parent
            text: pillRoot.text.length ? pillRoot.text : qsTr("Unknown")
            color: pillRoot.colorToken === pillRoot.theme.textMuted ? pillRoot.theme.textMuted : pillRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: pillRoot.theme.dataText
            font.weight: Font.DemiBold
        }
    }
}
