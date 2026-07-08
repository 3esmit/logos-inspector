function handle(root, eventName, args) {
    const changed = root.storageApp.applyStorageModuleEvent(eventName, args)
    if (changed && isTerminalRefreshEvent(eventName)) {
        root.queryNetworkConnection("storage", false)
        root.storageApp.refreshManifests(false)
    }
    return changed
}

function isTerminalRefreshEvent(eventName) {
    return eventName === "storageUploadDone"
        || eventName === "storageDownloadDone"
        || eventName === "storageRemoveDone"
}
