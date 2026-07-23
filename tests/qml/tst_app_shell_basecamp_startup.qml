pragma ComponentBehavior: Bound

import QtQuick
import QtQuick.Controls.Basic
import QtTest
import "../../qml"

TestCase {
    id: testRoot

    name: "AppShellBasecampStartup"
    when: windowShown
    width: 1180
    height: 820

    QtObject {
        id: basecampHost

        property int syncCallCount: 0
        property int asyncCallCount: 0
        property int nextToken: 1
        property var responses: ({})

        function reset() {
            syncCallCount = 0
            asyncCallCount = 0
            nextToken = 1
            responses = ({})
        }

        function inspectorResponse(value) {
            return JSON.stringify({
                ok: true,
                value: value,
                text: "",
                error: ""
            })
        }

        function controlResponse(value) {
            return JSON.stringify(JSON.stringify({
                ok: true,
                value: value,
                text: "",
                error: ""
            }))
        }

        function responseFor(method) {
            switch (String(method || "")) {
            case "sourcePolicy":
                return { defaults: ({}) }
            case "loadSettingsState":
                return ({})
            case "capabilityRegistryReport":
                return { schema_version: 1, capabilities: [] }
            case "loadBackupCatalog":
                return { version: 1, entries: [] }
            case "loadIdlState":
                return { version: 1, idls: [], account_idl_selections: ({}) }
            case "localNodesStatus":
                return { nodes: [] }
            default:
                return ({})
            }
        }

        function callModule(_moduleName, _method, _args) {
            syncCallCount += 1
            return JSON.stringify({ error: "unexpected synchronous Basecamp call" })
        }

        function callModuleAsync(moduleName, method, args, callback, _timeoutMs) {
            asyncCallCount += 1
            if (String(moduleName || "") === "logos_inspector"
                    && String(method || "") === "logosInspectorOwnsRuntimeModuleEvents") {
                callback(JSON.stringify(true))
                return
            }
            if (String(moduleName || "") !== "logos_inspector") {
                callback(JSON.stringify({}))
                return
            }
            if (String(method || "") === "callAsync") {
                const token = "test-token-" + nextToken
                nextToken += 1
                const requestedMethod = String(args[1] || "")
                const next = Object.assign({}, responses)
                next[token] = inspectorResponse(responseFor(requestedMethod))
                responses = next
                callback(controlResponse({
                    schema: "logos-inspector-async-bridge/v1",
                    correlationId: String(args[0] || ""),
                    token: token
                }))
                return
            }
            if (String(method || "") === "pollAsync") {
                const token = String(args[0] || "")
                callback(controlResponse({
                    schema: "logos-inspector-async-bridge/v1",
                    token: token,
                    status: "ready",
                    responseJson: responses[token]
                }))
                return
            }
            callback(controlResponse({}))
        }
    }

    ApplicationWindow {
        id: testWindow

        visible: true
        width: testRoot.width
        height: testRoot.height

        AppShell {
            id: shell

            anchors.fill: parent
            bridgeHost: basecampHost
        }
    }

    function test_startup_uses_only_basecamp_async_calls() {
        const model = findChild(shell, "appModel")
        verify(model !== null)

        tryVerify(function () {
            return model.sourceRouting.sourcePolicyLoaded
                && model.settingsStateLoaded
                && model.capabilityRegistryLoaded
                && model.backupCatalogLoaded
                && model.idlRegistry.loaded
        })
        compare(basecampHost.syncCallCount, 0)
        verify(basecampHost.asyncCallCount > 0)
    }
}
