.import "../storage/StorageOperationContracts.js" as StorageOperationContracts

function handle(root, event) {
    const eventName = String(event && event.eventName ? event.eventName : "")
    const changed = root.storageApp.applyStorageModuleEvent(eventName, event)
    if (changed && StorageOperationContracts.refreshAfterTerminalEvent(eventName)) {
        root.queryNetworkConnection("storage", false)
        root.storageApp.refreshManifests(false)
    }
    return changed
}
