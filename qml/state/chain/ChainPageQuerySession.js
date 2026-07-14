.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "ChainPageQuery.js" as ChainPageQuery

function refreshTransactionsPage(root, beforeBlock) {
    with (root) {
        if (root.operationPending("transactions.page.node")
                || root.operationPending("transactions.page.range")) {
            return null
        }
        const presentation = root.beginPresentation(qsTr("Transactions"), "transactions")
        return root.startOperation("transactions.page.node", "blockchainNode", [],
            qsTr("Transactions node state"), function (node) {
                if (!node || !node.ok) {
                    transactionsPageError = String(node && node.error
                        || qsTr("Transactions node query failed."))
                    root.completePresentation(presentation, qsTr("Transactions"),
                        transactionsPageError, true, null)
                    return false
                }
                const window = ChainPageQuery.slotWindow(beforeBlock,
                    ChainPageQuery.slotTip(node.value, true), transactionsPageBlockBatch)
                const slotFrom = window.slotFrom
                const slotTo = window.slotTo
                root.startOperation("transactions.page.range", "blockchainBlocks",
                    [slotFrom, slotTo], qsTr("Transactions"), function (blocks) {
                        if (!blocks || !blocks.ok) {
                            transactionsPageError = String(blocks && blocks.error
                                || qsTr("Transactions query failed."))
                            root.completePresentation(presentation, qsTr("Transactions"),
                                transactionsPageError, true, null)
                            return false
                        }
                        transactionsPageBeforeBlock = slotTo
                        transactionsPageRows = root.transactionRowsFromBlocks(blocks.value)
                            .slice(0, transactionsPageLimit)
                        transactionsPageNextBeforeBlock = slotFrom > 0 ? slotFrom - 1 : 0
                        transactionsPageError = ""
                        root.completePresentation(presentation, qsTr("Transactions"),
                            BridgeHelpers.formatValue(transactionsPageRows),
                            false, transactionsPageRows)
                        return false
                    })
                return false
            })
    }
}

function olderTransactionsPage(root) {
    refreshTransactionsPage(root, root.transactionsPageNextBeforeBlock)
}

function newerTransactionsPage(root) {
    refreshTransactionsPage(root, root.transactionsPageBeforeBlock + root.transactionsPageBlockBatch + 1)
}

function setTransactionsPageLimit(root, limit) {
    if (root.transactionsWorkflowRunning) {
        return false
    }
    const value = Math.max(1, Number(limit || root.transactionsPageLimit))
    if (root.transactionsPageLimit === value) {
        return false
    }
    root.transactionsPageLimit = value
    refreshTransactionsPage(root, root.transactionsPageBeforeBlock > 0 ? root.transactionsPageBeforeBlock : null)
    return true
}
