pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../services/BridgeHelpers.js" as BridgeHelpers
import "../state"
import "../theme"
import "accounts"
import "common"

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
    property int interactionInstructionIndex: 0
    property var interactionAccountValues: ({})
    property var interactionArgValues: ({})
    property string interactionProgramBinary: ""
    property int interactionRevision: 0
    property var pendingInstructionRequest: null
    readonly property string nullAddressBase58: "11111111111111111111111111111111"
    readonly property var favoriteEntry: root.detail ? root.model.favoriteAccountEntry(root.detail) : null

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    onDetailChanged: {
        Qt.callLater(root.resetDecodeState)
        Qt.callLater(root.resetRelatedTransactionDecodes)
        Qt.callLater(root.resetInteractionState)
    }

    onActiveDecodeChanged: {
        Qt.callLater(root.resetInteractionState)
    }

    Connections {
        target: root.model.registeredIdls

        function onCountChanged() {
            Qt.callLater(root.resetDecodeState)
            Qt.callLater(root.resetRelatedTransactionDecodes)
            Qt.callLater(root.resetInteractionState)
        }
    }

    AccountHeaderBlock {
        visible: root.detail !== null
        theme: root.theme
        titleText: root.detail ? root.accountHeader(root.detail) : ""
        copyText: root.detail ? root.accountCopyValue(root.detail) : ""
        tooltipText: root.detail ? root.accountHeaderTooltip(root.detail) : ""
        alternateText: root.detail ? root.accountAlternate(root.detail) : ""
    }

    RowLayout {
        visible: root.detail !== null
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
            text: root.detail && root.detail.private_reference ? qsTr("Private account reference") : qsTr("Public account")
            color: root.theme.textDim
            textFormat: Text.PlainText
            elide: Text.ElideRight
            font.pixelSize: root.theme.secondaryText
            Layout.fillWidth: true
            Layout.alignment: Qt.AlignVCenter
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

    AccountSectionBlock {
        theme: root.theme
        title: ""
        rows: root.accountRows()
        modelRef: root.model
    }

    AccountDecodeSection {
        theme: root.theme
        modelRef: root.model
        detail: root.detail
        dataView: root.dataView
        idlTypeLabels: root.idlTypeLabels
        selectedIdlTypeIndex: root.selectedIdlTypeIndex
        activeDecode: root.activeDecode
        activeDecodeError: root.activeDecodeError
        activeIdlTypeLabelText: root.activeIdlTypeLabel()
        decodeStatusMessage: root.decodeMessage()
        decodedRows: root.decodedRows()
        onDataViewSelected: value => root.dataView = value
        onIdlTypeSelected: index => root.selectIdlType(index)
        onTypedIdlTypeSelected: text => root.selectTypedIdlType(text)
        onRowActivated: (linkKind, linkValue) => root.model.openReference(linkKind, linkValue)
    }

    ColumnLayout {
        visible: root.canInteractWithIdl()
        spacing: 8
        Layout.fillWidth: true

        Text {
            text: qsTr("Interact")
            color: root.theme.text
            textFormat: Text.PlainText
            font.pixelSize: 14
            font.weight: Font.DemiBold
            Layout.fillWidth: true
        }

        Frame {
            padding: root.theme.gap
            Layout.fillWidth: true

            background: Rectangle {
                color: root.theme.surface
                radius: root.theme.radius
                border.width: 1
                border.color: root.theme.outlineMuted
            }

            contentItem: ColumnLayout {
                spacing: root.theme.gapSmall

                GridLayout {
                    columns: root.width < 700 ? 1 : 2
                    columnSpacing: root.theme.gap
                    rowSpacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ColumnLayout {
                        spacing: 6
                        Layout.fillWidth: true

                        Text {
                            text: qsTr("Instruction")
                            color: root.theme.textMuted
                            textFormat: Text.PlainText
                            font.pixelSize: root.theme.secondaryText
                            font.weight: Font.Medium
                            Layout.fillWidth: true
                        }

                        ComboBox {
                            id: instructionCombo
                            model: root.interactionInstructionLabels()
                            currentIndex: root.interactionInstructionIndex
                            textRole: ""
                            hoverEnabled: true
                            Layout.fillWidth: true
                            Layout.preferredHeight: root.theme.controlHeight
                            onActivated: index => root.selectInteractionInstruction(index)

                            contentItem: TextField {
                                text: instructionCombo.displayText
                                color: root.theme.text
                                verticalAlignment: Text.AlignVCenter
                                leftPadding: 12
                                rightPadding: 24
                                readOnly: true
                                clip: true
                                background: null
                                font.pixelSize: root.theme.primaryText
                            }

                            background: Rectangle {
                                radius: root.theme.radius
                                color: instructionCombo.hovered || instructionCombo.activeFocus ? root.theme.surfaceRaised : root.theme.field
                                border.width: instructionCombo.activeFocus ? 2 : 1
                                border.color: instructionCombo.activeFocus ? root.theme.accent : root.theme.outlineMuted
                            }

                            popup: Popup {
                                y: instructionCombo.height
                                width: instructionCombo.width
                                implicitHeight: Math.min(contentItem.implicitHeight, 280)
                                padding: 1

                                contentItem: ListView {
                                    clip: true
                                    implicitHeight: contentHeight
                                    model: instructionCombo.popup.visible ? instructionCombo.delegateModel : null
                                    currentIndex: instructionCombo.highlightedIndex
                                }

                                background: Rectangle {
                                    color: root.theme.surface
                                    radius: root.theme.radius
                                    border.width: 1
                                    border.color: root.theme.outline
                                }
                            }

                            delegate: ItemDelegate {
                                id: instructionDelegate

                                required property string modelData

                                width: instructionCombo.width
                                text: modelData
                                hoverEnabled: true
                                contentItem: Text {
                                    text: instructionDelegate.text
                                    color: root.theme.text
                                    textFormat: Text.PlainText
                                    elide: Text.ElideRight
                                    verticalAlignment: Text.AlignVCenter
                                    font.pixelSize: root.theme.primaryText
                                }
                                background: Rectangle {
                                    color: instructionDelegate.hovered ? root.theme.hover : root.theme.surface
                                }
                            }

                            Accessible.role: Accessible.ComboBox
                            Accessible.name: qsTr("Instruction")
                        }
                    }

                    FieldRow {
                        theme: root.theme
                        label: qsTr("Program binary")
                        placeholderText: qsTr("program.bin")
                        sourceText: root.interactionProgramBinary
                        syncSourceText: true
                        Layout.fillWidth: true
                        onTextEdited: text => {
                            root.interactionProgramBinary = text
                            root.interactionRevision += 1
                        }
                    }
                }

                GridLayout {
                    visible: root.interactionAccountFields().length > 0
                    columns: root.width < 700 ? 1 : 2
                    columnSpacing: root.theme.gap
                    rowSpacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    Repeater {
                        model: root.interactionAccountFields()

                        FieldRow {
                            required property var modelData

                            theme: root.theme
                            label: modelData.label
                            placeholderText: modelData.placeholder
                            sourceText: {
                                const revision = root.interactionRevision
                                return root.interactionFieldValue("account", modelData.name)
                            }
                            syncSourceText: true
                            Layout.fillWidth: true
                            onTextEdited: text => root.setInteractionFieldValue("account", modelData.name, text)
                        }
                    }
                }

                GridLayout {
                    visible: root.interactionArgFields().length > 0
                    columns: root.width < 700 ? 1 : 2
                    columnSpacing: root.theme.gap
                    rowSpacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    Repeater {
                        model: root.interactionArgFields()

                        FieldRow {
                            required property var modelData

                            theme: root.theme
                            label: modelData.label
                            placeholderText: modelData.placeholder
                            sourceText: {
                                const revision = root.interactionRevision
                                return root.interactionFieldValue("arg", modelData.name)
                            }
                            syncSourceText: true
                            Layout.fillWidth: true
                            onTextEdited: text => root.setInteractionFieldValue("arg", modelData.name, text)
                        }
                    }
                }

                StatusMessage {
                    visible: root.model.idlInstructionError.length > 0
                    theme: root.theme
                    tone: "warning"
                    title: qsTr("Instruction")
                    message: root.model.idlInstructionError
                    Layout.fillWidth: true
                }

                Text {
                    visible: root.interactionPreviewText().length > 0
                    text: root.interactionPreviewText()
                    color: root.theme.textMuted
                    textFormat: Text.PlainText
                    wrapMode: Text.WrapAnywhere
                    font.pixelSize: root.theme.secondaryText
                    Layout.fillWidth: true
                }

                RowLayout {
                    spacing: root.theme.gapSmall
                    Layout.fillWidth: true

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Preview")
                        enabled: root.interactionInputsComplete()
                        Layout.preferredWidth: 112
                        onClicked: root.model.previewIdlInstruction(root.interactionRequest())
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Configure wallet")
                        visible: !root.model.walletHomeConfigured()
                        Layout.preferredWidth: 156
                        onClicked: root.model.openLocalWallet("", "profiles")
                    }

                    Item {
                        Layout.fillWidth: true
                    }

                    ActionButton {
                        theme: root.theme
                        text: qsTr("Send")
                        primary: true
                        enabled: root.interactionInputsComplete() && root.model.walletHomeConfigured()
                        Layout.preferredWidth: 112
                        onClicked: {
                            root.pendingInstructionRequest = root.interactionRequest()
                            instructionConfirm.open()
                        }
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

                AccountTransactionRow {
                    theme: root.theme
                    header: true
                    columns: [qsTr("Tx hash"), qsTr("Direction"), qsTr("Instruction"), qsTr("IDL / Program"), qsTr("Affected")]
                }

                Repeater {
                    model: root.relatedRows()

                    AccountTransactionRow {
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

    ConfirmActionPopup {
        id: instructionConfirm

        theme: root.theme
        title: qsTr("Send IDL instruction")
        message: root.interactionConfirmMessage()
        confirmText: qsTr("Send")
        onAccepted: root.model.sendIdlInstruction(root.pendingInstructionRequest || root.interactionRequest())
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
        if (!decodeError.length && report.decode_error !== undefined) {
            decodeError = String(report.decode_error || "")
        }
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

    function favoriteButtonText() {
        return root.model.isFavoriteEntry(root.favoriteEntry) ? qsTr("Favorited") : qsTr("Favorite")
    }

    function favoriteButtonAccessibleName() {
        return root.model.isFavoriteEntry(root.favoriteEntry) ? qsTr("Remove account from favorites") : qsTr("Add account to favorites")
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
        root.model.autoDecodeAccountData(root.detail.data_hex, root.accountCacheId(), root.ownerProgramId(), function (response) {
            if (serial !== root.decodeRequestSerial) {
                return
            }
            if (response.ok && response.value) {
                root.activeDecode = response.value
                root.activeDecodeError = ""
                root.activeIdlLabel = String(response.entry.name || qsTr("IDL"))
                root.selectedIdlTypeIndex = root.indexForTypeInIdlKey(response.entry.key, response.value.account_type)
                root.model.cacheAccountIdlSelection(root.accountCacheId(), response.entry, response.value.account_type, root.ownerProgramId())
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
                    root.model.cacheAccountIdlSelection(root.accountCacheId(), option, response.value.account_type || option.accountType, root.ownerProgramId())
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

    function resetInteractionState() {
        const entry = root.interactionIdlEntry()
        root.interactionInstructionIndex = 0
        root.interactionAccountValues = ({})
        root.interactionArgValues = ({})
        root.interactionProgramBinary = entry ? String(entry.programBinary || "") : ""
        root.pendingInstructionRequest = null
        root.model.idlInstructionPreviewValue = null
        root.model.idlInstructionError = ""
        root.interactionRevision += 1
        Qt.callLater(root.prefillInteractionAccounts)
    }

    function interactionIdlEntry() {
        if (root.selectedIdlTypeIndex >= 0 && root.selectedIdlTypeIndex < root.idlTypeOptions.length) {
            const option = root.idlTypeOptions[root.selectedIdlTypeIndex]
            const entry = root.model.idlEntryForKey(option.idlKey)
            return entry || option
        }
        return root.model.cachedIdlEntryForAccount(root.accountCacheId(), root.ownerProgramId())
    }

    function interactionIdlObject() {
        const entry = root.interactionIdlEntry()
        if (!entry || !String(entry.json || "").length) {
            return null
        }
        const parsed = BridgeHelpers.parseJson(String(entry.json || ""))
        return parsed.ok && parsed.value ? parsed.value : null
    }

    function canInteractWithIdl() {
        if (!root.detail || root.detail.private_reference || !root.activeDecode || root.activeDecodeError.length > 0) {
            return false
        }
        if (!root.model.accountDecodeFullyConsumed(root.activeDecode)) {
            return false
        }
        const idl = root.interactionIdlObject()
        return idl !== null && Array.isArray(idl.instructions) && idl.instructions.length > 0
    }

    function interactionInstructions() {
        const idl = root.interactionIdlObject()
        if (!idl || !Array.isArray(idl.instructions)) {
            return []
        }
        const rows = []
        for (let i = 0; i < idl.instructions.length; ++i) {
            const instruction = idl.instructions[i] || {}
            if (String(instruction.name || "").length > 0) {
                rows.push(instruction)
            }
        }
        return rows
    }

    function interactionInstructionLabels() {
        return root.interactionInstructions().map(instruction => String(instruction.name || ""))
    }

    function interactionInstruction() {
        const instructions = root.interactionInstructions()
        if (instructions.length === 0) {
            return null
        }
        const index = Math.max(0, Math.min(root.interactionInstructionIndex, instructions.length - 1))
        return instructions[index] || null
    }

    function selectInteractionInstruction(index) {
        root.interactionInstructionIndex = Math.max(0, Number(index || 0))
        root.interactionAccountValues = ({})
        root.interactionArgValues = ({})
        root.model.idlInstructionPreviewValue = null
        root.model.idlInstructionError = ""
        root.interactionRevision += 1
        Qt.callLater(root.prefillInteractionAccounts)
    }

    function prefillInteractionAccounts() {
        if (!root.detail) {
            return
        }
        const fields = root.interactionAccountFields()
        const currentAccount = root.accountCopyValue(root.detail)
        if (!fields.length || !currentAccount.length) {
            return
        }
        let index = 0
        for (let i = 0; i < fields.length; ++i) {
            if (fields[i].required === true) {
                index = i
                break
            }
        }
        const field = fields[index]
        const values = root.copyInteractionMap(root.interactionAccountValues)
        if (!String(values[field.name] || "").trim().length) {
            values[field.name] = currentAccount
            root.interactionAccountValues = values
            root.interactionRevision += 1
        }
    }

    function interactionAccountFields() {
        const instruction = root.interactionInstruction()
        const accounts = instruction && Array.isArray(instruction.accounts) ? instruction.accounts : []
        const rows = []
        const seen = {}
        for (let i = 0; i < accounts.length; ++i) {
            const account = accounts[i] || {}
            if (account.pda !== undefined) {
                continue
            }
            const name = String(account.name || "")
            if (!name.length) {
                continue
            }
            const rest = account.rest === true
            const signer = account.signer === true
            rows.push({
                name: name,
                label: signer ? qsTr("%1 signer").arg(root.displayLabel(name)) : root.displayLabel(name),
                placeholder: rest ? qsTr("Public/<id>, Private/<id>") : qsTr("Public/<id> or Private/<id>"),
                required: !rest,
                rest: rest
            })
            seen[name] = true
        }
        for (let j = 0; j < accounts.length; ++j) {
            const pda = accounts[j] && accounts[j].pda ? accounts[j].pda : null
            const seeds = pda && Array.isArray(pda.seeds) ? pda.seeds : []
            for (let k = 0; k < seeds.length; ++k) {
                const seed = seeds[k] || {}
                const path = String(seed.path || "")
                if (String(seed.kind || "") === "account" && path.length > 0 && seen[path] !== true) {
                    rows.push({
                        name: path,
                        label: qsTr("%1 seed").arg(root.displayLabel(path)),
                        placeholder: qsTr("Public/<id>"),
                        required: true,
                        rest: false
                    })
                    seen[path] = true
                }
            }
        }
        return rows
    }

    function interactionArgFields() {
        const instruction = root.interactionInstruction()
        const args = instruction && Array.isArray(instruction.args) ? instruction.args : []
        const rows = []
        for (let i = 0; i < args.length; ++i) {
            const arg = args[i] || {}
            const name = String(arg.name || "")
            if (!name.length) {
                continue
            }
            const typeLabel = root.interactionTypeLabel(arg.type)
            rows.push({
                name: name,
                label: qsTr("%1 (%2)").arg(root.displayLabel(name)).arg(typeLabel),
                placeholder: root.interactionPlaceholder(arg.type),
                required: true
            })
        }
        return rows
    }

    function interactionTypeLabel(typeValue) {
        if (typeof typeValue === "string") {
            return typeValue
        }
        if (!typeValue || typeof typeValue !== "object") {
            return "value"
        }
        if (typeValue.array && Array.isArray(typeValue.array)) {
            const elem = root.interactionTypeLabel(typeValue.array[0])
            const count = typeValue.array.length > 1 ? String(typeValue.array[1]) : "?"
            return "[" + elem + "; " + count + "]"
        }
        if (typeValue.vec !== undefined) {
            return "Vec<" + root.interactionTypeLabel(typeValue.vec) + ">"
        }
        if (typeValue.option !== undefined) {
            return "Option<" + root.interactionTypeLabel(typeValue.option) + ">"
        }
        if (typeValue.defined !== undefined) {
            return String(typeValue.defined || "defined")
        }
        return "value"
    }

    function interactionPlaceholder(typeValue) {
        const label = root.interactionTypeLabel(typeValue)
        if (label === "bool") {
            return qsTr("true or false")
        }
        if (label.indexOf("[u8;") === 0) {
            return qsTr("0x...")
        }
        if (label.indexOf("Vec<") === 0) {
            return qsTr("comma values")
        }
        return qsTr("value")
    }

    function interactionFieldValue(kind, name) {
        const revision = root.interactionRevision
        const values = kind === "account" ? root.interactionAccountValues : root.interactionArgValues
        return String((values || {})[name] || "")
    }

    function setInteractionFieldValue(kind, name, text) {
        const values = root.copyInteractionMap(kind === "account" ? root.interactionAccountValues : root.interactionArgValues)
        values[name] = String(text || "")
        if (kind === "account") {
            root.interactionAccountValues = values
        } else {
            root.interactionArgValues = values
        }
        root.model.idlInstructionPreviewValue = null
        root.model.idlInstructionError = ""
        root.interactionRevision += 1
    }

    function copyInteractionMap(source) {
        const copy = {}
        const current = source || {}
        for (const key in current) {
            copy[key] = current[key]
        }
        return copy
    }

    function interactionPrivateMode() {
        const values = root.interactionAccountValues || {}
        for (const key in values) {
            if (String(values[key] || "").trim().toLowerCase().indexOf("private/") === 0) {
                return true
            }
        }
        return false
    }

    function interactionInputsComplete() {
        if (!root.canInteractWithIdl() || !root.interactionInstruction()) {
            return false
        }
        const accounts = root.interactionAccountFields()
        for (let i = 0; i < accounts.length; ++i) {
            if (accounts[i].required === true && !root.interactionFieldValue("account", accounts[i].name).trim().length) {
                return false
            }
        }
        const args = root.interactionArgFields()
        for (let j = 0; j < args.length; ++j) {
            if (args[j].required === true && !root.interactionFieldValue("arg", args[j].name).trim().length) {
                return false
            }
        }
        return !root.interactionPrivateMode() || root.interactionProgramBinary.trim().length > 0
    }

    function interactionRequest() {
        const entry = root.interactionIdlEntry() || {}
        const instruction = root.interactionInstruction() || {}
        return {
            idl_json: String(entry.json || ""),
            program_id_hex: String(entry.programIdHex || root.ownerProgramId()),
            program_binary: String(root.interactionProgramBinary || "").trim(),
            dependency_binaries: [],
            instruction: String(instruction.name || ""),
            accounts: root.copyInteractionMap(root.interactionAccountValues),
            args: root.copyInteractionMap(root.interactionArgValues)
        }
    }

    function interactionPreviewText() {
        const report = root.model.idlInstructionPreviewValue
        const instruction = root.interactionInstruction()
        if (!report || !instruction || String(report.instruction || "") !== String(instruction.name || "")) {
            return ""
        }
        const tx = String(report.tx_hash || report.txHash || "")
        if (tx.length > 0) {
            return qsTr("%1 transaction %2").arg(String(report.mode || "submitted")).arg(root.shortId(tx))
        }
        const words = Array.isArray(report.instruction_words) ? report.instruction_words.length : 0
        return qsTr("%1 preview, %2 word(s)").arg(String(report.mode || "public")).arg(words)
    }

    function interactionConfirmMessage() {
        const instruction = root.interactionInstruction()
        const name = instruction ? String(instruction.name || qsTr("instruction")) : qsTr("instruction")
        if (root.interactionPrivateMode()) {
            return qsTr("Submit private transaction for %1. Wallet will execute and prove locally.").arg(name)
        }
        return qsTr("Submit public transaction for %1.").arg(name)
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

    function ownerProgramId() {
        if (!root.detail) {
            return ""
        }
        return root.detail.owner_hex.length ? root.detail.owner_hex : root.detail.owner_base58
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

}
