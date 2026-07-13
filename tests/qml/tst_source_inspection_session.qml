import QtQml
import QtTest
import "../../qml/services"
import "../../qml/state"
import "fixtures"

TestCase {
    id: testRoot

    name: "SourceInspectionSession"

    BridgeHostFixture {
        id: fakeHost
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    QtObject {
        id: theme

        property string textMuted: "#777777"
        property string error: "#cc3333"
        property string success: "#228844"
    }

    SourceInspectionSession {
        id: storageSession

        model: model
        theme: theme
        family: "storage"
    }

    SourceInspectionSession {
        id: deliverySession

        model: model
        theme: theme
        family: "delivery"
    }

    function init() {
        fakeHost.reset()
        model.setNetworkConnectorMode("storage", "rest")
        model.setNetworkConnectorMode("delivery", "rest")
        model.storageCidProbe = ""
        model.storageModuleReport = null
        model.messagingModuleReport = null
        model.storageSourceReport = null
        model.messagingSourceReport = null
        model.networkConnectionStatus = ({})
        model.networkConnectionStatusRevision += 1
    }

    function test_family_views_expose_complete_page_contracts() {
        verify(Array.isArray(storageSession.view.healthRows))
        verify(Array.isArray(storageSession.view.activeOperationRows))
        verify(Array.isArray(storageSession.view.capacityRows))
        verify(Array.isArray(storageSession.view.cidRows))
        compare(storageSession.view.sourceShortLabel, "REST")

        verify(Array.isArray(deliverySession.view.healthRows))
        verify(Array.isArray(deliverySession.view.protocolRows))
        verify(Array.isArray(deliverySession.view.throughputRows))
        verify(Array.isArray(deliverySession.view.topicRows))
        compare(deliverySession.view.sourceShortLabel, "REST")
    }

    function test_storage_view_reacts_to_selected_cid() {
        compare(storageSession.view.cidRows.length, 2)

        model.storageCidProbe = "z-test"

        tryCompare(storageSession.view.cidRows, "length", 5)
        compare(storageSession.view.cidRows[0].copyText, "z-test")
    }
}
