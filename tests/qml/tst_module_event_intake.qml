import QtQuick
import QtTest
import "../../qml/services"
import "../../qml/state"
import "../../qml/state/modules/ModuleEventEnvelope.js" as ModuleEventEnvelope

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

        function callModuleJson(moduleName, method, argsJson) {
            const args = JSON.parse(String(argsJson || "[]"))
            if (String(method || "") !== "socialCommentRowFromEvent") {
                return JSON.stringify({ ok: true, value: {}, text: "OK", error: "" })
            }
            const event = args[0] || ({})
            return JSON.stringify({
                ok: true,
                value: {
                    key: "event|" + String(event.messageHash || ""),
                    cursor: "",
                    topic: String(event.topic || ""),
                    identity: event.payload && event.payload.identity || {},
                    displayName: "Peer",
                    body: String(event.payload && event.payload.body || ""),
                    createdAt: String(event.payload && event.payload.created_at || ""),
                    conversationId: String(event.payload && event.payload.conversation_id || "")
                },
                text: "OK",
                error: ""
            })
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
        bridge.moduleEventRegistrations = []
        model.deliveryModuleEvents = []
        model.deliveryModuleEventRevision = 0
        model.deliveryConnectionStatus = ""
        model.deliveryNodeStatus = ""
        model.social.socialCommentState = ({})
        model.social.socialCommentRevision = 0
        model.blocksPageRows = []
        model.blocksPageSlotFrom = 0
        model.blocksPageSlotTo = 0
        model.blocksLiveSource = ""
        model.blocksLiveUnknownEvents = 0
        model.blocksLiveCheckedAt = ""
        model.blocksLiveError = ""
    }

    function test_install_subscribes_module_event_catalog() {
        const count = intake.install()

        compare(count, 17)
        compare(intake.subscriptionCatalog().length, 3)
        compare(fakeHost.subscriptions.length, 17)
        compare(fakeHost.subscriptions[0].moduleName, model.deliveryModule)
        compare(fakeHost.subscriptions[0].eventName, "messageSent")
        compare(fakeHost.subscriptions[fakeHost.subscriptions.length - 1].moduleName, model.blockchainModule)
        compare(fakeHost.subscriptions[fakeHost.subscriptions.length - 1].eventName, "newBlock")
    }

    function test_raw_module_event_builds_canonical_envelope() {
        const envelope = ModuleEventEnvelope.fromRaw("delivery_module", "connectionStateChanged", [
            JSON.stringify({ connectionStatus: "connected" })
        ])

        compare(envelope.moduleName, "delivery_module")
        compare(envelope.eventName, "connectionStateChanged")
        compare(envelope.args.length, 1)
        compare(envelope.object.connectionStatus, "connected")
        compare(envelope.payload.connectionStatus, "connected")
    }

    function test_host_swap_resubscribes_catalog() {
        intake.install()
        compare(fakeHost.subscriptions.length, 17)
        const staleCallback = fakeHost.subscriptions[0].callback

        bridge.host = replacementHost

        tryVerify(function () { return replacementHost.subscriptions.length === 17 })
        staleCallback({ requestId: "stale" })
        compare(model.deliveryModuleEvents.length, 0)
    }

    function test_ingest_delivery_message_merges_social_comment() {
        const topic = "/cryptarchia/account/account-1/comments"
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
        compare(model.social.commentsView(topic).rows.length, 1)
        compare(model.social.commentsView(topic).rows[0].body, "hello")
    }

    function test_ingest_blockchain_event_updates_live_rows() {
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30

        verify(intake.ingest(model.blockchainModule, "newBlock", [
            JSON.stringify({ header: { slot: 31, id: "slot-31-event" }, transactions: [] })
        ]))

        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.id, "slot-31-event")
        compare(model.blocksLiveSource, "module_event")
        compare(model.blocksPageSlotTo, 31)
        verify(model.blockchainModuleEventRevision > 0)
    }

    function test_ingest_blockchain_wrapped_event_dedupes_live_rows() {
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30

        const wrapped = JSON.stringify({
            newBlock: {
                block: {
                    header: { slot: 31, id: "slot-31-wrapper" },
                    transactions: []
                }
            }
        })

        verify(intake.ingest(model.blockchainModule, "newBlock", [wrapped]))
        verify(intake.ingest(model.blockchainModule, "newBlock", [wrapped]))

        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.id, "slot-31-wrapper")
        compare(model.blocksPageRows[1].header.id, "slot-30")
        compare(model.blocksLiveSource, "module_event")
        compare(model.blocksPageSlotTo, 31)
    }
}
