function handle(root, eventName, args) {
    const effect = root.deliveryApp.applyModuleEvent(eventName, args)
    if (!effect || effect.changed !== true) {
        return false
    }
    if (effect.refreshMessagingConnection === true) {
        root.queryNetworkConnection("messaging", false)
    }
    if (effect.deliveryMessage) {
        root.applyIncomingSocialMessage(effect.deliveryMessage)
    }
    return true
}
