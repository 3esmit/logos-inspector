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
        if (!model.lezBlocksPageRows.length) {
            model.refreshLezBlocksPage();
        }
    }

    ListToolbar {
        theme: root.theme
        loadCount: root.model.lezBlocksPageLimit
        rangeText: root.rangeText()
        canGoNewer: root.model.lezBlocksPageBeforeBlock > 0
        canGoOlder: root.model.lezBlocksPageNextBeforeBlock > 0
        busy: root.model.busy
        Layout.fillWidth: true
        onRefresh: root.model.refreshLezBlocksPage(root.model.lezBlocksPageBeforeBlock > 0 ? root.model.lezBlocksPageBeforeBlock : null)
        onNewer: root.model.newerLezBlocksPage()
        onOlder: root.model.olderLezBlocksPage()
        onLoadCountSelected: function (count) {
            root.model.setLezBlocksPageLimit(count)
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
                columns: [qsTr("L2 block"), qsTr("Header"), qsTr("Tx"), qsTr("Bedrock")]
            }

            Repeater {
                model: root.blockRows()

                BlockRow {
                    required property var modelData

                    theme: root.theme
                    columns: [modelData.block, modelData.header, modelData.tx, modelData.status]
                    blockHash: modelData.blockHash
                    onCellActivated: function (column) {
                        if ((column === 0 || column === 1) && modelData.blockHash.length > 0) {
                            root.model.openReference("indexerBlock", modelData.blockHash)
                        }
                    }
                }
            }
        }
    }

    StatusMessage {
        visible: root.model.lezBlocksPageError.length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("L2 blocks unavailable")
        message: root.model.lezBlocksPageError
        Layout.fillWidth: true
    }

    function blockRows() {
        const blocks = root.model.lezBlocksPageRows || [];
        if (!blocks.length) {
            return [{
                block: "-",
                header: qsTr("No indexed blocks"),
                tx: "-",
                status: "-",
                blockHash: ""
            }];
        }
        return blocks.map(function (block) {
            return {
                block: root.numberText(block.block_id),
                header: root.shortHash(block.header_hash),
                tx: root.numberText(block.tx_count !== undefined ? block.tx_count : ((block.transactions || []).length)),
                status: String(block.bedrock_status || "-"),
                blockHash: String(block.header_hash || "")
            };
        });
    }

    function rangeText() {
        if (root.model.lezBlocksPageBeforeBlock <= 0) {
            return qsTr("Latest L2 blocks");
        }
        return qsTr("Before L2 block %1").arg(root.numberText(root.model.lezBlocksPageBeforeBlock));
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
                    Layout.fillWidth: index === 1
                    onActivated: rowRoot.cellActivated(index)
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header && rowRoot.blockHash.length > 0 && (index === 0 || index === 1);
        }

        function copyValueFor(index) {
            if (index === 1 && rowRoot.blockHash.length > 0) {
                return rowRoot.blockHash;
            }
            return String(rowRoot.columns[index] || "");
        }

        function columnWidth(index) {
            if (index === 0) {
                return 96;
            }
            if (index === 2) {
                return 64;
            }
            if (index === 3) {
                return 98;
            }
            return 220;
        }
    }
}
