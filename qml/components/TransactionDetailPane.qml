pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
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
    readonly property var favoriteEntry: root.detail ? root.model.favoriteStore.transactionEntry(root.detail) : null
    property string transactionView: "decoded"

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    onDetailChanged: transactionView = root.detail && root.detail.decoded !== null ? "decoded" : "raw"

    ListModel {
        id: transactionTabs

        ListElement { value: "decoded"; label: "Decoded" }
        ListElement { value: "raw"; label: "Raw" }
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 6
        Layout.fillWidth: true

        Text {
            text: qsTr("Transaction")
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

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            ActionButton {
                theme: root.theme
                text: root.favoriteButtonText()
                selected: root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry)
                enabled: root.favoriteEntry !== null
                Layout.preferredWidth: 118
                accessibleName: root.favoriteButtonAccessibleName()
                onClicked: root.model.favoriteStore.toggle(root.favoriteEntry)
            }

            Text {
                text: root.detail && root.detail.mode === "blockchain" ? qsTr("Mantle transaction") : qsTr("LEZ transaction")
                color: root.theme.textDim
                textFormat: Text.PlainText
                elide: Text.ElideRight
                font.pixelSize: root.theme.secondaryText
                Layout.fillWidth: true
                Layout.alignment: Qt.AlignVCenter
            }
        }
    }

    DetailSection {
        theme: root.theme
        title: qsTr("Overview")
        rows: root.overviewRows()
        labelWidth: 210
        surfaceColor: root.theme.surface
        onLinkActivated: function (kind, value) {
            root.model.entityNavigation.openReference(kind, value)
        }
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 8
        Layout.fillWidth: true

        RowLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Text {
                text: qsTr("Data")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 14
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            TabSwitch {
                theme: root.theme
                current: root.transactionView
                options: transactionTabs
                Layout.preferredWidth: 206
                onSelected: value => root.transactionView = value
            }
        }
    }

    ColumnLayout {
        visible: root.detail !== null && root.transactionView === "decoded"
        spacing: 14
        Layout.fillWidth: true

        StatusMessage {
            visible: root.detail && root.detail.decoded === null
            theme: root.theme
            tone: "info"
            title: qsTr("No IDL decode")
            message: qsTr("This transaction has no IDL-decoded instruction.")
            Layout.fillWidth: true
        }

        DetailSection {
            visible: root.detail && root.detail.decoded !== null
            theme: root.theme
            title: qsTr("Decoded instruction")
            rows: root.decodedRows()
            labelWidth: 210
            surfaceColor: root.theme.surface
            onLinkActivated: function (kind, value) {
                root.model.entityNavigation.openReference(kind, value)
            }
        }

        DetailSection {
            visible: root.detail && root.detail.decoded !== null
            theme: root.theme
            title: qsTr("Accounts")
            rows: root.decodedAccountRows()
            labelWidth: 210
            surfaceColor: root.theme.surface
            onLinkActivated: function (kind, value) {
                root.model.entityNavigation.openReference(kind, value)
            }
        }

        DetailSection {
            visible: root.detail && root.detail.decoded !== null
            theme: root.theme
            title: qsTr("Args")
            rows: root.decodedArgRows()
            labelWidth: 210
            surfaceColor: root.theme.surface
            onLinkActivated: function (kind, value) {
                root.model.entityNavigation.openReference(kind, value)
            }
        }
    }

    ColumnLayout {
        visible: root.detail !== null && root.transactionView === "raw"
        spacing: 14
        Layout.fillWidth: true

        StatusMessage {
            visible: root.detail && !root.hasRawData()
            theme: root.theme
            tone: "info"
            title: qsTr("No raw data")
            message: qsTr("No undecoded transaction rows are available.")
            Layout.fillWidth: true
        }

        ColumnLayout {
            visible: root.detail && root.detail.mode === "blockchain" && root.detail.ops.length > 0
            spacing: 10
            Layout.fillWidth: true

            Text {
                text: qsTr("Operations")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 14
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            Repeater {
                model: root.detail && root.detail.mode === "blockchain" ? root.detail.ops : []

                ColumnLayout {
                    id: operationBlock

                    required property var modelData

                    spacing: 8
                    Layout.fillWidth: true

                    DetailSection {
                        theme: root.theme
                        title: qsTr("Operation %1").arg(operationBlock.modelData.index)
                        rows: root.operationRows(operationBlock.modelData)
                        labelWidth: 210
                        surfaceColor: root.theme.surface
                        onLinkActivated: function (kind, value) {
                            root.model.entityNavigation.openReference(kind, value)
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

                            DetailValueRow {
                                theme: root.theme
                                label: qsTr("Payload")
                                value: qsTr("%1 field(s)").arg(root.fieldCount(operationBlock.modelData.payload))
                                monospace: false
                                labelWidth: 210
                            }

                            TextArea {
                                readOnly: true
                                text: root.formatValue(operationBlock.modelData.payload)
                                wrapMode: TextArea.Wrap
                                color: root.theme.textMuted
                                selectedTextColor: root.theme.selectedText
                                selectionColor: root.theme.accent
                                textFormat: Text.PlainText
                                font.family: "monospace"
                                font.pixelSize: 11
                                leftPadding: 12
                                rightPadding: 12
                                topPadding: 10
                                bottomPadding: 10
                                Layout.fillWidth: true
                                Layout.preferredHeight: 150

                                background: Rectangle {
                                    color: root.theme.field
                                    border.width: 1
                                    border.color: root.theme.outlineMuted
                                }
                            }
                        }
                    }
                }
            }
        }

        Repeater {
            model: root.detail && root.detail.mode === "lez" ? root.detail.sections : []

            DetailSection {
                required property var modelData

                theme: root.theme
                title: String(modelData.title || "")
                rows: root.inspectionRows(modelData.rows || [])
                labelWidth: 210
                surfaceColor: root.theme.surface
                onLinkActivated: function (kind, value) {
                    root.model.entityNavigation.openReference(kind, value)
                }
            }
        }
    }

    SocialPanel {
        visible: root.detail !== null
        theme: root.theme
        model: root.model
        title: qsTr("Transaction comments")
        topic: root.socialTopic()
        Layout.fillWidth: true
    }

    function normalize(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return null
        }

        if (value.type === "blockchain_transaction") {
            return {
                mode: "blockchain",
                hash: String(value.hash || ""),
                kind: qsTr("Blockchain"),
                block: String(value.block || ""),
                slot: value.slot,
                index: value.index,
                ops: Array.isArray(value.ops) ? value.ops : [],
                raw: value.raw || null,
                decoded: null,
                sections: []
            }
        }

        const summary = summaryFrom(value)
        if (!summary) {
            return null
        }

        const inspection = value.inspection || (value.raw_summary ? value : null)
        return {
            mode: "lez",
            hash: String(summary.hash || ""),
            kind: String(summary.kind || ""),
            summary: summary,
            sections: inspection && Array.isArray(inspection.sections) ? inspection.sections : [],
            decoded: value.decoded_instruction || null,
            steps: Array.isArray(value.steps) ? value.steps : []
        }
    }

    function summaryFrom(value) {
        if (value.raw_summary) {
            return value.raw_summary
        }
        if (value.inspection && value.inspection.raw_summary) {
            return value.inspection.raw_summary
        }
        if (value.hash && value.kind) {
            return value
        }
        return null
    }

    function hasRawData() {
        if (!root.detail) {
            return false
        }
        if (root.detail.mode === "blockchain") {
            return root.detail.ops.length > 0
        }
        return root.detail.sections.length > 0
    }

    function overviewRows() {
        if (!root.detail) {
            return []
        }
        if (root.detail.mode === "blockchain") {
            return [
                { label: qsTr("Block"), value: root.blockText(root.detail), monospace: true, linkKind: "block", linkValue: root.detail.block },
                { label: qsTr("Index in block"), value: root.valueText(root.detail.index), monospace: true },
                { label: qsTr("Ops"), value: root.valueText(root.detail.ops.length), monospace: true }
            ]
        }

        const summary = root.detail.summary || {}
        const rows = [
            { label: qsTr("Kind"), value: root.valueText(summary.kind), monospace: false },
            { label: qsTr("Hash"), value: root.valueText(summary.hash), monospace: true, linkKind: "transaction", linkValue: summary.hash }
        ]
        if (summary.program_id_hex) {
            rows.push({ label: qsTr("Program"), value: summary.program_id_hex, monospace: true, linkKind: "program", linkValue: summary.program_id_hex })
        }
        rows.push({ label: qsTr("Accounts"), value: root.valueText(root.count(summary.account_ids)), monospace: true })
        rows.push({ label: qsTr("Nonces"), value: root.valueText(root.count(summary.nonces)), monospace: true })
        rows.push({ label: qsTr("Instruction words"), value: root.valueText(root.count(summary.instruction_data)), monospace: true })
        if (summary.bytecode_len !== undefined && summary.bytecode_len !== null) {
            rows.push({ label: qsTr("Bytecode"), value: qsTr("%1 bytes").arg(summary.bytecode_len), monospace: true })
        }
        if (summary.raw_signature_valid !== undefined && summary.raw_signature_valid !== null) {
            rows.push({ label: qsTr("Raw signature"), value: root.validityText(summary.raw_signature_valid), monospace: false })
        }
        if (summary.prehash_signature_valid !== undefined && summary.prehash_signature_valid !== null) {
            rows.push({ label: qsTr("Prehash signature"), value: root.validityText(summary.prehash_signature_valid), monospace: false })
        }
        return rows
    }

    function operationRows(operation) {
        const rows = [
            { label: qsTr("Index"), value: root.valueText(operation.index), monospace: true },
            { label: qsTr("Opcode"), value: root.opcodeText(operation), monospace: true }
        ]
        if (operation.channel) {
            rows.push({ label: qsTr("Channel"), value: operation.channel, monospace: true, linkKind: "channel", linkValue: operation.channel })
        }
        if (operation.signer) {
            rows.push({ label: qsTr("Signer"), value: operation.signer, monospace: true, linkKind: "account", linkValue: operation.signer })
        }
        if (operation.parent) {
            rows.push({ label: qsTr("Parent message"), value: operation.parent, monospace: true, copyable: true })
        }
        if (operation.proof) {
            rows.push({ label: qsTr("Proof"), value: root.formatValue(operation.proof), monospace: true })
        }
        return rows
    }

    function inspectionRows(rows) {
        const result = []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            result.push({
                label: root.indexedLabel(row.label, row.index),
                value: root.valueText(row.value),
                subvalue: root.rowSubvalue(row),
                monospace: true,
                linkKind: root.referenceKind(row.label, row.value, row),
                linkValue: root.referenceValue(row)
            })
        }
        return result
    }

    function decodedRows() {
        const decoded = root.detail ? root.detail.decoded : null
        if (!decoded) {
            return []
        }
        return [
            { label: qsTr("Instruction"), value: root.valueText(decoded.instruction), monospace: false },
            { label: qsTr("Variant"), value: root.valueText(decoded.variant_index), monospace: true },
            { label: qsTr("Program"), value: root.valueText(decoded.program_id), monospace: true, linkKind: "program", linkValue: decoded.program_id },
            { label: qsTr("IDL"), value: root.valueText(decoded.idl_name), monospace: false },
            { label: qsTr("Remaining words"), value: root.valueText(root.count(decoded.remaining_words)), monospace: true }
        ]
    }

    function decodedAccountRows() {
        const decoded = root.detail ? root.detail.decoded : null
        const accounts = decoded && Array.isArray(decoded.accounts) ? decoded.accounts : []
        return accounts.map(function (row) {
            return {
                label: String(row.path || qsTr("Account")),
                value: root.valueText(row.value),
                monospace: true,
                linkKind: "account",
                linkValue: row.value
            }
        })
    }

    function decodedArgRows() {
        const decoded = root.detail ? root.detail.decoded : null
        const args = decoded && Array.isArray(decoded.args) ? decoded.args : []
        return args.map(function (row) {
            return {
                label: String(row.path || qsTr("Arg")),
                value: root.valueText(row.value),
                monospace: true,
                linkKind: root.referenceKind(row.path, row.value, row),
                linkValue: root.referenceValue(row)
            }
        })
    }

    function rowSubvalue(row) {
        const parts = []
        if (row.decimal && row.decimal !== row.value) {
            parts.push(qsTr("dec %1").arg(row.decimal))
        }
        if (row.hex && row.hex !== row.value) {
            parts.push(row.hex)
        }
        if (row.base58 && row.base58 !== row.value) {
            parts.push(row.base58)
        }
        return parts.join("  ")
    }

    function indexedLabel(label, index) {
        if (index === undefined || index === null) {
            return String(label || "")
        }
        return qsTr("%1 %2").arg(String(label || "")).arg(index)
    }

    function referenceKind(label, value, row) {
        const text = root.valueText(value)
        const lowered = String(label || "").toLowerCase()
        if (lowered.indexOf("channel") >= 0 && root.isLongHex(text)) {
            return "channel"
        }
        if ((lowered.indexOf("account") >= 0 || lowered.indexOf("owner") >= 0 || lowered.indexOf("signer") >= 0 || lowered.indexOf("program") >= 0) && text !== "-") {
            return lowered.indexOf("program") >= 0 ? "program" : "account"
        }
        if ((lowered.indexOf("tx") >= 0 || lowered.indexOf("transaction") >= 0 || lowered.indexOf("hash") >= 0) && root.isLongHex(text)) {
            return "transaction"
        }
        if (row && row.base58) {
            return "account"
        }
        return ""
    }

    function referenceValue(row) {
        if (row && row.base58) {
            return row.base58
        }
        return row ? root.valueText(row.value) : ""
    }

    function isLongHex(value) {
        return /^(0x)?[0-9a-fA-F]{64}$/.test(String(value || ""))
    }

    function opcodeText(operation) {
        const name = String(operation.opcode_name || qsTr("Unknown"))
        return qsTr("%1 %2 (%3)").arg(name).arg(operation.opcode_hex || "-").arg(operation.opcode)
    }

    function blockText(detail) {
        if (!detail.block) {
            return detail.slot ? qsTr("slot %1").arg(detail.slot) : "-"
        }
        return detail.slot ? qsTr("%1 (slot %2)").arg(root.shortHash(detail.block)).arg(detail.slot) : detail.block
    }

    function fieldCount(value) {
        if (Array.isArray(value)) {
            return value.length
        }
        if (value && typeof value === "object") {
            return Object.keys(value).length
        }
        return value === undefined || value === null ? 0 : 1
    }

    function count(value) {
        return Array.isArray(value) ? value.length : 0
    }

    function validityText(value) {
        return value ? qsTr("valid") : qsTr("invalid")
    }

    function valueText(value) {
        return UiFormat.valueText(value)
    }

    function shortHash(value) {
        return UiFormat.shortHash(value)
    }

    function formatValue(value) {
        if (value === undefined || value === null) {
            return "-"
        }
        if (typeof value === "string") {
            return value
        }
        return JSON.stringify(value, null, 2)
    }

    function favoriteButtonText() {
        return root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry) ? qsTr("Favorited") : qsTr("Favorite")
    }

    function favoriteButtonAccessibleName() {
        return root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry) ? qsTr("Remove transaction from favorites") : qsTr("Add transaction to favorites")
    }

    function socialTopic() {
        if (!root.detail || !root.detail.hash.length) {
            return ""
        }
        return root.model.social.commentTopic(root.detail.mode === "blockchain" ? "cryptarchia" : "lez", "transaction", root.detail.hash)
    }

}
