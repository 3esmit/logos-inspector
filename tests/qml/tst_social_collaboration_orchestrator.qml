import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "SocialCollaborationState"

    property bool gateEnabled: false
    property int saveSettingsCalls: 0
    property int sendCalls: 0
    property var commentPageRows: []

    QtObject {
        id: bridgeStub

        function callModule(moduleName, method, args) {
            if (method === "socialTopicValid") {
                return { ok: true, value: String(args[0] || "").indexOf("/valid/") === 0 }
            }
            if (method === "socialCommentTopic") {
                return { ok: true, value: "/" + args.join("/") + "/comments" }
            }
            return { ok: false, value: null, error: "unsupported" }
        }
    }

    QtObject {
        id: sourceRoutingStub

        function deliveryOperationAdapter() { return { source_mode: "rest", inputs: {} } }
        function storageOperationAdapter() { return { source_mode: "rest", inputs: {} } }
    }

    ListModel { id: registeredIdls }

    QtObject {
        id: gatewayStub

        function requestModule(moduleName, method) {
            if (method === "deliveryStoreQuery") {
                return { ok: true, value: { messages: [] }, error: "" }
            }
            if (method === "socialCommentPageFromStore") {
                return {
                    ok: true,
                    value: { rows: testRoot.commentPageRows, cursor: "cursor-1" },
                    error: ""
                }
            }
            return { ok: false, value: null, error: "unused" }
        }
        function callInspector(method) {
            if (method === "deliverySend") {
                testRoot.sendCalls += 1
                return { ok: true, value: {}, error: "" }
            }
            return { ok: false, value: null, error: "unused" }
        }
        function saveSettingsState() { testRoot.saveSettingsCalls += 1 }
        function saveIdlState() {}
        function socialGate(key) {
            return {
                enabled: testRoot.gateEnabled,
                status: testRoot.gateEnabled ? "enabled" : "unavailable",
                missing: testRoot.gateEnabled ? [] : [{
                    dependency: key,
                    label: "Delivery Store",
                    status: "unavailable"
                }],
                warnings: [],
                provenance: ["test"]
            }
        }
        function effectiveMessagingSourceMode(value) { return value }
        function configuredStorageRestUrl() { return "http://storage" }
        function normalizedIdlEntry(entry) { return entry }
        function idlEntryForKey() { return null }
        function idlNameFromJson() { return "IDL" }
        function canonicalProgramIdHex(value) { return String(value || "") }
        function normalizedHexText(value) { return String(value || "") }
        function accountOwnerCacheKey(value) { return String(value || "") }
        function zoneScopeKey() { return "zone-a" }
    }

    Domains.SocialCollaborationState {
        id: social

        bridge: bridgeStub
        inspectorModule: "logos_inspector"
        sourceRouting: sourceRoutingStub
        registeredIdls: registeredIdls
        gateway: gatewayStub
        busy: false
        messagingSourceMode: "rest"
        messagingMutatingDiagnosticsEnabled: false
        storageMutatingDiagnosticsEnabled: false
    }

    function init() {
        gateEnabled = false
        saveSettingsCalls = 0
        sendCalls = 0
        commentPageRows = []
        social.socialIdentities.clear()
        social.selectedSocialIdentityKey = ""
        social.socialConversationIdentityKeys = ({})
        social.socialCommentState = ({})
        social.socialIdentityRevision = 0
        social.socialCommentRevision = 0
    }

    function test_comment_rows_are_owned_and_deduplicated() {
        gateEnabled = true
        commentPageRows = [{ key: "a", body: "one" }]
        verify(social.loadComments("/valid/topic", true, 20, ""))

        commentPageRows = [
            { key: "a", body: "duplicate" },
            { key: "b", body: "two" }
        ]
        verify(social.loadComments("/valid/topic", false, 20, ""))
        const view = social.commentsView("/valid/topic")

        compare(view.rows.length, 2)
        compare(view.rows[0].body, "one")
        compare(view.rows[1].body, "two")
        verify(view.revision >= 4)
    }

    function test_identity_workflow_persists_through_narrow_gateway() {
        const identity = social.createIdentity("Alice")

        compare(identity.displayName, "Alice")
        compare(social.identitiesView().rows.length, 1)
        compare(social.settingsPayload().social_selected_identity_key, identity.key)
        compare(saveSettingsCalls, 1)
    }

    function test_post_workflow_reuses_per_topic_identity() {
        gateEnabled = true

        verify(social.postComment("/valid/topic-a", "first", "", null))
        verify(social.postComment("/valid/topic-a", "second", "", null))
        verify(social.postComment("/valid/topic-a", "second", "", null))
        verify(social.postComment("/valid/topic-b", "third", "", null))

        compare(social.identitiesView().rows.length, 2)
        compare(social.commentsView("/valid/topic-a").rows.length, 3)
        compare(social.commentsView("/valid/topic-b").rows.length, 1)
        compare(sendCalls, 4)
    }

    function test_gate_and_topic_helpers_use_explicit_dependencies() {
        const unavailable = social.commentsView("/valid/topic")
        verify(unavailable.readError.indexOf("Delivery Store") >= 0)

        gateEnabled = true
        verify(social.commentsView("/valid/topic").readGate.enabled)
        compare(social.commentTopic("cryptarchia", "block", "a"),
                "/cryptarchia/block/a/comments")
    }
}
