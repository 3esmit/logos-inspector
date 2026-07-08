import QtQuick
import QtTest
import "../../qml/utils/UiFormat.js" as UiFormat

TestCase {
    name: "UiFormat"

    function test_number_text_keeps_old_defaults_and_can_coerce() {
        compare(UiFormat.numberText(null), "-")
        compare(UiFormat.numberText("42"), "42")
        compare(UiFormat.numberText("42", { coerceNumericStrings: true }), "42")
    }

    function test_value_text_can_render_json_objects() {
        compare(UiFormat.valueText(null, "n/a"), "n/a")
        compare(UiFormat.valueText({ status: "ok" }, { objectMode: "json" }), "{\"status\":\"ok\"}")
    }

    function test_value_summary_unwraps_and_summarizes() {
        const options = {
            emptyText: "n/a",
            emptyArrayText: "empty",
            shortArrayLimit: 3,
            unwrapKeys: ["result", "value"],
            objectSummary: "fields"
        }

        compare(UiFormat.valueSummary(null, options), "n/a")
        compare(UiFormat.valueSummary([], options), "empty")
        compare(UiFormat.valueSummary({ result: { value: ["a", "b"] } }, options), "a, b")
        compare(UiFormat.valueSummary([1, 2, 3, 4], options), "4 item(s)")
        compare(UiFormat.valueSummary({ one: 1, two: 2 }, options), "2 field(s)")
    }

    function test_short_text_supports_middle_truncation_options() {
        compare(UiFormat.shortText("", { emptyText: "n/a", limit: 12, minimum: 12, tailLength: 5 }), "n/a")
        compare(UiFormat.shortText("abcdefghijklmnopqrstuvwxyz", { limit: 12, minimum: 12, tailLength: 5 }), "abcd...vwxyz")
    }

    function test_count_value_and_copy_value() {
        compare(UiFormat.countValue({ result: { peers: ["a", "b"] } }, { nestedKeys: ["peers", "result"] }), 2)
        compare(UiFormat.copyValue(null), "")
        compare(UiFormat.copyValue({ a: 1 }), "{\n  \"a\": 1\n}")
    }
}
