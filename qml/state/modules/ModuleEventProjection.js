.import "../storage/StorageOperationContracts.js" as StorageOperationContracts
.import "BlockchainModuleEvents.js" as BlockchainModuleEvents
.import "DeliveryModuleEvents.js" as DeliveryModuleEvents
.import "StorageModuleEvents.js" as StorageModuleEvents

function subscriptionCatalog(model) {
    if (!model) {
        return []
    }
    return [
        {
            moduleName: model.deliveryModule,
            events: [
                "messageSent",
                "messageError",
                "messagePropagated",
                "messageReceived",
                "connectionStateChanged",
                "nodeStarted",
                "nodeStopped"
            ]
        },
        {
            moduleName: model.storageModule,
            events: StorageOperationContracts.subscriptionEvents()
        },
        {
            moduleName: model.blockchainModule,
            events: [
                "newBlock"
            ]
        }
    ]
}

function project(model, moduleName, eventName, args) {
    if (!model) {
        return false
    }
    const moduleText = String(moduleName || "")
    const eventText = String(eventName || "")
    if (moduleText === model.deliveryModule) {
        return DeliveryModuleEvents.handle(model, eventText, args)
    }
    if (moduleText === model.storageModule) {
        return StorageModuleEvents.handle(model, eventText, args)
    }
    if (moduleText === model.blockchainModule && eventText === "newBlock") {
        return BlockchainModuleEvents.handleNewBlock(model, args)
    }
    return false
}
