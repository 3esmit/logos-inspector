pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../state/accounts" as AccountState
import "../theme"
import "../utils/UiFormat.js" as UiFormat
import "accounts"
import "common"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property var value: null
    readonly property var detail: normalize(value)
    property alias dataView: accountDecodeSession.dataView
    property alias idlTypeOptions: accountDecodeSession.idlTypeOptions
    property alias idlTypeLabels: accountDecodeSession.idlTypeLabels
    property alias selectedIdlTypeIndex: accountDecodeSession.selectedIdlTypeIndex
    property alias activeDecode: accountDecodeSession.activeDecode
    property alias activeDecodeError: accountDecodeSession.activeDecodeError
    property alias activeIdlLabel: accountDecodeSession.activeIdlLabel
    property alias decodeRequestSerial: accountDecodeSession.decodeRequestSerial
    property alias relatedTransactionDecodeMap: accountDecodeSession.relatedTransactionDecodeMap
    property alias relatedTransactionDecodeRevision: accountDecodeSession.relatedTransactionDecodeRevision
    property alias relatedTransactionDecodeSerial: accountDecodeSession.relatedTransactionDecodeSerial
    property alias interactionInstructionIndex: accountDecodeSession.interactionInstructionIndex
    property alias interactionAccountValues: accountDecodeSession.interactionAccountValues
    property alias interactionArgValues: accountDecodeSession.interactionArgValues
    property alias interactionProgramBinary: accountDecodeSession.interactionProgramBinary
    property alias interactionRevision: accountDecodeSession.interactionRevision
    property alias pendingInstructionRequest: accountDecodeSession.pendingInstructionRequest
    readonly property string nullAddressBase58: accountDecodeSession.nullAddressBase58
    readonly property var favoriteEntry: root.detail ? root.model.favoriteStore.accountEntry(root.detail) : null

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    AccountState.AccountDetailDecodeSession {
        id: accountDecodeSession

        model: root.model
        detail: root.detail
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
            selected: root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry)
            enabled: root.favoriteEntry !== null
            Layout.preferredWidth: 118
            accessibleName: root.favoriteButtonAccessibleName()
            onClicked: root.model.favoriteStore.toggle(root.favoriteEntry)
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
            onClicked: root.model.entityNavigation.openLocalWallet(root.detail ? root.detail.account_id : "", "privateSync")
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
        onRowActivated: (linkKind, linkValue) => root.model.entityNavigation.openReference(linkKind, linkValue)
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
                        onTextEdited: text => accountDecodeSession.setInteractionProgramBinary(text)
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
                        onClicked: root.model.entityNavigation.openLocalWallet("", "profiles")
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
                                root.model.entityNavigation.openReference("transaction", modelData.txHash)
                            } else if (column === 3) {
                                root.model.entityNavigation.openReference("program", modelData.programId)
                            }
                        }
                    }
                }
            }
        }
    }

    ColumnLayout {
        visible: root.detail !== null && !root.detail.private_reference
        spacing: root.theme.gapSmall
        Layout.fillWidth: true

        RowLayout {
            spacing: root.theme.gapSmall
            Layout.fillWidth: true

            Text {
                text: qsTr("Shared IDLs")
                color: root.theme.text
                textFormat: Text.PlainText
                font.pixelSize: 14
                font.weight: Font.DemiBold
                Layout.fillWidth: true
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Find")
                enabled: root.detail !== null && root.model.socialSharedIdlReadAvailable() && root.model.sharedIdlPolicy !== "disabled"
                Layout.preferredWidth: 92
                onClicked: root.model.refreshSharedIdlsForAccount(root.accountCacheId(), root.detail ? root.detail.data_hex : "", root.ownerProgramId())
            }

            ActionButton {
                theme: root.theme
                text: qsTr("Share")
                enabled: root.canShareActiveIdl()
                Layout.preferredWidth: 96
                onClicked: shareIdlConfirm.open()
            }
        }

        StatusMessage {
            visible: root.sharedIdlSuggestionRows().length > 0
            theme: root.theme
            tone: "info"
            title: qsTr("Verified shared IDLs")
            message: root.sharedIdlSuggestionText()
            Layout.fillWidth: true
        }
    }

    SocialPanel {
        visible: root.detail !== null && !root.detail.private_reference
        theme: root.theme
        model: root.model
        title: qsTr("Account comments")
        topic: root.accountSocialTopic()
        expectedAccountId: root.accountCacheId()
        Layout.fillWidth: true
    }

    ConfirmActionPopup {
        id: instructionConfirm

        theme: root.theme
        title: qsTr("Send IDL instruction")
        message: root.interactionConfirmMessage()
        confirmText: qsTr("Send")
        onAccepted: root.model.sendIdlInstruction(root.pendingInstructionRequest || root.interactionRequest())
    }

    ConfirmActionPopup {
        id: shareIdlConfirm

        theme: root.theme
        title: qsTr("Share account IDL")
        message: qsTr("This sends the selected IDL as a public Delivery message for %1.").arg(root.shortId(root.accountCacheId()))
        confirmText: qsTr("Share")
        confirmEnabled: root.canShareActiveIdl()
        onAccepted: root.model.publishAccountIdl(root.accountCacheId(), root.ownerProgramId(), root.shareIdlEntry())
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
        return root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry) ? qsTr("Favorited") : qsTr("Favorite")
    }

    function favoriteButtonAccessibleName() {
        return root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry) ? qsTr("Remove account from favorites") : qsTr("Add account to favorites")
    }

    function resetDecodeState() {
        accountDecodeSession.resetDecodeState()
    }

    function sourceItems() {
        return accountDecodeSession.sourceItems()
    }

    function rebuildIdlTypeOptions() {
        accountDecodeSession.rebuildIdlTypeOptions()
    }

    function idlAccountTypes(json) {
        return accountDecodeSession.idlAccountTypes(json)
    }

    function autoSelectDecode() {
        accountDecodeSession.autoSelectDecode()
    }

    function selectIdlType(index) {
        accountDecodeSession.selectIdlType(index)
    }

    function selectTypedIdlType(text) {
        accountDecodeSession.selectTypedIdlType(text)
    }

    function resetInteractionState() {
        accountDecodeSession.resetInteractionState()
    }

    function interactionIdlEntry() {
        return accountDecodeSession.interactionIdlEntry()
    }

    function interactionIdlObject() {
        return accountDecodeSession.interactionIdlObject()
    }

    function canInteractWithIdl() {
        return accountDecodeSession.canInteractWithIdl()
    }

    function interactionInstructions() {
        return accountDecodeSession.interactionInstructions()
    }

    function interactionInstructionLabels() {
        return accountDecodeSession.interactionInstructionLabels()
    }

    function interactionInstruction() {
        return accountDecodeSession.interactionInstruction()
    }

    function selectInteractionInstruction(index) {
        accountDecodeSession.selectInteractionInstruction(index)
    }

    function prefillInteractionAccounts() {
        accountDecodeSession.prefillInteractionAccounts()
    }

    function interactionAccountFields() {
        return accountDecodeSession.interactionAccountFields()
    }

    function interactionArgFields() {
        return accountDecodeSession.interactionArgFields()
    }

    function interactionTypeLabel(typeValue) {
        return accountDecodeSession.interactionTypeLabel(typeValue)
    }

    function interactionPlaceholder(typeValue) {
        return accountDecodeSession.interactionPlaceholder(typeValue)
    }

    function interactionFieldValue(kind, name) {
        return accountDecodeSession.interactionFieldValue(kind, name)
    }

    function setInteractionFieldValue(kind, name, text) {
        accountDecodeSession.setInteractionFieldValue(kind, name, text)
    }

    function copyInteractionMap(source) {
        return accountDecodeSession.copyInteractionMap(source)
    }

    function interactionPrivateMode() {
        return accountDecodeSession.interactionPrivateMode()
    }

    function interactionInputsComplete() {
        return accountDecodeSession.interactionInputsComplete()
    }

    function interactionRequest() {
        return accountDecodeSession.interactionRequest()
    }

    function interactionPreviewText() {
        return accountDecodeSession.interactionPreviewText()
    }

    function interactionConfirmMessage() {
        return accountDecodeSession.interactionConfirmMessage()
    }

    function indexForType(accountType) {
        return accountDecodeSession.indexForType(accountType)
    }

    function indexForTypeInIdl(idlIndex, accountType) {
        return accountDecodeSession.indexForTypeInIdl(idlIndex, accountType)
    }

    function indexForTypeInIdlKey(idlKey, accountType) {
        return accountDecodeSession.indexForTypeInIdlKey(idlKey, accountType)
    }

    function accountCacheId() {
        return accountDecodeSession.accountCacheId()
    }

    function ownerProgramId() {
        return accountDecodeSession.ownerProgramId()
    }

    function activeIdlTypeLabel() {
        return accountDecodeSession.activeIdlTypeLabel()
    }

    function decodeMessage() {
        return accountDecodeSession.decodeMessage()
    }

    function accountSocialTopic() {
        return accountDecodeSession.accountSocialTopic()
    }

    function shareIdlEntry() {
        return accountDecodeSession.shareIdlEntry()
    }

    function canShareActiveIdl() {
        return accountDecodeSession.canShareActiveIdl()
    }

    function sharedIdlSuggestionRows() {
        return accountDecodeSession.sharedIdlSuggestionRows()
    }

    function sharedIdlSuggestionText() {
        return accountDecodeSession.sharedIdlSuggestionText()
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
        return accountDecodeSession.decodedRows()
    }

    function relatedRows() {
        return accountDecodeSession.relatedRows()
    }

    function resetRelatedTransactionDecodes() {
        accountDecodeSession.resetRelatedTransactionDecodes()
    }

    function decodeRelatedTransactions(serial) {
        accountDecodeSession.decodeRelatedTransactions(serial)
    }

    function tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index, partialDecoded) {
        accountDecodeSession.tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index, partialDecoded)
    }

    function storeRelatedTransactionDecode(txHash, decoded) {
        accountDecodeSession.storeRelatedTransactionDecode(txHash, decoded)
    }

    function relatedTransactionDecode(txHash) {
        return accountDecodeSession.relatedTransactionDecode(txHash)
    }

    function copyRelatedTransactionDecodeMap() {
        return accountDecodeSession.copyRelatedTransactionDecodeMap()
    }

    function relatedTransactionSummary(tx) {
        return accountDecodeSession.relatedTransactionSummary(tx)
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
        return UiFormat.valueText(value, {
            emptyText: "-",
            objectMode: "json"
        })
    }

    function displayLabel(value) {
        const text = String(value || "").replace(/[._-]+/g, " ").trim()
        return text.length ? text : qsTr("Field")
    }

    function numberText(value) {
        return UiFormat.numberText(value, {
            emptyText: "-",
            coerceNumericStrings: true
        })
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
