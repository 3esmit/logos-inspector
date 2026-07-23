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
        const basecampModules = typeof bridge.prefersBasecampModules === "function"
            && bridge.prefersBasecampModules()
        if (basecampModules
                && typeof bridge.ensureRuntimeModuleEventOwnership === "function") {
            let completedSynchronously = false
            let count = 0
            const resolved = bridge.ensureRuntimeModuleEventOwnership(function () {
                count = root.installSubscriptionCatalog(true)
                completedSynchronously = true
            })
            return resolved === true && completedSynchronously ? count : 0
        }
        return root.installSubscriptionCatalog(basecampModules)
    }

    function installSubscriptionCatalog(basecampModules) {
        if (typeof bridge.startModuleWatcher === "function") {
            const started = bridge.startModuleWatcher()
            if (started === false) {
                root.ingest(model.deliveryModule, "eventStreamUnavailable", [{
                    source: "standalone_module_watcher",
                    status: "unavailable",
                    reason: qsTr("Standalone module watcher failed to start.")
                }])
            }
        }
        const rows = root.subscriptionCatalog()
        let count = 0
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            const moduleName = String(row.moduleName || "")
            const events = Array.isArray(row.events) ? row.events : []
            const subscribed = bridge.subscribeModuleEvents(moduleName, events)
            count += subscribed
            if (moduleName === String(model.deliveryModule || "")
                    && basecampModules) {
                const ready = subscribed === events.length
                root.ingest(moduleName,
                    ready ? "eventStreamReady" : "eventStreamUnavailable", [{
                        source: "basecamp_module_subscription",
                        status: ready ? "ready" : "unavailable",
                        reason: ready
                            ? qsTr("Delivery module event subscriptions are active.")
                            : qsTr("Delivery module event subscriptions are incomplete.")
                    }])
            }
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
        if (root.daemonRuntimeEvent(moduleName, eventName)
                && root.model
                && typeof root.model.invalidateAttachedRuntimeObservations === "function") {
            root.model.invalidateAttachedRuntimeObservations()
        }
        if (root.refreshesLocalNodeStatus(moduleName, eventName)) {
            root.queueLocalNodeRefresh()
        }
        return projected
    }

    function daemonRuntimeEvent(moduleName, eventName) {
        const moduleText = String(moduleName || "")
        const eventText = String(eventName || "")
        return moduleText === "logoscore_runtime"
            && (eventText === "daemonStarted"
                || eventText === "daemonStopped"
                || eventText === "daemonUnavailable")
    }

    function refreshesLocalNodeStatus(moduleName, eventName) {
        const moduleText = String(moduleName || "")
        const eventText = String(eventName || "")
        if (root.daemonRuntimeEvent(moduleText, eventText)) {
            return true
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
            if (root.model && root.model.metrics
                    && typeof root.model.metrics.resetDeliveryModuleEventTelemetry === "function") {
                root.model.metrics.resetDeliveryModuleEventTelemetry("unknown", "")
            }
            Qt.callLater(function () {
                root.install()
            })
        }

        function onModuleEventReceived(moduleName, eventName, args) {
            root.ingest(moduleName, eventName, args)
        }
    }
}
