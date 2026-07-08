import QtQml
import "modules/BlockchainModuleEvents.js" as BlockchainModuleEvents
import "modules/DeliveryModuleEvents.js" as DeliveryModuleEvents
import "modules/StorageModuleEvents.js" as StorageModuleEvents

QtObject {
    id: root

    required property var bridge
    required property var model

    function install() {
        if (!bridge || !model) {
            return 0
        }
        const rows = root.subscriptionCatalog()
        let count = 0
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            count += bridge.subscribeModuleEvents(String(row.moduleName || ""), row.events || [])
        }
        return count
    }

    function ingest(moduleName, eventName, args) {
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

    function subscriptionCatalog() {
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
                moduleName: model.blockchainModule,
                events: [
                    "newBlock"
                ]
            }
        ]
    }

    property Connections bridgeEvents: Connections {
        target: root.bridge
        ignoreUnknownSignals: true

        function onHostChanged() {
            Qt.callLater(function () {
                root.install()
            })
        }

        function onModuleEventReceived(moduleName, eventName, args) {
            root.ingest(moduleName, eventName, args)
        }
    }
}
