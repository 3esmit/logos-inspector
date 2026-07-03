pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQml.Models
import QtQuick.Layouts
import "../services/BridgeHelpers.js" as BridgeHelpers
import "../state"
import "../theme"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property var value: null
    readonly property var detail: normalize(value)
    property string dataView: "decoded"
    property var idlTypeOptions: []
    property var idlTypeLabels: []
    property int selectedIdlTypeIndex: -1
    property var activeDecode: detail ? detail.decode : null
    property string activeDecodeError: detail ? detail.decode_error : ""
    property string activeIdlLabel: ""
    property int decodeRequestSerial: 0
    property var relatedTransactionDecodeMap: ({})
    property int relatedTransactionDecodeRevision: 0
    property int relatedTransactionDecodeSerial: 0
    readonly property string nullAddressBase58: "11111111111111111111111111111111"

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    onDetailChanged: {
        Qt.callLater(root.resetDecodeState)
        Qt.callLater(root.resetRelatedTransactionDecodes)
    }

    ListModel {
        id: dataTabs

        ListElement { value: "decoded"; label: "Decoded" }
        ListElement { value: "raw"; label: "Raw" }
    }

    Connections {
        target: root.model.registeredIdls

        function onCountChanged() {
            Qt.callLater(root.resetDecodeState)
            Qt.callLater(root.resetRelatedTransactionDecodes)
        }
    }

    ColumnLayout {
        visible: root.detail !== null
        spacing: 6
        Layout.fillWidth: true

        CopyTextLine {
            text: root.detail ? root.accountHeader(root.detail) : ""
            theme: root.theme
            copyText: root.detail ? root.accountCopyValue(root.detail) : ""
            tooltipText: root.detail ? root.accountHeaderTooltip(root.detail) : ""
            monospace: true
            textColor: root.theme.text
            textPixelSize: 22
            textWeight: Font.DemiBold
            Layout.fillWidth: true
        }

        CopyTextLine {
            visible: root.detail && root.accountAlternate(root.detail).length > 0
            text: root.detail ? root.accountAlternate(root.detail) : ""
            theme: root.theme
            copyText: root.detail ? root.accountAlternate(root.detail) : ""
            monospace: true
            textColor: root.theme.textMuted
            textPixelSize: 12
            Layout.fillWidth: true
        }
    }

    SourceStrip {
        visible: root.detail !== null
        theme: root.theme
        sources: root.sourceItems()
        Layout.fillWidth: true
    }

    StatusMessage {
        visible: root.detail && root.detail.private_reference
        theme: root.theme
        tone: "info"
        title: qsTr("Private account reference")
        message: qsTr("Private account state is local wallet state. Public RPC can only expose public effects, commitments, nullifiers, or proofs when available.")
        Layout.fillWidth: true
    }

    RowLayout {
        visible: root.detail && root.detail.private_reference
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        ActionButton {
            theme: root.theme
            text: qsTr("Configure local wallet")
            Layout.preferredWidth: 172
            onClicked: root.model.openLocalWallet(root.detail ? root.detail.account_id : "", "privateSync")
        }

        ActionButton {
            theme: root.theme
            text: qsTr("Search public effects")
            enabled: false
            Layout.preferredWidth: 166
        }

        Item {
            Layout.fillWidth: true
        }
    }

    SectionBlock {
        theme: root.theme
        title: ""
        rows: root.accountRows()
    }

    ColumnLayout {
        visible: root.detail !== null && !root.detail.private_reference
        spacing: 8
        Layout.fillWidth: true

        RowLayout {
            spacing: root.theme.gap
            Layout.fillWidth: true

            Text {
                text: qsTr("Data [%1]").arg(root.detail ? root.numberText(root.dataBytes(root.detail.data_hex)) : "-")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 14
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            TabSwitch {
                visible: root.detail && root.dataBytes(root.detail.data_hex) > 0
                theme: root.theme
                current: root.dataView
                options: dataTabs
                Layout.preferredWidth: 206
                onSelected: value => root.dataView = value
            }
        }

        Frame {
            visible: root.detail && root.dataBytes(root.detail.data_hex) > 0
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

                ColumnLayout {
                    visible: root.dataView === "decoded"
                    spacing: root.theme.gap
                    Layout.fillWidth: true

                    RowLayout {
                        visible: root.idlTypeLabels.length > 0
                        spacing: root.theme.gapSmall
                        Layout.fillWidth: true
                        Layout.leftMargin: 12
                        Layout.rightMargin: 12
                        Layout.topMargin: 12

                        Text {
                            text: qsTr("IDL Type")
                            color: root.theme.textMuted
                            textFormat: Text.PlainText
                            font.pixelSize: 11
                            font.weight: Font.DemiBold
                            font.capitalization: Font.AllUppercase
                            Layout.preferredWidth: 92
                            Layout.alignment: Qt.AlignVCenter
                        }

                        ComboBox {
                            id: idlTypeCombo

                            editable: true
                            model: root.idlTypeLabels
                            currentIndex: root.selectedIdlTypeIndex
                            font.pixelSize: root.theme.secondaryText
                            Layout.fillWidth: true
                            Layout.preferredHeight: root.theme.controlHeight
                            onActivated: index => root.selectIdlType(index)
                            onAccepted: root.selectTypedIdlType(editText)

                            contentItem: TextField {
                                text: idlTypeCombo.editText
                                color: root.theme.text
                                placeholderText: qsTr("Search IDL type")
                                placeholderTextColor: root.theme.textDim
                                selectionColor: root.theme.accent
                                selectedTextColor: root.theme.selectedText
                                font: idlTypeCombo.font
                                leftPadding: 10
                                rightPadding: 10
                                readOnly: !idlTypeCombo.editable
                                background: null
                            }

                            background: Rectangle {
                                radius: root.theme.radius
                                color: idlTypeCombo.hovered || idlTypeCombo.activeFocus ? root.theme.surfaceRaised : root.theme.field
                                border.width: idlTypeCombo.activeFocus ? 2 : 1
                                border.color: idlTypeCombo.activeFocus ? root.theme.accent : root.theme.outlineMuted
                            }

                            Accessible.role: Accessible.ComboBox
                            Accessible.name: qsTr("IDL type")
                        }
                    }

                    Text {
                        visible: root.activeDecode !== null
                        text: qsTr("IDL Type: %1").arg(root.activeIdlTypeLabel())
                        color: root.theme.textMuted
                        textFormat: Text.PlainText
                        wrapMode: Text.WrapAnywhere
                        font.pixelSize: root.theme.dataText
                        Layout.fillWidth: true
                        Layout.leftMargin: 12
                        Layout.rightMargin: 12
                    }

                    StatusMessage {
                        visible: root.activeDecode === null
                        theme: root.theme
                        tone: root.activeDecodeError.length > 0 ? "warning" : "info"
                        title: root.activeDecodeError.length > 0 ? qsTr("Decode unavailable") : qsTr("No decoded data")
                        message: root.decodeMessage()
                        Layout.fillWidth: true
                        Layout.leftMargin: 12
                        Layout.rightMargin: 12
                        Layout.bottomMargin: 12
                    }

                    Repeater {
                        model: root.decodedRows()

                        DetailRow {
                            required property var modelData

                            theme: root.theme
                            label: String(modelData.label || "")
                            value: String(modelData.value || "-")
                            subvalue: String(modelData.subvalue || "")
                            subvalueCopyText: String(modelData.subvalueCopyText || "")
                            linkKind: String(modelData.linkKind || "")
                            linkValue: String(modelData.linkValue || "")
                            tooltipText: String(modelData.tooltipText || "")
                            monospace: modelData.monospace !== undefined ? modelData.monospace : true
                            onActivated: root.model.openReference(modelData.linkKind, modelData.linkValue)
                        }
                    }
                }

                TextArea {
                    visible: root.dataView === "raw"
                    readOnly: true
                    text: root.detail ? root.detail.data_hex : ""
                    wrapMode: TextArea.Wrap
                    color: root.detail && root.detail.data_hex.length ? root.theme.text : root.theme.textMuted
                    selectedTextColor: root.theme.selectedText
                    selectionColor: root.theme.accent
                    textFormat: Text.PlainText
                    font.family: "monospace"
                    font.pixelSize: root.theme.dataText
                    leftPadding: 12
                    rightPadding: 12
                    topPadding: 10
                    bottomPadding: 10
                    Layout.fillWidth: true
                    Layout.preferredHeight: 150

                    background: Rectangle {
                        color: root.theme.field
                        radius: root.theme.radius
                        border.width: 0
                    }
                }
            }
        }
    }

    Text {
        visible: root.detail && !root.detail.private_reference && root.detail.related_transactions_error.length > 0
        text: root.detail ? root.detail.related_transactions_error : ""
        color: root.theme.warning
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        font.pixelSize: 12
        Layout.fillWidth: true
    }

    ColumnLayout {
        visible: root.detail !== null && !root.detail.private_reference
        spacing: 8
        Layout.fillWidth: true

        Text {
            text: qsTr("Transactions [%1]").arg(root.detail ? root.numberText(root.detail.related_transactions.length) : "-")
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
                    columns: [qsTr("Tx hash"), qsTr("Direction"), qsTr("Instruction"), qsTr("IDL / Program"), qsTr("Affected")]
                }

                Repeater {
                    model: root.relatedRows()

                    TransactionRow {
                        required property var modelData

                        theme: root.theme
                        columns: [modelData.hashText, modelData.direction, modelData.instruction, modelData.programText, modelData.accounts]
                        txHash: modelData.txHash
                        programId: modelData.programId
                        onCellActivated: function (column) {
                            if (column === 0) {
                                root.model.openReference("transaction", modelData.txHash)
                            } else if (column === 3) {
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
        let privateReference = false

        if (value.type === "private_account_reference") {
            privateReference = true
            account = {
                account_id: String(value.account_id || "")
            }
        } else if (value.account && value.account.account_id !== undefined) {
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
            account_id_base58: String(report.account_id_base58 || report.account_id || (decode && decode.account_id) || ""),
            account_id_hex: String(report.account_id_hex || ""),
            account: report.account || null,
            balance: String(report.balance || (report.account && report.account.balance !== undefined ? report.account.balance : "")),
            nonce: String(report.nonce || (report.account && report.account.nonce !== undefined ? report.account.nonce : "")),
            owner_base58: String(report.owner_base58 || ""),
            owner_hex: String(report.owner_hex || ""),
            data_hex: String(report.data_hex || ""),
            related_transactions: Array.isArray(report.related_transactions) ? report.related_transactions : [],
            related_transactions_error: String(report.related_transactions_error || ""),
            decode: decode,
            decode_error: decodeError,
            decode_only: decodeOnly,
            private_reference: privateReference,
            raw: value
        }
    }

    function accountHeader(detail) {
        if (detail && detail.private_reference) {
            return qsTr("Private account reference")
        }
        if (detail && (detail.account_id_base58.length || detail.account_id_hex.length)) {
            return root.addressLabel(detail.account_id_base58, detail.account_id_hex)
        }
        return detail && detail.decode_only ? qsTr("Account data decode") : qsTr("Account")
    }

    function accountAlternate(detail) {
        if (!detail) {
            return ""
        }
        if (detail.private_reference) {
            return detail.account_id
        }
        if (detail.account_id_hex.length) {
            return root.hexAddressText(detail.account_id_hex)
        }
        if (detail.account_id.length && detail.account_id !== detail.account_id_base58) {
            return qsTr("input %1").arg(detail.account_id)
        }
        return ""
    }

    function accountCopyValue(detail) {
        if (!detail) {
            return ""
        }
        if (root.isNullAddress(detail.account_id_base58, detail.account_id_hex)) {
            return root.nullAddressBase58
        }
        if (detail.account_id_base58.length) {
            return detail.account_id_base58
        }
        return detail.account_id.length ? detail.account_id : ""
    }

    function accountHeaderTooltip(detail) {
        if (!detail || !root.isNullAddress(detail.account_id_base58, detail.account_id_hex)) {
            return ""
        }
        return root.accountCopyValue(detail)
    }

    function resetDecodeState() {
        root.decodeRequestSerial += 1
        if (!root.detail) {
            root.idlTypeOptions = []
            root.idlTypeLabels = []
            root.selectedIdlTypeIndex = -1
            root.activeDecode = null
            root.activeDecodeError = ""
            root.activeIdlLabel = ""
            return
        }

        root.dataView = "decoded"
        root.rebuildIdlTypeOptions()
        root.activeDecode = root.detail.decode
        root.activeDecodeError = root.detail.decode_error
        root.activeIdlLabel = ""
        root.selectedIdlTypeIndex = root.activeDecode ? root.indexForType(root.activeDecode.account_type) : -1
        if (root.selectedIdlTypeIndex >= 0) {
            root.activeIdlLabel = root.idlTypeOptions[root.selectedIdlTypeIndex].idlName
        }
        if (!root.detail.private_reference && !root.activeDecode) {
            root.autoSelectDecode()
        }
    }

    function sourceItems() {
        if (!root.detail) {
            return []
        }
        if (root.detail.private_reference) {
            return [qsTr("Local Wallet"), qsTr("private account reference"), qsTr("wallet state required")]
        }
        return [qsTr("L2 LEZ"), qsTr("sequencer latest"), qsTr("account id"), qsTr("L2 Indexer"), qsTr("finalized related txs"), qsTr("Local IDL"), qsTr("decode schema")]
    }

    function rebuildIdlTypeOptions() {
        const options = []
        const labels = []
        for (let i = 0; i < root.model.registeredIdls.count; ++i) {
            const idl = root.model.registeredIdls.get(i)
            const types = root.idlAccountTypes(idl.json)
            for (let j = 0; j < types.length; ++j) {
                const label = qsTr("%1: %2").arg(idl.name).arg(types[j])
                options.push({
                    idlIndex: i,
                    idlKey: String(idl.key || root.model.idlKey(idl.name, idl.programId, idl.json)),
                    idlName: String(idl.name || qsTr("IDL %1").arg(i + 1)),
                    programId: String(idl.programId || ""),
                    accountType: types[j],
                    json: String(idl.json || ""),
                    label: label
                })
                labels.push(label)
            }
        }
        root.idlTypeOptions = options
        root.idlTypeLabels = labels
    }

    function idlAccountTypes(json) {
        const parsed = BridgeHelpers.parseJson(String(json || ""))
        if (!parsed.ok || !parsed.value || !Array.isArray(parsed.value.accounts)) {
            return []
        }
        const rows = []
        for (let i = 0; i < parsed.value.accounts.length; ++i) {
            const account = parsed.value.accounts[i] || {}
            const name = String(account.name || "")
            if (name.length) {
                rows.push(name)
            }
        }
        return rows
    }

    function autoSelectDecode() {
        if (!root.detail || !root.detail.data_hex.length || root.model.registeredIdls.count === 0) {
            return
        }

        const serial = root.decodeRequestSerial + 1
        root.decodeRequestSerial = serial
        root.model.autoDecodeAccountData(root.detail.data_hex, root.accountCacheId(), function (response) {
            if (serial !== root.decodeRequestSerial) {
                return
            }
            if (response.ok && response.value) {
                root.activeDecode = response.value
                root.activeDecodeError = ""
                root.activeIdlLabel = String(response.entry.name || qsTr("IDL"))
                root.selectedIdlTypeIndex = root.indexForTypeInIdlKey(response.entry.key, response.value.account_type)
                root.model.cacheAccountIdlSelection(root.accountCacheId(), response.entry, response.value.account_type)
            } else {
                root.activeDecodeError = response.error || ""
            }
        })
    }

    function selectIdlType(index) {
        if (!root.detail || index < 0 || index >= root.idlTypeOptions.length) {
            return
        }
        const option = root.idlTypeOptions[index]
        const serial = root.decodeRequestSerial + 1
        root.decodeRequestSerial = serial
        root.selectedIdlTypeIndex = index
        root.activeIdlLabel = option.idlName
        root.model.decodeAccountDataAsync(root.detail.data_hex, option.json, option.accountType, function (response) {
            if (serial !== root.decodeRequestSerial) {
                return
            }
            if (response.ok && response.value) {
                root.activeDecode = response.value
                root.activeDecodeError = ""
                if (root.model.accountDecodeFullyConsumed(response.value)) {
                    root.model.cacheAccountIdlSelection(root.accountCacheId(), option, response.value.account_type || option.accountType)
                }
            } else {
                root.activeDecode = null
                root.activeDecodeError = response.error || qsTr("Decode failed.")
            }
        })
    }

    function selectTypedIdlType(text) {
        const needle = String(text || "").trim().toLowerCase()
        if (!needle.length) {
            return
        }
        for (let i = 0; i < root.idlTypeLabels.length; ++i) {
            if (String(root.idlTypeLabels[i]).toLowerCase().indexOf(needle) >= 0) {
                root.selectIdlType(i)
                return
            }
        }
    }

    function indexForType(accountType) {
        const text = String(accountType || "")
        if (!text.length) {
            return -1
        }
        for (let i = 0; i < root.idlTypeOptions.length; ++i) {
            if (root.idlTypeOptions[i].accountType === text) {
                return i
            }
        }
        return -1
    }

    function indexForTypeInIdl(idlIndex, accountType) {
        const text = String(accountType || "")
        for (let i = 0; i < root.idlTypeOptions.length; ++i) {
            const option = root.idlTypeOptions[i]
            if (option.idlIndex === idlIndex && option.accountType === text) {
                return i
            }
        }
        return root.indexForType(accountType)
    }

    function indexForTypeInIdlKey(idlKey, accountType) {
        const key = String(idlKey || "")
        const text = String(accountType || "")
        for (let i = 0; i < root.idlTypeOptions.length; ++i) {
            const option = root.idlTypeOptions[i]
            if (option.idlKey === key && option.accountType === text) {
                return i
            }
        }
        return root.indexForType(accountType)
    }

    function accountCacheId() {
        if (!root.detail) {
            return ""
        }
        return root.detail.account_id_base58.length ? root.detail.account_id_base58 : root.detail.account_id
    }

    function activeIdlTypeLabel() {
        if (!root.activeDecode) {
            return "-"
        }
        const accountType = root.valueText(root.activeDecode.account_type)
        if (root.activeIdlLabel.length) {
            return qsTr("%1: %2").arg(root.activeIdlLabel).arg(accountType)
        }
        return qsTr("IDL: %1").arg(accountType)
    }

    function decodeMessage() {
        if (root.activeDecodeError.length) {
            return root.activeDecodeError
        }
        if (!root.detail || !root.detail.data_hex.length) {
            return qsTr("No account data is available.")
        }
        if (root.model.registeredIdls.count === 0) {
            return qsTr("Register an account IDL in Programs to decode this data.")
        }
        return qsTr("No registered IDL type decoded this data.")
    }

    function accountRows() {
        if (!root.detail) {
            return []
        }

        const rows = []
        if (root.detail.private_reference) {
            rows.push({
                label: qsTr("Reference"),
                value: root.detail.account_id.length ? root.detail.account_id : qsTr("Private/<id>"),
                monospace: true
            })
            return rows
        }
        if (root.detail.balance.length) {
            rows.push({
                label: qsTr("Balance"),
                value: root.numberText(root.detail.balance),
                monospace: true
            })
        }
        if (root.detail.nonce.length) {
            rows.push({
                label: qsTr("Nonce"),
                value: root.numberText(root.detail.nonce),
                monospace: true
            })
        }
        if (root.detail.owner_base58.length || root.detail.owner_hex.length) {
            const ownerValue = root.addressCopyValue(root.detail.owner_base58, root.detail.owner_hex)
            rows.push({
                label: qsTr("Owner"),
                value: root.addressLabel(root.detail.owner_base58, root.detail.owner_hex),
                subvalue: root.detail.owner_hex.length ? root.hexAddressText(root.detail.owner_hex) : "",
                subvalueCopyText: root.detail.owner_hex.length ? root.hexAddressText(root.detail.owner_hex) : "",
                monospace: true,
                linkKind: "program",
                linkValue: ownerValue,
                tooltipText: root.isNullAddress(root.detail.owner_base58, root.detail.owner_hex) ? ownerValue : ""
            })
        }
        return rows
    }

    function decodedRows() {
        const decode = root.activeDecode
        if (!decode) {
            return []
        }

        const rows = []
        if (decode.remaining_data_hex) {
            rows.push({ label: qsTr("Remaining data"), value: root.shortLong(decode.remaining_data_hex), monospace: true })
        }

        const decodedRows = Array.isArray(decode.rows) ? decode.rows : []
        for (let i = 0; i < decodedRows.length; ++i) {
            const row = decodedRows[i]
            const rawValue = root.valueText(row.value)
            const kind = root.referenceKind(row.path, row.value)
            const useAlias = (kind === "account" || kind === "program") && root.isNullAddress(rawValue, rawValue)
            const aliased = useAlias ? root.addressLabel(rawValue, "") : rawValue
            rows.push({
                label: root.displayLabel(row.path || qsTr("Field")),
                value: aliased,
                monospace: true,
                linkKind: kind,
                linkValue: useAlias ? root.addressCopyValue(rawValue, rawValue) : rawValue,
                tooltipText: useAlias ? root.addressCopyValue(rawValue, rawValue) : ""
            })
        }
        return rows
    }

    function relatedRows() {
        const revision = root.relatedTransactionDecodeRevision
        const rows = root.detail ? root.detail.related_transactions : []
        if (!rows.length) {
            return [{
                hashText: qsTr("No related transactions loaded"),
                direction: "-",
                instruction: "-",
                programText: "-",
                accounts: "-",
                txHash: "",
                programId: ""
            }]
        }
        return rows.map(function (tx) {
            const txHash = String(tx.hash || "")
            const programId = String(tx.program_id_hex || "")
            const decoded = tx.decoded_instruction || root.relatedTransactionDecode(txHash)
            return {
                hashText: root.shortId(txHash),
                direction: root.directionText(tx.direction),
                instruction: decoded ? String(decoded.instruction || "-") : String(tx.kind || "-"),
                programText: decoded && decoded.idl_name ? String(decoded.idl_name) : root.shortId(programId),
                accounts: root.numberText(Array.isArray(tx.account_ids) ? tx.account_ids.length : 0),
                txHash: txHash,
                programId: programId
            }
        })
    }

    function resetRelatedTransactionDecodes() {
        root.relatedTransactionDecodeSerial += 1
        root.relatedTransactionDecodeMap = ({})
        root.relatedTransactionDecodeRevision += 1
        if (!root.detail || !root.detail.related_transactions.length || root.model.registeredIdls.count === 0) {
            return
        }
        root.decodeRelatedTransactions(root.relatedTransactionDecodeSerial)
    }

    function decodeRelatedTransactions(serial) {
        const rows = root.detail ? root.detail.related_transactions : []
        for (let i = 0; i < rows.length; ++i) {
            const summary = root.relatedTransactionSummary(rows[i])
            if (!summary) {
                continue
            }
            const candidates = root.model.transactionDecodeCandidates(summary)
            if (candidates.length > 0) {
                root.tryRelatedTransactionDecodeCandidate(serial, summary.hash, summary, candidates, 0, null)
            }
        }
    }

    function tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index, partialDecoded) {
        if (serial !== root.relatedTransactionDecodeSerial) {
            return
        }
        if (index >= candidates.length) {
            if (partialDecoded) {
                root.storeRelatedTransactionDecode(txHash, partialDecoded)
            }
            return
        }

        const candidate = candidates[index]
        root.model.decodeTransactionSummaryAsync(summary, candidate.entry.json, function (response) {
            if (serial !== root.relatedTransactionDecodeSerial) {
                return
            }
            if (response.ok && response.value && root.model.transactionDecodeFullyConsumed(response.value)) {
                const decoded = root.model.transactionDecodedInstruction(response.value)
                if (decoded) {
                    root.storeRelatedTransactionDecode(txHash, decoded)
                    return
                }
            }
            const nextPartial = partialDecoded || (response.ok && response.value ? root.model.transactionDecodedInstruction(response.value) : null)
            root.tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index + 1, nextPartial)
        })
    }

    function storeRelatedTransactionDecode(txHash, decoded) {
        const key = String(txHash || "")
        if (!key.length) {
            return
        }
        const next = root.copyRelatedTransactionDecodeMap()
        next[key] = decoded
        root.relatedTransactionDecodeMap = next
        root.relatedTransactionDecodeRevision += 1
    }

    function relatedTransactionDecode(txHash) {
        const revision = root.relatedTransactionDecodeRevision
        const key = String(txHash || "")
        return key.length ? (root.relatedTransactionDecodeMap || {})[key] || null : null
    }

    function copyRelatedTransactionDecodeMap() {
        const copy = {}
        const current = root.relatedTransactionDecodeMap || {}
        for (const key in current) {
            copy[key] = current[key]
        }
        return copy
    }

    function relatedTransactionSummary(tx) {
        if (!tx || typeof tx !== "object") {
            return null
        }
        const words = Array.isArray(tx.instruction_data) ? tx.instruction_data : []
        if (String(tx.kind || "") !== "Public" || words.length === 0) {
            return null
        }
        return {
            hash: String(tx.hash || ""),
            kind: String(tx.kind || ""),
            program_id_hex: String(tx.program_id_hex || ""),
            account_ids: Array.isArray(tx.account_ids) ? tx.account_ids : [],
            nonces: Array.isArray(tx.nonces) ? tx.nonces : [],
            instruction_data: words,
            bytecode_len: tx.bytecode_len === undefined ? null : tx.bytecode_len,
            raw_signature_valid: null,
            message_prehash: null,
            prehash_signature_valid: null
        }
    }

    function directionText(direction) {
        const value = String(direction || "").toLowerCase()
        if (value === "incoming") {
            return qsTr("Incoming")
        }
        if (value === "outgoing") {
            return qsTr("Outgoing")
        }
        return "-"
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

    function hexAddressText(hex) {
        const text = String(hex || "").replace(/^0x/, "")
        return text.length ? "0x" + text : ""
    }

    function addressLabel(baseValue, hexValue) {
        const base = String(baseValue || "")
        const hex = String(hexValue || "")
        if (root.isNullAddress(base, hex)) {
            return qsTr("Null")
        }
        if (base.length) {
            return base
        }
        return hex.length ? root.hexAddressText(hex) : "-"
    }

    function addressCopyValue(baseValue, hexValue) {
        const base = String(baseValue || "")
        if (root.isNullAddress(base, hexValue)) {
            return root.nullAddressBase58
        }
        return base.length ? base : root.hexAddressText(hexValue)
    }

    function isNullAddress(baseValue, hexValue) {
        const base = String(baseValue || "")
        const explicitHex = String(hexValue || "").replace(/^0x/, "")
        const inferredHex = explicitHex.length > 0
            ? explicitHex
            : (/^(0x)?[0-9a-fA-F]{32,}$/.test(base) ? base.replace(/^0x/, "") : "")
        return base === root.nullAddressBase58 || (inferredHex.length > 0 && /^[0]+$/.test(inferredHex))
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

    function displayLabel(value) {
        const text = String(value || "").replace(/[._-]+/g, " ").trim()
        return text.length ? text : qsTr("Field")
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
                        subvalueCopyText: String(modelData.subvalueCopyText || "")
                        linkKind: String(modelData.linkKind || "")
                        linkValue: String(modelData.linkValue || "")
                        tooltipText: String(modelData.tooltipText || "")
                        monospace: modelData.monospace !== undefined ? modelData.monospace : true
                        onActivated: root.model.openReference(modelData.linkKind, modelData.linkValue)
                    }
                }
            }
        }
    }

    component CopyTextLine: Item {
        id: copyLineRoot

        required property Theme theme
        property string text: ""
        property string copyText: text
        property string tooltipText: ""
        property bool monospace: true
        property color textColor: theme.text
        property int textPixelSize: theme.dataText
        property int textWeight: Font.Normal

        Layout.fillWidth: true
        implicitHeight: Math.max(copyLineText.implicitHeight, copyButton.implicitHeight)

        Row {
            id: copyLineRow

            width: copyLineRoot.width
            spacing: copyLineRoot.theme.gapTiny

            Text {
                id: copyLineText

                text: copyLineRoot.text
                width: Math.min(implicitWidth, Math.max(80, copyLineRoot.width - copyButton.implicitWidth - copyLineRow.spacing))
                color: copyLineRoot.textColor
                textFormat: Text.PlainText
                wrapMode: Text.WrapAnywhere
                font.family: copyLineRoot.monospace ? "monospace" : ""
                font.pixelSize: copyLineRoot.textPixelSize
                font.weight: copyLineRoot.textWeight

                MouseArea {
                    id: copyLineHover

                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.NoButton
                }

                ToolTip.visible: copyLineHover.containsMouse && copyLineRoot.tooltipText.length > 0
                ToolTip.text: copyLineRoot.tooltipText
            }

            ToolButton {
                id: copyButton

                visible: copyLineRoot.copyText.length > 0
                hoverEnabled: true
                focusPolicy: Qt.TabFocus
                width: 26
                height: 26
                padding: 0
                onClicked: copyLineRoot.copyToClipboard()

                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: qsTr("Copy")

                background: Rectangle {
                    radius: copyLineRoot.theme.radius
                    color: copyButton.down ? copyLineRoot.theme.accentMuted : (copyButton.hovered || copyButton.activeFocus ? copyLineRoot.theme.hover : "transparent")
                    border.width: copyButton.activeFocus ? 1 : 0
                    border.color: copyLineRoot.theme.accent
                }

                contentItem: Item {
                    Rectangle {
                        x: 7
                        y: 5
                        width: 10
                        height: 12
                        radius: 2
                        color: "transparent"
                        border.width: 1
                        border.color: copyButton.hovered || copyButton.activeFocus ? copyLineRoot.theme.accentHover : copyLineRoot.theme.textMuted
                    }

                    Rectangle {
                        x: 10
                        y: 8
                        width: 10
                        height: 12
                        radius: 2
                        color: copyLineRoot.theme.surface
                        border.width: 1
                        border.color: copyButton.hovered || copyButton.activeFocus ? copyLineRoot.theme.accentHover : copyLineRoot.theme.textMuted
                    }
                }

                Accessible.role: Accessible.Button
                Accessible.name: qsTr("Copy %1").arg(copyLineRoot.text)
            }
        }

        TextArea {
            id: copyBuffer

            visible: false
            text: copyLineRoot.copyText
        }

        Accessible.role: Accessible.StaticText
        Accessible.name: copyLineRoot.text

        function copyToClipboard() {
            copyBuffer.text = copyLineRoot.copyText
            copyBuffer.forceActiveFocus()
            copyBuffer.selectAll()
            copyBuffer.copy()
            copyBuffer.deselect()
            copyLineRoot.forceActiveFocus()
        }
    }

    component DetailRow: Item {
        id: rowRoot

        required property Theme theme
        property string label: ""
        property string value: ""
        property string subvalue: ""
        property string subvalueCopyText: ""
        property string linkKind: ""
        property string linkValue: ""
        property string tooltipText: ""
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
                wrapMode: Text.Wrap
                maximumLineCount: 2
                elide: Text.ElideRight
                font.pixelSize: 11
                font.weight: Font.DemiBold
                font.capitalization: Font.AllUppercase
                Layout.preferredWidth: 132
                Layout.maximumWidth: 132
                Layout.alignment: Qt.AlignTop
            }

            ColumnLayout {
                spacing: 2
                Layout.fillWidth: true

                LinkCell {
                    text: rowRoot.value
                    theme: rowRoot.theme
                    link: rowRoot.linkKind.length > 0
                    copyText: rowRoot.linkValue.length > 0 ? rowRoot.linkValue : rowRoot.value
                    tooltipText: rowRoot.tooltipText
                    monospace: rowRoot.monospace
                    wrap: true
                    Layout.fillWidth: true
                    onActivated: rowRoot.activated()
                }

                LinkCell {
                    visible: rowRoot.subvalue.length > 0
                    text: rowRoot.subvalue
                    theme: rowRoot.theme
                    copyable: rowRoot.subvalueCopyText.length > 0
                    copyText: rowRoot.subvalueCopyText
                    monospace: true
                    wrap: true
                    textColor: rowRoot.theme.textDim
                    textPixelSize: 11
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
            columns: rowRoot.columns.length > 0 ? rowRoot.columns.length : 5
            columnSpacing: 10

            Repeater {
                model: rowRoot.columns.length > 0 ? rowRoot.columns.length : 5

                LinkCell {
                    required property int index

                    theme: rowRoot.theme
                    text: String(rowRoot.columns[index] || "-")
                    header: rowRoot.header
                    link: rowRoot.linkFor(index)
                    copyText: rowRoot.copyValueFor(index)
                    monospace: !rowRoot.header
                    Layout.preferredWidth: rowRoot.columnWidth(index)
                    Layout.fillWidth: index === 0 || index === 2 || index === 3
                    onActivated: rowRoot.cellActivated(index)
                }
            }
        }

        function linkFor(index) {
            return !rowRoot.header
                && ((index === 0 && rowRoot.txHash.length > 0)
                    || (index === 3 && rowRoot.programId.length > 0))
        }

        function columnWidth(index) {
            if (index === 1 || index === 4) {
                return 92
            }
            if (index === 2) {
                return 160
            }
            return 180
        }

        function copyValueFor(index) {
            if (index === 0 && rowRoot.txHash.length > 0) {
                return rowRoot.txHash
            }
            if (index === 3 && rowRoot.programId.length > 0) {
                return rowRoot.programId
            }
            return String(rowRoot.columns[index] || "")
        }
    }
}
