import QtQml
import "../utils/UiFormat.js" as UiFormat

QtObject {
    id: root

    property var entries: []
    property int revision: 0
    property string filter: "all"

    signal openRequested(string openKind, string value, var entityRef)

    function clear() {
        entries = []
        revision = 0
        filter = "all"
    }

    function load(value) {
        entries = normalizedEntries(value)
        revision += 1
    }

    function payload() {
        return normalizedEntries(entries)
    }

    function normalizedEntries(value) {
        const rows = Array.isArray(value) ? value : []
        const result = []
        const seen = ({})
        for (let i = 0; i < rows.length; ++i) {
            const entry = normalizedEntry(rows[i])
            if (!entry) {
                continue
            }
            const key = favoriteKey(entry)
            if (seen[key] === true) {
                continue
            }
            seen[key] = true
            result.push(entry)
        }
        return result
    }

    function normalizedEntry(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return null
        }
        const kind = normalizedKind(value.kind)
        const item = {
            kind: kind,
            layer: normalizedLayer(value.layer),
            value: String(value.value || "").trim(),
            open_kind: String(value.open_kind || value.openKind || "").trim(),
            title: String(value.title || "").trim(),
            subtitle: String(value.subtitle || "").trim(),
            created_at: String(value.created_at || value.createdAt || "").trim(),
            entity_ref: normalizedEntityRef(value.entity_ref || value.entityRef)
        }
        if (!kind.length || !item.value.length) {
            return null
        }
        if (item.layer === "l2" && !item.entity_ref) {
            return null
        }
        if (!item.open_kind.length) {
            item.open_kind = defaultOpenKind(item)
        }
        if (!item.title.length) {
            item.title = defaultTitle(item)
        }
        if (!item.created_at.length) {
            item.created_at = new Date().toISOString()
        }
        return item
    }

    function normalizedKind(value) {
        const kind = String(value || "").toLowerCase()
        if (kind === "account" || kind === "transaction" || kind === "block"
                || kind === "program") {
            return kind
        }
        return ""
    }

    function normalizedLayer(value) {
        const layer = String(value || "").toLowerCase()
        if (layer === "l1" || layer === "l2") {
            return layer
        }
        return ""
    }

    function defaultOpenKind(entry) {
        if (entry.kind === "account") {
            return "account"
        }
        if (entry.kind === "transaction") {
            return entry.layer === "l1" ? "mantleTransaction" : "transaction"
        }
        if (entry.kind === "block") {
            return entry.layer === "l2" ? "indexerBlock" : "block"
        }
        if (entry.kind === "program") {
            return "program"
        }
        return ""
    }

    function defaultTitle(entry) {
        if (entry.kind === "account") {
            return qsTr("Account %1").arg(shortValue(entry.value))
        }
        if (entry.kind === "transaction") {
            return entry.layer === "l1"
                ? qsTr("Mantle transaction %1").arg(shortValue(entry.value))
                : qsTr("LEZ transaction %1").arg(shortValue(entry.value))
        }
        if (entry.kind === "block") {
            return entry.layer === "l2"
                ? qsTr("LEZ block %1").arg(shortValue(entry.value))
                : qsTr("Block %1").arg(shortValue(entry.value))
        }
        if (entry.kind === "program") {
            return qsTr("Program %1").arg(shortValue(entry.value))
        }
        return shortValue(entry.value)
    }

    function favoriteKey(entry) {
        const item = normalizedEntry(entry)
        if (!item) {
            return ""
        }
        if (item.layer === "l2" && item.entity_ref) {
            return "l2:" + JSON.stringify(item.entity_ref)
        }
        return [item.kind, item.layer, item.open_kind, normalizedValue(item.value)].join(":")
    }

    function normalizedValue(value) {
        const text = String(value || "").trim()
        if (text.indexOf("0x") === 0 && text.length === 66) {
            return text.slice(2).toLowerCase()
        }
        return text.toLowerCase()
    }

    function blockEntry(value, entityRef) {
        const detail = value && typeof value === "object" ? value : null
        if (!detail) {
            return null
        }
        const l2 = detail.type === "indexer_block" || detail.type === "sequencer_block"
            || (entityRef && String(entityRef.entity_kind || "") === "block")
        const hash = String(detail.hash || "")
        const id = l2 ? String(detail.block_id !== undefined && detail.block_id !== null ? detail.block_id : detail.slot || "") : String(detail.slot || "")
        const favoriteValue = hash.length ? hash : id
        if (!favoriteValue.length) {
            return null
        }
        const reference = l2 ? normalizedEntityRef(entityRef) : null
        if (l2 && !reference) {
            return null
        }
        return {
            kind: "block",
            layer: l2 ? "l2" : "l1",
            value: favoriteValue,
            open_kind: l2 ? (hash.length ? "indexerBlock" : "lezBlock") : "block",
            title: l2
                ? qsTr("LEZ block %1").arg(id.length ? id : shortValue(hash))
                : qsTr("Block at slot %1").arg(id.length ? id : shortValue(hash)),
            subtitle: hash.length ? hash : (l2 ? qsTr("LEZ block id") : qsTr("L1 slot")),
            created_at: new Date().toISOString(),
            entity_ref: reference
        }
    }

    function transactionEntry(value, entityRef) {
        const detail = value && typeof value === "object" ? value : null
        if (!detail) {
            return null
        }
        const hash = String(detail.hash || "")
        if (!hash.length) {
            return null
        }
        const l1 = String(detail.mode || "") === "blockchain"
        const reference = l1 ? null : normalizedEntityRef(entityRef)
        if (!l1 && !reference) {
            return null
        }
        return {
            kind: "transaction",
            layer: l1 ? "l1" : "l2",
            value: hash,
            open_kind: l1 ? "mantleTransaction" : "transaction",
            title: l1
                ? qsTr("Mantle transaction %1").arg(shortValue(hash))
                : qsTr("LEZ transaction %1").arg(shortValue(hash)),
            subtitle: l1 ? qsTr("Slot %1").arg(UiFormat.valueText(detail.slot)) : String(detail.kind || qsTr("LEZ transaction")),
            created_at: new Date().toISOString(),
            entity_ref: reference
        }
    }

    function accountEntry(value, entityRef) {
        const detail = value && typeof value === "object" ? value : null
        if (!detail) {
            return null
        }
        const accountId = String(detail.account_id_base58 || detail.account_id || detail.account_id_hex || "").trim()
        if (!accountId.length) {
            return null
        }
        const reference = normalizedEntityRef(entityRef)
        if (!reference) {
            return null
        }
        return {
            kind: "account",
            layer: "l2",
            value: accountId,
            open_kind: "account",
            title: detail.private_reference ? qsTr("Private account %1").arg(shortValue(accountId)) : qsTr("Account %1").arg(shortValue(accountId)),
            subtitle: String(detail.owner_base58 || detail.owner_hex || detail.account_id_hex || ""),
            created_at: new Date().toISOString(),
            entity_ref: reference
        }
    }

    function programEntry(value, entityRef) {
        const program = value && typeof value === "object" ? value : null
        const reference = normalizedEntityRef(entityRef)
        const programId = String(program && (program.hex || program.base58)
            || reference && reference.canonical_key || "")
        if (!reference || !programId.length) {
            return null
        }
        return {
            kind: "program",
            layer: "l2",
            value: programId,
            open_kind: "program",
            title: qsTr("Program %1").arg(shortValue(programId)),
            subtitle: String(reference.channel_id || ""),
            created_at: new Date().toISOString(),
            entity_ref: reference
        }
    }

    function l2EntityEntry(entityRef, title, subtitle) {
        const reference = normalizedEntityRef(entityRef)
        if (!reference) {
            return null
        }
        const kind = normalizedKind(reference.entity_kind)
        return normalizedEntry({
            kind: kind,
            layer: "l2",
            value: reference.canonical_key,
            open_kind: kind === "block" ? "lezBlock" : kind,
            title: String(title || defaultTitle({
                kind: kind,
                layer: "l2",
                value: reference.canonical_key
            })),
            subtitle: String(subtitle || reference.channel_id),
            created_at: new Date().toISOString(),
            entity_ref: reference
        })
    }

    function rows(targetFilter) {
        const target = normalizedKind(targetFilter)
        const rows = normalizedEntries(entries)
        const filtered = target.length ? rows.filter(function (entry) { return entry.kind === target }) : rows
        filtered.sort(function (left, right) {
            return String(right.created_at || "").localeCompare(String(left.created_at || ""))
        })
        return filtered
    }

    function count(targetFilter) {
        return rows(targetFilter).length
    }

    function isFavoriteEntry(entry) {
        const key = favoriteKey(entry)
        if (!key.length) {
            return false
        }
        const rows = Array.isArray(entries) ? entries : []
        for (let i = 0; i < rows.length; ++i) {
            if (favoriteKey(rows[i]) === key) {
                return true
            }
        }
        return false
    }

    function add(entry) {
        const item = normalizedEntry(entry)
        if (!item) {
            return false
        }
        const key = favoriteKey(item)
        const rows = Array.isArray(entries) ? entries.slice(0) : []
        for (let i = 0; i < rows.length; ++i) {
            if (favoriteKey(rows[i]) === key) {
                rows[i] = item
                entries = rows
                revision += 1
                return true
            }
        }
        rows.unshift(item)
        entries = rows
        revision += 1
        return true
    }

    function remove(entryOrKey) {
        const key = typeof entryOrKey === "string" ? String(entryOrKey || "") : favoriteKey(entryOrKey)
        if (!key.length) {
            return false
        }
        const rows = Array.isArray(entries) ? entries.slice(0) : []
        const next = rows.filter(function (entry) {
            return favoriteKey(entry) !== key
        })
        if (next.length === rows.length) {
            return false
        }
        entries = next
        revision += 1
        return true
    }

    function toggle(entry) {
        if (isFavoriteEntry(entry)) {
            return remove(entry)
        }
        return add(entry)
    }

    function open(entry) {
        const item = normalizedEntry(entry)
        if (!item) {
            return
        }
        let entity = null
        if (item.entity_ref) {
            entity = {
                layer: "l2",
                network_scope: item.entity_ref.network_scope,
                channel_id: item.entity_ref.channel_id,
                zone_kind: item.entity_ref.zone_kind,
                entity_kind: item.entity_ref.entity_kind,
                canonical_key: item.entity_ref.canonical_key,
                source: item.entity_ref.source
            }
        }
        openRequested(item.open_kind, item.value, entity)
    }

    function kindLabel(kind) {
        const value = normalizedKind(kind)
        if (value === "account") {
            return qsTr("Account")
        }
        if (value === "transaction") {
            return qsTr("Transaction")
        }
        if (value === "block") {
            return qsTr("Block")
        }
        if (value === "program") {
            return qsTr("Program")
        }
        return qsTr("Favorite")
    }

    function layerLabel(layer) {
        const value = normalizedLayer(layer)
        if (value === "l1") {
            return qsTr("L1")
        }
        if (value === "l2") {
            return qsTr("L2")
        }
        return qsTr("-")
    }

    function shortValue(value) {
        return UiFormat.shortHash(String(value || ""))
    }

    function normalizedEntityRef(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)
                || value.endpoint !== undefined || value.module_id !== undefined
                || value.context_revision !== undefined || value.request_revision !== undefined) {
            return null
        }
        const networkScope = normalizedNetworkScope(value.network_scope)
        const channelId = String(value.channel_id || "").trim()
        const zoneKind = String(value.zone_kind || "").trim()
        const entityKind = String(value.entity_kind || "").trim()
        const canonicalKey = String(value.canonical_key || "").trim()
        const source = normalizedSourceQualifier(value.source)
        if (!networkScope || !channelId.length
                || !zoneKind.length || !normalizedKind(entityKind).length
                || !canonicalKey.length || !source
                || unsafeReferenceText(channelId) || unsafeReferenceText(canonicalKey)) {
            return null
        }
        return {
            network_scope: networkScope,
            channel_id: channelId,
            zone_kind: zoneKind,
            entity_kind: entityKind,
            canonical_key: canonicalKey,
            source: source
        }
    }

    function normalizedSourceQualifier(value) {
        const source = value && typeof value === "object" ? value : ({ kind: "policy" })
        const kind = String(source.kind || "policy")
        if (kind === "policy") {
            return { kind: "policy" }
        }
        const sourceId = String(source.source_id || "").trim()
        const sourceRole = String(source.source_role || "").trim()
        if (kind !== "exact" || !sourceId.length || unsafeReferenceText(sourceId)
                || (sourceRole !== "indexer" && sourceRole !== "sequencer")) {
            return null
        }
        return {
            kind: "exact",
            source_id: sourceId,
            source_role: sourceRole
        }
    }

    function normalizedNetworkScope(value) {
        if (!value || typeof value !== "object" || Array.isArray(value)) {
            return null
        }
        const kind = String(value.kind || "")
        if (kind === "genesis_id") {
            const genesisId = String(value.genesis_id || "").trim()
            return genesisId.length > 0 && !unsafeReferenceText(genesisId)
                ? { kind: kind, genesis_id: genesisId } : null
        }
        if (kind === "finalized_anchor") {
            const genesisTime = String(value.genesis_time || "").trim()
            const blockId = String(value.block_id || "").trim()
            const parentId = String(value.parent_id || "").trim()
            const blockSlot = Number(value.block_slot)
            if (!genesisTime.length || !blockId.length || !parentId.length
                    || !Number.isFinite(blockSlot) || blockSlot < 0
                    || unsafeReferenceText(blockId) || unsafeReferenceText(parentId)) {
                return null
            }
            return {
                kind: kind,
                genesis_time: genesisTime,
                block_slot: Math.floor(blockSlot),
                block_id: blockId,
                parent_id: parentId
            }
        }
        return null
    }

    function unsafeReferenceText(value) {
        const text = String(value || "")
        return text.indexOf("://") >= 0 || text.indexOf("\n") >= 0
            || text.indexOf("\r") >= 0
    }
}
