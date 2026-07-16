import QtQml
import ".." as State

QtObject {
    id: root

    required property var gateway
    property var storageAdapterInitialization: ({
            source_mode: "",
            inputs: ({})
        })
    property var deliveryAdapterInitialization: ({
            source_mode: "",
            inputs: ({})
        })
    property bool storageMutatingDiagnosticsEnabled: true
    property bool deliveryMutatingDiagnosticsEnabled: true

    property int workflowGeneration: 0
    property string workflowKind: ""
    property string phase: ""
    property var intent: null
    property var completion: null

    readonly property bool running: workflowKind.length > 0

    property State.SourceOperationSession uploadSession: State.SourceOperationSession {
        id: uploadOperations

        gateway: root.gateway
        domain: "storage"
        adapterInitialization: root.storageAdapterInitialization
        mutatingDiagnosticsEnabled: root.storageMutatingDiagnosticsEnabled
        defaultLabel: qsTr("Upload shared IDL")
        busyError: qsTr("A Social write is already running.")
        operationValidator: function (operation) {
            return root.uploadOperationProblem(operation).length === 0
        }

        onTerminalOperation: function (operation) {
            root.completeUpload(operation)
        }
    }

    property State.SourceOperationSession deliverySession: State.SourceOperationSession {
        id: deliveryOperations

        gateway: root.gateway
        domain: "delivery"
        adapterInitialization: root.deliveryAdapterInitialization
        mutatingDiagnosticsEnabled: root.deliveryMutatingDiagnosticsEnabled
        defaultLabel: qsTr("Social message")
        busyError: qsTr("A Social write is already running.")
        operationValidator: function (operation) {
            return root.deliveryOperationProblem(operation).length === 0
        }

        onTerminalOperation: function (operation) {
            root.completeDelivery(operation)
        }
    }

    onStorageAdapterInitializationChanged: invalidate(qsTr("Storage source changed during Social write."))
    onDeliveryAdapterInitializationChanged: invalidate(qsTr("Delivery source changed during Social write."))

    function startComment(request, onComplete) {
        const value = request && typeof request === "object" ? request : ({})
        const topic = String(value.topic || "").trim()
        const payloadText = String(value.payloadText || "")
        if (!topic.length || !payloadText.length) {
            return reject(onComplete, qsTr("Comment topic and payload are required."))
        }
        return beginWorkflow("comment", {
            topic: topic,
            payloadText: payloadText,
            deliveryLabel: String(value.label || qsTr("Post comment")),
            deliveryAdapter: frozenValue(deliveryAdapterInitialization)
        }, onComplete, "delivery")
    }

    function startSharedIdl(request, onComplete) {
        const value = request && typeof request === "object" ? request : ({})
        const filename = String(value.filename || "").trim()
        const topic = String(value.topic || "").trim()
        const artifact = frozenValue(value.artifact)
        const message = frozenValue(value.message)
        if (!filename.length || !topic.length || !artifact || !message) {
            return reject(onComplete, qsTr("Shared IDL upload input is incomplete."))
        }
        return beginWorkflow("shared-idl", {
            filename: filename,
            artifact: artifact,
            blockSize: Math.max(1, Number(value.blockSize || 65536)),
            topic: topic,
            message: message,
            uploadLabel: String(value.uploadLabel || qsTr("Upload shared IDL")),
            deliveryLabel: String(value.deliveryLabel || qsTr("Share IDL")),
            storageAdapter: frozenValue(storageAdapterInitialization),
            deliveryAdapter: frozenValue(deliveryAdapterInitialization)
        }, onComplete, "upload")
    }

    function beginWorkflow(kind, frozenIntent, onComplete, initialPhase) {
        if (running) {
            return reject(onComplete, qsTr("A Social write is already running."))
        }
        workflowGeneration += 1
        const generation = workflowGeneration
        workflowKind = String(kind || "")
        phase = String(initialPhase || "")
        intent = frozenIntent || null
        completion = typeof onComplete === "function" ? onComplete : null
        uploadOperations.clearActive()
        deliveryOperations.clearActive()
        const admitted = phase === "upload" ? startUpload(generation) : startDelivery(generation)
        if (!admitted && generation === workflowGeneration && running) {
            finish(failure(qsTr("Social write could not be started.")))
        }
        return admitted
    }

    function startUpload(generation) {
        if (!isCurrent(generation, "upload") || !intent) {
            return false
        }
        const started = uploadOperations.start("storageUploadPayload", [intent.filename, intent.artifact, intent.blockSize], intent.uploadLabel, function (response, operation) {
            acceptStart(generation, "upload", response, operation)
        })
        return admitted(started)
    }

    function startDelivery(generation) {
        if (!isCurrent(generation, "delivery") || !intent) {
            return false
        }
        const started = deliveryOperations.start("deliverySend", [intent.topic, intent.payloadText], intent.deliveryLabel, function (response, operation) {
            acceptStart(generation, "delivery", response, operation)
        })
        return admitted(started)
    }

    function acceptStart(generation, expectedPhase, response, operation) {
        if (!isCurrent(generation, expectedPhase)) {
            return
        }
        if (!response || response.ok !== true) {
            finish(failure(String(response && response.error || qsTr("Social write failed to start."))))
            return
        }
        const problem = expectedPhase === "upload" ? uploadOperationProblem(operation) : deliveryOperationProblem(operation)
        if (problem.length) {
            finish(failure(problem))
        }
    }

    function poll() {
        const generation = workflowGeneration
        if (phase === "upload" && uploadOperations.view.running) {
            return uploadOperations.poll(false, function (response, operation) {
                return inspectPoll(generation, "upload", response, operation)
            })
        }
        if (phase === "delivery" && deliveryOperations.view.running) {
            return deliveryOperations.poll(false, function (response, operation) {
                return inspectPoll(generation, "delivery", response, operation)
            })
        }
        return null
    }

    function inspectPoll(generation, expectedPhase, response, previousOperation) {
        if (!isCurrent(generation, expectedPhase) || !response || response.ok !== true) {
            return false
        }
        const operation = response.value || null
        if (String(operation && operation.operationId || "") !== String(previousOperation && previousOperation.operationId || "")) {
            return false
        }
        const problem = expectedPhase === "upload" ? uploadOperationProblem(operation) : deliveryOperationProblem(operation)
        if (!problem.length) {
            return false
        }
        finish(failure(problem))
        return true
    }

    function completeUpload(operation) {
        if (!isCurrent(workflowGeneration, "upload") || !matchesActive(uploadOperations, operation)) {
            return
        }
        const problem = uploadOperationProblem(operation)
        if (problem.length) {
            finish(failure(problem))
            return
        }
        if (String(operation.status || "") !== "completed") {
            finish(failure(String(operation.error || qsTr("Shared IDL upload did not complete."))))
            return
        }

        const result = operation.result
        const cid = String(result && result.cid || "").trim()
        const message = copyMap(intent.message)
        message.idl_cid = cid
        message.storage = storageMetadata(cid, intent.storageAdapter)
        let payloadText = ""
        try {
            payloadText = JSON.stringify(message)
        } catch (error) {
            finish(failure(qsTr("Shared IDL message could not be serialized.")))
            return
        }

        const nextIntent = copyMap(intent)
        nextIntent.payloadText = payloadText
        nextIntent.cid = cid
        intent = nextIntent
        phase = "delivery"
        uploadOperations.clearActive()
        const generation = workflowGeneration
        if (!startDelivery(generation) && isCurrent(generation, "delivery")) {
            finish(failure(qsTr("Shared IDL delivery could not be started.")))
        }
    }

    function completeDelivery(operation) {
        if (!isCurrent(workflowGeneration, "delivery") || !matchesActive(deliveryOperations, operation)) {
            return
        }
        const problem = deliveryOperationProblem(operation)
        if (problem.length) {
            finish(failure(problem))
            return
        }
        if (String(operation.status || "") !== "completed") {
            finish(failure(String(operation.error || qsTr("Social message did not complete."))))
            return
        }
        finish({
            ok: true,
            value: operation.result,
            operation: operation,
            cid: String(intent && intent.cid || ""),
            text: "",
            error: ""
        })
    }

    function uploadOperationProblem(operation) {
        const common = operationProblem(operation, "storage", "storageUploadPayload")
        if (common.length || !intent) {
            return common || qsTr("Shared IDL upload has no active intent.")
        }
        if (String(operation.context && operation.context.filename || "") !== String(intent.filename || "")) {
            return qsTr("Shared IDL upload returned a different filename context.")
        }
        if (String(operation.status || "") !== "completed") {
            return ""
        }
        const result = operation.result && typeof operation.result === "object" && !Array.isArray(operation.result) ? operation.result : null
        if (!result) {
            return qsTr("Shared IDL upload returned no result.")
        }
        if (String(result.filename || "") !== String(intent.filename || "")) {
            return qsTr("Shared IDL upload returned a different filename.")
        }
        if (!String(result.cid || "").trim().length) {
            return qsTr("Shared IDL upload returned no CID.")
        }
        return ""
    }

    function deliveryOperationProblem(operation) {
        const common = operationProblem(operation, "delivery", "deliverySend")
        if (common.length || !intent) {
            return common || qsTr("Social delivery has no active intent.")
        }
        if (String(operation.context && operation.context.contentTopic || "") !== String(intent.topic || "")) {
            return qsTr("Social delivery returned a different topic context.")
        }
        if (String(operation.status || "") === "completed" && (operation.result === undefined || operation.result === null)) {
            return qsTr("Social delivery returned no completion result.")
        }
        return ""
    }

    function operationProblem(operation, domain, method) {
        const value = operation && typeof operation === "object" ? operation : null
        if (!value || !String(value.operationId || "").length) {
            return qsTr("Social write returned no operation identity.")
        }
        if (String(value.domain || "") !== String(domain || "") || String(value.method || "") !== String(method || "")) {
            return qsTr("Social write returned invalid operation identity.")
        }
        const status = String(value.status || "")
        if (!knownStatus(status)) {
            return qsTr("Social write returned unsupported status %1.").arg(status || qsTr("unknown"))
        }
        return ""
    }

    function knownStatus(status) {
        const value = String(status || "")
        return value === "running" || value === "awaiting_external" || value === "canceling" || value === "completed" || value === "failed" || value === "canceled" || value === "timed_out"
    }

    function matchesActive(session, operation) {
        const activeId = String(session.view.active && session.view.active.operationId || "")
        return activeId.length > 0 && activeId === String(operation && operation.operationId || "")
    }

    function storageMetadata(cid, adapter) {
        const initialization = adapter && typeof adapter === "object" ? adapter : ({})
        const sourceMode = String(initialization.source_mode || "")
        const inputs = initialization.inputs && typeof initialization.inputs === "object" ? initialization.inputs : ({})
        const metadata = {
            cid: String(cid || ""),
            provider: "logos_storage",
            source_mode: sourceMode
        }
        if (sourceMode === "rest" && String(inputs.rest_endpoint || "").trim().length) {
            metadata.endpoint = String(inputs.rest_endpoint).trim()
        }
        return metadata
    }

    function invalidateKind(kind, reason) {
        if (workflowKind !== String(kind || "")) {
            return false
        }
        invalidate(reason)
        return true
    }

    function invalidate(reason) {
        const callback = completion
        const wasRunning = running
        workflowGeneration += 1
        workflowKind = ""
        phase = ""
        intent = null
        completion = null
        uploadOperations.clearActive()
        deliveryOperations.clearActive()
        if (wasRunning && typeof callback === "function") {
            callback(failure(String(reason || qsTr("Social write was invalidated."))))
        }
    }

    function finish(response) {
        if (!running) {
            return false
        }
        const callback = completion
        workflowKind = ""
        phase = ""
        intent = null
        completion = null
        uploadOperations.clearActive()
        deliveryOperations.clearActive()
        if (typeof callback === "function") {
            callback(response)
        }
        return true
    }

    function isCurrent(generation, expectedPhase) {
        return running && Number(generation) === workflowGeneration && phase === String(expectedPhase || "")
    }

    function admitted(value) {
        return value !== null && value !== false && !(value && typeof value === "object" && value.ok === false)
    }

    function reject(callback, message) {
        if (typeof callback === "function") {
            callback(failure(message))
        }
        return false
    }

    function failure(message) {
        return {
            ok: false,
            value: null,
            text: "",
            error: String(message || qsTr("Social write failed."))
        }
    }

    function frozenValue(value) {
        if (value === undefined || value === null) {
            return null
        }
        try {
            return JSON.parse(JSON.stringify(value))
        } catch (error) {
            return null
        }
    }

    function copyMap(value) {
        const next = ({})
        const source = value && typeof value === "object" ? value : ({})
        const keys = Object.keys(source)
        for (let i = 0; i < keys.length; ++i) {
            next[keys[i]] = source[keys[i]]
        }
        return next
    }
}
