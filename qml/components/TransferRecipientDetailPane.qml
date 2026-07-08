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

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    ColumnLayout {
        visible: root.detail !== null
        spacing: 6
        Layout.fillWidth: true

        RowLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Text {
                text: root.detail && root.detail.source === "account_refs" ? qsTr("Account reference") : qsTr("Transfer recipient")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 22
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            ActionButton {
                visible: root.detail !== null && root.detail.source === "account_refs" && root.detail.address.length > 0
                theme: root.theme
                text: qsTr("Open account state")
                Layout.preferredWidth: 156
                onClicked: root.model.entityNavigation.openAccount(root.detail.address)
            }
        }

        SourceStrip {
            theme: root.theme
            sources: root.detail ? [qsTr("L2 Indexer"), root.sourceLabel(root.detail.source), qsTr("key: %1").arg(root.detail.source === "account_refs" ? qsTr("account id") : qsTr("recipient"))] : []
            Layout.fillWidth: true
        }

        LinkCell {
            theme: root.theme
            text: root.detail ? root.detail.address : ""
            copyable: root.detail !== null && root.detail.address.length > 0
            copyText: root.detail ? root.detail.address : ""
            monospace: true
            wrap: true
            textColor: root.theme.textMuted
            textPixelSize: 12
            Layout.fillWidth: true
        }
    }

    Text {
        visible: root.detail !== null
        text: root.detail && root.detail.source === "account_refs"
            ? qsTr("Indexer fallback from account references. Values here are not decoded transfer receipts.")
            : qsTr("Indexer-derived L2 transfer activity. This is not a local wallet directory; local/private wallet state requires explicit wallet integration.")
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
            label: root.detail && root.detail.source === "account_refs" ? qsTr("Referenced balance") : qsTr("Total received")
            value: root.detail && root.detail.source === "account_refs" ? root.valueText(root.detail.total_received) : (root.detail ? root.coinText(root.detail.total_received) : "-")
            delta: root.detail && root.detail.source === "account_refs" ? qsTr("Not decoded as transfer") : qsTr("Observed amount")
            deltaColor: root.theme.accent
        }

        MetricCard {
            theme: root.theme
            label: root.detail && root.detail.source === "account_refs" ? qsTr("References") : qsTr("Outputs")
            value: root.detail ? root.numberText(root.detail.source === "account_refs" ? root.detail.references : root.detail.outputs) : "-"
            delta: root.detail && root.detail.source === "account_refs" ? qsTr("Account mentions") : qsTr("Transfer outputs")
            deltaColor: root.theme.success
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Source txs")
            value: root.detail ? root.numberText(root.detail.txs) : "-"
            delta: qsTr("Loaded window")
        }

        MetricCard {
            theme: root.theme
            label: root.detail && root.detail.source === "account_refs" ? qsTr("Newest reference") : qsTr("Last L2 block")
            value: root.detail ? root.numberText(root.detail.last_slot) : "-"
            delta: root.detail && root.detail.source === "account_refs" ? qsTr("Loaded account window") : qsTr("Newest observed transfer")
        }
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 10
        Layout.fillWidth: true

        DetailSection {
            theme: root.theme
            title: qsTr("Source")
            rows: root.sourceRows()
            labelWidth: 132
            surfaceColor: root.theme.surface
            onLinkActivated: function (kind, value) {
                root.model.entityNavigation.openReference(kind, value)
            }
        }

        DetailSection {
            visible: root.detail && root.detail.source === "account_refs"
            theme: root.theme
            title: qsTr("Account references")
            rows: root.accountReferenceRows()
            labelWidth: 132
            surfaceColor: root.theme.surface
            onLinkActivated: function (kind, value) {
                root.model.entityNavigation.openReference(kind, value)
            }
        }

        Text {
            text: root.detail && root.detail.source === "account_refs" ? qsTr("Transactions") : qsTr("Transactions")
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
                { text: qsTr("Tx"), width: 180 },
                { text: qsTr("L2 block"), width: 96, fill: true },
                { text: qsTr("Header"), width: 180, fill: true },
                { text: qsTr("Amount"), width: 92 }
            ]
            rows: root.transferRows()
            onCellActivated: function (row, column, cell, rowData) {
                if (column === 0 && rowData.txHash.length > 0) {
                    root.model.entityNavigation.openTransaction(rowData.txHash)
                } else if (column === 2 && rowData.blockHash.length > 0) {
                    root.model.entityNavigation.openIndexerBlock(rowData.blockHash)
                }
            }
        }

        DetailSection {
            theme: root.theme
            title: qsTr("Raw extraction")
            rows: root.rawRows()
            labelWidth: 132
            surfaceColor: root.theme.surface
            onLinkActivated: function (kind, value) {
                root.model.entityNavigation.openReference(kind, value)
            }
        }
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value) || value.type !== "transfer_recipient") {
            return null
        }
        return {
            address: String(value.address || value.account_ref || value.recipient || ""),
            total_received: value.total_received,
            txs: value.txs,
            outputs: value.outputs,
            references: value.references,
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
                cells: [
                    { text: qsTr("No observed transfers in loaded range"), width: 180, monospace: false },
                    { text: "-", width: 96, fill: true },
                    { text: "-", width: 180, fill: true },
                    { text: "-", width: 92 }
                ],
                txHash: "",
                blockHash: ""
            }]
        }
        return rows.map(function (transfer) {
            const txHash = String(transfer.tx_hash || transfer.hash || "")
            const blockHash = String(transfer.block_hash || transfer.block || "")
            return {
                cells: [
                    { text: UiFormat.shortHash(txHash), width: 180, link: txHash.length > 0, copyText: txHash },
                    { text: root.numberText(transfer.slot), width: 96, fill: true },
                    { text: UiFormat.shortHash(blockHash), width: 180, fill: true, link: blockHash.length > 0, copyText: blockHash },
                    { text: root.detail && root.detail.source === "account_refs" ? root.valueText(transfer.value) : root.coinText(transfer.value), width: 92 }
                ],
                txHash: txHash,
                blockHash: blockHash
            }
        })
    }

    function sourceRows() {
        if (!root.detail) {
            return []
        }
        return [
            { label: qsTr("Scan method"), value: root.sourceLabel(root.detail.source), copyText: "" },
            { label: qsTr("Block window"), value: root.model.transferActivityBeforeBlock > 0 ? qsTr("Before L2 block %1").arg(root.numberText(root.model.transferActivityBeforeBlock)) : qsTr("Latest finalized L2 blocks"), copyText: "" },
            { label: qsTr("Confidence"), value: root.detail.source === "transfer_outputs" ? qsTr("decoded transfer output") : qsTr("fallback account reference"), copyText: "" }
        ]
    }

    function accountReferenceRows() {
        if (!root.detail || !root.detail.address.length) {
            return []
        }
        return [
            { label: qsTr("Affected"), value: root.detail.address, copyText: root.detail.address, linkKind: "account", linkValue: root.detail.address }
        ]
    }

    function rawRows() {
        if (!root.detail) {
            return []
        }
        return [
            { label: qsTr("Source"), value: root.detail.source || "-", copyText: "" },
            { label: qsTr("Loaded rows"), value: qsTr("%1 transaction row(s)").arg(root.numberText(root.detail.transfers.length)), copyText: "" }
        ]
    }

    function sourceLabel(source) {
        const value = String(source || "")
        if (value === "transfer_outputs") {
            return qsTr("transfer-output scan")
        }
        if (value === "account_refs") {
            return qsTr("account-ref fallback")
        }
        return value.length ? value : qsTr("unknown source")
    }

    function coinText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const text = String(value)
        if (/^-?[0-9]+$/.test(text) && text.length <= 15) {
            const numeric = Number(text)
            return numeric.toLocaleString(Qt.locale(), "f", 0)
        }
        return text
    }

    function valueText(value) {
        return UiFormat.valueText(value)
    }

    function numberText(value) {
        return UiFormat.numberText(value)
    }
}
