pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

Pane {
    id: root

    required property Theme theme
    required property AppModel model

    readonly property bool compact: width < 900

    leftPadding: root.theme.gap
    rightPadding: root.theme.gap
    topPadding: 5
    bottomPadding: 5
    Layout.fillWidth: true
    Layout.preferredHeight: footerFlow.implicitHeight + topPadding + bottomPadding

    background: Rectangle {
        color: root.theme.sidebar
    }

    contentItem: Flow {
        id: footerFlow

        spacing: root.theme.gapSmall

        Repeater {
            model: root.footerGroups()

            SourceGroup {
                required property var modelData

                theme: root.theme
                compact: root.compact
                first: modelData.first === true
                items: modelData.items || []
            }
        }
    }

    function footerGroups() {
        const revision = root.model.footerFieldRevision
        const groups = root.footerSourceGroups()
        const rows = []
        for (let i = 0; i < groups.length; ++i) {
            const items = root.footerGroupItems(groups[i])
            if (items.length > 0) {
                rows.push({
                    first: rows.length === 0,
                    items: items
                })
            }
        }
        return rows
    }

    function footerGroupItems(group) {
        const rows = []
        const keys = group.keys || []
        const statusKey = String(group.statusKey || "")
        if (statusKey.length > 0 && root.footerGroupVisible(keys)) {
            rows.push(root.footerFieldItem(statusKey))
        }
        for (let i = 0; i < keys.length; ++i) {
            const key = keys[i]
            if (key !== statusKey && root.model.footerFieldEnabled(key)) {
                rows.push(root.footerFieldItem(key))
            }
        }
        return rows
    }

    function footerGroupVisible(keys) {
        for (let i = 0; i < keys.length; ++i) {
            if (root.model.footerFieldEnabled(keys[i])) {
                return true
            }
        }
        return false
    }

    function footerSourceGroups() {
        return [
            { statusKey: "network.network", keys: [
                "network.network",
                "network.chain_id",
                "network.zone_id",
                "network.channel_id",
                "network.report_time"
            ] },
            { statusKey: "bedrock.node_health", keys: [
                "bedrock.node_health",
                "bedrock.peer_count",
                "bedrock.sync_state",
                "bedrock.tip_height",
                "bedrock.tip_hash",
                "bedrock.lib_height",
                "bedrock.lib_hash",
                "bedrock.tip_minus_lib",
                "bedrock.last_tip_time",
                "bedrock.last_lib_time",
                "bedrock.finality_lag_seconds"
            ] },
            { statusKey: "lez.rpc_health", keys: [
                "lez.rpc_health",
                "lez.sequencer_version",
                "lez.last_lez_block_id",
                "lez.last_lez_block_hash",
                "lez.last_lez_block_time",
                "lez.pending_tx_count",
                "lez.mempool_tx_count",
                "lez.rejected_tx_count_recent",
                "lez.blocks_produced_recent",
                "lez.publish_to_bedrock_status",
                "lez.last_published_channel_update",
                "lez.last_finalized_callback_height",
                "lez.pending_blocks_count"
            ] },
            { statusKey: "indexer.rpc_health", keys: [
                "indexer.rpc_health",
                "indexer.indexer_version",
                "indexer.indexed_finalized_height",
                "indexer.indexed_finalized_hash",
                "indexer.indexed_channel_message",
                "indexer.indexer_lag_vs_sequencer_head",
                "indexer.last_indexed_time",
                "indexer.db_health",
                "indexer.ingestion_status"
            ] },
            { statusKey: "storage.module", keys: [
                "storage.module",
                "storage.network",
                "storage.node_reachable",
                "storage.nat_mode",
                "storage.udp_discovery_port",
                "storage.tcp_transfer_port",
                "storage.peer_count",
                "storage.dht_connected",
                "storage.shared_files_count",
                "storage.manifest_count",
                "storage.local_storage_used",
                "storage.active_uploads",
                "storage.active_downloads",
                "storage.failed_transfers_recent",
                "storage.cid_fetch_test",
                "storage.last_error"
            ] },
            { statusKey: "messaging.connection_state", keys: [
                "messaging.connection_state",
                "messaging.module",
                "messaging.peer_count",
                "messaging.bootstrap_connected",
                "messaging.active_subscriptions",
                "messaging.content_topics",
                "messaging.outbound_queue",
                "messaging.message_sent_events_recent",
                "messaging.message_propagated_events_recent",
                "messaging.message_received_events_recent",
                "messaging.message_error_events_recent",
                "messaging.publish_latency_ms",
                "messaging.receive_latency_ms",
                "messaging.last_error"
            ] },
            { statusKey: "overall.status", keys: [
                "overall.status",
                "overall.main_risk",
                "overall.operator_action"
            ] }
        ]
    }

    function footerFieldItem(key) {
        return {
            label: root.footerFieldLabel(key),
            fullName: root.footerFieldName(key),
            value: root.footerFieldValue(key),
            accessibleValue: root.footerFieldAccessibleValue(key),
            tone: root.footerFieldTone(key),
            maximumWidth: root.footerFieldWidth(key),
            priority: root.footerFieldPriority(key),
            valueVisible: !root.footerFieldUsesColorOnly(key)
        }
    }

    function footerFieldLabel(key) {
        const lookup = {
            "network.network": qsTr("Net"),
            "network.chain_id": qsTr("Chain"),
            "network.zone_id": qsTr("Zone"),
            "network.channel_id": qsTr("Chan"),
            "network.report_time": qsTr("Report"),
            "bedrock.node_health": qsTr("Bedrock"),
            "bedrock.peer_count": qsTr("Peers"),
            "bedrock.sync_state": qsTr("Sync"),
            "bedrock.tip_height": qsTr("TIP"),
            "bedrock.tip_hash": qsTr("Tip hash"),
            "bedrock.lib_height": qsTr("LIB"),
            "bedrock.lib_hash": qsTr("LIB hash"),
            "bedrock.tip_minus_lib": qsTr("Gap"),
            "bedrock.last_tip_time": qsTr("Tip time"),
            "bedrock.last_lib_time": qsTr("LIB time"),
            "bedrock.finality_lag_seconds": qsTr("Final lag"),
            "lez.rpc_health": qsTr("LEZ"),
            "lez.sequencer_version": qsTr("Seq ver"),
            "lez.last_lez_block_id": qsTr("LEZ block"),
            "lez.last_lez_block_hash": qsTr("LEZ hash"),
            "lez.last_lez_block_time": qsTr("LEZ time"),
            "lez.pending_tx_count": qsTr("Pending"),
            "lez.mempool_tx_count": qsTr("Mempool"),
            "lez.rejected_tx_count_recent": qsTr("Rejects"),
            "lez.blocks_produced_recent": qsTr("Blocks"),
            "lez.publish_to_bedrock_status": qsTr("Publish"),
            "lez.last_published_channel_update": qsTr("Channel"),
            "lez.last_finalized_callback_height": qsTr("Final"),
            "lez.pending_blocks_count": qsTr("Blk pend"),
            "indexer.rpc_health": qsTr("Indexer"),
            "indexer.indexer_version": qsTr("Idx ver"),
            "indexer.indexed_finalized_height": qsTr("Idx final"),
            "indexer.indexed_finalized_hash": qsTr("Idx hash"),
            "indexer.indexed_channel_message": qsTr("Idx chan"),
            "indexer.indexer_lag_vs_sequencer_head": qsTr("Idx lag"),
            "indexer.last_indexed_time": qsTr("Idx time"),
            "indexer.db_health": qsTr("DB"),
            "indexer.ingestion_status": qsTr("Ingest"),
            "storage.module": qsTr("Storage"),
            "storage.network": qsTr("ST net"),
            "storage.node_reachable": qsTr("ST node"),
            "storage.nat_mode": qsTr("NAT"),
            "storage.udp_discovery_port": qsTr("UDP"),
            "storage.tcp_transfer_port": qsTr("TCP"),
            "storage.peer_count": qsTr("ST peers"),
            "storage.dht_connected": qsTr("DHT"),
            "storage.shared_files_count": qsTr("Files"),
            "storage.manifest_count": qsTr("Manifests"),
            "storage.local_storage_used": qsTr("Storage"),
            "storage.active_uploads": qsTr("Uploads"),
            "storage.active_downloads": qsTr("Downloads"),
            "storage.failed_transfers_recent": qsTr("Failures"),
            "storage.cid_fetch_test": qsTr("CID"),
            "storage.last_error": qsTr("ST error"),
            "messaging.module": qsTr("Delivery"),
            "messaging.connection_state": qsTr("Conn"),
            "messaging.peer_count": qsTr("MSG peers"),
            "messaging.bootstrap_connected": qsTr("Bootstrap"),
            "messaging.active_subscriptions": qsTr("Subs"),
            "messaging.content_topics": qsTr("Topics"),
            "messaging.outbound_queue": qsTr("Queue"),
            "messaging.message_sent_events_recent": qsTr("Sent"),
            "messaging.message_propagated_events_recent": qsTr("Prop"),
            "messaging.message_received_events_recent": qsTr("Recv"),
            "messaging.message_error_events_recent": qsTr("Msg err"),
            "messaging.publish_latency_ms": qsTr("Pub ms"),
            "messaging.receive_latency_ms": qsTr("Recv ms"),
            "messaging.last_error": qsTr("Msg error"),
            "overall.status": qsTr("Overall"),
            "overall.main_risk": qsTr("Risk"),
            "overall.operator_action": qsTr("Action")
        }
        return lookup[key] || key
    }

    function footerFieldName(key) {
        return root.footerFieldLabel(key)
    }

    function footerFieldValue(key) {
        switch (key) {
        case "network.network":
            return root.networkLabel()
        case "network.chain_id":
            return root.valueOrNa(root.networkValue("chain_id"))
        case "network.zone_id":
            return root.valueOrNa(root.probeValue("sequencer", "zone_id"))
        case "network.channel_id":
            return root.valueOrNa(root.probeValue("sequencer", "channel_id"))
        case "network.report_time":
            return new Date().toLocaleTimeString(Qt.locale(), "hh:mm")
        case "bedrock.node_health":
            return root.healthDisplayText("node", "consensus")
        case "bedrock.peer_count":
            return root.numberText(root.networkValue("n_peers"))
        case "bedrock.sync_state":
            return root.bedrockSyncState()
        case "bedrock.tip_height":
            return root.numberText(root.cryptarchiaValue("slot"))
        case "bedrock.tip_hash":
            return root.shortHash(root.cryptarchiaValue("tip_hash") || root.cryptarchiaValue("hash"))
        case "bedrock.lib_height":
            return root.numberText(root.cryptarchiaValue("lib_slot"))
        case "bedrock.lib_hash":
            return root.shortHash(root.cryptarchiaValue("lib_hash"))
        case "bedrock.tip_minus_lib":
            return root.numberText(root.tipMinusLib())
        case "bedrock.last_tip_time":
        case "bedrock.last_lib_time":
            return qsTr("n/a")
        case "bedrock.finality_lag_seconds":
            return root.valueOrNa(root.finalityLagSeconds())
        case "lez.rpc_health":
            return root.healthDisplayText("sequencer", "health")
        case "lez.last_lez_block_id":
            return root.numberText(root.lezBlockHeight())
        case "lez.sequencer_version":
            return qsTr("n/a")
        case "lez.last_lez_block_hash":
            return root.shortHash(root.latestBlockValue("header_hash"))
        case "lez.last_lez_block_time":
            return root.timeText(root.latestBlockValue("timestamp"))
        case "lez.pending_tx_count":
        case "lez.mempool_tx_count":
        case "lez.rejected_tx_count_recent":
        case "lez.blocks_produced_recent":
        case "lez.pending_blocks_count":
            return root.valueOrNa(root.model.dashboardMetricValue(key))
        case "lez.publish_to_bedrock_status":
            return root.valueOrNa(root.latestBlockValue("bedrock_status"))
        case "lez.last_published_channel_update":
            return qsTr("n/a")
        case "lez.last_finalized_callback_height":
            return root.valueOrNa(root.model.indexerHeadValue())
        case "indexer.rpc_health":
            return root.indexerDisplayStatus()
        case "indexer.indexer_version":
            return qsTr("n/a")
        case "indexer.indexed_finalized_height":
            return root.valueOrNa(root.model.indexerHeadValue())
        case "indexer.indexed_finalized_hash":
            return root.shortHash(root.latestBlockValue("header_hash"))
        case "indexer.indexed_channel_message":
            return qsTr("n/a")
        case "indexer.indexer_lag_vs_sequencer_head":
            return root.valueOrNa(root.indexerLag())
        case "indexer.last_indexed_time":
            return root.timeText(root.latestBlockValue("timestamp"))
        case "indexer.db_health":
            return root.indexerDisplayStatus()
        case "indexer.ingestion_status":
            return root.indexerStatus()
        case "storage.module":
            return root.moduleDisplayStatus("storage")
        case "storage.network":
            return root.networkLabel()
        case "storage.node_reachable":
            return root.connectionReachableStatus("storage")
        case "storage.nat_mode":
            return root.valueOrNa(root.model.openMetricValue("storage", ["storage_nat_mode", "nat_mode"]))
        case "storage.udp_discovery_port":
            return root.portStatus("storage", ["storage_udp_discovery_port_open", "udp_discovery_port_open"])
        case "storage.tcp_transfer_port":
            return root.portStatus("storage", ["storage_tcp_transfer_port_open", "tcp_transfer_port_open"])
        case "storage.peer_count":
        case "storage.shared_files_count":
        case "storage.manifest_count":
        case "storage.local_storage_used":
        case "storage.active_uploads":
        case "storage.active_downloads":
        case "storage.failed_transfers_recent":
            return root.valueOrNa(root.model.dashboardMetricValue(key))
        case "storage.dht_connected":
            return root.yesNo(root.model.openMetricValue("storage", ["storage_dht_connected", "dht_connected"]))
        case "storage.cid_fetch_test":
            return qsTr("n/a")
        case "storage.last_error":
            return root.valueOrNa(root.model.moduleLastError("storage"))
        case "messaging.module":
            return root.moduleDisplayStatus("messaging")
        case "messaging.connection_state":
            return root.connectionAccessibleStatus("messaging")
        case "messaging.peer_count":
        case "messaging.active_subscriptions":
        case "messaging.content_topics":
        case "messaging.outbound_queue":
        case "messaging.message_sent_events_recent":
        case "messaging.message_propagated_events_recent":
        case "messaging.message_received_events_recent":
        case "messaging.message_error_events_recent":
        case "messaging.publish_latency_ms":
        case "messaging.receive_latency_ms":
            return root.valueOrNa(root.model.dashboardMetricValue(key))
        case "messaging.bootstrap_connected":
            return root.valueOrNa(root.model.moduleProbeValue("messaging", "getAvailableNodeInfoIDs") ? qsTr("yes") : null)
        case "messaging.last_error":
            return root.valueOrNa(root.model.moduleLastError("messaging"))
        case "overall.status":
            return root.overallStatusDisplay()
        case "overall.main_risk":
            return root.mainRisk()
        case "overall.operator_action":
            return root.operatorAction()
        default:
            return qsTr("n/a")
        }
    }

    function footerFieldAccessibleValue(key) {
        const value = root.footerFieldValue(key)
        if (value.length > 0) {
            return value
        }
        if (key === "bedrock.node_health") {
            return root.healthAccessibleText("node", "consensus")
        }
        if (key === "lez.rpc_health") {
            return root.healthAccessibleText("sequencer", "health")
        }
        if (key === "indexer.rpc_health") {
            return root.indexerStatus()
        }
        if (key === "storage.module") {
            return root.moduleAccessibleStatus("storage")
        }
        if (key === "messaging.module") {
            return root.moduleAccessibleStatus("messaging")
        }
        if (key === "messaging.connection_state") {
            return root.connectionAccessibleStatus("messaging")
        }
        if (key === "overall.status") {
            return root.overallStatusText()
        }
        return value
    }

    function footerFieldTone(key) {
        if (key === "network.network" || key === "network.report_time") {
            return "info"
        }
        if (key === "bedrock.node_health") {
            return root.toneForProbe("node", "consensus")
        }
        if (key === "bedrock.peer_count") {
            return root.numberTone(root.networkValue("n_peers"))
        }
        if (key === "bedrock.sync_state") {
            return root.syncTone()
        }
        if (key === "lez.rpc_health") {
            return root.toneForProbe("sequencer", "health")
        }
        if (key === "lez.publish_to_bedrock_status") {
            return root.statusWordTone(root.footerFieldValue(key))
        }
        if (key === "indexer.rpc_health" || key === "indexer.db_health" || key === "indexer.ingestion_status") {
            return root.indexerStatusTone()
        }
        if (key === "storage.node_reachable" || key === "storage.dht_connected" || key === "storage.cid_fetch_test") {
            return root.booleanTone(root.footerFieldValue(key))
        }
        if (key === "storage.udp_discovery_port" || key === "storage.tcp_transfer_port") {
            return root.portTone(root.footerFieldValue(key))
        }
        if (key === "messaging.connection_state" || key === "messaging.bootstrap_connected") {
            return root.booleanTone(root.footerFieldValue(key))
        }
        if (key.indexOf("storage.") === 0) {
            return root.moduleTone("storage")
        }
        if (key.indexOf("messaging.") === 0) {
            return root.moduleTone("messaging")
        }
        if (key === "overall.status") {
            return root.overallTone()
        }
        if (key === "overall.main_risk" || key === "overall.operator_action") {
            return root.overallTone() === "success" ? "neutral" : root.overallTone()
        }
        return "neutral"
    }

    function footerFieldWidth(key) {
        if (key.indexOf("hash") >= 0 || key.indexOf("error") >= 0 || key === "overall.operator_action" || key === "overall.main_risk") {
            return 190
        }
        return 150
    }

    function footerFieldPriority(key) {
        return key.indexOf("_hash") >= 0 || key.indexOf("_time") >= 0 ? "low" : "normal"
    }

    function footerFieldUsesColorOnly(key) {
        const lookup = {
            "bedrock.node_health": true,
            "bedrock.sync_state": true,
            "lez.rpc_health": true,
            "lez.publish_to_bedrock_status": true,
            "indexer.rpc_health": true,
            "indexer.db_health": true,
            "indexer.ingestion_status": true,
            "storage.module": true,
            "storage.node_reachable": true,
            "storage.udp_discovery_port": true,
            "storage.tcp_transfer_port": true,
            "storage.dht_connected": true,
            "storage.cid_fetch_test": true,
            "messaging.module": true,
            "messaging.connection_state": true,
            "messaging.bootstrap_connected": true,
            "overall.status": true
        }
        return lookup[key] === true
    }

    function overview() {
        return root.model.dashboardOverview || {}
    }

    function nodeReport() {
        return root.model.dashboardNode || {}
    }

    function probe(section, field) {
        const target = root.overview()[section]
        return target ? target[field] : null
    }

    function probeValue(section, field) {
        const target = root.probe(section, field)
        return target && target.value !== undefined && target.value !== null ? root.model.scalarValue(target.value) : null
    }

    function probeOk(section, field) {
        const target = root.probe(section, field)
        return target && target.ok === true
    }

    function probeKnown(section, field) {
        return root.probe(section, field) !== null
    }

    function healthText(section, field) {
        if (!root.probeKnown(section, field)) {
            return qsTr("unknown")
        }
        return root.probeOk(section, field) ? qsTr("ok") : qsTr("error")
    }

    function healthDisplayText(section, field) {
        if (!root.probeKnown(section, field)) {
            return qsTr("unknown")
        }
        return root.probeOk(section, field) ? "" : qsTr("error")
    }

    function healthAccessibleText(section, field) {
        return root.healthText(section, field)
    }

    function toneForProbe(section, field) {
        if (!root.probeKnown(section, field)) {
            return "neutral"
        }
        return root.probeOk(section, field) ? "success" : "error"
    }

    function consensusValue() {
        const value = root.probeValue("node", "consensus")
        return value && typeof value === "object" ? value : {}
    }

    function cryptarchiaInfo() {
        const value = root.consensusValue().cryptarchia_info
        return value && typeof value === "object" ? value : {}
    }

    function cryptarchiaValue(key) {
        const value = root.cryptarchiaInfo()[key]
        return value === undefined || value === null ? null : root.model.scalarValue(value)
    }

    function reportValue(name) {
        const report = root.nodeReport()[name]
        return report && report.value ? report.value : {}
    }

    function networkValue(key) {
        const value = root.reportValue("network_info")[key]
        return value === undefined || value === null ? null : root.model.scalarValue(value)
    }

    function bedrockSyncState() {
        const value = root.consensusValue()
        if (typeof value.sync_state === "string") {
            return value.sync_state
        }
        if (typeof value.syncState === "string") {
            return value.syncState
        }
        const mode = value.mode
        if (typeof mode === "string") {
            return mode
        }
        if (mode && mode.Started) {
            return mode.Started
        }
        return qsTr("unknown")
    }

    function syncTone() {
        const value = String(root.bedrockSyncState() || "").toLowerCase()
        if (value === "unknown") {
            return "neutral"
        }
        if (value.indexOf("sync") >= 0 || value.indexOf("catch") >= 0 || value.indexOf("start") >= 0) {
            return "warning"
        }
        return "success"
    }

    function lezBlockHeight() {
        const blocks = root.model.dashboardBlocks || []
        if (blocks.length > 0) {
            const block = blocks[0] || {}
            if (block.block_id !== undefined && block.block_id !== null) {
                return block.block_id
            }
        }
        return root.probeValue("sequencer", "head")
    }

    function indexerStatus() {
        if (!root.probeKnown("indexer", "health")) {
            return qsTr("unknown")
        }
        if (!root.probeOk("indexer", "health")) {
            return qsTr("stalled")
        }
        const indexerHead = Number(root.probeValue("indexer", "head"))
        const sequencerHead = Number(root.probeValue("sequencer", "head"))
        if (Number.isFinite(indexerHead) && Number.isFinite(sequencerHead) && indexerHead < sequencerHead) {
            return qsTr("backfilling")
        }
        return qsTr("running")
    }

    function indexerStatusTone() {
        const value = root.indexerStatus()
        if (value === qsTr("running")) {
            return "success"
        }
        if (value === qsTr("backfilling")) {
            return "warning"
        }
        if (value === qsTr("stalled")) {
            return "error"
        }
        return "neutral"
    }

    function indexerDisplayStatus() {
        const value = root.indexerStatus()
        return value === qsTr("running") ? "" : value
    }

    function networkLabel() {
        const profile = String(root.model.networkProfile || "").toLowerCase()
        const sequencer = String(root.model.sequencerUrl || "").toLowerCase()
        if (profile.indexOf("local") >= 0 || sequencer.indexOf("127.0.0.1") >= 0 || sequencer.indexOf("localhost") >= 0) {
            return qsTr("local")
        }
        if (profile.indexOf("mainnet") >= 0 || sequencer.indexOf("mainnet") >= 0) {
            return qsTr("mainnet")
        }
        if (profile === "custom") {
            return qsTr("custom")
        }
        return qsTr("testnet")
    }

    function valueOrNa(value) {
        const scalar = root.model.scalarValue(value)
        if (scalar === undefined || scalar === null || scalar === "") {
            return qsTr("n/a")
        }
        return root.numberText(scalar)
    }

    function shortHash(value) {
        const text = String(value || "")
        if (!text.length) {
            return qsTr("n/a")
        }
        if (text.length <= 14) {
            return text
        }
        return text.slice(0, 8) + "..." + text.slice(-4)
    }

    function tipMinusLib() {
        return root.model.tipMinusLib()
    }

    function finalityLagSeconds() {
        return root.model.finalityLagSeconds()
    }

    function indexerLag() {
        return root.model.indexerLag()
    }

    function connectionStatus(kind) {
        return root.model.networkConnectionState(kind)
    }

    function moduleDisplayStatus(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return qsTr("unknown")
        }
        return status.ok ? "" : qsTr("stopped")
    }

    function moduleAccessibleStatus(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return qsTr("unknown")
        }
        return status.ok ? qsTr("running") : qsTr("stopped")
    }

    function connectionAccessibleStatus(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return qsTr("unknown")
        }
        return status.ok ? qsTr("connected") : qsTr("disconnected")
    }

    function connectionReachableStatus(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return qsTr("unknown")
        }
        return status.ok ? qsTr("yes") : qsTr("no")
    }

    function moduleTone(kind) {
        const status = root.connectionStatus(kind)
        if (!status.known) {
            return "neutral"
        }
        return status.ok ? "success" : "error"
    }

    function overallTone() {
        if (root.toneForProbe("node", "consensus") === "error"
                || root.toneForProbe("sequencer", "health") === "error"
                || root.indexerStatusTone() === "error"
                || root.moduleTone("storage") === "error"
                || root.moduleTone("messaging") === "error") {
            return "error"
        }
        if (root.toneForProbe("node", "consensus") === "warning"
                || root.toneForProbe("sequencer", "health") === "warning"
                || root.indexerStatusTone() === "warning"
                || root.moduleTone("storage") === "warning"
                || root.moduleTone("messaging") === "warning") {
            return "warning"
        }
        return "success"
    }

    function overallStatusText() {
        const tone = root.overallTone()
        if (tone === "success") {
            return qsTr("healthy")
        }
        if (tone === "error") {
            return qsTr("down")
        }
        return qsTr("degraded")
    }

    function overallStatusDisplay() {
        const tone = root.overallTone()
        if (tone === "success") {
            return ""
        }
        if (tone === "error") {
            return qsTr("down")
        }
        return qsTr("degraded")
    }

    function mainRisk() {
        if (root.toneForProbe("node", "consensus") === "error") {
            return qsTr("bedrock")
        }
        if (root.toneForProbe("sequencer", "health") === "error") {
            return qsTr("lez rpc")
        }
        if (root.indexerStatusTone() === "error" || root.indexerStatusTone() === "warning") {
            return qsTr("indexer")
        }
        if (root.moduleTone("storage") === "error") {
            return qsTr("storage")
        }
        if (root.moduleTone("messaging") === "error") {
            return qsTr("messaging")
        }
        return qsTr("none")
    }

    function operatorAction() {
        const risk = root.mainRisk()
        if (risk === qsTr("none")) {
            return qsTr("monitor")
        }
        if (risk === qsTr("indexer")) {
            return qsTr("check indexer")
        }
        if (risk === qsTr("bedrock")) {
            return qsTr("check node")
        }
        if (risk === qsTr("storage")) {
            return qsTr("check storage")
        }
        if (risk === qsTr("messaging")) {
            return qsTr("check messaging")
        }
        return qsTr("check rpc")
    }

    function latestBlockValue(key) {
        const blocks = root.model.dashboardBlocks || []
        if (!blocks.length) {
            return null
        }
        const block = blocks[0] || {}
        const value = block[key]
        return value === undefined || value === null ? null : value
    }

    function timeText(value) {
        const scalar = root.model.scalarValue(value)
        if (scalar === null) {
            return qsTr("n/a")
        }
        const number = Number(scalar)
        if (!Number.isFinite(number) || number <= 0) {
            return root.numberText(scalar)
        }
        const millis = number > 1000000000000 ? number : number * 1000
        return new Date(millis).toLocaleTimeString(Qt.locale(), "hh:mm")
    }

    function yesNo(value) {
        const scalar = root.model.scalarValue(value)
        if (scalar === null) {
            return qsTr("n/a")
        }
        if (typeof scalar === "boolean") {
            return scalar ? qsTr("yes") : qsTr("no")
        }
        const number = Number(scalar)
        if (Number.isFinite(number)) {
            return number > 0 ? qsTr("yes") : qsTr("no")
        }
        const text = String(scalar).toLowerCase()
        if (text === "true" || text === "yes" || text === "open" || text === "connected") {
            return qsTr("yes")
        }
        if (text === "false" || text === "no" || text === "blocked" || text === "disconnected") {
            return qsTr("no")
        }
        return String(scalar)
    }

    function portStatus(kind, metricNames) {
        const value = root.model.openMetricValue(kind, metricNames)
        if (value === null) {
            return qsTr("n/a")
        }
        return Number(value) > 0 ? qsTr("open") : qsTr("blocked")
    }

    function numberText(value) {
        const scalar = root.model.scalarValue(value)
        if (scalar === undefined || scalar === null || scalar === "") {
            return "-"
        }
        if (typeof scalar === "number") {
            return scalar.toLocaleString(Qt.locale(), "f", Number.isInteger(scalar) ? 0 : 2)
        }
        const number = Number(scalar)
        if (Number.isFinite(number) && String(scalar).match(/^[0-9]+$/)) {
            return number.toLocaleString(Qt.locale(), "f", 0)
        }
        return String(scalar)
    }

    function numberTone(value) {
        const number = Number(value)
        return Number.isFinite(number) && number > 0 ? "success" : "neutral"
    }

    function booleanTone(value) {
        const text = String(value || "").toLowerCase()
        if (text === String(qsTr("yes")).toLowerCase()
                || text === String(qsTr("open")).toLowerCase()
                || text === String(qsTr("connected")).toLowerCase()
                || text === String(qsTr("running")).toLowerCase()) {
            return "success"
        }
        if (text === String(qsTr("no")).toLowerCase()
                || text === String(qsTr("blocked")).toLowerCase()
                || text === String(qsTr("disconnected")).toLowerCase()
                || text === String(qsTr("stopped")).toLowerCase()) {
            return "error"
        }
        return "neutral"
    }

    function portTone(value) {
        const text = String(value || "").toLowerCase()
        if (text === String(qsTr("open")).toLowerCase()) {
            return "success"
        }
        if (text === String(qsTr("blocked")).toLowerCase()) {
            return "error"
        }
        return "neutral"
    }

    function statusWordTone(value) {
        const text = String(value || "").toLowerCase()
        if (!text.length || text === String(qsTr("n/a")).toLowerCase() || text === String(qsTr("unknown")).toLowerCase()) {
            return "neutral"
        }
        if (text.indexOf("fail") >= 0 || text.indexOf("error") >= 0 || text.indexOf("reject") >= 0 || text.indexOf("stalled") >= 0 || text.indexOf("down") >= 0) {
            return "error"
        }
        if (text.indexOf("pending") >= 0 || text.indexOf("sync") >= 0 || text.indexOf("backfill") >= 0 || text.indexOf("degraded") >= 0) {
            return "warning"
        }
        return "success"
    }

    component SourceGroup: RowLayout {
        id: groupRoot

        required property Theme theme
        property var items: []
        property bool first: false
        property bool compact: false

        spacing: groupRoot.theme.gapSmall

        Rectangle {
            visible: !groupRoot.first
            color: groupRoot.theme.outline
            radius: width / 2
            Layout.preferredWidth: 1
            Layout.preferredHeight: 14
            Layout.alignment: Qt.AlignVCenter
            Accessible.ignored: true
        }

        Repeater {
            model: groupRoot.items

            StatusToken {
                required property var modelData

                visible: !groupRoot.compact || String(modelData.priority || "normal") !== "low"
                theme: groupRoot.theme
                label: String(modelData.label || "")
                value: String(modelData.value || "")
                accessibleValue: String(modelData.accessibleValue || modelData.value || "-")
                tone: String(modelData.tone || "neutral")
                fullName: String(modelData.fullName || modelData.label || "")
                maximumTokenWidth: modelData.maximumWidth || 150
                valueVisible: modelData.valueVisible !== false
                Layout.alignment: Qt.AlignVCenter
            }
        }
    }

    component StatusToken: Control {
        id: token

        required property Theme theme
        property string label: ""
        property string value: "-"
        property string accessibleValue: value
        property string tone: "neutral"
        property string fullName: ""
        property int maximumTokenWidth: 140
        property bool valueVisible: true

        hoverEnabled: true
        padding: 0
        implicitWidth: Math.min(tokenRow.implicitWidth, maximumTokenWidth)
        implicitHeight: 22

        background: Item {}

        contentItem: RowLayout {
            id: tokenRow

            spacing: token.theme.gapTiny

            Rectangle {
                color: token.toneColor()
                radius: width / 2
                Layout.preferredWidth: 7
                Layout.preferredHeight: 7
                Layout.alignment: Qt.AlignVCenter
                Accessible.ignored: true
            }

            Text {
                text: token.label
                color: token.theme.textDim
                textFormat: Text.PlainText
                font.pixelSize: token.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                elide: Text.ElideRight
                Layout.maximumWidth: 74
            }

            Text {
                text: token.value
                visible: token.valueVisible && token.value.length > 0
                color: token.valueColor()
                textFormat: Text.PlainText
                font.pixelSize: token.theme.dataText
                font.family: "monospace"
                font.weight: Font.Medium
                elide: Text.ElideRight
                Layout.maximumWidth: Math.max(44, token.maximumTokenWidth - 84)
            }
        }

        ToolTip.visible: hovered && token.fullName.length > 0
        ToolTip.delay: 350
        ToolTip.text: qsTr("%1: %2").arg(token.fullName).arg(token.accessibleValue)

        Accessible.role: Accessible.StaticText
        Accessible.name: qsTr("%1: %2").arg(token.fullName.length > 0 ? token.fullName : token.label).arg(token.accessibleValue)

        function toneColor() {
            if (token.tone === "success") {
                return token.theme.success
            }
            if (token.tone === "warning") {
                return token.theme.warning
            }
            if (token.tone === "error") {
                return token.theme.error
            }
            if (token.tone === "info") {
                return token.theme.info
            }
            return token.theme.textDim
        }

        function valueColor() {
            if (token.tone === "error") {
                return token.theme.error
            }
            if (token.tone === "warning") {
                return token.theme.warning
            }
            if (token.tone === "success") {
                return token.theme.success
            }
            return token.theme.text
        }
    }
}
