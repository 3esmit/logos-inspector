pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property var value: null
    readonly property var detail: normalize(value)

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    ColumnLayout {
        visible: root.detail !== null
        spacing: 6
        Layout.fillWidth: true

        Text {
            text: root.detail ? qsTr("Home > Wallets > %1").arg(root.shortWallet(root.detail.address)) : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: 12
            Layout.fillWidth: true
        }

        Text {
            text: qsTr("Wallet")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 22
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Text {
            text: root.detail ? root.detail.address : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            font.family: "monospace"
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    Text {
        visible: root.detail !== null
        text: qsTr("Receive-side only. Logos transfers carry recipient public keys in outputs[].pk; the spend side references opaque note IDs, so we cannot yet show who sent these.")
        color: root.theme.textMuted
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 13
        Layout.fillWidth: true
    }

    GridLayout {
        visible: root.detail !== null
        columns: root.width < 720 ? 2 : 4
        columnSpacing: 12
        rowSpacing: 12
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            label: qsTr("Total received")
            value: root.detail ? root.coinText(root.detail.total_received) : "-"
            delta: qsTr("Receive side")
            deltaColor: root.theme.accent
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Outputs")
            value: root.detail ? root.numberText(root.detail.outputs) : "-"
            delta: qsTr("Transfer outputs")
            deltaColor: root.theme.success
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Source txs")
            value: root.detail ? root.numberText(root.detail.txs) : "-"
            delta: qsTr("Distinct hashes")
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Last seen slot")
            value: root.detail ? root.numberText(root.detail.last_slot) : "-"
            delta: qsTr("Newest transfer")
        }
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 10
        Layout.fillWidth: true

        Text {
            text: qsTr("Incoming transfers")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            font.weight: Font.DemiBold
            Layout.fillWidth: true
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

                TransferRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Slot"), qsTr("Tx hash"), qsTr("Block"), qsTr("Value")]
                }

                Repeater {
                    model: root.transferRows()

                    TransferRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.slot, modelData.tx, modelData.block, modelData.value]
                        txHash: modelData.txHash
                        blockHash: modelData.blockHash
                        onCellActivated: function (column) {
                            if (column === 1) {
                                root.model.openTransaction(modelData.txHash)
                            } else if (column === 2) {
                                root.model.openIndexerBlock(modelData.blockHash)
                            }
                        }
                    }
                }
            }
        }
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value) || value.type !== "wallet") {
            return null
        }
        return {
            address: String(value.address || value.wallet || ""),
            total_received: value.total_received,
            txs: value.txs,
            outputs: value.outputs,
            last_slot: value.last_slot,
            source: String(value.source || ""),
            transfers: Array.isArray(value.transfers) ? value.transfers : [],
            raw: value.raw || null
        }
    }

    function transferRows() {
        const rows = root.detail ? root.detail.transfers : []
        if (!rows.length) {
            return [{
                slot: "-",
                tx: qsTr("No incoming transfers in loaded range"),
                block: "-",
                value: "-",
                txHash: "",
                blockHash: ""
            }]
        }
        return rows.map(function (transfer) {
            const txHash = String(transfer.tx_hash || transfer.hash || "")
            const blockHash = String(transfer.block_hash || transfer.block || "")
            return {
                slot: root.numberText(transfer.slot),
                tx: root.shortHash(txHash),
                block: root.shortHash(blockHash),
                value: root.coinText(transfer.value),
                txHash: txHash,
                blockHash: blockHash
            }
        })
    }

    function coinText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return (numeric / 100).toLocaleString(Qt.locale(), "f", 2) + "E"
        }
        return String(value)
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return numeric.toLocaleString(Qt.locale())
        }
        return String(value)
    }

    function shortHash(value) {
        const text = String(value || "")
        if (text.length <= 16) {
            return text.length ? text : "-"
        }
        return text.slice(0, 8) + "..." + text.slice(-6)
    }

    function shortWallet(value) {
        const text = String(value || "")
        if (text.length <= 18) {
            return text.length ? text : "-"
        }
        return text.slice(0, 12) + "..." + text.slice(-8)
    }

    component TransferRow: Item {
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
            return !rowRoot.header
                && ((index === 1 && rowRoot.txHash.length > 0)
                    || (index === 2 && rowRoot.blockHash.length > 0))
        }

        function columnWidth(index) {
            if (index === 0) {
                return 96
            }
            if (index === 3) {
                return 92
            }
            return 180
        }
    }
}
