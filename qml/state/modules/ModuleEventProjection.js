.import "../storage/StorageOperationContracts.js" as StorageOperationContracts
.import "BlockchainModuleEvents.js" as BlockchainModuleEvents
.import "DeliveryModuleEvents.js" as DeliveryModuleEvents
.import "ModuleEventEnvelope.js" as ModuleEventEnvelope
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

function project(model, moduleName, eventName, args, forwardRuntimeEvent) {
    return projectEnvelope(
        model,
        ModuleEventEnvelope.fromRaw(moduleName, eventName, args),
        forwardRuntimeEvent
    )
}

function projectEnvelope(model, event, forwardRuntimeEvent) {
    if (!model) {
        return false
    }
    const moduleText = String(event && event.moduleName ? event.moduleName : "")
    const eventText = String(event && event.eventName ? event.eventName : "")
    if (moduleText === model.deliveryModule) {
        return DeliveryModuleEvents.handle(model, event, forwardRuntimeEvent)
    }
    if (moduleText === model.storageModule) {
        return StorageModuleEvents.handle(model, event, forwardRuntimeEvent)
    }
    if (moduleText === model.blockchainModule && eventText === "newBlock") {
        return BlockchainModuleEvents.handleNewBlock(model, event)
    }
    return false
}
