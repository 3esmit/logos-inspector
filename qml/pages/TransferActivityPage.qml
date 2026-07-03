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
        sources: [qsTr("L2 Indexer"), qsTr("account-reference scan"), qsTr("finalized blocks")]
        Layout.fillWidth: true
    }

    Component.onCompleted: {
        if (!model.transferActivityRows.length) {
            model.refreshTransferActivityPage();
        }
    }

    ListToolbar {
        theme: root.theme
        loadCount: root.model.transferActivityLimit
        rangeText: root.transferActivityRangeText()
        canGoNewer: root.model.transferActivityHistory.length > 0
        canGoOlder: root.model.transferActivityNextBeforeBlock > 0
        busy: root.model.busy
        Layout.fillWidth: true
        onRefresh: root.model.refreshTransferActivityPage()
        onNewer: root.model.previousTransferActivityPage()
        onOlder: root.model.nextTransferActivityPage()
        onLoadCountSelected: function (count) {
            root.model.setTransferActivityPageLimit(count)
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

            RecipientRow {
                theme: root.theme
                header: true
                columns: root.showAccountRefFallback()
                    ? [qsTr("Account"), qsTr("Source"), qsTr("Observed amount"), qsTr("Txs"), qsTr("References"), qsTr("Last L2 block")]
                    : [qsTr("Recipient"), qsTr("Source"), qsTr("Observed amount"), qsTr("Txs"), qsTr("Outputs"), qsTr("Last L2 block")]
            }

            Repeater {
                model: root.recipientRows()

                RecipientRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.recipient, modelData.source, modelData.received, modelData.txs, modelData.outputs, modelData.lastSlot]
                    recipient: modelData.recipientRaw
                    source: modelData.sourceRaw
                    onRecipientActivated: root.model.openRecipient(modelData.recipientRaw)
                }
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
        title: qsTr("Transfer activity unavailable")
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
                recipient: qsTr("No account references in loaded range"),
                recipientRaw: "",
                source: "-",
                sourceRaw: "",
                received: "-",
                txs: "-",
                outputs: "-",
                lastSlot: "-"
            }];
        }
        return recipients.map(function (recipient) {
            return {
                recipient: root.shortRecipient(recipient.account_ref || recipient.recipient),
                recipientRaw: String(recipient.account_ref || recipient.recipient || ""),
                source: root.sourceLabel(recipient.source),
                sourceRaw: String(recipient.source || ""),
                received: root.receivedText(recipient),
                txs: root.numberText(recipient.txs),
                outputs: root.numberText(recipient.source === "account_refs" ? recipient.references : recipient.outputs),
                lastSlot: root.numberText(recipient.last_slot)
            };
        });
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
        if (value === undefined || value === null || value === "") {
            return "-";
        }
        const numeric = Number(value);
        if (Number.isFinite(numeric)) {
            return numeric.toLocaleString(Qt.locale(), "f", 0);
        }
        return String(value);
    }

    function shortRecipient(value) {
        const text = String(value || "");
        if (text.length <= 18) {
            return text.length ? text : "-";
        }
        return text.slice(0, 12) + "..." + text.slice(-8);
    }

    component RecipientRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string recipient: ""
        property string source: ""
        property bool header: false
        signal recipientActivated()

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
            columns: 6
            columnSpacing: 10

            Repeater {
                model: 6

                LinkCell {
                    required property int index

                    theme: rowRoot.theme
                    text: String(rowRoot.columns[index] || "-")
                    header: rowRoot.header
                    link: rowRoot.linkFor(index)
                    copyable: !rowRoot.header && index === 0 && rowRoot.recipient.length > 0
                    copyText: rowRoot.recipient.length > 0 ? rowRoot.recipient : String(rowRoot.columns[index] || "")
                    monospace: !rowRoot.header
                    textColor: rowRoot.textColor(index)
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 0
                    onActivated: rowRoot.recipientActivated()
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header && index === 0 && rowRoot.recipient.length > 0;
        }

        function textColor(index) {
            if (rowRoot.linkFor(index)) {
                return rowRoot.theme.accent;
            }
            return rowRoot.header ? rowRoot.theme.textMuted : rowRoot.theme.text;
        }

        function columnWidth(index) {
            if (index === 0) {
                return 240;
            }
            if (index === 1) {
                return 112;
            }
            if (index === 2) {
                return 120;
            }
            return 82;
        }
    }
}
