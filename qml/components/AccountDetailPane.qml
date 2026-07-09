pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Layouts
import "../state"
import "../state/accounts" as AccountState
import "../theme"
import "accounts"
import "common"

ColumnLayout {
    id: root

    required property Theme theme
    required property AppModel model
    property var value: null
    readonly property var detail: accountWorkspace.detail
    property alias dataView: accountWorkspace.dataView
    property alias idlTypeOptions: accountWorkspace.idlTypeOptions
    property alias idlTypeLabels: accountWorkspace.idlTypeLabels
    property alias selectedIdlTypeIndex: accountWorkspace.selectedIdlTypeIndex
    property alias activeDecode: accountWorkspace.activeDecode
    property alias activeDecodeError: accountWorkspace.activeDecodeError
    property alias activeIdlLabel: accountWorkspace.activeIdlLabel
    property alias decodeRequestSerial: accountWorkspace.decodeRequestSerial
    property alias relatedTransactionDecodeMap: accountWorkspace.relatedTransactionDecodeMap
    property alias relatedTransactionDecodeRevision: accountWorkspace.relatedTransactionDecodeRevision
    property alias relatedTransactionDecodeSerial: accountWorkspace.relatedTransactionDecodeSerial
    property alias interactionInstructionIndex: accountWorkspace.interactionInstructionIndex
    property alias interactionAccountValues: accountWorkspace.interactionAccountValues
    property alias interactionArgValues: accountWorkspace.interactionArgValues
    property alias interactionProgramBinary: accountWorkspace.interactionProgramBinary
    property alias interactionRevision: accountWorkspace.interactionRevision
    property alias pendingInstructionRequest: accountWorkspace.pendingInstructionRequest
    readonly property string nullAddressBase58: accountWorkspace.nullAddressBase58
    readonly property var favoriteEntry: accountWorkspace.favoriteEntry

    visible: detail !== null
    spacing: 14
    Layout.fillWidth: true

    AccountState.AccountDetailInspectionWorkspace {
        id: accountWorkspace

        model: root.model
        value: root.value
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
                            onTextEdited: text => accountWorkspace.setInteractionProgramBinary(text)
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
        return accountWorkspace.normalize(value)
    }

    function accountHeader(detail) {
        return accountWorkspace.accountHeader(detail)
    }

    function accountAlternate(detail) {
        return accountWorkspace.accountAlternate(detail)
    }

    function accountCopyValue(detail) {
        return accountWorkspace.accountCopyValue(detail)
    }

    function accountHeaderTooltip(detail) {
        return accountWorkspace.accountHeaderTooltip(detail)
    }

    function favoriteButtonText() {
        return accountWorkspace.favoriteButtonText()
    }

    function favoriteButtonAccessibleName() {
        return accountWorkspace.favoriteButtonAccessibleName()
    }

    function resetDecodeState() {
        accountWorkspace.resetDecodeState()
    }

    function sourceItems() {
        return accountWorkspace.sourceItems()
    }

    function rebuildIdlTypeOptions() {
        accountWorkspace.rebuildIdlTypeOptions()
    }

    function idlAccountTypes(json) {
        return accountWorkspace.idlAccountTypes(json)
    }

    function autoSelectDecode() {
        accountWorkspace.autoSelectDecode()
    }

    function selectIdlType(index) {
        accountWorkspace.selectIdlType(index)
    }

    function selectTypedIdlType(text) {
        accountWorkspace.selectTypedIdlType(text)
    }

    function resetInteractionState() {
        accountWorkspace.resetInteractionState()
    }

    function interactionIdlEntry() {
        return accountWorkspace.interactionIdlEntry()
    }

    function interactionIdlObject() {
        return accountWorkspace.interactionIdlObject()
    }

    function canInteractWithIdl() {
        return accountWorkspace.canInteractWithIdl()
    }

    function interactionInstructions() {
        return accountWorkspace.interactionInstructions()
    }

    function interactionInstructionLabels() {
        return accountWorkspace.interactionInstructionLabels()
    }

    function interactionInstruction() {
        return accountWorkspace.interactionInstruction()
    }

    function selectInteractionInstruction(index) {
        accountWorkspace.selectInteractionInstruction(index)
    }

    function prefillInteractionAccounts() {
        accountWorkspace.prefillInteractionAccounts()
    }

    function interactionAccountFields() {
        return accountWorkspace.interactionAccountFields()
    }

    function interactionArgFields() {
        return accountWorkspace.interactionArgFields()
    }

    function interactionTypeLabel(typeValue) {
        return accountWorkspace.interactionTypeLabel(typeValue)
    }

    function interactionPlaceholder(typeValue) {
        return accountWorkspace.interactionPlaceholder(typeValue)
    }

    function interactionFieldValue(kind, name) {
        return accountWorkspace.interactionFieldValue(kind, name)
    }

    function setInteractionFieldValue(kind, name, text) {
        accountWorkspace.setInteractionFieldValue(kind, name, text)
    }

    function copyInteractionMap(source) {
        return accountWorkspace.copyInteractionMap(source)
    }

    function interactionPrivateMode() {
        return accountWorkspace.interactionPrivateMode()
    }

    function interactionInputsComplete() {
        return accountWorkspace.interactionInputsComplete()
    }

    function interactionRequest() {
        return accountWorkspace.interactionRequest()
    }

    function interactionPreviewText() {
        return accountWorkspace.interactionPreviewText()
    }

    function interactionConfirmMessage() {
        return accountWorkspace.interactionConfirmMessage()
    }

    function indexForType(accountType) {
        return accountWorkspace.indexForType(accountType)
    }

    function indexForTypeInIdl(idlIndex, accountType) {
        return accountWorkspace.indexForTypeInIdl(idlIndex, accountType)
    }

    function indexForTypeInIdlKey(idlKey, accountType) {
        return accountWorkspace.indexForTypeInIdlKey(idlKey, accountType)
    }

    function accountCacheId() {
        return accountWorkspace.accountCacheId()
    }

    function ownerProgramId() {
        return accountWorkspace.ownerProgramId()
    }

    function activeIdlTypeLabel() {
        return accountWorkspace.activeIdlTypeLabel()
    }

    function decodeMessage() {
        return accountWorkspace.decodeMessage()
    }

    function accountSocialTopic() {
        return accountWorkspace.accountSocialTopic()
    }

    function shareIdlEntry() {
        return accountWorkspace.shareIdlEntry()
    }

    function canShareActiveIdl() {
        return accountWorkspace.canShareActiveIdl()
    }

    function sharedIdlSuggestionRows() {
        return accountWorkspace.sharedIdlSuggestionRows()
    }

    function sharedIdlSuggestionText() {
        return accountWorkspace.sharedIdlSuggestionText()
    }

    function accountRows() {
        return accountWorkspace.accountRows()
    }

    function decodedRows() {
        return accountWorkspace.decodedRows()
    }

    function relatedRows() {
        return accountWorkspace.relatedRows()
    }

    function resetRelatedTransactionDecodes() {
        accountWorkspace.resetRelatedTransactionDecodes()
    }

    function decodeRelatedTransactions(serial) {
        accountWorkspace.decodeRelatedTransactions(serial)
    }

    function tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index, partialDecoded) {
        accountWorkspace.tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index, partialDecoded)
    }

    function storeRelatedTransactionDecode(txHash, decoded) {
        accountWorkspace.storeRelatedTransactionDecode(txHash, decoded)
    }

    function relatedTransactionDecode(txHash) {
        return accountWorkspace.relatedTransactionDecode(txHash)
    }

    function copyRelatedTransactionDecodeMap() {
        return accountWorkspace.copyRelatedTransactionDecodeMap()
    }

    function relatedTransactionSummary(tx) {
        return accountWorkspace.relatedTransactionSummary(tx)
    }

    function directionText(direction) {
        return accountWorkspace.directionText(direction)
    }

    function referenceKind(label, value) {
        return accountWorkspace.referenceKind(label, value)
    }

    function isLongHex(value) {
        return accountWorkspace.isLongHex(value)
    }

    function isLikelyAccount(value) {
        return accountWorkspace.isLikelyAccount(value)
    }

    function dataBytes(hex) {
        return accountWorkspace.dataBytes(hex)
    }

    function hexAddressText(hex) {
        return accountWorkspace.hexAddressText(hex)
    }

    function addressLabel(baseValue, hexValue) {
        return accountWorkspace.addressLabel(baseValue, hexValue)
    }

    function addressCopyValue(baseValue, hexValue) {
        return accountWorkspace.addressCopyValue(baseValue, hexValue)
    }

    function isNullAddress(baseValue, hexValue) {
        return accountWorkspace.isNullAddress(baseValue, hexValue)
    }

    function valueText(value) {
        return accountWorkspace.valueText(value)
    }

    function displayLabel(value) {
        return accountWorkspace.displayLabel(value)
    }

    function numberText(value) {
        return accountWorkspace.numberText(value)
    }

    function shortId(value) {
        return accountWorkspace.shortId(value)
    }

    function shortLong(value) {
        return accountWorkspace.shortLong(value)
    }

}
