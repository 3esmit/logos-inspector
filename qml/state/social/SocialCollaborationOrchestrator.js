.import "SharedIdlTransport.js" as SharedIdlTransport

function socialCommentTopic(root, layer, entity, id) {
    return socialRuntimeString(root, "socialCommentTopic", [String(layer || ""), String(entity || ""), String(id || "")])
}

function socialZoneCommentTopic(root, entityRef) {
    return socialRuntimeString(root, "socialZoneCommentTopic", [entityRef || null])
}

function socialZoneAccountIdlTopic(root, entityRef) {
    return socialRuntimeString(root, "socialZoneAccountIdlTopic", [entityRef || null])
}

function zoneSocialScope(entityRef) {
    if (!entityRef || typeof entityRef !== "object"
            || String(entityRef.entity_kind || "") === "program") {
        return null
    }
    const scope = entityRef.network_scope
    if (!scope || String(scope.kind || "") !== "genesis_id") {
        return null
    }
    return {
        network_scope: scope,
        zone_id: String(entityRef.channel_id || ""),
        entity_kind: String(entityRef.entity_kind || ""),
        canonical_entity_key: String(entityRef.canonical_key || "")
    }
}

function socialRuntimeString(root, method, args) {
    const bridge = root.bridge || null
    if (!bridge || typeof bridge.callModule !== "function") {
        return ""
    }
    const response = bridge.callModule(root.inspectorModule, method, args || [])
    return response && response.ok === true && typeof response.value === "string" ? response.value : ""
}

function copyMap(value) {
    const result = ({})
    const source = value && typeof value === "object" ? value : ({})
    const keys = Object.keys(source)
    for (let i = 0; i < keys.length; ++i) {
        result[keys[i]] = source[keys[i]]
    }
    return result
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
        exhausted: false,
        sending: false,
        sendError: ""
    }
}

function commentView(root, topic) {
    const revision = root.socialCommentRevision
    const state = socialCommentState(root, topic)
    const readGate = socialCommentReadGate(root, topic)
    const writeGate = socialCommentWriteGate(root, topic)
    return {
        revision: revision,
        state: state,
        rows: Array.isArray(state.rows) ? state.rows : [],
        readGate: readGate,
        writeGate: writeGate,
        writeAvailable: socialCommentSendAvailable(root, topic),
        readError: socialGateDetailText(root, readGate, qsTr("Comments are unavailable.")),
        writeError: socialCommentWriteError(root, state, writeGate)
    }
}

function loadSocialComments(root, topic, reset, pageSize, expectedAccountId) {
    const key = String(topic || "").trim()
    if (!key.length || !socialCommentReadAvailable(root, key)) {
        return false
    }
    const callerKey = "comments|" + key
    if (root.storeQueryCallerPending(callerKey)) {
        return false
    }
    const current = socialCommentState(root, key)
    const cursor = reset === true ? "" : String(current.cursor || "")
    setSocialCommentState(root, key, {
        rows: reset === true ? [] : socialComments(root, key),
        cursor: cursor,
        loading: true,
        error: "",
        exhausted: false
    })

    const query = querySocialStore(root, {
        callerKey: callerKey,
        family: "comments",
        accountId: String(expectedAccountId || ""),
        topic: key
    }, cursor, pageSize, qsTr("Comments"), function (response, ticket) {
        return finishSocialCommentQuery(
            root,
            key,
            cursor,
            String(expectedAccountId || ""),
            response,
            ticket
        )
    })
    if (!query.ok) {
        setSocialCommentState(root, key, {
            rows: socialComments(root, key),
            cursor: cursor,
            loading: false,
            error: query.error || qsTr("Comment query failed."),
            exhausted: query.storeUnavailable === true
        })
        return false
    }
    return true
}

function finishSocialCommentQuery(root, topic, cursor, expectedAccountId, response, ticket) {
    if (!ticket || !root.isCurrentStoreQuery(ticket)) {
        return false
    }
    if (!response || response.ok !== true) {
        setSocialCommentState(root, topic, {
            rows: socialComments(root, topic),
            cursor: cursor,
            loading: false,
            error: String(response && response.error || qsTr("Comment query failed.")),
            exhausted: false
        })
        return false
    }
    if (!root.gateway || typeof root.gateway.requestModuleAsync !== "function") {
        setSocialCommentState(root, topic, {
            rows: socialComments(root, topic),
            cursor: cursor,
            loading: false,
            error: qsTr("Comment decoder is unavailable."),
            exhausted: false
        })
        return false
    }
    const admitted = root.gateway.requestModuleAsync(
        root.inspectorModule,
        "socialCommentPageFromStore",
        [topic, response.value, expectedAccountId],
        qsTr("Comments"),
        false,
        function (page) {
            if (!root.isCurrentStoreQuery(ticket)) {
                return
            }
            applySocialCommentPage(root, topic, cursor, page)
            root.releaseStoreQuery(ticket)
        }
    )
    if (admitted === false || admitted === null) {
        if (root.isCurrentStoreQuery(ticket)) {
            setSocialCommentState(root, topic, {
                rows: socialComments(root, topic),
                cursor: cursor,
                loading: false,
                error: qsTr("Comment query could not be decoded."),
                exhausted: false
            })
            root.releaseStoreQuery(ticket)
        }
        return false
    }
    return true
}

function applySocialCommentPage(root, topic, cursor, page) {
    if (!page || page.ok !== true) {
        setSocialCommentState(root, topic, {
            rows: socialComments(root, topic),
            cursor: cursor,
            loading: false,
            error: String(page && page.error || qsTr("Comment decode failed.")),
            exhausted: false
        })
        return
    }

    const pageValue = page.value && typeof page.value === "object" ? page.value : ({})
    const incoming = Array.isArray(pageValue.rows) ? pageValue.rows : []
    const existing = socialComments(root, topic)
    const merged = mergeSocialCommentRows(existing, incoming)
    const nextCursor = String(pageValue.cursor || "")
    setSocialCommentState(root, topic, {
        rows: merged,
        cursor: nextCursor,
        loading: false,
        error: "",
        exhausted: incoming.length === 0 || !nextCursor.length || nextCursor === cursor
    })
}

function setSocialCommentState(root, topic, state) {
    const key = String(topic || "")
    if (!key.length) {
        return
    }
    const next = copyMap(root.socialCommentState || {})
    const merged = copyMap(socialCommentState(root, key))
    const update = state && typeof state === "object" ? state : ({})
    const fields = Object.keys(update)
    for (let i = 0; i < fields.length; ++i) {
        merged[fields[i]] = update[fields[i]]
    }
    next[key] = merged
    root.socialCommentState = next
    root.socialCommentRevision += 1
}

function invalidateSocialCommentRequests(root) {
    const current = root.socialCommentState || {}
    const keys = Object.keys(current)
    if (keys.length) {
        const next = ({})
        for (let i = 0; i < keys.length; ++i) {
            const state = socialCommentState(root, keys[i])
            if (state.sending === true || String(state.sendError || "").length) {
                next[keys[i]] = {
                    rows: [],
                    cursor: "",
                    loading: false,
                    error: "",
                    exhausted: false,
                    sending: state.sending === true,
                    sendError: String(state.sendError || "")
                }
            }
        }
        root.socialCommentState = next
        root.socialCommentRevision += 1
    }
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
    const current = socialCommentState(root, topic)
    const row = socialCommentRowFromIncomingEvent(root, incoming)
    if (!row) {
        return false
    }
    setSocialCommentState(root, topic, {
        rows: mergeSocialCommentRows(current.rows || [], [row]),
        cursor: String(current.cursor || ""),
        loading: false,
        error: "",
        exhausted: current.exhausted === true
    })
    return true
}

function applyIncomingDeliveryMessage(root, message) {
    const incoming = message || {}
    const payload = socialMessagePayload(incoming.payload)
    if (!payload || typeof payload !== "object" || String(payload.kind || "") !== "comment") {
        return false
    }
    return applyIncomingComment(root, {
        topic: String(incoming.topic || payload.conversation_id || ""),
        messageHash: String(incoming.messageHash || incoming.message_hash || incoming.hash || ""),
        payload: payload
    })
}

function socialMessagePayload(value) {
    if (value && typeof value === "object" && !Array.isArray(value)) {
        return value
    }
    const text = String(value || "").trim()
    if (!text.length) {
        return null
    }
    try {
        return JSON.parse(text)
    } catch (error) {
        return null
    }
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

function mergeSocialCommentRows(existingRows, incomingRows) {
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

function socialStoreCursor(value) {
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

function lastSocialMessageCursor(messages) {
    const rows = Array.isArray(messages) ? messages : []
    for (let i = rows.length - 1; i >= 0; --i) {
        const cursor = String(rows[i] && rows[i].cursor ? rows[i].cursor : "")
        if (cursor.length) {
            return cursor
        }
    }
    return ""
}

function postSocialComment(root, topic, body, identityKey, entityRef, onComplete) {
    const key = String(topic || "").trim()
    const text = String(body || "").trim()
    if (!key.length || !text.length || !socialCommentSendAvailable(root, key)) {
        return false
    }
    const zoneScope = zoneSocialScope(entityRef)
    if (key.indexOf("/lez/") === 0 && !zoneScope) {
        return false
    }
    const identity = socialIdentityForConversation(root, key, identityKey)
    if (!identity || !String(identity.key || "").length) {
        return false
    }
    const createdAt = new Date().toISOString()
    const payload = {
        kind: "comment",
        version: zoneScope ? 2 : 1,
        identity: socialIdentityPayload(identity),
        body: text,
        created_at: createdAt,
        conversation_id: key
    }
    if (zoneScope) {
        payload.scope = zoneScope
    }
    const current = socialCommentState(root, key)
    const localOrdinal = Array.isArray(current.rows) ? current.rows.length : 0
    const row = {
        key: [
            "local",
            createdAt,
            String(identity.key || ""),
            String(localOrdinal)
        ].join("|"),
        cursor: "",
        topic: key,
        identity: payload.identity,
        displayName: socialIdentityDisplayName(payload.identity),
        body: text,
        createdAt: createdAt,
        conversationId: key
    }
    setSocialCommentState(root, key, {
        sending: true,
        sendError: ""
    })
    return root.startCommentWrite({
        topic: key,
        payloadText: JSON.stringify(payload),
        label: qsTr("Post comment")
    }, function (response) {
        const latest = socialCommentState(root, key)
        const ok = response && response.ok === true
        setSocialCommentState(root, key, {
            rows: ok
                ? mergeSocialCommentRows(latest.rows || [], [row])
                : (latest.rows || []),
            sending: false,
            sendError: ok ? "" : String(response && response.error
                || qsTr("Comment was not delivered."))
        })
        if (typeof onComplete === "function") {
            onComplete(response)
        }
    })
}

function socialMessageSourceAvailable(root) {
    const mode = String(root.gateway.effectiveMessagingSourceMode(root.messagingSourceMode) || "").toLowerCase()
    return mode === "rest" || mode === "module"
}

function normalizedSocialGate(gate) {
    const state = gate && typeof gate === "object" ? gate : ({})
    return {
        enabled: state.enabled === true,
        status: String(state.status || (state.enabled === true ? "enabled" : "disabled")),
        missing: Array.isArray(state.missing) ? state.missing.slice(0) : [],
        warnings: Array.isArray(state.warnings) ? state.warnings.slice(0) : [],
        provenance: Array.isArray(state.provenance) ? state.provenance.slice(0) : []
    }
}

function socialGateWithInputMissing(gate, dependency, label) {
    const state = normalizedSocialGate(gate)
    const provenance = state.provenance.slice(0)
    provenance.push("input")
    return {
        enabled: false,
        status: state.enabled ? "input_required" : state.status,
        missing: state.missing.concat([{
            dependency: String(dependency || ""),
            label: String(label || dependency || qsTr("Input")),
            status: "input_required",
            capability: String(dependency || ""),
            provenance: "input"
        }]),
        warnings: state.warnings,
        provenance: provenance
    }
}

function socialGateWithTopic(root, gate, topic) {
    const state = normalizedSocialGate(gate)
    const key = String(topic || "").trim()
    if (!key.length) {
        return socialGateWithInputMissing(state, "social.topic", qsTr("Social topic"))
    }
    if (!state.enabled) {
        return state
    }
    if (!validSocialTopic(root, key)) {
        return socialGateWithInputMissing(state, "social.topic.valid", qsTr("Valid social topic"))
    }
    return state
}

function socialStoreGate(root) {
    return normalizedSocialGate(root.gateway.socialGate("comments.read"))
}

function socialCommentReadGate(root, topic) {
    return socialGateWithTopic(root, socialStoreGate(root), topic)
}

function socialCommentWriteGate(root, topic) {
    return socialGateWithTopic(root, root.gateway.socialGate("comments.write"), topic)
}

function socialSharedIdlReadGate(root) {
    return normalizedSocialGate(root.gateway.socialGate("shared_idl.read"))
}

function socialSharedIdlWriteGate(root, topic) {
    return socialGateWithTopic(root, root.gateway.socialGate("shared_idl.write"), topic)
}

function socialMissingDependencyText(row) {
    const dependency = String(row && row.dependency !== undefined ? row.dependency : "")
    const label = String(row && row.label !== undefined ? row.label : "")
    const status = String(row && row.status !== undefined ? row.status : "")
    const name = label.length && label !== dependency && dependency.length
        ? qsTr("%1: %2").arg(label).arg(dependency)
        : String(dependency || label || qsTr("Required Social capability"))
    if (status.length && status !== "unavailable") {
        return qsTr("%1 (%2)").arg(name).arg(status)
    }
    return name
}

function socialGateDetailText(root, gate, fallback) {
    const state = normalizedSocialGate(gate)
    if (state.enabled) {
        return String(fallback || "")
    }
    const missing = Array.isArray(state.missing) ? state.missing : []
    if (!missing.length) {
        return String(fallback || qsTr("Required Social capability is unavailable."))
    }
    const details = []
    for (let i = 0; i < missing.length; ++i) {
        details.push(socialMissingDependencyText(missing[i]))
    }
    return qsTr("Missing %1").arg(details.join(", "))
}

function socialStoreAvailable(root) {
    return socialStoreGate(root).enabled === true
}

function querySocialStore(root, scope, cursor, pageSize, label, callback) {
    const gate = socialStoreGate(root)
    if (!gate.enabled) {
        return {
            ok: false,
            error: socialGateDetailText(root, gate, qsTr("Delivery Store capability is unavailable.")),
            storeUnavailable: true
        }
    }
    const ticket = root.queryDeliveryStore(
        scope,
        String(cursor || ""),
        socialPageSize(root, pageSize),
        String(label || qsTr("Delivery Store")),
        callback
    )
    return {
        ok: ticket !== null,
        ticket: ticket,
        error: ticket !== null ? "" : qsTr("Delivery Store query could not be started."),
        storeUnavailable: false
    }
}

function socialCommentSendAvailable(root, topic) {
    return !root.busy
        && root.messagingMutatingDiagnosticsEnabled === true
        && root.writesRunning !== true
        && socialCommentWriteGate(root, topic).enabled === true
}

function socialCommentWriteError(root, state, gate) {
    const value = state && typeof state === "object" ? state : ({})
    if (String(value.sendError || "").length) {
        return String(value.sendError)
    }
    if (value.sending === true || root.writesRunning === true) {
        return qsTr("A Social write is already running.")
    }
    if (root.messagingMutatingDiagnosticsEnabled !== true) {
        return qsTr("Enable mutating diagnostics to post comments.")
    }
    return socialGateDetailText(root, gate, qsTr("Posting is unavailable."))
}

function socialCommentReadAvailable(root, topic) {
    return socialCommentReadGate(root, topic).enabled === true
}

function socialSharedIdlReadAvailable(root) {
    return socialSharedIdlReadGate(root).enabled === true
}

function socialSharedIdlWriteAvailable(root, topic) {
    return root.storageMutatingDiagnosticsEnabled === true
        && root.messagingMutatingDiagnosticsEnabled === true
        && root.writesRunning !== true
        && socialSharedIdlWriteGate(root, topic).enabled === true
}

function validSocialTopic(root, topic) {
    const bridge = root.bridge || null
    if (!bridge || typeof bridge.callModule !== "function") {
        return false
    }
    const response = bridge.callModule(root.inspectorModule, "socialTopicValid", [String(topic || "")])
    return response && response.ok === true && response.value === true
}

function socialPageSize(root, pageSize) {
    const value = Number(pageSize || root.socialCommentPageSize || 20)
    return Number.isFinite(value) ? Math.max(1, Math.min(100, Math.floor(value))) : 20
}

function loadSocialSettings(root, value) {
    const settings = value || {}
    root.invalidateSharedIdlRequests()
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

function identityView(root) {
    const revision = root.socialIdentityRevision
    return {
        revision: revision,
        rows: socialIdentityRows(root),
        defaultMode: normalizedSocialIdentityDefaultMode(root.socialIdentityDefaultMode),
        selectedKey: String(root.selectedSocialIdentityKey || "")
    }
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
    root.gateway.saveSettingsState()
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
    const next = copyMap(current)
    next[conversation] = entry.key
    root.socialConversationIdentityKeys = next
    root.gateway.saveSettingsState()
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
    root.gateway.saveSettingsState()
    return true
}

function setSocialIdentityDefaultMode(root, mode) {
    root.socialIdentityDefaultMode = normalizedSocialIdentityDefaultMode(mode)
    root.gateway.saveSettingsState()
}

function normalizedSocialIdentityDefaultMode(value) {
    const text = String(value || "").trim().toLowerCase()
    return text === "manual" ? "manual" : "perConversation"
}

function socialIdentityPayload(identity) {
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
    root.invalidateSharedIdlRequests()
    root.sharedIdlPolicy = normalizedSharedIdlPolicy(policy)
    root.gateway.saveSettingsState()
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
    root.invalidateSharedIdlRequests()
    root.sharedIdlAutoShare = enabled === true
    root.gateway.saveSettingsState()
}

function refreshSharedIdlsForAccount(root, entityRef, dataHex, ownerProgramId) {
    const account = String(entityRef && entityRef.canonical_key || "").trim()
    const request = {
        policy: normalizedSharedIdlPolicy(root.sharedIdlPolicy),
        accountId: account,
        dataHex: String(dataHex || "").trim(),
        ownerProgramId: String(ownerProgramId || ""),
        topic: socialZoneAccountIdlTopic(root, entityRef),
        readEnabled: socialSharedIdlReadAvailable(root)
    }
    if (request.policy === "disabled" || !request.accountId.length
            || !request.topic.length || !request.dataHex.length
            || request.readEnabled !== true) {
        return false
    }
    const callerKey = "shared-idl|" + account
    if (root.storeQueryCallerPending(callerKey)) {
        return false
    }

    const query = querySocialStore(root, {
        callerKey: callerKey,
        family: "shared-idl",
        accountId: account,
        topic: request.topic
    }, "", 20, qsTr("Shared IDLs"), function (response, ticket) {
        if (!response || response.ok !== true || !root.isCurrentStoreQuery(ticket)) {
            return false
        }
        const admitted = SharedIdlTransport.acceptedEntriesFromStore(
            root,
            request,
            response.value,
            function (acceptedResponse) {
                if (!root.isCurrentStoreQuery(ticket)) {
                    return
                }
                const entries = acceptedResponse && acceptedResponse.ok === true
                        && Array.isArray(acceptedResponse.value)
                    ? acceptedResponse.value : []
                for (let i = 0; i < entries.length; ++i) {
                    applySharedIdlPolicy(root, account, entries[i])
                }
                root.releaseStoreQuery(ticket)
            }
        )
        if (admitted === false || admitted === null) {
            root.releaseStoreQuery(ticket)
            return false
        }
        return true
    })
    return query.ok
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
            root.gateway.saveIdlState()
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
    const normalized = root.gateway.normalizedIdlEntry(entry, 0)
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
    return root.gateway.idlEntryForKey(key) !== null
}

function storeSharedIdl(root, accountId, entry) {
    const owner = String(entry && entry.programIdHex || "")
        || root.gateway.canonicalProgramIdHex(entry && entry.programId)
        || root.gateway.normalizedHexText(entry && entry.programId)
    const cacheKey = sharedIdlCacheKey(root, accountId, owner)
    if (!cacheKey.length || !entry || !String(entry.key || "").length) {
        return
    }
    const next = copyMap(root.socialSharedIdls || {})
    const rows = Array.isArray(next[cacheKey]) ? next[cacheKey].slice(0) : []
    for (let i = 0; i < rows.length; ++i) {
        if (String(rows[i].key || "") === String(entry.key || "")) {
            return
        }
    }
    rows.push(entry)
    next[cacheKey] = rows
    trimZoneSharedIdls(root, next, 100)
    root.socialSharedIdls = next
    root.sharedIdlRevision += 1
}

function sharedIdlSuggestions(root, accountId, ownerProgramId) {
    const revision = root.sharedIdlRevision
    const cacheKey = sharedIdlCacheKey(root, accountId, ownerProgramId)
    const rows = cacheKey.length ? (root.socialSharedIdls || {})[cacheKey] : null
    return Array.isArray(rows) ? rows : []
}

function sharedIdlEntriesForAccount(root, accountId, ownerProgramId) {
    const policy = normalizedSharedIdlPolicy(root.sharedIdlPolicy)
    if (policy !== "sessionOnly") {
        return []
    }
    const owner = root.gateway.accountOwnerCacheKey(ownerProgramId)
    const rows = sharedIdlSuggestions(root, accountId, owner)
    const result = []
    for (let i = 0; i < rows.length; ++i) {
        const entry = rows[i] || {}
        const program = String(entry.programIdHex || "") || root.gateway.canonicalProgramIdHex(entry.programId) || root.gateway.normalizedHexText(entry.programId)
        if (!owner.length || program === owner) {
            result.push(entry)
        }
    }
    return result
}

function sharedIdlCacheKey(root, accountId, ownerProgramId) {
    const zoneScope = root.gateway.zoneScopeKey()
    const account = String(accountId || "").trim()
    const owner = root.gateway.accountOwnerCacheKey(ownerProgramId)
    if (!zoneScope.length || !account.length || !owner.length) {
        return ""
    }
    return [zoneScope, account, owner].join("|")
}

function trimZoneSharedIdls(root, values, limit) {
    const zoneScope = root.gateway.zoneScopeKey()
    const keys = Object.keys(values).filter(function (key) {
        return String(key).indexOf(zoneScope + "|") === 0
    }).sort()
    let total = 0
    for (let i = 0; i < keys.length; ++i) {
        total += Array.isArray(values[keys[i]]) ? values[keys[i]].length : 0
    }
    let excess = Math.max(0, total - Number(limit || 100))
    for (let i = 0; i < keys.length && excess > 0; ++i) {
        const rows = Array.isArray(values[keys[i]]) ? values[keys[i]].slice(0) : []
        const remove = Math.min(excess, rows.length)
        rows.splice(0, remove)
        excess -= remove
        values[keys[i]] = rows
    }
}

function publishAccountIdl(root, entityRef, ownerProgramId, idlEntry, onComplete, preparedTopic) {
    const entry = idlEntry || {}
    const topic = String(preparedTopic || socialZoneAccountIdlTopic(root, entityRef))
    const scope = zoneSocialScope(entityRef)
    const idlJson = String(entry.json || "")
    if (!topic.length || !scope || !idlJson.length
            || !socialSharedIdlWriteAvailable(root, topic)) {
        return false
    }
    const identity = socialIdentityForConversation(root, topic, "")
    if (!identity || !String(identity.key || "").length) {
        return false
    }
    return SharedIdlTransport.publish(root, {
        accountId: String(entityRef && entityRef.canonical_key || "").trim(),
        topic: topic,
        scope: scope,
        identity: socialIdentityPayload(identity),
        programId: String(ownerProgramId || entry.programIdHex || entry.programId || ""),
        idlName: String(entry.name || root.gateway.idlNameFromJson(idlJson) || qsTr("IDL")),
        idlJson: idlJson,
        writeEnabled: true
    }, onComplete)
}

function maybeAutoShareAccountIdl(root, entityRef, ownerProgramId, idlEntry) {
    if (root.sharedIdlAutoShare !== true || !idlEntry || String(idlEntry.source || "") === "shared") {
        return false
    }
    const topic = socialZoneAccountIdlTopic(root, entityRef)
    const key = [String(entityRef && entityRef.canonical_key || ""), topic,
        String(idlEntry.key || "")].join("|")
    if (!topic.length || (root.socialAutoSharedIdls || {})[key] === true) {
        return false
    }
    return publishAccountIdl(root, entityRef, ownerProgramId, idlEntry, function (response) {
        if (!response || response.ok !== true) {
            return
        }
        const next = copyMap(root.socialAutoSharedIdls || {})
        next[key] = true
        root.socialAutoSharedIdls = next
        root.gateway.saveSettingsState()
    }, topic)
}
