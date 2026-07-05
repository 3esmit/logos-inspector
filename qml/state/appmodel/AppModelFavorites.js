.import "../../utils/UiFormat.js" as UiFormat

function normalizedFavoriteEntries(root, value) {
    const rows = Array.isArray(value) ? value : []
    const result = []
    const seen = ({})
    for (let i = 0; i < rows.length; ++i) {
        const entry = normalizedFavoriteEntry(root, rows[i])
        if (!entry) {
            continue
        }
        const key = favoriteKey(root, entry)
        if (seen[key] === true) {
            continue
        }
        seen[key] = true
        result.push(entry)
    }
    return result
}

function normalizedFavoriteEntry(root, value) {
    if (!value || typeof value !== "object" || Array.isArray(value)) {
        return null
    }
    const kind = normalizedFavoriteKind(root, value.kind)
    const item = {
        kind: kind,
        layer: normalizedFavoriteLayer(root, value.layer),
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
        item.open_kind = defaultFavoriteOpenKind(root, item)
    }
    if (!item.title.length) {
        item.title = defaultFavoriteTitle(root, item)
    }
    if (!item.created_at.length) {
        item.created_at = new Date().toISOString()
    }
    return item
}

function normalizedFavoriteKind(root, value) {
    const kind = String(value || "").toLowerCase()
    if (kind === "account" || kind === "transaction" || kind === "block") {
        return kind
    }
    return ""
}

function normalizedFavoriteLayer(root, value) {
    const layer = String(value || "").toLowerCase()
    if (layer === "l1" || layer === "l2") {
        return layer
    }
    return ""
}

function defaultFavoriteOpenKind(root, entry) {
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

function defaultFavoriteTitle(root, entry) {
    if (entry.kind === "account") {
        return qsTr("Account %1").arg(shortFavoriteValue(root, entry.value))
    }
    if (entry.kind === "transaction") {
        return entry.layer === "l1"
            ? qsTr("Mantle transaction %1").arg(shortFavoriteValue(root, entry.value))
            : qsTr("LEZ transaction %1").arg(shortFavoriteValue(root, entry.value))
    }
    if (entry.kind === "block") {
        return entry.layer === "l2"
            ? qsTr("LEZ block %1").arg(shortFavoriteValue(root, entry.value))
            : qsTr("Block %1").arg(shortFavoriteValue(root, entry.value))
    }
    return shortFavoriteValue(root, entry.value)
}

function favoriteKey(root, entry) {
    const item = normalizedFavoriteEntry(root, entry)
    if (!item) {
        return ""
    }
    return [item.kind, item.layer, item.open_kind, normalizedFavoriteValue(root, item.value)].join(":")
}

function normalizedFavoriteValue(root, value) {
    const text = String(value || "").trim()
    if (text.indexOf("0x") === 0 && text.length === 66) {
        return text.slice(2).toLowerCase()
    }
    return text.toLowerCase()
}

function favoriteBlockEntry(root, value) {
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
            ? qsTr("LEZ block %1").arg(id.length ? id : shortFavoriteValue(root, hash))
            : qsTr("Block at slot %1").arg(id.length ? id : shortFavoriteValue(root, hash)),
        subtitle: hash.length ? hash : (l2 ? qsTr("LEZ block id") : qsTr("L1 slot")),
        created_at: new Date().toISOString()
    }
}

function favoriteTransactionEntry(root, value) {
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
            ? qsTr("Mantle transaction %1").arg(shortFavoriteValue(root, hash))
            : qsTr("LEZ transaction %1").arg(shortFavoriteValue(root, hash)),
        subtitle: l1 ? qsTr("Slot %1").arg(UiFormat.valueText(detail.slot)) : String(detail.kind || qsTr("LEZ transaction")),
        created_at: new Date().toISOString()
    }
}

function favoriteAccountEntry(root, value) {
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
        title: detail.private_reference ? qsTr("Private account %1").arg(shortFavoriteValue(root, accountId)) : qsTr("Account %1").arg(shortFavoriteValue(root, accountId)),
        subtitle: String(detail.owner_base58 || detail.owner_hex || detail.account_id_hex || ""),
        created_at: new Date().toISOString()
    }
}

function favoriteRows(root, filter) {
    const revision = root.favoritesRevision
    const target = normalizedFavoriteKind(root, filter)
    const rows = normalizedFavoriteEntries(root, root.favorites)
    const filtered = target.length ? rows.filter(function (entry) { return entry.kind === target }) : rows
    filtered.sort(function (left, right) {
        return String(right.created_at || "").localeCompare(String(left.created_at || ""))
    })
    return filtered
}

function favoriteCount(root, filter) {
    return favoriteRows(root, filter).length
}

function isFavoriteEntry(root, entry) {
    const key = favoriteKey(root, entry)
    if (!key.length) {
        return false
    }
    const rows = Array.isArray(root.favorites) ? root.favorites : []
    for (let i = 0; i < rows.length; ++i) {
        if (favoriteKey(root, rows[i]) === key) {
            return true
        }
    }
    return false
}

function addFavorite(root, entry) {
    const item = normalizedFavoriteEntry(root, entry)
    if (!item) {
        return false
    }
    const key = favoriteKey(root, item)
    const rows = Array.isArray(root.favorites) ? root.favorites.slice(0) : []
    for (let i = 0; i < rows.length; ++i) {
        if (favoriteKey(root, rows[i]) === key) {
            rows[i] = item
            root.favorites = rows
            root.favoritesRevision += 1
            return true
        }
    }
    rows.unshift(item)
    root.favorites = rows
    root.favoritesRevision += 1
    return true
}

function removeFavorite(root, entryOrKey) {
    const key = typeof entryOrKey === "string" ? String(entryOrKey || "") : favoriteKey(root, entryOrKey)
    if (!key.length) {
        return false
    }
    const rows = Array.isArray(root.favorites) ? root.favorites.slice(0) : []
    const next = rows.filter(function (entry) {
        return favoriteKey(root, entry) !== key
    })
    if (next.length === rows.length) {
        return false
    }
    root.favorites = next
    root.favoritesRevision += 1
    return true
}

function toggleFavorite(root, entry) {
    if (isFavoriteEntry(root, entry)) {
        return removeFavorite(root, entry)
    }
    return addFavorite(root, entry)
}

function openFavorite(root, entry) {
    const item = normalizedFavoriteEntry(root, entry)
    if (!item) {
        return
    }
    root.openReference(item.open_kind, item.value)
}

function favoriteKindLabel(root, kind) {
    const value = normalizedFavoriteKind(root, kind)
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

function favoriteLayerLabel(root, layer) {
    const value = normalizedFavoriteLayer(root, layer)
    if (value === "l1") {
        return qsTr("L1")
    }
    if (value === "l2") {
        return qsTr("L2")
    }
    return qsTr("-")
}

function shortFavoriteValue(root, value) {
    return UiFormat.shortHash(String(value || ""))
}
