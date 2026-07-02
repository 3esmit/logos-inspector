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
            text: root.detail && root.detail.decode_only ? qsTr("Account data decode") : qsTr("Account")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 22
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Text {
            visible: root.detail && root.detail.account_id.length > 0
            text: root.detail ? root.detail.account_id : ""
            color: root.theme.textMuted
            textFormat: Text.PlainText
            wrapMode: Text.WrapAnywhere
            font.family: "monospace"
            font.pixelSize: 12
            Layout.fillWidth: true
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
            label: qsTr("Data bytes")
            value: root.detail ? root.numberText(root.dataBytes(root.detail.data_hex)) : "-"
            delta: root.detail && root.detail.data_hex.length ? qsTr("Raw account data") : qsTr("No hex")
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Related txs")
            value: root.detail ? root.numberText(root.detail.related_transactions.length) : "-"
            delta: root.detail && root.detail.related_transactions_error.length ? qsTr("Indexer error") : qsTr("Indexer links")
            deltaColor: root.detail && root.detail.related_transactions_error.length ? root.theme.warning : root.theme.accent
        }

        MetricCard {
            theme: root.theme
            label: qsTr("IDL type")
            value: root.detail && root.detail.decode ? root.detail.decode.account_type : "-"
            delta: root.detail && root.detail.decode ? qsTr("Decoded") : qsTr("No decode")
            deltaColor: root.detail && root.detail.decode ? root.theme.success : root.theme.textMuted
        }

        MetricCard {
            theme: root.theme
            label: qsTr("Consumed")
            value: root.detail && root.detail.decode ? qsTr("%1/%2").arg(root.numberText(root.detail.decode.consumed_bytes)).arg(root.numberText(root.detail.decode.total_bytes)) : "-"
            delta: root.detail && root.detail.decode ? qsTr("%1 remaining").arg(root.numberText(root.detail.decode.remaining_bytes)) : qsTr("Bytes")
        }
    }

    SectionBlock {
        theme: root.theme
        title: qsTr("Account")
        rows: root.accountRows()
    }

    Text {
        visible: root.detail && root.detail.decode_error.length > 0
        text: root.detail ? root.detail.decode_error : ""
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        Layout.fillWidth: true
    }

    SectionBlock {
        visible: root.detail && root.detail.decode !== null
        theme: root.theme
        title: qsTr("Decoded data")
        rows: root.decodedRows()
    }

    Text {
        visible: root.detail && root.detail.related_transactions_error.length > 0
        text: root.detail ? root.detail.related_transactions_error : ""
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 8
        Layout.fillWidth: true

        Text {
            text: qsTr("Related transactions")
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
                    columns: [qsTr("Tx hash"), qsTr("Kind"), qsTr("Program"), qsTr("Accounts")]
                }

                Repeater {
                    model: root.relatedRows()

                    TransactionRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.hashText, modelData.kind, modelData.programText, modelData.accounts]
                        txHash: modelData.txHash
                        programId: modelData.programId
                        onCellActivated: function (column) {
                            if (column === 0) {
                                root.model.openReference("transaction", modelData.txHash)
                            } else if (column === 2) {
                                root.model.openReference("program", modelData.programId)
                            }
                        }
                    }
                }
            }
        }
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return null
        }

        let account = null
        let decode = null
        let decodeError = ""
        let decodeOnly = false

        if (value.account && value.account.account_id !== undefined) {
            account = value.account
            decode = value.decode || null
            decodeError = String(value.decode_error || "")
        } else if (value.account_id !== undefined && value.account !== undefined && value.data_hex !== undefined) {
            account = value
        } else if (value.account_type !== undefined && value.rows !== undefined && value.decoded !== undefined) {
            decode = value
            decodeOnly = true
        } else {
            return null
        }

        const report = account || {}
        return {
            account_id: String(report.account_id || (decode && decode.account_id) || ""),
            account: report.account || null,
            data_hex: String(report.data_hex || ""),
            related_transactions: Array.isArray(report.related_transactions) ? report.related_transactions : [],
            related_transactions_error: String(report.related_transactions_error || ""),
            decode: decode,
            decode_error: decodeError,
            decode_only: decodeOnly,
            raw: value
        }
    }

    function accountRows() {
        if (!root.detail) {
            return []
        }

        const rows = []
        if (root.detail.account_id.length) {
            rows.push({
                label: qsTr("Account ID"),
                value: root.detail.account_id,
                monospace: true,
                linkKind: "account",
                linkValue: root.detail.account_id
            })
        }
        if (root.detail.data_hex.length) {
            rows.push({
                label: qsTr("Data hex"),
                value: root.shortLong(root.detail.data_hex),
                subvalue: qsTr("%1 bytes").arg(root.numberText(root.dataBytes(root.detail.data_hex))),
                monospace: true
            })
        }

        const account = root.detail.account
        if (account && typeof account === "object" && !Array.isArray(account)) {
            const keys = Object.keys(account).sort()
            for (let i = 0; i < keys.length; ++i) {
                const key = keys[i]
                if (key === "data" || key === "data_hex") {
                    continue
                }
                rows.push({
                    label: key,
                    value: root.valueText(account[key]),
                    monospace: true,
                    linkKind: root.referenceKind(key, account[key]),
                    linkValue: root.valueText(account[key])
                })
            }
        }
        return rows
    }

    function decodedRows() {
        const decode = root.detail ? root.detail.decode : null
        if (!decode) {
            return []
        }

        const rows = [
            { label: qsTr("Account type"), value: root.valueText(decode.account_type), monospace: false },
            { label: qsTr("Consumed bytes"), value: root.numberText(decode.consumed_bytes), monospace: true },
            { label: qsTr("Total bytes"), value: root.numberText(decode.total_bytes), monospace: true },
            { label: qsTr("Remaining bytes"), value: root.numberText(decode.remaining_bytes), monospace: true }
        ]
        if (decode.remaining_data_hex) {
            rows.push({ label: qsTr("Remaining data"), value: root.shortLong(decode.remaining_data_hex), monospace: true })
        }

        const decodedRows = Array.isArray(decode.rows) ? decode.rows : []
        for (let i = 0; i < decodedRows.length; ++i) {
            const row = decodedRows[i]
            rows.push({
                label: String(row.path || qsTr("Field")),
                value: root.valueText(row.value),
                monospace: true,
                linkKind: root.referenceKind(row.path, row.value),
                linkValue: root.valueText(row.value)
            })
        }
        return rows
    }

    function relatedRows() {
        const rows = root.detail ? root.detail.related_transactions : []
        if (!rows.length) {
            return [{
                hashText: qsTr("No related transactions loaded"),
                kind: "-",
                programText: "-",
                accounts: "-",
                txHash: "",
                programId: ""
            }]
        }
        return rows.map(function (tx) {
            const txHash = String(tx.hash || "")
            const programId = String(tx.program_id_hex || "")
            return {
                hashText: root.shortId(txHash),
                kind: String(tx.kind || "-"),
                programText: root.shortId(programId),
                accounts: root.numberText(Array.isArray(tx.account_ids) ? tx.account_ids.length : 0),
                txHash: txHash,
                programId: programId
            }
        })
    }

    function referenceKind(label, value) {
        const text = root.valueText(value)
        const lowered = String(label || "").toLowerCase()
        if (!text.length || text === "-") {
            return ""
        }
        if (lowered.indexOf("channel") >= 0 && root.isLongHex(text)) {
            return "channel"
        }
        if (lowered.indexOf("transaction") >= 0 || lowered.indexOf("tx") >= 0) {
            return root.isLongHex(text) ? "transaction" : ""
        }
        if (lowered.indexOf("hash") >= 0 && root.isLongHex(text)) {
            return "transaction"
        }
        if (lowered.indexOf("program") >= 0) {
            return "program"
        }
        if (lowered.indexOf("account") >= 0 || lowered.indexOf("owner") >= 0 || lowered.indexOf("authority") >= 0 || lowered.indexOf("signer") >= 0) {
            return "account"
        }
        if (root.isLikelyAccount(text)) {
            return "account"
        }
        return ""
    }

    function isLongHex(value) {
        return /^(0x)?[0-9a-fA-F]{64}$/.test(String(value || ""))
    }

    function isLikelyAccount(value) {
        const text = String(value || "")
        return text.length >= 32 && text.length <= 128 && /^[1-9A-HJ-NP-Za-km-z]+$/.test(text)
    }

    function dataBytes(hex) {
        const text = String(hex || "").replace(/^0x/, "")
        return Math.floor(text.length / 2)
    }

    function valueText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        if (typeof value === "number") {
            return value % 1 === 0 ? value.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        if (typeof value === "object") {
            return JSON.stringify(value)
        }
        return String(value)
    }

    function numberText(value) {
        if (value === undefined || value === null || value === "") {
            return "-"
        }
        const numeric = Number(value)
        if (Number.isFinite(numeric)) {
            return numeric % 1 === 0 ? numeric.toLocaleString(Qt.locale(), "f", 0) : String(value)
        }
        return String(value)
    }

    function shortId(value) {
        const text = String(value || "")
        if (text.length <= 18) {
            return text.length ? text : "-"
        }
        return text.slice(0, 10) + "..." + text.slice(-6)
    }

    function shortLong(value) {
        const text = String(value || "")
        if (text.length <= 160) {
            return text.length ? text : "-"
        }
        return text.slice(0, 120) + "..." + text.slice(-24)
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
                        subvalue: String(modelData.subvalue || "")
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
        property string subvalue: ""
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
                Layout.preferredWidth: 132
                Layout.alignment: Qt.AlignTop
            }

            ColumnLayout {
                spacing: 2
                Layout.fillWidth: true

                LinkCell {
                    text: rowRoot.value
                    theme: rowRoot.theme
                    link: rowRoot.linkKind.length > 0
                    monospace: rowRoot.monospace
                    wrap: true
                    Layout.fillWidth: true
                    onActivated: rowRoot.activated()
                }

                Text {
                    visible: rowRoot.subvalue.length > 0
                    text: rowRoot.subvalue
                    color: rowRoot.theme.textDim
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    font.family: "monospace"
                    font.pixelSize: 11
                    Layout.fillWidth: true
                }
            }
        }
    }

    component TransactionRow: Item {
        id: rowRoot

        required property Theme theme
        property var columns: []
        property string txHash: ""
        property string programId: ""
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
                    monospace: !rowRoot.header
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 0 || index === 2
                    onActivated: rowRoot.cellActivated(index)
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header
                && ((index === 0 && rowRoot.txHash.length > 0)
                    || (index === 2 && rowRoot.programId.length > 0))
        }

        function columnWidth(index) {
            if (index === 1 || index === 3) {
                return 92
            }
            return 180
        }
    }
}
