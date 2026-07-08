import QtQml

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
            interval: root.intervalFor("indexer")
            repeat: true
            running: root.enabled("indexer")
            onTriggered: root.tick("indexer")
        },
        Timer {
            interval: root.intervalFor("execution")
            repeat: true
            running: root.enabled("execution")
            onTriggered: root.tick("execution")
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
            interval: root.intervalFor("liveBlocks")
            repeat: true
            running: root.enabled("liveBlocks")
            onTriggered: root.tick("liveBlocks")
        }
    ]

    function intervalFor(kind) {
        if (!model) {
            return 1
        }
        switch (String(kind || "")) {
        case "dashboard":
            return Math.max(1, Number(model.dashboardRefreshInterval ? model.dashboardRefreshInterval() : 0))
        case "storageOperation":
        case "deliveryOperation":
            return Math.max(1, Number(operationPollInterval || 500))
        case "liveBlocks":
            return Math.max(1, Number(model.refreshInterval ? model.refreshInterval(model.blockchainRefreshRate) : 0))
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
        case "indexer":
        case "execution":
        case "messaging":
        case "storage":
            return root.refreshRateFor(kind) > 0
        case "dashboard":
            return model.currentView === "overview"
                && root.intervalFor("dashboard") > 0
        case "storageOperation":
            return root.storageApp() && root.storageApp().activeStorageOperationRunning()
        case "deliveryOperation":
            return root.deliveryApp() && root.deliveryApp().activeDeliveryOperationRunning()
        case "liveBlocks":
            return model.blocksLiveEnabled === true && model.currentView === "blocks"
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
        case "indexer":
        case "execution":
        case "messaging":
        case "storage":
            return model.queryNetworkConnection(kind, false)
        case "dashboard":
            return model.refreshDashboard()
        case "storageOperation":
            return root.storageApp() ? root.storageApp().pollStorageOperation(false) : null
        case "deliveryOperation":
            return root.deliveryApp() ? root.deliveryApp().pollDeliveryOperation(false) : null
        case "liveBlocks":
            return model.refreshBlocksLivePage()
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
        case "indexer":
            return Number(model.indexerRefreshRate || 0)
        case "execution":
            return Number(model.executionRefreshRate || 0)
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

    function deliveryApp() {
        return model && model.deliveryApp ? model.deliveryApp : null
    }
}
