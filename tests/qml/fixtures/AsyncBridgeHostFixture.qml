import QtQuick

QtObject {
    id: root

    signal moduleCallFinished(int requestId, string responseJson)

    property int callCount: 0
    property string lastModule: ""
    property string lastMethod: ""
    property var lastArgs: []
    property var calls: []
    property var responses: ({})
    property var defaultResponse: ({
        ok: true,
        value: {},
        text: "OK",
        error: ""
    })
    property bool strictUnexpectedCalls: false
    property bool deferAsyncRequests: false
    property var pendingAsyncRequests: []

    function reset() {
        callCount = 0
        lastModule = ""
        lastMethod = ""
        lastArgs = []
        calls = []
        responses = ({})
        strictUnexpectedCalls = false
        deferAsyncRequests = false
        pendingAsyncRequests = []
    }

    function callModuleJson(moduleName, method, argsJson) {
        const call = recordCall(moduleName, method, argsJson)
        return call.responseJson
    }

    function callModuleJsonAsync(requestId, moduleName, method, argsJson) {
        const call = recordCall(moduleName, method, argsJson)
        const request = {
            requestId: requestId,
            responseJson: call.responseJson
        }
        if (deferAsyncRequests) {
            pendingAsyncRequests = pendingAsyncRequests.concat([request])
            return
        }
        Qt.callLater(function () {
            root.moduleCallFinished(request.requestId, request.responseJson)
        })
    }

    function completeAsyncAt(index, response) {
        const requests = pendingAsyncRequests.slice()
        if (index < 0 || index >= requests.length) {
            return false
        }
        const request = requests.splice(index, 1)[0]
        pendingAsyncRequests = requests
        const responseJson = response === undefined
            ? request.responseJson : JSON.stringify(response)
        moduleCallFinished(request.requestId, responseJson)
        return true
    }

    function recordCall(moduleName, method, argsJson) {
        callCount += 1
        lastModule = String(moduleName || "")
        lastMethod = String(method || "")
        lastArgs = JSON.parse(String(argsJson || "[]"))
        calls = calls.concat([{
            module: lastModule,
            method: lastMethod,
            args: lastArgs
        }])
        if (responses[lastMethod] !== undefined) {
            const response = responses[lastMethod]
            return {
                responseJson: JSON.stringify(
                    typeof response === "function" ? response(lastArgs) : response)
            }
        }
        if (strictUnexpectedCalls) {
            throw new Error("Unexpected bridge call: " + lastMethod)
        }
        return { responseJson: JSON.stringify(defaultResponse) }
    }
}
