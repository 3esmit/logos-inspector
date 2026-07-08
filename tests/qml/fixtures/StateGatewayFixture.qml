import QtQuick

QtObject {
    property int callCount: 0
    property int requestCount: 0
    property string lastMethod: ""
    property var lastArgs: []
    property string lastLabel: ""
    property bool lastShowResult: false
    property var calls: []
    property var requests: []
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
    property var history: []
    property string openedSection: ""
    property string openedSubSection: ""

    function reset() {
        callCount = 0
        requestCount = 0
        lastMethod = ""
        lastArgs = []
        lastLabel = ""
        lastShowResult = false
        calls = []
        requests = []
        responses = ({})
        callResponses = ({})
        requestResponses = ({})
        busy = false
        statusText = ""
        resultTitle = ""
        resultText = ""
        resultIsError = false
        resultValue = null
        history = []
        openedSection = ""
        openedSubSection = ""
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

    function request(method, args, label, showResult, callback) {
        requestCount += 1
        lastMethod = String(method || "")
        lastArgs = args || []
        lastLabel = String(label || "")
        lastShowResult = showResult === true
        requests = requests.concat([{
            method: lastMethod,
            args: lastArgs,
            label: lastLabel,
            showResult: lastShowResult
        }])
        calls = calls.concat([{
            method: lastMethod,
            args: lastArgs,
            label: lastLabel,
            showResult: lastShowResult
        }])
        const response = responseFor(lastMethod, requestResponses)
        if (callback) {
            callback(response)
        }
        return response
    }

    function setBusy(value, label) {
        busy = value === true
        const labelText = String(label || "")
        if (busy && labelText.length) {
            statusText = labelText
        }
    }

    function setResult(title, text, isError, value) {
        resultTitle = String(title || "")
        resultText = String(text || "")
        resultIsError = isError === true
        resultValue = value === undefined ? null : value
    }

    function clearResult() {
        resultTitle = ""
        resultText = ""
        resultIsError = false
        resultValue = null
    }

    function appendOperationHistory(operation, detail) {
        history = history.concat([{
            operation: operation,
            detail: String(detail || "")
        }])
    }

    function openSettings(section, subSection) {
        openedSection = String(section || "")
        openedSubSection = String(subSection || "")
    }

    function valueText(value) {
        return String(value)
    }
}
