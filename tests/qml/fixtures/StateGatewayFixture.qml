import QtQuick

QtObject {
    property int callCount: 0
    property int requestCount: 0
    property int nextRequestId: 1
    property int rejectedResponseCount: 0
    property bool deferRequests: false
    property bool asyncSupported: true
    property bool basecampModules: false
    property string lastMethod: ""
    property var lastArgs: []
    property string lastLabel: ""
    property bool lastShowResult: false
    property var calls: []
    property var requests: []
    property var pendingRequests: []
    property var responses: ({})
    property var callResponses: ({})
    property var requestResponses: ({})
    property var defaultResponse: ({
        ok: true,
        value: {},
        text: "OK",
        error: ""
    })
    property bool busy: false
    property string statusText: ""
    property string resultTitle: ""
    property string resultText: ""
    property bool resultIsError: false
    property var resultValue: null
    property string resultOwner: ""
    property var history: []
    property string openedSection: ""
    property string openedSubSection: ""
    property int attachedRuntimeInvalidationCount: 0
    property int storageObservationCount: 0
    property int storageRefreshCount: 0
    property string lastStorageRefreshCid: ""
    property var storageRefreshCallback: null
    property bool deferStorageObservations: false
    property var storageObservationResponse: ({
        ok: true,
        value: {},
        text: "OK",
        error: ""
    })
    property var pendingStorageObservations: []

    function reset() {
        callCount = 0
        requestCount = 0
        nextRequestId = 1
        rejectedResponseCount = 0
        deferRequests = false
        asyncSupported = true
        basecampModules = false
        lastMethod = ""
        lastArgs = []
        lastLabel = ""
        lastShowResult = false
        calls = []
        requests = []
        pendingRequests = []
        responses = ({})
        callResponses = ({})
        requestResponses = ({})
        busy = false
        statusText = ""
        resultTitle = ""
        resultText = ""
        resultIsError = false
        resultValue = null
        resultOwner = ""
        history = []
        openedSection = ""
        openedSubSection = ""
        attachedRuntimeInvalidationCount = 0
        storageObservationCount = 0
        storageRefreshCount = 0
        lastStorageRefreshCid = ""
        storageRefreshCallback = null
        deferStorageObservations = false
        storageObservationResponse = ({
            ok: true,
            value: {},
            text: "OK",
            error: ""
        })
        pendingStorageObservations = []
    }

    function responseFor(method, primary) {
        if (primary[method] !== undefined) {
            return primary[method]
        }
        if (responses[method] !== undefined) {
            return responses[method]
        }
        return defaultResponse
    }

    function call(method, args, label) {
        callCount += 1
        lastMethod = String(method || "")
        lastArgs = args || []
        lastLabel = String(label || "")
        calls = calls.concat([{
            method: lastMethod,
            args: lastArgs,
            label: lastLabel
        }])
        return responseFor(lastMethod, callResponses)
    }

    function request(method, args, label, showResult, callback, acceptResponse) {
        const requestId = nextRequestId
        nextRequestId += 1
        requestCount += 1
        lastMethod = String(method || "")
        lastArgs = args || []
        lastLabel = String(label || "")
        lastShowResult = showResult === true
        requests = requests.concat([{
            method: lastMethod,
            args: lastArgs,
            label: lastLabel,
            showResult: lastShowResult,
            acceptResponse: acceptResponse
        }])
        calls = calls.concat([{
            method: lastMethod,
            args: lastArgs,
            label: lastLabel,
            showResult: lastShowResult
        }])
        const response = responseFor(lastMethod, requestResponses)
        if (deferRequests) {
            pendingRequests = pendingRequests.concat([{
                requestId: requestId,
                method: lastMethod,
                response: response,
                callback: callback,
                acceptResponse: acceptResponse
            }])
            return requestId
        }
        if (acceptResponse && !acceptResponse(response)) {
            rejectedResponseCount += 1
            return requestId
        }
        if (callback) {
            callback(response)
        }
        return response
    }

    function requestInspector(method, args, callback) {
        return request(method, args, "", false, callback)
    }

    function supportsAsync() {
        return asyncSupported
    }

    function prefersBasecampModules() {
        return basecampModules
    }

    function completeRequestAt(index, response) {
        const rows = pendingRequests.slice()
        if (index < 0 || index >= rows.length) {
            return false
        }
        const request = rows.splice(index, 1)[0]
        pendingRequests = rows
        const completedResponse = response === undefined ? request.response : response
        if (request.acceptResponse
                && !request.acceptResponse(completedResponse)) {
            rejectedResponseCount += 1
            return true
        }
        if (request.callback) {
            request.callback(completedResponse)
        }
        return true
    }

    function setBusy(value, label) {
        busy = value === true
        const labelText = String(label || "")
        if (busy && labelText.length) {
            statusText = labelText
        }
    }

    function setResult(title, text, isError, value, owner) {
        resultTitle = String(title || "")
        resultText = String(text || "")
        resultIsError = isError === true
        resultValue = value === undefined ? null : value
        resultOwner = String(owner || "")
    }

    function clearResult() {
        resultTitle = ""
        resultText = ""
        resultIsError = false
        resultValue = null
        resultOwner = ""
    }

    function appendOperationHistory(operation, detail) {
        history = history.concat([{
            operation: operation,
            detail: String(detail || "")
        }])
    }

    function invalidateAttachedRuntimeObservations() {
        attachedRuntimeInvalidationCount += 1
        return true
    }

    function openSettings(section, subSection) {
        openedSection = String(section || "")
        openedSubSection = String(subSection || "")
    }

    function observeStorage(callback) {
        storageObservationCount += 1
        if (deferStorageObservations) {
            pendingStorageObservations = pendingStorageObservations.concat([callback])
            return {
                ok: true,
                pending: true,
                text: "",
                error: ""
            }
        }
        if (callback) {
            callback(storageObservationResponse)
        }
        return storageObservationResponse
    }

    function refreshStorageObservations(cid) {
        storageRefreshCount += 1
        lastStorageRefreshCid = String(cid || "")
        return typeof storageRefreshCallback === "function"
            ? storageRefreshCallback(lastStorageRefreshCid)
            : null
    }

    function completeStorageObservationAt(index, response) {
        const callbacks = pendingStorageObservations.slice(0)
        if (index < 0 || index >= callbacks.length) {
            return false
        }
        const callback = callbacks.splice(index, 1)[0]
        pendingStorageObservations = callbacks
        if (callback) {
            callback(response === undefined ? storageObservationResponse : response)
        }
        return true
    }

    function valueText(value) {
        return String(value)
    }
}
