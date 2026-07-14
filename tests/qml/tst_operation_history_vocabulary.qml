import QtQuick
import QtTest
import "../../qml/state/OperationHistoryVocabulary.js" as OperationHistoryVocabulary

TestCase {
    name: "OperationHistoryVocabulary"

    function test_runtime_status_text_and_tone_are_centralized() {
        compare(OperationHistoryVocabulary.runtimeStatusText({
            status: "running",
            label: "Upload file"
        }, "Runtime operation"), "Upload file")
        compare(OperationHistoryVocabulary.runtimeTone({ status: "running" }), "warning")

        compare(OperationHistoryVocabulary.runtimeStatusText({ status: "completed" }), "Complete")
        compare(OperationHistoryVocabulary.runtimeTone({ status: "completed" }), "success")

        compare(OperationHistoryVocabulary.runtimeStatusText({ status: "awaiting_external" }), "Waiting for completion")
        compare(OperationHistoryVocabulary.runtimeTone({ status: "awaiting_external" }), "warning")
        verify(OperationHistoryVocabulary.isRuntimeActiveStatus("awaiting_external"))
        verify(!OperationHistoryVocabulary.isRuntimeTerminalStatus("awaiting_external"))

        compare(OperationHistoryVocabulary.runtimeStatusText({ status: "dispatched" }), "Dispatched")
        compare(OperationHistoryVocabulary.runtimeTone({ status: "dispatched" }), "warning")
        verify(OperationHistoryVocabulary.isRuntimeTerminalStatus("dispatched"))
        verify(OperationHistoryVocabulary.isRuntimeSuccessfulTerminalStatus("dispatched"))

        compare(OperationHistoryVocabulary.runtimeStatusText({ status: "failed" }), "Failed")
        compare(OperationHistoryVocabulary.runtimeTone({ status: "failed" }), "error")

        compare(OperationHistoryVocabulary.runtimeStatusText(null), "Idle")
        compare(OperationHistoryVocabulary.runtimeTone(null), "neutral")
    }

    function test_runtime_snapshot_freshness_uses_cursor_with_legacy_compatibility() {
        verify(OperationHistoryVocabulary.runtimeSnapshotIsNewer(null, {
            operationId: "operation-1",
            status: "running"
        }))
        verify(OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "running"
        }, {
            operationId: "operation-1",
            status: "awaiting_external"
        }))
        verify(OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "running"
        }, {
            operationId: "operation-1",
            status: "awaiting_external",
            eventCursor: 1
        }))
        verify(!OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "awaiting_external",
            eventCursor: 2
        }, {
            operationId: "operation-1",
            status: "canceling"
        }))
        verify(!OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "awaiting_external",
            eventCursor: 2
        }, {
            operationId: "operation-1",
            status: "canceling",
            eventCursor: 2
        }))
        verify(!OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "awaiting_external",
            eventCursor: 2
        }, {
            operationId: "operation-1",
            status: "running",
            eventCursor: 1
        }))
        verify(OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "awaiting_external",
            eventCursor: 2
        }, {
            operationId: "operation-1",
            status: "canceling",
            eventCursor: 3
        }))
        verify(OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "running",
            eventCursor: 4,
            progress: 0.2,
            bytesWritten: 20,
            coalescedCount: 1,
            updatedAt: 10
        }, {
            operationId: "operation-1",
            status: "running",
            eventCursor: 4,
            progress: 0.3,
            bytesWritten: 30,
            coalescedCount: 2,
            updatedAt: 11
        }))
        verify(!OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "running",
            eventCursor: 4,
            progress: 0.3,
            bytesWritten: 30,
            updatedAt: 11
        }, {
            operationId: "operation-1",
            status: "running",
            eventCursor: 4,
            progress: 0.25,
            bytesWritten: 29,
            updatedAt: 12
        }))
        verify(!OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "running",
            eventCursor: 4,
            progress: 0.3,
            bytesWritten: 30,
            coalescedCount: 2,
            updatedAt: 11
        }, {
            operationId: "operation-1",
            status: "running",
            eventCursor: 4,
            progress: 0.3,
            bytesWritten: 30,
            coalescedCount: 2,
            updatedAt: 12
        }))
        verify(!OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "running",
            eventCursor: 4,
            progress: 0.3,
            bytesWritten: 30,
            updatedAt: 11
        }, {
            operationId: "operation-1",
            status: "canceling",
            eventCursor: 4,
            progress: 0.3,
            bytesWritten: 30,
            updatedAt: 12
        }))
        verify(!OperationHistoryVocabulary.runtimeSnapshotIsNewer({
            operationId: "operation-1",
            status: "completed",
            eventCursor: 3
        }, {
            operationId: "operation-1",
            status: "awaiting_external",
            eventCursor: 4
        }))
    }
}
