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

        function activeStorageOperationRunning() {
            return running
        }

        function pollStorageOperation(showResult) {
            polls += 1
            return showResult
        }
    }

    QtObject {
        id: deliveryState

        property bool running: false
        property int polls: 0

        function activeDeliveryOperationRunning() {
            return running
        }

        function pollDeliveryOperation(showResult) {
            polls += 1
            return showResult
        }
    }

    QtObject {
        id: fakeModel

        property int blockchainRefreshRate: 30
        property int indexerRefreshRate: 0
        property int executionRefreshRate: 0
        property int messagingRefreshRate: 0
        property int storageRefreshRate: 0
        property string currentView: "overview"
        property bool blocksLiveEnabled: false
        property QtObject storageApp: storageState
        property QtObject deliveryApp: deliveryState
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
        fakeModel.indexerRefreshRate = 0
        fakeModel.executionRefreshRate = 0
        fakeModel.messagingRefreshRate = 0
        fakeModel.storageRefreshRate = 0
        fakeModel.currentView = "overview"
        fakeModel.blocksLiveEnabled = false
        fakeModel.queriedKinds = []
        fakeModel.dashboardCalls = 0
        fakeModel.liveCalls = 0
        storageState.running = false
        storageState.polls = 0
        deliveryState.running = false
        deliveryState.polls = 0
    }

    function test_tick_routes_refresh_consumers() {
        scheduler.tick("blockchain")
        scheduler.tick("dashboard")
        storageState.running = true
        deliveryState.running = true
        scheduler.tick("storageOperation")
        scheduler.tick("deliveryOperation")
        scheduler.tick("liveBlocks")

        compare(fakeModel.queriedKinds[0], "blockchain")
        compare(fakeModel.dashboardCalls, 1)
        compare(storageState.polls, 1)
        compare(deliveryState.polls, 1)
        compare(fakeModel.liveCalls, 1)
    }

    function test_enabled_guards_page_scoped_consumers() {
        verify(scheduler.enabled("blockchain"))
        verify(!scheduler.enabled("indexer"))
        verify(!scheduler.enabled("storageOperation"))
        storageState.running = true
        verify(scheduler.enabled("storageOperation"))
        verify(scheduler.enabled("dashboard"))
        fakeModel.currentView = "blocks"
        verify(!scheduler.enabled("dashboard"))
        fakeModel.blocksLiveEnabled = true
        verify(scheduler.enabled("liveBlocks"))
    }
}
