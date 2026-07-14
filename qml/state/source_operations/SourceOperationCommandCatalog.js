.pragma library

function storageCommand(method, args) {
    const action = storageActionForMethod(method)
    return {
        method: String(method || ""),
        action: action,
        requiredInputs: storageRequiredInputs(action, args),
        runtime: String(method || "") !== "storageExists"
    }
}

function storageActionForMethod(method) {
    switch (String(method || "")) {
    case "storageManifests":
        return "manifests"
    case "storageExists":
        return "exists"
    case "storageDownloadManifest":
        return "read_by_cid"
    case "storageFetch":
        return "cache"
    case "storageUploadUrl":
        return "upload"
    case "storageUploadBackupCatalogEntry":
        return "backup_upload"
    case "storageDownloadToUrl":
        return "download"
    case "storageRemove":
        return "remove"
    default:
        return "storage"
    }
}

function storageRequiredInputs(action, args) {
    const values = Array.isArray(args) ? args : []
    switch (String(action || "")) {
    case "exists":
    case "read_by_cid":
    case "cache":
    case "remove":
        return [{ key: "cid", label: qsTr("CID"), value: values[0] }]
    case "download":
        return [
            { key: "cid", label: qsTr("CID"), value: values[0] },
            { key: "path", label: qsTr("Save path"), value: values[1] }
        ]
    case "upload":
        return [{ key: "path", label: qsTr("File path"), value: values[0] }]
    case "backup_upload":
        return [{ key: "backup_catalog_id", label: qsTr("Backup catalog ID"), value: values[0] }]
    default:
        return []
    }
}

function deliveryCommand(method, args) {
    return {
        method: String(method || ""),
        action: deliveryActionForMethod(method),
        requiredInputs: deliveryRequiredInputs(method, args),
        runtime: true
    }
}

function deliveryActionForMethod(method) {
    switch (String(method || "")) {
    case "deliveryStoreQuery":
        return "store_query"
    case "deliverySubscribe":
        return "subscribe"
    case "deliveryUnsubscribe":
        return "unsubscribe"
    case "deliverySend":
        return "send"
    case "deliveryCreateNode":
        return "node_create"
    case "deliveryStart":
        return "node_start"
    case "deliveryStop":
        return "node_stop"
    default:
        return "delivery"
    }
}

function deliveryRequiredInputs(method, args) {
    const values = Array.isArray(args) ? args : []
    switch (String(method || "")) {
    case "deliveryStoreQuery":
        return [{ key: "topic", label: qsTr("Content topic"), value: values[1] }]
    case "deliverySubscribe":
    case "deliveryUnsubscribe":
    case "deliverySend":
        return [{ key: "topic", label: qsTr("Content topic"), value: values[0] }]
    case "deliveryCreateNode":
        return [{ key: "config", label: qsTr("Node config"), value: values[0] }]
    default:
        return []
    }
}

function gateDetailText(gate, fallbackLabel) {
    const value = gate || {}
    const missing = Array.isArray(value.missing) ? value.missing : []
    if (missing.length > 0) {
        const first = missing[0] || {}
        const dependency = String(first.dependency || first.capability || "")
        const label = String(first.label || dependency || fallbackLabel || qsTr("Capability"))
        return dependency.length ? qsTr("%1 unavailable: %2").arg(label).arg(dependency) : qsTr("%1 unavailable").arg(label)
    }
    return qsTr("%1 unavailable.").arg(String(fallbackLabel || qsTr("Capability")))
}

function operationCompleted(operation) {
    return String(operation && operation.status ? operation.status : "") === "completed"
}
