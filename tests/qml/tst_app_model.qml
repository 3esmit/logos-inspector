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

    BridgeClient {
        id: bridgeClient

        host: fakeHost
    }

    AppModel {
        id: model

        bridge: bridgeClient
    }

    function init() {
        fakeHost.callCount = 0
        fakeHost.lastMethod = ""
        fakeHost.lastArgs = []
        fakeHost.responses = ({})
        model.currentView = "overview"
        model.dashboardNode = null
        model.blockchainModuleReport = null
        model.dashboardMetricHistory = ({})
        model.dashboardMetricLastSeen = ({})
        model.dashboardMetricHistoryRevision = 0
        model.blocksPageRows = []
        model.blocksPageSlotFrom = 0
        model.blocksPageSlotTo = 0
        model.blocksPageError = ""
        model.lezBlocksPageRows = []
        model.lezBlocksPageBeforeBlock = 0
        model.lezBlocksPageNextBeforeBlock = 0
        model.lezBlocksPageError = ""
        model.blockDetailValue = null
        model.registeredIdls.clear()
        model.idlStateLoaded = false
        model.accountIdlSelections = ({})
        model.accountIdlSelectionRevision = 0
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
}
