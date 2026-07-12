import QtQml
import "../../services/BridgeHelpers.js" as BridgeHelpers
import "../../utils/UiFormat.js" as UiFormat
import "AccountInteractionState.js" as AccountInteraction

QtObject {
    id: root

    property var model: null
    property var detail: null
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

    onDetailChanged: {
        Qt.callLater(root.resetDecodeState)
        Qt.callLater(root.resetRelatedTransactionDecodes)
        Qt.callLater(root.resetInteractionState)
    }

    onActiveDecodeChanged: Qt.callLater(root.resetInteractionState)

    property Connections registeredIdlConnections: Connections {
        target: root.model && root.model.registeredIdls ? root.model.registeredIdls : null

        function onCountChanged() {
            Qt.callLater(root.resetDecodeState)
            Qt.callLater(root.resetRelatedTransactionDecodes)
            Qt.callLater(root.resetInteractionState)
        }
    }

    property Connections modelConnections: Connections {
        target: root.model

        function onSharedIdlRevisionChanged() {
            Qt.callLater(root.resetDecodeState)
        }
    }

    function resetDecodeState() {
        root.decodeRequestSerial += 1
        if (!root.detail || !root.model) {
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
        const entityRef = root.zoneAccountEntityRef()
        if (!root.detail.private_reference && entityRef && root.detail.data_hex.length
                && typeof root.model.refreshSharedIdlsForAccount === "function") {
            root.model.refreshSharedIdlsForAccount(entityRef, root.detail.data_hex, root.ownerProgramId())
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
        const idls = root.model && root.model.registeredIdls ? root.model.registeredIdls : null
        const count = idls ? idls.count : 0
        for (let i = 0; i < count; ++i) {
            const idl = idls.get(i)
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
        if (!root.model || !root.detail || !root.detail.data_hex.length) {
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
                const entityRef = root.zoneAccountEntityRef()
                if (entityRef) {
                    root.model.maybeAutoShareAccountIdl(entityRef, root.ownerProgramId(), response.entry)
                }
            } else {
                root.activeDecodeError = response.error || ""
            }
        })
    }

    function selectIdlType(index) {
        if (!root.model || !root.detail || index < 0 || index >= root.idlTypeOptions.length) {
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
                    const entityRef = root.zoneAccountEntityRef()
                    if (entityRef) {
                        root.model.maybeAutoShareAccountIdl(entityRef, root.ownerProgramId(), option)
                    }
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
        if (root.model) {
            root.model.idlInstructionPreviewValue = null
            root.model.idlInstructionError = ""
        }
        root.interactionRevision += 1
        Qt.callLater(root.prefillInteractionAccounts)
    }

    function interactionIdlEntry() {
        if (!root.model) {
            return null
        }
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
        if (!root.model || !root.detail || root.detail.private_reference || !root.activeDecode || root.activeDecodeError.length > 0) {
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
        if (root.model) {
            root.model.idlInstructionPreviewValue = null
            root.model.idlInstructionError = ""
        }
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
        return AccountInteraction.accountFields(root, root.interactionInstruction())
    }

    function interactionArgFields() {
        return AccountInteraction.argFields(root, root.interactionInstruction())
    }

    function interactionTypeLabel(typeValue) {
        return AccountInteraction.typeLabelText(typeValue)
    }

    function interactionPlaceholder(typeValue) {
        return AccountInteraction.placeholder(typeValue)
    }

    function interactionFieldValue(kind, name) {
        return AccountInteraction.fieldValue(root, kind, name)
    }

    function setInteractionFieldValue(kind, name, text) {
        const values = root.copyInteractionMap(kind === "account" ? root.interactionAccountValues : root.interactionArgValues)
        values[name] = String(text || "")
        if (kind === "account") {
            root.interactionAccountValues = values
        } else {
            root.interactionArgValues = values
        }
        if (root.model) {
            root.model.idlInstructionPreviewValue = null
            root.model.idlInstructionError = ""
        }
        root.interactionRevision += 1
    }

    function setInteractionProgramBinary(text) {
        root.interactionProgramBinary = String(text || "")
        root.interactionRevision += 1
    }

    function copyInteractionMap(source) {
        return AccountInteraction.copyMap(source)
    }

    function interactionPrivateMode() {
        return AccountInteraction.privateMode(root)
    }

    function interactionInputsComplete() {
        return AccountInteraction.inputsComplete(root)
    }

    function interactionRequest() {
        return AccountInteraction.request(root)
    }

    function interactionPreviewText() {
        return AccountInteraction.previewText(root)
    }

    function interactionConfirmMessage() {
        return AccountInteraction.confirmMessage(root)
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

    function zoneAccountEntityRef() {
        const value = root.model && root.model.currentInspectionEntityRef
            ? root.model.currentInspectionEntityRef : null
        if (!value || String(value.layer || "") !== "l2"
                || String(value.entity_kind || "") !== "account"
                || String(value.canonical_key || "") !== root.accountCacheId()) {
            return null
        }
        return {
            network_scope: value.network_scope,
            channel_id: String(value.channel_id || ""),
            zone_kind: String(value.zone_kind || "unknown"),
            entity_kind: "account",
            canonical_key: String(value.canonical_key || ""),
            source: value.source || { kind: "policy" }
        }
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
        const idlCount = root.model && root.model.registeredIdls ? root.model.registeredIdls.count : 0
        if (root.activeDecodeError.length) {
            return root.activeDecodeError
        }
        if (!root.detail || !root.detail.data_hex.length) {
            return qsTr("No account data is available.")
        }
        if (idlCount === 0 && root.sharedIdlSuggestionRows().length === 0) {
            return qsTr("Register an account IDL in Programs to decode this data.")
        }
        return qsTr("No registered IDL type decoded this data.")
    }

    function accountSocialTopic() {
        const entityRef = root.zoneAccountEntityRef()
        return entityRef && root.model ? root.model.socialZoneCommentTopic(entityRef) : ""
    }

    function shareIdlEntry() {
        return root.interactionIdlEntry()
    }

    function canShareActiveIdl() {
        const entry = root.shareIdlEntry()
        const entityRef = root.zoneAccountEntityRef()
        const topic = entityRef && root.model
            ? root.model.socialZoneAccountIdlTopic(entityRef) : ""
        return root.model !== null
            && entityRef !== null
            && root.detail !== null
            && !root.detail.private_reference
            && root.activeDecode !== null
            && root.model.accountDecodeFullyConsumed(root.activeDecode)
            && entry !== null
            && String(entry.json || "").length > 0
            && String(entry.source || "") !== "shared"
            && root.model.socialSharedIdlWriteAvailable(topic)
    }

    function sharedIdlSuggestionRows() {
        if (!root.model || typeof root.model.sharedIdlSuggestions !== "function") {
            return []
        }
        const revision = root.model.sharedIdlRevision
        return root.model.sharedIdlSuggestions(root.accountCacheId(), root.ownerProgramId())
    }

    function sharedIdlSuggestionText() {
        const rows = root.sharedIdlSuggestionRows()
        if (!rows.length) {
            return ""
        }
        return rows.map(function (entry) {
            return String(entry.name || qsTr("Shared IDL"))
        }).join(", ")
    }

    function decodedRows() {
        return AccountInteraction.decodedRows(root)
    }

    function relatedRows() {
        return AccountInteraction.relatedRows(root)
    }

    function resetRelatedTransactionDecodes() {
        const idlCount = root.model && root.model.registeredIdls ? root.model.registeredIdls.count : 0
        root.relatedTransactionDecodeSerial += 1
        root.relatedTransactionDecodeMap = ({})
        root.relatedTransactionDecodeRevision += 1
        if (!root.detail || !root.detail.related_transactions.length || idlCount === 0) {
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
        if (!root.model || serial !== root.relatedTransactionDecodeSerial) {
            return
        }
        const remaining = Array.isArray(candidates) ? candidates.slice(Math.max(0, Number(index || 0))) : []
        if (remaining.length === 0) {
            if (partialDecoded) {
                root.storeRelatedTransactionDecode(txHash, partialDecoded)
            }
            return
        }

        root.model.resolveTransactionDecodeSessionAsync(summary, root.model.programDecodeCandidatePayload(remaining), function (response) {
            if (serial !== root.relatedTransactionDecodeSerial) {
                return
            }
            const decoded = root.model.transactionDecodeSessionInstruction(response)
            if (decoded) {
                root.storeRelatedTransactionDecode(txHash, decoded)
                return
            }
            if (partialDecoded) {
                root.storeRelatedTransactionDecode(txHash, partialDecoded)
            }
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
        return AccountInteraction.relatedTransactionSummary(tx)
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

    function isNullAddress(baseValue, hexValue) {
        const base = String(baseValue || "")
        const explicitHex = String(hexValue || "").replace(/^0x/, "")
        const inferredHex = explicitHex.length > 0
            ? explicitHex
            : (/^(0x)?[0-9a-fA-F]{32,}$/.test(base) ? base.replace(/^0x/, "") : "")
        return base === root.nullAddressBase58 || (inferredHex.length > 0 && /^[0]+$/.test(inferredHex))
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

    function hexAddressText(hex) {
        const text = String(hex || "").replace(/^0x/, "")
        return text.length ? "0x" + text : ""
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
