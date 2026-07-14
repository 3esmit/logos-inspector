import QtQml
import QtTest
import "../../qml/state/chain" as Chain

TestCase {
    id: testRoot

    name: "ChainOperationCoordinator"

    property var chainSourceArgs: ["module"]
    property int chainConfigurationGeneration: 0

    QtObject {
        id: gateway

        property var startRequests: []
        property var startCallbacks: []
        property var statusRequests: []
        property var cancelRequests: []
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

        function runtimeOperationCancel(operationId, showResult, callback) {
            cancelRequests = cancelRequests.concat([String(operationId || "")])
            if (callback) {
                callback({ ok: true, value: { operationId: operationId }, text: "", error: "" })
            }
            return cancelRequests.length
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

    Chain.ChainOperationCoordinator {
        id: coordinator

        gateway: gateway
        sourceArgs: testRoot.chainSourceArgs
        configurationGeneration: testRoot.chainConfigurationGeneration
        maxPollsPerTick: 4
    }

    function init() {
        coordinator.invalidateSource("")
        coordinator.nextGeneration = 0
        coordinator.nextPollToken = 0
        coordinator.maxActive = 16
        coordinator.maxPollsPerTick = 4
        chainSourceArgs = ["module"]
        chainConfigurationGeneration = 0
        gateway.startRequests = []
        gateway.startCallbacks = []
        gateway.statusRequests = []
        gateway.cancelRequests = []
        gateway.history = []
    }

    function command(method, args) {
        return {
            method: method,
            args: args || [],
            label: method
        }
    }

    function operation(index, id, status, cursor, result) {
        const request = gateway.startRequests[index]
        return {
            operationId: id,
            clientRequestId: request.clientRequestId,
            domain: "blockchain",
            backend: contextFor(request).source,
            method: request.method,
            label: request.label,
            status: status,
            eventCursor: cursor,
            context: contextFor(request),
            result: result === undefined ? null : result,
            error: status === "failed" ? "failed" : ""
        }
    }

    function contextFor(request) {
        const args = request.args
        let source = "rpc"
        let endpoint = String(args[0] || "")
        let offset = 1
        if (args[0] === "module" || args[0] === "logoscore_cli") {
            source = String(args[0])
            endpoint = ""
        } else if (args[0] === "rpc") {
            endpoint = String(args[1] || "")
            offset = 2
        }
        const context = { source: source }
        if (endpoint.length) {
            context.endpoint = endpoint
        }
        switch (request.method) {
        case "blockchainBlocks":
            context.slotFrom = Number(args[offset])
            context.slotTo = Number(args[offset + 1])
            context.slotRange = String(context.slotFrom) + ":" + String(context.slotTo)
            if (typeof args[offset + 2] === "number") {
                context.limit = Number(args[offset + 2])
            }
            break
        case "blockchainLiveBlocks":
            context.slotFrom = Number(args[offset])
            context.slotTo = Number(args[offset + 1])
            context.slotRange = String(context.slotFrom) + ":" + String(context.slotTo)
            context.limit = typeof args[offset + 2] === "number"
                ? Number(args[offset + 2]) : 50
            break
        case "blockchainBlock":
            context.blockId = String(args[offset])
            break
        case "blockchainTransaction":
            context.transactionId = String(args[offset])
            break
        }
        return context
    }

    function test_request_freezes_source_context_and_unwraps_completed_result() {
        chainSourceArgs = ["http://blockchain.local"]
        let response = null
        coordinator.start("network.blockchain", command("blockchainNode"), function (value) {
            response = value
            return false
        })

        compare(gateway.startRequests.length, 1)
        compare(gateway.startRequests[0].domain, "blockchain")
        compare(gateway.startRequests[0].args[0], "http://blockchain.local")
        verify(String(gateway.startRequests[0].clientRequestId).length > 0)
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-node", "completed", 1, { node: true })
        })

        verify(response !== null)
        verify(response.ok)
        verify(response.value.node)
        compare(coordinator.pendingCount, 0)
        compare(gateway.history.length, 1)
    }

    function test_distinct_callers_overlap_poll_once_and_complete_in_reverse() {
        const completed = []
        coordinator.start("dashboard.node", command("blockchainNode"), function (response) {
            completed.push(response.value.node)
            return false
        })
        coordinator.start("dashboard.live", command("blockchainLiveBlocks", [1, 2, 5]), function (response) {
            completed.push(response.value.source)
            return false
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-node", "running", 1, null)
        })
        gateway.replyStart(1, {
            ok: true,
            value: operation(1, "op-live", "awaiting_external", 1, null)
        })

        compare(coordinator.poll(), 2)
        compare(coordinator.poll(), 0)
        gateway.replyStatus(1, {
            ok: true,
            value: operation(1, "op-live", "completed", 2, { source: "live", blocks: [] })
        })
        gateway.replyStatus(0, {
            ok: true,
            value: operation(0, "op-node", "completed", 2, { node: "node" })
        })

        compare(completed.join(","), "live,node")
        compare(coordinator.pendingCount, 0)
        compare(gateway.history.length, 2)
    }

    function test_same_caller_is_rejected_without_abandoning_first_operation() {
        let firstCalls = 0
        let rejected = null
        const first = coordinator.start("blocks.page.node", command("blockchainNode"), function () {
            firstCalls += 1
            return false
        })
        const second = coordinator.start("blocks.page.node", command("blockchainNode"), function (response) {
            rejected = response
            return false
        })

        verify(first !== null)
        compare(second, null)
        verify(rejected !== null)
        verify(rejected.busy)
        compare(gateway.startRequests.length, 1)
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-node", "completed", 1, { node: true })
        })
        compare(firstCalls, 1)
    }

    function test_wrong_identity_and_non_completed_terminal_never_project_result() {
        const responses = []
        coordinator.start("detail.block", command("blockchainBlock", ["block-a"]), function (response) {
            responses.push(response)
            return false
        })
        const wrong = operation(0, "op-block", "completed", 1, { header: {} })
        wrong.clientRequestId = "wrong-client"
        gateway.replyStart(0, { ok: true, value: wrong })

        compare(responses.length, 1)
        verify(!responses[0].ok)
        compare(gateway.history.length, 0)

        coordinator.start("detail.transaction", command("blockchainTransaction", ["tx-a"]), function (response) {
            responses.push(response)
            return false
        })
        gateway.replyStart(1, {
            ok: true,
            value: operation(1, "op-tx", "dispatched", 1, { hash: "tx-a" })
        })
        compare(responses.length, 2)
        verify(!responses[1].ok)
        compare(responses[1].terminalStatus, "dispatched")
        compare(gateway.history.length, 1)
    }

    function test_source_invalidation_completes_once_and_rejects_late_reply() {
        let calls = 0
        let invalidated = false
        coordinator.start("network.blockchain", command("blockchainNode"), function (response) {
            calls += 1
            invalidated = response.invalidated === true
            return false
        })

        chainSourceArgs = ["http://new-source.local"]
        compare(calls, 1)
        verify(invalidated)
        compare(coordinator.pendingCount, 0)
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-late", "completed", 1, { node: true })
        })
        compare(calls, 1)
    }

    function test_source_invalidation_rejects_late_poll_reply() {
        let calls = 0
        coordinator.start("network.blockchain", command("blockchainNode"), function (response) {
            calls += 1
            verify(response.invalidated)
            return false
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-node", "running", 1, null)
        })
        compare(coordinator.poll(), 1)

        chainConfigurationGeneration += 1
        compare(calls, 1)
        gateway.replyStatus(0, {
            ok: true,
            value: operation(0, "op-node", "completed", 2, { node: true })
        })
        compare(calls, 1)
        compare(gateway.history.length, 0)
    }

    function test_source_invalidation_cancels_every_known_operation() {
        let invalidated = 0
        coordinator.start("dashboard.node", command("blockchainNode"), function (response) {
            if (response.invalidated) {
                invalidated += 1
            }
            return false
        })
        coordinator.start("dashboard.live", command("blockchainLiveBlocks", [1, 2, 5]), function (response) {
            if (response.invalidated) {
                invalidated += 1
            }
            return false
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-node", "running", 1, null)
        })
        gateway.replyStart(1, {
            ok: true,
            value: operation(1, "op-live", "awaiting_external", 1, null)
        })

        coordinator.invalidateSource("source changed")

        compare(gateway.cancelRequests.join(","), "op-node,op-live")
        compare(invalidated, 2)
        compare(coordinator.pendingCount, 0)
        compare(coordinator.activeTicketCount, 0)
    }

    function test_late_active_start_for_invalidated_ticket_is_canceled() {
        let invalidated = 0
        coordinator.start("network.blockchain", command("blockchainNode"), function (response) {
            if (response.invalidated) {
                invalidated += 1
            }
            return false
        })

        coordinator.invalidateSource("source changed")
        compare(invalidated, 1)
        compare(gateway.cancelRequests.length, 0)
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-late", "awaiting_external", 1, null)
        })

        compare(gateway.cancelRequests.join(","), "op-late")
        compare(invalidated, 1)
        compare(coordinator.pendingCount, 0)
    }

    function test_throwing_callback_does_not_block_other_invalidation_callbacks() {
        let observed = 0
        coordinator.start("throwing", command("blockchainNode"), function () {
            throw new Error("expected callback failure")
        })
        coordinator.start("observed", command("blockchainNode"), function (response) {
            if (response.invalidated) {
                observed += 1
            }
            return false
        })

        coordinator.invalidateSource("source changed")

        compare(observed, 1)
        compare(coordinator.pendingCount, 0)
        compare(coordinator.activeTicketCount, 0)
    }

    function test_poll_ignores_wrong_context_then_accepts_newer_exact_snapshot() {
        let response = null
        coordinator.start("detail.block", command("blockchainBlock", ["block-a"]), function (value) {
            response = value
            return false
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-block", "running", 1, null)
        })
        compare(coordinator.poll(), 1)
        const wrong = operation(0, "op-block", "completed", 2, { header: {} })
        wrong.context.blockId = "block-b"
        gateway.replyStatus(0, { ok: true, value: wrong })

        compare(response, null)
        compare(coordinator.pendingCount, 1)
        compare(coordinator.poll(), 1)
        gateway.replyStatus(1, {
            ok: true,
            value: operation(0, "op-block", "completed", 2, { header: {} })
        })
        verify(response !== null)
        verify(response.ok)
    }

    function test_max_active_rejects_excess_caller_without_abandoning_admitted_operation() {
        coordinator.maxActive = 1
        let admittedCalls = 0
        let rejected = null
        const admitted = coordinator.start("one", command("blockchainNode"), function () {
            admittedCalls += 1
            return false
        })
        const excess = coordinator.start("two", command("blockchainNode"), function (response) {
            rejected = response
            return false
        })

        verify(admitted !== null)
        compare(excess, null)
        verify(rejected !== null)
        verify(!rejected.ok)
        compare(gateway.startRequests.length, 1)
        compare(coordinator.pendingCount, 1)
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-one", "completed", 1, { node: true })
        })
        compare(admittedCalls, 1)
        compare(coordinator.pendingCount, 0)
    }

    function test_poll_ignores_mismatched_operation_id_until_valid_snapshot() {
        let response = null
        coordinator.start("network.blockchain", command("blockchainNode"), function (value) {
            response = value
            return false
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-node", "running", 1, null)
        })
        compare(coordinator.poll(), 1)
        gateway.replyStatus(0, {
            ok: true,
            value: operation(0, "op-foreign", "completed", 2, { node: "foreign" })
        })

        compare(response, null)
        compare(coordinator.pendingCount, 1)
        compare(coordinator.poll(), 1)
        gateway.replyStatus(1, {
            ok: true,
            value: operation(0, "op-node", "completed", 2, { node: "accepted" })
        })
        verify(response !== null)
        verify(response.ok)
        compare(response.value.node, "accepted")
    }

    function test_poll_ignores_duplicate_and_stale_cursors_until_newer_snapshot() {
        let response = null
        coordinator.start("network.blockchain", command("blockchainNode"), function (value) {
            response = value
            return false
        })
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-node", "running", 2, null)
        })

        compare(coordinator.poll(), 1)
        gateway.replyStatus(0, {
            ok: true,
            value: operation(0, "op-node", "completed", 2, { node: "duplicate" })
        })
        compare(response, null)
        compare(coordinator.pendingCount, 1)

        compare(coordinator.poll(), 1)
        gateway.replyStatus(1, {
            ok: true,
            value: operation(0, "op-node", "completed", 1, { node: "stale" })
        })
        compare(response, null)
        compare(coordinator.pendingCount, 1)

        compare(coordinator.poll(), 1)
        gateway.replyStatus(2, {
            ok: true,
            value: operation(0, "op-node", "completed", 3, { node: "newer" })
        })
        verify(response !== null)
        verify(response.ok)
        compare(response.value.node, "newer")
    }

    function test_poll_cap_round_robins_active_operations() {
        coordinator.maxPollsPerTick = 1
        coordinator.start("one", command("blockchainNode"), function () { return false })
        coordinator.start("two", command("blockchainBlocks", [1, 2]), function () { return false })
        gateway.replyStart(0, {
            ok: true,
            value: operation(0, "op-one", "running", 1, null)
        })
        gateway.replyStart(1, {
            ok: true,
            value: operation(1, "op-two", "running", 1, null)
        })

        compare(coordinator.poll(), 1)
        compare(coordinator.poll(), 1)
        compare(gateway.statusRequests.length, 2)
        verify(gateway.statusRequests[0].operationId !== gateway.statusRequests[1].operationId)
    }
}
