import QtQuick

QtObject {
    id: root

    required property var model
    property int operationPollInterval: 500

    property list<QtObject> timers: [
        Timer {
            interval: root.intervalFor("blockchain")
            repeat: true
            running: root.enabled("blockchain")
            onTriggered: root.tick("blockchain")
        },
        Timer {
            interval: root.intervalFor("messaging")
            repeat: true
            running: root.enabled("messaging")
            onTriggered: root.tick("messaging")
        },
        Timer {
            interval: root.intervalFor("storage")
            repeat: true
            running: root.enabled("storage")
            onTriggered: root.tick("storage")
        },
        Timer {
            interval: root.intervalFor("dashboard")
            repeat: true
            running: root.enabled("dashboard")
            onTriggered: root.tick("dashboard")
        },
        Timer {
            interval: root.intervalFor("storageOperation")
            repeat: true
            running: root.enabled("storageOperation")
            onTriggered: root.tick("storageOperation")
        },
        Timer {
            interval: root.intervalFor("deliveryOperation")
            repeat: true
            running: root.enabled("deliveryOperation")
            onTriggered: root.tick("deliveryOperation")
        },
        Timer {
            interval: root.intervalFor("socialOperation")
            repeat: true
            running: root.enabled("socialOperation")
            onTriggered: root.tick("socialOperation")
        },
        Timer {
            interval: root.intervalFor("chainOperation")
            repeat: true
            running: root.enabled("chainOperation")
            onTriggered: root.tick("chainOperation")
        },
        Timer {
            interval: root.intervalFor("liveBlocks")
            repeat: true
            running: root.enabled("liveBlocks")
            onTriggered: root.tick("liveBlocks")
        },
        Timer {
            id: zonesStatusTimer

            interval: root.intervalFor("zonesStatus")
            repeat: true
            running: root.enabled("zonesStatus")
            onTriggered: root.tick("zonesStatus")
        }
    ]

    property Connections zonesStatusConnections: Connections {
        target: root.zoneState()
        ignoreUnknownSignals: true

        function onStatusRefreshRequested() {
            root.triggerZonesStatus()
        }
    }

    property Connections applicationStateConnections: Connections {
        target: Application

        function onActiveChanged() {
            if (Application.active) {
                root.applicationResumed()
            }
        }
    }

    function intervalFor(kind) {
        if (!model) {
            return 1
        }
        switch (String(kind || "")) {
        case "dashboard":
            return Math.max(1, Number(model.dashboardRefreshInterval ? model.dashboardRefreshInterval() : 0))
        case "storageOperation":
        case "deliveryOperation":
        case "socialOperation":
        case "chainOperation":
            return Math.max(1, Number(operationPollInterval || 500))
        case "liveBlocks":
            return Math.max(1, Number(model.refreshInterval ? model.refreshInterval(model.blockchainRefreshRate) : 0))
        case "zonesStatus":
            return Math.max(1, Number(root.zoneState() ? root.zoneState().statusPollInterval : 0))
        default:
            return Math.max(1, Number(model.refreshInterval ? model.refreshInterval(root.refreshRateFor(kind)) : 0))
        }
    }

    function enabled(kind) {
        if (!model) {
            return false
        }
        switch (String(kind || "")) {
        case "blockchain":
        case "messaging":
        case "storage":
            return root.refreshRateFor(kind) > 0
        case "dashboard":
            return model.shell.currentView === "overview"
                && root.intervalFor("dashboard") > 0
        case "storageOperation":
            return (root.storageApp() && root.storageApp().operation.running)
                || model.backupCatalogTransferRunning === true
                || model.backupCatalogUploadRunning === true
                || model.backupCatalogDownloadRunning === true
        case "deliveryOperation":
            return root.deliveryApp() && root.deliveryApp().operation.running
        case "socialOperation":
            return root.socialState() && root.socialState().operationsRunning === true
        case "chainOperation":
            return root.chainState() && root.chainState().operationsRunning === true
        case "liveBlocks":
            return model.blocksLiveEnabled === true && model.shell.currentView === "blocks"
        case "zonesStatus":
            return root.zoneState() && root.zoneState().statusPollingEnabled === true
        default:
            return false
        }
    }

    function tick(kind) {
        if (!model) {
            return null
        }
        switch (String(kind || "")) {
        case "blockchain":
        case "messaging":
        case "storage":
            return model.queryNetworkConnection(kind, false)
        case "dashboard":
            return model.refreshDashboard()
        case "storageOperation":
            return root.pollStorageOperations()
        case "deliveryOperation":
            return root.deliveryApp() ? root.deliveryApp().pollDeliveryOperation(false) : null
        case "socialOperation":
            return root.socialState() ? root.socialState().pollOperations() : null
        case "chainOperation":
            return root.chainState() ? root.chainState().pollOperations() : null
        case "liveBlocks":
            return model.chainPages ? model.chainPages.refreshBlocksLivePage() : model.refreshBlocksLivePage()
        case "zonesStatus":
            return root.zoneState() ? root.zoneState().pollStatus() : null
        default:
            return null
        }
    }

    function refreshRateFor(kind) {
        if (!model) {
            return 0
        }
        switch (String(kind || "")) {
        case "blockchain":
            return Number(model.blockchainRefreshRate || 0)
        case "messaging":
            return Number(model.messagingRefreshRate || 0)
        case "storage":
            return Number(model.storageRefreshRate || 0)
        default:
            return 0
        }
    }

    function storageApp() {
        return model && model.storageApp ? model.storageApp : null
    }

    function pollStorageOperations() {
        let result = null
        const storage = root.storageApp()
        if (storage && storage.operation.running) {
            result = storage.pollStorageOperation(false)
        }
        if (model.backupCatalog && model.backupCatalogUploadRunning === true
                && typeof model.backupCatalog.pollUpload === "function") {
            const uploadResult = model.backupCatalog.pollUpload()
            result = result === null ? uploadResult : result
        }
        if (model.backupCatalog && model.backupCatalogDownloadRunning === true
                && typeof model.backupCatalog.pollDownload === "function") {
            const downloadResult = model.backupCatalog.pollDownload()
            result = result === null ? downloadResult : result
        }
        return result
    }

    function deliveryApp() {
        return model && model.deliveryApp ? model.deliveryApp : null
    }

    function socialState() {
        return model && model.social ? model.social : null
    }

    function chainState() {
        return model && model.chainPages ? model.chainPages : null
    }

    function zoneState() {
        return model && model.zoneInspection ? model.zoneInspection : null
    }

    function triggerZonesStatus() {
        if (!root.enabled("zonesStatus")) {
            return false
        }
        const result = root.tick("zonesStatus")
        zonesStatusTimer.restart()
        return result
    }

    function applicationResumed() {
        const state = root.zoneState()
        return state && typeof state.appResumed === "function" ? state.appResumed() : false
    }
}
