import QtQuick
import QtTest
import "../../qml/services"
import "../../qml/state"
import "../../qml/state/source_routing/SourcePolicyCatalog.js" as SourcePolicyCatalog
import "../../qml/state/source_routing/SourceDiagnosticsProjection.js" as SourceDiagnostics
import "../../qml/state/status/StatusFactsProjection.js" as StatusFactsProjection
import "fixtures"

TestCase {
    id: testRoot

    name: "AppModel"

    BridgeHostFixture {
        id: fakeHost
    }

    QtObject {
        id: basecampHost

        property int callCount: 0
        property string lastModule: ""
        property string lastMethod: ""
        property var lastArgs: []
        property bool serializeResults: false

        function callModule(moduleName, method, args) {
            callCount += 1
            lastModule = String(moduleName || "")
            lastMethod = String(method || "")
            lastArgs = args || []
            if (lastModule === "logos_inspector" && lastMethod === "call") {
                const response = JSON.stringify({
                    ok: true,
                    value: {
                        method: lastArgs[0],
                        args: JSON.parse(String(lastArgs[1] || "[]"))
                    },
                    text: "OK",
                    error: ""
                })
                return serializeResults ? JSON.stringify(response) : response
            }
            return "direct"
        }
    }

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    BridgeClient {
        id: basecampBridgeClient

        host: basecampHost
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    ModuleEventIntake {
        id: moduleEventIntake

        bridge: bridgeClient
        model: model
    }

    AppModel {
        id: basecampModel

        bridge: basecampBridgeClient
    }

    function init() {
        fakeHost.reset()
        basecampHost.callCount = 0
        basecampHost.lastModule = ""
        basecampHost.lastMethod = ""
        basecampHost.lastArgs = []
        basecampHost.serializeResults = false
        model.currentView = "overview"
        model.statusText = "Ready"
        model.busy = false
        model.resultTitle = "Output"
        model.resultText = ""
        model.resultValue = null
        model.resultIsError = false
        model.resultOwner = ""
        model.navigationBackStack = []
        model.navigationForwardStack = []
        model.navigationRevision = 0
        model.navigationRestoring = false
        model.favoriteStore.clear()
        model.dashboardNode = null
        model.dashboardSequencerBlocks = []
        model.blockchainModuleReport = null
        model.storageModuleReport = null
        model.messagingModuleReport = null
        model.storageSourceReport = null
        model.messagingSourceReport = null
        model.deliveryModuleEvents = []
        model.deliveryModuleEventRevision = 0
        model.deliveryConnectionStatus = ""
        model.deliveryNodeStatus = ""
        model.blockchainModuleEventRevision = 0
        model.blockchainLastEventText = ""
        model.storageActiveOperation = null
        model.storageActiveOperationRevision = 0
        model.runtimeOperations = ({})
        model.runtimeOperationEventSeq = ({})
        model.runtimeOperationHistory = []
        model.runtimeOperationsRevision = 0
        model.networkConnectionStatus = ({})
        model.networkConnectionStatusRevision = 0
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.blocksPageRows = []
        model.blocksPageSlotFrom = 0
        model.blocksPageSlotTo = 0
        model.blocksPageError = ""
        model.blocksLiveEnabled = false
        model.blocksLiveError = ""
        model.blocksLiveSource = ""
        model.blocksLiveUnknownEvents = 0
        model.blocksLiveCheckedAt = ""
        model.lezBlocksPageRows = []
        model.lezBlocksPageBeforeBlock = 0
        model.lezBlocksPageNextBeforeBlock = 0
        model.lezBlocksPageError = ""
        model.lezBlocksPageLoading = false
        model.lezBlocksPageRequestSerial = 0
        model.lezTransactionsPageRows = []
        model.lezTransactionsPageBeforeBlock = 0
        model.lezTransactionsPageNextBeforeBlock = 0
        model.lezTransactionsPageOverflowRows = []
        model.lezTransactionsPageOverflowNextBeforeBlock = 0
        model.lezTransactionsPageError = ""
        model.transferActivityRows = []
        model.transferActivityBeforeBlock = 0
        model.transferActivityNextBeforeBlock = 0
        model.transferActivityOverflowRows = []
        model.transferActivityOverflowNextBeforeBlock = 0
        model.transferActivityHistory = []
        model.transferActivityError = ""
        model.blockDetailValue = null
        model.blockDetailError = ""
        model.transactionDetailValue = null
        model.transactionDetailError = ""
        model.accountDetailValue = null
        model.transferRecipientDetailValue = null
        model.channelDetailValue = null
        model.channelDetailError = ""
        model.blockchainSourceMode = "auto"
        model.indexerSourceMode = "auto"
        model.executionSourceMode = "rpc"
        model.messagingSourceMode = "auto"
        model.storageSourceMode = "auto"
        model.sourcePolicy = ({})
        model.sourcePolicyLoaded = false
        basecampModel.blockchainSourceMode = "auto"
        basecampModel.indexerSourceMode = "auto"
        basecampModel.executionSourceMode = "rpc"
        basecampModel.messagingSourceMode = "auto"
        basecampModel.storageSourceMode = "auto"
        basecampModel.sourcePolicy = ({})
        basecampModel.sourcePolicyLoaded = false
        model.registeredIdls.clear()
        model.socialIdentities.clear()
        model.idlStateLoaded = false
        model.walletStateLoaded = false
        model.settingsStateLoaded = false
        model.socialIdentityDefaultMode = "perConversation"
        model.selectedSocialIdentityKey = ""
        model.socialConversationIdentityKeys = ({})
        model.socialIdentityRevision = 0
        model.socialCommentState = ({})
        model.socialCommentRevision = 0
        model.socialSharedIdls = ({})
        model.sharedIdlPolicy = "suggestion"
        model.sharedIdlAutoShare = false
        model.socialAutoSharedIdls = ({})
        model.sharedIdlRevision = 0
        model.accountIdlSelections = ({})
        model.accountIdlSelectionRevision = 0
        model.idlInstructionPreviewValue = null
        model.idlInstructionError = ""
        model.walletPublicKeyProbe = ""
        model.bedrockWalletModuleError = ""
        model.walletBinary = ""
        model.walletHome = ""
        model.walletCreatePrivacy = "public"
        model.walletCreateLabel = ""
        model.walletSendFrom = ""
        model.walletSendTo = ""
        model.walletSendToKeys = ""
        model.walletSendToNpk = ""
        model.walletSendToVpk = ""
        model.walletSendToIdentifier = ""
        model.walletSendAmount = ""
        model.walletAdvancedCommand = ""
        model.localWalletStatus = null
        model.localWalletStatusError = ""
        model.localWalletOperations = []
        model.localNodesReport = null
        model.localNodesError = ""
        model.localNodesOperations = []
        model.localNodesRevision = 0
        model.localDevnets = []
    }

    function installSourceModePolicy(targetModel) {
        targetModel.sourcePolicy = SourcePolicyCatalog.fallbackPolicy()
        targetModel.sourcePolicyLoaded = true
    }

    function sourceOption(options, key) {
        for (let i = 0; i < options.length; ++i) {
            const option = options[i] || {}
            if (String(option.key || "") === key) {
                return option
            }
        }
        return ({})
    }

    function test_basecamp_bridge_routes_inspector_calls_through_generic_call() {
        const response = basecampBridgeClient.callModule("logos_inspector", "blockchainLiveBlocks", ["http://127.0.0.1:8080", 1, 2, 3])

        compare(basecampHost.callCount, 1)
        compare(basecampHost.lastModule, "logos_inspector")
        compare(basecampHost.lastMethod, "call")
        compare(basecampHost.lastArgs[0], "blockchainLiveBlocks")
        compare(JSON.parse(basecampHost.lastArgs[1])[3], 3)
        verify(response.ok)
        compare(response.value.method, "blockchainLiveBlocks")
        compare(response.value.args[1], 1)
    }

    function test_basecamp_bridge_decodes_json_serialized_inspector_response() {
        basecampHost.serializeResults = true

        const response = basecampBridgeClient.callModule("logos_inspector", "blockchainLiveBlocks", ["http://127.0.0.1:8080", 1, 2, 3])

        compare(basecampHost.callCount, 1)
        compare(basecampHost.lastModule, "logos_inspector")
        compare(basecampHost.lastMethod, "call")
        verify(response.ok)
        compare(response.value.method, "blockchainLiveBlocks")
        compare(response.value.args[3], 3)
    }

    function test_basecamp_bridge_keeps_inspector_module_version_direct() {
        const response = basecampBridgeClient.callModule("logos_inspector", "moduleVersion", [])

        compare(basecampHost.callCount, 1)
        compare(basecampHost.lastModule, "logos_inspector")
        compare(basecampHost.lastMethod, "moduleVersion")
        verify(response.ok)
        compare(response.value, "direct")
    }

    function test_core_source_args_keep_rpc_shape_in_standalone_auto() {
        compare(model.effectiveCoreSourceMode(model.blockchainSourceMode), "rpc")

        const args = model.blockchainArgs([1, 2])

        compare(args.length, 3)
        compare(args[0], model.nodeUrl)
        compare(args[1], 1)
        compare(args[2], 2)
    }

    function test_local_node_action_wrapper_dispatches_confirmation_token() {
        model.networkProfile = "local"
        fakeHost.callCount = 0
        fakeHost.lastMethod = ""
        fakeHost.lastArgs = []
        fakeHost.responses = ({
            localNodesAction: {
                ok: true,
                value: {
                    active_devnet: "devnet",
                    summary: { total: 0, installed: 0, running: 0, needs_configuration: 0 },
                    nodes: [],
                    operations: [{ action: "start", node: "bedrock", status: "started", detail: "ok" }],
                    tools: {}
                },
                text: "OK",
                error: ""
            },
            localDevnetList: {
                ok: true,
                value: { devnets: ["devnet"] },
                text: "OK",
                error: ""
            }
        })

        model.runLocalNodeAction("start", "bedrock", "", "", "Start Bedrock")

        tryCompare(fakeHost, "callCount", 2)
        compare(fakeHost.calls[0].method, "localNodesAction")
        compare(fakeHost.calls[0].args[0], "local")
        compare(fakeHost.calls[0].args[1].action, "start")
        compare(fakeHost.calls[0].args[1].node, "bedrock")
        compare(fakeHost.calls[0].args[2], "confirm-local-node-action")
        compare(fakeHost.calls[1].method, "localDevnetList")
        compare(model.localNodesOperations.length, 1)
        const localNodeHistory = model.runtimeOperationHistoryRows("localNodes")
        compare(localNodeHistory.length, 1)
        compare(localNodeHistory[0].label, "Start Bedrock")
        compare(localNodeHistory[0].status, "completed")
    }

    function test_core_source_args_use_module_shape_in_basecamp_auto() {
        compare(basecampModel.effectiveCoreSourceMode(basecampModel.indexerSourceMode), "module")

        const args = basecampModel.indexerArgs(["hash-1"])

        compare(args.length, 3)
        compare(args[0], "module")
        compare(args[1], basecampModel.indexerUrl)
        compare(args[2], "hash-1")
    }

    function test_rpc_only_helpers_keep_rpc_shape_in_basecamp() {
        compare(basecampModel.effectiveCoreSourceMode(basecampModel.blockchainSourceMode), "module")

        const channelArgs = basecampModel.blockchainRpcArgs([10, 20])
        compare(channelArgs.length, 3)
        compare(channelArgs[0], basecampModel.nodeUrl)
        compare(channelArgs[1], 10)
        compare(channelArgs[2], 20)

        const programArgs = basecampModel.executionRpcArgs([])
        compare(programArgs.length, 1)
        compare(programArgs[0], basecampModel.sequencerUrl)

        const executionArgs = basecampModel.executionArgs(["tx-1"])
        compare(executionArgs.length, 2)
        compare(executionArgs[0], basecampModel.sequencerUrl)
        compare(executionArgs[1], "tx-1")
    }

    function test_account_lookup_args_use_module_indexer_for_history() {
        basecampModel.indexerSourceMode = "module"

        const args = basecampModel.accountLookupArgs("account-1")

        compare(args.length, 5)
        compare(args[0], "rpc")
        compare(args[1], basecampModel.sequencerUrl)
        compare(args[2], "module")
        compare(args[3], basecampModel.indexerUrl)
        compare(args[4], "account-1")
    }

    function test_messaging_and_storage_auto_use_standalone_routes_without_basecamp() {
        compare(model.normalizedMessagingSourceMode(model.messagingSourceMode), "auto")
        compare(model.effectiveMessagingSourceMode(model.messagingSourceMode), "rest")
        compare(model.deliverySourceReportArgs()[0], "rest")
        compare(model.deliverySourceReportArgs()[1], model.configuredMessagingRestUrl())
        compare(model.deliverySourceReportArgs()[2], model.messagingMetricsUrl)
        compare(model.deliverySourceTarget(), model.configuredMessagingRestUrl())

        compare(model.normalizedStorageSourceMode(model.storageSourceMode), "auto")
        compare(model.effectiveStorageSourceMode(model.storageSourceMode), "rest")
        compare(model.storageSourceReportArgs(false)[0], "rest")
        compare(model.storageSourceReportArgs(false)[1], model.configuredStorageRestUrl())
        compare(model.storageSourceReportArgs(false)[2], model.storageMetricsUrl)
        compare(model.storageSourceTarget(), model.configuredStorageRestUrl())
    }

    function test_source_routing_state_owns_runtime_source_views() {
        const delivery = model.sourceRouting.deliverySourceView()
        compare(delivery.mode, "auto")
        compare(delivery.effectiveMode, "rest")
        compare(delivery.label, "Auto: Direct Waku REST")
        compare(delivery.target, model.configuredMessagingRestUrl())
        compare(delivery.reportArgs()[0], "rest")
        compare(delivery.reportArgs()[1], model.configuredMessagingRestUrl())

        const storage = model.sourceRouting.storageSourceView()
        compare(storage.mode, "auto")
        compare(storage.effectiveMode, "rest")
        compare(storage.target, model.configuredStorageRestUrl())
        compare(storage.reportArgs(false)[0], "rest")
        compare(storage.reportArgs(false)[1], model.configuredStorageRestUrl())

        const execution = model.sourceRouting.coreSourceView("execution")
        compare(execution.role, "execution")
        compare(execution.effectiveMode, "rpc")
        compare(execution.target, model.sequencerUrl)
        compare(execution.args(["head"])[0], model.sequencerUrl)
    }

    function test_source_policy_catalog_fallback_supports_pending_modes_without_bridge_policy() {
        model.sourcePolicy = ({})
        model.sourcePolicyLoaded = false

        compare(model.normalizedMessagingSourceMode("metrics"), "metrics")
        compare(model.normalizedMessagingSourceMode("network-monitor"), "network-monitor")
        compare(model.normalizedMessagingSourceMode("delivery network monitor"), "network-monitor")
        compare(model.normalizedMessagingSourceMode("network monitor"), "unsupported")
        compare(model.normalizedStorageSourceMode("metrics"), "metrics")

        model.messagingSourceMode = "network-monitor"
        compare(model.effectiveMessagingSourceMode(model.messagingSourceMode), "network-monitor")
        compare(model.deliverySourceReportArgs()[0], "network-monitor")
        compare(model.deliverySourceTarget(), model.configuredMessagingRestUrl())

        model.storageSourceMode = "metrics"
        compare(model.effectiveStorageSourceMode(model.storageSourceMode), "metrics")
        compare(model.storageSourceReportArgs(false)[0], "metrics")
        compare(model.storageSourceTarget(), model.storageMetricsUrl)

        const deliveryOptions = model.sourceModeOptions("delivery")
        const deliveryKeys = deliveryOptions.map(option => option.key)
        verify(deliveryKeys.indexOf("metrics") >= 0)
        verify(deliveryKeys.indexOf("network-monitor") >= 0)
        compare(sourceOption(deliveryOptions, "network-monitor").label, "Delivery Network Monitor")
    }

    function test_messaging_and_storage_auto_use_module_routes_in_basecamp() {
        compare(basecampModel.effectiveMessagingSourceMode(basecampModel.messagingSourceMode), "module")
        compare(basecampModel.deliverySourceReportArgs()[0], "module")
        compare(basecampModel.deliverySourceReportArgs()[1], "")
        compare(basecampModel.deliverySourceReportArgs()[2], "")
        compare(basecampModel.deliverySourceTarget(), basecampModel.deliveryModule)

        compare(basecampModel.effectiveStorageSourceMode(basecampModel.storageSourceMode), "module")
        compare(basecampModel.storageSourceReportArgs(false)[0], "module")
        compare(basecampModel.storageSourceReportArgs(false)[1], "")
        compare(basecampModel.storageSourceReportArgs(false)[2], "")
        compare(basecampModel.storageSourceTarget(), basecampModel.storageModule)
    }

    function test_source_policy_load_supplies_defaults_and_profile_matching() {
        fakeHost.responses = ({
            sourcePolicy: {
                ok: true,
                value: {
                    defaults: {
                        sequencer_endpoint: "https://policy-sequencer.invalid/",
                        local_sequencer_endpoint: "http://policy-local.invalid/",
                        indexer_endpoint: "http://policy-indexer.invalid/",
                        node_endpoint: "http://policy-node.invalid/",
                        delivery_rest_endpoint: "http://policy-delivery.invalid:8645",
                        delivery_metrics_endpoint: "http://policy-delivery.invalid:8008/metrics",
                        storage_rest_endpoint: "http://policy-storage.invalid/api/storage/v1",
                        storage_metrics_endpoint: "http://policy-storage.invalid:8008/metrics"
                    },
                    network_profiles: [
                        {
                            id: "default",
                            sequencer_endpoint: "https://policy-sequencer.invalid/",
                            indexer_endpoint: "http://policy-indexer.invalid/",
                            node_endpoint: "http://policy-node.invalid/"
                        },
                        {
                            id: "local",
                            sequencer_endpoint: "http://policy-local.invalid/",
                            indexer_endpoint: "http://policy-indexer.invalid/",
                            node_endpoint: "http://policy-node.invalid/"
                        }
                    ],
                    source_modes: {
                        core: [
                            { key: "auto", aliases: ["auto"], effective: "rpc" },
                            { key: "rpc", aliases: ["rpc"], effective: "rpc" },
                            { key: "module", aliases: ["basecamp"], effective: "module" }
                        ],
                        delivery: [
                            { key: "auto", aliases: ["auto"], effective: "rest" },
                            { key: "rest", aliases: ["direct waku rest"], effective: "rest" },
                            { key: "network-monitor", aliases: ["discovery crawler"], effective: "network-monitor" }
                        ],
                        storage: [
                            { key: "auto", aliases: ["auto"], effective: "rest" },
                            { key: "rest", aliases: ["standalone rest"], effective: "rest" },
                            { key: "module", aliases: ["basecamp module"], effective: "module" }
                        ]
                    }
                },
                text: "OK",
                error: ""
            }
        })

        verify(model.loadSourcePolicy())
        compare(fakeHost.lastMethod, "sourcePolicy")
        verify(model.sourcePolicyLoaded)

        model.messagingRestUrl = ""
        model.storageRestUrl = ""
        compare(model.configuredMessagingRestUrl(), "http://policy-delivery.invalid:8645")
        compare(model.configuredStorageRestUrl(), "http://policy-storage.invalid/api/storage/v1")
        compare(model.normalizedCoreSourceMode("basecamp"), "module")
        compare(model.effectiveCoreSourceMode("basecamp"), "module")
        compare(model.normalizedMessagingSourceMode("direct waku rest"), "rest")
        compare(model.normalizedStorageSourceMode("standalone rest"), "rest")

        model.applyProfile(1)
        compare(model.sequencerUrl, "http://policy-local.invalid/")
        compare(model.indexerUrl, "http://policy-indexer.invalid/")
        compare(model.nodeUrl, "http://policy-node.invalid/")
        compare(model.inferNetworkProfileFromEndpoints(model.sequencerUrl, model.indexerUrl, model.nodeUrl), "local")

        model.applyProfile(0)
        compare(model.sequencerUrl, "https://policy-sequencer.invalid/")
        compare(model.inferNetworkProfileFromEndpoints(model.sequencerUrl, model.indexerUrl, model.nodeUrl), "default")
    }

    function test_settings_query_caches_execution_head_for_footer_metrics() {
        fakeHost.responses = {
            head: {
                ok: true,
                value: 42,
                text: "42",
                error: ""
            }
        }

        model.queryNetworkConnection("execution", false)

        tryVerify(function () { return model.networkConnectionIsPending("execution") === false })
        compare(model.sequencerHeadValue(), 42)
        verify(model.dashboardOverview.sequencer.health.ok)
        compare(model.dashboardOverview.sequencer.head.value, 42)
    }

    function test_settings_query_caches_blockchain_node_for_footer_metrics() {
        fakeHost.responses = {
            blockchainNode: {
                ok: true,
                value: {
                    cryptarchia_info: {
                        ok: true,
                        value: { cryptarchia_info: { slot: 77, lib_slot: 70 } },
                        error: null
                    },
                    network_info: {
                        ok: true,
                        value: { n_peers: 4 },
                        error: null
                    }
                },
                text: "OK",
                error: ""
            }
        }

        model.queryNetworkConnection("blockchain", false)

        tryVerify(function () { return model.networkConnectionIsPending("blockchain") === false })
        compare(model.cryptarchiaValue("slot"), 77)
        compare(model.networkValue("n_peers"), 4)
    }

    function test_default_footer_storage_failure_field_is_registered_recent_key() {
        const defaults = model.defaultFooterFieldSelections()

        verify(defaults["storage.failed_transfers_recent"] === true)
        verify(defaults["storage.failed_transfers_total"] !== true)
    }

    function test_status_facts_dashboard_projection_owns_graph_keys_and_labels() {
        const keys = StatusFactsProjection.dashboardGraphKeys()

        verify(keys.indexOf("storage.failed_transfers_recent") >= 0)
        verify(keys.indexOf("storage.failed_transfers_total") >= 0)
        verify(keys.indexOf("messaging.network_ingress_recent") >= 0)
        compare(StatusFactsProjection.dashboardMetricLabel("storage.failed_transfers_total"), "transfer failures total")
        compare(StatusFactsProjection.dashboardMetricGroup("messaging.network_ingress_recent"), "Messaging / Delivery")
        compare(StatusFactsProjection.dashboardMetricTone("storage.failed_transfers_recent", 2), "error")
        compare(StatusFactsProjection.dashboardMetricTone("storage.failed_transfers_total", 2), "neutral")
    }

    function test_explicit_rest_blank_urls_use_visible_defaults() {
        model.messagingSourceMode = "rest"
        model.messagingRestUrl = ""
        compare(model.deliverySourceReportArgs()[0], "rest")
        compare(model.deliverySourceReportArgs()[1], "http://127.0.0.1:8645")
        compare(model.deliverySourceTarget(), "http://127.0.0.1:8645")

        model.storageSourceMode = "rest"
        model.storageRestUrl = ""
        compare(model.storageSourceReportArgs(false)[0], "rest")
        compare(model.storageSourceReportArgs(false)[1], "http://127.0.0.1:8080/api/storage/v1")
        compare(model.storageSourceTarget(), "http://127.0.0.1:8080/api/storage/v1")
    }

    function test_storage_unsupported_pending_modes_stay_inert() {
        installSourceModePolicy(model)

        compare(model.normalizedStorageSourceMode("module"), "module")
        model.storageSourceMode = "module"
        compare(model.effectiveStorageSourceMode(model.storageSourceMode), "module")
        compare(model.storageSourceReportArgs(false)[0], "module")
        compare(model.storageSourceReportArgs(false)[1], "")
        compare(model.storageSourceTarget(), model.storageModule)

        compare(model.normalizedStorageSourceMode("c-library"), "unsupported")
        compare(model.normalizedStorageSourceMode("local-os"), "unsupported")
        model.storageSourceMode = "unsupported"
        compare(model.effectiveStorageSourceMode(model.storageSourceMode), "unsupported")
        compare(model.storageSourceReportArgs(false)[0], "unsupported")
    }

    function test_delivery_network_monitor_source_is_supported() {
        installSourceModePolicy(model)

        compare(model.normalizedMessagingSourceMode("network-monitor"), "network-monitor")
        compare(model.normalizedMessagingSourceMode("delivery network monitor"), "network-monitor")
        compare(model.normalizedMessagingSourceMode("discovery crawler"), "network-monitor")
        compare(model.normalizedMessagingSourceMode("network monitor"), "unsupported")
        compare(model.normalizedMessagingSourceMode("crawler"), "unsupported")
        model.messagingSourceMode = "network-monitor"

        compare(model.effectiveMessagingSourceMode(model.messagingSourceMode), "network-monitor")
        compare(model.deliverySourceReportArgs()[0], "network-monitor")
        compare(model.deliverySourceReportArgs()[1], model.configuredMessagingRestUrl())
        compare(model.deliverySourceReportArgs()[2], model.messagingMetricsUrl)
        compare(model.deliverySourceTarget(), model.configuredMessagingRestUrl())
    }

    function test_source_mode_options_labels_and_targets_come_from_policy() {
        installSourceModePolicy(model)

        const storageOptions = model.sourceModeOptions("storage")
        verify(storageOptions.length >= 5)
        compare(sourceOption(storageOptions, "rest").label, "Standalone REST")

        model.storageSourceMode = "module"
        compare(model.storageSourceLabel(), "Storage module")
        compare(model.storageSourceTarget(), model.storageModule)
        verify(model.sourceModeSupportsCidProbe("storage", model.storageSourceMode))
        verify(model.sourceModeSupportsMutatingDiagnostics("storage", model.storageSourceMode))

        model.messagingSourceMode = "metrics"
        compare(model.deliverySourceLabel(), "Metrics only")
        compare(model.deliverySourceTarget(), model.messagingMetricsUrl)
        verify(model.sourceModeUsesEndpoint("delivery", model.messagingSourceMode, "metrics"))
    }

    function test_source_report_health_facts_drive_connection_state_without_probes() {
        const report = {
            module: "delivery_rest",
            health: {
                reachable: true,
                ready: true,
                status: "healthy",
                summary: "delivery source ready",
                detail: "node health Ready; connection Connected"
            },
            capability_facts: [
                { key: "metrics", label: "Metrics", available: true, evidence: "known Waku metric family observed" }
            ],
            probes: []
        }

        verify(model.moduleReportReachable(report))
        verify(model.deliveryReportHealthy(report))
        compare(model.networkConnectionSummary("messaging", report), "delivery source ready")
        verify(model.sourceCapabilityAvailable(report, "metrics"))
        compare(model.sourceCapabilityEvidence(report, "metrics"), "known Waku metric family observed")
    }

    function test_source_report_health_facts_mark_storage_not_ready_without_probe_names() {
        const report = {
            module: "storage_rest",
            health: {
                reachable: true,
                ready: false,
                status: "degraded",
                summary: "storage source degraded",
                detail: "required storage facts missing"
            },
            capability_facts: [
                { key: "identity", label: "Identity", available: false, evidence: "not observed" }
            ],
            probes: []
        }

        verify(model.moduleReportReachable(report))
        verify(!model.storageReportReady(report))
        compare(model.networkConnectionSummary("storage", report), "required storage facts missing")
        compare(model.sourceCapabilityAvailable(report, "identity"), false)
    }

    function test_module_probe_lookup_ignores_source_facts_without_probe_names() {
        model.storageModuleReport = {
            module: "storage_rest",
            probe_facts: [
                {
                    key: "peerId",
                    label: "renamed",
                    source: "opaque",
                    ok: true,
                    value: "peer-from-fact",
                    error: null
                },
                {
                    key: "collectMetrics",
                    label: "renamed metrics",
                    source: "opaque",
                    ok: false,
                    value: null,
                    error: "metrics unavailable"
                }
            ],
            probes: [
                {
                    label: "unrelated",
                    source: "opaque",
                    ok: true,
                    value: "wrong"
                }
            ]
        }

        compare(model.moduleProbeValue("storage", "peerId"), null)
        compare(model.moduleProbeError("storage", "collectMetrics"), "")
    }

    function test_source_diagnostics_prefer_current_report_facts() {
        model.storageModuleReport = {
            module: "storage_rest",
            probe_facts: [
                {
                    key: "space",
                    label: "stale",
                    source: "old",
                    ok: true,
                    value: "stale-space",
                    error: null
                }
            ],
            probes: []
        }
        const report = {
            module: "storage_rest",
            probe_facts: [
                {
                    key: "space",
                    label: "current",
                    source: "new",
                    ok: true,
                    value: "current-space",
                    error: null
                },
                {
                    key: "manifests",
                    label: "failed",
                    source: "new",
                    ok: false,
                    value: null,
                    error: "manifest probe failed"
                }
            ],
            probes: []
        }

        compare(SourceDiagnostics.probeValue(model, "storage", report, "space"), "current-space")
        compare(SourceDiagnostics.failedProbeCount(report), 1)
    }

    function test_source_report_view_uses_fact_only_source_evidence() {
        const report = {
            module: "storage_rest",
            health: {
                reachable: true,
                ready: true,
                status: "healthy",
                summary: "storage source ready",
                detail: "space observed"
            },
            probe_facts: [
                {
                    key: "space",
                    label: "Repository space",
                    source: "storage",
                    ok: true,
                    value: { used: 1 },
                    error: null
                }
            ],
            capability_facts: [
                {
                    key: "space",
                    label: "Repository space",
                    available: true,
                    evidence: "1 field(s)",
                    value: { used: 1 }
                }
            ],
            probes: []
        }
        const view = model.storageReportView(report)

        compare(view.probeValue("space").used, 1)
        verify(view.capabilityAvailable("space"))
    }

    function test_delivery_throughput_metric_aliases() {
        model.messagingModuleReport = {
            module: "delivery_metrics",
            probes: [
                {
                    label: "delivery_metrics.collectOpenMetricsText",
                    ok: true,
                    value: [
                        "libp2p_network_bytes_total{direction=\"in\"} 20",
                        "waku_service_requests_total{service=\"/vac/waku/store-query/3.0.0\"} 4",
                        "waku_store_messages 7"
                    ].join("\n")
                }
            ]
        }

        compare(model.dashboardMetricRawValue("messaging.network_ingress_recent"), 20)
        compare(model.dashboardMetricRawValue("messaging.store_query_requests_recent"), 4)
        compare(model.dashboardMetricRawValue("messaging.store_messages"), 7)
    }

    function test_storage_active_operation_state_updates_revision() {
        const before = model.storageActiveOperationRevision

        model.updateStorageActiveOperation({ operationId: "op-1", status: "running" })

        verify(model.storageActiveOperationRevision > before)
        compare(model.storageActiveOperation.operationId, "op-1")

        model.clearStorageActiveOperation()

        compare(model.storageActiveOperation, null)
    }

    function test_node_operation_start_dispatches_generic_request() {
        fakeHost.responses = {
            runtimeOperationStart: {
                ok: true,
                value: {
                    operationId: "op-2",
                    domain: "delivery",
                    method: "deliverySend",
                    status: "running",
                    label: "Send message"
                },
                text: "OK",
                error: ""
            }
        }
        let seen = null

        model.runtimeOperationStart({
            domain: "delivery",
            method: "deliverySend",
            args: ["rest", "http://127.0.0.1:8645", true, "/topic/1/a/proto", "hello"],
            label: "Send message"
        }, false, function (response) {
            seen = response
        })

        tryVerify(function () { return seen !== null })
        compare(fakeHost.lastMethod, "runtimeOperationStart")
        compare(fakeHost.lastArgs[0].method, "deliverySend")
        compare(model.runtimeOperations["op-2"].domain, "delivery")
    }

    function test_node_operation_history_filters_by_domain() {
        const storageOperation = {
            operationId: "op-storage",
            domain: "storage",
            method: "storageFetch",
            status: "completed",
            label: "Cache CID",
            result: { cid: "z-storage" }
        }
        const deliveryOperation = {
            operationId: "op-delivery",
            domain: "delivery",
            method: "deliverySend",
            status: "failed",
            label: "Send message",
            error: "send failed"
        }

        model.appendRuntimeOperationHistory(storageOperation, "")
        model.appendRuntimeOperationHistory(deliveryOperation, "")

        const storageRows = model.runtimeOperationHistoryRows("storage")
        compare(storageRows.length, 1)
        compare(storageRows[0].operationId, "op-storage")
        compare(storageRows[0].detail, "z-storage")
        compare(model.runtimeOperationHistoryRows("delivery")[0].detail, "send failed")
    }

    function test_wallet_profile_configured_accepts_checked_env_home_source() {
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = ""
        model.localWalletStatus = {
            status: "ok",
            home_source: "LEE_WALLET_HOME_DIR"
        }

        verify(model.walletHomeConfigured())
        verify(model.walletProfileConfigured())
    }

    function test_transfer_recipient_lookup_uses_overflow_rows() {
        model.transferActivityRows = [
            { recipient: "visible", account_ref: "visible", source: "transfer_outputs", transfers: [] }
        ]
        model.transferActivityOverflowRows = [
            { recipient: "overflow", account_ref: "overflow", source: "transfer_outputs", transfers: [] }
        ]

        const detail = model.chainPages.transferRecipientDetailById("overflow")

        verify(detail !== null)
        compare(detail.address, "overflow")
    }

    function test_navigation_delegates() {
        compare(model.viewTitle(), "Dashboard")
        verify(model.navRows().length > 0)

        model.selectView("programs")

        compare(model.currentView, "programs")
        compare(model.parentNavKeyForView("programs"), "l2")
        compare(model.navTokenForView("programs"), "PRG")
    }

    function test_favorites_toggle_and_filter_rows() {
        const blockEntry = model.favoriteStore.blockEntry({
            type: "blockchain_block",
            hash: "block-hash",
            slot: 12,
            height: 12
        })
        const txEntry = model.favoriteStore.transactionEntry({
            mode: "lez",
            hash: "tx-hash",
            kind: "transfer"
        })

        verify(blockEntry !== null)
        compare(blockEntry.kind, "block")
        compare(blockEntry.layer, "l1")
        verify(txEntry !== null)
        compare(txEntry.kind, "transaction")
        compare(txEntry.layer, "l2")

        verify(model.favoriteStore.add(blockEntry))
        verify(model.favoriteStore.add(txEntry))
        compare(model.favoriteStore.count("all"), 2)
        compare(model.favoriteStore.count("block"), 1)
        compare(model.favoriteStore.rows("block")[0].value, "block-hash")
        verify(model.favoriteStore.isFavoriteEntry(blockEntry))

        verify(model.favoriteStore.toggle(blockEntry))
        verify(!model.favoriteStore.isFavoriteEntry(blockEntry))
        compare(model.favoriteStore.count("all"), 1)
    }

    function test_favorites_persist_in_settings_state() {
        fakeHost.responses = {
            loadSettingsState: {
                ok: true,
                value: {
                    favorites: [
                        {
                            kind: "account",
                            layer: "l2",
                            value: "account-1",
                            open_kind: "account",
                            title: "Account account-1",
                            created_at: "2026-07-05T00:00:00.000Z"
                        }
                    ]
                },
                text: "OK",
                error: ""
            }
        }

        model.loadSettingsState()

        compare(model.favoriteStore.entries.length, 1)
        compare(model.favoriteStore.entries[0].value, "account-1")
        compare(model.settingsStatePayload().favorites.length, 1)

        fakeHost.callCount = 0
        fakeHost.lastMethod = ""
        fakeHost.lastArgs = []
        const txEntry = {
            kind: "transaction",
            layer: "l1",
            value: "tx-1",
            open_kind: "mantleTransaction",
            title: "Mantle transaction tx-1",
            created_at: "2026-07-05T00:01:00.000Z"
        }

        verify(model.favoriteStore.add(txEntry))

        compare(fakeHost.lastMethod, "saveSettingsState")
        compare(fakeHost.lastArgs[0].favorites.length, 2)
    }

    function test_social_settings_round_trip_identity_and_shared_idl_policy() {
        fakeHost.responses = {
            loadSettingsState: {
                ok: true,
                value: {
                    social_identities: [
                        {
                            key: "local-1",
                            display_name: "Ada",
                            local_id: "local-1",
                            key_material: "secret",
                            created_at: "2026-07-05T00:00:00.000Z"
                        }
                    ],
                    social_identity_default_mode: "manual",
                    social_selected_identity_key: "local-1",
                    social_conversation_identity_keys: {
                        "/lez/account/a/comments": "local-1"
                    },
                    shared_idl_policy: "sessionOnly",
                    shared_idl_auto_share: true
                },
                text: "OK",
                error: ""
            }
        }

        model.loadSettingsState()

        compare(model.socialIdentities.count, 1)
        compare(model.socialIdentities.get(0).displayName, "Ada")
        compare(model.socialIdentityDefaultMode, "manual")
        compare(model.selectedSocialIdentityKey, "local-1")
        compare(model.sharedIdlPolicy, "sessionOnly")
        compare(model.sharedIdlAutoShare, true)
        const payload = model.settingsStatePayload()
        compare(payload.social_identities.length, 1)
        compare(payload.social_identity_default_mode, "manual")
        compare(payload.shared_idl_policy, "sessionOnly")
        compare(payload.shared_idl_auto_share, true)
    }

    function test_social_comment_topics_for_supported_detail_kinds() {
        fakeHost.responses = {
            socialCommentTopic: function(args) {
                const layer = String(args[0] || "")
                const entity = String(args[1] || "")
                const id = String(args[2] || "")
                if (id.indexOf("/") >= 0) {
                    return { ok: true, value: "", text: "", error: "" }
                }
                return { ok: true, value: "/" + layer + "/" + entity + "/" + id + "/comments", text: "OK", error: "" }
            },
            socialLezAccountIdlTopic: function(args) {
                const account = String(args[0] || "")
                return { ok: true, value: account.length ? "/lez/account/" + account + "/idl" : "", text: "OK", error: "" }
            }
        }

        compare(model.socialCommentTopic("cryptarchia", "transaction", "tx-1"), "/cryptarchia/transaction/tx-1/comments")
        compare(model.socialCommentTopic("cryptarchia", "block", "block-1"), "/cryptarchia/block/block-1/comments")
        compare(model.socialCommentTopic("cryptarchia", "account", "account-1"), "/cryptarchia/account/account-1/comments")
        compare(model.socialCommentTopic("lez", "transaction", "tx-2"), "/lez/transaction/tx-2/comments")
        compare(model.socialCommentTopic("lez", "block", "102"), "/lez/block/102/comments")
        compare(model.socialCommentTopic("lez", "account", "account-2"), "/lez/account/account-2/comments")
        compare(model.socialLezAccountIdlTopic("account-2"), "/lez/account/account-2/idl")
        compare(model.socialCommentTopic("lez", "account", "bad/id"), "")
    }

    function test_social_comment_paging_appends_without_replacing_rows() {
        const first = [{
            key: "cursor-1",
            cursor: "cursor-1",
            displayName: "Ada",
            body: "first",
            createdAt: "2026-07-05T00:00:00.000Z"
        }]
        const second = [{
            key: "cursor-2",
            cursor: "cursor-2",
            displayName: "Bea",
            body: "second",
            createdAt: "2026-07-05T00:01:00.000Z"
        }]

        model.setSocialCommentState("/lez/account/a/comments", {
            rows: first,
            cursor: "cursor-1",
            loading: false,
            error: "",
            exhausted: false
        })
        const merged = model.mergeSocialCommentRows(model.socialComments("/lez/account/a/comments"), second)

        compare(merged.length, 2)
        compare(merged[0].body, "first")
        compare(merged[1].body, "second")
    }

    function test_social_identity_default_creates_per_topic_and_reuses_same_topic() {
        model.settingsStateLoaded = true

        const first = model.socialIdentityForConversation("/lez/account/a/comments", "")
        const again = model.socialIdentityForConversation("/lez/account/a/comments", "")
        const second = model.socialIdentityForConversation("/lez/account/b/comments", "")

        compare(model.socialIdentities.count, 2)
        compare(first.key, again.key)
        verify(first.key !== second.key)
        compare(model.socialConversationIdentityKeys["/lez/account/a/comments"], first.key)
        compare(fakeHost.lastMethod, "saveSettingsState")
    }

    function test_shared_idl_policies_store_register_or_ignore_verified_entries() {
        model.idlStateLoaded = true
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programId: "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            programIdHex: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedTopic: "/lez/account/account-1/idl",
            sharedIdentity: { display_name: "Ada" },
            sharedAccountId: "account-1",
            accountType: "State"
        }

        model.setSharedIdlPolicy("disabled")
        verify(!model.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.sharedIdlSuggestions("account-1").length, 0)

        model.setSharedIdlPolicy("suggestion")
        verify(model.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.sharedIdlSuggestions("account-1").length, 1)
        compare(model.registeredIdls.count, 0)

        model.socialSharedIdls = ({})
        model.setSharedIdlPolicy("sessionOnly")
        verify(model.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.sharedIdlEntriesForAccount("account-1", sharedEntry.programIdHex).length, 1)
        compare(model.registeredIdls.count, 0)

        model.setSharedIdlPolicy("autoRegister")
        verify(model.applySharedIdlPolicy("account-1", sharedEntry))
        compare(model.registeredIdls.count, 1)
        compare(model.registeredIdls.get(0).source, "shared")
        compare(model.idlEntryAt(0).accountType, "State")
    }

    function test_shared_idl_policy_rejects_wrong_account_or_non_shared_entries() {
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programIdHex: "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedAccountId: "account-2",
            accountType: "State"
        }
        const localEntry = {
            key: "local-1",
            name: "Local",
            programIdHex: sharedEntry.programIdHex,
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local",
            sharedAccountId: "account-1",
            accountType: "State"
        }
        model.setSharedIdlPolicy("suggestion")

        verify(!model.applySharedIdlPolicy("account-1", sharedEntry))
        verify(!model.applySharedIdlPolicy("account-1", localEntry))
        compare(model.sharedIdlSuggestions("account-1").length, 0)
        compare(model.registeredIdls.count, 0)
    }

    function test_local_idl_priority_beats_shared_match() {
        const programIdHex = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        const localEntry = {
            key: "local-1",
            name: "Local",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local",
            sharedTopic: "",
            sharedIdentity: {},
            sharedAccountId: ""
        }
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedTopic: "/lez/account/account-1/idl",
            sharedIdentity: {},
            sharedAccountId: "account-1",
            accountType: "State"
        }
        model.registeredIdls.append(localEntry)
        model.setSharedIdlPolicy("sessionOnly")
        model.applySharedIdlPolicy("account-1", sharedEntry)
        model.cacheAccountIdlSelection("account-1", sharedEntry, "State", programIdHex)

        const candidates = model.accountDecodeCandidates("account-1", programIdHex)

        compare(candidates.length, 2)
        compare(candidates[0].entry.key, "local-1")
        compare(candidates[1].entry.key, "shared-1")
    }

    function test_registered_shared_idl_keeps_local_priority_and_account_type() {
        const programIdHex = "1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        const sharedEntry = {
            key: "shared-1",
            name: "Shared",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Shared\",\"accounts\":[]}",
            source: "shared",
            sharedTopic: "/lez/account/account-1/idl",
            sharedIdentity: {},
            sharedAccountId: "account-1",
            accountType: "State"
        }
        const localEntry = {
            key: "local-1",
            name: "Local",
            programId: "0x" + programIdHex,
            programIdHex: programIdHex,
            programBinary: "",
            json: "{\"name\":\"Local\",\"accounts\":[]}",
            source: "local",
            sharedTopic: "",
            sharedIdentity: {},
            sharedAccountId: "",
            accountType: ""
        }

        model.setSharedIdlPolicy("autoRegister")
        verify(model.applySharedIdlPolicy("account-1", sharedEntry))
        model.registeredIdls.append(localEntry)

        const entries = model.idlEntriesForProgram(programIdHex)
        compare(entries.length, 2)
        compare(entries[0].key, "local-1")
        compare(entries[1].key, "shared-1")
        compare(entries[1].accountType, "State")

        const candidates = model.accountDecodeCandidates("account-1", programIdHex)
        const payload = model.programDecodeCandidatePayload(candidates)
        compare(candidates[0].entry.key, "local-1")
        compare(candidates[1].entry.key, "shared-1")
        compare(payload[1].accountType, "State")
    }

    function test_settings_backup_to_storage_uses_wallet_profile_and_persists_cid() {
        model.settingsStateLoaded = true
        model.idlStateLoaded = true
        model.walletStateLoaded = true
        model.storageMutatingDiagnosticsEnabled = true
        model.walletHome = "/tmp/wallet-home"
        model.settingsBackupEncrypted = true
        fakeHost.responses = {
            storageBackupSettings: {
                ok: true,
                value: {
                    cid: "cid-backup",
                    encrypted: true
                },
                text: "OK",
                error: ""
            }
        }

        verify(model.backupSettingsToStorage(true))

        const backupCalls = fakeHost.calls.filter(function (call) {
            return call.method === "storageBackupSettings"
        })
        compare(backupCalls.length, 1)
        compare(backupCalls[0].args[0], "rest")
        compare(backupCalls[0].args[1], model.configuredStorageRestUrl())
        compare(backupCalls[0].args[2], true)
        compare(backupCalls[0].args[3], true)
        compare(backupCalls[0].args[4].wallet_home, "/tmp/wallet-home")
        compare(model.settingsBackupCid, "cid-backup")
        compare(model.settingsRestoreCid, "cid-backup")
    }

    function test_settings_restore_from_storage_reloads_local_state() {
        model.settingsStateLoaded = true
        model.idlStateLoaded = true
        model.walletStateLoaded = true
        model.storageMutatingDiagnosticsEnabled = true
        model.walletHome = "/tmp/wallet-home"
        model.settingsBackupEncrypted = true
        fakeHost.responses = {
            storageRestoreSettings: {
                ok: true,
                value: {
                    restored: true,
                    encrypted: true,
                    idl_count: 2,
                    favorites: 3
                },
                text: "OK",
                error: ""
            },
            loadSettingsState: {
                ok: true,
                value: {
                    favorites: []
                },
                text: "OK",
                error: ""
            },
            loadIdlState: {
                ok: true,
                value: {
                    idls: [],
                    account_idl_selections: {}
                },
                text: "OK",
                error: ""
            },
            loadWalletState: {
                ok: true,
                value: {
                    profile: {
                        label: "Local wallet",
                        wallet_home: "/tmp/wallet-home"
                    },
                    operations: []
                },
                text: "OK",
                error: ""
            }
        }

        verify(model.restoreSettingsFromStorage("cid-restore", true))

        const restoreCalls = fakeHost.calls.filter(function (call) {
            return call.method === "storageRestoreSettings"
        })
        compare(restoreCalls.length, 1)
        compare(restoreCalls[0].args[3], "cid-restore")
        compare(restoreCalls[0].args[4].wallet_home, "/tmp/wallet-home")
        verify(fakeHost.calls.some(function (call) { return call.method === "loadSettingsState" }))
        verify(fakeHost.calls.some(function (call) { return call.method === "loadIdlState" }))
        verify(fakeHost.calls.some(function (call) { return call.method === "loadWalletState" }))
        compare(model.settingsBackupCid, "cid-restore")
        verify(model.settingsBackupStatus.indexOf("2 IDLs") >= 0)
    }

    function test_navigation_history_tracks_page_selection() {
        verify(!model.canNavigateBack())
        verify(!model.canNavigateForward())

        model.selectView("blocks")

        compare(model.currentView, "blocks")
        verify(model.canNavigateBack())
        compare(model.navigationBackLabel(), "Dashboard")
        verify(!model.canNavigateForward())

        model.selectView("transactions")

        compare(model.currentView, "transactions")
        compare(model.navigationBackStack.length, 2)

        model.navigateBack()

        compare(model.currentView, "blocks")
        verify(model.canNavigateBack())
        verify(model.canNavigateForward())
        compare(model.navigationForwardLabel(), "Mantle Tx")

        model.selectView("programs")

        compare(model.currentView, "programs")
        verify(!model.canNavigateForward())
    }

    function test_navigation_history_restores_detail_state() {
        model.currentView = "blockDetail"
        model.blockDetailValue = { type: "blockchain_block", hash: "old-block", slot: 1 }
        model.resultTitle = "Block"
        model.resultText = "old result"
        model.resultValue = { hash: "old-block" }
        model.resultOwner = "blockDetail"

        model.pushNavigationHistory()

        model.blockDetailValue = { type: "blockchain_block", hash: "new-block", slot: 2 }
        model.resultText = "new result"
        model.resultValue = { hash: "new-block" }

        compare(model.navigationBackLabel(), "Block old-block")

        model.navigateBack()

        compare(model.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "old-block")
        compare(model.resultText, "old result")
        compare(model.resultOwner, "blockDetail")
        verify(model.canNavigateForward())

        model.navigateForward()

        compare(model.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "new-block")
        compare(model.resultText, "new result")
    }

    function test_navigation_history_records_deep_block_opener() {
        model.currentView = "blockDetail"
        model.blockDetailValue = { type: "blockchain_block", hash: "old-block", slot: 1 }
        model.resultTitle = "Block"
        model.resultText = "old result"
        model.resultValue = { hash: "old-block" }
        model.resultOwner = "blockDetail"
        model.blocksPageRows = [
            { header: { slot: 7, id: "new-block" }, transactions: [] }
        ]

        model.openBlockchainBlock("7")

        compare(model.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "new-block")
        compare(model.navigationBackStack.length, 1)

        model.navigateBack()

        compare(model.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "old-block")
        compare(model.resultText, "old result")

        model.navigateForward()

        compare(model.currentView, "blockDetail")
        verify(model.blockDetailValue !== null)
        compare(model.blockDetailValue.hash, "new-block")
    }

    function test_dashboard_metric_history_prefix_clear() {
        model.dashboardMetricHistory = {
            "messaging.messages": [{ timestamp: 1, value: 1 }],
            "storage.files": [{ timestamp: 1, value: 2 }],
            "chain.height": [{ timestamp: 1, value: 3 }]
        }
        model.dashboardMetricLastSeen = {
            "messaging.messages": { timestamp: 2, value: 1 },
            "storage.files": { timestamp: 2, value: 2 }
        }

        model.clearDashboardMetricHistoryForPrefix("messaging.")

        compare(model.dashboardMetricHistory["messaging.messages"], undefined)
        compare(model.dashboardMetricLastSeen["messaging.messages"], undefined)
        verify(model.dashboardMetricHistory["storage.files"] !== undefined)
        verify(model.dashboardMetricLastSeen["storage.files"] !== undefined)
        verify(model.dashboardMetricHistory["chain.height"] !== undefined)
        compare(model.dashboardMetricHistoryRevision, 1)
    }

    function test_dashboard_metric_history_keeps_pre_change_sample() {
        const values = [100, 100, 100, 100, 100, 101, 101, 101, 101, 102, 101, 101, 101, 102]
        for (let i = 0; i < values.length; ++i) {
            setTipMinusLib(values[i])
            model.recordDashboardSnapshot()
        }

        const samples = model.dashboardMetricHistory["bedrock.tip_minus_lib"]
        const storedValues = samples.map(function (sample) {
            return sample.value
        })

        compare(storedValues.length, 8)
        compare(JSON.stringify(storedValues), JSON.stringify([100, 100, 101, 101, 102, 101, 101, 102]))
        for (let j = 1; j < samples.length; ++j) {
            verify(samples[j].timestamp > samples[j - 1].timestamp)
        }
    }

    function test_dashboard_metric_history_keeps_300_samples() {
        for (let i = 0; i < 310; ++i) {
            setTipMinusLib(i)
            model.recordDashboardSnapshot()
        }

        const samples = model.dashboardMetricHistory["bedrock.tip_minus_lib"]

        compare(samples.length, 300)
        compare(samples[0].value, 10)
        compare(samples[299].value, 309)
    }

    function test_idl_registration_delegates() {
        const programId = "0x1234567890abcdef1234567890abcdef1234567890abcdef1234567890abcdef"
        const idlJson = JSON.stringify({
            name: "Sample",
            instructions: [],
            accounts: []
        })

        model.idlStateLoaded = true
        model.registerIdl("", programId, idlJson)

        compare(model.registeredIdls.count, 1)
        compare(model.registeredIdls.get(0).name, "Sample")
        compare(model.registeredIdls.get(0).programIdHex, programId.slice(2))
        compare(fakeHost.lastMethod, "saveIdlState")
    }

    function test_deploy_program_binary_uses_wallet_confirmation_and_logs_execution_operation() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        fakeHost.responses = {
            localWalletDeployProgram: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "submitted",
                    program_id_hex: "abc123",
                    deployment_tx_hash: "tx123"
                },
                text: "OK",
                error: ""
            }
        }

        model.deployProgramBinary("/tmp/program.bin")

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletDeployProgram"
            })
        })
        const deployCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletDeployProgram"
        })
        compare(deployCalls.length, 1)
        compare(deployCalls[0].args[0].wallet_binary, "/usr/bin/lee-wallet")
        compare(deployCalls[0].args[0].wallet_home, "/tmp/wallet-home")
        compare(deployCalls[0].args[1], "/tmp/program.bin")
        compare(deployCalls[0].args[2], "confirm-deploy-program")
        compare(model.localWalletOperations.length, 0)
        const history = model.runtimeOperationHistoryRows("execution")
        compare(history.length, 1)
        compare(history[0].label, "Program deploy")
        compare(history[0].status, "completed")
    }

    function test_preview_idl_instruction_uses_execution_adapter() {
        fakeHost.responses = {
            localWalletInstructionPreview: {
                ok: true,
                value: {
                    mode: "preview",
                    instruction: "transfer",
                    instruction_words: ["0x01", "0x02"]
                },
                text: "OK",
                error: ""
            }
        }

        model.previewIdlInstruction({ instruction: "transfer" })

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletInstructionPreview"
            })
        })
        const previewCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletInstructionPreview"
        })
        compare(previewCalls.length, 1)
        compare(previewCalls[0].args[0].instruction, "transfer")
        compare(model.idlInstructionPreviewValue.mode, "preview")
        compare(model.idlInstructionError, "")
        compare(model.localWalletOperations.length, 0)
    }

    function test_send_idl_instruction_uses_execution_confirmation_and_logs_operation() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        fakeHost.responses = {
            localWalletInstructionSubmit: {
                ok: true,
                value: {
                    mode: "tx",
                    instruction: "transfer",
                    tx_hash: "0xabcdef123456"
                },
                text: "OK",
                error: ""
            }
        }

        model.sendIdlInstruction({ instruction: "transfer" })

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletInstructionSubmit"
            })
        })
        const instructionCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletInstructionSubmit"
        })
        compare(instructionCalls.length, 1)
        compare(instructionCalls[0].args[0].wallet_binary, "/usr/bin/lee-wallet")
        compare(instructionCalls[0].args[0].wallet_home, "/tmp/wallet-home")
        compare(instructionCalls[0].args[1].instruction, "transfer")
        compare(instructionCalls[0].args[2], "confirm-idl-instruction")
        compare(model.localWalletOperations.length, 0)
        compare(model.idlInstructionPreviewValue.mode, "tx")
        compare(model.idlInstructionError, "")
        const history = model.runtimeOperationHistoryRows("execution")
        compare(history.length, 1)
        compare(history[0].label, "IDL instruction")
        compare(history[0].status, "completed")
    }

    function test_create_wallet_account_uses_confirmation_and_logs_operation() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        model.walletCreatePrivacy = "private"
        model.walletCreateLabel = "receiver"
        fakeHost.responses = {
            localWalletCreateAccount: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "created",
                    command: "wallet account new private",
                    account_id: "Private/abc123"
                },
                text: "OK",
                error: ""
            }
        }

        model.createWalletAccount()

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletCreateAccount"
            })
        })
        const calls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletCreateAccount"
        })
        compare(calls.length, 1)
        compare(calls[0].args[0].wallet_binary, "/usr/bin/lee-wallet")
        compare(calls[0].args[1], "private")
        compare(calls[0].args[2], "receiver")
        compare(calls[0].args[3], "confirm-create-account")
        compare(model.walletCreateLabel, "")
        compare(model.localWalletOperations[0].label, "Create account")
        compare(model.localWalletOperations[0].status, "created")
        compare(model.runtimeOperationHistoryRows("wallet")[0].label, "Create account")
    }

    function test_send_wallet_transaction_uses_confirmation_and_logs_operation() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        model.walletSendFrom = "Public/source"
        model.walletSendTo = "Private/recipient"
        model.walletSendAmount = "37"
        fakeHost.responses = {
            localWalletSendTransaction: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "submitted",
                    command: "wallet auth-transfer send",
                    tx_hash: "tx123"
                },
                text: "OK",
                error: ""
            }
        }

        model.sendWalletTransaction()

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletSendTransaction"
            })
        })
        const calls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletSendTransaction"
        })
        compare(calls.length, 1)
        compare(calls[0].args[1].from, "Public/source")
        compare(calls[0].args[1].to, "Private/recipient")
        compare(calls[0].args[1].amount, "37")
        compare(calls[0].args[2], "confirm-send-transaction")
        compare(model.localWalletOperations[0].label, "Send transaction")
        compare(model.localWalletOperations[0].status, "submitted")
    }

    function test_read_incoming_wallet_transactions_uses_private_sync_confirmation() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        fakeHost.responses = {
            localWalletSyncPrivate: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "submitted",
                    wallet_home_source: "profile"
                },
                text: "OK",
                error: ""
            }
        }

        model.readIncomingWalletTransactions()

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletSyncPrivate"
            })
        })
        const syncCalls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletSyncPrivate"
        })
        compare(syncCalls.length, 1)
        compare(syncCalls[0].args[1], "confirm-sync-private")
        compare(model.localWalletOperations[0].label, "Read incoming")
        compare(model.localWalletOperations[0].status, "submitted")
    }

    function test_run_wallet_command_uses_confirmation_and_logs_operation() {
        model.walletStateLoaded = true
        model.walletBinary = "/usr/bin/lee-wallet"
        model.walletHome = "/tmp/wallet-home"
        fakeHost.responses = {
            localWalletCommand: {
                ok: true,
                value: {
                    source: "local_wallet_cli",
                    status: "completed",
                    command: "wallet account get --account-id Public/abc"
                },
                text: "OK",
                error: ""
            }
        }

        model.runWalletCommand(["account", "get", "--account-id", "Public/abc"])

        tryVerify(function () {
            return fakeHost.calls.some(function (call) {
                return call.method === "localWalletCommand"
            })
        })
        const calls = fakeHost.calls.filter(function (call) {
            return call.method === "localWalletCommand"
        })
        compare(calls.length, 1)
        compare(calls[0].args[1][0], "account")
        compare(calls[0].args[1][2], "--account-id")
        compare(calls[0].args[2], "confirm-wallet-command")
        compare(model.localWalletOperations[0].label, "Wallet command")
        compare(model.localWalletOperations[0].status, "completed")
    }

    function test_blocks_page_uses_tip_range_and_blocks_backend() {
        fakeHost.responses = {
            blockchainNode: {
                ok: true,
                value: {
                    cryptarchia_info: {
                        value: {
                            cryptarchia_info: {
                                slot: 30,
                                lib_slot: 20
                            }
                        }
                    }
                },
                text: "OK",
                error: ""
            },
            blockchainBlocks: {
                ok: true,
                value: [
                    { header: { slot: 30, id: "tip" }, transactions: [], _chain: { status: "pending" } },
                    { header: { slot: 20, id: "lib" }, transactions: [], _chain: { status: "finalized" } }
                ],
                text: "OK",
                error: ""
            }
        }

        model.chainPages.refreshBlocksPage()

        compare(fakeHost.lastMethod, "blockchainBlocks")
        compare(fakeHost.lastArgs[1], 0)
        compare(fakeHost.lastArgs[2], 30)
        compare(fakeHost.lastArgs[3], 20)
        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.slot, 30)
        compare(model.chainPages.blockStatus(model.blocksPageRows[0]), "pending")
        compare(model.chainPages.blockStatus(model.blocksPageRows[1]), "finalized")
    }

    function test_blocks_live_mode_merges_and_dedupes_snapshot() {
        model.currentView = "blocks"
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30
        fakeHost.responses = {
            blockchainNode: {
                ok: true,
                value: {
                    cryptarchia_info: {
                        value: {
                            cryptarchia_info: {
                                slot: 31,
                                lib_slot: 20
                            }
                        }
                    }
                },
                text: "OK",
                error: ""
            },
            blockchainLiveBlocks: {
                ok: true,
                value: {
                    source: "blocks_range",
                    blocks: [
                        { header: { slot: 31, id: "slot-31" }, transactions: [] },
                        { header: { slot: 30, id: "slot-30-live" }, transactions: [] }
                    ],
                    unknown_events: [
                        { kind: "heartbeat" }
                    ]
                },
                text: "live",
                error: ""
            }
        }

        compare(model.chainPages.mergeLiveBlocks(fakeHost.responses.blockchainLiveBlocks.value.blocks, model.blocksPageRows, 20).length, 2)
        model.chainPages.startBlocksLiveMode()

        compare(model.blocksLiveEnabled, true)
        compare(fakeHost.lastMethod, "blockchainLiveBlocks")
        compare(fakeHost.lastArgs[1], 30)
        compare(fakeHost.lastArgs[2], 31)
        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.id, "slot-31")
        compare(model.blocksPageRows[1].header.id, "slot-30-live")
        compare(model.blocksLiveSource, "blocks_range")
        compare(model.blocksLiveUnknownEvents, 1)
        compare(model.resultOwner, "blocks")
        compare(model.resultValue.unknown_events.length, 1)
    }

    function test_blockchain_module_new_block_event_updates_live_rows() {
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]
        model.blocksPageSlotFrom = 30
        model.blocksPageSlotTo = 30

        moduleEventIntake.ingest(model.blockchainModule, "newBlock", [
            JSON.stringify({ header: { slot: 31, id: "slot-31-event" }, transactions: [] })
        ])

        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.id, "slot-31-event")
        compare(model.blocksLiveSource, "module_event")
        compare(model.blocksPageSlotTo, 31)
        verify(model.blockchainModuleEventRevision > 0)
    }

    function test_delivery_module_message_event_merges_social_comment() {
        const topic = "/lez/account/account-1/comments"
        const payload = {
            kind: "comment",
            version: 1,
            identity: { display_name: "Peer" },
            body: "hello",
            created_at: "2026-07-07T00:00:00Z",
            conversation_id: topic
        }

        moduleEventIntake.ingest(model.deliveryModule, "messageReceived", [
            "hash-1",
            topic,
            JSON.stringify(payload),
            "1000"
        ])

        compare(model.deliveryModuleEventRows()[0].label, "messageReceived")
        compare(model.socialComments(topic).length, 1)
        compare(model.socialComments(topic)[0].body, "hello")
    }

    function test_stop_blocks_live_mode_keeps_paged_rows() {
        model.blocksLiveEnabled = true
        model.blocksLiveSource = "blocks_range+stream"
        model.blocksLiveUnknownEvents = 1
        model.blocksLiveCheckedAt = "10:00:00"
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]

        model.chainPages.stopBlocksLiveMode()

        compare(model.blocksLiveEnabled, false)
        compare(model.blocksLiveError, "")
        compare(model.blocksLiveSource, "")
        compare(model.blocksLiveUnknownEvents, 0)
        compare(model.blocksLiveCheckedAt, "")
        compare(model.blocksPageRows.length, 1)
        compare(model.blocksPageRows[0].header.id, "slot-30")
    }

    function test_lez_blocks_page_merges_sequencer_and_indexer_blocks() {
        fakeHost.responses = {
            sequencerBlocks: {
                ok: true,
                value: [
                    { block_id: 102, header_hash: "seq-102", tx_count: 0, bedrock_status: "Submitted", transactions: [] },
                    { block_id: 101, header_hash: "seq-101", tx_count: 1, bedrock_status: "Submitted", transactions: [{ hash: "tx-101", instruction_data: [1] }] }
                ],
                text: "OK",
                error: ""
            },
            indexerBlocks: {
                ok: true,
                value: [
                    { block_id: 100, header_hash: "idx-100", tx_count: 0, bedrock_status: "Finalized", transactions: [] }
                ],
                text: "OK",
                error: ""
            }
        }

        model.chainPages.refreshLezBlocksPage()

        tryCompare(model, "lezBlocksPageLoading", false)
        compare(model.lezBlocksPageRows.length, 3)
        compare(model.lezBlocksPageRows[0].block_id, 102)
        compare(model.lezBlocksPageRows[0].source, "sequencer")
        compare(model.lezBlocksPageRows[2].block_id, 100)
        compare(model.lezBlocksPageRows[2].source, "indexer")
        compare(model.lezBlocksPageNextBeforeBlock, 100)

        model.openReference("indexerBlock", "seq-102", model.lezBlocksPageRows[0])

        compare(model.currentView, "l2BlockDetail")
        compare(model.blockDetailValue.type, "sequencer_block")
        compare(model.blockDetailValue.status, "Submitted")
    }

    function test_lez_blocks_page_finishes_from_first_available_source() {
        model.chainPages.finishLezBlocksPage(0, {
            ok: true,
            value: [
                { block_id: 203, header_hash: "seq-203", tx_count: 0, bedrock_status: "Submitted", transactions: [] }
            ],
            text: "OK",
            error: ""
        }, null)

        compare(model.lezBlocksPageRows.length, 1)
        compare(model.lezBlocksPageRows[0].block_id, 203)
        compare(model.lezBlocksPageRows[0].source, "sequencer")
        compare(model.lezBlocksPageError, "")
    }

    function test_lez_transactions_older_consumes_overflow_rows_before_fetching_more_blocks() {
        model.lezTransactionsPageLimit = 2
        model.lezTransactionsBlockBatch = 2
        fakeHost.responses = {
            indexerBlocks: {
                ok: true,
                value: [
                    {
                        block_id: 12,
                        header_hash: "block-12",
                        transactions: [
                            { hash: "tx-1", instruction_data: [1] },
                            { hash: "tx-2", instruction_data: [2] },
                            { hash: "tx-3", instruction_data: [3] }
                        ]
                    }
                ],
                text: "OK",
                error: ""
            }
        }

        model.chainPages.refreshLezTransactionsPage()
        const callsAfterFirstPage = fakeHost.callCount

        compare(model.lezTransactionsPageRows.length, 2)
        compare(model.lezTransactionsPageRows[0].hash, "tx-1")
        compare(model.lezTransactionsPageOverflowRows.length, 1)

        model.chainPages.olderLezTransactionsPage()

        compare(fakeHost.callCount, callsAfterFirstPage)
        compare(model.lezTransactionsPageRows.length, 1)
        compare(model.lezTransactionsPageRows[0].hash, "tx-3")
        compare(model.lezTransactionsPageOverflowRows.length, 0)
    }

    function test_transfer_activity_older_consumes_overflow_rows_before_fetching_more_blocks() {
        model.transferActivityLimit = 2
        fakeHost.responses = {
            indexerTransferRecipients: {
                ok: true,
                value: {
                    recipients: [
                        { recipient: "r1", last_slot: 12, transfer_count: 1 },
                        { recipient: "r2", last_slot: 11, transfer_count: 1 },
                        { recipient: "r3", last_slot: 10, transfer_count: 1 }
                    ],
                    next_before_block: 9
                },
                text: "OK",
                error: ""
            }
        }

        model.chainPages.refreshTransferActivityPage()
        const callsAfterFirstPage = fakeHost.callCount

        compare(model.transferActivityRows.length, 2)
        compare(model.transferActivityRows[0].recipient, "r1")
        compare(model.transferActivityOverflowRows.length, 1)

        model.chainPages.nextTransferActivityPage()

        compare(fakeHost.callCount, callsAfterFirstPage)
        compare(model.transferActivityRows.length, 1)
        compare(model.transferActivityRows[0].recipient, "r3")
        compare(model.transferActivityOverflowRows.length, 0)
        compare(model.transferActivityNextBeforeBlock, 9)
    }

    function test_indexer_status_falls_back_to_health_and_head() {
        model.currentView = "indexer"
        fakeHost.responses = {
            indexerStatus: {
                ok: true,
                value: {
                    state: "unavailable",
                    lastError: "Method not found",
                    raw: {
                        error: {
                            code: -32601,
                            message: "Method not found"
                        }
                    }
                },
                text: "status unavailable",
                error: ""
            },
            indexerHealth: {
                ok: true,
                value: {
                    status: "healthy",
                    health: "ok"
                },
                text: "healthy",
                error: ""
            },
            indexerFinalizedHead: {
                ok: true,
                value: 42,
                text: "42",
                error: ""
            }
        }

        model.refreshIndexerStatus()

        compare(fakeHost.lastMethod, "indexerFinalizedHead")
        compare(model.resultOwner, "indexer")
        compare(model.resultIsError, false)
        compare(model.resultValue.status.state, "unavailable")
        compare(model.resultValue.status.indexedBlockId, 42)
        compare(model.resultValue.indexer.health.ok, true)
        compare(model.resultValue.indexer.head.value, 42)
    }

    function test_blockchain_module_probe_value_reads_peer_id() {
        model.blockchainModuleReport = {
            module: model.blockchainModule,
            module_info: { ok: true, value: {}, label: "module", source: "logoscore modules" },
            probes: [
                {
                    label: "blockchain_module.get_peer_id",
                    source: "blockchain_module get_peer_id",
                    ok: true,
                    value: "peer-123",
                    error: null
                }
            ]
        }

        compare(model.moduleProbeValue("blockchain", "get_peer_id"), "peer-123")
    }

    function test_bedrock_wallet_known_addresses_unwraps_module_payload() {
        model.blockchainModuleReport = blockchainWalletReport("wallet_get_known_addresses", {
            runner: "plain logoscore",
            value: {
                result: {
                    value: {
                        addresses: [
                            "addr-1",
                            { address: "addr-2", label: "default" }
                        ]
                    }
                }
            }
        })

        const rows = model.bedrockWalletModuleKnownAddressRows()

        compare(rows.length, 2)
        compare(rows[0].address, "addr-1")
        compare(rows[1].address, "addr-2")
        compare(rows[1].label, "default")
    }

    function test_bedrock_wallet_empty_known_addresses_are_known_shape() {
        model.blockchainModuleReport = blockchainWalletReport("wallet_get_known_addresses", {
            result: {
                value: []
            }
        })

        compare(model.bedrockWalletModuleKnownAddressRows().length, 0)
        compare(model.bedrockWalletModuleListKnown("wallet_get_known_addresses"), true)
    }

    function test_bedrock_wallet_notes_rows_format_note_fields() {
        model.blockchainModuleReport = blockchainWalletReport("wallet_get_notes", {
            result: {
                value: {
                    notes: [
                        {
                            note_id: "note-1",
                            value: "42",
                            commitment: "cm-1",
                            nullifier: "nf-1",
                            tip: "tip-1"
                        }
                    ]
                }
            }
        })

        const rows = model.bedrockWalletModuleNoteRows()

        compare(rows.length, 1)
        compare(rows[0].id, "note-1")
        compare(rows[0].value, "42")
        compare(rows[0].commitment, "cm-1")
        compare(rows[0].nullifier, "nf-1")
        compare(rows[0].tip, "tip-1")
    }

    function test_bedrock_wallet_voucher_rows_format_commitments() {
        model.blockchainModuleReport = blockchainWalletReport("wallet_get_claimable_vouchers", {
            result: {
                value: {
                    claimable_vouchers: [
                        {
                            voucher_commitment: "voucher-cm",
                            nullifier_hash: "voucher-nf",
                            amount: "7",
                            header_id: "header-1"
                        }
                    ]
                }
            }
        })

        const rows = model.bedrockWalletModuleVoucherRows()

        compare(rows.length, 1)
        compare(rows[0].commitment, "voucher-cm")
        compare(rows[0].nullifier, "voucher-nf")
        compare(rows[0].value, "7")
        compare(rows[0].tip, "header-1")
    }

    function test_bedrock_wallet_module_failure_keeps_other_probes_readable() {
        model.blockchainModuleReport = {
            module: model.blockchainModule,
            module_info: { ok: true, value: {}, label: "module", source: "logoscore modules" },
            probes: [
                {
                    label: "blockchain_module.wallet_get_known_addresses",
                    source: "blockchain_module wallet_get_known_addresses",
                    ok: true,
                    value: { result: { value: ["addr-ok"] } },
                    error: null
                },
                {
                    label: "blockchain_module.wallet_get_notes(addr-ok)",
                    source: "blockchain_module wallet_get_notes addr-ok",
                    ok: false,
                    value: null,
                    error: "module unavailable"
                }
            ]
        }

        compare(model.bedrockWalletModuleKnownAddressRows().length, 1)
        compare(model.bedrockWalletModuleNoteRows().length, 0)
        compare(model.moduleProbeError("blockchain", "wallet_get_notes"), "module unavailable")
    }

    function test_bedrock_wallet_module_methods_are_read_only() {
        const methods = model.bedrockWalletModuleReadOnlyMethods()

        verify(methods.indexOf("wallet_get_known_addresses") >= 0)
        verify(methods.indexOf("wallet_get_balance") >= 0)
        verify(methods.indexOf("wallet_get_notes") >= 0)
        verify(methods.indexOf("wallet_get_claimable_vouchers") >= 0)
        compare(methods.filter(function (method) {
            return method.indexOf("wallet_get_") !== 0
        }).length, 0)
    }

    function test_source_empty_text_uses_sync_and_shape_state() {
        compare(model.chainPages.sourceEmptyText("indexer", "", "No indexed blocks"), "No indexed blocks")

        model.updateNetworkConnectionStatus("indexer", {
            ok: true,
            value: { state: "syncing", indexedBlockId: 12 },
            text: "syncing",
            error: ""
        })

        compare(model.chainPages.sourceEmptyText("indexer", "", "No indexed blocks"), "Source reachable; syncing")
        compare(model.chainPages.sourceProblemTitle("indexer", "Response shape unknown. Raw JSON remains available.", "L2 blocks unavailable"), "Response shape unknown")
    }

    function test_bedrock_network_summary_unwraps_probe_slot() {
        const value = {
            cryptarchia_info: {
                ok: true,
                value: {
                    cryptarchia_info: {
                        slot: 42
                    }
                }
            }
        }

        compare(model.networkConnectionSummary("blockchain", value), "slot 42")
    }

    function test_dashboard_refresh_loads_recent_blocks_for_both_chains() {
        fakeHost.responses = {
            blockchainNode: {
                ok: true,
                value: {
                    cryptarchia_info: {
                        value: {
                            cryptarchia_info: {
                                slot: 30,
                                lib_slot: 20
                            }
                        }
                    }
                },
                text: "OK",
                error: ""
            },
            blockchainBlocks: {
                ok: true,
                value: [
                    {
                        header: { slot: 30, id: "l1-tip" },
                        transactions: [{ mantle_tx: { hash: "l1-tx", ops: [{ opcode: 17 }] } }]
                    },
                    { header: { slot: 29, id: "l1-pending-2" }, transactions: [] },
                    { header: { slot: 28, id: "l1-pending-3" }, transactions: [] },
                    { header: { slot: 20, id: "l1-lib" }, transactions: [], _chain: { status: "finalized" } },
                    { header: { slot: 19, id: "l1-finalized-2" }, transactions: [], _chain: { status: "finalized" } }
                ],
                text: "OK",
                error: ""
            },
            sequencerBlocks: {
                ok: true,
                value: [
                    { block_id: 104, header_hash: "seq-104", tx_count: 0, bedrock_status: "Submitted", transactions: [] },
                    { block_id: 103, header_hash: "seq-103", tx_count: 0, bedrock_status: "Submitted", transactions: [] },
                    { block_id: 102, header_hash: "seq-102", tx_count: 1, bedrock_status: "Submitted", transactions: [{ hash: "l2-tx", instruction_data: [1, 2] }] }
                ],
                text: "OK",
                error: ""
            },
            indexerBlocks: {
                ok: true,
                value: [
                    { block_id: 101, header_hash: "idx-101", tx_count: 0, bedrock_status: "Finalized", transactions: [] },
                    { block_id: 100, header_hash: "idx-100", tx_count: 0, bedrock_status: "Finalized", transactions: [] }
                ],
                text: "OK",
                error: ""
            }
        }

        model.refreshDashboard()

        tryCompare(model, "dashboardRefreshing", false)
        compare(model.dashboardSequencerBlocks.length, 3)
        compare(model.dashboardSequencerBlocks[0].block_id, 104)
        compare(model.dashboardBlocks.length, 2)
        compare(model.dashboardBlocks[0].block_id, 101)
        compare(model.lezBlocksPageRows.length, 0)
    }

    function setTipMinusLib(value) {
        model.dashboardNode = {
            cryptarchia_info: {
                value: {
                    cryptarchia_info: {
                        slot: value,
                        lib_slot: 0
                    }
                }
            }
        }
    }

    function blockchainWalletReport(method, value) {
        return {
            module: model.blockchainModule,
            module_info: { ok: true, value: {}, label: "module", source: "logoscore modules" },
            probes: [
                {
                    label: "blockchain_module." + method,
                    source: "blockchain_module " + method,
                    ok: true,
                    value: value,
                    error: null
                }
            ]
        }
    }
}
