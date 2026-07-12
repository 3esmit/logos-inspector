import QtQml

QtObject {
    id: root

    property var model: null
    property var value: null
    readonly property var detail: normalize(value)
    readonly property var favoriteEntry: root.detail && root.model && root.model.favoriteStore ? root.model.favoriteStore.accountEntry(root.detail) : null
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

    property AccountDetailDecodeSession decodeSession: AccountDetailDecodeSession {
        id: accountDecodeSession

        model: root.model
        detail: root.detail
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
        return accountDecodeSession.accountCopyValue(detail)
    }

    function accountHeaderTooltip(detail) {
        if (!detail || !root.isNullAddress(detail.account_id_base58, detail.account_id_hex)) {
            return ""
        }
        return root.accountCopyValue(detail)
    }

    function favoriteButtonText() {
        return root.model && root.model.favoriteStore && root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry) ? qsTr("Favorited") : qsTr("Favorite")
    }

    function favoriteButtonAccessibleName() {
        return root.model && root.model.favoriteStore && root.model.favoriteStore.isFavoriteEntry(root.favoriteEntry) ? qsTr("Remove account from favorites") : qsTr("Add account to favorites")
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

    function resetDecodeState() { accountDecodeSession.resetDecodeState() }
    function sourceItems() { return accountDecodeSession.sourceItems() }
    function rebuildIdlTypeOptions() { accountDecodeSession.rebuildIdlTypeOptions() }
    function idlAccountTypes(json) { return accountDecodeSession.idlAccountTypes(json) }
    function autoSelectDecode() { accountDecodeSession.autoSelectDecode() }
    function selectIdlType(index) { accountDecodeSession.selectIdlType(index) }
    function selectTypedIdlType(text) { accountDecodeSession.selectTypedIdlType(text) }
    function resetInteractionState() { accountDecodeSession.resetInteractionState() }
    function interactionIdlEntry() { return accountDecodeSession.interactionIdlEntry() }
    function interactionIdlObject() { return accountDecodeSession.interactionIdlObject() }
    function canInteractWithIdl() { return accountDecodeSession.canInteractWithIdl() }
    function interactionInstructions() { return accountDecodeSession.interactionInstructions() }
    function interactionInstructionLabels() { return accountDecodeSession.interactionInstructionLabels() }
    function interactionInstruction() { return accountDecodeSession.interactionInstruction() }
    function selectInteractionInstruction(index) { accountDecodeSession.selectInteractionInstruction(index) }
    function prefillInteractionAccounts() { accountDecodeSession.prefillInteractionAccounts() }
    function interactionAccountFields() { return accountDecodeSession.interactionAccountFields() }
    function interactionArgFields() { return accountDecodeSession.interactionArgFields() }
    function interactionTypeLabel(typeValue) { return accountDecodeSession.interactionTypeLabel(typeValue) }
    function interactionPlaceholder(typeValue) { return accountDecodeSession.interactionPlaceholder(typeValue) }
    function interactionFieldValue(kind, name) { return accountDecodeSession.interactionFieldValue(kind, name) }
    function setInteractionFieldValue(kind, name, text) { accountDecodeSession.setInteractionFieldValue(kind, name, text) }
    function setInteractionProgramBinary(text) { accountDecodeSession.setInteractionProgramBinary(text) }
    function copyInteractionMap(source) { return accountDecodeSession.copyInteractionMap(source) }
    function interactionPrivateMode() { return accountDecodeSession.interactionPrivateMode() }
    function interactionInputsComplete() { return accountDecodeSession.interactionInputsComplete() }
    function interactionRequest() { return accountDecodeSession.interactionRequest() }
    function interactionPreviewText() { return accountDecodeSession.interactionPreviewText() }
    function interactionConfirmMessage() { return accountDecodeSession.interactionConfirmMessage() }
    function indexForType(accountType) { return accountDecodeSession.indexForType(accountType) }
    function indexForTypeInIdl(idlIndex, accountType) { return accountDecodeSession.indexForTypeInIdl(idlIndex, accountType) }
    function indexForTypeInIdlKey(idlKey, accountType) { return accountDecodeSession.indexForTypeInIdlKey(idlKey, accountType) }
    function accountCacheId() { return accountDecodeSession.accountCacheId() }
    function ownerProgramId() { return accountDecodeSession.ownerProgramId() }
    function zoneAccountEntityRef() { return accountDecodeSession.zoneAccountEntityRef() }
    function activeIdlTypeLabel() { return accountDecodeSession.activeIdlTypeLabel() }
    function decodeMessage() { return accountDecodeSession.decodeMessage() }
    function accountSocialTopic() { return accountDecodeSession.accountSocialTopic() }
    function shareIdlEntry() { return accountDecodeSession.shareIdlEntry() }
    function canShareActiveIdl() { return accountDecodeSession.canShareActiveIdl() }
    function sharedIdlSuggestionRows() { return accountDecodeSession.sharedIdlSuggestionRows() }
    function sharedIdlSuggestionText() { return accountDecodeSession.sharedIdlSuggestionText() }
    function decodedRows() { return accountDecodeSession.decodedRows() }
    function relatedRows() { return accountDecodeSession.relatedRows() }
    function resetRelatedTransactionDecodes() { accountDecodeSession.resetRelatedTransactionDecodes() }
    function decodeRelatedTransactions(serial) { accountDecodeSession.decodeRelatedTransactions(serial) }
    function tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index, partialDecoded) { accountDecodeSession.tryRelatedTransactionDecodeCandidate(serial, txHash, summary, candidates, index, partialDecoded) }
    function storeRelatedTransactionDecode(txHash, decoded) { accountDecodeSession.storeRelatedTransactionDecode(txHash, decoded) }
    function relatedTransactionDecode(txHash) { return accountDecodeSession.relatedTransactionDecode(txHash) }
    function copyRelatedTransactionDecodeMap() { return accountDecodeSession.copyRelatedTransactionDecodeMap() }
    function relatedTransactionSummary(tx) { return accountDecodeSession.relatedTransactionSummary(tx) }
    function directionText(direction) { return accountDecodeSession.directionText(direction) }
    function referenceKind(label, value) { return accountDecodeSession.referenceKind(label, value) }
    function isLongHex(value) { return accountDecodeSession.isLongHex(value) }
    function isLikelyAccount(value) { return accountDecodeSession.isLikelyAccount(value) }
    function isNullAddress(baseValue, hexValue) { return accountDecodeSession.isNullAddress(baseValue, hexValue) }
    function dataBytes(hex) {
        const text = String(hex || "").replace(/^0x/, "")
        return Math.floor(text.length / 2)
    }
    function addressLabel(baseValue, hexValue) { return accountDecodeSession.addressLabel(baseValue, hexValue) }
    function addressCopyValue(baseValue, hexValue) { return accountDecodeSession.addressCopyValue(baseValue, hexValue) }
    function hexAddressText(hex) { return accountDecodeSession.hexAddressText(hex) }
    function valueText(value) { return accountDecodeSession.valueText(value) }
    function displayLabel(value) { return accountDecodeSession.displayLabel(value) }
    function numberText(value) { return accountDecodeSession.numberText(value) }
    function shortId(value) { return accountDecodeSession.shortId(value) }
    function shortLong(value) { return accountDecodeSession.shortLong(value) }
}
