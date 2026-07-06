import QtQml
import "../utils/UiFormat.js" as UiFormat

QtObject {
    id: root

    property var entries: []
    property int revision: 0
    property string filter: "all"

    signal openRequested(string openKind, string value)

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
            created_at: String(value.created_at || value.createdAt || "").trim()
        }
        if (!kind.length || !item.value.length) {
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
        if (kind === "account" || kind === "transaction" || kind === "block") {
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
        return shortValue(entry.value)
    }

    function favoriteKey(entry) {
        const item = normalizedEntry(entry)
        if (!item) {
            return ""
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

    function blockEntry(value) {
        const detail = value && typeof value === "object" ? value : null
        if (!detail) {
            return null
        }
        const l2 = detail.type === "indexer_block" || detail.type === "sequencer_block"
        const hash = String(detail.hash || "")
        const id = l2 ? String(detail.block_id !== undefined && detail.block_id !== null ? detail.block_id : detail.slot || "") : String(detail.slot || "")
        const favoriteValue = hash.length ? hash : id
        if (!favoriteValue.length) {
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
            created_at: new Date().toISOString()
        }
    }

    function transactionEntry(value) {
        const detail = value && typeof value === "object" ? value : null
        if (!detail) {
            return null
        }
        const hash = String(detail.hash || "")
        if (!hash.length) {
            return null
        }
        const l1 = String(detail.mode || "") === "blockchain"
        return {
            kind: "transaction",
            layer: l1 ? "l1" : "l2",
            value: hash,
            open_kind: l1 ? "mantleTransaction" : "transaction",
            title: l1
                ? qsTr("Mantle transaction %1").arg(shortValue(hash))
                : qsTr("LEZ transaction %1").arg(shortValue(hash)),
            subtitle: l1 ? qsTr("Slot %1").arg(UiFormat.valueText(detail.slot)) : String(detail.kind || qsTr("LEZ transaction")),
            created_at: new Date().toISOString()
        }
    }

    function accountEntry(value) {
        const detail = value && typeof value === "object" ? value : null
        if (!detail) {
            return null
        }
        const accountId = String(detail.account_id_base58 || detail.account_id || detail.account_id_hex || "").trim()
        if (!accountId.length) {
            return null
        }
        return {
            kind: "account",
            layer: "l2",
            value: accountId,
            open_kind: "account",
            title: detail.private_reference ? qsTr("Private account %1").arg(shortValue(accountId)) : qsTr("Account %1").arg(shortValue(accountId)),
            subtitle: String(detail.owner_base58 || detail.owner_hex || detail.account_id_hex || ""),
            created_at: new Date().toISOString()
        }
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
        openRequested(item.open_kind, item.value)
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
}
