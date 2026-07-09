import QtQuick
import QtTest
import "../../qml/state/social/SocialCollaborationOrchestrator.js" as Social

TestCase {
    name: "SocialCollaborationOrchestrator"

    QtObject {
        id: socialRoot

        property bool gateEnabled: false
        property int socialCommentPageSize: 20

        function socialGate(key) {
            return {
                enabled: gateEnabled,
                status: gateEnabled ? "enabled" : "unavailable",
                missing: gateEnabled ? [] : [{ dependency: key, label: "Delivery Store", status: "unavailable" }],
                warnings: [],
                provenance: ["test"]
            }
        }

        function copyMap(value) {
            const copy = {}
            const source = value || {}
            for (const key in source) {
                copy[key] = source[key]
            }
            return copy
        }

        function validSocialTopic(topic) {
            return String(topic || "").indexOf("/valid/") === 0
        }
    }

    function test_merge_comment_rows_deduplicates_by_key() {
        const rows = Social.mergeSocialCommentRows(socialRoot, [{ key: "a", body: "one" }], [
            { key: "a", body: "duplicate" },
            { key: "b", body: "two" }
        ])

        compare(rows.length, 2)
        compare(rows[0].body, "one")
        compare(rows[1].body, "two")
    }

    function test_page_size_is_clamped() {
        socialRoot.socialCommentPageSize = 20

        compare(Social.socialPageSize(socialRoot, -10), 1)
        compare(Social.socialPageSize(socialRoot, 200), 100)
        compare(Social.socialPageSize(socialRoot, "bad"), 20)
    }

    function test_gate_detail_reports_missing_dependency_and_topic_input() {
        const missing = Social.socialGateDetailText(socialRoot, Social.socialStoreGate(socialRoot), "fallback")
        verify(missing.indexOf("Delivery Store") >= 0)

        const topicGate = Social.socialCommentReadGate(socialRoot, "")
        compare(topicGate.status, "unavailable")
        verify(Social.socialGateDetailText(socialRoot, topicGate, "").indexOf("Social topic") >= 0)
    }
}
