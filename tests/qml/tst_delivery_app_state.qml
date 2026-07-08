import QtQuick
import QtTest
import "../../qml/state"
import "fixtures"

TestCase {
    id: testRoot

    name: "DeliveryAppState"

    StateGatewayFixture {
        id: gateway
    }

    DeliveryAppState {
        id: state

        gateway: gateway
        effectiveSourceMode: "rest"
        sourceLabel: "Direct REST"
        sourceTarget: "http://delivery"
        sourceTargetKind: "rest_endpoint"
        usesRestEndpoint: true
        supportsMutatingDiagnostics: true
        restEndpoint: "http://delivery"
        moduleName: "delivery_module"
        networkPreset: "logos.test"
        mutatingDiagnosticsEnabled: true
    }

    function init() {
        gateway.reset()
        state.deliveryModuleEvents = []
        state.deliveryModuleEventRevision = 0
        state.deliveryConnectionStatus = ""
        state.deliveryNodeStatus = ""
    }

    function test_message_received_event_returns_delivery_message_effect() {
        const topic = "/lez/account/account-1/comments"
        const payload = {
            kind: "comment",
            version: 1,
            identity: { display_name: "Peer" },
            body: "hello",
            created_at: "2026-07-07T00:00:00Z",
            conversation_id: topic
        }

        const effect = state.applyModuleEvent("messageReceived", [
            "hash-1",
            topic,
            JSON.stringify(payload),
            "1000"
        ])

        verify(effect.changed)
        verify(effect.deliveryMessage)
        compare(effect.deliveryMessage.topic, topic)
        compare(effect.deliveryMessage.messageHash, "hash-1")
        compare(effect.deliveryMessage.payload, JSON.stringify(payload))
        compare(state.moduleEventRows()[0].label, "messageReceived")
        compare(state.moduleEventRows()[0].status, "event")
    }

    function test_connection_event_returns_refresh_effect() {
        const effect = state.applyModuleEvent("connectionStateChanged", [
            JSON.stringify({
                connectionStatus: "connected",
                requestId: "connection-1"
            })
        ])

        verify(effect.changed)
        verify(effect.refreshMessagingConnection)
        verify(!effect.deliveryMessage)
        compare(state.moduleEventSummary(), "connected")
        compare(state.moduleEventRows()[0].label, "connectionStateChanged")
    }
}
