import QtQuick

QtObject {
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

    function reset() {
        callCount = 0
        lastModule = ""
        lastMethod = ""
        lastArgs = []
        calls = []
        responses = ({})
        strictUnexpectedCalls = false
    }

    function callModuleJson(moduleName, method, argsJson) {
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
            return JSON.stringify(responses[lastMethod])
        }
        if (strictUnexpectedCalls) {
            throw new Error("Unexpected bridge call: " + lastMethod)
        }
        return JSON.stringify(defaultResponse)
    }
}
