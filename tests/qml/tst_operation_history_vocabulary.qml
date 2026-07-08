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

        compare(OperationHistoryVocabulary.runtimeStatusText({ status: "failed" }), "Failed")
        compare(OperationHistoryVocabulary.runtimeTone({ status: "failed" }), "error")

        compare(OperationHistoryVocabulary.runtimeStatusText(null), "Idle")
        compare(OperationHistoryVocabulary.runtimeTone(null), "neutral")
    }
}
