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

            TransactionRow {
                theme: root.theme
                header: true
                columns: [qsTr("L1 slot"), qsTr("Tx hash"), qsTr("Header"), qsTr("Ops")]
            }

            Repeater {
                model: root.transactionRows()

                TransactionRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.slot, modelData.hash, modelData.block, modelData.ops]
                    txHash: modelData.txHash
                    blockHash: modelData.blockHash
                    onCellActivated: function (column) {
                        if (column === 1) {
                            root.model.openMantleTransaction(modelData.txHash);
                        } else if (column === 2) {
                            root.model.openBlockchainBlock(modelData.blockHash);
                        }
                    }
                }
            }
        }
    }

    StatusMessage {
        visible: root.model.transactionsPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Transactions unavailable")
        message: root.model.transactionsPageError
        Layout.fillWidth: true
    }

    TransactionDetailPane {
        value: root.model.transactionDetailValue
        theme: root.theme
        model: root.model
    }

    StatusMessage {
        visible: root.model.transactionDetailValue === null
        theme: root.theme
        tone: "info"
        title: qsTr("Transaction detail")
        message: qsTr("Select a transaction hash to inspect operations, decoded instruction data, account references, and linked block context.")
        Layout.fillWidth: true
    }

    function transactionRows() {
        const transactions = root.model.transactionsPageRows || [];
        if (!transactions.length) {
            return [{
                slot: "-",
                hash: qsTr("No transactions in loaded range"),
                block: "-",
                ops: "-"
            }];
        }
        return transactions.map(function (tx) {
            return {
                slot: root.numberText(tx.slot),
                hash: root.shortHash(tx.hash),
                block: root.shortHash(tx.block),
                ops: root.numberText(tx.ops),
                txHash: String(tx.hash || ""),
                blockHash: String(tx.block || "")
            };
        });
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

    component TransactionRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string txHash: ""
        property string blockHash: ""
        property bool header: false
        signal cellActivated(int column)

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 36 : 42

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
                    Layout.fillWidth: index === 1 || index === 2
                    onActivated: rowRoot.cellActivated(index)
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header && ((index === 1 && rowRoot.txHash.length > 0) || (index === 2 && rowRoot.blockHash.length > 0));
        }

        function copyValueFor(index) {
            if (index === 1 && rowRoot.txHash.length > 0) {
                return rowRoot.txHash
            }
            if (index === 2 && rowRoot.blockHash.length > 0) {
                return rowRoot.blockHash
            }
            return String(rowRoot.columns[index] || "")
        }

        function columnWidth(index) {
            if (index === 0) {
                return 96;
            }
            if (index === 3) {
                return 64;
            }
            return 180;
        }
    }
}
