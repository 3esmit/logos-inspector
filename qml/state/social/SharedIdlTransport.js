function refreshSharedIdlsForAccount(root, accountId, dataHex, ownerProgramId) {
    const policy = root.normalizedSharedIdlPolicy(root.sharedIdlPolicy)
    const account = String(accountId || "").trim()
    const data = String(dataHex || "").trim()
    const topic = root.socialLezAccountIdlTopic(account)
    if (policy === "disabled" || !topic.length || !data.length || !root.socialSharedIdlReadAvailable()) {
        return false
    }
    const response = querySharedIdlStore(root, topic, "", 20, qsTr("Shared IDLs"))
    if (!response.ok) {
        return false
    }
    const acceptedResponse = root.requestModule(
        root.inspectorModule,
        "acceptedSharedIdlEntriesFromStoreWithStorage",
        [
            topic,
            response.value,
            account,
            data,
            String(ownerProgramId || ""),
            root.effectiveStorageSourceMode(root.storageSourceMode),
            root.configuredStorageRestUrl(),
            false
        ],
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

function publishAccountIdl(root, accountId, ownerProgramId, idlEntry) {
    const account = String(accountId || "").trim()
    const topic = root.socialLezAccountIdlTopic(account)
    const entry = idlEntry || {}
    const idlJson = String(entry.json || "")
    if (!topic.length || !idlJson.length || !root.socialSharedIdlWriteAvailable(topic)) {
        return false
    }
    const identity = root.socialIdentityForConversation(topic, "")
    const programId = String(ownerProgramId || entry.programIdHex || entry.programId || "")
    const createdAt = new Date().toISOString()
    const idlName = String(entry.name || root.idlNameFromJson(idlJson) || qsTr("IDL"))
    const artifact = {
        kind: "lez_account_idl_artifact",
        version: 1,
        account_id: account,
        program_id: programId,
        idl_name: idlName,
        idl_json: idlJson,
        created_at: createdAt
    }
    const upload = root.callInspector(
        "storageUploadPayload",
        [
            root.effectiveStorageSourceMode(root.storageSourceMode),
            root.configuredStorageRestUrl(),
            root.storageMutatingDiagnosticsEnabled === true,
            "logos-inspector-shared-idl.json",
            artifact,
            65536
        ],
        qsTr("Upload shared IDL")
    )
    if (!upload.ok || !upload.value || !String(upload.value.cid || "").length) {
        return false
    }
    const cid = String(upload.value.cid || "")
    const payload = {
        kind: "lez_account_idl",
        version: 1,
        identity: root.socialIdentityPayload(identity),
        account_id: account,
        program_id: programId,
        idl_name: idlName,
        idl_cid: cid,
        storage: {
            cid: cid,
            provider: "logos_storage",
            endpoint: root.configuredStorageRestUrl()
        },
        created_at: createdAt
    }
    const response = root.callInspector(
        "deliverySend",
        root.socialDeliveryArgs([topic, JSON.stringify(payload)]),
        qsTr("Share IDL")
    )
    return response.ok === true
}

function maybeAutoShareAccountIdl(root, accountId, ownerProgramId, idlEntry) {
    if (root.sharedIdlAutoShare !== true || !idlEntry || String(idlEntry.source || "") === "shared") {
        return false
    }
    const topic = root.socialLezAccountIdlTopic(accountId)
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

function querySharedIdlStore(root, topic, cursor, pageSize, label) {
    return root.requestModule(
        root.inspectorModule,
        "deliveryStoreQuery",
        root.socialDeliveryArgs(["", String(topic || ""), "", String(cursor || ""), root.socialPageSize(pageSize), true, true]),
        String(label || qsTr("Delivery Store")),
        false,
        false
    )
}
