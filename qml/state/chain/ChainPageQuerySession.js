.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "ChainPageQuery.js" as ChainPageQuery

function refreshTransactionsPage(root, beforeBlock) {
    with (root) {
        const node = requestModule(inspectorModule, "blockchainNode", root.blockchainArgs([]), qsTr("Transactions node state"), false)
        if (!node.ok) {
            transactionsPageError = node.error
            setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        const window = ChainPageQuery.slotWindow(beforeBlock, ChainPageQuery.slotTip(node.value, true), transactionsPageBlockBatch)
        const slotFrom = window.slotFrom
        const slotTo = window.slotTo
        const blocks = requestModule(inspectorModule, "blockchainBlocks", root.blockchainArgs([slotFrom, slotTo]), qsTr("Transactions"), false)
        if (!blocks.ok) {
            transactionsPageError = blocks.error
            setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        transactionsPageBeforeBlock = slotTo
        if (!Array.isArray(blocks.value)) {
            transactionsPageRows = []
            transactionsPageNextBeforeBlock = 0
            transactionsPageError = qsTr("Response shape unknown. Raw JSON remains available.")
            setResult(qsTr("Transactions"), BridgeHelpers.formatValue(blocks.value), false, blocks.value)
            return
        }
        transactionsPageRows = root.transactionRowsFromBlocks(blocks.value).slice(0, transactionsPageLimit)
        transactionsPageNextBeforeBlock = slotFrom > 0 ? slotFrom - 1 : 0
        transactionsPageError = ""
        setResult(qsTr("Transactions"), BridgeHelpers.formatValue(transactionsPageRows), false, transactionsPageRows)
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

function refreshLezBlocksPage(root, beforeBlock) {
    const completeLezBlocksPage = finishLezBlocksPage
    with (root) {
        const before = root.normalizedPositiveInteger(beforeBlock)
        const beforeArg = before > 0 ? before : null
        const limit = Math.max(1, Number(lezBlocksPageLimit || 1))
        const serial = lezBlocksPageRequestSerial + 1
        lezBlocksPageRequestSerial = serial
        lezBlocksPageLoading = true

        let sequencerDone = false
        let indexerDone = false
        let sequencerResponse = null
        let indexerResponse = null

        function completeIfReady() {
            if (serial !== lezBlocksPageRequestSerial) {
                return
            }
            const hasUsableRows = responseBlockArray(sequencerResponse) !== null
                || responseBlockArray(indexerResponse) !== null
            if (!hasUsableRows && (!sequencerDone || !indexerDone)) {
                return
            }
            lezBlocksPageLoading = false
            completeLezBlocksPage(root, before, sequencerResponse, indexerResponse)
        }

        requestModuleAsync(inspectorModule, "sequencerBlocks", root.executionArgs([beforeArg, limit]), qsTr("L2 blocks"), false, function (response) {
            sequencerDone = true
            sequencerResponse = response
            completeIfReady()
        })
        requestModuleAsync(inspectorModule, "indexerBlocks", root.indexerArgs([beforeArg, limit]), qsTr("L2 indexed blocks"), false, function (response) {
            indexerDone = true
            indexerResponse = response
            completeIfReady()
        })
    }
}

function finishLezBlocksPage(root, before, sequencerResponse, indexerResponse) {
    with (root) {
        const sequencerBlocks = responseBlockArray(sequencerResponse)
        const indexerBlocks = responseBlockArray(indexerResponse)
        if (sequencerBlocks !== null || indexerBlocks !== null) {
            const report = root.lezBlockListReport(sequencerBlocks || [], indexerBlocks || [], lezBlocksPageLimit)
            if (!report.ok) {
                lezBlocksPageError = report.error
                setResult(qsTr("L2 blocks"), lezBlocksPageError, true)
                return
            }
            const blocks = root.lezBlockListRows(report.value)
            lezBlocksPageBeforeBlock = before
            lezBlocksPageRows = blocks
            lezBlocksPageNextBeforeBlock = root.nextIndexerBlocksCursor(blocks)
            lezBlocksPageError = ""
            setResult(qsTr("L2 blocks"), BridgeHelpers.formatValue(lezBlocksPageRows), false, lezBlocksPageRows)
            return
        }

        const unknownShapeResponse = (sequencerResponse && sequencerResponse.ok)
            ? sequencerResponse
            : ((indexerResponse && indexerResponse.ok) ? indexerResponse : null)
        if (unknownShapeResponse !== null) {
            lezBlocksPageBeforeBlock = before
            lezBlocksPageRows = []
            lezBlocksPageNextBeforeBlock = 0
            lezBlocksPageError = qsTr("Response shape unknown. Raw JSON remains available.")
            setResult(qsTr("L2 blocks"), BridgeHelpers.formatValue(unknownShapeResponse.value), false, unknownShapeResponse.value)
            return
        }

        lezBlocksPageError = (sequencerResponse && sequencerResponse.error) || (indexerResponse && indexerResponse.error) || qsTr("L2 blocks unavailable")
        setResult(qsTr("L2 blocks"), lezBlocksPageError, true)
    }
}

function responseBlockArray(response) {
    return response && response.ok === true && Array.isArray(response.value) ? response.value : null
}

function olderLezBlocksPage(root) {
    if (!root.lezBlocksPageLoading && root.lezBlocksPageNextBeforeBlock > 0) {
        refreshLezBlocksPage(root, root.lezBlocksPageNextBeforeBlock)
    }
}

function newerLezBlocksPage(root) {
    if (!root.lezBlocksPageLoading) {
        refreshLezBlocksPage(root, null)
    }
}

function setLezBlocksPageLimit(root, limit) {
    const value = Math.max(1, Number(limit || root.lezBlocksPageLimit))
    if (root.lezBlocksPageLimit === value) {
        return
    }
    root.lezBlocksPageLimit = value
    refreshLezBlocksPage(root, root.lezBlocksPageBeforeBlock > 0 ? root.lezBlocksPageBeforeBlock : null)
}

function refreshLezTransactionsPage(root, beforeBlock) {
    with (root) {
        const before = root.normalizedPositiveInteger(beforeBlock)
        const blockLimit = Math.max(lezTransactionsBlockBatch, lezTransactionsPageLimit)
        const response = requestModule(inspectorModule, "indexerBlocks", root.indexerArgs([before > 0 ? before : null, blockLimit]), qsTr("L2 transactions"), false, false)
        if (!response.ok) {
            lezTransactionsPageError = response.error
            setResult(qsTr("L2 transactions"), lezTransactionsPageError, true)
            return
        }

        if (!Array.isArray(response.value)) {
            lezTransactionsPageBeforeBlock = before
            lezTransactionsPageRows = []
            lezTransactionsPageNextBeforeBlock = 0
            lezTransactionsPageOverflowRows = []
            lezTransactionsPageOverflowNextBeforeBlock = 0
            lezTransactionsPageError = qsTr("Response shape unknown. Raw JSON remains available.")
            setResult(qsTr("L2 transactions"), BridgeHelpers.formatValue(response.value), false, response.value)
            return
        }

        const blocks = root.sortedIndexerBlocks(response.value)
        const rows = root.lezTransactionRowsFromBlocks(blocks)
        const cursor = root.nextIndexerBlocksCursor(blocks)
        lezTransactionsPageBeforeBlock = before
        lezTransactionsPageRows = rows.slice(0, lezTransactionsPageLimit)
        lezTransactionsPageOverflowRows = rows.slice(lezTransactionsPageLimit)
        lezTransactionsPageOverflowNextBeforeBlock = cursor
        lezTransactionsPageNextBeforeBlock = cursor
        lezTransactionsPageError = ""
        setResult(qsTr("L2 transactions"), BridgeHelpers.formatValue(lezTransactionsPageRows), false, lezTransactionsPageRows)
    }
}

function olderLezTransactionsPage(root) {
    if (Array.isArray(root.lezTransactionsPageOverflowRows) && root.lezTransactionsPageOverflowRows.length > 0) {
        root.lezTransactionsPageRows = root.lezTransactionsPageOverflowRows.slice(0, root.lezTransactionsPageLimit)
        root.lezTransactionsPageOverflowRows = root.lezTransactionsPageOverflowRows.slice(root.lezTransactionsPageLimit)
        root.lezTransactionsPageNextBeforeBlock = root.lezTransactionsPageOverflowRows.length > 0 ? root.lezTransactionsPageNextBeforeBlock : root.lezTransactionsPageOverflowNextBeforeBlock
        root.setResult(qsTr("L2 transactions"), BridgeHelpers.formatValue(root.lezTransactionsPageRows), false, root.lezTransactionsPageRows)
        return
    }
    if (root.lezTransactionsPageNextBeforeBlock > 0) {
        refreshLezTransactionsPage(root, root.lezTransactionsPageNextBeforeBlock)
    }
}

function newerLezTransactionsPage(root) {
    refreshLezTransactionsPage(root, null)
}

function setLezTransactionsPageLimit(root, limit) {
    const value = Math.max(1, Number(limit || root.lezTransactionsPageLimit))
    if (root.lezTransactionsPageLimit === value) {
        return
    }
    root.lezTransactionsPageLimit = value
    refreshLezTransactionsPage(root, root.lezTransactionsPageBeforeBlock > 0 ? root.lezTransactionsPageBeforeBlock : null)
}
