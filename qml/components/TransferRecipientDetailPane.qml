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
                visible: root.detail !== null && root.detail.address.length > 0
                theme: root.theme
                text: qsTr("Open account state")
                Layout.preferredWidth: 156
                onClicked: root.model.openAccount(root.detail.address)
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
        text: qsTr("Indexer-derived L2 transfer activity. This is not a local wallet directory; local/private wallet state requires explicit wallet integration.")
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
            delta: qsTr("Observed amount")
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
            delta: qsTr("Loaded window")
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Last L2 block")
            value: root.detail ? root.numberText(root.detail.last_slot) : "-"
            delta: qsTr("Newest observed transfer")
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
        }

        DetailSection {
            visible: root.detail && root.detail.source === "account_refs"
            theme: root.theme
            title: qsTr("Account references")
            rows: root.accountReferenceRows()
        }

        Text {
            text: root.detail && root.detail.source === "account_refs" ? qsTr("Transactions") : qsTr("Transactions")
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
                    columns: [qsTr("Tx"), qsTr("L2 block"), qsTr("Header"), qsTr("Amount")]
                }

                Repeater {
                    model: root.transferRows()

                    TransferRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.tx, modelData.slot, modelData.block, modelData.value]
                        txHash: modelData.txHash
                        blockHash: modelData.blockHash
                        onCellActivated: function (column) {
                            if (column === 0) {
                                root.model.openTransaction(modelData.txHash)
                            } else if (column === 2) {
                                root.model.openIndexerBlock(modelData.blockHash)
                            }
                        }
                    }
                }
            }
        }

        DetailSection {
            theme: root.theme
            title: qsTr("Raw extraction")
            rows: root.rawRows()
        }
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value) || value.type !== "transfer_recipient") {
            return null
        }
        return {
            address: String(value.address || value.recipient || ""),
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
                tx: qsTr("No observed transfers in loaded range"),
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
            return numeric.toLocaleString(Qt.locale(), "f", 0)
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

    function shortRecipient(value) {
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

                LinkCell {
                    required property int index

                    theme: rowRoot.theme
                    text: String(rowRoot.columns[index] || "-")
                    header: rowRoot.header
                    link: rowRoot.linkFor(index)
                    copyable: rowRoot.copyValueFor(index).length > 0 && !rowRoot.header
                    copyText: rowRoot.copyValueFor(index)
                    monospace: !rowRoot.header
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 1 || index === 2
                    onActivated: rowRoot.cellActivated(index)
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header
                && ((index === 0 && rowRoot.txHash.length > 0)
                    || (index === 2 && rowRoot.blockHash.length > 0))
        }

        function copyValueFor(index) {
            if (index === 0 && rowRoot.txHash.length > 0) {
                return rowRoot.txHash
            }
            if (index === 2 && rowRoot.blockHash.length > 0) {
                return rowRoot.blockHash
            }
            return String(rowRoot.columns[index] || "")
        }

        function columnWidth(index) {
            if (index === 3) {
                return 92
            }
            if (index === 1) {
                return 96
            }
            return 180
        }
    }

    component DetailSection: ColumnLayout {
        id: sectionRoot

        required property Theme theme
        property string title: ""
        property var rows: []

        visible: rows.length > 0
        spacing: 6
        Layout.fillWidth: true

        Text {
            text: sectionRoot.title
            color: sectionRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: sectionRoot.theme.primaryText
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Frame {
            padding: 0
            Layout.fillWidth: true

            background: Rectangle {
                color: sectionRoot.theme.surface
                radius: sectionRoot.theme.radius
                border.width: 1
                border.color: sectionRoot.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: 0

                Repeater {
                    model: sectionRoot.rows

                    DetailRow {
                        required property var modelData

                        theme: sectionRoot.theme
                        label: String(modelData.label || "")
                        value: String(modelData.value || "-")
                        copyText: String(modelData.copyText !== undefined ? modelData.copyText : modelData.value || "")
                        linkKind: String(modelData.linkKind || "")
                        linkValue: String(modelData.linkValue || "")
                        onActivated: root.model.openReference(linkKind, linkValue)
                    }
                }
            }
        }
    }

    component DetailRow: Item {
        id: detailRowRoot

        required property Theme theme
        property string label: ""
        property string value: "-"
        property string copyText: value
        property string linkKind: ""
        property string linkValue: ""
        signal activated()

        Layout.fillWidth: true
        implicitHeight: Math.max(40, rowBody.implicitHeight + 16)

        GridLayout {
            id: rowBody

            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            columns: 2
            columnSpacing: 12

            Text {
                text: detailRowRoot.label
                color: detailRowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: detailRowRoot.theme.labelText
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 132
            }

            LinkCell {
                theme: detailRowRoot.theme
                text: detailRowRoot.value
                link: detailRowRoot.linkKind.length > 0
                copyable: detailRowRoot.copyText.length > 0
                copyText: detailRowRoot.copyText
                monospace: true
                wrap: true
                Layout.fillWidth: true
                onActivated: detailRowRoot.activated()
            }
        }
    }
}
