import QtQuick
import QtTest
import "../../qml/state/domains" as Domains

TestCase {
    id: testRoot

    name: "SocialCollaborationState"

    property bool gateEnabled: false
    property int saveSettingsCalls: 0
    property int sendCalls: 0
    property int syncSendCalls: 0
    property bool autoCompleteSend: true
    property var commentPageRows: []
    property bool autoCompleteStore: true
    property bool autoCompleteCommentDecode: true
    property bool autoCompleteHydration: true
    property var startRequests: []
    property var startCallbacks: []
    property var commentDecodeRequests: []
    property var commentDecodeCallbacks: []
    property var hydrationRequests: []
    property var hydrationCallbacks: []
    property int syncStoreCalls: 0
    property string commentDecodeError: ""

    QtObject {
        id: bridgeStub

        function callModule(moduleName, method, args) {
            if (method === "socialTopicValid") {
                const topic = String(args[0] || "")
                return {
                    ok: true,
                    value: topic.indexOf("/valid/") === 0 || topic.indexOf("/lez/account/") === 0
                }
            }
            if (method === "socialCommentTopic") {
                return {
                    ok: true,
                    value: "/" + args.join("/") + "/comments"
                }
            }
            if (method === "socialZoneAccountIdlTopic") {
                return {
                    ok: true,
                    value: "/lez/account/" + String(args[0] && args[0].canonical_key || "") + "/idl"
                }
            }
            return {
                ok: false,
                value: null,
                error: "unsupported"
            }
        }
    }

    QtObject {
        id: sourceRoutingStub

        function deliveryOperationAdapter() {
            return {
                source_mode: "rest",
                inputs: {}
            }
        }
        function storageOperationAdapter() {
            return {
                source_mode: "rest",
                inputs: {}
            }
        }

        function effectiveMessagingSourceMode(value) {
            return value
        }
    }

    ListModel {
        id: registeredIdls
    }

    QtObject {
        id: gatewayStub

        function requestModule(moduleName, method, args) {
            if (method === "deliveryStoreQuery") {
                testRoot.syncStoreCalls += 1
                return {
                    ok: false,
                    value: null,
                    error: "synchronous Store query"
                }
            }
            return {
                ok: false,
                value: null,
                error: "unused"
            }
        }
        function requestModuleAsync(moduleName, method, args, label, showResult, callback) {
            if (method === "socialCommentPageFromStore") {
                testRoot.commentDecodeRequests = testRoot.commentDecodeRequests.concat([
                    {
                        moduleName: moduleName,
                        method: method,
                        args: args
                    }
                ])
                testRoot.commentDecodeCallbacks = testRoot.commentDecodeCallbacks.concat([callback])
                if (testRoot.commentDecodeError.length) {
                    if (testRoot.autoCompleteCommentDecode) {
                        callback({
                            ok: false,
                            value: null,
                            error: testRoot.commentDecodeError
                        })
                    }
                    return testRoot.commentDecodeRequests.length
                }
                const storeValue = args[1] || {}
                const response = {
                    ok: true,
                    value: {
                        rows: Array.isArray(storeValue.rows) ? storeValue.rows : testRoot.commentPageRows,
                        cursor: String(storeValue.cursor || "cursor-1")
                    },
                    error: ""
                }
                if (testRoot.autoCompleteCommentDecode) {
                    callback(response)
                }
                return testRoot.commentDecodeRequests.length
            }
            testRoot.hydrationRequests = testRoot.hydrationRequests.concat([
                {
                    moduleName: moduleName,
                    method: method,
                    args: args
                }
            ])
            testRoot.hydrationCallbacks = testRoot.hydrationCallbacks.concat([callback])
            if (testRoot.autoCompleteHydration) {
                callback({
                    ok: true,
                    value: [],
                    error: ""
                })
            }
            return testRoot.hydrationRequests.length
        }
        function startRuntimeOperation(request, showResult, callback) {
            testRoot.startRequests = testRoot.startRequests.concat([request])
            testRoot.startCallbacks = testRoot.startCallbacks.concat([callback])
            if (String(request && request.method || "") === "deliverySend") {
                testRoot.sendCalls += 1
                if (testRoot.autoCompleteSend) {
                    callback({
                        ok: true,
                        value: testRoot.sendOperation("send-" + testRoot.sendCalls, "completed", request.payload.topic, {
                            sent: true
                        }, testRoot.sendCalls)
                    })
                }
                return testRoot.startRequests.length
            }
            if (String(request && request.method || "") === "storageUploadPayload") {
                return testRoot.startRequests.length
            }
            if (testRoot.autoCompleteStore) {
                callback({
                    ok: true,
                    value: testRoot.storeOperation("store-" + testRoot.startRequests.length, "completed", {
                        messages: []
                    }, testRoot.startRequests.length)
                })
            }
            return testRoot.startRequests.length
        }
        function runtimeOperationStatus() {
            return null
        }
        function appendOperationHistory() {
        }
        function callInspector(method) {
            if (method === "deliverySend") {
                testRoot.syncSendCalls += 1
                return {
                    ok: true,
                    value: {},
                    error: ""
                }
            }
            return {
                ok: false,
                value: null,
                error: "unused"
            }
        }
        function saveSettingsState() {
            testRoot.saveSettingsCalls += 1
        }
        function saveIdlState() {
        }
        function socialGate(key) {
            return {
                enabled: testRoot.gateEnabled,
                status: testRoot.gateEnabled ? "enabled" : "unavailable",
                missing: testRoot.gateEnabled ? [] : [
                    {
                        dependency: key,
                        label: "Delivery Store",
                        status: "unavailable"
                    }
                ],
                warnings: [],
                provenance: ["test"]
            }
        }
        function configuredStorageRestUrl() {
            return "http://storage"
        }
        function normalizedIdlEntry(entry) {
            return entry
        }
        function idlEntryForKey(key) {
            const wanted = String(key || "")
            for (let i = 0; i < registeredIdls.count; ++i) {
                const entry = registeredIdls.get(i)
                if (String(entry.key || "") === wanted) {
                    return entry
                }
            }
            return null
        }
        function zoneAccountEntityRef(accountId) {
            const entity = testRoot.autoShareEntity()
            entity.canonical_key = String(accountId || "")
            return entity
        }
        function idlNameFromJson() {
            return "IDL"
        }
        function canonicalProgramIdHex(value) {
            return String(value || "")
        }
        function normalizedHexText(value) {
            return String(value || "")
        }
        function accountOwnerCacheKey(value) {
            return String(value || "")
        }
        function zoneScopeKey() {
            return "zone-a"
        }
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
        messagingMutatingDiagnosticsEnabled: testRoot.messagingMutatingEnabled
        storageMutatingDiagnosticsEnabled: testRoot.storageMutatingEnabled
    }

    property bool messagingMutatingEnabled: false
    property bool storageMutatingEnabled: false

    function init() {
        gateEnabled = false
        saveSettingsCalls = 0
        sendCalls = 0
        syncSendCalls = 0
        autoCompleteSend = true
        messagingMutatingEnabled = false
        storageMutatingEnabled = false
        commentPageRows = []
        autoCompleteStore = true
        autoCompleteCommentDecode = true
        autoCompleteHydration = true
        startRequests = []
        startCallbacks = []
        commentDecodeRequests = []
        commentDecodeCallbacks = []
        hydrationRequests = []
        hydrationCallbacks = []
        syncStoreCalls = 0
        commentDecodeError = ""
        social.invalidateSourceRequests()
        social.socialIdentities.clear()
        social.selectedSocialIdentityKey = ""
        social.socialConversationIdentityKeys = ({})
        social.socialCommentState = ({})
        social.socialIdentityRevision = 0
        social.socialCommentRevision = 0
        social.socialSharedIdls = ({})
        registeredIdls.clear()
        social.sharedIdlAutoShare = false
        social.socialAutoSharedIdls = ({})
        social.sharedIdlRevision = 0
    }

    function storeOperation(id, status, result, eventCursor) {
        return {
            operationId: id,
            domain: "delivery",
            method: "deliveryStoreQuery",
            label: "Store",
            status: status,
            eventCursor: eventCursor,
            result: result,
            error: ""
        }
    }

    function sendOperation(id, status, topic, result, eventCursor) {
        return {
            operationId: id,
            domain: "delivery",
            method: "deliverySend",
            label: "Post comment",
            status: status,
            eventCursor: eventCursor,
            context: {
                source: "rest",
                contentTopic: topic
            },
            result: result,
            error: status === "failed" ? "send failed" : ""
        }
    }

    function uploadOperation(id, status, result, eventCursor) {
        return {
            operationId: id,
            domain: "storage",
            method: "storageUploadPayload",
            label: "Upload shared IDL",
            status: status,
            eventCursor: eventCursor,
            context: {
                source: "rest",
                filename: "logos-inspector-shared-idl.json"
            },
            result: result,
            error: status === "failed" ? "upload failed" : ""
        }
    }

    function replyStart(index, operation) {
        startCallbacks[index]({
            ok: true,
            value: operation,
            error: ""
        })
    }

    function replyCommentDecode(index, response) {
        commentDecodeCallbacks[index](response)
    }

    function sharedEntry(key) {
        return {
            key: key,
            name: key,
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            programIdHex: "program-1",
            source: "shared",
            sharedAccountId: "account-1",
            sharedTopic: "/lez/account/account-1/idl",
            accountType: "State"
        }
    }

    function autoShareEntity() {
        return {
            canonical_key: "account-1",
            entity_kind: "account",
            channel_id: "zone-a",
            network_scope: {
                kind: "genesis_id",
                genesis_id: "genesis-a"
            }
        }
    }

    function autoShareEntry() {
        return {
            key: "idl-1",
            name: "Local",
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local"
        }
    }

    function test_comment_rows_are_owned_and_deduplicated() {
        gateEnabled = true
        commentPageRows = [
            {
                key: "a",
                body: "one"
            }
        ]
        verify(social.loadComments("/valid/topic", true, 20, ""))

        commentPageRows = [
            {
                key: "a",
                body: "duplicate"
            },
            {
                key: "b",
                body: "two"
            }
        ]
        verify(social.loadComments("/valid/topic", false, 20, ""))
        const view = social.commentsView("/valid/topic")

        compare(view.rows.length, 2)
        compare(view.rows[0].body, "one")
        compare(view.rows[1].body, "two")
        verify(view.revision >= 4)
        compare(syncStoreCalls, 0)
    }

    function test_comment_topics_overlap_and_complete_in_reverse() {
        gateEnabled = true
        autoCompleteStore = false

        verify(social.loadComments("/valid/topic-a", true, 20, ""))
        verify(social.loadComments("/valid/topic-b", true, 20, ""))
        verify(social.commentsView("/valid/topic-a").state.loading)
        verify(social.commentsView("/valid/topic-b").state.loading)

        replyStart(1, storeOperation("store-b", "completed", {
            rows: [
                {
                    key: "b",
                    body: "two"
                }
            ],
            cursor: "cursor-b"
        }, 1))
        replyStart(0, storeOperation("store-a", "completed", {
            rows: [
                {
                    key: "a",
                    body: "one"
                }
            ],
            cursor: "cursor-a"
        }, 1))

        compare(social.commentsView("/valid/topic-a").rows[0].body, "one")
        compare(social.commentsView("/valid/topic-b").rows[0].body, "two")
        compare(syncStoreCalls, 0)
    }

    function test_duplicate_same_topic_pending_query_is_rejected_without_losing_first() {
        gateEnabled = true
        autoCompleteStore = false

        verify(social.loadComments("/valid/topic", true, 20, ""))
        verify(!social.loadComments("/valid/topic", true, 20, ""))
        compare(startCallbacks.length, 1)
        verify(social.commentsView("/valid/topic").state.loading)
        replyStart(0, storeOperation("store-first", "completed", {
            rows: [
                {
                    key: "first",
                    body: "first"
                }
            ],
            cursor: "cursor-first"
        }, 1))

        const view = social.commentsView("/valid/topic")
        compare(view.rows.length, 1)
        compare(view.rows[0].key, "first")
        verify(!view.state.loading)
    }

    function test_new_same_topic_request_rejects_old_deferred_decode() {
        gateEnabled = true
        autoCompleteCommentDecode = false

        verify(social.loadComments("/valid/topic", true, 20, ""))
        verify(social.loadComments("/valid/topic", true, 20, ""))
        compare(commentDecodeCallbacks.length, 2)
        replyCommentDecode(1, {
            ok: true,
            value: {
                rows: [
                    {
                        key: "new",
                        body: "new"
                    }
                ],
                cursor: "new-cursor"
            },
            error: ""
        })
        replyCommentDecode(0, {
            ok: true,
            value: {
                rows: [
                    {
                        key: "old",
                        body: "old"
                    }
                ],
                cursor: "old-cursor"
            },
            error: ""
        })

        const view = social.commentsView("/valid/topic")
        compare(view.rows.length, 1)
        compare(view.rows[0].key, "new")
        verify(!view.state.loading)
    }

    function test_comment_source_invalidation_clears_loading_and_rejects_late_reply() {
        gateEnabled = true
        autoCompleteStore = false

        verify(social.loadComments("/valid/topic", true, 20, ""))
        verify(social.commentsView("/valid/topic").state.loading)
        social.invalidateSourceRequests()
        verify(!social.commentsView("/valid/topic").state.loading)
        replyStart(0, storeOperation("store-late", "completed", {
            rows: [
                {
                    key: "late",
                    body: "late"
                }
            ],
            cursor: "cursor-late"
        }, 1))

        compare(social.commentsView("/valid/topic").rows.length, 0)
    }

    function test_source_invalidation_discards_idle_rows_and_cursor_before_load_more() {
        gateEnabled = true
        commentPageRows = [
            {
                key: "old",
                body: "old"
            }
        ]
        verify(social.loadComments("/valid/topic", true, 20, ""))
        compare(social.commentsView("/valid/topic").state.cursor, "cursor-1")

        social.invalidateSourceRequests()
        const invalidated = social.commentsView("/valid/topic").state
        compare(invalidated.rows.length, 0)
        compare(invalidated.cursor, "")

        autoCompleteStore = false
        const startIndex = startRequests.length
        verify(social.loadComments("/valid/topic", false, 20, ""))
        compare(startRequests[startIndex].payload.cursor, "")
        replyStart(startIndex, storeOperation("store-new-source", "completed", {
            rows: [
                {
                    key: "new",
                    body: "new"
                }
            ],
            cursor: "new-cursor"
        }, 1))
        compare(social.commentsView("/valid/topic").rows[0].key, "new")
    }

    function test_comment_terminal_and_decode_failures_clear_loading() {
        gateEnabled = true
        autoCompleteStore = false

        verify(social.loadComments("/valid/terminal", true, 20, ""))
        const failed = storeOperation("store-failed", "failed", null, 1)
        failed.error = "store failed"
        replyStart(0, failed)
        compare(social.commentsView("/valid/terminal").state.error, "store failed")
        verify(!social.commentsView("/valid/terminal").state.loading)

        commentDecodeError = "decode failed"
        verify(social.loadComments("/valid/decode", true, 20, ""))
        replyStart(1, storeOperation("store-decode", "completed", {
            messages: []
        }, 1))
        compare(social.commentsView("/valid/decode").state.error, "decode failed")
        verify(!social.commentsView("/valid/decode").state.loading)
    }

    function test_shared_idl_hydration_rejects_superseded_callback() {
        gateEnabled = true
        autoCompleteHydration = false
        social.setSharedIdlPolicy("suggestion")
        const entity = {
            canonical_key: "account-1",
            entity_kind: "account",
            channel_id: "zone-a",
            network_scope: {
                kind: "genesis_id",
                genesis_id: "genesis-a"
            }
        }

        verify(social.refreshSharedIdlsForAccount(entity, "aabb", "program-1"))
        verify(social.refreshSharedIdlsForAccount(entity, "ccdd", "program-1"))
        compare(hydrationRequests.length, 2)
        compare(hydrationRequests[0].method, "acceptedSharedIdlEntriesFromStoreWithStorage")

        hydrationCallbacks[1]({
            ok: true,
            value: [sharedEntry("new")],
            error: ""
        })
        hydrationCallbacks[0]({
            ok: true,
            value: [sharedEntry("old")],
            error: ""
        })

        const suggestions = social.sharedIdlSuggestions("account-1", "program-1")
        compare(suggestions.length, 1)
        compare(suggestions[0].key, "new")
        compare(syncStoreCalls, 0)
    }

    function test_shared_idl_settings_reload_rejects_pending_hydration() {
        gateEnabled = true
        autoCompleteHydration = false
        social.setSharedIdlPolicy("suggestion")
        const entity = {
            canonical_key: "account-1",
            entity_kind: "account",
            channel_id: "zone-a",
            network_scope: {
                kind: "genesis_id",
                genesis_id: "genesis-a"
            }
        }

        verify(social.refreshSharedIdlsForAccount(entity, "aabb", "program-1"))
        compare(hydrationCallbacks.length, 1)
        social.loadSettings({
            shared_idl_policy: "disabled"
        })
        hydrationCallbacks[0]({
            ok: true,
            value: [sharedEntry("late")],
            error: ""
        })

        compare(social.sharedIdlPolicy, "disabled")
        compare(social.sharedIdlSuggestions("account-1", "program-1").length, 0)
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
        messagingMutatingEnabled = true

        verify(social.postComment("/valid/topic-a", "first", "", null))
        verify(social.postComment("/valid/topic-a", "second", "", null))
        verify(social.postComment("/valid/topic-a", "second", "", null))
        verify(social.postComment("/valid/topic-b", "third", "", null))

        compare(social.identitiesView().rows.length, 2)
        compare(social.commentsView("/valid/topic-a").rows.length, 3)
        compare(social.commentsView("/valid/topic-b").rows.length, 1)
        compare(sendCalls, 4)
        compare(syncSendCalls, 0)
    }

    function test_post_workflow_projects_only_terminal_delivery() {
        gateEnabled = true
        messagingMutatingEnabled = true
        autoCompleteSend = false
        let completion = null

        verify(social.postComment("/valid/topic", "first", "", null, function (response) {
            completion = response
        }))

        compare(sendCalls, 1)
        compare(syncSendCalls, 0)
        compare(social.commentsView("/valid/topic").rows.length, 0)
        verify(social.commentsView("/valid/topic").state.sending)
        verify(!social.postComment("/valid/other", "second", "", null))

        replyStart(0, sendOperation("send-held", "completed", "/valid/topic", ["request-1", "hash-1"], 1))

        verify(completion && completion.ok)
        compare(social.commentsView("/valid/topic").rows.length, 1)
        verify(!social.commentsView("/valid/topic").state.sending)
    }

    function test_post_terminal_failure_keeps_row_absent_and_allows_retry() {
        gateEnabled = true
        messagingMutatingEnabled = true
        autoCompleteSend = false

        verify(social.postComment("/valid/topic", "first", "", null))
        verify(social.commentsView("/valid/topic").state.sending)

        replyStart(0, sendOperation("send-failed", "failed", "/valid/topic", null, 1))

        const view = social.commentsView("/valid/topic")
        compare(view.rows.length, 0)
        verify(!view.state.sending)
        compare(view.state.sendError, "send failed")
        compare(view.writeError, "send failed")
        verify(view.writeAvailable)
        compare(syncSendCalls, 0)
    }

    function test_pending_comment_policy_invalidation_retains_write_error() {
        gateEnabled = true
        messagingMutatingEnabled = true
        autoCompleteSend = false

        verify(social.postComment("/valid/topic", "first", "", null))
        verify(social.commentsView("/valid/topic").state.sending)

        messagingMutatingEnabled = false

        const invalidated = social.commentsView("/valid/topic")
        verify(!social.writesRunning)
        verify(!invalidated.state.sending)
        verify(String(invalidated.state.sendError).length > 0)
        compare(invalidated.rows.length, 0)
        const retainedError = invalidated.state.sendError

        replyStart(0, sendOperation("send-late", "completed", "/valid/topic", {
            sent: true
        }, 1))
        const afterLateReply = social.commentsView("/valid/topic")
        compare(afterLateReply.rows.length, 0)
        compare(afterLateReply.state.sendError, retainedError)
    }

    function test_auto_share_marker_waits_for_upload_and_delivery_completion() {
        gateEnabled = true
        messagingMutatingEnabled = true
        storageMutatingEnabled = true
        autoCompleteSend = false
        social.sharedIdlAutoShare = true
        const entity = autoShareEntity()
        const entry = autoShareEntry()
        const markerKey = [entity.canonical_key, social.zoneAccountIdlTopic(entity), entry.key].join("|")

        verify(social.maybeAutoShareAccountIdl(entity, "program-1", entry))
        compare(startRequests.length, 1)
        compare(startRequests[0].method, "storageUploadPayload")
        compare(Object.keys(social.socialAutoSharedIdls).length, 0)

        replyStart(0, uploadOperation("upload-idl", "completed", {
            cid: "cid-idl",
            filename: "logos-inspector-shared-idl.json"
        }, 1))
        compare(startRequests.length, 2)
        compare(startRequests[1].method, "deliverySend")
        compare(Object.keys(social.socialAutoSharedIdls).length, 0)
        const savesBeforeTerminal = saveSettingsCalls

        replyStart(1, sendOperation("send-idl", "completed", startRequests[1].payload.topic, ["request-1", "hash-1"], 1))

        const markers = social.settingsPayload().social_auto_shared_idls
        compare(Object.keys(markers).length, 1)
        compare(markers[markerKey], true)
        compare(saveSettingsCalls, savesBeforeTerminal + 1)
        verify(!social.maybeAutoShareAccountIdl(entity, "program-1", entry))
        compare(startRequests.length, 2)
        compare(syncSendCalls, 0)
    }

    function test_registered_idl_can_be_shared_from_public_ui_seam() {
        gateEnabled = true
        messagingMutatingEnabled = true
        storageMutatingEnabled = true
        autoCompleteSend = false
        social.createIdentity("IDL publisher")
        registeredIdls.append({
            key: "idl-1",
            name: "Local",
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            programIdHex: "program-1",
            source: "local"
        })

        verify(social.publishRegisteredIdl("account-1", "idl-1"))
        compare(startRequests.length, 1)
        compare(startRequests[0].method, "storageUploadPayload")

        replyStart(0, uploadOperation("upload-idl", "completed", {
            cid: "cid-idl",
            filename: "logos-inspector-shared-idl.json"
        }, 1))

        compare(startRequests.length, 2)
        compare(startRequests[1].method, "deliverySend")
        compare(startRequests[1].payload.topic, "/lez/account/account-1/idl")
    }

    function test_auto_share_delivery_failure_does_not_persist_marker() {
        gateEnabled = true
        messagingMutatingEnabled = true
        storageMutatingEnabled = true
        autoCompleteSend = false
        social.sharedIdlAutoShare = true
        const entity = autoShareEntity()
        const entry = autoShareEntry()
        const markerKey = [entity.canonical_key, social.zoneAccountIdlTopic(entity), entry.key].join("|")

        verify(social.maybeAutoShareAccountIdl(entity, "program-1", entry))
        replyStart(0, uploadOperation("upload-idl", "completed", {
            cid: "cid-idl",
            filename: "logos-inspector-shared-idl.json"
        }, 1))
        const savesBeforeTerminal = saveSettingsCalls

        replyStart(1, sendOperation("send-idl", "failed", startRequests[1].payload.topic, null, 1))

        const markers = social.settingsPayload().social_auto_shared_idls
        compare(Object.keys(markers).length, 0)
        verify(markers[markerKey] !== true)
        compare(saveSettingsCalls, savesBeforeTerminal)
        verify(!social.writesRunning)
        compare(syncSendCalls, 0)
    }

    function test_gate_and_topic_helpers_use_explicit_dependencies() {
        const unavailable = social.commentsView("/valid/topic")
        verify(unavailable.readError.indexOf("Delivery Store") >= 0)

        gateEnabled = true
        verify(social.commentsView("/valid/topic").readGate.enabled)
        compare(social.commentTopic("cryptarchia", "block", "a"), "/cryptarchia/block/a/comments")
    }
}
