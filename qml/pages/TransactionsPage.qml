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

    RowLayout {
        spacing: 12
        Layout.fillWidth: true

        ColumnLayout {
            spacing: 6
            Layout.fillWidth: true

            Text {
                text: qsTr("Home > Transactions")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 12
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Transactions")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 28
                font.weight: Font.Bold
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Newest first.")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 14
                Layout.fillWidth: true
            }
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Latest")
            primary: true
            enabled: !root.model.busy
            Layout.preferredWidth: 104
            onClicked: root.model.refreshTransactionsPage()
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Older >")
            enabled: !root.model.busy && root.model.transactionsPageNextBeforeBlock > 0
            Layout.preferredWidth: 104
            onClicked: root.model.olderTransactionsPage()
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
                columns: [qsTr("Slot"), qsTr("Tx hash"), qsTr("Block"), qsTr("Ops")]
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
                            root.model.openTransaction(modelData.txHash);
                        } else if (column === 2) {
                            root.model.openIndexerBlock(modelData.blockHash);
                        }
                    }
                }
            }
        }
    }

    Text {
        visible: root.model.transactionsPageError.length > 0
        text: root.model.transactionsPageError
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        Layout.fillWidth: true
    }

    TransactionDetailPane {
        value: root.model.transactionDetailValue
        theme: root.theme
        model: root.model
    }

    Panel {
        visible: root.model.transactionDetailValue === null
        theme: root.theme
        title: qsTr("Transaction detail")
        Layout.fillWidth: true

        Text {
            text: qsTr("Select a transaction hash to inspect operations, decoded instruction data, account references, and linked block context.")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 14
            Layout.fillWidth: true
        }
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
            return value.toLocaleString(Qt.locale());
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

        GridLayout {
            anchors.fill: parent
            anchors.leftMargin: 14
            anchors.rightMargin: 14
            columns: 4
            columnSpacing: 10

            Repeater {
                model: 4

                Text {
                    required property int index

                    text: String(rowRoot.columns[index] || "-")
                    color: rowRoot.linkFor(index) ? rowRoot.theme.accent : (rowRoot.header ? rowRoot.theme.textMuted : rowRoot.theme.text)
                    textFormat: Text.PlainText
                    font.family: rowRoot.header ? "" : "monospace"
                    font.pixelSize: rowRoot.header ? 11 : 12
                    font.weight: rowRoot.header ? Font.DemiBold : Font.Normal
                    font.capitalization: rowRoot.header ? Font.AllUppercase : Font.MixedCase
                    font.underline: rowRoot.linkFor(index)
                    elide: Text.ElideRight
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 1 || index === 2

                    MouseArea {
                        anchors.fill: parent
                        enabled: rowRoot.linkFor(parent.index)
                        cursorShape: Qt.PointingHandCursor
                        onClicked: rowRoot.cellActivated(parent.index)
                    }
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header && ((index === 1 && rowRoot.txHash.length > 0) || (index === 2 && rowRoot.blockHash.length > 0));
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
