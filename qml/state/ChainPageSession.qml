import QtQml
import "chain/AppModelPages.js" as AppModelPages

QtObject {
    id: root

    required property var gateway
    property var capabilityFacade: null
    property string inspectorModule: "logos_inspector"

    property var dashboardOverview: null
    property var dashboardNode: null
    property var dashboardL1Blocks: []
    property var dashboardBlocks: []
    property var dashboardProvisionalBlocks: []
    property var dashboardLezBlockRows: []
    property string dashboardError: ""
    property var blockDetailValue: null
    property string blockDetailError: ""
    property var transactionDetailValue: null
    property string transactionDetailError: ""

    property var blocksPageRows: []
    property int blocksPageSlotFrom: 0
    property int blocksPageSlotTo: 0
    property int blocksPageWindow: 2000
    property int blocksPageLimit: 20
    property string blocksPageError: ""
    property bool blocksLiveEnabled: false
    property string blocksLiveError: ""
    property string blocksLiveSource: ""
    property int blocksLiveUnknownEvents: 0
    property string blocksLiveCheckedAt: ""

    property var transactionsPageRows: []
    property int transactionsPageBeforeBlock: 0
    property int transactionsPageNextBeforeBlock: 0
    property int transactionsPageBlockBatch: 1000
    property int transactionsPageLimit: 20
    property string transactionsPageError: ""

    function requestModule(moduleName, method, args, label, showResult, cacheResult) {
        return gateway.requestModule(moduleName, method, args, label, showResult, cacheResult)
    }

    function requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse) {
        return gateway.requestModuleAsync(moduleName, method, args, label, showResult, callback, acceptResponse)
    }

    function setResult(title, text, isError, value, owner) {
        return gateway.setResult(title, text, isError, value, owner)
    }

    function blockchainArgs(extra) { return gateway.blockchainArgs(extra) }

    function l1Gate() { return capabilityGate("l1") }

    function capabilityGate(expression) {
        if (capabilityFacade && typeof capabilityFacade.gateFor === "function") {
            return capabilityFacade.gateFor(expression)
        }
        return {
            enabled: true,
            status: "enabled",
            missing: [],
            warnings: [],
            provenance: ["network_inspection_compatibility"]
        }
    }

    function blockchainRpcArgs(extra) { return gateway.blockchainRpcArgs(extra) }

    function networkConnectionState(kind) { return gateway.networkConnectionState(kind) }

    function cryptarchiaInfo() {
        const report = dashboardNode
        const probe = report ? report.cryptarchia_info : null
        return probe && probe.value ? probe.value.cryptarchia_info : null
    }

    function valueToString(value) { return gateway.valueToString(value) }

    function canonicalProgramIdHex(value) { return gateway.canonicalProgramIdHex(value) }

    function normalizedHexText(value) { return gateway.normalizedHexText(value) }

    function refreshBlocksPage(anchorSlot) { return AppModelPages.refreshBlocksPage(root, anchorSlot) }

    function startBlocksLiveMode() { return AppModelPages.startBlocksLiveMode(root) }

    function stopBlocksLiveMode() { return AppModelPages.stopBlocksLiveMode(root) }

    function refreshBlocksLivePage() { return AppModelPages.refreshBlocksLivePage(root) }

    function mergeLiveBlocks(liveBlocks, existingBlocks, limit) { return AppModelPages.mergeLiveBlocks(root, liveBlocks, existingBlocks, limit) }

    function blocksLiveStatusText() { return AppModelPages.blocksLiveStatusText(root) }

    function olderBlocksPage() { return AppModelPages.olderBlocksPage(root) }

    function newerBlocksPage() { return AppModelPages.newerBlocksPage(root) }

    function setBlocksPageLimit(limit) { return AppModelPages.setBlocksPageLimit(root, limit) }

    function sortedBlocks(blocks) { return AppModelPages.sortedBlocks(root, blocks) }

    function blockSlot(block) { return AppModelPages.blockSlot(root, block) }

    function blockHash(block) { return AppModelPages.blockHash(root, block) }

    function blockParent(block) { return AppModelPages.blockParent(root, block) }

    function blockProof(block) { return AppModelPages.blockProof(root, block) }

    function blockRoot(block) { return AppModelPages.blockRoot(root, block) }

    function blockHeight(block) { return AppModelPages.blockHeight(root, block) }

    function blockVersion(block) { return AppModelPages.blockVersion(root, block) }

    function blockSignature(block) { return AppModelPages.blockSignature(root, block) }

    function blockStatus(block) { return AppModelPages.blockStatus(root, block) }

    function blockchainInfo() { return AppModelPages.blockchainInfo(root) }

    function sourceEmptyText(source, error, fallback) { return AppModelPages.sourceEmptyText(root, source, error, fallback) }

    function sourceProblemTitle(source, error, fallback) { return AppModelPages.sourceProblemTitle(root, source, error, fallback) }

    function blockTransactions(block) { return AppModelPages.blockTransactions(root, block) }

    function blockchainBlockDetail(block) { return AppModelPages.blockchainBlockDetail(root, block) }

    function blockchainBlockDetailById(value) { return AppModelPages.blockchainBlockDetailById(root, value) }

    function normalizedHashOrValue(value) { return AppModelPages.normalizedHashOrValue(root, value) }

    function refreshTransactionsPage(beforeBlock) { return AppModelPages.refreshTransactionsPage(root, beforeBlock) }

    function olderTransactionsPage() { return AppModelPages.olderTransactionsPage(root) }

    function newerTransactionsPage() { return AppModelPages.newerTransactionsPage(root) }

    function setTransactionsPageLimit(limit) { return AppModelPages.setTransactionsPageLimit(root, limit) }

    function transactionRowsFromBlocks(blocks) { return AppModelPages.transactionRowsFromBlocks(root, blocks) }

    function sortedBlockchainBlocks(blocks) { return AppModelPages.sortedBlockchainBlocks(root, blocks) }

    function transactionHash(tx) { return AppModelPages.transactionHash(root, tx) }

    function transactionOps(tx) { return AppModelPages.transactionOps(root, tx) }

    function operationSummary(op, tx, index) { return AppModelPages.operationSummary(root, op, tx, index) }

    function byteHex(value) { return AppModelPages.byteHex(root, value) }

    function operationName(opcode) { return AppModelPages.operationName(root, opcode) }

}
