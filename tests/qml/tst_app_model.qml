import QtQuick
import QtTest
import "../../qml/services"
import "../../qml/state"

TestCase {
    id: testRoot

    name: "AppModel"

    QtObject {
        id: fakeHost

        property int callCount: 0
        property string lastMethod: ""

        function callModuleJson(moduleName, method, argsJson) {
            callCount += 1
            lastMethod = String(method || "")
            return JSON.stringify({
                ok: true,
                value: {},
                text: "OK",
                error: ""
            })
        }
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    function init() {
        fakeHost.callCount = 0
        fakeHost.lastMethod = ""
        model.currentView = "overview"
        model.dashboardMetricHistory = ({})
        model.dashboardMetricHistoryRevision = 0
        model.registeredIdls.clear()
        model.idlStateLoaded = false
        model.accountIdlSelections = ({})
        model.accountIdlSelectionRevision = 0
    }

    function test_navigation_delegates() {
        compare(model.viewTitle(), "Dashboard")
        verify(model.navRows().length > 0)

        model.selectView("programs")

        compare(model.currentView, "programs")
        compare(model.parentNavKeyForView("programs"), "l2")
        compare(model.navTokenForView("programs"), "PRG")
    }

    function test_dashboard_metric_history_prefix_clear() {
        model.dashboardMetricHistory = {
            "messaging.messages": [{ timestamp: 1, value: 1 }],
            "storage.files": [{ timestamp: 1, value: 2 }],
            "chain.height": [{ timestamp: 1, value: 3 }]
        }

        model.clearDashboardMetricHistoryForPrefix("messaging.")

        compare(model.dashboardMetricHistory["messaging.messages"], undefined)
        verify(model.dashboardMetricHistory["storage.files"] !== undefined)
        verify(model.dashboardMetricHistory["chain.height"] !== undefined)
        compare(model.dashboardMetricHistoryRevision, 1)
    }

    function test_idl_registration_delegates() {
        const programId = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        const idlJson = JSON.stringify({
            name: "Sample",
            instructions: [],
            accounts: []
        })

        model.idlStateLoaded = true
        model.registerIdl("", programId, idlJson)

        compare(model.registeredIdls.count, 1)
        compare(model.registeredIdls.get(0).name, "Sample")
        compare(model.registeredIdls.get(0).programIdHex, programId.slice(2))
        compare(fakeHost.lastMethod, "saveIdlState")
    }
}
