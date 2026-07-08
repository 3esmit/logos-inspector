pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../../../components"
import "../../../state"
import "../../../theme"
import "../../../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model

    width: parent ? parent.width : 900
    spacing: 16

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / L2 LEZ / Transfer Activity")
        title: qsTr("L2 Transfer Activity")
        layerLabel: qsTr("L2 LEZ")
        subtitle: qsTr("Indexer-derived transfer activity and account-reference fallbacks from finalized L2 blocks.")
        Layout.fillWidth: true
    }

    SourceStrip {
        theme: root.theme
        sources: [qsTr("L2 Indexer"), root.transferSourceLabel(), qsTr("finalized blocks")]
        Layout.fillWidth: true
    }

    Component.onCompleted: {
        if (!model.transferActivityRows.length) {
            model.refreshTransferActivityPage();
        }
    }

    PagedInspectionTable {
        theme: root.theme
        loadCount: root.model.transferActivityLimit
        rangeText: root.transferActivityRangeText()
        canGoNewer: root.model.transferActivityHistory.length > 0
        canGoOlder: root.model.transferActivityNextBeforeBlock > 0 || root.model.transferActivityOverflowRows.length > 0
        busy: root.model.busy
        Layout.fillWidth: true
        headerCells: root.recipientHeaderCells()
        rows: root.recipientRows()
        onRefreshRequested: root.model.refreshTransferActivityPage()
        onNewerRequested: root.model.previousTransferActivityPage()
        onOlderRequested: root.model.nextTransferActivityPage()
        onLoadCountSelected: function (count) {
            root.model.setTransferActivityPageLimit(count)
        }
        onCellActivated: function (row, column, cell, rowData) {
            if (column === 0 && rowData.recipientRaw.length > 0) {
                root.model.openRecipient(rowData.recipientRaw)
            }
        }
    }

    StatusMessage {
        visible: root.showAccountRefFallback()
        theme: root.theme
        tone: "info"
        title: qsTr("Account-reference fallback")
        message: qsTr("No decoded transfer outputs in this range. Showing account references observed in finalized L2 transactions.")
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.model.transferActivityError.length > 0
        theme: root.theme
        tone: "warning"
        title: root.model.sourceProblemTitle("indexer", root.model.transferActivityError, qsTr("Transfer activity unavailable"))
        message: root.model.transferActivityError
        Layout.fillWidth: true
    }

    TransferRecipientDetailPane {
        value: root.model.transferRecipientDetailValue
        theme: root.theme
        model: root.model
    }

    StatusMessage {
        visible: root.model.transferRecipientDetailValue === null
        theme: root.theme
        tone: "info"
        title: root.showAccountRefFallback() ? qsTr("Account reference") : qsTr("Transfer recipient")
        message: root.showAccountRefFallback()
            ? qsTr("Select an account reference to inspect source transactions and linked L2 blocks. This is not decoded transfer output data.")
            : qsTr("Select a recipient to inspect observed transfer outputs, source transactions, and linked L2 blocks. This is indexer-derived activity, not a local wallet directory.")
        Layout.fillWidth: true
    }

    function recipientRows() {
        const recipients = root.model.transferActivityRows || [];
        if (!recipients.length) {
            return [{
                recipientRaw: "",
                sourceRaw: "",
                cells: [
                    { text: root.model.sourceEmptyText("indexer", root.model.transferActivityError, qsTr("No account references in loaded range")), width: 240, fill: true, monospace: false },
                    { text: "-", width: 112, monospace: false },
                    { text: "-", width: 120 },
                    { text: "-", width: 82 },
                    { text: "-", width: 82 },
                    { text: "-", width: 82 }
                ]
            }];
        }
        return recipients.map(function (recipient) {
            const recipientId = String(recipient.account_ref || recipient.recipient || "")
            return {
                recipientRaw: recipientId,
                sourceRaw: String(recipient.source || ""),
                cells: [
                    { text: root.shortRecipient(recipientId), width: 240, fill: true, link: recipientId.length > 0, copyable: recipientId.length > 0, copyText: recipientId },
                    { text: root.sourceLabel(recipient.source), width: 112, monospace: false },
                    { text: root.receivedText(recipient), width: 120 },
                    { text: root.numberText(recipient.txs), width: 82 },
                    { text: root.numberText(recipient.source === "account_refs" ? recipient.references : recipient.outputs), width: 82 },
                    { text: root.numberText(recipient.last_slot), width: 82 }
                ]
            };
        });
    }

    function transferSourceLabel() {
        const rows = (root.model.transferActivityRows || []).concat(root.model.transferActivityOverflowRows || [])
        for (let i = 0; i < rows.length; ++i) {
            if (String(rows[i].source || "") === "transfer_outputs") {
                return qsTr("transfer-output scan")
            }
        }
        return qsTr("account-reference fallback")
    }

    function recipientHeaderCells() {
        return root.showAccountRefFallback()
            ? [
                { text: qsTr("Account"), width: 240, fill: true },
                { text: qsTr("Source"), width: 112 },
                { text: qsTr("Observed amount"), width: 120 },
                { text: qsTr("Txs"), width: 82 },
                { text: qsTr("References"), width: 82 },
                { text: qsTr("Last L2 block"), width: 82 }
            ]
            : [
                { text: qsTr("Recipient"), width: 240, fill: true },
                { text: qsTr("Source"), width: 112 },
                { text: qsTr("Observed amount"), width: 120 },
                { text: qsTr("Txs"), width: 82 },
                { text: qsTr("Outputs"), width: 82 },
                { text: qsTr("Last L2 block"), width: 82 }
            ]
    }

    function showAccountRefFallback() {
        const recipients = root.model.transferActivityRows || []
        return recipients.length > 0 && recipients.every(function (recipient) {
            return String(recipient.source || "") === "account_refs"
        })
    }

    function transferActivityRangeText() {
        if (root.model.transferActivityBeforeBlock > 0) {
            return qsTr("Before L2 block %1").arg(root.numberText(root.model.transferActivityBeforeBlock));
        }
        return qsTr("Latest finalized L2 blocks");
    }

    function sourceLabel(source) {
        const value = String(source || "")
        if (value === "transfer_outputs") {
            return qsTr("transfer output")
        }
        if (value === "account_refs") {
            return qsTr("account ref")
        }
        return value.length ? value : "-"
    }

    function receivedText(recipient) {
        if (recipient.received === undefined || recipient.received === null || recipient.received === "") {
            return "-";
        }
        return root.coinText(recipient.received);
    }

    function coinText(value) {
        const text = String(value)
        if (/^-?[0-9]+$/.test(text) && text.length <= 15) {
            const numeric = Number(text);
            return numeric.toLocaleString(Qt.locale(), "f", 0);
        }
        return text;
    }

    function numberText(value) {
        return UiFormat.numberText(value);
    }

    function shortRecipient(value) {
        return UiFormat.shortMiddle(value, 18, 12, 8);
    }
}
