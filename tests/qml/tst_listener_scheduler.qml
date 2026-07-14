import QtQml
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "ListenerScheduler"

    QtObject {
        id: storageState

        property bool running: false
        property int polls: 0
        readonly property var operation: ({ running: storageState.running })

        function pollStorageOperation(showResult) {
            polls += 1
            return showResult
        }
    }

    QtObject {
        id: deliveryState

        property bool running: false
        property int polls: 0
        readonly property var operation: ({ running: deliveryState.running })

        function pollDeliveryOperation(showResult) {
            polls += 1
            return showResult
        }
    }

    QtObject {
        id: backupCatalogState

        property int polls: 0
        property int downloadPolls: 0

        function pollUpload() {
            polls += 1
            return polls
        }

        function pollDownload() {
            downloadPolls += 1
            return downloadPolls
        }
    }

    QtObject {
        id: socialState

        property bool operationsRunning: false
        property int polls: 0

        function pollOperations() {
            polls += 1
            return polls
        }
    }

    QtObject {
        id: chainState

        property bool operationsRunning: false
        property int polls: 0

        function pollOperations() {
            polls += 1
            return polls
        }

        function refreshBlocksLivePage() {
            fakeModel.liveCalls += 1
            return fakeModel.liveCalls
        }
    }

    QtObject {
        id: zoneState

        signal statusRefreshRequested()

        property bool statusPollingEnabled: false
        property int statusPollInterval: 5000
        property int polls: 0
        property int resumes: 0

        function pollStatus() {
            polls += 1
            return true
        }

        function appResumed() {
            resumes += 1
            return true
        }
    }

    QtObject {
        id: fakeModel

        property int blockchainRefreshRate: 30
        property int messagingRefreshRate: 0
        property int storageRefreshRate: 0
        property QtObject shell: QtObject {
            property string currentView: "overview"
        }
        property bool blocksLiveEnabled: false
        property bool backupCatalogUploadRunning: false
        property bool backupCatalogDownloadRunning: false
        property bool backupCatalogTransferRunning: false
        property QtObject storageApp: storageState
        property QtObject backupCatalog: backupCatalogState
        property QtObject deliveryApp: deliveryState
        property QtObject social: socialState
        property QtObject chainPages: chainState
        property QtObject zoneInspection: zoneState
        property var queriedKinds: []
        property int dashboardCalls: 0
        property int liveCalls: 0

        function refreshInterval(seconds) {
            return Math.max(5, Number(seconds || 0)) * 1000
        }

        function dashboardRefreshInterval() {
            return 15000
        }

        function queryNetworkConnection(kind, showResult) {
            queriedKinds = queriedKinds.concat([String(kind || "")])
            return showResult
        }

        function refreshDashboard() {
            dashboardCalls += 1
            return dashboardCalls
        }

        function refreshBlocksLivePage() {
            liveCalls += 1
            return liveCalls
        }
    }

    ListenerScheduler {
        id: scheduler

        model: fakeModel
        operationPollInterval: 10
    }

    function init() {
        fakeModel.blockchainRefreshRate = 30
        fakeModel.messagingRefreshRate = 0
        fakeModel.storageRefreshRate = 0
        fakeModel.shell.currentView = "overview"
        fakeModel.blocksLiveEnabled = false
        fakeModel.backupCatalogUploadRunning = false
        fakeModel.backupCatalogDownloadRunning = false
        fakeModel.backupCatalogTransferRunning = false
        fakeModel.queriedKinds = []
        fakeModel.dashboardCalls = 0
        fakeModel.liveCalls = 0
        storageState.running = false
        storageState.polls = 0
        backupCatalogState.polls = 0
        backupCatalogState.downloadPolls = 0
        deliveryState.running = false
        deliveryState.polls = 0
        socialState.operationsRunning = false
        socialState.polls = 0
        chainState.operationsRunning = false
        chainState.polls = 0
        zoneState.statusPollingEnabled = false
        zoneState.statusPollInterval = 5000
        zoneState.polls = 0
        zoneState.resumes = 0
    }

    function test_tick_routes_refresh_consumers() {
        scheduler.tick("blockchain")
        scheduler.tick("dashboard")
        storageState.running = true
        fakeModel.backupCatalogUploadRunning = true
        deliveryState.running = true
        socialState.operationsRunning = true
        chainState.operationsRunning = true
        scheduler.tick("storageOperation")
        scheduler.tick("deliveryOperation")
        scheduler.tick("socialOperation")
        scheduler.tick("chainOperation")
        scheduler.tick("liveBlocks")

        compare(fakeModel.queriedKinds[0], "blockchain")
        compare(fakeModel.dashboardCalls, 1)
        compare(storageState.polls, 1)
        compare(backupCatalogState.polls, 1)
        compare(deliveryState.polls, 1)
        compare(socialState.polls, 1)
        compare(chainState.polls, 1)
        compare(fakeModel.liveCalls, 1)
    }

    function test_enabled_guards_page_scoped_consumers() {
        verify(scheduler.enabled("blockchain"))
        verify(!scheduler.enabled("indexer"))
        verify(!scheduler.enabled("storageOperation"))
        storageState.running = true
        verify(scheduler.enabled("storageOperation"))
        storageState.running = false
        fakeModel.backupCatalogUploadRunning = true
        verify(scheduler.enabled("storageOperation"))
        verify(!scheduler.enabled("socialOperation"))
        socialState.operationsRunning = true
        verify(scheduler.enabled("socialOperation"))
        compare(scheduler.intervalFor("socialOperation"), 10)
        verify(!scheduler.enabled("chainOperation"))
        chainState.operationsRunning = true
        verify(scheduler.enabled("chainOperation"))
        compare(scheduler.intervalFor("chainOperation"), 10)
        verify(scheduler.enabled("dashboard"))
        fakeModel.shell.currentView = "blocks"
        verify(!scheduler.enabled("dashboard"))
        fakeModel.blocksLiveEnabled = true
        verify(scheduler.enabled("liveBlocks"))
    }

    function test_backup_download_enables_storage_polling_and_uses_download_session() {
        fakeModel.backupCatalogDownloadRunning = true
        fakeModel.backupCatalogTransferRunning = true

        verify(scheduler.enabled("storageOperation"))
        scheduler.tick("storageOperation")

        compare(backupCatalogState.polls, 0)
        compare(backupCatalogState.downloadPolls, 1)

        fakeModel.backupCatalogDownloadRunning = false
        fakeModel.backupCatalogTransferRunning = false
        verify(!scheduler.enabled("storageOperation"))
    }

    function test_zones_status_uses_adaptive_interval_and_immediate_signal() {
        zoneState.statusPollingEnabled = true
        zoneState.statusPollInterval = 2000

        verify(scheduler.enabled("zonesStatus"))
        compare(scheduler.intervalFor("zonesStatus"), 2000)

        zoneState.statusRefreshRequested()

        compare(zoneState.polls, 1)
        scheduler.applicationResumed()
        compare(zoneState.resumes, 1)
    }

    function test_channel_source_probes_are_not_scheduled_by_qml() {
        verify(!scheduler.enabled("indexer"))
        verify(!scheduler.enabled("execution"))
        compare(scheduler.tick("indexer"), null)
        compare(scheduler.tick("execution"), null)
        compare(fakeModel.queriedKinds.length, 0)
    }
}
