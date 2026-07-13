import QtQuick
import QtTest
import "../../qml/state"
import "fixtures"

TestCase {
    id: testRoot

    name: "LocalNodesState"

    StateGatewayFixture {
        id: gateway
    }

    LocalNodesState {
        id: state

        gateway: gateway
        networkProfile: "default"
        busy: gateway.busy
    }

    function init() {
        gateway.reset()

        state.networkProfile = "default"
        state.report = null
        state.error = ""
        state.operations = []
        state.revision = 0
        state.devnets = []
        state.clearActionDraft()
    }

    function sampleReport() {
        return {
            profile: "local",
            mode: "localnet",
            available_network_actions: ["new_network", "load_network", "reset_network", "delete_network"],
            available_runtime_actions: ["stop_runtime"],
            primary_problem: "missing_sequencer_binary",
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
            },
            runtime: {
                ownership: "inspector_managed",
                run_state: "running",
                modules_dir: "/tmp/modules",
                detail: "Inspector-managed logoscore daemon process is running"
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
        state.networkProfile = "local"
        state.report = null
        state.revision += 1

        verify(!state.localMode())
        compare(state.networkActions().length, 0)
        verify(!state.networkActionEnabled("new_network"))

        state.report = ({
            profile: "default",
            mode: "public_testnet",
            available_network_actions: []
        })
        state.revision += 1

        verify(!state.networkActionEnabled("new_network"))
        verify(!state.networkActionEnabled("delete_network"))

        state.report = ({
            profile: "local",
            mode: "localnet",
            active_devnet: "devnet",
            available_network_actions: ["new_network", "load_network", "reset_network", "delete_network"]
        })
        state.revision += 1

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
        compare(state.modeLabel(), "Local Devnet")

        gateway.busy = true
        verify(!state.actionEnabled("bedrock", "start"))
        gateway.busy = false

        compare(state.actionLabel("reset_network"), "Reset Local Devnet")
        verify(state.runtimeActionEnabled("stop_runtime"))
        compare(state.runtimeState(), "running")
        compare(state.runtimeModulesDir(), "/tmp/modules")
        compare(state.nodeByKind("sequencer").label, "Sequencer")
        compare(state.toolProblem(), "sequencer_service not found. Local sequencer start requires a configured binary.")
    }

    function test_node_action_draft_owns_confirmation_facts() {
        state.report = sampleReport()
        state.revision += 1

        state.beginNodeAction("start", "bedrock")

        compare(state.pendingAction, "start")
        compare(state.pendingNode, "bedrock")
        compare(state.pendingNetworkId, "")
        compare(state.pendingWorkspace, "")
        compare(state.actionDraftTitle(), "Start Bedrock")
        verify(state.actionDraftMessage().indexOf("This starts Bedrock") === 0)
    }

    function test_network_action_draft_runs_pending_request() {
        state.networkProfile = "local"
        state.beginNetworkAction("load_network", "", "/tmp/local-devnet")
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

        compare(state.actionDraftTitle(), "Load Local Devnet")
        compare(state.actionDraftMessage(), "This loads the Local Devnet manifest from /tmp/local-devnet and sets it as Active Devnet.")

        state.runPendingAction()

        compare(state.pendingAction, "")
        compare(gateway.calls[0].args[1].action, "load_network")
        compare(gateway.calls[0].args[1].workspace_path, "/tmp/local-devnet")
        compare(gateway.history[0].operation.label, "Load Local Devnet")
    }

    function test_runtime_action_draft_serializes_managed_runtime_paths() {
        state.networkProfile = "local"
        state.beginRuntimeAction("start_runtime", "/tmp/modules", "/tmp/logoscore")
        gateway.responses = ({
            localNodesAction: {
                ok: true,
                value: sampleReport(),
                text: "OK",
                error: ""
            },
            localDevnetList: {
                ok: true,
                value: { devnets: [] },
                text: "OK",
                error: ""
            }
        })

        compare(state.actionDraftTitle(), "Start Local Runtime")
        compare(state.actionDraftMessage(), "This starts an Inspector-managed LogosCore runtime using modules from /tmp/modules.")

        state.runPendingAction()

        compare(gateway.calls[0].args[1].action, "start_runtime")
        compare(gateway.calls[0].args[1].runtime_modules_dir, "/tmp/modules")
        compare(gateway.calls[0].args[1].runtime_binary_path, "/tmp/logoscore")
        compare(gateway.history[0].operation.label, "Start Local Runtime")
    }
}
