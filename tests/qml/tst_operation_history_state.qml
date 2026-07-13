import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "OperationHistoryState"

    Domains.OperationHistoryState {
        id: history
    }

    function init() {
        history.runtimeOperations = ({})
        history.runtimeOperationEventSeq = ({})
        history.runtimeOperationHistory = []
        history.runtimeOperationsRevision = 0
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
        compare(history.runtimeOperations["op-terminal"].result.cid, "cid-complete")
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

    function test_history_preserves_result_payload_for_status_facades() {
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

        compare(row.result.action, "restart")
        compare(row.result.operation_id, "op-read")
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
        verify(row.acknowledgement.dispatched)
        compare(row.terminalReason, "completion_unobservable")
    }
}
