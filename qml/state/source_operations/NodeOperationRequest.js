.pragma library

function envelope(adapterInitialization, payload, mutatingEnabled) {
    return {
        adapter: adapterInitialization || ({ source_mode: "", inputs: ({}) }),
        payload: payload || ({}),
        mutating_enabled: mutatingEnabled === true
    }
}

function storagePayload(method, args) {
    const values = Array.isArray(args) ? args : []
    switch (String(method || "")) {
    case "storageManifests":
        return {}
    case "storageExists":
    case "storageDownloadManifest":
    case "storageFetch":
    case "storageRemove":
        return { cid: String(values[0] || "") }
    case "storageUploadUrl":
        return {
            path: String(values[0] || ""),
            block_size: Number(values[1] || 65536)
        }
    case "storageUploadPayload":
        return {
            filename: String(values[0] || ""),
            payload: values[1] !== undefined ? values[1] : ({}),
            block_size: Number(values[2] || 65536)
        }
    case "storageUploadBackupCatalogEntry":
        return {
            backup_catalog_id: String(values[0] || ""),
            block_size: Number(values[1] || 65536)
        }
    case "storageDownloadBackupCatalogEntry":
        return {
            cid: String(values[0] || ""),
            local_only: values[1] === true
        }
    case "storageDownloadToUrl":
        return {
            cid: String(values[0] || ""),
            path: String(values[1] || ""),
            local_only: values[2] === true,
            block_size: Number(values[3] || 65536)
        }
    default:
        return {}
    }
}

function deliveryPayload(method, args) {
    const values = Array.isArray(args) ? args : []
    switch (String(method || "")) {
    case "deliverySubscribe":
    case "deliveryUnsubscribe":
        return { topic: String(values[0] || "") }
    case "deliverySend":
        return {
            topic: String(values[0] || ""),
            payload: String(values[1] || "")
        }
    case "deliveryCreateNode":
        return { config: String(values[0] || "") }
    case "deliveryStart":
    case "deliveryStop":
        return {}
    case "deliveryStoreQuery":
        return {
            peer_addr: String(values[0] || ""),
            content_topics: String(values[1] || ""),
            pubsub_topic: String(values[2] || ""),
            cursor: String(values[3] || ""),
            page_size: Number(values[4] || 20),
            ascending: values[5] === true,
            include_data: values[6] === true
        }
    default:
        return {}
    }
}

function payload(domain, method, args) {
    if (String(domain || "") === "storage") {
        return storagePayload(method, args)
    }
    if (String(domain || "") === "delivery") {
        return deliveryPayload(method, args)
    }
    return {}
}
