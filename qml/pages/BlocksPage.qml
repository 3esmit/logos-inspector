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
        if (!model.blocksPageRows.length) {
            model.refreshBlocksPage();
        }
    }

    RowLayout {
        spacing: 12
        Layout.fillWidth: true

        ColumnLayout {
            spacing: 6
            Layout.fillWidth: true

            Text {
                text: qsTr("Home > Blocks")
                color: root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 12
                Layout.fillWidth: true
            }

            Text {
                text: qsTr("Blocks")
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
            onClicked: root.model.refreshBlocksPage()
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Older >")
            enabled: !root.model.busy && root.model.blocksPageSlotFrom > 0
            Layout.preferredWidth: 104
            onClicked: root.model.olderBlocksPage()
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

            BlockRow {
                theme: root.theme
                header: true
                columns: [qsTr("Slot"), qsTr("Height"), qsTr("Header"), qsTr("Tx"), qsTr("Leader"), qsTr("Status")]
            }

            Repeater {
                model: root.blockRows()

                BlockRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.slot, modelData.height, modelData.header, modelData.tx, modelData.leader, modelData.status]
                    blockHash: modelData.blockHash
                    rawBlock: modelData.rawBlock
                    status: modelData.status
                    onCellActivated: function (column) {
                        if (column === 0 || column === 2) {
                            root.model.openReference("block", modelData.slotRaw, modelData.rawBlock);
                        }
                    }
                }
            }
        }
    }

    Text {
        visible: root.model.blocksPageError.length > 0
        text: root.model.blocksPageError
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        Layout.fillWidth: true
    }

    BlockDetailPane {
        value: root.model.blockDetailValue
        theme: root.theme
        model: root.model
    }

    Panel {
        visible: root.model.blockDetailValue === null
        theme: root.theme
        title: qsTr("Block detail")
        Layout.fillWidth: true

        Text {
            text: qsTr("Select a block header or slot to inspect its parent, consensus fields, and transaction list.")
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            font.pixelSize: 14
            Layout.fillWidth: true
        }
    }

    function blockRows() {
        const blocks = root.model.blocksPageRows || [];
        if (!blocks.length) {
            return [{
                slot: "-",
                slotRaw: "",
                height: "-",
                header: qsTr("No blocks in loaded range"),
                tx: "-",
                leader: "-",
                status: "-",
                blockHash: "",
                rawBlock: null
            }];
        }
        return blocks.map(function (block) {
            const header = block.header || {};
            const proof = header.proof_of_leadership || {};
            const transactions = block.transactions || [];
            const hash = root.model.blockHash(block);
            return {
                slot: root.numberText(header.slot),
                slotRaw: String(header.slot || ""),
                height: root.numberText(block.height || header.height),
                header: root.shortHash(hash),
                tx: root.numberText(transactions.length),
                leader: root.shortHash(proof.leader_key),
                status: root.statusText(header.slot),
                blockHash: hash,
                rawBlock: block
            };
        });
    }

    function statusText(slot) {
        const value = Number(slot || 0);
        const info = root.blockchainInfo();
        if (!value || !info) {
            return "-";
        }
        if (info.lib_slot !== undefined && value <= Number(info.lib_slot)) {
            return qsTr("confirmed");
        }
        if (info.slot !== undefined && value <= Number(info.slot)) {
            return qsTr("pending");
        }
        return "-";
    }

    function blockchainInfo() {
        const report = root.model.dashboardNode;
        const probe = report ? report.cryptarchia_info : null;
        return probe && probe.value ? probe.value.cryptarchia_info : null;
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

    component BlockRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string blockHash: ""
        property var rawBlock: null
        property string status: ""
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
            columns: 6
            columnSpacing: 10

            Repeater {
                model: 6

                Text {
                    required property int index

                    text: String(rowRoot.columns[index] || "-")
                    color: rowRoot.linkFor(index) ? rowRoot.theme.accent : rowRoot.textColor(index)
                    textFormat: Text.PlainText
                    font.family: rowRoot.header ? "" : "monospace"
                    font.pixelSize: rowRoot.header ? 11 : 12
                    font.weight: rowRoot.header ? Font.DemiBold : Font.Normal
                    font.capitalization: rowRoot.header ? Font.AllUppercase : Font.MixedCase
                    font.underline: rowRoot.linkFor(index)
                    elide: Text.ElideRight
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 2 || index === 4

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
            return !rowRoot.header
                && rowRoot.rawBlock !== null
                && ((index === 0 && String(rowRoot.columns[0] || "").length > 0)
                    || (index === 2 && rowRoot.blockHash.length > 0));
        }

        function columnWidth(index) {
            if (index === 0) {
                return 96;
            }
            if (index === 1 || index === 3 || index === 5) {
                return 72;
            }
            return 140;
        }

        function textColor(index) {
            if (rowRoot.header) {
                return rowRoot.theme.textMuted;
            }
            if (index !== 5) {
                return rowRoot.theme.text;
            }
            if (rowRoot.status === "confirmed") {
                return rowRoot.theme.success;
            }
            if (rowRoot.status === "pending") {
                return rowRoot.theme.warning;
            }
            return rowRoot.theme.textMuted;
        }
    }
}
