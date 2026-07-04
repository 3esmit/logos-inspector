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
        if (!model.lezTransactionsPageRows.length) {
            model.refreshLezTransactionsPage();
        }
    }

    ListToolbar {
        theme: root.theme
        loadCount: root.model.lezTransactionsPageLimit
        rangeText: root.rangeText()
        canGoNewer: root.model.lezTransactionsPageBeforeBlock > 0
        canGoOlder: root.model.lezTransactionsPageNextBeforeBlock > 0
        busy: root.model.busy
        Layout.fillWidth: true
        onRefresh: root.model.refreshLezTransactionsPage(root.model.lezTransactionsPageBeforeBlock > 0 ? root.model.lezTransactionsPageBeforeBlock : null)
        onNewer: root.model.newerLezTransactionsPage()
        onOlder: root.model.olderLezTransactionsPage()
        onLoadCountSelected: function (count) {
            root.model.setLezTransactionsPageLimit(count)
        }
    }

    DataTableFrame {
        theme: root.theme
        Layout.fillWidth: true
        headerCells: [
            { text: qsTr("L2 block"), width: 96 },
            { text: qsTr("Tx hash"), width: 180, fill: true },
            { text: qsTr("Kind"), width: 180, fill: true },
            { text: qsTr("Words / bytes"), width: 72 }
        ]
        rows: root.transactionRows()
        onCellActivated: function (row, column, cell, rowData) {
            if (column === 0 && rowData.blockHash.length > 0) {
                root.model.openReference("indexerBlock", rowData.blockHash)
            } else if (column === 1 && rowData.txHash.length > 0) {
                root.model.openReference("transaction", rowData.txHash)
            }
        }
    }

    StatusMessage {
        visible: root.model.lezTransactionsPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("L2 transactions unavailable")
        message: root.model.lezTransactionsPageError
        Layout.fillWidth: true
    }

    function transactionRows() {
        const transactions = root.model.lezTransactionsPageRows || [];
        if (!transactions.length) {
            return [{
                cells: [
                    { text: "-", width: 96 },
                    { text: qsTr("No indexed transactions"), width: 180, fill: true, monospace: false },
                    { text: "-", width: 180, fill: true },
                    { text: "-", width: 72 }
                ],
                txHash: "",
                blockHash: ""
            }];
        }
        return transactions.map(function (tx) {
            const txHash = root.validHash(tx.hash) ? String(tx.hash || "") : ""
            const blockHash = String(tx.block_hash || "")
            return {
                cells: [
                    { text: root.numberText(tx.block_id), width: 96, link: blockHash.length > 0, copyText: blockHash },
                    { text: UiFormat.shortHash(tx.hash), width: 180, fill: true, link: txHash.length > 0, copyText: txHash },
                    { text: String(tx.kind || "-"), width: 180, fill: true, monospace: false },
                    { text: root.numberText(tx.ops), width: 72 }
                ],
                txHash: txHash,
                blockHash: blockHash
            };
        });
    }

    function rangeText() {
        if (root.model.lezTransactionsPageBeforeBlock <= 0) {
            return qsTr("Latest L2 transactions");
        }
        return qsTr("Before L2 block %1").arg(root.numberText(root.model.lezTransactionsPageBeforeBlock));
    }

    function validHash(value) {
        const text = String(value || "");
        return text.length > 0 && text !== "-";
    }

    function numberText(value) {
        return UiFormat.numberText(value);
    }
}
