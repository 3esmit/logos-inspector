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

    Component.onCompleted: {
        if (!model.blocksPageRows.length) {
            model.chainPages.refreshBlocksPage();
        }
    }

    PagedInspectionTable {
        theme: root.theme
        loadCount: root.model.blocksPageLimit
        rangeText: root.slotRangeText()
        canGoNewer: !root.model.blocksLiveEnabled && root.canLoadNewer()
        canGoOlder: !root.model.blocksLiveEnabled && root.model.blocksPageSlotFrom > 0
        busy: root.model.shell.busy
        Layout.fillWidth: true
        headerCells: [
            { text: qsTr("L1 slot"), width: 96 },
            { text: qsTr("Height"), width: 72 },
            { text: qsTr("Header"), width: 140, fill: true },
            { text: qsTr("Tx"), width: 72 },
            { text: qsTr("Leader"), width: 140, fill: true },
            { text: qsTr("Status"), width: 72 }
        ]
        rows: root.blockRows()
        onRefreshRequested: root.model.blocksLiveEnabled ? root.model.chainPages.refreshBlocksLivePage() : root.model.chainPages.refreshBlocksPage()
        onNewerRequested: root.model.chainPages.newerBlocksPage()
        onOlderRequested: root.model.chainPages.olderBlocksPage()
        onLoadCountSelected: function (count) {
            root.model.chainPages.setBlocksPageLimit(count)
        }
        onCellActivated: function (row, column, cell, rowData) {
            if (rowData.rawBlock !== null && (column === 0 || column === 2)) {
                root.model.entityNavigation.openReference("block", rowData.slotRaw, rowData.rawBlock)
            }
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: root.model.chainPages.blocksLiveStatusText()
                color: root.model.blocksLiveEnabled ? root.theme.success : root.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: root.theme.dataText
                font.weight: Font.Medium
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: root.model.blocksLiveEnabled ? qsTr("Refresh Live") : qsTr("Live")
                primary: !root.model.blocksLiveEnabled
                enabled: !root.model.shell.busy
                Layout.preferredWidth: root.model.blocksLiveEnabled ? 126 : 82
                onClicked: root.model.blocksLiveEnabled ? root.model.chainPages.refreshBlocksLivePage() : root.model.chainPages.startBlocksLiveMode()
            }

            ActionButton {
                visible: root.model.blocksLiveEnabled
                theme: root.theme
                text: qsTr("Stop")
                enabled: !root.model.shell.busy
                Layout.preferredWidth: 82
                onClicked: root.model.chainPages.stopBlocksLiveMode()
            }
        }

        StatusMessage {
            visible: root.model.blocksLiveError.length > 0
            theme: root.theme
            tone: "warning"
            title: qsTr("Live blocks unavailable")
            message: root.model.blocksLiveError
            Layout.fillWidth: true
        }

        StatusMessage {
            visible: root.model.blocksLiveUnknownEvents > 0
            theme: root.theme
            tone: "info"
            title: qsTr("Unknown live events")
            message: qsTr("%1 raw events preserved in output.").arg(root.model.blocksLiveUnknownEvents)
            Layout.fillWidth: true
        }
    }

    StatusMessage {
        visible: root.model.blocksPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: root.model.chainPages.sourceProblemTitle("blockchain", root.model.blocksPageError, qsTr("Blocks unavailable"))
        message: root.model.blocksPageError
        Layout.fillWidth: true
    }

    function blockRows() {
        const blocks = root.model.blocksPageRows || [];
        if (!blocks.length) {
            return [{
                slotRaw: "",
                cells: [
                    { text: "-", width: 96 },
                    { text: "-", width: 72 },
                    { text: root.model.chainPages.sourceEmptyText("blockchain", root.model.blocksPageError, qsTr("No blocks in loaded range")), width: 140, fill: true, monospace: false },
                    { text: "-", width: 72 },
                    { text: "-", width: 140, fill: true },
                    { text: "-", width: 72 }
                ],
                blockHash: "",
                leaderHash: "",
                rawBlock: null,
                selected: false
            }];
        }
        return blocks.map(function (block) {
            const header = block.header || {};
            const proof = header.proof_of_leadership || {};
            const transactions = block.transactions || [];
            const hash = root.model.chainPages.blockHash(block);
            const status = root.model.chainPages.blockStatus(block);
            return {
                slotRaw: String(header.slot || ""),
                cells: [
                    { text: root.numberText(header.slot), width: 96, link: true, copyable: false },
                    { text: root.numberText(block.height || header.height), width: 72 },
                    { text: UiFormat.shortHash(hash), width: 140, fill: true, link: hash.length > 0, copyText: hash },
                    { text: root.numberText(transactions.length), width: 72 },
                    { text: UiFormat.shortHash(proof.leader_key), width: 140, fill: true, copyable: String(proof.leader_key || "").length > 0, copyText: String(proof.leader_key || "") },
                    { text: status, width: 72, tone: root.statusTone(status), monospace: false }
                ],
                blockHash: hash,
                leaderHash: String(proof.leader_key || ""),
                rawBlock: block,
                selected: root.isSelectedBlock(hash)
            };
        });
    }

    function blockchainInfo() {
        return root.model.chainPages.blockchainInfo();
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
        return qsTr("L1 slots %1-%2").arg(root.numberText(root.model.blocksPageSlotFrom)).arg(root.numberText(root.model.blocksPageSlotTo));
    }

    function canLoadNewer() {
        const current = root.chainSlot("slot")
        return root.model.blocksPageSlotTo > 0 && current > 0 && root.model.blocksPageSlotTo < current
    }

    function isSelectedBlock(hash) {
        const detail = root.model.blockDetailValue;
        return detail !== null && String(detail.hash || "") === String(hash || "");
    }

    function numberText(value) {
        return UiFormat.numberText(value);
    }

    function statusTone(value) {
        if (value === "finalized" || value === "confirmed") {
            return "success";
        }
        if (value === "pending") {
            return "warning";
        }
        if (value === "orphaned") {
            return "error";
        }
        return "neutral";
    }
}
