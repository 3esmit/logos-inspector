import QtQuick
import QtTest
import "../../qml/services"
import "../../qml/state"

TestCase {
    id: testRoot

    name: "AppModel"

    QtObject {
        id: fakeHost

        property int callCount: 0
        property string lastMethod: ""
        property var lastArgs: []
        property var calls: []
        property var responses: ({})

        function callModuleJson(moduleName, method, argsJson) {
            callCount += 1
            lastMethod = String(method || "")
            lastArgs = JSON.parse(String(argsJson || "[]"))
            calls = calls.concat([{ method: lastMethod, args: lastArgs }])
            const response = responses[lastMethod]
            if (response !== undefined) {
                return JSON.stringify(response)
            }
            return JSON.stringify({
                ok: true,
                value: {},
                text: "OK",
                error: ""
            })
        }
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

    AppModel {
        id: basecampModel

        bridge: basecampBridgeClient
    }

    function init() {
        fakeHost.callCount = 0
        fakeHost.lastMethod = ""
        fakeHost.lastArgs = []
        fakeHost.calls = []
        fakeHost.responses = ({})
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
        model.favorites = []
        model.favoritesRevision = 0
        model.favoritesFilter = "all"
        model.dashboardNode = null
        model.dashboardSequencerBlocks = []
        model.blockchainModuleReport = null
        model.storageModuleReport = null
        model.messagingModuleReport = null
        model.storageActiveOperation = null
        model.storageActiveOperationRevision = 0
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
        basecampModel.blockchainSourceMode = "auto"
        basecampModel.indexerSourceMode = "auto"
        basecampModel.executionSourceMode = "rpc"
        basecampModel.messagingSourceMode = "auto"
        basecampModel.storageSourceMode = "auto"
        model.registeredIdls.clear()
        model.idlStateLoaded = false
        model.walletStateLoaded = false
        model.accountIdlSelections = ({})
        model.accountIdlSelectionRevision = 0
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

    function test_local_node_action_dispatches_confirmation_token() {
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
            }
        })

        model.runLocalNodeAction("start", "bedrock", "", "", "Start Bedrock")

        tryCompare(fakeHost, "callCount", 1)
        compare(fakeHost.lastMethod, "localNodesAction")
        compare(fakeHost.lastArgs[0], "local")
        compare(fakeHost.lastArgs[1].action, "start")
        compare(fakeHost.lastArgs[1].node, "bedrock")
        compare(fakeHost.lastArgs[2], "confirm-local-node-action")
        compare(model.localNodesOperations.length, 1)
    }

    function test_local_node_network_actions_follow_profile_mode() {
        model.networkProfile = "default"
        model.localNodesReport = ({ active_devnet: "devnet" })
        model.localNodesRevision += 1

        verify(!model.localNodeNetworkActionEnabled("new_network"))
        verify(!model.localNodeNetworkActionEnabled("delete_network"))

        model.networkProfile = "local"
        model.localNodesReport = ({ active_devnet: "devnet" })
        model.localNodesRevision += 1

        verify(model.localNodeNetworkActionEnabled("new_network"))
        verify(model.localNodeNetworkActionEnabled("reset_network"))
        verify(model.localNodeNetworkActionEnabled("delete_network"))
    }

    function test_core_source_args_keep_rpc_shape_in_basecamp_auto() {
        compare(basecampModel.effectiveCoreSourceMode(basecampModel.indexerSourceMode), "rpc")

        const args = basecampModel.indexerArgs(["hash-1"])

        compare(args.length, 2)
        compare(args[0], basecampModel.indexerUrl)
        compare(args[1], "hash-1")
    }

    function test_rpc_only_helpers_keep_rpc_shape_in_basecamp_auto() {
        compare(basecampModel.effectiveCoreSourceMode(basecampModel.blockchainSourceMode), "rpc")

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

    function test_account_lookup_args_stay_rpc_for_account_decode_contract() {
        basecampModel.indexerSourceMode = "module"

        const args = basecampModel.accountLookupArgs("account-1")

        compare(args.length, 3)
        compare(args[0], basecampModel.sequencerUrl)
        compare(args[1], basecampModel.indexerUrl)
        compare(args[2], "account-1")
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

    function test_messaging_and_storage_auto_use_standalone_routes_in_basecamp() {
        compare(basecampModel.effectiveMessagingSourceMode(basecampModel.messagingSourceMode), "rest")
        compare(basecampModel.deliverySourceReportArgs()[0], "rest")
        compare(basecampModel.deliverySourceReportArgs()[1], basecampModel.configuredMessagingRestUrl())
        compare(basecampModel.deliverySourceReportArgs()[2], basecampModel.messagingMetricsUrl)
        compare(basecampModel.deliverySourceTarget(), basecampModel.configuredMessagingRestUrl())

        compare(basecampModel.effectiveStorageSourceMode(basecampModel.storageSourceMode), "rest")
        compare(basecampModel.storageSourceReportArgs(false)[0], "rest")
        compare(basecampModel.storageSourceReportArgs(false)[1], basecampModel.configuredStorageRestUrl())
        compare(basecampModel.storageSourceReportArgs(false)[2], basecampModel.storageMetricsUrl)
        compare(basecampModel.storageSourceTarget(), basecampModel.configuredStorageRestUrl())
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
        compare(model.normalizedMessagingSourceMode("network-monitor"), "network-monitor")
        model.messagingSourceMode = "network-monitor"

        compare(model.effectiveMessagingSourceMode(model.messagingSourceMode), "network-monitor")
        compare(model.deliverySourceReportArgs()[0], "network-monitor")
        compare(model.deliverySourceReportArgs()[1], model.configuredMessagingRestUrl())
        compare(model.deliverySourceReportArgs()[2], model.messagingMetricsUrl)
        compare(model.deliverySourceTarget(), model.configuredMessagingRestUrl())
    }

    function test_delivery_rest_health_rejects_missing_connection_status() {
        const report = {
            module: "delivery_rest",
            probes: [
                { label: "delivery_rest.health", ok: true, value: { status: "ok" } },
                { label: "delivery_rest.nodeHealth", ok: true, value: "healthy" }
            ]
        }

        verify(!model.deliveryReportHealthy(report))
    }

    function test_delivery_rest_health_rejects_unhealthy_node_without_connection_status() {
        const report = {
            module: "delivery_rest",
            probes: [
                { label: "delivery_rest.health", ok: true, value: { status: "ok" } },
                { label: "delivery_rest.nodeHealth", ok: true, value: "unhealthy" }
            ]
        }

        verify(!model.deliveryReportHealthy(report))
    }

    function test_delivery_metrics_health_requires_known_metric_family() {
        verify(model.deliveryReportHealthy({
            module: "delivery_metrics",
            probes: [
                { label: "delivery_metrics.collectOpenMetricsText", ok: true, value: "libp2p_peers 3\n" }
            ]
        }))
        verify(!model.deliveryReportHealthy({
            module: "delivery_metrics",
            probes: [
                { label: "delivery_metrics.collectOpenMetricsText", ok: true, value: "process_cpu_seconds_total 3\n" }
            ]
        }))
    }

    function test_delivery_network_monitor_health_accepts_peer_snapshot() {
        verify(model.deliveryReportHealthy({
            module: "delivery_network_monitor",
            probes: [
                { label: "delivery_network_monitor.allPeersInfo", ok: true, value: [{ peerId: "peer-a" }] }
            ]
        }))
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

        const detail = model.transferRecipientDetailById("overflow")

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
        const blockEntry = model.favoriteBlockEntry({
            type: "blockchain_block",
            hash: "block-hash",
            slot: 12,
            height: 12
        })
        const txEntry = model.favoriteTransactionEntry({
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

        verify(model.addFavorite(blockEntry))
        verify(model.addFavorite(txEntry))
        compare(model.favoriteCount("all"), 2)
        compare(model.favoriteCount("block"), 1)
        compare(model.favoriteRows("block")[0].value, "block-hash")
        verify(model.isFavoriteEntry(blockEntry))

        verify(model.toggleFavorite(blockEntry))
        verify(!model.isFavoriteEntry(blockEntry))
        compare(model.favoriteCount("all"), 1)
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

        compare(model.favorites.length, 1)
        compare(model.favorites[0].value, "account-1")
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

        verify(model.addFavorite(txEntry))

        compare(fakeHost.lastMethod, "saveSettingsState")
        compare(fakeHost.lastArgs[0].favorites.length, 2)
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

    function test_deploy_program_binary_uses_wallet_confirmation_and_logs_operation() {
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
        compare(model.localWalletOperations.length, 1)
        compare(model.localWalletOperations[0].label, "Deploy program")
        compare(model.localWalletOperations[0].status, "submitted")
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

        model.refreshBlocksPage()

        compare(fakeHost.lastMethod, "blockchainBlocks")
        compare(fakeHost.lastArgs[1], 0)
        compare(fakeHost.lastArgs[2], 30)
        compare(fakeHost.lastArgs[3], 20)
        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.slot, 30)
        compare(model.blockStatus(model.blocksPageRows[0]), "pending")
        compare(model.blockStatus(model.blocksPageRows[1]), "finalized")
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

        compare(model.mergeLiveBlocks(fakeHost.responses.blockchainLiveBlocks.value.blocks, model.blocksPageRows, 20).length, 2)
        model.startBlocksLiveMode()

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

    function test_stop_blocks_live_mode_keeps_paged_rows() {
        model.blocksLiveEnabled = true
        model.blocksLiveSource = "blocks_range+stream"
        model.blocksLiveUnknownEvents = 1
        model.blocksLiveCheckedAt = "10:00:00"
        model.blocksPageRows = [
            { header: { slot: 30, id: "slot-30" }, transactions: [] }
        ]

        model.stopBlocksLiveMode()

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

        model.refreshLezBlocksPage()

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
        model.finishLezBlocksPage(0, {
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

        model.refreshLezTransactionsPage()
        const callsAfterFirstPage = fakeHost.callCount

        compare(model.lezTransactionsPageRows.length, 2)
        compare(model.lezTransactionsPageRows[0].hash, "tx-1")
        compare(model.lezTransactionsPageOverflowRows.length, 1)

        model.olderLezTransactionsPage()

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

        model.refreshTransferActivityPage()
        const callsAfterFirstPage = fakeHost.callCount

        compare(model.transferActivityRows.length, 2)
        compare(model.transferActivityRows[0].recipient, "r1")
        compare(model.transferActivityOverflowRows.length, 1)

        model.nextTransferActivityPage()

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
        compare(model.sourceEmptyText("indexer", "", "No indexed blocks"), "No indexed blocks")

        model.updateNetworkConnectionStatus("indexer", {
            ok: true,
            value: { state: "syncing", indexedBlockId: 12 },
            text: "syncing",
            error: ""
        })

        compare(model.sourceEmptyText("indexer", "", "No indexed blocks"), "Source reachable; syncing")
        compare(model.sourceProblemTitle("indexer", "Response shape unknown. Raw JSON remains available.", "L2 blocks unavailable"), "Response shape unknown")
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
