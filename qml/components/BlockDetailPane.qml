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
            text: root.detail ? qsTr("Home > Blocks > %1").arg(root.shortHash(root.detail.hash)) : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            font.pixelSize: 12
            Layout.fillWidth: true
        }

        Text {
            text: root.detail ? qsTr("Block at slot %1").arg(root.valueText(root.detail.slot)) : ""
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 22
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Text {
            text: root.detail ? root.detail.hash : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            font.family: "monospace"
            font.pixelSize: 12
            Layout.fillWidth: true
        }
    }

    SectionBlock {
        theme: root.theme
        title: qsTr("Overview")
        rows: root.overviewRows()
    }

    Text {
        visible: root.detail !== null
        text: qsTr("Cryptarchia PoL leader key - not a wallet address. By design it cannot be linked to a stable operator identity.")
        color: root.theme.textDim
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
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

                TransactionRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Index"), qsTr("Tx hash"), qsTr("Ops")]
                }

                Repeater {
                    model: root.transactionRows()

                    TransactionRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.index, modelData.hashText, modelData.ops]
                        transaction: modelData.transaction
                        onActivated: root.model.openBlockchainTransaction(modelData.transaction, root.detail)
                    }
                }
            }
        }
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value) || (value.type !== "blockchain_block" && value.type !== "indexer_block")) {
            return null
        }
        return {
            hash: String(value.hash || ""),
            parent: String(value.parent || ""),
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
            { label: qsTr("Parent"), value: root.valueText(root.detail.parent), monospace: true, linkKind: "block", linkValue: root.detail.parent },
            { label: qsTr("Slot"), value: root.valueText(root.detail.slot), monospace: true, linkKind: "block", linkValue: root.valueText(root.detail.slot) },
            { label: qsTr("Height"), value: root.valueText(root.detail.height), monospace: true },
            { label: qsTr("Status"), value: root.valueText(root.detail.status), monospace: false },
            { label: qsTr("Version"), value: root.valueText(root.detail.version), monospace: true },
            { label: qsTr("Block root"), value: root.valueText(root.detail.block_root), monospace: true },
            { label: qsTr("Voucher cm"), value: root.valueText(root.detail.voucher_cm), monospace: true },
            { label: qsTr("Entropy"), value: root.valueText(root.detail.entropy), monospace: true },
            { label: qsTr("Signature"), value: root.detail.signature.length ? root.detail.signature : qsTr("- (not in this source)"), monospace: true },
            { label: qsTr("Leader key"), value: root.valueText(root.detail.leader_key), monospace: true }
        ]
    }

    function transactionRows() {
        const rows = root.detail ? root.detail.transactions : []
        if (!rows.length) {
            return [{
                index: "-",
                hashText: qsTr("No transactions"),
                ops: "-",
                transaction: null
            }]
        }
        return rows.map(function (tx) {
            return {
                index: root.valueText(tx.index),
                hashText: root.shortHash(tx.hash),
                ops: root.valueText(tx.ops),
                transaction: tx
            }
        })
    }

    function valueText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        if (typeof value === "number") {
            return value.toLocaleString(Qt.locale())
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

    component SectionBlock: ColumnLayout {
        id: sectionRoot

        required property Theme theme
        property string title: ""
        property var rows: []

        visible: rows.length > 0
        spacing: 6
        Layout.fillWidth: true

        Text {
            visible: sectionRoot.title.length > 0
            text: sectionRoot.title
            color: sectionRoot.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
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
                        linkKind: String(modelData.linkKind || "")
                        linkValue: String(modelData.linkValue || "")
                        monospace: modelData.monospace !== undefined ? modelData.monospace : true
                        onActivated: root.model.openReference(modelData.linkKind, modelData.linkValue)
                    }
                }
            }
        }
    }

    component DetailRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string linkKind: ""
        property string linkValue: ""
        property bool monospace: true
        signal activated()

        Layout.fillWidth: true
        implicitHeight: Math.max(42, rowGrid.implicitHeight + 18)

        GridLayout {
            id: rowGrid

            anchors.fill: parent
            anchors.leftMargin: 12
            anchors.rightMargin: 12
            anchors.topMargin: 8
            anchors.bottomMargin: 8
            columns: 2
            columnSpacing: 14
            rowSpacing: 3

            Text {
                text: rowRoot.label
                color: rowRoot.theme.textMuted
                textFormat: Text.PlainText
                font.pixelSize: 11
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 128
                Layout.alignment: Qt.AlignTop
            }

            Text {
                text: rowRoot.value
                color: rowRoot.linkKind.length ? rowRoot.theme.accent : rowRoot.theme.text
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: rowRoot.monospace ? "monospace" : ""
                font.pixelSize: 12
                font.underline: rowRoot.linkKind.length > 0
                Layout.fillWidth: true

                MouseArea {
                    anchors.fill: parent
                    enabled: rowRoot.linkKind.length > 0
                    cursorShape: Qt.PointingHandCursor
                    onClicked: rowRoot.activated()
                }
            }
        }
    }

    component TransactionRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property var transaction: null
        property bool header: false
        signal activated()

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
            columns: 3
            columnSpacing: 10

            Repeater {
                model: 3

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
                    Layout.fillWidth: index === 1

                    MouseArea {
                        anchors.fill: parent
                        enabled: rowRoot.linkFor(parent.index)
                        cursorShape: Qt.PointingHandCursor
                        onClicked: rowRoot.activated()
                    }
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header && index === 1 && rowRoot.transaction !== null && String(rowRoot.transaction.hash || "").length > 0
        }

        function columnWidth(index) {
            if (index === 0 || index === 2) {
                return 72
            }
            return 240
        }
    }
}
