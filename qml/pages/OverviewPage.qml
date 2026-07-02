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
        if (!model.dashboardOverview) {
            model.refreshDashboard();
        }
    }

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Dashboard")
        title: qsTr("Dashboard")
        subtitle: qsTr("%1 profile across %2 network. Open blocks, transactions, wallets, channels, and accounts from live chain references.").arg(root.profileLabel(root.model.networkProfile)).arg(root.chainLabel())
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Refresh")
            primary: true
            enabled: !root.model.busy
            Layout.preferredWidth: 116
            onClicked: root.model.refreshDashboard()
        }
    }

    GridLayout {
        columns: root.width < 1040 ? 1 : 2
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
                    title: qsTr("Latest Blocks")
                    action: qsTr("View all")
                    onActivated: root.model.selectView("blocks")
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Slot"), qsTr("Header"), qsTr("Tx"), qsTr("Status")]
                    columnWidths: [86, -1, 58, 86]
                }

                Repeater {
                    model: root.blockRows()

                    DashboardRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.slot, modelData.header, modelData.tx, modelData.status]
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
                    title: qsTr("Latest Transactions")
                    action: qsTr("View all")
                    onActivated: root.model.selectView("transactions")
                }

                DashboardRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Slot"), qsTr("Tx hash"), qsTr("Block"), qsTr("Ops")]
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
        if (model.networkProfile === "local" || model.networkProfile === "local-node") {
            return qsTr("Local");
        }
        if (model.networkProfile === "custom") {
            return qsTr("Custom");
        }
        return qsTr("Testnet");
    }

    function profileLabel(value) {
        if (value === "local") {
            return qsTr("Local");
        }
        if (value === "local-node") {
            return qsTr("Local node");
        }
        if (value === "testnet-indexer-local") {
            return qsTr("Mixed");
        }
        if (value === "custom") {
            return qsTr("Custom");
        }
        return qsTr("Testnet");
    }

    function consensusValue() {
        const node = overview().node;
        const probe = node ? node.consensus : null;
        return probe && probe.value ? probe.value : {};
    }

    function cryptarchiaInfo() {
        return consensusValue().cryptarchia_info || {};
    }

    function cryptarchiaValue(key) {
        const value = cryptarchiaInfo()[key];
        return value === undefined || value === null ? null : value;
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
        return value === undefined || value === null ? null : value;
    }

    function mantleValue(key) {
        const value = mantleInfo()[key];
        return value === undefined || value === null ? null : value;
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

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-";
        }
        if (typeof value === "number") {
            return value.toLocaleString(Qt.locale(), "f", 0);
        }
        return String(value);
    }

    function shortHash(value) {
        const text = String(value || "");
        if (text.length <= 16) {
            return text.length ? text : "-";
        }
        return text.slice(0, 8) + "..." + text.slice(-6);
    }

    function blockRows() {
        const blocks = model.dashboardBlocks || [];
        if (blocks.length > 0) {
            return blocks.slice(0, 5).map(function (block) {
                return {
                    slot: root.numberText(block.block_id),
                    slotRaw: String(block.block_id || ""),
                    header: root.shortHash(block.header_hash),
                    tx: root.numberText(block.tx_count),
                    status: block.bedrock_status || "-",
                    blockHash: String(block.header_hash || "")
                };
            });
        }
        return [
            {
                slot: root.numberText(root.cryptarchiaValue("slot")),
                header: root.shortHash(root.cryptarchiaValue("tip")),
                tx: "-",
                status: qsTr("Tip"),
                blockHash: String(root.cryptarchiaValue("tip") || "")
            },
            {
                slot: root.numberText(root.cryptarchiaValue("lib_slot")),
                header: root.shortHash(root.cryptarchiaValue("lib")),
                tx: "-",
                status: qsTr("LIB"),
                blockHash: String(root.cryptarchiaValue("lib") || "")
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

        function columnWidth(index) {
            const value = Number(rowRoot.columnWidths[index] || -1)
            return value > 0 ? value : 120
        }

        function columnFills(index) {
            return Number(rowRoot.columnWidths[index] || -1) <= 0
        }
    }

}
