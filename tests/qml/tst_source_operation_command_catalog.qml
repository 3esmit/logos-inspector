import QtQuick
import QtTest
import "../../qml/state/source_operations/SourceOperationCommandCatalog.js" as SourceOperationCommandCatalog

TestCase {
    name: "SourceOperationCommandCatalog"

    function test_storage_commands_map_methods_to_actions_and_inputs() {
        const upload = SourceOperationCommandCatalog.storageCommand("storageUploadUrl", ["/tmp/file.bin"])
        const exists = SourceOperationCommandCatalog.storageCommand("storageExists", ["cid"])

        compare(upload.action, "upload")
        compare(upload.requiredInputs.length, 1)
        compare(upload.requiredInputs[0].key, "path")
        compare(upload.requiredInputs[0].value, "/tmp/file.bin")
        verify(upload.runtime)

        compare(exists.action, "exists")
        compare(exists.requiredInputs[0].key, "cid")
        compare(exists.requiredInputs[0].value, "cid")
        verify(!exists.runtime)
    }

    function test_delivery_commands_map_runtime_and_inputs() {
        const query = SourceOperationCommandCatalog.deliveryCommand("deliveryStoreQuery", ["/topic"])
        const send = SourceOperationCommandCatalog.deliveryCommand("deliverySend", ["/topic"])

        compare(query.action, "store_query")
        compare(query.requiredInputs[0].key, "topic")
        verify(!query.runtime)

        compare(send.action, "send")
        compare(send.requiredInputs[0].value, "/topic")
        verify(send.runtime)
    }

    function test_gate_detail_prefers_missing_dependency() {
        const detail = SourceOperationCommandCatalog.gateDetailText({
            missing: [{ dependency: "storage.connect", label: "Storage" }]
        }, "Fallback")

        compare(detail, "Storage unavailable: storage.connect")
        compare(SourceOperationCommandCatalog.gateDetailText({}, "Fallback"), "Fallback unavailable.")
    }

    function test_operation_completed_is_status_based() {
        verify(SourceOperationCommandCatalog.operationCompleted({ status: "completed" }))
        verify(!SourceOperationCommandCatalog.operationCompleted({ status: "failed" }))
    }
}
