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
        state.statusLoading = false
        state.statusGeneration = 0
        state.statusRefreshDeferred = false
        state.statusRefreshShowResult = false
        state.statusRefreshIncludePackageCatalog = false
        state.devnets = []
        state.packageCatalog = null
        state.packageCatalogError = ""
        state.packageCatalogLoading = false
        state.packageCatalogGeneration = 0
        state.observedNodes = ({})
        state.clearActionDraft()
    }

    function samplePackageCatalog(installed) {
        return {
            modules_dir: "/tmp/modules",
            package: {
                name: "lez_indexer_module",
                versions: [{
                    version: "1.1.0",
                    released_at: "2026-07-17T12:00:00Z",
                    root_hash: "root-hash-1.1.0"
                }, {
                    version: "1.0.0",
                    released_at: "2026-07-01T12:00:00Z",
                    root_hash: "root-hash-1.0.0-repack"
                }, {
                    version: "1.0.0",
                    released_at: "2026-06-01T12:00:00Z",
                    root_hash: "root-hash-1.0.0"
                }]
            },
            installed: installed === undefined ? null : installed
        }
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

    function testnetReport() {
        const nodes = ["bedrock", "indexer", "storage", "messaging"].map(function (kind) {
            return {
                key: kind,
                label: kind,
                available_actions: [],
                install_state: "needs_configuration",
                run_state: "not_initialized",
                ownership: "external",
                process_id: null
            }
        })
        return {
            profile: "default",
            mode: "public_testnet",
            available_network_actions: [],
            available_runtime_actions: ["start_runtime"],
            active_devnet: "logos-testnet",
            workspace_root: "/tmp/logos-testnet",
            summary: {
                total: nodes.length,
                installed: 0,
                running: 0,
                needs_configuration: nodes.length
            },
            nodes: nodes,
            operations: [],
            runtime: {
                ownership: "external",
                run_state: "not_configured"
            }
        }
    }

    function managedTestnetReport() {
        const report = testnetReport()
        report.available_runtime_actions = ["stop_runtime"]
        report.runtime = {
            ownership: "inspector_managed",
            run_state: "running"
        }
        report.nodes = report.nodes.map(function (node) {
            return Object.assign({}, node, {
                install_state: "installed",
                run_state: "running",
                ownership: "inspector_managed",
                process_id: node.key === "indexer" ? 42 : null
            })
        })
        report.summary = {
            total: report.nodes.length,
            installed: report.nodes.length,
            running: report.nodes.length,
            needs_configuration: 0
        }
        return report
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

    function test_refresh_coalesces_and_applies_newest_requested_status() {
        gateway.deferRequests = true
        const first = sampleReport()
        first.summary = { total: 2, running: 1, needs_configuration: 0 }
        const second = sampleReport()
        second.summary = { total: 2, running: 2, needs_configuration: 0 }

        state.refresh(false)
        state.refresh(true)

        compare(gateway.requestCount, 1)
        verify(state.statusLoading)
        verify(state.statusRefreshDeferred)

        verify(gateway.completeRequestAt(0, {
            ok: true, value: first, text: "OK", error: ""
        }))

        compare(gateway.requestCount, 2)
        verify(state.statusLoading)
        verify(!state.statusRefreshDeferred)
        verify(gateway.requests[1].showResult)

        verify(gateway.completeRequestAt(0, {
            ok: true, value: second, text: "OK", error: ""
        }))

        verify(!state.statusLoading)
        compare(state.report.summary.running, 2)
    }

    function test_action_invalidates_older_status_response() {
        state.networkProfile = "local"
        gateway.deferRequests = true
        const actionReport = sampleReport()
        actionReport.summary = { total: 2, running: 2, needs_configuration: 0 }
        const staleReport = sampleReport()
        staleReport.summary = { total: 2, running: 0, needs_configuration: 0 }

        state.refresh(false)
        state.runAction("start", "bedrock", "", "", "Start Bedrock")

        compare(gateway.requestCount, 2)
        verify(gateway.completeRequestAt(1, {
            ok: true, value: actionReport, text: "OK", error: ""
        }))
        compare(state.report.summary.running, 2)

        verify(gateway.completeRequestAt(0, {
            ok: true, value: staleReport, text: "OK", error: ""
        }))
        compare(state.report.summary.running, 2)
    }

    function test_profile_change_rejects_previous_status_response() {
        gateway.deferRequests = true
        const staleDefault = sampleReport()
        staleDefault.profile = "default"
        const currentLocal = sampleReport()
        currentLocal.profile = "local"

        state.refresh(false)
        state.networkProfile = "local"
        state.refresh(false)

        compare(gateway.requestCount, 2)
        verify(gateway.completeRequestAt(0, {
            ok: true, value: staleDefault, text: "OK", error: ""
        }))
        compare(state.report, null)

        verify(gateway.completeRequestAt(0, {
            ok: true, value: currentLocal, text: "OK", error: ""
        }))
        compare(state.report.profile, "local")
        compare(state.networkProfile, "local")
    }

    function test_refresh_retries_after_shared_busy_state_clears() {
        gateway.deferRequests = true
        gateway.busy = true

        compare(state.refresh(false), null)
        compare(gateway.requestCount, 0)
        verify(state.statusRefreshDeferred)

        gateway.busy = false

        tryCompare(gateway, "requestCount", 1)
        verify(state.statusLoading)
        verify(gateway.completeRequestAt(0, {
            ok: true, value: sampleReport(), text: "OK", error: ""
        }))
        verify(!state.statusLoading)
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
        compare(state.operations[0].action, "start")
        compare(state.operations[0].node, "bedrock")
        compare(state.operations[0].status, "failed")
        compare(gateway.history.length, 1)
        compare(gateway.history[0].operation.status, "failed")
    }

    function test_needs_configuration_action_is_not_reported_as_completed() {
        state.networkProfile = "default"
        const report = testnetReport()
        report.operations = [{
            action: "install",
            node: "indexer",
            status: "needs_configuration",
            detail: "install an official Indexer package"
        }]
        gateway.responses = ({
            localNodesAction: {
                ok: true,
                value: report,
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

        state.runAction("install", "indexer", "", "", "Install Indexer")

        compare(state.error, "")
        compare(gateway.history.length, 1)
        compare(gateway.history[0].operation.status, "failed")
        compare(gateway.history[0].operation.result.status, "needs_configuration")
        compare(gateway.history[0].operation.error, "install an official Indexer package")
        compare(gateway.resultTitle, "Install Indexer")
        compare(gateway.resultText, "install an official Indexer package")
        verify(gateway.resultIsError)
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

        compare(state.modeLabel(), "Testnet")
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

    function test_runtime_modules_dir_defaults_to_system_modules() {
        state.report = testnetReport()
        state.revision += 1

        compare(state.runtimeModulesDir(), "/opt/logos-node/modules")
        state.beginRuntimeAction("start_runtime", "", "")
        compare(state.pendingRuntimeModulesDir, "/opt/logos-node/modules")

        gateway.responses = ({
            localNodesAction: {
                ok: true,
                value: testnetReport(),
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
        state.runPendingAction()
        compare(gateway.calls[0].args[1].runtime_modules_dir, "/opt/logos-node/modules")
    }

    function test_runtime_diagnostics_follow_managed_node_lifecycle() {
        verify(!state.runtimeDiagnosticsReady("messaging"))

        state.report = testnetReport()
        state.revision += 1
        verify(state.runtimeDiagnosticsReady("messaging"))
        verify(state.runtimeDiagnosticsReady("storage"))

        const report = testnetReport()
        report.runtime = {
            ownership: "inspector_managed",
            run_state: "running"
        }
        report.nodes[3].run_state = "running"
        state.report = report
        state.revision += 1

        verify(state.runtimeDiagnosticsReady("messaging"))
        verify(!state.runtimeDiagnosticsReady("storage"))

        report.nodes[2].run_state = "running"
        state.report = Object.assign({}, report)
        state.revision += 1

        verify(state.runtimeDiagnosticsReady("storage"))
    }

    function test_testnet_observation_health_is_separate_from_control_ownership() {
        state.report = testnetReport()
        state.observedNodes = ({
            bedrock: { status: "healthy", detail: "Online" },
            indexer: {
                status: "reachable",
                head: 22352,
                upstream_head: 22418,
                detail: "Indexer caught up"
            },
            storage: { status: "healthy", detail: "25 DHT peers" },
            messaging: { status: "healthy", detail: "10 relay peers" }
        })

        compare(state.summaryText(), "4/4 online")
        compare(state.summaryTone(), "success")
        compare(state.observedRunState("bedrock"), "online")
        compare(state.observedRunState("indexer"), "online")
        compare(state.controlState(state.nodeByKind("indexer")), "external")
        verify(!state.actionEnabled("indexer", "stop"))

        state.observedNodes = ({
            bedrock: { status: "healthy" },
            indexer: {
                status: "reachable",
                head: 6001,
                upstream_head: 22418
            },
            storage: { status: "healthy" },
            messaging: { status: "healthy" }
        })

        compare(state.summaryText(), "3/4 online")
        compare(state.summaryTone(), "warning")
        compare(state.observedRunState("indexer"), "syncing")
    }

    function test_testnet_summary_counts_configured_channel_indexers_individually() {
        state.report = testnetReport()
        state.observedNodes = ({
            bedrock: { status: "healthy" },
            indexer: {
                channels: [{
                    channel_id: "a".repeat(64),
                    status: "reachable",
                    head: 101,
                    upstream_head: 104
                }, {
                    channel_id: "b".repeat(64),
                    status: "reachable",
                    head: 90,
                    upstream_head: 91
                }]
            },
            storage: { status: "healthy" },
            messaging: { status: "healthy" }
        })

        compare(state.summaryText(), "5/5 online")
        compare(state.summaryTone(), "success")

        state.observedNodes = Object.assign({}, state.observedNodes, {
            indexer: {
                channels: [{
                    channel_id: "a".repeat(64),
                    status: "reachable",
                    head: 101,
                    upstream_head: 104
                }, {
                    channel_id: "b".repeat(64),
                    status: "unreachable",
                    head: null,
                    upstream_head: 91
                }]
            }
        })

        compare(state.summaryText(), "4/5 online")
        compare(state.summaryTone(), "error")
    }

    function test_configured_channel_indexer_overrides_legacy_stopped_indexer_node() {
        const report = managedTestnetReport()
        report.nodes = report.nodes.map(function (node) {
            if (node.key !== "indexer") {
                return node
            }
            return Object.assign({}, node, {
                run_state: "stopped",
                process_id: null
            })
        })
        state.report = report
        state.observedNodes = ({
            bedrock: { status: "healthy" },
            indexer: {
                channels: [{
                    channel_id: "a".repeat(64),
                    status: "reachable",
                    head: 101,
                    upstream_head: 104
                }]
            },
            storage: { status: "healthy" },
            messaging: { status: "healthy" }
        })

        compare(state.nodeByKind("indexer").run_state, "stopped")
        compare(state.observedRunState("indexer"), "online")
        compare(state.summaryText(), "4/4 online")
        compare(state.summaryTone(), "success")
    }

    function test_testnet_indexer_finality_window_preserves_reachability() {
        state.report = testnetReport()
        state.observedNodes = ({
            indexer: {
                status: "reachable",
                head: 22162,
                upstream_head: 22418
            }
        })

        compare(state.observedRunState("indexer"), "online")

        state.observedNodes = ({
            indexer: {
                status: "reachable",
                head: 22161,
                upstream_head: 22418
            }
        })

        compare(state.observedRunState("indexer"), "syncing")
    }

    function test_managed_testnet_lifecycle_stays_online_when_diagnostics_degrade() {
        state.report = managedTestnetReport()
        state.observedNodes = ({
            bedrock: { status: "unavailable", detail: "diagnostic timeout" },
            indexer: {
                status: "reachable",
                head: 24,
                upstream_head: 23509,
                detail: "backfilling"
            },
            storage: { status: "unavailable", detail: "diagnostic timeout" },
            messaging: { status: "unavailable", detail: "diagnostic contention" }
        })

        compare(state.summaryText(), "4/4 online")
        compare(state.summaryTone(), "success")
        compare(state.observedRunState("bedrock"), "online")
        compare(state.observedRunState("indexer"), "online")
        compare(state.observedRunState("storage"), "online")
        compare(state.observedRunState("messaging"), "online")
    }

    function test_managed_testnet_lifecycle_rejects_stale_healthy_observation() {
        const report = managedTestnetReport()
        report.nodes[2].run_state = "initializing"
        state.report = report
        state.observedNodes = ({
            storage: { status: "healthy", detail: "stale source response" }
        })

        compare(state.observedRunState("storage"), "initializing")
        compare(state.summaryText(), "3/4 online")
        compare(state.summaryTone(), "warning")

        report.nodes[2].run_state = "stopping"
        state.report = Object.assign({}, report)

        compare(state.observedRunState("storage"), "stopping")
        compare(state.summaryText(), "3/4 online")
        compare(state.summaryTone(), "warning")

        report.nodes[2].run_state = "stopped"
        state.report = Object.assign({}, report)

        compare(state.observedRunState("storage"), "unavailable")
        compare(state.summaryText(), "3/4 online")
        compare(state.summaryTone(), "error")
    }

    function test_managed_process_stop_does_not_fall_back_to_stale_diagnostics() {
        const report = managedTestnetReport()
        report.runtime = {
            ownership: "external",
            run_state: "not_configured"
        }
        report.nodes[1].process_id = null
        report.nodes[1].run_state = "stopped"
        state.report = report
        state.observedNodes = ({
            indexer: {
                status: "reachable",
                head: 23509,
                upstream_head: 23509,
                detail: "stale source response"
            }
        })

        compare(state.controlState(state.nodeByKind("indexer")), "managed")
        compare(state.observedRunState("indexer"), "unavailable")
    }

    function test_managed_indexer_projects_module_sync_states() {
        const report = managedTestnetReport()
        state.report = report

        report.nodes[1].run_state = "syncing"
        state.report = Object.assign({}, report)
        compare(state.observedRunState("indexer"), "syncing")

        report.nodes[1].run_state = "caught_up"
        state.report = Object.assign({}, report)
        compare(state.observedRunState("indexer"), "online")

        report.nodes[1].run_state = "error"
        state.report = Object.assign({}, report)
        compare(state.observedRunState("indexer"), "unavailable")
    }

    function test_external_installed_process_remains_diagnostic_driven() {
        const report = testnetReport()
        report.nodes[1].install_state = "installed"
        report.nodes[1].run_state = "stopped"
        report.nodes[1].ownership = "external"
        state.report = report
        state.observedNodes = ({
            indexer: {
                status: "reachable",
                head: 23509,
                upstream_head: 23509,
                detail: "external Indexer"
            }
        })

        compare(state.controlState(state.nodeByKind("indexer")), "external")
        compare(state.observedRunState("indexer"), "online")
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

    function test_messaging_stop_action_draft_acknowledges_legacy_identity_rotation() {
        state.beginNodeAction("stop", "messaging")

        compare(state.actionDraftTitle(), "Stop Messaging")
        verify(state.actionDraftMessage().indexOf("one-time rotation is unavoidable") >= 0)
        verify(state.pendingAllowIdentityRotation)

        gateway.responses = ({
            localNodesAction: {
                ok: true,
                value: testnetReport(),
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
        state.runPendingAction()

        compare(gateway.calls[0].args[1].action, "stop")
        compare(gateway.calls[0].args[1].node, "messaging")
        verify(gateway.calls[0].args[1].allow_identity_rotation)
        verify(!state.pendingAllowIdentityRotation)
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
        verify(state.actionDraftMessage().indexOf("This starts an Inspector-managed LogosCore runtime using modules from /tmp/modules.") === 0)
        verify(state.actionDraftMessage().indexOf("one-time rotation is unavoidable") >= 0)
        verify(state.pendingAllowIdentityRotation)

        state.runPendingAction()

        compare(gateway.calls[0].args[1].action, "start_runtime")
        compare(gateway.calls[0].args[1].runtime_modules_dir, "/tmp/modules")
        compare(gateway.calls[0].args[1].runtime_binary_path, "/tmp/logoscore")
        verify(gateway.calls[0].args[1].allow_identity_rotation)
        compare(gateway.history[0].operation.label, "Start Local Runtime")
    }

    function test_stop_runtime_confirmation_acknowledges_legacy_messaging_identity_rotation() {
        state.beginRuntimeAction("stop_runtime", "/tmp/modules", "/tmp/logoscore")
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

        compare(state.actionDraftTitle(), "Stop Local Runtime")
        verify(state.actionDraftMessage().indexOf("one-time rotation is unavoidable") >= 0)
        verify(state.pendingAllowIdentityRotation)
        state.runPendingAction()

        compare(gateway.calls[0].args[1].action, "stop_runtime")
        verify(gateway.calls[0].args[1].allow_identity_rotation)
        verify(!state.pendingAllowIdentityRotation)
    }

    function test_package_catalog_loads_exact_releases_for_modules_directory() {
        gateway.responses = ({
            localNodePackageCatalog: {
                ok: true,
                value: samplePackageCatalog({
                    version: "1.0.0",
                    root_hash: "root-hash-1.0.0"
                }),
                text: "OK",
                error: ""
            }
        })

        state.refreshPackageCatalog("/tmp/modules")

        compare(gateway.calls.length, 1)
        compare(gateway.calls[0].method, "localNodePackageCatalog")
        compare(gateway.calls[0].args.length, 1)
        compare(gateway.calls[0].args[0], "/tmp/modules")
        verify(!state.packageCatalogLoading)
        compare(state.packageCatalogError, "")
        compare(state.packageName(), "lez_indexer_module")
        compare(state.packageCatalogModulesDir(), "/tmp/modules")
        compare(state.packageReleases().length, 3)
        compare(
            state.packageRelease("1.0.0", "root-hash-1.0.0-repack").released_at,
            "2026-07-01T12:00:00Z")
        compare(
            state.packageRelease("1.0.0", "root-hash-1.0.0").released_at,
            "2026-06-01T12:00:00Z")
        compare(state.packageRelease("1.0.0"), null)

        let selection = state.defaultPackageSelection()
        compare(selection.version, "1.0.0")
        compare(selection.root_hash, "root-hash-1.0.0")

        state.packageCatalog = samplePackageCatalog(null)
        selection = state.defaultPackageSelection()
        compare(selection.version, "1.1.0")
        compare(selection.root_hash, "root-hash-1.1.0")
    }

    function test_package_catalog_failure_clears_stale_package_state() {
        state.packageCatalog = samplePackageCatalog()
        gateway.responses = ({
            localNodePackageCatalog: {
                ok: false,
                value: null,
                text: "",
                error: "catalog unavailable"
            }
        })

        state.refreshPackageCatalog("/tmp/modules")

        compare(state.packageCatalog, null)
        compare(state.packageCatalogError, "catalog unavailable")
        verify(!state.packageCatalogLoading)
    }

    function test_indexer_install_draft_serializes_exact_package_identity() {
        state.networkProfile = "default"
        const report = testnetReport()
        report.nodes[1].available_actions = ["install"]
        state.report = report
        state.packageCatalog = samplePackageCatalog()
        state.beginNodeAction(
            "install",
            "indexer",
            "1.0.0",
            "root-hash-1.0.0-repack",
            "/tmp/modules")

        compare(state.actionDraftTitle(), "Install Indexer 1.0.0")
        compare(
            state.actionDraftMessage(),
            "This downloads official lez_indexer_module 1.0.0, verifies root hash root-hash-1.0.0-repack, and installs it into /tmp/modules. LogosCore Runtime must be stopped. After installation, start the runtime, then use Zone Sources to start the selected Channel Indexer.")

        gateway.responses = ({
            localNodesAction: {
                ok: true,
                value: report,
                text: "OK",
                error: ""
            },
            localDevnetList: {
                ok: true,
                value: { devnets: [] },
                text: "OK",
                error: ""
            },
            localNodePackageCatalog: {
                ok: true,
                value: samplePackageCatalog({
                    version: "1.0.0",
                    root_hash: "root-hash-1.0.0-repack"
                }),
                text: "OK",
                error: ""
            }
        })

        state.runPendingAction()

        compare(gateway.calls[0].method, "localNodesAction")
        compare(gateway.calls[0].args[1].action, "install")
        compare(gateway.calls[0].args[1].node, "indexer")
        compare(gateway.calls[0].args[1].runtime_modules_dir, "/tmp/modules")
        compare(gateway.calls[0].args[1].package_version, "1.0.0")
        compare(gateway.calls[0].args[1].package_root_hash, "root-hash-1.0.0-repack")
        compare(gateway.calls[2].method, "localNodePackageCatalog")
        compare(gateway.calls[2].args[0], "/tmp/modules")
        compare(state.installedPackage().version, "1.0.0")
        compare(state.installedPackage().root_hash, "root-hash-1.0.0-repack")
    }
}
