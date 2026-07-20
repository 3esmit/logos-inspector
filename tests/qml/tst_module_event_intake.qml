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
        property var calls: []

        function onModuleEvent(moduleName, eventName, callback) {
            subscriptions = subscriptions.concat([{
                moduleName: String(moduleName || ""),
                eventName: String(eventName || ""),
                callback: callback
            }])
        }

        function callModuleJson(moduleName, method, argsJson) {
            calls = calls.concat([{
                moduleName: String(moduleName || ""),
                method: String(method || ""),
                args: JSON.parse(String(argsJson || "[]"))
            }])
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
        id: basecampHost

        property var subscriptions: []
        property var calls: []
        property var asyncResponses: ({})
        property int nextAsyncToken: 1
        property string asyncBridgeSchemaResponse: "logos-inspector-async-bridge/v1"
        property bool logosInspectorOwnsRuntimeModuleEvents: true

        function onModuleEvent(moduleName, eventName) {
            subscriptions = subscriptions.concat([{
                moduleName: String(moduleName || ""),
                eventName: String(eventName || "")
            }])
            return true
        }

        function callModule(moduleName, method, args) {
            if (String(moduleName || "") === "logos_inspector"
                    && String(method || "") === "asyncBridgeSchema") {
                return JSON.stringify(asyncBridgeSchemaResponse)
            }
            calls = calls.concat([{
                moduleName: String(moduleName || ""),
                method: String(method || ""),
                args: args || []
            }])
            return JSON.stringify({ ok: true, value: {}, text: "OK", error: "" })
        }

        function callModuleAsync(moduleName, method, args, callback) {
            calls = calls.concat([{
                moduleName: String(moduleName || ""),
                method: String(method || ""),
                args: args || []
            }])
            if (method === "callAsync") {
                const token = "event-token-" + nextAsyncToken
                nextAsyncToken += 1
                const inspectorMethod = String(args[1] || "")
                const inspectorArgs = JSON.parse(String(args[2] || "[]"))
                calls = calls.concat([{
                    moduleName: "logos_inspector",
                    method: inspectorMethod,
                    args: inspectorArgs
                }])
                const responses = Object.assign({}, asyncResponses)
                responses[token] = JSON.stringify({
                    ok: true,
                    value: {},
                    text: "OK",
                    error: ""
                })
                asyncResponses = responses
                callback(JSON.stringify(JSON.stringify({
                    ok: true,
                    value: {
                        schema: "logos-inspector-async-bridge/v1",
                        correlationId: String(args[0] || ""),
                        token: token
                    },
                    text: "",
                    error: ""
                })))
                return
            }
            if (method === "pollAsync") {
                const token = String(args[0] || "")
                callback(JSON.stringify(JSON.stringify({
                    ok: true,
                    value: {
                        schema: "logos-inspector-async-bridge/v1",
                        token: token,
                        status: "ready",
                        responseJson: asyncResponses[token]
                    },
                    text: "",
                    error: ""
                })))
                return
            }
            if (method === "releaseAsync") {
                const responses = Object.assign({}, asyncResponses)
                delete responses[String(args[0] || "")]
                asyncResponses = responses
            }
            callback(JSON.stringify(JSON.stringify({
                ok: true,
                value: {},
                text: "",
                error: ""
            })))
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

    QtObject {
        id: directEventOwnerHost

        signal moduleEventReceived(string moduleName, string eventName, var args)

        property var subscriptions: []
        property var calls: []
        property bool ownerResponse: true

        function onModuleEvent(moduleName, eventName) {
            subscriptions = subscriptions.concat([{
                moduleName: String(moduleName || ""),
                eventName: String(eventName || "")
            }])
            return true
        }

        function callModule(moduleName, method, args) {
            calls = calls.concat([{
                moduleName: String(moduleName || ""),
                method: String(method || ""),
                args: args || []
            }])
            if (String(moduleName || "") === "logos_inspector"
                    && String(method || "") === "logosInspectorOwnsRuntimeModuleEvents") {
                return JSON.stringify(ownerResponse)
            }
            return JSON.stringify({ error: "Invalid response" })
        }
    }

    QtObject {
        id: nativeWatcherHost

        signal moduleEventJson(string moduleName, string eventName, string argsJson)

        property var calls: []
        property bool watcherStarts: true

        function startModuleWatcher() {
            return watcherStarts
        }

        function backendOwnsRuntimeModuleEvents() {
            return true
        }

        function callModuleJson(moduleName, method, argsJson) {
            calls = calls.concat([{
                moduleName: String(moduleName || ""),
                method: String(method || ""),
                args: JSON.parse(String(argsJson || "[]"))
            }])
            return JSON.stringify({ ok: true, value: {}, text: "OK", error: "" })
        }
    }

    QtObject {
        id: failedWatcherHost

        signal moduleEventJson(string moduleName, string eventName, string argsJson)

        function startModuleWatcher() {
            return false
        }

        function callModuleJson(moduleName, method, argsJson) {
            return JSON.stringify({ ok: true, value: {}, text: "OK", error: "" })
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
        fakeHost.calls = []
        basecampHost.subscriptions = []
        basecampHost.calls = []
        basecampHost.asyncResponses = ({})
        basecampHost.nextAsyncToken = 1
        basecampHost.asyncBridgeSchemaResponse = "logos-inspector-async-bridge/v1"
        basecampHost.logosInspectorOwnsRuntimeModuleEvents = true
        replacementHost.subscriptions = []
        directEventOwnerHost.subscriptions = []
        directEventOwnerHost.calls = []
        directEventOwnerHost.ownerResponse = true
        nativeWatcherHost.calls = []
        nativeWatcherHost.watcherStarts = true
        bridge.moduleEventSubscriptions = ({})
        bridge.moduleEventRegistrations = []
        model.deliveryModuleEvents = []
        model.deliveryModuleEventRevision = 0
        model.deliveryConnectionStatus = ""
        model.deliveryNodeStatus = ""
        model.messagingSourceMode = "logoscore_cli"
        model.metrics.resetDeliveryModuleEventTelemetry("unknown", "")
        model.social.socialCommentState = ({})
        model.social.socialCommentRevision = 0
        model.nodeUrl = "http://127.0.0.1:8080/"
        model.networkConnectorConfig = ({
            scopes: {
                l1: {
                    connector_id: "direct_l1_rpc",
                    provenance: "test"
                },
                delivery: {
                    connector_id: "logoscore_cli_delivery_module",
                    provenance: "test"
                }
            }
        })
        model.blockchainSourceMode = "rpc"
        wait(0)
        model.blocksPageRows = []
        model.blocksPageSlotFrom = 0
        model.blocksPageSlotTo = 0
        model.blocksLiveSource = ""
        model.blocksLiveUnknownEvents = 0
        model.blocksLiveCheckedAt = ""
        model.blocksLiveError = ""
        model.blockchainModuleEventRevision = 0
        model.blockchainLastEventText = ""
        model.walletPublicKeyProbe = ""
    }

    function useHostBlockchainModule() {
        model.networkConnectorConfig = ({
            scopes: {
                l1: {
                    connector_id: "blockchain_module",
                    provenance: "test"
                }
            }
        })
        model.blockchainSourceMode = "module"
        wait(0)
    }

    function runtimeModuleEventCalls(calls) {
        const rows = Array.isArray(calls) ? calls : []
        let count = 0
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            if (row.method === "runtimeOperationModuleEvent"
                    || (row.method === "call"
                        && row.args
                        && row.args[0] === "runtimeOperationModuleEvent")) {
                count += 1
            }
        }
        return count
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

    function test_host_swap_clears_previous_delivery_event_telemetry() {
        bridge.host = nativeWatcherHost
        verify(intake.ingest(model.deliveryModule, "eventStreamReady", [{
            source: "logoscore.watch",
            status: "ready"
        }]))
        verify(intake.ingest(model.deliveryModule, "messageSent", [
            "request-old-host", "hash-old-host", "1000"
        ]))
        compare(model.metrics.deliveryModuleEventStreamStatus, "ready")
        compare(model.metrics.deliveryModuleEventTimestamps[
            "messaging.message_sent_events_recent"].length, 1)

        bridge.host = replacementHost

        tryCompare(model.metrics, "deliveryModuleEventStreamStatus", "unknown")
        compare(model.metrics.deliveryModuleEventTimestamps[
            "messaging.message_sent_events_recent"].length, 0)
        compare(model.metrics.dashboardMetricValue(
            "messaging.message_sent_events_recent"), null)
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

    function test_standalone_event_forwards_to_runtime_reducer() {
        verify(intake.forwardsRuntimeOperationEvents())
        verify(intake.ingest(model.deliveryModule, "messageSent", ["request-1", "hash-1"]))

        tryVerify(function () {
            return runtimeModuleEventCalls(fakeHost.calls) === 1
        })
    }

    function test_delivery_watcher_events_drive_independent_metrics_once() {
        const sentKey = "messaging.message_sent_events_recent"
        const propagatedKey = "messaging.message_propagated_events_recent"

        verify(intake.ingest(model.deliveryModule, "eventStreamReady", [{
            source: "logoscore.watch",
            status: "ready"
        }]))
        compare(model.metrics.dashboardMetricValue(sentKey), null)
        const now = model.metrics.deliveryModuleEventNowMs
        model.metrics.deliveryModuleEventCoverageStartedAtMs =
            model.metrics.emptyDeliveryModuleEventCoverage(
                now - model.messagingRollingWindow * 1000 - 1)
        model.metrics.deliveryModuleEventRevision += 1
        compare(model.metrics.dashboardMetricValue(sentKey), 0)
        compare(model.metrics.dashboardMetricValue(propagatedKey), 0)

        verify(intake.ingest(model.deliveryModule, "messageSent", [
            "request-metrics", "hash-metrics", "1000"
        ]))
        compare(model.metrics.dashboardMetricValue(sentKey), 1)
        compare(model.metrics.dashboardMetricValue(propagatedKey), 0)

        verify(intake.ingest(model.deliveryModule, "messageReceived", [
            "received-hash", "/test/topic", "payload", "1001"
        ]))
        compare(model.metrics.dashboardMetricValue(sentKey), 1)
        compare(model.metrics.dashboardMetricValue(propagatedKey), 0)

        verify(intake.ingest(model.deliveryModule, "messagePropagated", [
            "request-metrics", "hash-metrics", "1002"
        ]))
        compare(model.metrics.dashboardMetricValue(sentKey), 1)
        compare(model.metrics.dashboardMetricValue(propagatedKey), 1)

        verify(intake.ingest(model.deliveryModule, "eventStreamUnavailable", [{
            source: "logoscore.watch",
            status: "unavailable",
            reason: "watch exited"
        }]))
        compare(model.metrics.dashboardMetricValue(sentKey), null)
        compare(model.metrics.dashboardMetricValue(propagatedKey), null)
    }

    function test_standalone_watcher_start_failure_is_explicit() {
        bridge.host = failedWatcherHost

        tryCompare(model.metrics, "deliveryModuleEventStreamStatus", "unavailable")
        verify(model.metrics.deliveryModuleEventStreamReason.indexOf("failed to start") >= 0)
    }

    function test_basecamp_subscriptions_make_zero_traffic_coverage_explicit() {
        bridge.host = basecampHost

        tryCompare(model.metrics, "deliveryModuleEventStreamStatus", "ready")
        compare(basecampHost.subscriptions.length, 17)
        compare(model.metrics.dashboardMetricValue(
            "messaging.message_sent_events_recent"), null)
        verify(model.metrics.deliveryModuleEventMetricUnavailableReason(
            "messaging.message_sent_events_recent").indexOf("continuous") >= 0)
    }

    function test_basecamp_event_projects_without_second_runtime_ingress() {
        bridge.host = basecampHost
        verify(!intake.forwardsRuntimeOperationEvents())
        basecampHost.calls = []

        verify(intake.ingest(model.deliveryModule, "messageSent", ["request-2", "hash-2"]))

        compare(model.deliveryModuleEventRows()[0].label, "messageSent")
        compare(runtimeModuleEventCalls(basecampHost.calls), 0)
    }

    function test_basecamp_without_native_event_owner_keeps_legacy_ingress() {
        bridge.host = basecampHost
        basecampHost.logosInspectorOwnsRuntimeModuleEvents = false
        verify(intake.forwardsRuntimeOperationEvents())
        basecampHost.calls = []

        verify(intake.ingest(model.deliveryModule, "messageSent", ["request-3", "hash-3"]))

        compare(model.deliveryModuleEventRows()[0].label, "messageSent")
        compare(runtimeModuleEventCalls(basecampHost.calls), 1)
    }

    function test_direct_native_event_owner_keeps_projection_subscription() {
        bridge.host = directEventOwnerHost

        compare(intake.install(), 17)
        compare(directEventOwnerHost.subscriptions.length, 17)
        verify(!intake.forwardsRuntimeOperationEvents())
        directEventOwnerHost.calls = []

        directEventOwnerHost.moduleEventReceived(
            model.deliveryModule,
            "messageSent",
            ["request-4", "hash-4"]
        )

        compare(model.deliveryModuleEventRows()[0].label, "messageSent")
        compare(runtimeModuleEventCalls(directEventOwnerHost.calls), 0)
    }

    function test_native_watcher_event_refreshes_local_nodes_without_second_ingress() {
        bridge.host = nativeWatcherHost
        verify(!intake.forwardsRuntimeOperationEvents())

        nativeWatcherHost.moduleEventJson(
            model.deliveryModule,
            "nodeStarted",
            JSON.stringify([{ success: true, simulated: true }])
        )

        tryVerify(function () {
            for (let i = 0; i < nativeWatcherHost.calls.length; ++i) {
                if (nativeWatcherHost.calls[i].method === "localNodesStatus") {
                    return true
                }
            }
            return false
        })
        compare(model.deliveryModuleEventRows()[0].label, "nodeStarted")
        compare(runtimeModuleEventCalls(nativeWatcherHost.calls), 0)
    }

    function test_native_watcher_refreshes_all_local_node_lifecycle_routes() {
        bridge.host = nativeWatcherHost
        nativeWatcherHost.calls = []

        nativeWatcherHost.moduleEventJson(
            model.blockchainModule,
            "nodeStarted",
            JSON.stringify([{ node: "bedrock", success: true, simulated: true }])
        )
        tryVerify(function () {
            return nativeWatcherHost.calls.some(function(call) {
                return call.method === "localNodesStatus"
            })
        })

        nativeWatcherHost.calls = []
        nativeWatcherHost.moduleEventJson(
            "indexer_service",
            "nodeStopped",
            JSON.stringify([{ node: "indexer", success: true, simulated: true }])
        )
        tryVerify(function () {
            return nativeWatcherHost.calls.some(function(call) {
                return call.method === "localNodesStatus"
            })
        })

        nativeWatcherHost.calls = []
        nativeWatcherHost.moduleEventJson(
            "sequencer_service",
            "nodeUnavailable",
            JSON.stringify([{ node: "sequencer", simulated: true }])
        )
        tryVerify(function () {
            return nativeWatcherHost.calls.some(function(call) {
                return call.method === "localNodesStatus"
            })
        })
        compare(runtimeModuleEventCalls(nativeWatcherHost.calls), 0)
    }

    function test_ingest_blockchain_event_updates_live_rows() {
        useHostBlockchainModule()
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
        useHostBlockchainModule()
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

    function test_direct_rpc_rejects_untagged_blockchain_event() {
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30-rpc" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30
        model.nodeUrl = "http://127.0.0.1:18080/"
        wait(0)

        compare(model.blocksPageRows.length, 0)
        compare(model.blocksPageSlotFrom, 0)
        compare(model.blocksPageSlotTo, 0)
        model.walletPublicKeyProbe = "slot-31-untagged"
        fakeHost.calls = []

        verify(!intake.ingest(model.blockchainModule, "newBlock", [
            JSON.stringify({ header: { slot: 31, id: "slot-31-untagged" }, transactions: [] })
        ]))
        wait(0)

        compare(model.blocksPageRows.length, 0)
        compare(model.blocksPageSlotFrom, 0)
        compare(model.blocksPageSlotTo, 0)
        compare(model.blocksLiveSource, "")
        compare(model.blockchainModuleEventRevision, 0)
        compare(model.blockchainLastEventText, "")
        compare(fakeHost.calls.length, 0)
    }

    function test_logoscore_cli_rejects_untagged_blockchain_event() {
        model.networkConnectorConfig = ({
            scopes: {
                l1: {
                    connector_id: "logoscore_cli_blockchain_module",
                    provenance: "test"
                }
            }
        })
        model.blockchainSourceMode = "logoscore_cli"
        wait(0)

        compare(model.sourceRouting.coreSourceView("blockchain").resolvedMode,
                "logoscore_cli")
        verify(!intake.ingest(model.blockchainModule, "newBlock", [
            JSON.stringify({ header: { slot: 31, id: "slot-31-cli" }, transactions: [] })
        ]))
        compare(model.blocksPageRows.length, 0)
        compare(model.blocksPageSlotFrom, 0)
        compare(model.blocksPageSlotTo, 0)
        compare(model.blocksLiveSource, "")
        compare(model.blockchainModuleEventRevision, 0)
        compare(model.blockchainLastEventText, "")
    }

    function test_logoscore_cli_accepts_only_typed_blockchain_watch_event() {
        model.networkConnectorConfig = ({
            scopes: {
                l1: {
                    connector_id: "logoscore_cli_blockchain_module",
                    provenance: "test"
                }
            }
        })
        model.blockchainSourceMode = "logoscore_cli"
        wait(0)
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30-cli" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30

        verify(!intake.ingest(model.blockchainModule, "newBlock", [{
            source: "logoscore_cli_watch",
            protocol: "logoscore.watch",
            version: 2,
            payload: { header: { slot: 31, id: "slot-31-wrong-version" }, transactions: [] }
        }]))
        compare(model.blocksPageRows.length, 1)
        compare(model.blockchainModuleEventRevision, 0)

        verify(intake.ingest(model.blockchainModule, "newBlock", [{
            source: "logoscore_cli_watch",
            protocol: "logoscore.watch",
            version: 1,
            timestamp: 1784558400000,
            payload: { header: { slot: 31, id: "slot-31-cli-watch" }, transactions: [] }
        }]))

        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.id, "slot-31-cli-watch")
        compare(model.blocksPageSlotTo, 31)
        compare(model.blocksLiveSource, "logoscore_cli_watch")
        verify(model.blockchainModuleEventRevision > 0)
    }
}
