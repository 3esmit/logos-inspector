import QtQml
import "chain/AppModelPages.js" as AppModelPages

QtObject {
    id: root

    required property var gateway
    property string inspectorModule: "logos_inspector"

    property var dashboardOverview: null
    property var dashboardNode: null
    property var dashboardL1Blocks: []
    property var dashboardBlocks: []
    property var dashboardSequencerBlocks: []
    property string dashboardError: ""
    property var blockDetailValue: null
    property string blockDetailError: ""
    property var transactionDetailValue: null
    property string transactionDetailError: ""
    property var accountDetailValue: null
    property var transferRecipientDetailValue: null
    property var channelDetailValue: null
    property string channelDetailError: ""

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

    property var lezBlocksPageRows: []
    property int lezBlocksPageBeforeBlock: 0
    property int lezBlocksPageNextBeforeBlock: 0
    property int lezBlocksPageLimit: 20
    property string lezBlocksPageError: ""
    property bool lezBlocksPageLoading: false
    property int lezBlocksPageRequestSerial: 0

    property var lezTransactionsPageRows: []
    property int lezTransactionsPageBeforeBlock: 0
    property int lezTransactionsPageNextBeforeBlock: 0
    property var lezTransactionsPageOverflowRows: []
    property int lezTransactionsPageOverflowNextBeforeBlock: 0
    property int lezTransactionsBlockBatch: 50
    property int lezTransactionsPageLimit: 20
    property string lezTransactionsPageError: ""

    property var transferActivityRows: []
    property int transferActivityBeforeBlock: 0
    property int transferActivityNextBeforeBlock: 0
    property var transferActivityOverflowRows: []
    property int transferActivityOverflowNextBeforeBlock: 0
    property int transferActivityBlockBatch: 50
    property int transferActivityLimit: 20
    property var transferActivityHistory: []
    property string transferActivityError: ""

    property var channelsPageRows: []
    property int channelsPageSlotFrom: 0
    property int channelsPageSlotTo: 0
    property int channelsPageWindow: 4000
    property int channelsPageLimit: 20
    property string channelsPageError: ""

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

    function indexerArgs(extra) { return gateway.indexerArgs(extra) }

    function executionArgs(extra) { return gateway.executionArgs(extra) }

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

    function refreshLezBlocksPage(beforeBlock) { return AppModelPages.refreshLezBlocksPage(root, beforeBlock) }

    function finishLezBlocksPage(beforeBlock, sequencerResponse, indexerResponse) { return AppModelPages.finishLezBlocksPage(root, beforeBlock, sequencerResponse, indexerResponse) }

    function olderLezBlocksPage() { return AppModelPages.olderLezBlocksPage(root) }

    function newerLezBlocksPage() { return AppModelPages.newerLezBlocksPage(root) }

    function setLezBlocksPageLimit(limit) { return AppModelPages.setLezBlocksPageLimit(root, limit) }

    function refreshLezTransactionsPage(beforeBlock) { return AppModelPages.refreshLezTransactionsPage(root, beforeBlock) }

    function olderLezTransactionsPage() { return AppModelPages.olderLezTransactionsPage(root) }

    function newerLezTransactionsPage() { return AppModelPages.newerLezTransactionsPage(root) }

    function setLezTransactionsPageLimit(limit) { return AppModelPages.setLezTransactionsPageLimit(root, limit) }

    function sortedIndexerBlocks(blocks) { return AppModelPages.sortedIndexerBlocks(root, blocks) }

    function mergedLezBlocks(sequencerBlocks, indexerBlocks, limit) { return AppModelPages.mergedLezBlocks(root, sequencerBlocks, indexerBlocks, limit) }

    function indexerBlockId(block) { return AppModelPages.indexerBlockId(root, block) }

    function indexerBlockHash(block) { return AppModelPages.indexerBlockHash(root, block) }

    function nextIndexerBlocksCursor(blocks) { return AppModelPages.nextIndexerBlocksCursor(root, blocks) }

    function normalizedPositiveInteger(value) { return AppModelPages.normalizedPositiveInteger(root, value) }

    function lezTransactionRowsFromBlocks(blocks) { return AppModelPages.lezTransactionRowsFromBlocks(root, blocks) }

    function lezTransactionHash(tx) { return AppModelPages.lezTransactionHash(root, tx) }

    function transactionProgramIdHex(tx) { return AppModelPages.transactionProgramIdHex(root, tx) }

    function lezTransactionOpCount(tx) { return AppModelPages.lezTransactionOpCount(root, tx) }

    function transactionRowsFromBlocks(blocks) { return AppModelPages.transactionRowsFromBlocks(root, blocks) }

    function sortedBlockchainBlocks(blocks) { return AppModelPages.sortedBlockchainBlocks(root, blocks) }

    function transactionHash(tx) { return AppModelPages.transactionHash(root, tx) }

    function transactionOps(tx) { return AppModelPages.transactionOps(root, tx) }

    function operationSummary(op, tx, index) { return AppModelPages.operationSummary(root, op, tx, index) }

    function byteHex(value) { return AppModelPages.byteHex(root, value) }

    function operationName(opcode) { return AppModelPages.operationName(root, opcode) }

    function refreshTransferActivityPage(beforeBlock, preserveHistory) { return AppModelPages.refreshTransferActivityPage(root, beforeBlock, preserveHistory) }

    function nextTransferActivityPage() { return AppModelPages.nextTransferActivityPage(root) }

    function previousTransferActivityPage() { return AppModelPages.previousTransferActivityPage(root) }

    function setTransferActivityPageLimit(limit) { return AppModelPages.setTransferActivityPageLimit(root, limit) }

    function nextTransferActivityBlock(recipients) { return AppModelPages.nextTransferActivityBlock(root, recipients) }

    function transferRecipientDetail(row) { return AppModelPages.transferRecipientDetail(root, row) }

    function transferRecipientDetailById(value) { return AppModelPages.transferRecipientDetailById(root, value) }

    function refreshChannelsPage(anchorSlot) { return AppModelPages.refreshChannelsPage(root, anchorSlot) }

    function olderChannelsPage() { return AppModelPages.olderChannelsPage(root) }

    function newerChannelsPage() { return AppModelPages.newerChannelsPage(root) }

    function setChannelsPageLimit(limit) { return AppModelPages.setChannelsPageLimit(root, limit) }

    function channelDetail(row) { return AppModelPages.channelDetail(root, row) }

    function channelDetailById(value) { return AppModelPages.channelDetailById(root, value) }
}
