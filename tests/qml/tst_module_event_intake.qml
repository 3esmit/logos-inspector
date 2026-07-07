import QtQuick
import QtTest
import "../../qml/services"
import "../../qml/state"

TestCase {
    id: testRoot

    name: "ModuleEventIntake"

    QtObject {
        id: fakeHost

        property var subscriptions: []

        function onModuleEvent(moduleName, eventName, callback) {
            subscriptions = subscriptions.concat([{
                moduleName: String(moduleName || ""),
                eventName: String(eventName || ""),
                callback: callback
            }])
        }
    }

    QtObject {
        id: replacementHost

        property var subscriptions: []

        function onModuleEvent(moduleName, eventName, callback) {
            subscriptions = subscriptions.concat([{
                moduleName: String(moduleName || ""),
                eventName: String(eventName || ""),
                callback: callback
            }])
        }
    }

    BridgeClient {
        id: bridge

        host: fakeHost
    }

    AppModel {
        id: model

        bridge: bridge
    }

    ModuleEventIntake {
        id: intake

        bridge: bridge
        model: model
    }

    function init() {
        bridge.host = fakeHost
        fakeHost.subscriptions = []
        replacementHost.subscriptions = []
        bridge.moduleEventSubscriptions = ({})
        model.deliveryModuleEvents = []
        model.deliveryModuleEventRevision = 0
        model.deliveryConnectionStatus = ""
        model.deliveryNodeStatus = ""
        model.socialCommentState = ({})
        model.socialCommentRevision = 0
    }

    function test_install_subscribes_module_event_catalog() {
        const count = intake.install()

        compare(count, 17)
        compare(fakeHost.subscriptions.length, 17)
        compare(fakeHost.subscriptions[0].moduleName, model.deliveryModule)
        compare(fakeHost.subscriptions[0].eventName, "messageSent")
        compare(fakeHost.subscriptions[fakeHost.subscriptions.length - 1].moduleName, model.blockchainModule)
        compare(fakeHost.subscriptions[fakeHost.subscriptions.length - 1].eventName, "newBlock")
    }

    function test_host_swap_resubscribes_catalog() {
        intake.install()
        compare(fakeHost.subscriptions.length, 17)

        bridge.host = replacementHost

        tryVerify(function () { return replacementHost.subscriptions.length === 17 })
    }

    function test_ingest_delivery_message_merges_social_comment() {
        const topic = "/lez/account/account-1/comments"
        const payload = {
            kind: "comment",
            version: 1,
            identity: { display_name: "Peer" },
            body: "hello",
            created_at: "2026-07-07T00:00:00Z",
            conversation_id: topic
        }

        verify(intake.ingest(model.deliveryModule, "messageReceived", [
            "hash-1",
            topic,
            JSON.stringify(payload),
            "1000"
        ]))

        compare(model.deliveryModuleEventRows()[0].label, "messageReceived")
        compare(model.socialComments(topic).length, 1)
        compare(model.socialComments(topic)[0].body, "hello")
    }
}
