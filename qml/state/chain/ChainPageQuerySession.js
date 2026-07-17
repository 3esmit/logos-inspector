.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "ChainPageQuery.js" as ChainPageQuery

function refreshTransactionsPage(root, beforeBlock, pagePosition) {
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
                const observedTipSlot = ChainPageQuery.slotTip(node.value, true)
                const latestRequest = beforeBlock === undefined
                    || beforeBlock === null
                const candidateSessionTip = latestRequest
                        || transactionsPageSessionTip <= 0
                    ? observedTipSlot : transactionsPageSessionTip
                const tipSlot = candidateSessionTip > 0
                    ? candidateSessionTip : observedTipSlot
                const window = ChainPageQuery.slotWindow(beforeBlock,
                    tipSlot, transactionsPageBlockBatch)
                const slotFrom = window.slotFrom
                const slotTo = window.slotTo
                root.startOperation("transactions.page.range", "blockchainBlocks",
                    [slotFrom, slotTo, transactionsPageBlockScanLimit],
                    qsTr("Transactions"), function (blocks) {
                        if (!blocks || !blocks.ok) {
                            transactionsPageError = String(blocks && blocks.error
                                || qsTr("Transactions query failed."))
                            root.completePresentation(presentation, qsTr("Transactions"),
                                transactionsPageError, true, null)
                            return false
                        }
                        transactionsPageBeforeBlock = slotTo
                        transactionsPageSessionTip = candidateSessionTip
                        transactionsPageWindowAtLatest = tipSlot > 0
                            && slotTo >= tipSlot
                        transactionsPageWindowRows = root.transactionRowsFromBlocks(
                            blocks.value)
                        transactionsPageWindowLoaded = true
                        const offset = pagePosition === "tail"
                            ? tailPageOffset(root, transactionsPageWindowRows.length)
                            : 0
                        projectTransactionsWindow(root, offset)
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
    if (root.transactionsWorkflowRunning) {
        return false
    }
    const nextOffset = root.transactionsPageRowOffset
        + root.transactionsPageLimit
    if (root.transactionsPageWindowLoaded
            && nextOffset < root.transactionsPageWindowRows.length) {
        presentTransactionsWindow(root, nextOffset)
        return true
    }
    if (root.transactionsPageNextBeforeBlock <= 0) {
        return false
    }
    refreshTransactionsPage(root, root.transactionsPageNextBeforeBlock)
    return true
}

function newerTransactionsPage(root) {
    if (root.transactionsWorkflowRunning) {
        return false
    }
    if (root.transactionsPageWindowLoaded
            && root.transactionsPageRowOffset > 0) {
        presentTransactionsWindow(root, Math.max(0,
            root.transactionsPageRowOffset - root.transactionsPageLimit))
        return true
    }
    if (root.transactionsPageBeforeBlock <= 0
            || root.transactionsPageAtLatest) {
        return false
    }
    refreshTransactionsPage(root,
        root.transactionsPageBeforeBlock + root.transactionsPageBlockBatch + 1,
        "tail")
    return true
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
    if (root.transactionsPageWindowLoaded) {
        const alignedOffset = Math.floor(
            root.transactionsPageRowOffset / value) * value
        presentTransactionsWindow(root, alignedOffset)
        return true
    }
    const beforeBlock = root.transactionsPageAtLatest
        ? null
        : root.transactionsPageBeforeBlock > 0
            ? root.transactionsPageBeforeBlock
            : null
    refreshTransactionsPage(root, beforeBlock)
    return true
}

function tailPageOffset(root, rowCount) {
    const count = Math.max(0, Number(rowCount || 0))
    if (count === 0) {
        return 0
    }
    const limit = Math.max(1, Number(root.transactionsPageLimit || 1))
    return Math.floor((count - 1) / limit) * limit
}

function projectTransactionsWindow(root, offset) {
    const rows = Array.isArray(root.transactionsPageWindowRows)
        ? root.transactionsPageWindowRows : []
    const limit = Math.max(1, Number(root.transactionsPageLimit || 1))
    const maximum = rows.length > 0 ? rows.length - 1 : 0
    const start = Math.max(0, Math.min(maximum, Number(offset || 0)))
    root.transactionsPageRowOffset = start
    root.transactionsPageRows = rows.slice(start, start + limit)
    root.transactionsPageAtLatest = root.transactionsPageWindowAtLatest
        && start === 0
}

function presentTransactionsWindow(root, offset) {
    projectTransactionsWindow(root, offset)
    root.setResult(qsTr("Transactions"),
        BridgeHelpers.formatValue(root.transactionsPageRows), false,
        root.transactionsPageRows, "transactions")
}
