function storageDependency(action) {
    switch (String(action || "")) {
    case "manifests":
        return "storage.manifests.read"
    case "exists":
        return "storage.content.exists"
    case "read_by_cid":
    case "fetch":
        return "storage.content.read_by_cid"
    case "upload":
        return "storage.content.upload"
    case "backup_read_by_cid":
        return { all_of: ["storage.content.read_by_cid", "storage.backup.sync_read_by_cid"] }
    case "backup_upload":
        return { all_of: ["storage.content.upload", "storage.backup.sync_upload"] }
    case "cache":
    case "download":
        return "storage.content.download_to_file"
    case "remove":
        return "storage.content.remove"
    default:
        return "storage"
    }
}

function deliveryDependency(action) {
    switch (String(action || "")) {
    case "store_query":
        return "delivery.store.query"
    case "subscribe":
        return "delivery.subscribe"
    case "send":
        return "delivery.send"
    default:
        return "delivery"
    }
}

function diagnosticsDependency(action) {
    switch (String(action || "")) {
    case "modules.status":
        return "diagnostics.modules.status.read"
    case "modules.info":
        return "diagnostics.modules.info.read"
    case "modules.metrics":
        return "diagnostics.modules.metrics.read"
    case "probe":
        return "diagnostics.provider.probe"
    case "storage":
        return "diagnostics.storage.read"
    case "delivery":
        return "diagnostics.delivery.read"
    case "wallet":
        return "diagnostics.wallet.read"
    case "local_nodes":
        return "diagnostics.local_nodes.read"
    default:
        return "diagnostics"
    }
}

function socialDependency(action) {
    switch (String(action || "")) {
    case "comments.read":
        return "delivery.store.query"
    case "comments.write":
        return { all_of: ["delivery.send", "social.identity.local"] }
    case "shared_idl.read":
        return { all_of: ["delivery.store.query", "storage.content.read_by_cid", "storage.shared_idl.sync_read"] }
    case "shared_idl.write":
        return { all_of: ["storage.content.upload", "storage.shared_idl.sync_upload", "delivery.send", "social.identity.local"] }
    case "sync.live":
        return "delivery.subscribe"
    default:
        return "delivery"
    }
}

function walletDependency(action) {
    switch (String(action || "")) {
    case "l1.read":
        return "wallet.l1.accounts.read"
    case "l1.sign":
        return { all_of: ["wallet.l1.sign", "wallet.l1.submit"] }
    case "l2.preview":
        return { all_of: ["program_decode.static", "wallet.l2.instruction.preview"] }
    case "l2.submit":
        return { all_of: ["program_decode.static", "wallet.l2.instruction.submit"] }
    case "program.deploy":
        return "wallet.l2.program.deploy"
    default:
        return "wallet"
    }
}

function programDecodeDependency() {
    return "program_decode.static"
}
