pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import ".."
import "../../state"
import "../../theme"

ColumnLayout {
    id: root

    required property Theme theme
    property var block: null
    property AppModel modelRef

    visible: root.block !== null
    spacing: root.theme.gap
    Layout.fillWidth: true

    SequencerDetailSection {
        theme: root.theme
        title: qsTr("Block")
        rows: root.overviewRows()
        modelRef: root.modelRef
    }

    StatusMessage {
        visible: root.block && String(root.block.decode_warning || "").length > 0
        theme: root.theme
        tone: "warning"
        title: qsTr("Decode warning")
        message: String(root.block ? root.block.decode_warning || "" : "")
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.block !== null
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        Text {
            text: qsTr("Transactions (%1)").arg(root.valueText(root.block ? root.block.tx_count : 0))
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: root.theme.primaryText
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

                SequencerTransactionRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Index"), qsTr("Hash"), qsTr("Kind"), qsTr("Program")]
                }

                Repeater {
                    model: root.transactionRows()

                    SequencerTransactionRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.index, modelData.hashText, modelData.kind, modelData.programText]
                        hash: modelData.hash
                        program: modelData.program
                        modelRef: root.modelRef
                    }
                }
            }
        }
    }

    function overviewRows() {
        const value = root.block || {}
        return [
            { label: qsTr("L2 block ID"), value: root.valueText(value.block_id), monospace: true, linkKind: "lezBlock", linkValue: root.valueText(value.block_id) },
            { label: qsTr("Header hash"), value: root.valueText(value.header_hash), monospace: true, linkKind: "", linkValue: "" },
            { label: qsTr("Previous header hash"), value: root.valueText(value.parent_hash), monospace: true, linkKind: "", linkValue: "" },
            { label: qsTr("Timestamp"), value: root.valueText(value.timestamp), monospace: true },
            { label: qsTr("Bedrock status"), value: root.valueText(value.bedrock_status), monospace: false },
            { label: qsTr("Transactions"), value: root.valueText(value.tx_count), monospace: true }
        ]
    }

    function transactionRows() {
        const transactions = root.block && Array.isArray(root.block.transactions) ? root.block.transactions : []
        if (!transactions.length) {
            return [{
                index: "-",
                hashText: qsTr("No transactions"),
                kind: "-",
                programText: "-",
                hash: "",
                program: ""
            }]
        }
        return transactions.map(function (tx, index) {
            return {
                index: root.valueText(index),
                hashText: root.shortHash(tx.hash),
                kind: root.valueText(tx.kind),
                programText: tx.program_id_hex ? root.shortHash(tx.program_id_hex) : "-",
                hash: String(tx.hash || ""),
                program: String(tx.program_id_hex || "")
            }
        })
    }

    function valueText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        if (typeof value === "number") {
            return value % 1 === 0 ? value.toLocaleString(Qt.locale(), "f", 0) : String(value)
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
}
