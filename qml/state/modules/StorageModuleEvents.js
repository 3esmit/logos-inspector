.import "../storage/StorageOperationContracts.js" as StorageOperationContracts

function handle(root, eventName, args) {
    const changed = root.storageApp.applyStorageModuleEvent(eventName, args)
    if (changed && StorageOperationContracts.refreshAfterTerminalEvent(eventName)) {
        root.queryNetworkConnection("storage", false)
        root.storageApp.refreshManifests(false)
    }
    return changed
}
