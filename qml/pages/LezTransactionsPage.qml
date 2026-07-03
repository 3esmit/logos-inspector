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
                columns: [qsTr("L2 block"), qsTr("Tx hash"), qsTr("Kind"), qsTr("Words")]
            }

            Repeater {
                model: root.transactionRows()

                TransactionRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.block, modelData.hash, modelData.kind, modelData.words]
                    txHash: modelData.txHash
                    blockHash: modelData.blockHash
                    onCellActivated: function (column) {
                        if (column === 0 && modelData.blockHash.length > 0) {
                            root.model.openReference("indexerBlock", modelData.blockHash)
                        } else if (column === 1 && modelData.txHash.length > 0) {
                            root.model.openReference("transaction", modelData.txHash)
                        }
                    }
                }
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
                block: "-",
                hash: qsTr("No indexed transactions"),
                kind: "-",
                words: "-",
                txHash: "",
                blockHash: ""
            }];
        }
        return transactions.map(function (tx) {
            return {
                block: root.numberText(tx.block_id),
                hash: root.shortHash(tx.hash),
                kind: String(tx.kind || "-"),
                words: root.numberText(tx.ops),
                txHash: root.validHash(tx.hash) ? String(tx.hash || "") : "",
                blockHash: String(tx.block_hash || "")
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

        Rectangle {
            anchors.left: parent.left
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            height: 1
            color: rowRoot.theme.outlineMuted
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
            return !rowRoot.header && ((index === 0 && rowRoot.blockHash.length > 0) || (index === 1 && rowRoot.txHash.length > 0));
        }

        function copyValueFor(index) {
            if (index === 0 && rowRoot.blockHash.length > 0) {
                return rowRoot.blockHash;
            }
            if (index === 1 && rowRoot.txHash.length > 0) {
                return rowRoot.txHash;
            }
            return String(rowRoot.columns[index] || "");
        }

        function columnWidth(index) {
            if (index === 0) {
                return 96;
            }
            if (index === 3) {
                return 72;
            }
            return 180;
        }
    }
}
