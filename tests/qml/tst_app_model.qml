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
        property var responses: ({})

        function callModuleJson(moduleName, method, argsJson) {
            callCount += 1
            lastMethod = String(method || "")
            lastArgs = JSON.parse(String(argsJson || "[]"))
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
        fakeHost.responses = ({})
        basecampHost.callCount = 0
        basecampHost.lastModule = ""
        basecampHost.lastMethod = ""
        basecampHost.lastArgs = []
        basecampHost.serializeResults = false
        model.currentView = "overview"
        model.dashboardNode = null
        model.blockchainModuleReport = null
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
        model.blockDetailValue = null
        model.blockchainSourceMode = "auto"
        model.indexerSourceMode = "auto"
        model.executionSourceMode = "auto"
        basecampModel.blockchainSourceMode = "auto"
        basecampModel.indexerSourceMode = "auto"
        basecampModel.executionSourceMode = "auto"
        model.registeredIdls.clear()
        model.idlStateLoaded = false
        model.accountIdlSelections = ({})
        model.accountIdlSelectionRevision = 0
        model.walletPublicKeyProbe = ""
        model.bedrockWalletModuleError = ""
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

    function test_core_source_args_use_module_shape_in_basecamp_auto() {
        compare(basecampModel.effectiveCoreSourceMode(basecampModel.indexerSourceMode), "module")

        const args = basecampModel.indexerArgs(["hash-1"])

        compare(args.length, 3)
        compare(args[0], "module")
        compare(args[1], basecampModel.indexerUrl)
        compare(args[2], "hash-1")
    }

    function test_account_lookup_args_can_mix_execution_rpc_and_indexer_module() {
        model.executionSourceMode = "rpc"
        model.indexerSourceMode = "module"

        const args = model.accountLookupArgs("account-1")

        compare(args.length, 5)
        compare(args[0], "rpc")
        compare(args[1], model.sequencerUrl)
        compare(args[2], "module")
        compare(args[3], model.indexerUrl)
        compare(args[4], "account-1")
    }

    function test_navigation_delegates() {
        compare(model.viewTitle(), "Dashboard")
        verify(model.navRows().length > 0)

        model.selectView("programs")

        compare(model.currentView, "programs")
        compare(model.parentNavKeyForView("programs"), "l2")
        compare(model.navTokenForView("programs"), "PRG")
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

        compare(model.blocksPageRows.length, 5)
        compare(model.blocksPageRows[0].header.id, "l1-tip")
        compare(model.lezBlocksPageRows.length, 5)
        compare(model.lezBlocksPageRows[0].block_id, 104)
        tryCompare(model, "dashboardRefreshing", false)
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
