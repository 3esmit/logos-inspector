function acceptedEntriesFromStore(root, request, storeValue, callback) {
    const value = request || {}
    if (String(value.policy || "") === "disabled"
            || !String(value.topic || "").length
            || !String(value.dataHex || "").length
            || value.readEnabled !== true
            || typeof callback !== "function"
            || !root.gateway
            || typeof root.gateway.requestModuleAsync !== "function") {
        return false
    }
    return root.gateway.requestModuleAsync(
        root.inspectorModule,
        "acceptedSharedIdlEntriesFromStoreWithStorage",
        [
            value.topic,
            storeValue,
            String(value.accountId || ""),
            value.dataHex,
            String(value.ownerProgramId || ""),
            root.sourceRouting.storageOperationAdapter(),
            false
        ],
        qsTr("Shared IDLs"),
        false,
        function (response) {
            callback({
                ok: response && response.ok === true && Array.isArray(response.value),
                value: response && Array.isArray(response.value) ? response.value : [],
                error: String(response && response.error || "")
            })
        }
    )
}

function publish(root, request, callback) {
    const value = request || {}
    const idlJson = String(value.idlJson || "")
    if (!String(value.topic || "").length || !value.scope || !idlJson.length
            || value.writeEnabled !== true) {
        return false
    }
    const createdAt = new Date().toISOString()
    const artifact = {
        kind: "lez_account_idl_artifact",
        version: 2,
        account_id: String(value.accountId || ""),
        program_id: String(value.programId || ""),
        idl_name: String(value.idlName || qsTr("IDL")),
        idl_json: idlJson,
        created_at: createdAt,
        scope: value.scope
    }
    const message = {
        kind: "lez_account_idl",
        version: 2,
        identity: value.identity || {},
        account_id: String(value.accountId || ""),
        program_id: String(value.programId || ""),
        idl_name: String(value.idlName || qsTr("IDL")),
        created_at: createdAt,
        scope: value.scope
    }
    return root.startSharedIdlWrite({
        filename: "logos-inspector-shared-idl.json",
        artifact: artifact,
        blockSize: 65536,
        topic: String(value.topic || ""),
        message: message,
        uploadLabel: qsTr("Upload shared IDL"),
        deliveryLabel: qsTr("Share IDL")
    }, callback)
}
