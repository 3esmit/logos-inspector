import QtQml
import "modules/ModuleEventProjection.js" as ModuleEventProjection

QtObject {
    id: root

    required property var bridge
    required property var model
    property bool localNodeRefreshQueued: false

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
        const projected = ModuleEventProjection.project(
            model,
            moduleName,
            eventName,
            args,
            root.forwardsRuntimeOperationEvents()
        )
        if (root.refreshesLocalNodeStatus(moduleName, eventName)) {
            root.queueLocalNodeRefresh()
        }
        return projected
    }

    function refreshesLocalNodeStatus(moduleName, eventName) {
        const moduleText = String(moduleName || "")
        const eventText = String(eventName || "")
        if (moduleText === "logoscore_runtime") {
            return eventText === "daemonStarted"
                || eventText === "daemonStopped"
                || eventText === "daemonUnavailable"
        }
        if (moduleText === String(model && model.blockchainModule ? model.blockchainModule : "")) {
            return eventText === "moduleReady"
                || eventText === "moduleUnavailable"
                || eventText === "nodeStarted"
                || eventText === "nodeStopped"
                || eventText === "nodeUnavailable"
        }
        if (moduleText === "indexer_service" || moduleText === "sequencer_service") {
            return eventText === "nodeStarted"
                || eventText === "nodeStopped"
                || eventText === "nodeUnavailable"
        }
        if (moduleText === String(model && model.deliveryModule ? model.deliveryModule : "")) {
            return eventText === "moduleReady"
                || eventText === "moduleUnavailable"
                || eventText === "nodeStarted"
                || eventText === "nodeStopped"
                || eventText === "nodeUnavailable"
        }
        if (moduleText === String(model && model.storageModule ? model.storageModule : "")) {
            return eventText === "moduleReady"
                || eventText === "moduleUnavailable"
                || eventText === "storageStart"
                || eventText === "storageStop"
                || eventText === "nodeUnavailable"
        }
        return false
    }

    function queueLocalNodeRefresh() {
        if (root.localNodeRefreshQueued) {
            return
        }
        root.localNodeRefreshQueued = true
        Qt.callLater(function () {
            root.localNodeRefreshQueued = false
            if (root.model && typeof root.model.refreshLocalNodes === "function") {
                root.model.refreshLocalNodes(false)
            }
        })
    }

    function forwardsRuntimeOperationEvents() {
        return !bridge || typeof bridge.backendOwnsRuntimeModuleEvents !== "function"
            || bridge.backendOwnsRuntimeModuleEvents() !== true
    }

    function subscriptionCatalog() {
        return ModuleEventProjection.subscriptionCatalog(model)
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
