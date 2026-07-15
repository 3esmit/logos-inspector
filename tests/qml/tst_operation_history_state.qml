import QtQuick
import QtTest
import "../../qml/state/domains" as Domains
import "../../qml/state/runtime/RuntimeOperationLifecycle.js" as RuntimeOperationLifecycle

TestCase {
    id: testRoot

    name: "OperationHistoryState"

    Domains.OperationHistoryState {
        id: history
    }

    QtObject {
        id: rejectedDispatchRoot

        property string inspectorModule: "logos_inspector"
        property var operationHistory: history
        property QtObject bridge: QtObject {
            property int hostEpoch: 1
            property var host: testRoot
        }

        function requestModuleAsync() {
            return null
        }
    }

    QtObject {
        id: duplicateDispatchRoot

        property string inspectorModule: "logos_inspector"
        property var operationHistory: history
        property QtObject bridge: QtObject {
            property int hostEpoch: 1
            property var host: testRoot
        }

        function requestModuleAsync(moduleName, method, args, label, showResult, callback) {
            const operationId = String(args && args[0] || "")
            const response = testRoot.eventResponse(
                operationId,
                1,
                2,
                false,
                [testRoot.eventValue(operationId, 1)]
            )
            callback(response)
            callback(response)
            return 1
        }
    }

    function init() {
        history.runtimeOperations = ({})
        history.runtimeOperationEventSeq = ({})
        history.runtimeOperationEventFacts = ({})
        history.runtimeOperationHistory = []
        history.runtimeOperationsRevision = 0
        history.runtimeOperationPollGenerations = ({})
        history.runtimeOperationPendingPolls = ({})
        history.runtimeOperationTerminalOrder = []
        history.runtimeOperationCursorOrder = []
    }

    function test_update_operation_and_event_seq_are_facade_owned() {
        verify(history.updateOperation({
            operationId: "op-1",
            domain: "storage",
            method: "storageManifests",
            status: "running"
        }))
        verify(history.setEventSeq("op-1", 4))

        compare(history.runtimeOperations["op-1"].method, "storageManifests")
        compare(history.runtimeOperationEventSeq["op-1"], 4)
        compare(history.runtimeOperationsRevision, 2)
    }

    function test_terminal_operation_dominates_late_nonterminal_projection() {
        verify(history.updateOperation({
            operationId: "op-terminal",
            domain: "storage",
            method: "storageUploadUrl",
            status: "completed",
            result: { cid: "cid-complete" }
        }))

        verify(!history.updateOperation({
            operationId: "op-terminal",
            domain: "storage",
            method: "storageUploadUrl",
            status: "awaiting_external"
        }))

        compare(history.runtimeOperations["op-terminal"].status, "completed")
        compare(history.runtimeOperations["op-terminal"].result, undefined)
        verify(history.runtimeOperations["op-terminal"].resultProjectionOmitted)
        compare(history.runtimeOperationsRevision, 1)
    }

    function test_update_operation_rejects_stale_event_cursor_projection() {
        verify(history.updateOperation({
            operationId: "op-ordered",
            status: "awaiting_external",
            eventCursor: 4
        }))
        verify(!history.updateOperation({
            operationId: "op-ordered",
            status: "running",
            eventCursor: 3
        }))
        verify(!history.updateOperation({
            operationId: "op-ordered",
            status: "canceling"
        }))
        verify(history.updateOperation({
            operationId: "op-ordered",
            status: "canceling",
            eventCursor: 5
        }))

        compare(history.runtimeOperations["op-ordered"].status, "canceling")
        compare(history.runtimeOperations["op-ordered"].eventCursor, 5)
        compare(history.runtimeOperationsRevision, 2)
    }

    function test_event_sequence_never_regresses_under_reversed_callbacks() {
        verify(history.setEventSeq("op-events", 7))
        verify(!history.setEventSeq("op-events", 3))
        verify(!history.setEventSeq("op-events", 7))
        verify(!history.setEventSeq("op-events", 7.5))
        verify(!history.setEventSeq("op-events", Number.MAX_SAFE_INTEGER + 1))
        verify(history.setEventSeq("op-events", 8))

        compare(history.runtimeOperationEventSeq["op-events"], 8)
        compare(history.runtimeOperationsRevision, 2)
    }

    function test_rows_filter_by_domain_and_reverse_newest_first() {
        history.append({
            domain: "storage",
            method: "storageManifests",
            status: "completed",
            label: "List files"
        }, "ok")
        history.append({
            domain: "delivery",
            method: "deliveryStoreQuery",
            status: "completed",
            label: "Messages"
        }, "ok")
        history.append({
            domain: "storage",
            method: "storageExists",
            status: "completed",
            label: "CID"
        }, "ok")

        const rows = history.rows("storage")

        compare(rows.length, 2)
        compare(rows[0].method, "storageExists")
        compare(rows[1].method, "storageManifests")
    }

    function test_read_operations_get_safe_restart_metadata() {
        history.append({
            domain: "storage",
            method: "storageManifests",
            status: "completed",
            sourceMode: "rest",
            endpoint: "http://storage"
        }, "listed")

        const row = history.rows("storage")[0]

        compare(row.operationClass, "read_poll")
        compare(row.restartPolicy, "safe_read_polling")
        compare(row.confirmationRequired, false)
        compare(row.affectedInputs[0].key, "domain")
    }

    function test_mutating_and_wallet_operations_default_to_manual_restart() {
        history.append({
            domain: "storage",
            method: "storageUploadUrl",
            status: "completed",
            path: "/tmp/file.bin"
        }, "uploaded")
        history.append({
            domain: "wallet",
            method: "localWalletInstructionSubmit",
            status: "completed"
        }, "submitted")

        const rows = history.rows("")

        compare(rows[0].operationClass, "signing_submission")
        compare(rows[0].restartPolicy, "manual_required")
        compare(rows[0].confirmationRequired, true)
        compare(rows[1].operationClass, "mutating")
        compare(rows[1].restartPolicy, "manual_required")
        compare(rows[1].confirmationRequired, true)
    }

    function test_explicit_import_metadata_is_preserved() {
        history.append({
            domain: "backup",
            method: "import",
            status: "completed",
            operationClass: "import_apply",
            restartPolicy: "no_restart",
            confirmationRequired: false,
            affectedInputs: [{ key: "section", value: "favorites" }]
        }, "favorites")

        const row = history.rows("backup")[0]

        compare(row.operationClass, "import_apply")
        compare(row.restartPolicy, "no_restart")
        compare(row.confirmationRequired, false)
        compare(row.affectedInputs[0].value, "favorites")
    }

    function test_backend_policy_facts_drive_history_metadata() {
        history.append({
            domain: "storage",
            method: "storageUploadUrl",
            status: "running",
            policyFacts: {
                operationClass: "mutating",
                restartPolicy: "manual_required",
                confirmationRequired: true,
                affectedInputs: [{ key: "path", value: "/tmp/upload.bin" }],
                provenance: ["runtime_operation_policy"]
            }
        }, "upload")

        const row = history.rows("storage")[0]

        compare(row.operationClass, "mutating")
        compare(row.restartPolicy, "manual_required")
        compare(row.confirmationRequired, true)
        compare(row.affectedInputs[0].key, "path")
        compare(row.affectedInputs[0].value, "/tmp/upload.bin")
    }

    function test_history_derives_detail_then_strips_result_payload() {
        history.append({
            domain: "backup",
            method: "settingsBackupImportPolicy",
            status: "completed",
            result: {
                action: "restart",
                operation_id: "op-read"
            }
        }, "restarted")

        const row = history.rows("backup")[0]

        compare(row.detail, "restarted")
        compare(row.result, undefined)
        verify(row.resultProjectionOmitted)
    }

    function test_history_preserves_distinct_conversation_identities() {
        history.append({
            operationId: "operation-1",
            clientRequestId: "client-1",
            bridgeCallbackId: 7,
            moduleSessionId: "session-1",
            moduleRequestId: "request-1",
            externalSessionId: "session-1",
            requestId: "request-1",
            eventCursor: 9,
            status: "dispatched",
            acknowledgement: { dispatched: true },
            terminalReason: "completion_unobservable"
        }, "dispatched")

        const row = history.rows("")[0]
        compare(row.operationId, "operation-1")
        compare(row.clientRequestId, "client-1")
        compare(row.bridgeCallbackId, 7)
        compare(row.moduleSessionId, "session-1")
        compare(row.moduleRequestId, "request-1")
        compare(row.externalSessionId, "session-1")
        compare(row.requestId, "request-1")
        compare(row.eventCursor, 9)
        compare(row.acknowledgement, undefined)
        verify(row.acknowledgementProjectionOmitted)
        compare(row.terminalReason, "completion_unobservable")
    }

    function test_same_cursor_accepts_only_monotonic_coalesced_progress() {
        verify(history.updateOperation({
            operationId: "op-progress",
            backend: "rest",
            status: "running",
            eventCursor: 4,
            progress: 0.2,
            bytesWritten: 20,
            coalescedCount: 1,
            updatedAt: 10
        }))
        verify(history.updateOperation({
            operationId: "op-progress",
            backend: "rest",
            status: "running",
            eventCursor: 4,
            progress: 0.3,
            bytesWritten: 30,
            coalescedCount: 2,
            updatedAt: 11
        }))
        verify(!history.updateOperation({
            operationId: "op-progress",
            backend: "rest",
            status: "running",
            eventCursor: 4,
            progress: 0.25,
            bytesWritten: 29,
            coalescedCount: 3,
            updatedAt: 12
        }))

        compare(history.runtimeOperations["op-progress"].progress, 0.3)
        compare(history.runtimeOperations["op-progress"].bytesWritten, 30)
    }

    function test_event_poll_is_one_in_flight_and_rejects_stale_contexts() {
        const operationId = "op-poll"
        verify(history.updateOperation(runtimeOperation(operationId, 1)))
        const context = pollContext(operationId, "config-1", 3, testRoot)
        const first = history.beginEventPoll(operationId, 1, context)

        verify(first !== null)
        compare(history.pendingEventPollCount, 1)
        compare(history.beginEventPoll(operationId, 1, context), null)

        const configStale = history.finishEventPoll(
            first,
            { ok: false, error: "ignored" },
            pollContext(operationId, "config-2", 3, testRoot)
        )
        verify(configStale.stale)
        verify(!configStale.accepted)
        compare(history.pendingEventPollCount, 0)

        const second = history.beginEventPoll(operationId, 1, context)
        verify(second.generation > first.generation)
        const hostStale = history.finishEventPoll(
            second,
            { ok: false, error: "ignored" },
            pollContext(operationId, "config-1", 4, testRoot)
        )
        verify(hostStale.stale)

        const third = history.beginEventPoll(operationId, 1, context)
        const backendContext = pollContext(operationId, "config-1", 3, testRoot)
        backendContext.backendIdentity = "different-backend"
        const backendStale = history.finishEventPoll(
            third,
            { ok: false, error: "ignored" },
            backendContext
        )
        verify(backendStale.stale)
        compare(history.pendingEventPollCount, 0)
    }

    function test_failed_poll_dispatch_releases_admission_ticket() {
        const operationId = "op-rejected-dispatch"
        verify(history.updateOperation(runtimeOperation(operationId, 0)))

        compare(RuntimeOperationLifecycle.runtimeOperationEvents(
            rejectedDispatchRoot,
            operationId,
            0,
            false,
            null
        ), null)
        compare(history.pendingEventPollCount, 0)
        verify(!history.eventPollPending(operationId))
    }

    function test_duplicate_bridge_callback_delivers_poll_once() {
        const operationId = "op-duplicate-callback"
        verify(history.updateOperation(runtimeOperation(operationId, 0)))
        let callbackCount = 0

        compare(RuntimeOperationLifecycle.runtimeOperationEvents(
            duplicateDispatchRoot,
            operationId,
            0,
            false,
            function () { callbackCount += 1 }
        ), 1)

        compare(callbackCount, 1)
        compare(history.pendingEventPollCount, 0)
        compare(history.runtimeOperationEventSeq[operationId], 1)
    }

    function test_history_gap_resets_to_retained_window() {
        const operationId = "op-gap"
        verify(history.updateOperation(runtimeOperation(operationId, 2)))
        verify(history.setEventSeq(operationId, 2))
        const context = pollContext(operationId, "config", 1, testRoot)
        const ticket = history.beginEventPoll(operationId, 2, context)
        const completion = history.finishEventPoll(ticket, eventResponse(
            operationId,
            5,
            7,
            true,
            [eventValue(operationId, 5), eventValue(operationId, 6)]
        ), context)

        verify(completion.accepted)
        compare(history.runtimeOperationEventSeq[operationId], 6)
        compare(history.eventFacts(operationId).oldestSeq, 5)
        verify(history.eventFacts(operationId).historyTruncated)
        verify(history.eventFacts(operationId).resetRequired)
    }

    function test_future_cursor_reset_can_move_projection_backward() {
        const operationId = "op-future"
        verify(history.updateOperation(runtimeOperation(operationId, 9)))
        verify(history.setEventSeq(operationId, 9))
        const context = pollContext(operationId, "config", 1, testRoot)
        const ticket = history.beginEventPoll(operationId, 9, context)
        const completion = history.finishEventPoll(ticket, eventResponse(
            operationId,
            2,
            5,
            true,
            [
                eventValue(operationId, 2),
                eventValue(operationId, 3),
                eventValue(operationId, 4)
            ]
        ), context)

        verify(completion.accepted)
        compare(history.runtimeOperationEventSeq[operationId], 4)
        compare(history.runtimeOperations[operationId].eventCursor, 4)
    }

    function test_backward_or_malformed_cursor_without_reset_is_rejected() {
        const operationId = "op-invalid-window"
        verify(history.updateOperation(runtimeOperation(operationId, 9)))
        verify(history.setEventSeq(operationId, 9))
        const context = pollContext(operationId, "config", 1, testRoot)
        let ticket = history.beginEventPoll(operationId, 9, context)
        const backward = eventResponse(
            operationId,
            2,
            5,
            false,
            [eventValue(operationId, 4)]
        )
        let completion = history.finishEventPoll(ticket, backward, context)
        verify(completion.invalid)
        verify(!completion.accepted)
        compare(history.runtimeOperationEventSeq[operationId], 9)

        ticket = history.beginEventPoll(operationId, 9, context)
        const malformed = eventResponse(operationId, 10, 11, false, [])
        malformed.value.nextSeq = 10.5
        completion = history.finishEventPoll(ticket, malformed, context)
        verify(completion.invalid)
        compare(history.runtimeOperationEventSeq[operationId], 9)
    }

    function test_modern_window_rejects_sequence_zero_and_method_change() {
        const operationId = "op-strict-window"
        verify(history.updateOperation(runtimeOperation(operationId, 0)))
        const context = pollContext(operationId, "config", 1, testRoot)
        let ticket = history.beginEventPoll(operationId, 0, context)
        const zeroSequence = eventResponse(
            operationId,
            0,
            1,
            false,
            [eventValue(operationId, 0)]
        )
        let completion = history.finishEventPoll(ticket, zeroSequence, context)
        verify(completion.invalid)
        compare(history.runtimeOperationEventSeq[operationId], undefined)

        ticket = history.beginEventPoll(operationId, 0, context)
        const changedMethod = eventResponse(
            operationId,
            1,
            2,
            false,
            [eventValue(operationId, 1)]
        )
        changedMethod.value.operation.method = "storageExists"
        completion = history.finishEventPoll(ticket, changedMethod, context)
        verify(completion.invalid)
        compare(history.runtimeOperationEventSeq[operationId], undefined)
        compare(history.runtimeOperations[operationId].method, "storageManifests")
    }

    function test_poll_rejects_empty_backend_revision_override() {
        const operationId = "op-empty-backend-revision"
        verify(history.updateOperation(runtimeOperation(operationId, 0)))
        const context = pollContext(operationId, "config", 1, testRoot)
        const ticket = history.beginEventPoll(operationId, 0, context)
        const response = eventResponse(
            operationId,
            1,
            2,
            false,
            [eventValue(operationId, 1)]
        )
        response.value.operation.projectionBackendRevision = ""

        const completion = history.finishEventPoll(ticket, response, context)

        verify(completion.invalid)
        verify(!completion.accepted)
        compare(history.runtimeOperationEventSeq[operationId], undefined)
    }

    function test_terminal_projection_maps_and_payloads_stay_bounded() {
        const payload = "x".repeat(history.maxDiagnosticPayloadBytes + 1)
        verify(history.updateOperation({
            operationId: "active-operation",
            backend: "rest",
            status: "running",
            eventCursor: 1,
            context: { payload: payload }
        }))
        verify(history.setEventSeq("active-operation", 1))

        for (let i = 0; i < history.maxTerminalOperations + 12; ++i) {
            const operationId = "terminal-" + String(i)
            verify(history.updateOperation({
                operationId: operationId,
                backend: "rest",
                status: "completed",
                eventCursor: 1,
                result: { payload: payload },
                acknowledgement: { payload: payload },
                context: { payload: payload }
            }))
            history.setEventSeq(operationId, 1)
        }

        compare(Object.keys(history.runtimeOperations).length, history.maxTerminalOperations + 1)
        verify(history.runtimeOperations["active-operation"] !== undefined)
        verify(history.runtimeOperations["terminal-0"] === undefined)
        const latest = history.runtimeOperations[
            "terminal-" + String(history.maxTerminalOperations + 11)]
        compare(latest.result, undefined)
        compare(latest.acknowledgement, undefined)
        compare(latest.context, undefined)
        verify(latest.resultProjectionOmitted)
        verify(latest.acknowledgementProjectionOmitted)
        verify(latest.contextProjectionOmitted)
        verify(Object.keys(history.runtimeOperationEventSeq).length
            <= history.maxTerminalOperations + 1)
    }

    function test_null_terminal_payload_is_not_marked_omitted() {
        verify(history.updateOperation({
            operationId: "terminal-null-payload",
            backend: "rest",
            status: "completed",
            eventCursor: 1,
            result: null,
            acknowledgement: null,
            context: {}
        }))

        const projected = history.runtimeOperations["terminal-null-payload"]
        compare(projected.result, null)
        compare(projected.acknowledgement, null)
        compare(projected.resultProjectionOmitted, undefined)
        compare(projected.acknowledgementProjectionOmitted, undefined)

        history.append({
            operationId: "terminal-null-history",
            status: "completed",
            result: null,
            acknowledgement: null
        }, "done")
        const row = history.rows("")[0]
        compare(row.resultProjectionOmitted, undefined)
        compare(row.acknowledgementProjectionOmitted, undefined)
    }

    function test_diagnostic_text_and_structured_fields_are_byte_bounded() {
        const payload = "é".repeat(history.maxDiagnosticPayloadBytes)
        verify(history.updateOperation({
            operationId: "terminal-bounded-fields",
            backend: "rest",
            status: "failed",
            eventCursor: 1,
            error: payload,
            unknownPayload: { payload: payload }
        }))
        const projected = history.runtimeOperations["terminal-bounded-fields"]
        verify(history.utf8ByteLength(projected.error) <= history.maxDiagnosticTextBytes)
        verify(projected.errorProjectionTruncated)
        verify(projected.errorProjectionOriginalBytes > history.maxDiagnosticTextBytes)
        compare(projected.unknownPayload, undefined)

        history.append({
            operationId: "terminal-bounded-row",
            status: "failed",
            error: payload,
            operationClass: "mutating",
            affectedInputs: [{ key: "payload", value: payload }],
            provenance: [payload],
            externalCorrelation: { payload: payload },
            terminalEventContract: { payload: payload }
        }, "")
        const row = history.rows("")[0]
        verify(history.utf8ByteLength(row.detail) <= history.maxDiagnosticTextBytes)
        verify(row.detailProjectionTruncated)
        verify(row.provenanceProjectionOmitted)
        verify(row.externalCorrelationProjectionOmitted)
        verify(row.terminalEventContractProjectionOmitted)
        verify(row.affectedInputsProjectionOmitted)
    }

    function test_projection_rejects_unknown_status_and_active_overflow() {
        verify(!history.updateOperation({
            operationId: "unknown-status",
            status: "mystery",
            eventCursor: 0
        }))
        for (let i = 0; i < history.maxProjectedActiveOperations; ++i) {
            verify(history.updateOperation({
                operationId: "active-cap-" + String(i),
                domain: "storage",
                backend: "rest",
                method: "storageManifests",
                status: "running",
                eventCursor: 0
            }))
        }
        verify(!history.updateOperation({
            operationId: "active-overflow",
            domain: "storage",
            backend: "rest",
            method: "storageManifests",
            status: "running",
            eventCursor: 0
        }))

        const context = {
            hostEpoch: 1,
            hostIdentity: testRoot,
            configurationIdentity: "config",
            backendIdentity: "",
            backendRevision: ""
        }
        const ticket = history.beginEventPoll("active-overflow", 0, context)
        const completion = history.finishEventPoll(ticket, eventResponse(
            "active-overflow",
            1,
            2,
            false,
            [eventValue("active-overflow", 1)]
        ), context)
        verify(completion.invalid)
        compare(completion.error, "snapshot_rejected")
        compare(history.runtimeOperationEventSeq["active-overflow"], undefined)
        compare(history.eventFacts("active-overflow"), null)
    }

    function test_terminal_pruning_preserves_in_flight_poll_owner() {
        const protectedId = "terminal-protected"
        verify(history.updateOperation({
            operationId: protectedId,
            domain: "storage",
            backend: "rest",
            status: "completed",
            eventCursor: 1,
            updatedAt: 1
        }))
        const context = pollContext(protectedId, "config", 1, testRoot)
        const ticket = history.beginEventPoll(protectedId, 1, context)
        verify(ticket !== null)

        for (let i = 0; i < history.maxTerminalOperations + 8; ++i) {
            verify(history.updateOperation({
                operationId: "terminal-prune-" + String(i),
                domain: "storage",
                backend: "rest",
                status: "completed",
                eventCursor: 1,
                updatedAt: i + 2
            }))
        }

        verify(history.runtimeOperations[protectedId] !== undefined)
        verify(history.eventPollPending(protectedId))
        compare(Object.keys(history.runtimeOperations).length, history.maxTerminalOperations)
        verify(history.abandonEventPoll(ticket))
    }

    function test_diagnostic_payload_limit_counts_utf8_bytes() {
        const payload = "é".repeat(Math.floor(history.maxDiagnosticPayloadBytes / 2) + 1)
        verify(payload.length < history.maxDiagnosticPayloadBytes)

        verify(history.updateOperation({
            operationId: "active-unicode-payload",
            backend: "rest",
            status: "running",
            eventCursor: 1,
            context: { payload: payload }
        }))

        compare(history.runtimeOperations["active-unicode-payload"].context, undefined)
        verify(history.runtimeOperations["active-unicode-payload"].contextProjectionOmitted)
    }

    function test_poll_generation_exhaustion_fails_closed() {
        history.runtimeOperationPollGenerations = {
            exhausted: Number.MAX_SAFE_INTEGER
        }

        compare(history.beginEventPoll("exhausted", 0, {
            hostEpoch: 1,
            configurationIdentity: "config"
        }), null)
        compare(history.pendingEventPollCount, 0)
    }

    function test_purged_terminal_payload_is_not_reported_as_success() {
        let response = RuntimeOperationLifecycle.runtimeOperationResponse(testRoot, {
            operationId: "purged-completion",
            status: "completed",
            resultPurged: true
        })
        verify(!response.ok)
        verify(String(response.error || "").indexOf("bounded history") >= 0)

        response = RuntimeOperationLifecycle.runtimeOperationResponse(testRoot, {
            operationId: "purged-dispatch",
            status: "dispatched",
            acknowledgementPurged: true
        })
        verify(!response.ok)
        verify(String(response.error || "").indexOf("bounded history") >= 0)

        response = RuntimeOperationLifecycle.runtimeOperationResponse(testRoot, {
            operationId: "retained-completion",
            status: "completed",
            result: { cid: "cid-retained" },
            resultPurged: false,
            acknowledgementPurged: true
        })
        verify(response.ok)
        compare(response.value.cid, "cid-retained")

        response = RuntimeOperationLifecycle.runtimeOperationResponse(testRoot, {
            operationId: "retained-dispatch",
            status: "dispatched",
            acknowledgementPurged: false,
            resultPurged: true
        })
        verify(response.ok)
    }

    function runtimeOperation(operationId, cursor) {
        return {
            operationId: String(operationId || ""),
            domain: "storage",
            backend: "rest",
            method: "storageManifests",
            status: "running",
            eventCursor: Number(cursor || 0),
            progress: 0.1,
            bytesWritten: 10,
            updatedAt: 1
        }
    }

    function pollContext(operationId, configurationIdentity, hostEpoch, hostIdentity) {
        const operation = history.runtimeOperations[String(operationId || "")] || null
        return {
            hostEpoch: Number(hostEpoch || 0),
            hostIdentity: hostIdentity || null,
            configurationIdentity: String(configurationIdentity || ""),
            backendIdentity: history.operationBackendIdentity(operation),
            backendRevision: history.operationBackendRevision(operation)
        }
    }

    function eventValue(operationId, seq) {
        return {
            operationId: String(operationId || ""),
            seq: Number(seq),
            eventCursor: Number(seq),
            phase: "running"
        }
    }

    function eventResponse(operationId, oldestSeq, nextSeq, resetRequired, events) {
        const cursor = Number(nextSeq) - 1
        return {
            ok: true,
            value: {
                operation: {
                    operationId: String(operationId || ""),
                    domain: "storage",
                    backend: "rest",
                    method: "storageManifests",
                    status: "running",
                    eventCursor: cursor,
                    progress: 0.5,
                    bytesWritten: 50,
                    updatedAt: 2
                },
                events: Array.isArray(events) ? events : [],
                oldestSeq: Number(oldestSeq),
                nextSeq: Number(nextSeq),
                eventCursor: cursor,
                droppedCount: resetRequired ? 2 : 0,
                coalescedCount: 1,
                retainedCount: Array.isArray(events) ? events.length : 0,
                retainedBytes: 128,
                historyTruncated: resetRequired === true,
                resetRequired: resetRequired === true
            },
            text: "OK",
            error: ""
        }
    }
}
