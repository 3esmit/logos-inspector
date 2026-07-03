pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../components"
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        model.refreshDashboard();
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Dashboard")
        title: qsTr("Dashboard")
        layerLabel: qsTr("System")
        subtitle: qsTr("%1 profile across %2 network. Open blocks, transactions, transfer activity, channels, and accounts from live chain references.").arg(root.profileLabel(root.model.networkProfile)).arg(root.chainLabel())
        Layout.fillWidth: true
    }

    Panel {
        visible: root.selectedDashboardGraphItems().length > 0
        theme: root.theme
        title: qsTr("Graphs")

        GridLayout {
            columns: root.width < 760 ? 1 : (root.width < 1180 ? 2 : 3)
            columnSpacing: root.theme.gap
            rowSpacing: root.theme.gap
            Layout.fillWidth: true

            Repeater {
                model: root.selectedDashboardGraphItems()

                GraphTile {
                    required property var modelData

                    theme: root.theme
                    title: String(modelData.title || "")
                    group: String(modelData.group || "")
                    value: String(modelData.value || "-")
                    numericValue: Number(modelData.numericValue)
                    tone: String(modelData.tone || "neutral")
                    samples: modelData.samples || []
                    Layout.fillWidth: true
                }
            }
        }
    }

    GridLayout {
        columns: root.width < 860 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: root.theme.surface
                radius: root.theme.radius
                border.width: 1
                border.color: root.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                DashboardHeader {
                    theme: root.theme
                    title: qsTr("Latest L1 Blocks / Mantle")
                    action: qsTr("View all")
                    onActivated: root.model.selectView("blocks")
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("L1 slot"), qsTr("Header"), qsTr("Tx"), qsTr("Finality")]
                    columnWidths: [86, -1, 58, 86]
                }

                Repeater {
                    model: root.l1BlockRows()

                    DashboardRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.slot, modelData.header, modelData.tx, modelData.status]
                        columnWidths: [86, -1, 58, 86]
                        linkKinds: ["block", "block", "", ""]
                        linkValues: [modelData.slotRaw, modelData.blockHash, "", ""]
                        onCellActivated: function (column) {
                            if (column === 0 || column === 1) {
                                root.model.openReference("block", column === 0 ? modelData.slotRaw : modelData.blockHash)
                            }
                        }
                    }
                }
            }
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: root.theme.surface
                radius: root.theme.radius
                border.width: 1
                border.color: root.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                DashboardHeader {
                    theme: root.theme
                    title: qsTr("Latest L2 Blocks")
                    action: qsTr("View all")
                    onActivated: {
                        root.model.selectView("l2Blocks")
                    }
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("L2 block"), qsTr("Header"), qsTr("Tx"), qsTr("Bedrock")]
                    columnWidths: [86, -1, 58, 86]
                }

                Repeater {
                    model: root.l2BlockRows()

                    DashboardRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.height, modelData.header, modelData.tx, modelData.status]
                        columnWidths: [86, -1, 58, 86]
                        linkKinds: ["indexerBlock", "indexerBlock", "", ""]
                        linkValues: [modelData.blockHash, modelData.blockHash, "", ""]
                        onCellActivated: function (column) {
                            root.model.openReference(column === 0 || column === 1 ? "indexerBlock" : "", modelData.blockHash)
                        }
                    }
                }
            }
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: root.theme.surface
                radius: root.theme.radius
                border.width: 1
                border.color: root.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                DashboardHeader {
                    theme: root.theme
                    title: qsTr("Latest L2 Transactions")
                    action: qsTr("View all")
                    onActivated: {
                        root.model.selectView("l2Transactions")
                    }
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("L2 block"), qsTr("Tx hash"), qsTr("Header"), qsTr("Ops")]
                    columnWidths: [86, -1, -1, 58]
                }

                Repeater {
                    model: root.transactionRows()

                    DashboardRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.slot, modelData.hash, modelData.block, modelData.ops]
                        columnWidths: [86, -1, -1, 58]
                        linkKinds: ["", "transaction", "indexerBlock", ""]
                        linkValues: ["", modelData.txHash, modelData.blockHash, ""]
                        onCellActivated: function (column) {
                            if (column === 1) {
                                root.model.openReference("transaction", modelData.txHash)
                            } else if (column === 2) {
                                root.model.openReference("indexerBlock", modelData.blockHash)
                            }
                        }
                    }
                }
            }
        }
    }

    StatusMessage {
        visible: root.model.dashboardError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Dashboard refresh failed")
        message: root.model.dashboardError
        Layout.fillWidth: true
    }

    function overview() {
        return model.dashboardOverview || {};
    }

    function nodeReport() {
        return model.dashboardNode || {};
    }

    function chainLabel() {
        const profile = String(model.networkProfile || "").toLowerCase()
        const sequencer = String(model.sequencerUrl || "").toLowerCase()
        if (sequencer.indexOf("127.0.0.1") >= 0 || sequencer.indexOf("localhost") >= 0) {
            return qsTr("Local");
        }
        if (profile.indexOf("mainnet") >= 0 || sequencer.indexOf("mainnet") >= 0) {
            return qsTr("Mainnet")
        }
        if (sequencer.indexOf("testnet") >= 0 || sequencer.indexOf("lez.logos.co") >= 0) {
            return qsTr("Testnet")
        }
        if (profile === "custom") {
            return qsTr("Custom");
        }
        return qsTr("Testnet");
    }

    function profileLabel(value) {
        if (value === "local") {
            return qsTr("Local");
        }
        if (value === "custom") {
            return qsTr("Custom");
        }
        return qsTr("Testnet");
    }

    function consensusValue() {
        const nodeReport = root.nodeReport();
        const infoProbe = nodeReport ? nodeReport.cryptarchia_info : null;
        if (infoProbe && infoProbe.value) {
            return infoProbe.value;
        }
        const node = overview().node;
        const probe = node ? node.consensus : null;
        return probe && probe.value ? probe.value : {};
    }

    function cryptarchiaInfo() {
        return consensusValue().cryptarchia_info || {};
    }

    function cryptarchiaValue(key) {
        const value = cryptarchiaInfo()[key];
        return value === undefined || value === null ? null : root.model.scalarValue(value);
    }

    function networkInfo() {
        return root.reportValue("network_info");
    }

    function mantleInfo() {
        return root.reportValue("mantle_metrics");
    }

    function reportValue(key) {
        const report = nodeReport()[key];
        return report && report.value ? report.value : {};
    }

    function networkValue(key) {
        const value = networkInfo()[key];
        return value === undefined || value === null ? null : root.model.scalarValue(value);
    }

    function mantleValue(key) {
        const value = mantleInfo()[key];
        return value === undefined || value === null ? null : root.model.scalarValue(value);
    }

    function modeText() {
        const mode = consensusValue().mode;
        if (typeof mode === "string") {
            return mode;
        }
        if (mode && mode.Started) {
            return mode.Started;
        }
        return "-";
    }

    function libDeltaText() {
        const slot = cryptarchiaValue("slot");
        const lib = cryptarchiaValue("lib_slot");
        if (slot === null || lib === null) {
            return qsTr("Above LIB");
        }
        return qsTr("+%1 above LIB").arg(root.numberText(slot - lib));
    }

    function selectedDashboardGraphItems() {
        const revision = root.model.dashboardGraphRevision
        const keys = [
            "bedrock.peer_count",
            "bedrock.tip_minus_lib",
            "bedrock.finality_lag_seconds",
            "lez.pending_tx_count",
            "lez.mempool_tx_count",
            "lez.rejected_tx_count_recent",
            "lez.blocks_produced_recent",
            "lez.pending_blocks_count",
            "indexer.indexer_lag_vs_sequencer_head",
            "storage.peer_count",
            "storage.shared_files_count",
            "storage.manifest_count",
            "storage.local_storage_used",
            "storage.active_uploads",
            "storage.active_downloads",
            "storage.failed_transfers_total",
            "messaging.peer_count",
            "messaging.active_subscriptions",
            "messaging.content_topics",
            "messaging.outbound_queue",
            "messaging.message_sent_events_recent",
            "messaging.message_propagated_events_recent",
            "messaging.message_received_events_recent",
            "messaging.message_error_events_recent",
            "messaging.publish_latency_ms",
            "messaging.receive_latency_ms"
        ]
        const rows = []
        for (let i = 0; i < keys.length; ++i) {
            if (root.model.dashboardGraphEnabled(keys[i])) {
                rows.push(root.dashboardGraphItem(keys[i]))
            }
        }
        return rows
    }

    function dashboardGraphItem(key) {
        const raw = root.model.dashboardMetricValue(key)
        const numeric = Number(raw)
        return {
            key: key,
            title: root.dashboardMetricLabel(key),
            group: root.dashboardMetricGroup(key),
            value: root.dashboardMetricText(raw),
            numericValue: numeric,
            tone: root.dashboardMetricTone(key, numeric),
            samples: root.model.dashboardMetricSamples(key)
        }
    }

    function dashboardMetricTone(key, numeric) {
        if (!Number.isFinite(numeric)) {
            return "neutral"
        }
        if (key === "bedrock.peer_count" || key === "storage.peer_count" || key === "messaging.peer_count" || key === "lez.blocks_produced_recent") {
            return numeric > 0 ? "success" : "neutral"
        }
        if (key === "storage.failed_transfers_total") {
            return numeric > 0 ? "neutral" : "success"
        }
        if (key.indexOf("rejected_") >= 0 || key.indexOf("failed_") >= 0 || key.indexOf("_error_") >= 0) {
            return numeric > 0 ? "error" : "neutral"
        }
        if (key.indexOf("_lag") >= 0 || key.indexOf("_queue") >= 0 || key.indexOf("pending_") >= 0 || key.indexOf("mempool_") >= 0 || key === "bedrock.tip_minus_lib") {
            return numeric > 0 ? "warning" : "neutral"
        }
        return "neutral"
    }

    function dashboardMetricGroup(key) {
        if (key.indexOf("bedrock.") === 0) {
            return qsTr("Bedrock Blockchain")
        }
        if (key.indexOf("lez.") === 0) {
            return qsTr("LEZ Sequencer")
        }
        if (key.indexOf("indexer.") === 0) {
            return qsTr("Indexer")
        }
        if (key.indexOf("storage.") === 0) {
            return qsTr("Storage")
        }
        return qsTr("Messaging / Delivery")
    }

    function dashboardMetricLabel(key) {
        switch (String(key || "")) {
        case "storage.active_uploads":
            return qsTr("upload requests total")
        case "storage.active_downloads":
            return qsTr("download requests total")
        case "storage.failed_transfers_total":
            return qsTr("transfer failures total")
        }
        const parts = String(key || "").split(".")
        return parts.length > 1 ? parts[1].replace(/_/g, " ") : key
    }

    function dashboardMetricValue(key) {
        return root.model.dashboardMetricValue(key)
    }

    function dashboardMetricText(value) {
        if (value === undefined || value === null || value === "") {
            return qsTr("n/a")
        }
        return root.numberText(value)
    }

    function tipMinusLib() {
        const slot = Number(root.cryptarchiaValue("slot"))
        const lib = Number(root.cryptarchiaValue("lib_slot"))
        return Number.isFinite(slot) && Number.isFinite(lib) ? Math.max(0, slot - lib) : null
    }

    function finalityLagSeconds() {
        const gap = root.tipMinusLib()
        return gap === null ? null : gap * 2
    }

    function indexerLag() {
        const sequencerHead = Number(root.probeValue("sequencer", "head"))
        const indexerHead = Number(root.probeValue("indexer", "head"))
        return Number.isFinite(sequencerHead) && Number.isFinite(indexerHead) ? Math.max(0, sequencerHead - indexerHead) : null
    }

    function probe(section, field) {
        const target = root.overview()[section]
        return target ? target[field] || null : null
    }

    function probeValue(section, field) {
        const target = root.probe(section, field)
        return target && target.value !== undefined && target.value !== null ? root.model.scalarValue(target.value) : null
    }

    function numberText(value) {
        const scalar = root.model.scalarValue(value)
        if (scalar === undefined || scalar === null || scalar === "") {
            return "-";
        }
        if (typeof scalar === "number") {
            return scalar.toLocaleString(Qt.locale(), "f", Number.isInteger(scalar) ? 0 : 2);
        }
        return String(scalar);
    }

    function shortHash(value) {
        const text = String(value || "");
        if (text.length <= 16) {
            return text.length ? text : "-";
        }
        return text.slice(0, 8) + "..." + text.slice(-6);
    }

    function l1BlockRows() {
        const blocks = root.model.blocksPageRows || []
        if (blocks.length > 0 && root.blocksPageRowsAreCurrent()) {
            return blocks.slice(0, 5).map(function (block) {
                const header = block.header || {}
                const transactions = Array.isArray(block.transactions) ? block.transactions : []
                const hash = root.model.blockHash(block)
                return {
                    slot: root.numberText(header.slot),
                    slotRaw: String(header.slot || ""),
                    header: root.shortHash(hash),
                    tx: root.numberText(transactions.length),
                    status: root.model.blockStatus(block),
                    blockHash: hash
                }
            })
        }
        return [
            {
                slot: root.numberText(root.cryptarchiaValue("slot")),
                slotRaw: String(root.cryptarchiaValue("slot") || ""),
                header: root.shortHash(root.cryptarchiaValue("tip")),
                tx: "-",
                status: qsTr("Tip"),
                blockHash: String(root.cryptarchiaValue("tip") || "")
            },
            {
                slot: root.numberText(root.cryptarchiaValue("lib_slot")),
                slotRaw: String(root.cryptarchiaValue("lib_slot") || ""),
                header: root.shortHash(root.cryptarchiaValue("lib")),
                tx: "-",
                status: qsTr("LIB"),
                blockHash: String(root.cryptarchiaValue("lib") || "")
            }
        ]
    }

    function blocksPageRowsAreCurrent() {
        const libSlot = Number(root.cryptarchiaValue("lib_slot"))
        const slotTo = Number(root.model.blocksPageSlotTo)
        return Number.isFinite(libSlot) && libSlot > 0 && Number.isFinite(slotTo) && slotTo >= libSlot
    }

    function l2BlockRows() {
        const blocks = model.dashboardBlocks || [];
        if (blocks.length > 0) {
            return blocks.slice(0, 5).map(function (block) {
                return {
                    height: root.numberText(block.block_id),
                    header: root.shortHash(block.header_hash),
                    tx: root.numberText(block.tx_count),
                    status: block.bedrock_status || "-",
                    blockHash: String(block.header_hash || "")
                };
            });
        }
        return [
            {
                height: "-",
                header: qsTr("No indexed L2 blocks"),
                tx: "-",
                status: "-",
                blockHash: ""
            }
        ];
    }

    function transactionRows() {
        const rows = [];
        const blocks = model.dashboardBlocks || [];
        for (let i = 0; i < blocks.length && rows.length < 5; ++i) {
            const block = blocks[i];
            const transactions = block.transactions || [];
            for (let j = 0; j < transactions.length && rows.length < 5; ++j) {
                const tx = transactions[j];
                rows.push({
                    slot: root.numberText(block.block_id),
                    hash: root.shortHash(tx.hash),
                    block: root.shortHash(block.header_hash),
                    ops: root.numberText((tx.instruction_data || []).length),
                    txHash: String(tx.hash || ""),
                    blockHash: String(block.header_hash || "")
                });
            }
        }
        if (rows.length > 0) {
            return rows;
        }
        return [
            {
                slot: "-",
                hash: qsTr("No indexed transactions"),
                block: "-",
                ops: "-",
                txHash: "",
                blockHash: ""
            }
        ];
    }

    component GraphTile: Frame {
        id: graphRoot

        required property Theme theme
        property string title: ""
        property string group: ""
        property string value: "-"
        property real numericValue: NaN
        property string tone: "neutral"
        property var samples: []

        padding: graphRoot.theme.gap
        Layout.fillWidth: true

        background: Rectangle {
            color: graphRoot.theme.field
            radius: graphRoot.theme.radius
            border.width: 1
            border.color: graphRoot.theme.outlineMuted
        }

        contentItem: ColumnLayout {
            spacing: graphRoot.theme.gapSmall

            RowLayout {
                spacing: graphRoot.theme.gapSmall
                Layout.fillWidth: true

                ColumnLayout {
                    spacing: 2
                    Layout.fillWidth: true

                    Text {
                        text: graphRoot.title
                        color: graphRoot.theme.text
                        textFormat: Text.PlainText
                        font.pixelSize: graphRoot.theme.secondaryText
                        font.weight: Font.DemiBold
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }

                    Text {
                        text: graphRoot.group
                        color: graphRoot.theme.textMuted
                        textFormat: Text.PlainText
                        font.pixelSize: graphRoot.theme.dataText
                        elide: Text.ElideRight
                        Layout.fillWidth: true
                    }
                }

                Text {
                    text: graphRoot.value
                    color: graphRoot.toneColor()
                    textFormat: Text.PlainText
                    font.family: "monospace"
                    font.pixelSize: 18
                    font.weight: Font.DemiBold
                    Layout.alignment: Qt.AlignVCenter
                }
            }

            Canvas {
                id: lineChart

                Layout.fillWidth: true
                Layout.preferredHeight: 54
                property var samples: graphRoot.samples
                property string tone: graphRoot.tone
                antialiasing: true
                onSamplesChanged: requestPaint()
                onToneChanged: requestPaint()
                onWidthChanged: requestPaint()
                onHeightChanged: requestPaint()

                onPaint: {
                    const ctx = getContext("2d")
                    if (ctx.reset) {
                        ctx.reset()
                    }
                    ctx.clearRect(0, 0, width, height)
                    const inset = 5
                    const chartWidth = Math.max(1, width - inset * 2)
                    const chartHeight = Math.max(1, height - inset * 2)
                    ctx.lineWidth = 1
                    ctx.strokeStyle = graphRoot.theme.outlineMuted
                    ctx.beginPath()
                    ctx.moveTo(inset, height - inset)
                    ctx.lineTo(width - inset, height - inset)
                    ctx.stroke()

                    const raw = Array.isArray(graphRoot.samples) && graphRoot.samples.length > 0 ? graphRoot.samples : [{ timestamp: Date.now(), value: graphRoot.numericValue }]
                    const samples = []
                    for (let i = 0; i < raw.length; ++i) {
                        const sample = raw[i]
                        const value = Number(sample && typeof sample === "object" ? sample.value : sample)
                        if (Number.isFinite(value)) {
                            const timestamp = Number(sample && typeof sample === "object" ? sample.timestamp : i)
                            samples.push({
                                timestamp: Number.isFinite(timestamp) ? timestamp : i,
                                value: value
                            })
                        }
                    }
                    if (samples.length === 0) {
                        return
                    }

                    ctx.lineWidth = 2
                    ctx.lineJoin = "round"
                    ctx.lineCap = "round"
                    ctx.strokeStyle = graphRoot.toneColor()
                    if (samples.length === 1) {
                        const singleY = inset + chartHeight / 2
                        ctx.beginPath()
                        ctx.moveTo(inset, singleY)
                        ctx.lineTo(width - inset, singleY)
                        ctx.stroke()
                        ctx.fillStyle = ctx.strokeStyle
                        ctx.beginPath()
                        ctx.arc(width - inset, singleY, 3, 0, Math.PI * 2)
                        ctx.fill()
                        return
                    }

                    let min = samples[0].value
                    let max = samples[0].value
                    let minTimestamp = samples[0].timestamp
                    let maxTimestamp = samples[0].timestamp
                    for (let j = 1; j < samples.length; ++j) {
                        min = Math.min(min, samples[j].value)
                        max = Math.max(max, samples[j].value)
                        minTimestamp = Math.min(minTimestamp, samples[j].timestamp)
                        maxTimestamp = Math.max(maxTimestamp, samples[j].timestamp)
                    }
                    const span = Math.max(1, max - min)
                    const timeSpan = Math.max(1, maxTimestamp - minTimestamp)
                    ctx.beginPath()
                    for (let k = 0; k < samples.length; ++k) {
                        const x = inset + ((samples[k].timestamp - minTimestamp) / timeSpan * chartWidth)
                        const y = inset + chartHeight - ((samples[k].value - min) / span * chartHeight)
                        if (k === 0) {
                            ctx.moveTo(x, y)
                        } else {
                            ctx.lineTo(x, y)
                        }
                    }
                    ctx.stroke()
                }
            }
        }

        function toneColor() {
            if (graphRoot.tone === "success") {
                return graphRoot.theme.success
            }
            if (graphRoot.tone === "warning") {
                return graphRoot.theme.warning
            }
            if (graphRoot.tone === "error") {
                return graphRoot.theme.error
            }
            return graphRoot.theme.accent
        }
    }

    component DashboardHeader: Item {
        id: headerRoot

        required property Theme theme
        property string title: ""
        property string action: ""
        signal activated()

        Layout.fillWidth: true
        Layout.preferredHeight: 48

        RowLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            spacing: 10

            Text {
                text: headerRoot.title
                color: headerRoot.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 15
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            ActionButton {
                visible: headerRoot.action.length > 0
                theme: headerRoot.theme
                text: headerRoot.action
                Layout.preferredWidth: Math.max(96, headerRoot.action.length * 8 + 28)
                onClicked: headerRoot.activated()
            }
        }
    }

    component DashboardRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property var columnWidths: [-1, -1, -1, -1]
        property var linkKinds: ["", "", "", ""]
        property var linkValues: ["", "", "", ""]
        property bool header: false
        signal cellActivated(int column)

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 34 : 38

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : "transparent"
            border.width: 0
        }

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 4
            columnSpacing: 10

            Repeater {
                model: 4

                LinkCell {
                    required property int index

                    theme: rowRoot.theme
                    text: String(rowRoot.columns[index] || "-")
                    header: rowRoot.header
                    link: rowRoot.linkFor(index)
                    copyText: rowRoot.copyValueFor(index)
                    monospace: !rowRoot.header
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: rowRoot.columnFills(index)
                    onActivated: rowRoot.cellActivated(index)
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header
                && index >= 0
                && index < rowRoot.linkKinds.length
                && String(rowRoot.linkKinds[index] || "").length > 0
                && String(rowRoot.linkValues[index] || "").length > 0
        }

        function copyValueFor(index) {
            if (index >= 0 && index < rowRoot.linkValues.length && String(rowRoot.linkValues[index] || "").length > 0) {
                return String(rowRoot.linkValues[index])
            }
            return String(rowRoot.columns[index] || "")
        }

        function columnWidth(index) {
            const value = Number(rowRoot.columnWidths[index] || -1)
            return value > 0 ? value : 120
        }

        function columnFills(index) {
            return Number(rowRoot.columnWidths[index] || -1) <= 0
        }
    }

}
