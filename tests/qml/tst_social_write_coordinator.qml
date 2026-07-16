import QtQuick
import QtTest
import "../../qml/state/social" as Social

TestCase {
    id: testRoot

    name: "SocialWriteCoordinator"

    property var startRequests: []
    property var startCallbacks: []
    property var statusRequests: []
    property var statusCallbacks: []
    property var history: []

    QtObject {
        id: gatewayStub

        function startRuntimeOperation(request, showResult, callback) {
            testRoot.startRequests = testRoot.startRequests.concat([request])
            testRoot.startCallbacks = testRoot.startCallbacks.concat([callback])
            return testRoot.startRequests.length
        }

        function runtimeOperationStatus(operationId, showResult, callback) {
            testRoot.statusRequests = testRoot.statusRequests.concat([operationId])
            testRoot.statusCallbacks = testRoot.statusCallbacks.concat([callback])
            return testRoot.statusRequests.length
        }

        function appendOperationHistory(operation, detail) {
            testRoot.history = testRoot.history.concat([operation])
        }
    }

    Social.SocialWriteCoordinator {
        id: coordinator

        gateway: gatewayStub
        storageAdapterInitialization: ({
                source_mode: "rest",
                inputs: {
                    rest_endpoint: "http://storage-a"
                }
            })
        deliveryAdapterInitialization: ({
                source_mode: "rest",
                inputs: {
                    rest_endpoint: "http://delivery-a"
                }
            })
        storageMutatingDiagnosticsEnabled: true
        deliveryMutatingDiagnosticsEnabled: true
    }

    function init() {
        coordinator.invalidate("")
        coordinator.storageAdapterInitialization = ({
                source_mode: "rest",
                inputs: {
                    rest_endpoint: "http://storage-a"
                }
            })
        coordinator.deliveryAdapterInitialization = ({
                source_mode: "rest",
                inputs: {
                    rest_endpoint: "http://delivery-a"
                }
            })
        coordinator.storageMutatingDiagnosticsEnabled = true
        coordinator.deliveryMutatingDiagnosticsEnabled = true
        startRequests = []
        startCallbacks = []
        statusRequests = []
        statusCallbacks = []
        history = []
    }

    function operation(id, domain, method, status, context, result, cursor) {
        return {
            operationId: id,
            domain: domain,
            method: method,
            label: method,
            status: status,
            context: context || {},
            result: result,
            error: status === "failed" ? "operation failed" : "",
            eventCursor: cursor === undefined ? 1 : cursor
        }
    }

    function uploadOperation(id, status, result, cursor) {
        return operation(id, "storage", "storageUploadPayload", status, {
            source: "rest",
            filename: "shared-idl.json"
        }, result, cursor)
    }

    function sendOperation(id, status, topic, result, cursor) {
        return operation(id, "delivery", "deliverySend", status, {
            source: "rest",
            contentTopic: topic
        }, result, cursor)
    }

    function sharedRequest(topic) {
        return {
            filename: "shared-idl.json",
            artifact: {
                kind: "artifact",
                idl_json: "{}"
            },
            blockSize: 65536,
            topic: topic,
            message: {
                kind: "lez_account_idl",
                version: 2,
                account_id: "account-1",
                program_id: "program-1"
            }
        }
    }

    function replyStart(index, value) {
        startCallbacks[index]({
            ok: true,
            value: value,
            error: ""
        })
    }

    function replyStatus(index, value) {
        statusCallbacks[index]({
            ok: true,
            value: value,
            error: ""
        })
    }

    function test_comment_waits_for_exact_terminal_completion() {
        let completion = null

        verify(coordinator.startComment({
            topic: "/topic/comment",
            payloadText: "{\"kind\":\"comment\"}"
        }, function (response) {
            completion = response
        }))

        compare(startRequests.length, 1)
        compare(startRequests[0].domain, "delivery")
        compare(startRequests[0].method, "deliverySend")
        compare(startRequests[0].payload.topic, "/topic/comment")
        verify(coordinator.running)
        compare(completion, null)

        replyStart(0, sendOperation("send-1", "awaiting_external", "/topic/comment", null, 1))
        compare(completion, null)
        verify(coordinator.poll() !== null)
        compare(statusRequests[0], "send-1")

        replyStatus(0, sendOperation("send-1", "completed", "/topic/comment", ["request-1", "hash-1"], 2))

        verify(completion && completion.ok)
        compare(completion.value[1], "hash-1")
        verify(!coordinator.running)
        compare(history.length, 1)
    }

    function test_shared_idl_upload_completes_before_single_delivery_start() {
        let completion = null

        verify(coordinator.startSharedIdl(sharedRequest("/topic/idl"), function (response) {
            completion = response
        }))

        compare(startRequests.length, 1)
        compare(startRequests[0].domain, "storage")
        compare(startRequests[0].method, "storageUploadPayload")
        compare(startRequests[0].payload.filename, "shared-idl.json")
        replyStart(0, uploadOperation("upload-1", "completed", {
            cid: "cid-idl",
            filename: "shared-idl.json"
        }, 1))

        compare(startRequests.length, 2)
        compare(startRequests[1].domain, "delivery")
        compare(startRequests[1].method, "deliverySend")
        const message = JSON.parse(startRequests[1].payload.payload)
        compare(message.idl_cid, "cid-idl")
        compare(message.storage.cid, "cid-idl")
        compare(message.storage.source_mode, "rest")
        compare(message.storage.endpoint, "http://storage-a")
        compare(completion, null)

        replyStart(1, sendOperation("send-idl", "completed", "/topic/idl", {
            sent: true
        }, 1))

        verify(completion && completion.ok)
        compare(completion.cid, "cid-idl")
        compare(history.length, 2)
        verify(!coordinator.running)
    }

    function test_polled_upload_completion_starts_delivery_once() {
        let completion = null
        verify(coordinator.startSharedIdl(sharedRequest("/topic/idl"), function (response) {
            completion = response
        }))
        replyStart(0, uploadOperation("upload-polled", "awaiting_external", null, 1))

        compare(startRequests.length, 1)
        verify(coordinator.poll() !== null)
        compare(statusRequests[0], "upload-polled")
        replyStatus(0, uploadOperation("upload-polled", "completed", {
            cid: "cid-polled",
            filename: "shared-idl.json"
        }, 2))

        compare(startRequests.length, 2)
        compare(startRequests[1].method, "deliverySend")
        compare(JSON.parse(startRequests[1].payload.payload).idl_cid, "cid-polled")
        replyStart(1, sendOperation("send-polled", "completed", "/topic/idl", {
            sent: true
        }, 1))

        verify(completion && completion.ok)
        compare(startRequests.length, 2)
    }

    function test_upload_identity_and_result_must_match_before_delivery() {
        const cases = [uploadOperation("upload-a", "completed", {
                cid: "cid-a",
                filename: "other.json"
            }, 1), uploadOperation("upload-b", "completed", {
                cid: " ",
                filename: "shared-idl.json"
            }, 1), operation("upload-c", "storage", "storageUploadPayload", "completed", {
                source: "rest",
                filename: "other.json"
            }, {
                cid: "cid-c",
                filename: "shared-idl.json"
            }, 1)]

        for (let i = 0; i < cases.length; ++i) {
            let completion = null
            verify(coordinator.startSharedIdl(sharedRequest("/topic/idl"), function (response) {
                completion = response
            }))
            const index = startCallbacks.length - 1
            replyStart(index, cases[i])

            verify(completion && !completion.ok)
            compare(startRequests.length, i + 1)
            verify(!coordinator.running)
        }
    }

    function test_delivery_dispatched_wrong_topic_and_null_result_are_failures() {
        const cases = [sendOperation("send-a", "dispatched", "/topic/comment", {
                dispatched: true
            }, 1), sendOperation("send-b", "completed", "/topic/other", {
                sent: true
            }, 1), sendOperation("send-c", "completed", "/topic/comment", null, 1)]

        for (let i = 0; i < cases.length; ++i) {
            let completion = null
            verify(coordinator.startComment({
                topic: "/topic/comment",
                payloadText: "{}"
            }, function (response) {
                completion = response
            }))
            replyStart(i, cases[i])

            verify(completion && !completion.ok)
            verify(!coordinator.running)
        }
        compare(history.length, 0)
    }

    function test_legacy_write_policy_flags_do_not_block_runtime_admission() {
        coordinator.deliveryMutatingDiagnosticsEnabled = false
        let commentCompletion = null
        verify(coordinator.startComment({
            topic: "/topic/comment",
            payloadText: "{}"
        }, function (response) {
            commentCompletion = response
        }))
        compare(startRequests.length, 1)
        compare(startRequests[0].mutating_enabled, true)

        coordinator.invalidate("")
        coordinator.storageMutatingDiagnosticsEnabled = false
        coordinator.deliveryMutatingDiagnosticsEnabled = false
        let sharedCompletion = null
        verify(coordinator.startSharedIdl(sharedRequest("/topic/idl"), function (response) {
            sharedCompletion = response
        }))
        compare(startRequests.length, 2)
        compare(startRequests[1].mutating_enabled, true)
    }

    function test_duplicate_admission_is_rejected_during_both_stages() {
        let rejected = null
        verify(coordinator.startSharedIdl(sharedRequest("/topic/idl"), function () {}))
        verify(!coordinator.startComment({
            topic: "/topic/comment",
            payloadText: "{}"
        }, function (response) {
            rejected = response
        }))
        verify(rejected && !rejected.ok)
        compare(startRequests.length, 1)

        replyStart(0, uploadOperation("upload-1", "completed", {
            cid: "cid-idl",
            filename: "shared-idl.json"
        }, 1))
        compare(startRequests.length, 2)
        rejected = null
        verify(!coordinator.startSharedIdl(sharedRequest("/topic/other"), function (response) {
            rejected = response
        }))
        verify(rejected && !rejected.ok)
        compare(startRequests.length, 2)
    }

    function test_source_invalidation_rejects_late_start_completion() {
        let completion = null
        verify(coordinator.startSharedIdl(sharedRequest("/topic/idl"), function (response) {
            completion = response
        }))

        coordinator.storageAdapterInitialization = ({
                source_mode: "rest",
                inputs: {
                    rest_endpoint: "http://storage-b"
                }
            })

        verify(completion && !completion.ok)
        verify(!coordinator.running)
        replyStart(0, uploadOperation("late-upload", "completed", {
            cid: "late-cid",
            filename: "shared-idl.json"
        }, 1))
        compare(startRequests.length, 1)
        compare(history.length, 0)
    }

    function test_shared_idl_policy_invalidation_rejects_late_delivery() {
        let completions = 0
        let completion = null
        verify(coordinator.startSharedIdl(sharedRequest("/topic/idl"), function (response) {
            completions += 1
            completion = response
        }))
        replyStart(0, uploadOperation("upload-1", "completed", {
            cid: "cid-idl",
            filename: "shared-idl.json"
        }, 1))
        compare(startRequests.length, 2)

        verify(coordinator.invalidateKind("shared-idl", "policy changed"))
        compare(completions, 1)
        verify(completion && !completion.ok)
        verify(!coordinator.running)

        replyStart(1, sendOperation("late-send", "completed", "/topic/idl", {
            sent: true
        }, 1))
        compare(completions, 1)
        compare(history.length, 1)
    }

    function test_old_poll_cannot_mutate_replacement_workflow() {
        let oldCompletion = null
        let newCompletion = null
        verify(coordinator.startComment({
            topic: "/topic/old",
            payloadText: "{}"
        }, function (response) {
            oldCompletion = response
        }))
        replyStart(0, sendOperation("send-old", "running", "/topic/old", null, 1))
        verify(coordinator.poll() !== null)

        coordinator.invalidate("replace")
        verify(oldCompletion && !oldCompletion.ok)
        verify(coordinator.startComment({
            topic: "/topic/new",
            payloadText: "{}"
        }, function (response) {
            newCompletion = response
        }))
        replyStart(1, sendOperation("send-new", "running", "/topic/new", null, 1))

        replyStatus(0, sendOperation("send-old", "completed", "/topic/old", {
            sent: true
        }, 2))
        compare(newCompletion, null)
        verify(coordinator.running)
        compare(coordinator.deliverySession.view.active.operationId, "send-new")

        verify(coordinator.poll() !== null)
        replyStatus(1, sendOperation("send-new", "completed", "/topic/new", {
            sent: true
        }, 2))
        verify(newCompletion && newCompletion.ok)
    }

    function test_terminal_callback_can_admit_replacement_and_duplicate_is_ignored() {
        let firstCompletions = 0
        let replacement = null
        verify(coordinator.startComment({
            topic: "/topic/first",
            payloadText: "{}"
        }, function (response) {
            firstCompletions += 1
            if (response && response.ok === true) {
                verify(coordinator.startComment({
                    topic: "/topic/replacement",
                    payloadText: "{}"
                }, function (nextResponse) {
                    replacement = nextResponse
                }))
            }
        }))

        const firstTerminal = sendOperation("send-first", "completed", "/topic/first", {
            sent: true
        }, 1)
        replyStart(0, firstTerminal)

        compare(firstCompletions, 1)
        compare(startRequests.length, 2)
        verify(coordinator.running)
        compare(coordinator.deliverySession.view.active, null)
        replyStart(0, firstTerminal)
        compare(firstCompletions, 1)
        compare(startRequests.length, 2)

        replyStart(1, sendOperation("send-replacement", "completed", "/topic/replacement", {
            sent: true
        }, 1))
        verify(replacement && replacement.ok)
        verify(!coordinator.running)
    }

    function test_terminal_failure_statuses_finish_once() {
        const statuses = ["failed", "canceled", "timed_out"]
        for (let i = 0; i < statuses.length; ++i) {
            let completions = 0
            let last = null
            verify(coordinator.startComment({
                topic: "/topic/comment",
                payloadText: "{}"
            }, function (response) {
                completions += 1
                last = response
            }))
            replyStart(i, sendOperation("send-" + i, statuses[i], "/topic/comment", null, 1))

            compare(completions, 1)
            verify(last && !last.ok)
            verify(!coordinator.running)
        }
    }
}
