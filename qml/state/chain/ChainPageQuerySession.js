.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "ChainPageQuery.js" as ChainPageQuery

function refreshTransactionsPage(root, beforeBlock) {
    with (root) {
        const node = requestModule(inspectorModule, "blockchainNode", root.blockchainArgs([]), qsTr("Transactions node state"), false)
        if (!node.ok) {
            transactionsPageError = node.error
            shell.setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        const window = ChainPageQuery.slotWindow(beforeBlock, ChainPageQuery.slotTip(node.value, true), transactionsPageBlockBatch)
        const slotFrom = window.slotFrom
        const slotTo = window.slotTo
        const blocks = requestModule(inspectorModule, "blockchainBlocks", root.blockchainArgs([slotFrom, slotTo]), qsTr("Transactions"), false)
        if (!blocks.ok) {
            transactionsPageError = blocks.error
            shell.setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        transactionsPageBeforeBlock = slotTo
        if (!Array.isArray(blocks.value)) {
            transactionsPageRows = []
            transactionsPageNextBeforeBlock = 0
            transactionsPageError = qsTr("Response shape unknown. Raw JSON remains available.")
            shell.setResult(qsTr("Transactions"), BridgeHelpers.formatValue(blocks.value), false, blocks.value)
            return
        }
        transactionsPageRows = root.transactionRowsFromBlocks(blocks.value).slice(0, transactionsPageLimit)
        transactionsPageNextBeforeBlock = slotFrom > 0 ? slotFrom - 1 : 0
        transactionsPageError = ""
        shell.setResult(qsTr("Transactions"), BridgeHelpers.formatValue(transactionsPageRows), false, transactionsPageRows)
    }
}

function olderTransactionsPage(root) {
    refreshTransactionsPage(root, root.transactionsPageNextBeforeBlock)
}

function newerTransactionsPage(root) {
    refreshTransactionsPage(root, root.transactionsPageBeforeBlock + root.transactionsPageBlockBatch + 1)
}

function setTransactionsPageLimit(root, limit) {
    const value = Math.max(1, Number(limit || root.transactionsPageLimit))
    if (root.transactionsPageLimit === value) {
        return
    }
    root.transactionsPageLimit = value
    refreshTransactionsPage(root, root.transactionsPageBeforeBlock > 0 ? root.transactionsPageBeforeBlock : null)
}
