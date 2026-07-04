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
        model.dashboardMetricHistory = ({})
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

        model.clearDashboardMetricHistoryForPrefix("messaging.")

        compare(model.dashboardMetricHistory["messaging.messages"], undefined)
        verify(model.dashboardMetricHistory["storage.files"] !== undefined)
        verify(model.dashboardMetricHistory["chain.height"] !== undefined)
        compare(model.dashboardMetricHistoryRevision, 1)
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

    function test_blocks_page_uses_tip_range_and_all_blocks_backend() {
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
            blockchainAllBlocks: {
                ok: true,
                value: [
                    { header: { slot: 30, id: "tip" }, transactions: [] },
                    { header: { slot: 20, id: "lib" }, transactions: [] }
                ],
                text: "OK",
                error: ""
            }
        }

        model.refreshBlocksPage()

        compare(fakeHost.lastMethod, "blockchainAllBlocks")
        compare(fakeHost.lastArgs[1], 0)
        compare(fakeHost.lastArgs[2], 30)
        compare(model.blocksPageRows.length, 2)
        compare(model.blocksPageRows[0].header.slot, 30)
        compare(model.blockStatus(model.blocksPageRows[0]), "pending")
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
}
