import QtQml
import QtTest
import "../../qml/state/social" as Social

TestCase {
    id: testRoot

    name: "DeliveryStoreQueryCoordinator"

    QtObject {
        id: gateway

        property var startRequests: []
        property var startCallbacks: []
        property var statusRequests: []
        property var history: []

        function startRuntimeOperation(request, showResult, callback) {
            startRequests = startRequests.concat([request])
            startCallbacks = startCallbacks.concat([callback])
            return startRequests.length
        }

        function runtimeOperationStatus(operationId, showResult, callback) {
            statusRequests = statusRequests.concat([{
                operationId: String(operationId || ""),
                callback: callback
            }])
            return statusRequests.length
        }

        function appendOperationHistory(operation, detail) {
            history = history.concat([{ operation: operation, detail: detail }])
        }

        function replyStart(index, response) {
            startCallbacks[index](response)
        }

        function replyStatus(index, response) {
            statusRequests[index].callback(response)
        }
    }

    Social.DeliveryStoreQueryCoordinator {
        id: coordinator

        gateway: gateway
        adapterInitialization: ({
            source_mode: "rest",
            inputs: { endpoint: "http://delivery" }
        })
        maxPollsPerTick: 4
    }

    function init() {
        coordinator.invalidateSource()
        coordinator.nextGeneration = 0
        coordinator.nextPollToken = 0
        coordinator.maxPendingQueries = 16
        gateway.startRequests = []
        gateway.startCallbacks = []
        gateway.statusRequests = []
        gateway.history = []
    }

    function scope(key, topic) {
        return {
            callerKey: key,
            family: "comments",
            accountId: "",
            topic: topic
        }
    }

    function operation(id, status, cursor, result) {
        return {
            operationId: id,
            domain: "delivery",
            method: "deliveryStoreQuery",
            label: "Store",
            status: status,
            eventCursor: cursor,
            result: result === undefined ? null : result,
            error: status === "failed" ? "failed" : ""
        }
    }

    function test_immediate_terminal_unwraps_result_once() {
        let calls = 0
        let value = null
        coordinator.start(scope("comments|a", "/topic/a"), "", 20, "Comments", function (response) {
            calls += 1
            value = response.value
            return false
        })

        compare(gateway.startRequests.length, 1)
        compare(gateway.startRequests[0].payload.content_topics, "/topic/a")
        gateway.replyStart(0, {
            ok: true,
            value: operation("op-a", "completed", 1, { marker: "result" })
        })

        compare(calls, 1)
        compare(value.marker, "result")
        compare(coordinator.pendingCount, 0)
        compare(gateway.history.length, 1)
    }

    function test_dispatched_or_missing_result_is_not_query_success() {
        const responses = []
        coordinator.start(scope("comments|a", "/topic/a"), "", 20, "A", function (response) {
            responses.push(response)
            return false
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation("op-a", "dispatched", 1, null)
        })
        verify(!responses[0].ok)
        compare(responses[0].value, null)

        coordinator.start(scope("comments|b", "/topic/b"), "", 20, "B", function (response) {
            responses.push(response)
            return false
        })
        gateway.replyStart(1, {
            ok: true,
            value: operation("op-b", "completed", 1, null)
        })
        verify(!responses[1].ok)
        compare(responses[1].value, null)
    }

    function test_distinct_topics_overlap_and_complete_in_reverse() {
        const completed = []
        coordinator.start(scope("comments|a", "/topic/a"), "", 20, "A", function (response) {
            completed.push(response.value.topic)
            return false
        })
        coordinator.start(scope("comments|b", "/topic/b"), "", 20, "B", function (response) {
            completed.push(response.value.topic)
            return false
        })
        gateway.replyStart(0, { ok: true, value: operation("op-a", "running", 1, null) })
        gateway.replyStart(1, { ok: true, value: operation("op-b", "running", 1, null) })

        compare(coordinator.pendingCount, 2)
        compare(coordinator.poll(), 2)
        gateway.replyStatus(1, {
            ok: true,
            value: operation("op-b", "completed", 2, { topic: "b" })
        })
        gateway.replyStatus(0, {
            ok: true,
            value: operation("op-a", "completed", 2, { topic: "a" })
        })

        compare(completed.join(","), "b,a")
        compare(coordinator.pendingCount, 0)
    }

    function test_same_caller_pending_start_is_rejected_and_bounded() {
        let oldCalls = 0
        let newCalls = 0
        const oldTicket = coordinator.start(
            scope("comments|a", "/topic/a"), "old", 20, "Old", function () {
                oldCalls += 1
                return false
            }
        )
        const newTicket = coordinator.start(
            scope("comments|a", "/topic/a"), "new", 20, "New", function () {
                newCalls += 1
                return false
            }
        )

        verify(coordinator.isCurrent(oldTicket))
        compare(newTicket, null)
        compare(gateway.startRequests.length, 1)
        compare(newCalls, 1)
        gateway.replyStart(0, {
            ok: true,
            value: operation("op-old", "completed", 1, { marker: "old" })
        })

        compare(oldCalls, 1)
        compare(newCalls, 1)
    }

    function test_poll_is_single_flight_and_failure_can_retry() {
        let calls = 0
        coordinator.start(scope("comments|a", "/topic/a"), "", 20, "A", function () {
            calls += 1
            return false
        })
        gateway.replyStart(0, { ok: true, value: operation("op-a", "running", 1, null) })

        compare(coordinator.poll(), 1)
        compare(coordinator.poll(), 0)
        gateway.replyStatus(0, {
            ok: true,
            value: operation("wrong-op", "completed", 2, { marker: "wrong" })
        })
        compare(calls, 0)

        compare(coordinator.poll(), 1)
        gateway.replyStatus(1, { ok: false, error: "transport" })
        compare(coordinator.poll(), 1)
        gateway.replyStatus(2, {
            ok: true,
            value: operation("op-a", "completed", 2, { marker: "done" })
        })
        compare(calls, 1)
    }

    function test_source_invalidation_rejects_late_start() {
        let calls = 0
        const ticket = coordinator.start(
            scope("comments|a", "/topic/a"), "", 20, "A", function () {
                calls += 1
                return false
            }
        )
        coordinator.invalidateSource()
        verify(!coordinator.isCurrent(ticket))

        gateway.replyStart(0, {
            ok: true,
            value: operation("op-a", "completed", 1, { marker: "late" })
        })
        compare(calls, 0)
        compare(coordinator.pendingCount, 0)
    }

    function test_start_rejects_missing_operation_identity() {
        let response = null
        coordinator.start(scope("comments|a", "/topic/a"), "", 20, "A", function (value) {
            response = value
            return false
        })
        const invalid = operation("op-a", "completed", 1, { marker: "invalid" })
        delete invalid.domain
        gateway.replyStart(0, { ok: true, value: invalid })

        verify(response !== null)
        verify(!response.ok)
        compare(coordinator.pendingCount, 0)
        compare(gateway.history.length, 0)
    }

    function test_poll_rejects_missing_operation_identity() {
        let calls = 0
        coordinator.start(scope("comments|a", "/topic/a"), "", 20, "A", function () {
            calls += 1
            return false
        })
        gateway.replyStart(0, { ok: true, value: operation("op-a", "running", 1, null) })
        compare(coordinator.poll(), 1)
        const invalid = operation("op-a", "completed", 2, { marker: "invalid" })
        delete invalid.method
        gateway.replyStatus(0, { ok: true, value: invalid })

        compare(calls, 0)
        compare(coordinator.pendingCount, 1)
        compare(coordinator.poll(), 1)
    }

    function test_retained_ticket_protects_downstream_async_work() {
        let retained = null
        coordinator.start(scope("shared-idl|a", "/topic/a"), "", 20, "IDL", function (response, ticket) {
            retained = ticket
            return true
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation("op-a", "completed", 1, { messages: [] })
        })

        verify(coordinator.isCurrent(retained))
        verify(coordinator.release(retained))
        verify(!coordinator.isCurrent(retained))
    }

    function test_retained_downstream_work_counts_toward_bound() {
        coordinator.maxPendingQueries = 1
        coordinator.start(scope("shared-idl|a", "/topic/a"), "", 20, "IDL", function () {
            return true
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation("op-a", "completed", 1, { messages: [] })
        })

        let rejected = null
        const ticket = coordinator.start(scope("shared-idl|b", "/topic/b"), "", 20, "IDL", function (response) {
            rejected = response
            return false
        })
        compare(ticket, null)
        verify(rejected !== null)
        verify(!rejected.ok)
        compare(gateway.startRequests.length, 1)
    }
}
