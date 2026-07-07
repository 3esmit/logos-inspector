.import "BlockchainModuleEvents.js" as BlockchainModuleEvents
.import "DeliveryModuleEvents.js" as DeliveryModuleEvents
.import "StorageModuleEvents.js" as StorageModuleEvents

function moduleEventSubscriptions(root) {
    with (root) {
        return [
            {
                moduleName: deliveryModule,
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
                moduleName: storageModule,
                events: [
                    "storageStart",
                    "storageStop",
                    "storageConnect",
                    "storageUploadProgress",
                    "storageUploadDone",
                    "storageDownloadProgress",
                    "storageDownloadDone",
                    "storageDownloadManifestDone",
                    "storageRemoveDone"
                ]
            },
            {
                moduleName: blockchainModule,
                events: [
                    "newBlock"
                ]
            }
        ]
    }
}

function handleModuleEvent(root, moduleName, eventName, args) {
    with (root) {
        const moduleText = String(moduleName || "")
        const eventText = String(eventName || "")
        if (moduleText === deliveryModule) {
            return DeliveryModuleEvents.handle(root, eventText, args)
        }
        if (moduleText === storageModule) {
            return StorageModuleEvents.handle(root, eventText, args)
        }
        if (moduleText === blockchainModule && eventText === "newBlock") {
            return BlockchainModuleEvents.handleNewBlock(root, args)
        }
        return false
    }
}

function deliveryModuleEventRows(root) {
    return DeliveryModuleEvents.eventRows(root)
}

function deliveryModuleEventSummary(root) {
    return DeliveryModuleEvents.eventSummary(root)
}
