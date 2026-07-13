.import "../source_operations/NodeOperationRequest.js" as NodeOperationRequest

function acceptedEntries(root, request) {
    const value = request || {}
    if (String(value.policy || "") === "disabled"
            || !String(value.topic || "").length
            || !String(value.dataHex || "").length
            || value.readEnabled !== true) {
        return []
    }
    const response = queryStore(root, value.topic, "", 20, qsTr("Shared IDLs"))
    if (!response.ok) {
        return []
    }
    const acceptedResponse = root.gateway.requestModule(
        root.inspectorModule,
        "acceptedSharedIdlEntriesFromStoreWithStorage",
        [
            value.topic,
            response.value,
            String(value.accountId || ""),
            value.dataHex,
            String(value.ownerProgramId || ""),
            root.sourceRouting.storageOperationAdapter(),
            false
        ],
        qsTr("Shared IDLs"),
        false,
        false
    )
    return acceptedResponse.ok && Array.isArray(acceptedResponse.value)
        ? acceptedResponse.value : []
}

function publish(root, request) {
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
    const upload = root.gateway.callInspector(
        "storageUploadPayload",
        [NodeOperationRequest.envelope(
            root.sourceRouting.storageOperationAdapter(),
            {
                filename: "logos-inspector-shared-idl.json",
                payload: artifact,
                block_size: 65536
            },
            root.storageMutatingDiagnosticsEnabled === true
        )],
        qsTr("Upload shared IDL")
    )
    if (!upload.ok || !upload.value || !String(upload.value.cid || "").length) {
        return false
    }
    const cid = String(upload.value.cid || "")
    const payload = {
        kind: "lez_account_idl",
        version: 2,
        identity: value.identity || {},
        account_id: String(value.accountId || ""),
        program_id: String(value.programId || ""),
        idl_name: String(value.idlName || qsTr("IDL")),
        idl_cid: cid,
        storage: {
            cid: cid,
            provider: "logos_storage",
            endpoint: root.gateway.configuredStorageRestUrl()
        },
        created_at: createdAt,
        scope: value.scope
    }
    const response = root.gateway.callInspector(
        "deliverySend",
        deliveryArgs(root, "deliverySend", [value.topic, JSON.stringify(payload)]),
        qsTr("Share IDL")
    )
    return response.ok === true
}

function queryStore(root, topic, cursor, pageSize, label) {
    return root.gateway.requestModule(
        root.inspectorModule,
        "deliveryStoreQuery",
        deliveryArgs(root, "deliveryStoreQuery", [
            "",
            String(topic || ""),
            "",
            String(cursor || ""),
            pageSize,
            true,
            true
        ]),
        String(label || qsTr("Delivery Store")),
        false,
        false
    )
}

function deliveryArgs(root, method, extra) {
    return [NodeOperationRequest.envelope(
        root.sourceRouting.deliveryOperationAdapter(),
        NodeOperationRequest.deliveryPayload(method, extra),
        root.messagingMutatingDiagnosticsEnabled === true
    )]
}
