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

    PageHeader {
        theme: root.theme
        breadcrumb: qsTr("Home / Blocks")
        title: qsTr("Blocks")
        subtitle: qsTr("Newest first from the configured blockchain node. Open a slot or header to inspect consensus fields and transactions.")
        Layout.fillWidth: true

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

    GridLayout {
        columns: root.width < 760 ? 2 : 4
        columnSpacing: 12
        rowSpacing: 12
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Node")
            value: root.endpointLabel(root.model.nodeUrl)
            delta: root.shortEndpoint(root.model.nodeUrl)
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Finalized")
            value: root.numberText(root.chainSlot("lib_slot"))
            delta: qsTr("LIB slot")
            deltaColor: root.chainSlot("lib_slot") > 0 ? root.theme.success : root.theme.textMuted
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Head")
            value: root.numberText(root.chainSlot("slot"))
            delta: qsTr("Node slot")
            deltaColor: root.chainSlot("slot") > 0 ? root.theme.warning : root.theme.textMuted
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Loaded")
            value: root.numberText((root.model.blocksPageRows || []).length)
            delta: root.slotRangeText()
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
                    selected: root.isSelectedBlock(modelData.blockHash)
                    onCellActivated: function (column) {
                        if (column === 0 || column === 2) {
                            root.model.openReference("block", modelData.slotRaw, modelData.rawBlock);
                        }
                    }
                }
            }
        }
    }

    StatusMessage {
        visible: root.model.blocksPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Blocks unavailable")
        message: root.model.blocksPageError
        Layout.fillWidth: true
    }

    BlockDetailPane {
        value: root.model.blockDetailValue
        theme: root.theme
        model: root.model
    }

    StatusMessage {
        visible: root.model.blockDetailValue === null
        theme: root.theme
        tone: "info"
        title: qsTr("Block detail")
        message: qsTr("Select a block header or slot to inspect its parent, consensus fields, and transaction list.")
        Layout.fillWidth: true
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
            const status = root.model.blockStatus(block);
            return {
                slot: root.numberText(header.slot),
                slotRaw: String(header.slot || ""),
                height: root.numberText(block.height || header.height),
                header: root.shortHash(hash),
                tx: root.numberText(transactions.length),
                leader: root.shortHash(proof.leader_key),
                status: status,
                blockHash: hash,
                rawBlock: block
            };
        });
    }

    function blockchainInfo() {
        return root.model.blockchainInfo();
    }

    function chainSlot(field) {
        const info = root.blockchainInfo();
        if (!info || info[field] === undefined) {
            return 0;
        }
        return Number(info[field] || 0);
    }

    function slotRangeText() {
        if (root.model.blocksPageSlotTo <= 0) {
            return qsTr("No range loaded");
        }
        return qsTr("Slots %1-%2").arg(root.numberText(root.model.blocksPageSlotFrom)).arg(root.numberText(root.model.blocksPageSlotTo));
    }

    function endpointLabel(value) {
        const text = String(value || "");
        if (!text.length) {
            return "-";
        }
        if (text.indexOf("127.0.0.1") >= 0 || text.indexOf("localhost") >= 0) {
            return qsTr("Local");
        }
        if (text.indexOf("testnet") >= 0) {
            return qsTr("Testnet");
        }
        return qsTr("Custom");
    }

    function shortEndpoint(value) {
        const text = String(value || "");
        if (!text.length) {
            return qsTr("Not configured");
        }
        return text.replace(/^https?:\/\//, "").replace(/\/$/, "");
    }

    function isSelectedBlock(hash) {
        const detail = root.model.blockDetailValue;
        return detail !== null && String(detail.hash || "") === String(hash || "");
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

    component BlockRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string blockHash: ""
        property var rawBlock: null
        property string status: ""
        property bool header: false
        property bool selected: false
        signal cellActivated(int column)

        Layout.fillWidth: true
        Layout.preferredHeight: rowRoot.header ? 36 : 42

        Rectangle {
            anchors.fill: parent
            color: rowRoot.header ? rowRoot.theme.field : (rowRoot.selected ? rowRoot.theme.accentMuted : "transparent")
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
                    monospace: !rowRoot.header
                    textColor: rowRoot.textColor(index)
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 2 || index === 4
                    onActivated: rowRoot.cellActivated(index)
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
            if (rowRoot.status === "finalized" || rowRoot.status === "confirmed") {
                return rowRoot.theme.success;
            }
            if (rowRoot.status === "pending") {
                return rowRoot.theme.warning;
            }
            return rowRoot.theme.textMuted;
        }
    }
}
