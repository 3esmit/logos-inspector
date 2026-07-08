pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../components/common"
import "../../../state"
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        if (!model.lezBlocksPageRows.length) {
            model.refreshLezBlocksPage();
        }
    }

    ListToolbar {
        theme: root.theme
        loadCount: root.model.lezBlocksPageLimit
        rangeText: root.rangeText()
        canGoNewer: root.model.lezBlocksPageBeforeBlock > 0
        canGoOlder: root.model.lezBlocksPageNextBeforeBlock > 0
        busy: root.model.busy || root.model.lezBlocksPageLoading
        Layout.fillWidth: true
        onRefresh: root.model.refreshLezBlocksPage(root.model.lezBlocksPageBeforeBlock > 0 ? root.model.lezBlocksPageBeforeBlock : null)
        onNewer: root.model.newerLezBlocksPage()
        onOlder: root.model.olderLezBlocksPage()
        onLoadCountSelected: function (count) {
            root.model.setLezBlocksPageLimit(count)
        }
    }

    DataTableFrame {
        theme: root.theme
        Layout.fillWidth: true
        headerCells: [
            { text: qsTr("L2 block"), width: 96 },
            { text: qsTr("Header"), width: 220, fill: true },
            { text: qsTr("Tx"), width: 64 },
            { text: qsTr("Bedrock"), width: 98 }
        ]
        rows: root.blockRows()
        onCellActivated: function (row, column, cell, rowData) {
            if ((column === 0 || column === 1) && rowData.blockHash.length > 0) {
                root.model.openReference("indexerBlock", rowData.blockHash, rowData.rawBlock)
            }
        }
    }

    StatusMessage {
        visible: root.model.lezBlocksPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: root.model.sourceProblemTitle("indexer", root.model.lezBlocksPageError, qsTr("L2 blocks unavailable"))
        message: root.model.lezBlocksPageError
        Layout.fillWidth: true
    }

    function blockRows() {
        const blocks = root.model.lezBlocksPageRows || [];
        if (!blocks.length) {
            return [{
                cells: [
                    { text: "-", width: 96 },
                    { text: root.model.sourceEmptyText("indexer", root.model.lezBlocksPageError, qsTr("No indexed blocks")), width: 220, fill: true, monospace: false },
                    { text: "-", width: 64 },
                    { text: "-", width: 98 }
                ],
                blockHash: "",
                rawBlock: null
            }];
        }
        return blocks.map(function (block) {
            const blockHash = String(block.header_hash || "")
            return {
                cells: [
                    { text: root.numberText(block.block_id), width: 96, link: blockHash.length > 0, copyText: blockHash },
                    { text: UiFormat.shortHash(blockHash), width: 220, fill: true, link: blockHash.length > 0, copyText: blockHash },
                    { text: root.numberText(block.tx_count !== undefined ? block.tx_count : ((block.transactions || []).length)), width: 64 },
                    { text: String(block.bedrock_status || "-"), width: 98, monospace: false }
                ],
                blockHash: blockHash,
                rawBlock: block
            };
        });
    }

    function rangeText() {
        if (root.model.lezBlocksPageBeforeBlock <= 0) {
            return qsTr("Latest L2 blocks");
        }
        return qsTr("Before L2 block %1").arg(root.numberText(root.model.lezBlocksPageBeforeBlock));
    }

    function numberText(value) {
        return UiFormat.numberText(value);
    }
}
