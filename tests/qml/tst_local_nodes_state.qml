import QtQuick
import QtTest
import "../../qml/state"

TestCase {
    id: testRoot

    name: "LocalNodesState"

    QtObject {
        id: gateway

        property int requestCount: 0
        property string lastMethod: ""
        property var lastArgs: []
        property string lastLabel: ""
        property bool lastShowResult: false
        property var calls: []
        property var responses: ({})
        property bool busy: false
        property string statusText: ""
        property string resultTitle: ""
        property string resultText: ""
        property bool resultIsError: false
        property var resultValue: null
        property var history: []

        function request(method, args, label, showResult, callback) {
            requestCount += 1
            lastMethod = String(method || "")
            lastArgs = args || []
            lastLabel = String(label || "")
            lastShowResult = showResult === true
            calls = calls.concat([{
                method: lastMethod,
                args: lastArgs,
                label: lastLabel,
                showResult: lastShowResult
            }])
            const response = responses[lastMethod] !== undefined ? responses[lastMethod] : {
                ok: true,
                value: {},
                text: "OK",
                error: ""
            }
            callback(response)
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

        function appendOperationHistory(operation, detail) {
            history = history.concat([{
                operation: operation,
                detail: String(detail || "")
            }])
        }
    }

    LocalNodesState {
        id: state

        gateway: gateway
        networkProfile: "default"
        busy: gateway.busy
    }

    function init() {
        gateway.requestCount = 0
        gateway.lastMethod = ""
        gateway.lastArgs = []
        gateway.lastLabel = ""
        gateway.lastShowResult = false
        gateway.calls = []
        gateway.responses = ({})
        gateway.busy = false
        gateway.statusText = ""
        gateway.resultTitle = ""
        gateway.resultText = ""
        gateway.resultIsError = false
        gateway.resultValue = null
        gateway.history = []

        state.networkProfile = "default"
        state.report = null
        state.error = ""
        state.operations = []
        state.revision = 0
        state.devnets = []
    }

    function sampleReport() {
        return {
            active_devnet: "devnet",
            workspace_root: "/tmp/logos-devnet",
            summary: {
                total: 2,
                running: 1,
                needs_configuration: 0
            },
            nodes: [
                {
                    key: "bedrock",
                    label: "Bedrock",
                    available_actions: ["start", "stop"],
                    install_state: "installed",
                    run_state: "running"
                },
                {
                    key: "sequencer",
                    label: "Sequencer",
                    available_actions: ["install"],
                    install_state: "needs_configuration",
                    run_state: "stopped"
                }
            ],
            operations: [
                {
                    action: "start",
                    node: "bedrock",
                    status: "started",
                    detail: "ok"
                }
            ],
            tools: {
                logoscore: {
                    available: true
                }
            }
        }
    }

    function test_refresh_updates_report_and_operations() {
        gateway.responses = ({
            localNodesStatus: {
                ok: true,
                value: sampleReport(),
                text: "OK",
                error: ""
            }
        })

        state.refresh(true)

        compare(gateway.requestCount, 1)
        compare(gateway.lastMethod, "localNodesStatus")
        compare(gateway.lastArgs[0], "default")
        verify(gateway.lastShowResult)
        compare(state.report.summary.total, 2)
        compare(state.operations.length, 1)
        compare(state.error, "")
        compare(state.revision, 1)
        compare(state.summaryText(), "1/2 running")
    }

    function test_refresh_records_error() {
        gateway.responses = ({
            localNodesStatus: {
                ok: false,
                value: null,
                text: "",
                error: "status failed"
            }
        })

        state.refresh(false)

        compare(state.report, null)
        compare(state.error, "status failed")
        compare(state.revision, 1)
    }

    function test_run_action_dispatches_confirmation_token_and_history() {
        state.networkProfile = "local"
        gateway.responses = ({
            localNodesAction: {
                ok: true,
                value: sampleReport(),
                text: "OK",
                error: ""
            },
            localDevnetList: {
                ok: true,
                value: {
                    devnets: ["devnet"]
                },
                text: "OK",
                error: ""
            }
        })

        state.runAction("start", "bedrock", "", "", "Start Bedrock")

        compare(gateway.requestCount, 2)
        compare(gateway.calls[0].method, "localNodesAction")
        compare(gateway.calls[0].args[0], "local")
        compare(gateway.calls[0].args[1].action, "start")
        compare(gateway.calls[0].args[1].node, "bedrock")
        compare(gateway.calls[0].args[2], "confirm-local-node-action")
        compare(gateway.calls[1].method, "localDevnetList")
        compare(state.operations.length, 1)
        compare(state.devnets.length, 1)
        compare(gateway.busy, false)
        compare(gateway.statusText, "Start Bedrock")
        compare(gateway.history.length, 1)
        compare(gateway.history[0].operation.label, "Start Bedrock")
        compare(gateway.history[0].operation.status, "completed")
        compare(gateway.history[0].detail, "ok")
    }

    function test_run_action_rejects_when_busy() {
        gateway.busy = true

        const response = state.runAction("start", "bedrock", "", "", "Start Bedrock")

        compare(response, null)
        compare(gateway.requestCount, 0)
        compare(gateway.resultTitle, "Local nodes")
        verify(gateway.resultIsError)
    }

    function test_failed_action_appends_operation_history() {
        state.networkProfile = "local"
        gateway.responses = ({
            localNodesAction: {
                ok: false,
                value: null,
                text: "",
                error: "start failed"
            }
        })

        state.runAction("start", "bedrock", "", "", "Start Bedrock")

        compare(state.error, "start failed")
        compare(state.operations.length, 1)
        compare(state.operations[0].status, "failed")
        compare(gateway.history.length, 1)
        compare(gateway.history[0].operation.status, "failed")
    }

    function test_network_actions_follow_profile_mode() {
        state.report = ({ active_devnet: "devnet" })
        state.revision += 1

        verify(!state.networkActionEnabled("new_network"))
        verify(!state.networkActionEnabled("delete_network"))

        state.networkProfile = "local"

        verify(state.networkActionEnabled("new_network"))
        verify(state.networkActionEnabled("reset_network"))
        verify(state.networkActionEnabled("delete_network"))
    }

    function test_node_actions_and_tool_problem_are_derived_from_state() {
        state.networkProfile = "local"
        state.report = sampleReport()
        state.revision += 1

        verify(state.actionEnabled("bedrock", "start"))
        verify(!state.actionEnabled("bedrock", "purge"))
        compare(state.modeLabel(), "Localnet")

        gateway.busy = true
        verify(!state.actionEnabled("bedrock", "start"))
        gateway.busy = false

        compare(state.actionLabel("reset_network"), "Reset network")
        compare(state.nodeByKind("sequencer").label, "Sequencer")
        compare(state.toolProblem(), "sequencer_service not found. Local sequencer start requires a configured binary.")
    }
}
