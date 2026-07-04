pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../components"
import "../components/common"
import "../state"
import "../theme"
import "../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    Component.onCompleted: {
        if (!model.transactionsPageRows.length) {
            model.refreshTransactionsPage();
        }
    }

    ListToolbar {
        theme: root.theme
        loadCount: root.model.transactionsPageLimit
        rangeText: root.transactionRangeText()
        canGoNewer: root.canLoadNewer()
        canGoOlder: root.model.transactionsPageNextBeforeBlock > 0
        busy: root.model.busy
        Layout.fillWidth: true
        onRefresh: root.model.refreshTransactionsPage()
        onNewer: root.model.newerTransactionsPage()
        onOlder: root.model.olderTransactionsPage()
        onLoadCountSelected: function (count) {
            root.model.setTransactionsPageLimit(count)
        }
    }

    DataTableFrame {
        theme: root.theme
        Layout.fillWidth: true
        headerCells: [
            { text: qsTr("L1 slot"), width: 96 },
            { text: qsTr("Tx hash"), width: 180, fill: true },
            { text: qsTr("Header"), width: 180, fill: true },
            { text: qsTr("Ops"), width: 64 }
        ]
        rows: root.transactionRows()
        onCellActivated: function (row, column, cell, rowData) {
            if (column === 1 && rowData.txHash.length > 0) {
                root.model.openMantleTransaction(rowData.txHash)
            } else if (column === 2 && rowData.blockHash.length > 0) {
                root.model.openBlockchainBlock(rowData.blockHash)
            }
        }
    }

    StatusMessage {
        visible: root.model.transactionsPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: root.model.sourceProblemTitle("blockchain", root.model.transactionsPageError, qsTr("Transactions unavailable"))
        message: root.model.transactionsPageError
        Layout.fillWidth: true
    }

    function transactionRows() {
        const transactions = root.model.transactionsPageRows || [];
        if (!transactions.length) {
            return [{
                cells: [
                    { text: "-", width: 96 },
                    { text: root.model.sourceEmptyText("blockchain", root.model.transactionsPageError, qsTr("No transactions in loaded range")), width: 180, fill: true, monospace: false },
                    { text: "-", width: 180, fill: true },
                    { text: "-", width: 64 }
                ],
                txHash: "",
                blockHash: ""
            }];
        }
        return transactions.map(function (tx) {
            const txHash = String(tx.hash || "")
            const blockHash = String(tx.block || "")
            return {
                cells: [
                    { text: root.numberText(tx.slot), width: 96 },
                    { text: UiFormat.shortHash(txHash), width: 180, fill: true, link: txHash.length > 0, copyText: txHash },
                    { text: UiFormat.shortHash(blockHash), width: 180, fill: true, link: blockHash.length > 0, copyText: blockHash },
                    { text: root.numberText(tx.ops), width: 64 }
                ],
                txHash: txHash,
                blockHash: blockHash
            };
        });
    }

    function numberText(value) {
        return UiFormat.numberText(value);
    }

    function transactionRangeText() {
        if (root.model.transactionsPageBeforeBlock <= 0) {
            return qsTr("No range loaded")
        }
        return qsTr("Before L1 slot %1").arg(root.numberText(root.model.transactionsPageBeforeBlock))
    }

    function canLoadNewer() {
        const current = root.chainSlot("lib_slot")
        return root.model.transactionsPageBeforeBlock > 0 && current > 0 && root.model.transactionsPageBeforeBlock < current
    }

    function chainSlot(field) {
        const info = root.model.blockchainInfo()
        if (!info || info[field] === undefined || info[field] === null) {
            return 0
        }
        return Number(info[field] || 0)
    }

}
