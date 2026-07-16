pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../state/status/StatusFactsProjection.js" as StatusFactsProjection
import "../../../theme"
import "../controls"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        if (model.metrics.dashboardRefreshInterval() > 0 && model.bridgeSupportsAsync()) {
            Qt.callLater(function () {
                model.metrics.refreshDashboard()
            })
        }
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
        objectName: "dashboardActivityGrid"
        columns: root.width < 860 ? 1 : 2
        columnSpacing: root.theme.gap
        rowSpacing: root.theme.gap
        Layout.fillWidth: true

        Frame {
            objectName: "dashboardL1BlocksPanel"
            padding: 0
            Layout.minimumWidth: 0
            Layout.preferredWidth: 1
            Layout.fillWidth: true
            Layout.row: 0
            Layout.column: 0

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
                    title: qsTr("Recent L1 Blocks")
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
                                root.model.entityNavigation.openReference("block", column === 0 ? modelData.slotRaw : modelData.blockHash)
                            }
                        }
                    }
                }
            }
        }

        DashboardZonesPanel {
            theme: root.theme
            model: root.model
            Layout.minimumWidth: 0
            Layout.preferredWidth: 1
            Layout.fillWidth: true
            Layout.row: root.width < 860 ? 2 : 0
            Layout.column: root.width < 860 ? 0 : 1
            Layout.rowSpan: root.width < 860 ? 1 : 2
            Layout.alignment: Qt.AlignTop
        }

        Frame {
            objectName: "dashboardL1TransactionsPanel"
            padding: 0
            Layout.minimumWidth: 0
            Layout.preferredWidth: 1
            Layout.fillWidth: true
            Layout.row: 1
            Layout.column: 0

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
                    title: qsTr("Recent L1 Transactions")
                    action: qsTr("View all")
                    onActivated: {
                        root.model.selectView("transactions")
                    }
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("L1 slot"), qsTr("Tx hash"), qsTr("Header"), qsTr("Ops")]
                    columnWidths: [86, -1, -1, 58]
                }

                Repeater {
                    model: root.l1TransactionRows()

                    DashboardRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.slot, modelData.hash, modelData.block, modelData.ops]
                        columnWidths: [86, -1, -1, 58]
                        linkKinds: ["", "mantleTransaction", "block", ""]
                        linkValues: ["", modelData.txHash, modelData.blockHash, ""]
                        onCellActivated: function (column) {
                            if (column === 1) {
                                root.model.entityNavigation.openMantleTransaction(modelData.txHash)
                            } else if (column === 2) {
                                root.model.entityNavigation.openReference("block", modelData.blockHash)
                            }
                        }
                    }
                }
            }
        }

    }

    StatusMessage {
        visible: root.model.metrics.dashboardError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Dashboard refresh failed")
        message: root.model.metrics.dashboardError
        Layout.fillWidth: true
    }

    function overview() {
        return model.metrics.dashboardOverview || {};
    }

    function nodeReport() {
        return model.metrics.dashboardNode || {};
    }

    function chainLabel() {
        const profile = String(model.networkProfile || "").toLowerCase()
        const node = String(model.nodeUrl || "").toLowerCase()
        if (node.indexOf("127.0.0.1") >= 0 || node.indexOf("localhost") >= 0) {
            return qsTr("Local");
        }
        if (profile.indexOf("mainnet") >= 0 || node.indexOf("mainnet") >= 0) {
            return qsTr("Mainnet")
        }
        if (profile.indexOf("testnet") >= 0 || node.indexOf("testnet") >= 0) {
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
        return value === undefined || value === null ? null : root.model.metrics.scalarValue(value);
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
        return value === undefined || value === null ? null : root.model.metrics.scalarValue(value);
    }

    function mantleValue(key) {
        const value = mantleInfo()[key];
        return value === undefined || value === null ? null : root.model.metrics.scalarValue(value);
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
        return StatusFactsProjection.selectedDashboardGraphItems(root.model)
    }

    function dashboardGraphItem(key) {
        return StatusFactsProjection.dashboardGraphItem(root.model, key)
    }

    function dashboardMetricTone(key, numeric) {
        return StatusFactsProjection.dashboardMetricTone(key, numeric)
    }

    function dashboardMetricGroup(key) {
        return StatusFactsProjection.dashboardMetricGroup(key)
    }

    function dashboardMetricLabel(key) {
        return StatusFactsProjection.dashboardMetricLabel(key)
    }

    function dashboardMetricValue(key) {
        return root.model.metrics.dashboardMetricValue(key)
    }

    function dashboardMetricText(value) {
        return StatusFactsProjection.dashboardMetricText(root.model, value)
    }

    function tipMinusLib() {
        return root.model.metrics.tipMinusLib()
    }

    function finalityLagSeconds() {
        return root.model.metrics.finalityLagSeconds()
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
        return target && target.value !== undefined && target.value !== null ? root.model.metrics.scalarValue(target.value) : null
    }

    function numberText(value) {
        const scalar = root.model.metrics.scalarValue(value)
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
        const dashboardL1Blocks = root.model.metrics.dashboardL1Blocks || []
        if (dashboardL1Blocks.length > 0) {
            return dashboardL1Blocks.slice(0, 5).map(function (block) {
                const header = block.header || {}
                const transactions = Array.isArray(block.transactions) ? block.transactions : []
                const hash = root.model.chainPages.blockHash(block)
                return {
                    slot: root.numberText(header.slot),
                    slotRaw: String(header.slot || ""),
                    header: root.shortHash(hash),
                    tx: root.numberText(transactions.length),
                    status: root.model.chainPages.blockStatus(block),
                    blockHash: hash
                }
            })
        }
        const blocks = root.model.chainPages.blocksPageRows || []
        if (blocks.length > 0 && root.blocksPageRowsAreCurrent()) {
            return blocks.slice(0, 5).map(function (block) {
                const header = block.header || {}
                const transactions = Array.isArray(block.transactions) ? block.transactions : []
                const hash = root.model.chainPages.blockHash(block)
                return {
                    slot: root.numberText(header.slot),
                    slotRaw: String(header.slot || ""),
                    header: root.shortHash(hash),
                    tx: root.numberText(transactions.length),
                    status: root.model.chainPages.blockStatus(block),
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
        const libSlot = Number(root.cryptarchiaValue("slot"))
        const slotTo = Number(root.model.chainPages.blocksPageSlotTo)
        return Number.isFinite(libSlot) && libSlot > 0 && Number.isFinite(slotTo) && slotTo >= libSlot
    }

    function l1TransactionRows() {
        const transactions = root.model.chainPages.transactionRowsFromBlocks(
            root.model.chainPages.blocksPageRows || []).slice(0, 5)
        if (transactions.length > 0) {
            return transactions.map(function (tx) {
                const txHash = String(tx.hash || "")
                const blockHash = String(tx.block || "")
                return {
                    slot: root.numberText(tx.slot),
                    hash: root.shortHash(txHash),
                    block: root.shortHash(blockHash),
                    ops: root.numberText(tx.ops),
                    txHash: txHash,
                    blockHash: blockHash
                }
            })
        }
        return [
            {
                slot: "-",
                hash: qsTr("No L1 transactions"),
                block: "-",
                ops: "-",
                txHash: "",
                blockHash: ""
            }
        ]
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
        property int historyPointCount: 0

        onSamplesChanged: historyPointCount = graphRoot.validSampleCount()

        Component.onCompleted: historyPointCount = graphRoot.validSampleCount()

        objectName: "dashboardGraphTile"
        Accessible.role: Accessible.Chart
        Accessible.name: graphRoot.title
        Accessible.description: qsTr("%1 history points; current value %2")
            .arg(graphRoot.historyPointCount)
            .arg(graphRoot.value)

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

                    const raw = graphRoot.sampleSequence()
                    const sourceSamples = raw.length > 0 ? raw : [{ timestamp: Date.now(), value: graphRoot.numericValue }]
                    const samples = []
                    for (let i = 0; i < sourceSamples.length; ++i) {
                        const sample = sourceSamples[i]
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

        function validSampleCount() {
            const raw = graphRoot.sampleSequence()
            let count = 0
            for (let index = 0; index < raw.length; ++index) {
                const sample = raw[index]
                const value = Number(sample && typeof sample === "object" ? sample.value : sample)
                if (Number.isFinite(value)) {
                    count += 1
                }
            }
            return count
        }

        function sampleSequence() {
            const candidate = graphRoot.samples
            return candidate && typeof candidate.length === "number" ? candidate : []
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

    component DashboardRow: DataTableRow {
        id: rowRoot

        property var columns: []
        property var columnWidths: [-1, -1, -1, -1]
        property var linkKinds: ["", "", "", ""]
        property var linkValues: ["", "", "", ""]

        headerHeight: 34
        rowHeight: 38
        cells: rowRoot.rowCells()

        function rowCells() {
            const result = []
            for (let i = 0; i < 4; ++i) {
                const linkValue = i < rowRoot.linkValues.length ? String(rowRoot.linkValues[i] || "") : ""
                result.push({
                    text: String(rowRoot.columns[i] || "-"),
                    width: rowRoot.columnWidth(i),
                    fill: rowRoot.columnFills(i),
                    link: !rowRoot.header && i < rowRoot.linkKinds.length && String(rowRoot.linkKinds[i] || "").length > 0 && linkValue.length > 0,
                    copyText: linkValue.length > 0 ? linkValue : String(rowRoot.columns[i] || "")
                })
            }
            return result
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
