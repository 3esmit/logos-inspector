function handle(root, event, forwardRuntimeEvent) {
    const eventName = String(event && event.eventName || "")
    if (eventName === "eventStreamReady"
            || eventName === "eventStreamUnavailable") {
        return root.metrics
            && typeof root.metrics.recordDeliveryModuleEvent === "function"
            && root.metrics.recordDeliveryModuleEvent(eventName, event)
    }
    const effect = root.deliveryApp.applyModuleEvent(
        eventName,
        event,
        forwardRuntimeEvent
    )
    if (!effect || effect.changed !== true) {
        return false
    }
    if (effect.refreshMessagingConnection === true) {
        root.metrics.queryNetworkConnection("messaging", false, false, "module-event")
    }
    if (effect.deliveryMessage) {
        root.social.applyIncomingDeliveryMessage(effect.deliveryMessage)
    }
    if (root.metrics
            && typeof root.metrics.recordDeliveryModuleEvent === "function") {
        root.metrics.recordDeliveryModuleEvent(eventName, event)
    }
    return true
}
