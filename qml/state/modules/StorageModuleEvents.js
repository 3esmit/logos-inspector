function handle(root, event, forwardRuntimeEvent) {
    const eventName = String(event && event.eventName ? event.eventName : "")
    const submitted = root.storageApp.applyStorageModuleEvent(
        eventName,
        event,
        undefined,
        forwardRuntimeEvent
    )
    if (rawEventInvalidatesStorageObservations(eventName)) {
        root.metrics.queryNetworkConnection("storage", false, false, "module-event")
        root.storageApp.refreshManifests(false)
    }
    return submitted !== null && submitted !== undefined
}

function rawEventInvalidatesStorageObservations(eventName) {
    const name = String(eventName || "")
    return name === "storageDownloadDone" || name === "storageRemoveDone"
}
