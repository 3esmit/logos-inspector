function socialCommentTopic(root, layer, entity, id) {
    const layerText = normalizedSocialLayer(layer)
    const entityText = normalizedSocialEntity(entity)
    const idText = normalizedSocialTopicId(id)
    if (!layerText.length || !entityText.length || !idText.length) {
        return ""
    }
    return "/" + layerText + "/" + entityText + "/" + idText + "/comments"
}

function socialLezAccountIdlTopic(root, accountId) {
    const idText = normalizedSocialTopicId(accountId)
    return idText.length ? "/lez/account/" + idText + "/idl" : ""
}

function normalizedSocialTopicId(value) {
    const text = String(value || "").trim().replace(/^\/+/, "").replace(/\/+$/, "")
    return text.length > 0 && text.indexOf("/") < 0 ? text : ""
}

function normalizedSocialLayer(value) {
    const text = String(value || "").trim().toLowerCase()
    if (text === "cryptarchia" || text === "bedrock" || text === "l1") {
        return "cryptarchia"
    }
    if (text === "lez" || text === "l2") {
        return "lez"
    }
    return ""
}

function normalizedSocialEntity(value) {
    const text = String(value || "").trim().toLowerCase()
    if (text === "transaction" || text === "tx") {
        return "transaction"
    }
    if (text === "block") {
        return "block"
    }
    if (text === "account") {
        return "account"
    }
    return ""
}

function socialComments(root, topic) {
    const state = socialCommentState(root, topic)
    return Array.isArray(state.rows) ? state.rows : []
}

function socialCommentState(root, topic) {
    const key = String(topic || "")
    const state = root.socialCommentState || {}
    return state[key] || {
        rows: [],
        cursor: "",
        loading: false,
        error: "",
        exhausted: false
    }
}

function loadSocialComments(root, topic, reset, pageSize, expectedAccountId) {
    with (root) {
        const key = String(topic || "").trim()
        if (!key.length) {
            return false
        }
        if (!root.socialStoreAvailable()) {
            setSocialCommentState(key, {
                rows: reset ? [] : root.socialComments(key),
                cursor: "",
                loading: false,
                error: qsTr("Delivery Store requires Direct Waku REST source."),
                exhausted: true
            })
            return false
        }

        const current = root.socialCommentStateForTopic(key)
        const cursor = reset === true ? "" : String(current.cursor || "")
        setSocialCommentState(key, {
            rows: reset === true ? [] : root.socialComments(key),
            cursor: cursor,
            loading: true,
            error: "",
            exhausted: false
        })

        const response = root.requestModule(
            inspectorModule,
            "deliveryStoreQuery",
            root.socialDeliveryArgs(["", key, "", cursor, root.socialPageSize(pageSize), true, true]),
            qsTr("Comments"),
            false,
            false
        )
        if (!response.ok) {
            setSocialCommentState(key, {
                rows: reset === true ? [] : root.socialComments(key),
                cursor: cursor,
                loading: false,
                error: response.error || qsTr("Comment query failed."),
                exhausted: false
            })
            return false
        }

        const page = root.requestModule(
            inspectorModule,
            "socialCommentPageFromStore",
            [key, response.value, String(expectedAccountId || "")],
            qsTr("Comments"),
            false,
            false
        )
        if (!page.ok) {
            setSocialCommentState(key, {
                rows: reset === true ? [] : root.socialComments(key),
                cursor: cursor,
                loading: false,
                error: page.error || qsTr("Comment decode failed."),
                exhausted: false
            })
            return false
        }

        const pageValue = page.value && typeof page.value === "object" ? page.value : ({})
        const incoming = Array.isArray(pageValue.rows) ? pageValue.rows : []
        const existing = reset === true ? [] : root.socialComments(key)
        const merged = root.mergeSocialCommentRows(existing, incoming)
        const nextCursor = String(pageValue.cursor || "")
        setSocialCommentState(key, {
            rows: merged,
            cursor: nextCursor,
            loading: false,
            error: "",
            exhausted: incoming.length === 0 || !nextCursor.length || nextCursor === cursor
        })
        return true
    }
}

function setSocialCommentState(root, topic, state) {
    const key = String(topic || "")
    if (!key.length) {
        return
    }
    const next = root.copyMap(root.socialCommentState || {})
    next[key] = state || {}
    root.socialCommentState = next
    root.socialCommentRevision += 1
}

function applyIncomingComment(root, event) {
    const incoming = event || {}
    const payload = incoming.payload || {}
    if (String(payload.kind || "") !== "comment") {
        return false
    }
    const topic = String(incoming.topic || payload.conversation_id || "")
    if (!topic.length) {
        return false
    }
    const current = root.socialCommentStateForTopic(topic)
    const row = socialCommentRowFromIncomingEvent(root, incoming) || {
        key: "event|" + String(incoming.messageHash || "") + "|" + String(payload.created_at || ""),
        cursor: "",
        topic: topic,
        identity: payload.identity || {},
        displayName: socialIdentityDisplayName(payload.identity),
        body: String(payload.body || ""),
        createdAt: String(payload.created_at || ""),
        conversationId: String(payload.conversation_id || topic)
    }
    root.setSocialCommentState(topic, {
        rows: root.mergeSocialCommentRows(current.rows || [], [row]),
        cursor: String(current.cursor || ""),
        loading: false,
        error: "",
        exhausted: current.exhausted === true
    })
    return true
}

function socialCommentRowFromIncomingEvent(root, incoming) {
    const bridge = root.bridge || null
    if (!bridge || typeof bridge.callModule !== "function") {
        return null
    }
    const response = bridge.callModule(root.inspectorModule, "socialCommentRowFromEvent", [incoming || {}])
    if (response.ok === true && response.value && typeof response.value === "object" && !Array.isArray(response.value)
            && String(response.value.body || "").length > 0) {
        return response.value
    }
    return null
}

function socialCommentRowsFromMessages(root, messages) {
    const rows = []
    const values = Array.isArray(messages) ? messages : []
    for (let i = 0; i < values.length; ++i) {
        const message = values[i] || {}
        const payload = message.payload || {}
        if (String(payload.kind || "") !== "comment") {
            continue
        }
        rows.push({
            key: socialMessageRowKey(message),
            cursor: String(message.cursor || ""),
            topic: String(message.topic || payload.conversation_id || ""),
            identity: payload.identity || {},
            displayName: socialIdentityDisplayName(payload.identity),
            body: String(payload.body || ""),
            createdAt: String(payload.created_at || ""),
            conversationId: String(payload.conversation_id || "")
        })
    }
    return rows
}

function mergeSocialCommentRows(root, existingRows, incomingRows) {
    const rows = Array.isArray(existingRows) ? existingRows.slice(0) : []
    const seen = {}
    for (let i = 0; i < rows.length; ++i) {
        seen[String(rows[i].key || socialCommentDedupeKey(rows[i]))] = true
    }
    const incoming = Array.isArray(incomingRows) ? incomingRows : []
    for (let j = 0; j < incoming.length; ++j) {
        const row = incoming[j] || {}
        const key = String(row.key || socialCommentDedupeKey(row))
        if (seen[key] === true) {
            continue
        }
        const copy = Object.assign({}, row)
        copy.key = key
        rows.push(copy)
        seen[key] = true
    }
    return rows
}

function socialCommentDedupeKey(row) {
    return [
        String(row.cursor || ""),
        String(row.createdAt || ""),
        String(row.displayName || ""),
        String(row.body || "")
    ].join("|")
}

function socialMessageRowKey(message) {
    const payload = message && message.payload ? message.payload : {}
    return [
        String(message && message.cursor ? message.cursor : ""),
        String(payload.created_at || ""),
        socialIdentityDisplayName(payload.identity),
        String(payload.body || payload.idl_name || "")
    ].join("|")
}

function socialStoreCursor(root, value) {
    return firstStoreCursor(value, 0)
}

function firstStoreCursor(value, depth) {
    if (depth > 5 || value === undefined || value === null) {
        return ""
    }
    if (Array.isArray(value)) {
        for (let i = 0; i < value.length; ++i) {
            const cursor = firstStoreCursor(value[i], depth + 1)
            if (cursor.length) {
                return cursor
            }
        }
        return ""
    }
    if (typeof value !== "object") {
        return ""
    }
    const keys = ["paginationCursor", "pagination_cursor", "nextCursor", "next_cursor"]
    for (let keyIndex = 0; keyIndex < keys.length; ++keyIndex) {
        const cursor = String(value[keys[keyIndex]] || "").trim()
        if (cursor.length) {
            return cursor
        }
    }
    const childKeys = ["value", "result", "page", "pagination"]
    for (let childIndex = 0; childIndex < childKeys.length; ++childIndex) {
        const childCursor = firstStoreCursor(value[childKeys[childIndex]], depth + 1)
        if (childCursor.length) {
            return childCursor
        }
    }
    return ""
}

function lastSocialMessageCursor(root, messages) {
    const rows = Array.isArray(messages) ? messages : []
    for (let i = rows.length - 1; i >= 0; --i) {
        const cursor = String(rows[i] && rows[i].cursor ? rows[i].cursor : "")
        if (cursor.length) {
            return cursor
        }
    }
    return ""
}

function postSocialComment(root, topic, body, identityKey) {
    with (root) {
        const key = String(topic || "").trim()
        const text = String(body || "").trim()
        if (!key.length || !text.length || !root.socialCommentSendAvailable(key)) {
            return false
        }
        const identity = root.socialIdentityForConversation(key, identityKey)
        if (!identity || !String(identity.key || "").length) {
            return false
        }
        const createdAt = new Date().toISOString()
        const payload = {
            kind: "comment",
            version: 1,
            identity: root.socialIdentityPayload(identity),
            body: text,
            created_at: createdAt,
            conversation_id: key
        }
        const response = root.callInspector(
            "deliverySend",
            root.socialDeliveryArgs([key, JSON.stringify(payload)]),
            qsTr("Post comment")
        )
        if (!response.ok) {
            return false
        }
        const current = root.socialCommentStateForTopic(key)
        const row = {
            key: "local|" + createdAt + "|" + String(identity.key || ""),
            cursor: "",
            topic: key,
            identity: payload.identity,
            displayName: socialIdentityDisplayName(payload.identity),
            body: text,
            createdAt: createdAt,
            conversationId: key
        }
        setSocialCommentState(root, key, {
            rows: root.mergeSocialCommentRows(current.rows || [], [row]),
            cursor: String(current.cursor || ""),
            loading: false,
            error: "",
            exhausted: current.exhausted === true
        })
        return true
    }
}

function socialDeliveryArgs(root, extra) {
    return [
        root.effectiveMessagingSourceMode(root.messagingSourceMode),
        root.configuredMessagingRestUrl(),
        root.messagingMutatingDiagnosticsEnabled === true
    ].concat(extra || [])
}

function socialMessageSourceAvailable(root) {
    const mode = String(root.effectiveMessagingSourceMode(root.messagingSourceMode) || "").toLowerCase()
    return mode === "rest" || mode === "module"
}

function socialStoreAvailable(root) {
    return String(root.effectiveMessagingSourceMode(root.messagingSourceMode) || "").toLowerCase() === "rest"
}

function socialCommentSendAvailable(root, topic) {
    return !root.busy
        && root.socialMessageSourceAvailable()
        && root.messagingMutatingDiagnosticsEnabled === true
        && root.validSocialTopic(topic)
}

function validSocialTopic(root, topic) {
    return /^\/[^/]+\/[^/]+\/[^/]+\/[^/]+$/.test(String(topic || "").trim())
}

function socialPageSize(root, pageSize) {
    const value = Number(pageSize || root.socialCommentPageSize || 20)
    return Number.isFinite(value) ? Math.max(1, Math.min(100, Math.floor(value))) : 20
}

function loadSocialSettings(root, value) {
    const settings = value || {}
    root.socialIdentities.clear()
    const identities = Array.isArray(settings.social_identities) ? settings.social_identities : []
    for (let i = 0; i < identities.length; ++i) {
        const entry = normalizedSocialIdentityEntry(root, identities[i], i)
        if (entry.key.length) {
            root.socialIdentities.append(entry)
        }
    }
    root.socialIdentityDefaultMode = normalizedSocialIdentityDefaultMode(settings.social_identity_default_mode || root.socialIdentityDefaultMode)
    root.selectedSocialIdentityKey = String(settings.social_selected_identity_key || root.selectedSocialIdentityKey || "")
    root.socialConversationIdentityKeys = settings.social_conversation_identity_keys && typeof settings.social_conversation_identity_keys === "object"
        ? settings.social_conversation_identity_keys
        : ({})
    root.sharedIdlPolicy = normalizedSharedIdlPolicy(settings.shared_idl_policy || root.sharedIdlPolicy)
    root.sharedIdlAutoShare = settings.shared_idl_auto_share === true
    root.socialAutoSharedIdls = settings.social_auto_shared_idls && typeof settings.social_auto_shared_idls === "object"
        ? settings.social_auto_shared_idls
        : ({})
    root.socialIdentityRevision += 1
    root.sharedIdlRevision += 1
}

function socialSettingsPayload(root) {
    return {
        social_identities: socialIdentityRows(root),
        social_identity_default_mode: normalizedSocialIdentityDefaultMode(root.socialIdentityDefaultMode),
        social_selected_identity_key: String(root.selectedSocialIdentityKey || ""),
        social_conversation_identity_keys: root.socialConversationIdentityKeys || {},
        shared_idl_policy: normalizedSharedIdlPolicy(root.sharedIdlPolicy),
        shared_idl_auto_share: root.sharedIdlAutoShare === true,
        social_auto_shared_idls: root.socialAutoSharedIdls || {}
    }
}

function normalizedSocialIdentityEntry(root, entry, fallbackIndex) {
    const row = entry || {}
    const key = String(row.key || row.local_id || row.localId || "")
    const fallbackName = qsTr("Pseudonym %1").arg(Number(fallbackIndex || 0) + 1)
    return {
        key: key,
        displayName: String(row.displayName || row.display_name || row.name || fallbackName),
        localId: String(row.localId || row.local_id || key),
        keyMaterial: String(row.keyMaterial || row.key_material || ""),
        createdAt: String(row.createdAt || row.created_at || "")
    }
}

function socialIdentityRows(root) {
    const rows = []
    for (let i = 0; i < root.socialIdentities.count; ++i) {
        rows.push(normalizedSocialIdentityEntry(root, root.socialIdentities.get(i), i))
    }
    return rows
}

function createSocialIdentity(root, displayName) {
    const createdAt = new Date().toISOString()
    const index = root.socialIdentities.count + 1
    const localId = "local-" + socialRandomHex(16)
    const entry = {
        key: localId,
        displayName: String(displayName || "").trim() || qsTr("Pseudonym %1").arg(index),
        localId: localId,
        keyMaterial: socialRandomHex(64),
        createdAt: createdAt
    }
    root.socialIdentities.append(entry)
    root.selectedSocialIdentityKey = entry.key
    root.socialIdentityRevision += 1
    root.saveSettingsState()
    return entry
}

function socialRandomHex(length) {
    const alphabet = "0123456789abcdef"
    let value = ""
    for (let i = 0; i < length; ++i) {
        value += alphabet.charAt(Math.floor(Math.random() * alphabet.length))
    }
    return value
}

function socialIdentityForKey(root, key) {
    const wanted = String(key || "")
    if (!wanted.length) {
        return null
    }
    for (let i = 0; i < root.socialIdentities.count; ++i) {
        const entry = normalizedSocialIdentityEntry(root, root.socialIdentities.get(i), i)
        if (entry.key === wanted) {
            return entry
        }
    }
    return null
}

function socialIdentityForConversation(root, topic, key) {
    const explicitIdentity = socialIdentityForKey(root, key)
    if (explicitIdentity) {
        return explicitIdentity
    }
    const mode = normalizedSocialIdentityDefaultMode(root.socialIdentityDefaultMode)
    if (mode === "manual") {
        return socialIdentityForKey(root, root.selectedSocialIdentityKey)
            || firstSocialIdentity(root)
            || createSocialIdentity(root, "")
    }

    const conversation = String(topic || "")
    const current = root.socialConversationIdentityKeys || {}
    const currentKey = String(current[conversation] || "")
    const currentIdentity = socialIdentityForKey(root, currentKey)
    if (currentIdentity) {
        return currentIdentity
    }
    const entry = createSocialIdentity(root, "")
    const next = root.copyMap(current)
    next[conversation] = entry.key
    root.socialConversationIdentityKeys = next
    root.saveSettingsState()
    return entry
}

function firstSocialIdentity(root) {
    return root.socialIdentities.count > 0 ? normalizedSocialIdentityEntry(root, root.socialIdentities.get(0), 0) : null
}

function selectSocialIdentity(root, key) {
    const entry = socialIdentityForKey(root, key)
    if (!entry) {
        return false
    }
    root.selectedSocialIdentityKey = entry.key
    root.saveSettingsState()
    return true
}

function setSocialIdentityDefaultMode(root, mode) {
    root.socialIdentityDefaultMode = normalizedSocialIdentityDefaultMode(mode)
    root.saveSettingsState()
}

function normalizedSocialIdentityDefaultMode(value) {
    const text = String(value || "").trim().toLowerCase()
    return text === "manual" ? "manual" : "perConversation"
}

function socialIdentityPayload(root, identity) {
    const entry = identity || {}
    return {
        display_name: String(entry.displayName || entry.display_name || ""),
        local_id: String(entry.localId || entry.local_id || entry.key || "")
    }
}

function socialIdentityDisplayName(identity) {
    const value = identity || {}
    return String(value.display_name || value.displayName || value.name || value.local_id || value.localId || qsTr("Pseudonym"))
}

function setSharedIdlPolicy(root, policy) {
    root.sharedIdlPolicy = normalizedSharedIdlPolicy(policy)
    root.saveSettingsState()
}

function normalizedSharedIdlPolicy(value) {
    const text = String(value || "").trim().toLowerCase()
    if (text === "autoregister" || text === "auto-register" || text === "auto register") {
        return "autoRegister"
    }
    if (text === "sessiononly" || text === "session-only" || text === "session only") {
        return "sessionOnly"
    }
    if (text === "disabled" || text === "off") {
        return "disabled"
    }
    return "suggestion"
}

function setSharedIdlAutoShare(root, enabled) {
    root.sharedIdlAutoShare = enabled === true
    root.saveSettingsState()
}

function refreshSharedIdlsForAccount(root, accountId, dataHex, ownerProgramId) {
    with (root) {
        const policy = normalizedSharedIdlPolicy(sharedIdlPolicy)
        const account = String(accountId || "").trim()
        const data = String(dataHex || "").trim()
        const topic = root.socialLezAccountIdlTopic(account)
        if (policy === "disabled" || !topic.length || !data.length || !root.socialStoreAvailable()) {
            return false
        }
        const response = root.requestModule(
            inspectorModule,
            "deliveryStoreQuery",
            root.socialDeliveryArgs(["", topic, "", "", root.socialPageSize(20), true, true]),
            qsTr("Shared IDLs"),
            false,
            false
        )
        if (!response.ok) {
            return false
        }
        const acceptedResponse = root.requestModule(
            inspectorModule,
            "acceptedSharedIdlEntriesFromStore",
            [topic, response.value, account, data, String(ownerProgramId || "")],
            qsTr("Shared IDLs"),
            false,
            false
        )
        if (!acceptedResponse.ok || !Array.isArray(acceptedResponse.value)) {
            return false
        }
        let accepted = 0
        for (let i = 0; i < acceptedResponse.value.length; ++i) {
            const entry = acceptedResponse.value[i] || null
            if (entry && root.applySharedIdlPolicy(account, entry)) {
                accepted += 1
            }
        }
        return accepted > 0
    }
}

function applySharedIdlPolicy(root, accountId, entry) {
    const policy = normalizedSharedIdlPolicy(root.sharedIdlPolicy)
    if (policy === "disabled") {
        return false
    }
    const acceptedEntry = acceptedSharedIdlEntryForAccount(root, accountId, entry)
    if (!acceptedEntry) {
        return false
    }
    if (policy === "autoRegister") {
        if (!idlEntryExists(root, acceptedEntry.key)) {
            root.registeredIdls.append(acceptedEntry)
            root.saveIdlState()
        }
        root.sharedIdlRevision += 1
        return true
    }
    storeSharedIdl(root, accountId, acceptedEntry)
    return true
}

function acceptedSharedIdlEntryForAccount(root, accountId, entry) {
    const account = String(accountId || "").trim()
    if (!account.length || !entry) {
        return null
    }
    const normalized = root.normalizedIdlEntry(entry, 0)
    if (!normalized || String(normalized.source || "") !== "shared") {
        return null
    }
    if (String(normalized.sharedAccountId || "") !== account) {
        return null
    }
    if (!String(normalized.key || "").length || !String(normalized.json || "").length
            || !String(normalized.programIdHex || "").length || !String(normalized.accountType || "").length) {
        return null
    }
    return normalized
}

function idlEntryExists(root, key) {
    return root.idlEntryForKey(key) !== null
}

function storeSharedIdl(root, accountId, entry) {
    const cacheKey = String(accountId || "")
    if (!cacheKey.length || !entry || !String(entry.key || "").length) {
        return
    }
    const next = root.copyMap(root.socialSharedIdls || {})
    const rows = Array.isArray(next[cacheKey]) ? next[cacheKey].slice(0) : []
    for (let i = 0; i < rows.length; ++i) {
        if (String(rows[i].key || "") === String(entry.key || "")) {
            return
        }
    }
    rows.push(entry)
    next[cacheKey] = rows
    root.socialSharedIdls = next
    root.sharedIdlRevision += 1
}

function sharedIdlSuggestions(root, accountId) {
    const revision = root.sharedIdlRevision
    const rows = (root.socialSharedIdls || {})[String(accountId || "")]
    return Array.isArray(rows) ? rows : []
}

function sharedIdlEntriesForAccount(root, accountId, ownerProgramId) {
    const policy = normalizedSharedIdlPolicy(root.sharedIdlPolicy)
    if (policy !== "sessionOnly") {
        return []
    }
    const owner = root.accountOwnerCacheKey(ownerProgramId)
    const rows = sharedIdlSuggestions(root, accountId)
    const result = []
    for (let i = 0; i < rows.length; ++i) {
        const entry = rows[i] || {}
        const program = String(entry.programIdHex || "") || root.canonicalProgramIdHex(entry.programId) || root.normalizedHexText(entry.programId)
        if (!owner.length || program === owner) {
            result.push(entry)
        }
    }
    return result
}

function publishAccountIdl(root, accountId, ownerProgramId, idlEntry) {
    with (root) {
        const account = String(accountId || "").trim()
        const topic = root.socialLezAccountIdlTopic(account)
        const entry = idlEntry || {}
        const idlJson = String(entry.json || "")
        if (!topic.length || !idlJson.length || !root.socialCommentSendAvailable(topic)) {
            return false
        }
        const identity = root.socialIdentityForConversation(topic, "")
        const programId = String(ownerProgramId || entry.programIdHex || entry.programId || "")
        const payload = {
            kind: "lez_account_idl",
            version: 1,
            identity: root.socialIdentityPayload(identity),
            account_id: account,
            program_id: programId,
            idl_name: String(entry.name || root.idlNameFromJson(idlJson) || qsTr("IDL")),
            idl_json: idlJson,
            created_at: new Date().toISOString()
        }
        const response = root.callInspector(
            "deliverySend",
            root.socialDeliveryArgs([topic, JSON.stringify(payload)]),
            qsTr("Share IDL")
        )
        return response.ok === true
    }
}

function maybeAutoShareAccountIdl(root, accountId, ownerProgramId, idlEntry) {
    if (root.sharedIdlAutoShare !== true || !idlEntry || String(idlEntry.source || "") === "shared") {
        return false
    }
    const topic = socialLezAccountIdlTopic(root, accountId)
    const key = [String(accountId || ""), topic, String(idlEntry.key || "")].join("|")
    if (!topic.length || (root.socialAutoSharedIdls || {})[key] === true) {
        return false
    }
    if (!publishAccountIdl(root, accountId, ownerProgramId, idlEntry)) {
        return false
    }
    const next = root.copyMap(root.socialAutoSharedIdls || {})
    next[key] = true
    root.socialAutoSharedIdls = next
    root.saveSettingsState()
    return true
}
