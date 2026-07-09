import QtQuick
import QtTest
import "../../qml/state/chain/InspectionReportRows.js" as InspectionReportRows

TestCase {
    id: testRoot

    name: "InspectionReportRows"

    QtObject {
        id: projectionRoot

        property var transferActivityRows: [
            { recipient: "0xabc", received: 42, txs: 2, outputs: 3, source: "scan" }
        ]
        property var transferActivityOverflowRows: [
            { recipient: "0xdef", received: 8 }
        ]
        property var channelsPageRows: [
            { channel_id: "0xchan", last_tx_hash: "0xtx", last_block_hash: "0xblock", accredited_keys: ["key-a", "key-b"] }
        ]

        function sortedIndexerBlocks(blocks) {
            const rows = Array.isArray(blocks) ? blocks.slice(0) : []
            rows.sort(function (left, right) {
                return indexerBlockId(right) - indexerBlockId(left)
            })
            return rows
        }

        function indexerBlockId(block) { return Number(block && block.block_id !== undefined ? block.block_id : 0) }

        function indexerBlockHash(block) { return String(block && block.header_hash ? block.header_hash : "") }

        function canonicalProgramIdHex(value) {
            const text = String(value || "")
            return text.indexOf("0x") === 0 ? text.slice(2) : text
        }

        function normalizedHexText(value) { return String(value || "").toLowerCase() }

        function normalizedHashOrValue(value) { return String(value || "").toLowerCase() }
    }

    function test_lez_transaction_rows_have_stable_contract() {
        const rows = InspectionReportRows.lezTransactionRowsFromBlocks(projectionRoot, [{
            block_id: 3,
            header_hash: "block-3",
            transactions: [{ tx_hash: "tx-3", message: { program_id_hex: "0xabc" }, ops: [{}, {}] }]
        }, {
            block_id: 9,
            header_hash: "block-9",
            transactions: [{ hash: "tx-9", program_id_hex: "0xdef", bytecode_len: 4 }]
        }])

        compare(rows.length, 2)
        compare(rows[0].block_id, 9)
        compare(rows[0].hash, "tx-9")
        compare(rows[0].program_id_hex, "def")
        compare(rows[0].ops, 4)
        compare(rows[1].block_hash, "block-3")
        compare(rows[1].ops, 2)
    }

    function test_transfer_recipient_and_channel_detail_contracts() {
        const recipient = InspectionReportRows.transferRecipientDetailById(projectionRoot, "0xabc")
        const channel = InspectionReportRows.channelDetailById(projectionRoot, "0xchan")

        compare(recipient.type, "transfer_recipient")
        compare(recipient.address, "0xabc")
        compare(recipient.total_received, 42)
        compare(channel.type, "channel")
        compare(channel.channel_id, "0xchan")
        compare(channel.tx_hash, "0xtx")
        compare(channel.key_values.length, 2)
    }
}
