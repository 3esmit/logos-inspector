function handle(root, event, forwardRuntimeEvent) {
    const effect = root.deliveryApp.applyModuleEvent(
        event.eventName,
        event,
        forwardRuntimeEvent
    )
    if (!effect || effect.changed !== true) {
        return false
    }
    if (effect.refreshMessagingConnection === true) {
        root.queryNetworkConnection("messaging", false)
    }
    if (effect.deliveryMessage) {
        root.social.applyIncomingDeliveryMessage(effect.deliveryMessage)
    }
    return true
}
