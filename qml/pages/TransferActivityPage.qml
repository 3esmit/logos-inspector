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
                columns: [qsTr("Recipient"), qsTr("Observed amount"), qsTr("Txs"), qsTr("Outputs"), qsTr("Last L2 block")]
            }

            Repeater {
                model: root.recipientRows()

                RecipientRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.recipient, modelData.received, modelData.txs, modelData.outputs, modelData.lastSlot]
                    recipient: modelData.recipientRaw
                    onRecipientActivated: root.model.openRecipient(modelData.recipientRaw)
                }
            }
        }
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
        title: qsTr("Transfer recipient")
        message: qsTr("Select a recipient to inspect observed transfer outputs, source transactions, and linked L2 blocks. This is indexer-derived activity, not a local wallet directory.")
        Layout.fillWidth: true
    }

    function recipientRows() {
        const recipients = root.model.transferActivityRows || [];
        if (!recipients.length) {
            return [{
                recipient: qsTr("No transfer recipients in loaded range"),
                recipientRaw: "",
                received: "-",
                txs: "-",
                outputs: "-",
                lastSlot: "-"
            }];
        }
        return recipients.map(function (recipient) {
            return {
                recipient: root.shortRecipient(recipient.recipient),
                recipientRaw: String(recipient.recipient || ""),
                received: root.receivedText(recipient),
                txs: root.numberText(recipient.txs),
                outputs: root.numberText(recipient.outputs),
                lastSlot: root.numberText(recipient.last_slot)
            };
        });
    }

    function transferActivityRangeText() {
        if (root.model.transferActivityBeforeBlock > 0) {
            return qsTr("Before L2 block %1").arg(root.numberText(root.model.transferActivityBeforeBlock));
        }
        return qsTr("Latest finalized L2 transfer activity");
    }

    function receivedText(recipient) {
        if (recipient.received === undefined || recipient.received === null || recipient.received === "") {
            return "-";
        }
        return root.coinText(recipient.received);
    }

    function coinText(value) {
        const numeric = Number(value);
        if (Number.isFinite(numeric)) {
            return (numeric / 100).toLocaleString(Qt.locale(), "f", 2) + "E";
        }
        return String(value);
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
            columns: 5
            columnSpacing: 10

            Repeater {
                model: 5

                LinkCell {
                    required property int index

                    theme: rowRoot.theme
                    text: String(rowRoot.columns[index] || "-")
                    header: rowRoot.header
                    link: rowRoot.linkFor(index)
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
                return 120;
            }
            return 82;
        }
    }
}
