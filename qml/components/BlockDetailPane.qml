pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Layouts
import "../state"
import "../theme"
import "common"
import "../utils/UiFormat.js" as UiFormat

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property var value: null
    readonly property var detail: normalize(value)
    readonly property var favoriteEntry: root.detail ? root.model.favoriteBlockEntry(root.detail) : null

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    ColumnLayout {
        visible: root.detail !== null
        spacing: 6
        Layout.fillWidth: true

        Text {
            text: root.detail ? root.titleText() : ""
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 22
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        LinkCell {
            text: root.detail ? root.detail.hash : ""
            theme: root.theme
            link: root.detail !== null && root.detail.hash.length > 0
            copyable: root.detail !== null && root.detail.hash.length > 0
            copyText: root.detail ? root.detail.hash : ""
            monospace: true
            wrap: true
            textColor: root.theme.textMuted
            textPixelSize: 12
            Layout.fillWidth: true
            onActivated: root.model.openReference(root.isLezBlock() ? "indexerBlock" : "block", root.detail.hash)
        }

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: root.favoriteButtonText()
                selected: root.model.isFavoriteEntry(root.favoriteEntry)
                enabled: root.favoriteEntry !== null
                Layout.preferredWidth: 118
                accessibleName: root.favoriteButtonAccessibleName()
                onClicked: root.model.toggleFavorite(root.favoriteEntry)
            }

            Text {
                text: root.isLezBlock() ? qsTr("LEZ block") : qsTr("Bedrock block")
                color: root.theme.textDim
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.secondaryText
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
            }
        }
    }

    GridLayout {
        visible: root.detail !== null
        columns: root.width < 760 ? 2 : 4
        columnSpacing: 12
        rowSpacing: 12
        Layout.fillWidth: true

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Source")
            value: root.sourceText()
            delta: root.sourceDetailText()
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Status")
            value: root.detail ? UiFormat.valueText(root.detail.status) : "-"
            delta: root.detail ? root.positionText() : qsTr("Slot")
            deltaColor: root.statusColor(root.detail ? root.detail.status : "")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: qsTr("Transactions")
            value: root.detail ? UiFormat.valueText(root.detail.transactions.length) : "-"
            delta: qsTr("In this block")
        }

        MetricCard {
            theme: root.theme
            compact: true
            label: root.isLezBlock() ? qsTr("Block ID") : qsTr("Height")
            value: root.detail ? UiFormat.valueText(root.detail.height) : "-"
            delta: root.isLezBlock() ? qsTr("LEZ block id") : qsTr("Chain height")
        }
    }

    DetailSection {
        theme: root.theme
        title: qsTr("Overview")
        rows: root.overviewRows()
        labelWidth: 128
        surfaceColor: root.theme.surface
        onLinkActivated: function (kind, value) {
            root.model.openReference(kind, value)
        }
    }

    StatusMessage {
        visible: root.detail !== null && !root.isIndexerBlock() && root.detail.leader_key.length > 0
        theme: root.theme
        tone: "info"
        title: qsTr("Leader key")
        message: qsTr("Cryptarchia PoL leader key is not a wallet address and cannot be linked to a stable operator identity.")
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 10
        Layout.fillWidth: true

        Text {
            text: root.detail ? qsTr("Transactions (%1)").arg(root.detail.transactions.length) : ""
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        DataTableFrame {
            theme: root.theme
            Layout.fillWidth: true
            headerCells: [
                { text: qsTr("Index"), width: 72 },
                { text: qsTr("Tx hash"), width: 240, fill: true },
                { text: qsTr("Ops"), width: 72 }
            ]
            rows: root.transactionRows()
            onCellActivated: function (row, column, cell, rowData) {
                if (!rowData.transaction) {
                    return
                }
                if (root.isLezBlock()) {
                    root.model.openTransaction(rowData.transaction.hash)
                } else {
                    root.model.openBlockchainTransaction(rowData.transaction, root.detail)
                }
            }
        }
    }

    SocialPanel {
        visible: root.detail !== null
        theme: root.theme
        model: root.model
        title: qsTr("Block comments")
        topic: root.socialTopic()
        Layout.fillWidth: true
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value) || (value.type !== "blockchain_block" && value.type !== "indexer_block" && value.type !== "sequencer_block")) {
            return null
        }
        return {
            type: String(value.type || ""),
            hash: String(value.hash || ""),
            parent: String(value.parent || ""),
            block_id: value.block_id,
            slot: value.slot,
            height: value.height,
            status: String(value.status || ""),
            version: value.version,
            block_root: String(value.block_root || ""),
            voucher_cm: String(value.voucher_cm || ""),
            entropy: String(value.entropy || ""),
            signature: String(value.signature || ""),
            leader_key: String(value.leader_key || ""),
            transactions: Array.isArray(value.transactions) ? value.transactions : [],
            raw: value.raw || null
        }
    }

    function overviewRows() {
        if (!root.detail) {
            return []
        }
        return [
            { label: root.isLezBlock() ? qsTr("Parent LEZ block") : qsTr("Parent"), value: UiFormat.valueText(root.detail.parent), monospace: true, linkKind: root.detail.parent.length ? (root.isLezBlock() ? "indexerBlock" : "block") : "", linkValue: root.detail.parent, copyable: root.detail.parent.length > 0 },
            { label: root.isLezBlock() ? qsTr("LEZ block ID") : qsTr("Slot"), value: UiFormat.valueText(root.detail.slot), monospace: true, linkKind: UiFormat.valueText(root.detail.slot) !== "-" && !root.isLezBlock() ? "block" : "", linkValue: root.detail.slot },
            { label: qsTr("Height"), value: UiFormat.valueText(root.detail.height), monospace: true },
            { label: qsTr("Status"), value: UiFormat.valueText(root.detail.status), monospace: false },
            { label: qsTr("Version"), value: root.isLezBlock() ? qsTr("- (not in this source)") : UiFormat.valueText(root.detail.version), monospace: true },
            { label: qsTr("Block root"), value: UiFormat.valueText(root.detail.block_root), monospace: true, copyable: root.detail.block_root.length > 0 },
            { label: qsTr("Voucher cm"), value: UiFormat.valueText(root.detail.voucher_cm), monospace: true, copyable: root.detail.voucher_cm.length > 0 },
            { label: qsTr("Entropy"), value: UiFormat.valueText(root.detail.entropy), monospace: true, copyable: root.detail.entropy.length > 0 },
            { label: qsTr("Signature"), value: root.detail.signature.length ? root.detail.signature : qsTr("- (not in this source)"), monospace: true, copyable: root.detail.signature.length > 0 },
            { label: qsTr("Leader key"), value: UiFormat.valueText(root.detail.leader_key), monospace: true, copyable: root.detail.leader_key.length > 0 }
        ]
    }

    function transactionRows() {
        const rows = root.detail ? root.detail.transactions : []
        if (!rows.length) {
            return [{
                cells: [
                    { text: "-", width: 72 },
                    { text: qsTr("No transactions"), width: 240, fill: true, monospace: false },
                    { text: "-", width: 72 }
                ],
                transaction: null
            }]
        }
        return rows.map(function (tx) {
            const hash = String(tx.hash || "")
            return {
                cells: [
                    { text: UiFormat.valueText(tx.index), width: 72 },
                    { text: UiFormat.shortHash(hash), width: 240, fill: true, link: hash.length > 0, copyText: hash },
                    { text: UiFormat.valueText(tx.ops), width: 72 }
                ],
                transaction: tx
            }
        })
    }

    function sourceText() {
        if (!root.detail) {
            return "-"
        }
        if (root.detail.type === "indexer_block") {
            return qsTr("Indexer")
        }
        return root.detail.type === "sequencer_block" ? qsTr("Sequencer") : qsTr("Node")
    }

    function sourceDetailText() {
        if (!root.detail) {
            return "-"
        }
        if (root.detail.type === "indexer_block") {
            return qsTr("Indexer lookup")
        }
        if (root.detail.type === "sequencer_block") {
            return qsTr("Sequencer RPC")
        }
        return qsTr("Node block")
    }

    function isIndexerBlock() {
        return root.detail !== null && root.detail.type === "indexer_block"
    }

    function isLezBlock() {
        return root.detail !== null && (root.detail.type === "indexer_block" || root.detail.type === "sequencer_block")
    }

    function titleText() {
        if (!root.detail) {
            return ""
        }
        return root.isLezBlock()
            ? qsTr("LEZ block %1").arg(UiFormat.valueText(root.detail.block_id))
            : qsTr("Block at slot %1").arg(UiFormat.valueText(root.detail.slot))
    }

    function positionText() {
        if (!root.detail) {
            return qsTr("Slot")
        }
        return root.isLezBlock()
            ? qsTr("Block ID %1").arg(UiFormat.valueText(root.detail.block_id))
            : qsTr("Slot %1").arg(UiFormat.valueText(root.detail.slot))
    }

    function statusColor(value) {
        const status = String(value || "")
        if (status === "finalized" || status === "confirmed") {
            return root.theme.success
        }
        if (status === "pending") {
            return root.theme.warning
        }
        return root.theme.textMuted
    }

    function favoriteButtonText() {
        return root.model.isFavoriteEntry(root.favoriteEntry) ? qsTr("Favorited") : qsTr("Favorite")
    }

    function favoriteButtonAccessibleName() {
        return root.model.isFavoriteEntry(root.favoriteEntry) ? qsTr("Remove block from favorites") : qsTr("Add block to favorites")
    }

    function socialTopic() {
        if (!root.detail) {
            return ""
        }
        const id = root.isLezBlock()
            ? (String(root.detail.block_id || "").length ? String(root.detail.block_id) : root.detail.hash)
            : (root.detail.hash.length ? root.detail.hash : String(root.detail.slot || ""))
        return root.model.socialCommentTopic(root.isLezBlock() ? "lez" : "cryptarchia", "block", id)
    }

}
