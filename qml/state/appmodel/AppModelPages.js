.import "../../services/BridgeHelpers.js" as BridgeHelpers

function refreshBlocksPage(root, anchorSlot) {
    with (root) {
        const node = requestModule(inspectorModule, "blockchainNode", root.blockchainArgs([]), qsTr("Blocks node state"), false)
        if (!node.ok) {
            blocksPageError = node.error
            setResult(qsTr("Blocks"), blocksPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.slot || info.lib_slot || 0) : 0
        const requestedSlot = Math.max(0, Number(anchorSlot === undefined || anchorSlot === null ? fallbackSlot : anchorSlot))
        const slotTo = fallbackSlot > 0 ? Math.min(requestedSlot, Number(fallbackSlot)) : requestedSlot
        const slotFrom = Math.max(0, slotTo - blocksPageWindow)
        const blockLimit = Math.max(5, Number(blocksPageLimit || 5))
        const blocks = requestModule(inspectorModule, "blockchainBlocks", root.blockchainArgs([slotFrom, slotTo, blockLimit]), qsTr("Blocks"), false)
        if (!blocks.ok) {
            blocksPageError = blocks.error
            setResult(qsTr("Blocks"), blocksPageError, true)
            return
        }

        dashboardNode = node.value
        blocksPageSlotFrom = slotFrom
        blocksPageSlotTo = slotTo
        if (!Array.isArray(blocks.value)) {
            blocksPageRows = []
            blocksPageError = qsTr("Response shape unknown. Raw JSON remains available.")
            setResult(qsTr("Blocks"), BridgeHelpers.formatValue(blocks.value), false, blocks.value)
            return
        }
        blocksPageRows = sortedBlocks(blocks.value).slice(0, blocksPageLimit)
        blocksPageError = ""
        setResult(qsTr("Blocks"), BridgeHelpers.formatValue(blocksPageRows), false, blocksPageRows)
    }
}

function startBlocksLiveMode(root) {
    with (root) {
        blocksLiveEnabled = true
        blocksLiveError = ""
        blocksLiveSource = ""
        blocksLiveUnknownEvents = 0
        blocksLiveCheckedAt = ""
        if (!blocksPageRows.length) {
            refreshBlocksPage()
        }
        refreshBlocksLivePage()
    }
}

function stopBlocksLiveMode(root) {
    with (root) {
        blocksLiveEnabled = false
        blocksLiveError = ""
        blocksLiveSource = ""
        blocksLiveUnknownEvents = 0
        blocksLiveCheckedAt = ""
    }
}

function refreshBlocksLivePage(root) {
    with (root) {
        const node = requestModule(inspectorModule, "blockchainNode", root.blockchainArgs([]), qsTr("Live blocks node state"), false)
        if (!node.ok) {
            blocksLiveError = node.error
            return
        }
        dashboardNode = node.value
        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const tip = Number(info ? (info.slot || info.lib_slot || 0) : 0)
        const slotTo = tip > 0 ? tip : Math.max(0, Number(blocksPageSlotTo || 0))
        if (slotTo <= 0) {
            blocksLiveError = qsTr("No L1 tip available.")
            return
        }
        const existingTo = Math.max(0, Number(blocksPageSlotTo || 0))
        const slotFrom = existingTo > 0 ? Math.min(existingTo, slotTo) : Math.max(0, slotTo - blocksPageWindow)
        const limit = Math.max(5, Number(blocksPageLimit || 5))
        const response = requestModule(inspectorModule, "blockchainLiveBlocks", root.blockchainArgs([slotFrom, slotTo, limit]), qsTr("Live blocks"), false)
        if (!response.ok) {
            blocksLiveError = response.error
            return
        }

        const report = response.value || {}
        const liveBlocks = Array.isArray(report.blocks) ? report.blocks : []
        const merged = root.mergeLiveBlocks(liveBlocks, blocksPageRows, blocksPageLimit)
        blocksPageRows = merged
        blocksPageSlotTo = Math.max(slotTo, maxBlockSlot(root, merged))
        blocksPageSlotFrom = merged.length ? minBlockSlot(root, merged) : slotFrom
        blocksLiveSource = String(report.source || "")
        blocksLiveUnknownEvents = Array.isArray(report.unknown_events) ? report.unknown_events.length : 0
        blocksLiveCheckedAt = new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss")
        blocksLiveError = ""
        blocksPageError = ""
        setResult(qsTr("Live blocks"), BridgeHelpers.formatValue(report), false, report)
    }
}

function mergeLiveBlocks(root, liveBlocks, existingBlocks, limit) {
    with (root) {
        const rows = []
        const seen = ({})
        appendUniqueBlocks(root, rows, seen, Array.isArray(liveBlocks) ? liveBlocks : [])
        appendUniqueBlocks(root, rows, seen, Array.isArray(existingBlocks) ? existingBlocks : [])
        const sorted = root.sortedBlocks(rows)
        return sorted.slice(0, Math.max(1, Number(limit || sorted.length || 1)))
    }
}

function appendUniqueBlocks(root, rows, seen, blocks) {
    with (root) {
        for (let i = 0; i < blocks.length; ++i) {
            const block = blocks[i]
            const keys = blockDedupeKeys(root, block)
            let duplicate = false
            for (let keyIndex = 0; keyIndex < keys.length; ++keyIndex) {
                if (seen[keys[keyIndex]] === true) {
                    duplicate = true
                    break
                }
            }
            if (duplicate) {
                continue
            }
            for (let seenIndex = 0; seenIndex < keys.length; ++seenIndex) {
                seen[keys[seenIndex]] = true
            }
            rows.push(block)
        }
    }
}

function blockDedupeKeys(root, block) {
    with (root) {
        const keys = []
        const hash = root.blockHash(block)
        if (hash.length) {
            keys.push("hash:" + hash)
        }
        const slot = root.blockSlot(block)
        if (slot > 0) {
            keys.push("slot:" + slot)
        }
        return keys
    }
}

function maxBlockSlot(root, blocks) {
    with (root) {
        let max = 0
        const rows = Array.isArray(blocks) ? blocks : []
        for (let i = 0; i < rows.length; ++i) {
            max = Math.max(max, root.blockSlot(rows[i]))
        }
        return max
    }
}

function minBlockSlot(root, blocks) {
    with (root) {
        let min = 0
        const rows = Array.isArray(blocks) ? blocks : []
        for (let i = 0; i < rows.length; ++i) {
            const slot = root.blockSlot(rows[i])
            if (slot > 0 && (min === 0 || slot < min)) {
                min = slot
            }
        }
        return min
    }
}

function blocksLiveStatusText(root) {
    with (root) {
        if (!blocksLiveEnabled) {
            return qsTr("Paged")
        }
        if (blocksLiveError.length > 0) {
            return qsTr("Live error")
        }
        if (blocksLiveCheckedAt.length > 0) {
            return qsTr("Live %1").arg(blocksLiveCheckedAt)
        }
        return qsTr("Live")
    }
}

function olderBlocksPage(root) {
    with (root) {
        refreshBlocksPage(Math.max(0, blocksPageSlotFrom - 1))
    }
}

function newerBlocksPage(root) {
    with (root) {
        refreshBlocksPage(blocksPageSlotTo + blocksPageWindow + 1)
    }
}

function setBlocksPageLimit(root, limit) {
    with (root) {
        const value = Math.max(1, Number(limit || blocksPageLimit))
        if (blocksPageLimit === value) {
            return
        }
        blocksPageLimit = value
        refreshBlocksPage(blocksPageSlotTo > 0 ? blocksPageSlotTo : null)
    }
}

function sortedBlocks(root, blocks) {
    with (root) {
        const copy = Array.isArray(blocks) ? blocks.slice(0) : []
        copy.sort(function (left, right) {
            return blockSlot(right) - blockSlot(left)
        })
        return copy
    }
}

function blockSlot(root, block) {
    with (root) {
        return Number(block && block.header ? (block.header.slot || 0) : 0)
    }
}

function blockHash(root, block) {
    with (root) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.id || header.hash || raw.header_hash || raw.hash || "")
    }
}

function blockParent(root, block) {
    with (root) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.parent_block || header.parent_hash || header.parent || raw.parent_hash || raw.parent || "")
    }
}

function blockProof(root, block) {
    with (root) {
        const raw = block || {}
        const header = raw.header || {}
        return header.proof_of_leadership || raw.proof_of_leadership || {}
    }
}

function blockRoot(root, block) {
    with (root) {
        const raw = block || {}
        const header = raw.header || {}
        return String(header.block_root || raw.block_root || "")
    }
}

function blockHeight(root, block) {
    with (root) {
        const raw = block || {}
        const header = raw.header || {}
        return raw.height !== undefined ? raw.height : header.height
    }
}

function blockVersion(root, block) {
    with (root) {
        const raw = block || {}
        const header = raw.header || {}
        return raw.version !== undefined ? raw.version : header.version
    }
}

function blockSignature(root, block) {
    with (root) {
        const raw = block || {}
        const header = raw.header || {}
        return String(raw.signature_hex || raw.signature || header.signature_hex || header.signature || "")
    }
}

function blockStatus(root, block) {
    with (root) {
        const raw = block || {}
        const explicitStatus = String(raw.bedrock_status || raw.status || "")
        if (explicitStatus.length) {
            return explicitStatus
        }
        const chain = raw._chain || {}
        const chainStatus = String(chain.status || "")
        if (chainStatus === "finalized") {
            return qsTr("finalized")
        }
        if (chainStatus === "pending") {
            return qsTr("pending")
        }
        if (chainStatus === "orphaned") {
            return qsTr("orphaned")
        }

        const slot = blockSlot(block)
        const info = blockchainInfo()
        if (!slot || !info) {
            return "-"
        }
        if (info.lib_slot !== undefined && slot <= Number(info.lib_slot)) {
            return qsTr("finalized")
        }
        if (info.slot !== undefined && slot <= Number(info.slot)) {
            return qsTr("pending")
        }
        return "-"
    }
}

function blockchainInfo(root) {
    with (root) {
        const report = dashboardNode
        const probe = report ? report.cryptarchia_info : null
        return probe && probe.value ? probe.value.cryptarchia_info : null
    }
}

function sourceEmptyText(root, source, error, fallback) {
    with (root) {
        const state = sourceState(root, source, error)
        if (state === "unknown_shape") {
            return qsTr("Response shape unknown")
        }
        if (state === "unavailable") {
            return qsTr("Source unavailable")
        }
        if (state === "syncing") {
            return qsTr("Source reachable; syncing")
        }
        return String(fallback || qsTr("No rows in loaded range"))
    }
}

function sourceProblemTitle(root, source, error, fallback) {
    with (root) {
        const state = sourceState(root, source, error)
        if (state === "unknown_shape") {
            return qsTr("Response shape unknown")
        }
        if (state === "syncing") {
            return qsTr("Source syncing")
        }
        return String(fallback || qsTr("Source unavailable"))
    }
}

function sourceState(root, source, error) {
    with (root) {
        if (responseShapeUnknown(root, error)) {
            return "unknown_shape"
        }
        if (String(error || "").length > 0) {
            return "unavailable"
        }
        const sourceName = String(source || "")
        if (sourceName === "indexer") {
            return indexerSourceState(root)
        }
        if (sourceName === "blockchain") {
            return blockchainSourceState(root)
        }
        return "ready"
    }
}

function indexerSourceState(root) {
    with (root) {
        const status = root.networkConnectionState("indexer")
        if (status.known === true && status.ok !== true) {
            return "unavailable"
        }
        const value = status && status.value ? status.value : null
        const normalized = indexerStatusStateText(root, value)
        if (normalized.indexOf("sync") >= 0 || normalized.indexOf("catch") >= 0 || normalized.indexOf("index") >= 0) {
            return "syncing"
        }
        return "ready"
    }
}

function blockchainSourceState(root) {
    with (root) {
        const status = root.networkConnectionState("blockchain")
        if (status.known === true && status.ok !== true) {
            return "unavailable"
        }
        const info = root.cryptarchiaInfo()
        const state = String(info.mode || info.sync_state || info.syncState || "").toLowerCase()
        if (state.indexOf("sync") >= 0 || state.indexOf("catch") >= 0 || state.indexOf("start") >= 0) {
            return "syncing"
        }
        return "ready"
    }
}

function indexerStatusStateText(root, value) {
    with (root) {
        if (!value || typeof value !== "object") {
            return String(value || "").toLowerCase()
        }
        const status = value.status && typeof value.status === "object" ? value.status : value
        return String(status.state || status.status || "").toLowerCase()
    }
}

function responseShapeUnknown(root, error) {
    with (root) {
        return String(error || "").toLowerCase().indexOf("response shape unknown") >= 0
    }
}

function blockTransactions(root, block) {
    with (root) {
        const raw = block || {}
        const transactions = Array.isArray(raw.transactions) ? raw.transactions : []
        const rows = []
        for (let i = 0; i < transactions.length; ++i) {
            const tx = transactions[i]
            const ops = transactionOps(tx)
            rows.push({
                index: i,
                hash: transactionHash(tx),
                ops: ops.length,
                operations: ops.map(function (op, index) {
                    return operationSummary(op, tx, index)
                }),
                raw: tx
            })
        }
        return rows
    }
}

function blockchainBlockDetail(root, block) {
    with (root) {
        const proof = blockProof(block)
        return {
            type: "blockchain_block",
            hash: blockHash(block),
            parent: blockParent(block),
            slot: blockSlot(block),
            height: blockHeight(block),
            status: blockStatus(block),
            version: blockVersion(block),
            block_root: blockRoot(block),
            voucher_cm: String(proof.voucher_cm || ""),
            entropy: String(proof.entropy_contribution || proof.entropy || ""),
            signature: blockSignature(block),
            leader_key: String(proof.leader_key || ""),
            transactions: blockTransactions(block),
            raw: block
        }
    }
}

function blockchainBlockDetailById(root, value) {
    with (root) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = blocksPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const block = rows[i]
            const hash = blockHash(block)
            const slot = String(blockSlot(block))
            if (normalizedHashOrValue(hash) === wanted || slot === wanted) {
                return blockchainBlockDetail(block)
            }
        }
        return null
    }
}

function normalizedHashOrValue(root, value) {
    with (root) {
        let text = root.valueToString(value).trim().toLowerCase()
        if (text.startsWith("0x") && text.length === 66) {
            text = text.slice(2)
        }
        return text
    }
}

function refreshTransactionsPage(root, beforeBlock) {
    with (root) {
        const node = requestModule(inspectorModule, "blockchainNode", root.blockchainArgs([]), qsTr("Transactions node state"), false)
        if (!node.ok) {
            transactionsPageError = node.error
            setResult(qsTr("Transactions"), transactionsPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.lib_slot || info.slot || 0) : 0
        const requestedSlot = Math.max(0, Number(beforeBlock === undefined || beforeBlock === null ? fallbackSlot : beforeBlock))
        const slotTo = fallbackSlot > 0 ? Math.min(requestedSlot, Number(fallbackSlot)) : requestedSlot
        const slotFrom = Math.max(0, slotTo - transactionsPageBlockBatch)
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
        transactionsPageRows = transactionRowsFromBlocks(blocks.value).slice(0, transactionsPageLimit)
        transactionsPageNextBeforeBlock = slotFrom > 0 ? slotFrom - 1 : 0
        transactionsPageError = ""
        setResult(qsTr("Transactions"), BridgeHelpers.formatValue(transactionsPageRows), false, transactionsPageRows)
    }
}

function olderTransactionsPage(root) {
    with (root) {
        refreshTransactionsPage(transactionsPageNextBeforeBlock)
    }
}

function newerTransactionsPage(root) {
    with (root) {
        refreshTransactionsPage(transactionsPageBeforeBlock + transactionsPageBlockBatch + 1)
    }
}

function setTransactionsPageLimit(root, limit) {
    with (root) {
        const value = Math.max(1, Number(limit || transactionsPageLimit))
        if (transactionsPageLimit === value) {
            return
        }
        transactionsPageLimit = value
        refreshTransactionsPage(transactionsPageBeforeBlock > 0 ? transactionsPageBeforeBlock : null)
    }
}

function refreshLezBlocksPage(root, beforeBlock) {
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
            const hasUsableRows = responseBlockArray(root, sequencerResponse) !== null
                || responseBlockArray(root, indexerResponse) !== null
            if (!hasUsableRows && (!sequencerDone || !indexerDone)) {
                return
            }
            lezBlocksPageLoading = false
            root.finishLezBlocksPage(before, sequencerResponse, indexerResponse)
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
        const sequencerBlocks = responseBlockArray(root, sequencerResponse)
        const indexerBlocks = responseBlockArray(root, indexerResponse)
        if (sequencerBlocks !== null) {
            const blocks = root.mergedLezBlocks(sequencerBlocks, indexerBlocks || [], lezBlocksPageLimit)
            lezBlocksPageBeforeBlock = before
            lezBlocksPageRows = blocks
            lezBlocksPageNextBeforeBlock = root.nextIndexerBlocksCursor(blocks)
            lezBlocksPageError = ""
            setResult(qsTr("L2 blocks"), BridgeHelpers.formatValue(lezBlocksPageRows), false, lezBlocksPageRows)
            return
        }

        if (indexerBlocks !== null) {
            const blocks = root.sortedIndexerBlocks(indexerBlocks)
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

function responseBlockArray(root, response) {
    with (root) {
        return response && response.ok === true && Array.isArray(response.value) ? response.value : null
    }
}

function olderLezBlocksPage(root) {
    with (root) {
        if (!lezBlocksPageLoading && lezBlocksPageNextBeforeBlock > 0) {
            refreshLezBlocksPage(lezBlocksPageNextBeforeBlock)
        }
    }
}

function newerLezBlocksPage(root) {
    with (root) {
        if (!lezBlocksPageLoading) {
            refreshLezBlocksPage(null)
        }
    }
}

function setLezBlocksPageLimit(root, limit) {
    with (root) {
        const value = Math.max(1, Number(limit || lezBlocksPageLimit))
        if (lezBlocksPageLimit === value) {
            return
        }
        lezBlocksPageLimit = value
        refreshLezBlocksPage(lezBlocksPageBeforeBlock > 0 ? lezBlocksPageBeforeBlock : null)
    }
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
    with (root) {
        if (Array.isArray(lezTransactionsPageOverflowRows) && lezTransactionsPageOverflowRows.length > 0) {
            lezTransactionsPageRows = lezTransactionsPageOverflowRows.slice(0, lezTransactionsPageLimit)
            lezTransactionsPageOverflowRows = lezTransactionsPageOverflowRows.slice(lezTransactionsPageLimit)
            lezTransactionsPageNextBeforeBlock = lezTransactionsPageOverflowRows.length > 0 ? lezTransactionsPageNextBeforeBlock : lezTransactionsPageOverflowNextBeforeBlock
            setResult(qsTr("L2 transactions"), BridgeHelpers.formatValue(lezTransactionsPageRows), false, lezTransactionsPageRows)
            return
        }
        if (lezTransactionsPageNextBeforeBlock > 0) {
            refreshLezTransactionsPage(lezTransactionsPageNextBeforeBlock)
        }
    }
}

function newerLezTransactionsPage(root) {
    with (root) {
        refreshLezTransactionsPage(null)
    }
}

function setLezTransactionsPageLimit(root, limit) {
    with (root) {
        const value = Math.max(1, Number(limit || lezTransactionsPageLimit))
        if (lezTransactionsPageLimit === value) {
            return
        }
        lezTransactionsPageLimit = value
        refreshLezTransactionsPage(lezTransactionsPageBeforeBlock > 0 ? lezTransactionsPageBeforeBlock : null)
    }
}

function sortedIndexerBlocks(root, blocks) {
    with (root) {
        const copy = Array.isArray(blocks) ? blocks.slice(0) : []
        copy.sort(function (left, right) {
            return root.indexerBlockId(right) - root.indexerBlockId(left)
        })
        return copy
    }
}

function mergedLezBlocks(root, sequencerBlocks, indexerBlocks, limit) {
    with (root) {
        const rows = []
        const seen = ({})
        const indexedById = ({})
        const indexed = root.sortedIndexerBlocks(indexerBlocks)
        for (let i = 0; i < indexed.length; ++i) {
            const block = sourceLezBlock(root, indexed[i], "indexer")
            const id = root.indexerBlockId(block)
            if (id > 0) {
                indexedById[String(id)] = block
            }
        }

        const sequenced = root.sortedIndexerBlocks(sequencerBlocks)
        for (let j = 0; j < sequenced.length; ++j) {
            const block = sourceLezBlock(root, sequenced[j], "sequencer")
            const id = root.indexerBlockId(block)
            appendLezBlock(root, rows, seen, id > 0 && indexedById[String(id)] ? indexedById[String(id)] : block)
        }

        for (let k = 0; k < indexed.length; ++k) {
            appendLezBlock(root, rows, seen, sourceLezBlock(root, indexed[k], "indexer"))
        }

        return root.sortedIndexerBlocks(rows).slice(0, Math.max(1, Number(limit || rows.length || 1)))
    }
}

function sourceLezBlock(root, block, source) {
    with (root) {
        const copy = Object.assign({}, block || {})
        copy.source = source
        return copy
    }
}

function appendLezBlock(root, rows, seen, block) {
    with (root) {
        const id = root.indexerBlockId(block)
        const hash = root.indexerBlockHash(block)
        const key = id > 0 ? "id:" + id : (hash.length ? "hash:" + hash : "")
        if (key.length && seen[key] === true) {
            return
        }
        if (key.length) {
            seen[key] = true
        }
        rows.push(block)
    }
}

function indexerBlockId(root, block) {
    with (root) {
        return Number(block && block.block_id !== undefined ? block.block_id : 0)
    }
}

function indexerBlockHash(root, block) {
    with (root) {
        return String(block && block.header_hash ? block.header_hash : "")
    }
}

function nextIndexerBlocksCursor(root, blocks) {
    with (root) {
        let oldest = 0
        const rows = Array.isArray(blocks) ? blocks : []
        for (let i = 0; i < rows.length; ++i) {
            const id = root.indexerBlockId(rows[i])
            if (id > 0 && (oldest === 0 || id < oldest)) {
                oldest = id
            }
        }
        return oldest > 0 ? oldest : 0
    }
}

function normalizedPositiveInteger(root, value) {
    with (root) {
        const number = Number(value === undefined || value === null ? 0 : value)
        return Number.isFinite(number) && number > 0 ? Math.floor(number) : 0
    }
}

function lezTransactionRowsFromBlocks(root, blocks) {
    with (root) {
        const rows = []
        const sorted = root.sortedIndexerBlocks(blocks)
        for (let i = 0; i < sorted.length; ++i) {
            const block = sorted[i]
            const transactions = Array.isArray(block.transactions) ? block.transactions : []
            for (let j = 0; j < transactions.length; ++j) {
                const tx = transactions[j]
                rows.push({
                    block_id: root.indexerBlockId(block),
                    block_hash: root.indexerBlockHash(block),
                    hash: root.lezTransactionHash(tx),
                    index: tx && tx.index !== undefined ? tx.index : j,
                    kind: String(tx && tx.kind ? tx.kind : ""),
                    program_id_hex: root.transactionProgramIdHex(tx),
                    ops: root.lezTransactionOpCount(tx),
                    raw: tx
                })
            }
        }
        return rows
    }
}

function lezTransactionHash(root, tx) {
    with (root) {
        return String((tx && (tx.hash || tx.tx_hash || tx.transaction_hash)) || "")
    }
}

function transactionProgramIdHex(root, tx) {
    with (root) {
        const value = tx || {}
        const message = value.message && typeof value.message === "object" ? value.message : {}
        const programId = String(value.program_id_hex || value.programIdHex || value.program_id || value.programId
            || message.program_id_hex || message.programIdHex || message.program_id || message.programId || "")
        return root.canonicalProgramIdHex(programId) || root.normalizedHexText(programId)
    }
}

function lezTransactionOpCount(root, tx) {
    with (root) {
        if (tx && Array.isArray(tx.instruction_data)) {
            return tx.instruction_data.length
        }
        if (tx && Array.isArray(tx.ops)) {
            return tx.ops.length
        }
        if (tx && tx.bytecode_len !== undefined && tx.bytecode_len !== null) {
            return tx.bytecode_len
        }
        return 0
    }
}

function transactionRowsFromBlocks(root, blocks) {
    with (root) {
        const rows = []
        const sorted = sortedBlockchainBlocks(blocks)
        for (let i = 0; i < sorted.length; ++i) {
            const block = sorted[i]
            const header = block.header || {}
            const transactions = Array.isArray(block.transactions) ? block.transactions : []
            for (let j = 0; j < transactions.length; ++j) {
                const tx = transactions[j]
                const ops = transactionOps(tx)
                rows.push({
                    slot: header.slot || 0,
                    hash: transactionHash(tx),
                    block: header.id || header.hash || "",
                    index: j,
                    ops: ops.length,
                    operations: ops.map(function (op, index) {
                        return operationSummary(op, tx, index)
                    }),
                    raw: tx
                })
            }
        }
        return rows
    }
}

function sortedBlockchainBlocks(root, blocks) {
    with (root) {
        const copy = Array.isArray(blocks) ? blocks.slice(0) : []
        copy.sort(function (left, right) {
            return Number(right.header ? (right.header.slot || 0) : 0) - Number(left.header ? (left.header.slot || 0) : 0)
        })
        return copy
    }
}

function transactionHash(root, tx) {
    with (root) {
        const mantle = tx && tx.mantle_tx ? tx.mantle_tx : tx
        return String((mantle && mantle.hash) || (tx && tx.hash) || "")
    }
}

function transactionOps(root, tx) {
    with (root) {
        const mantle = tx && tx.mantle_tx ? tx.mantle_tx : tx
        return mantle && Array.isArray(mantle.ops) ? mantle.ops : []
    }
}

function operationSummary(root, op, tx, index) {
    with (root) {
        const opcode = Number(op && op.opcode !== undefined ? op.opcode : -1)
        const payload = op && op.payload ? op.payload : {}
        const proofs = tx && tx.ops_proofs ? tx.ops_proofs : []
        return {
            index: index,
            opcode: opcode,
            opcode_hex: byteHex(opcode),
            opcode_name: operationName(opcode),
            channel: String(payload.channel_id || payload.channelId || payload.channel || ""),
            signer: String(payload.signer || ""),
            parent: String(payload.parent || payload.parent_id || payload.parentId || ""),
            payload: payload,
            proof: Array.isArray(proofs) && proofs.length > index ? proofs[index] : null
        }
    }
}

function byteHex(root, value) {
    with (root) {
        const number = Number(value)
        if (number < 0 || !Number.isFinite(number)) {
            return "-"
        }
        const hex = number.toString(16)
        return "0x" + (hex.length < 2 ? "0" + hex : hex)
    }
}

function operationName(root, opcode) {
    with (root) {
        if (opcode === 0) {
            return "Transfer"
        }
        if (opcode === 16) {
            return "ChannelConfig"
        }
        if (opcode === 17) {
            return "ChannelInscribe"
        }
        if (opcode === 18) {
            return "ChannelDeposit"
        }
        if (opcode === 19) {
            return "ChannelWithdraw"
        }
        if (opcode === 32) {
            return "SDPDeclare"
        }
        if (opcode === 33) {
            return "SDPWithdraw"
        }
        if (opcode === 34) {
            return "SDPActive"
        }
        if (opcode === 48) {
            return "LeaderClaim"
        }
        return qsTr("Unknown")
    }
}

function refreshTransferActivityPage(root, beforeBlock, preserveHistory) {
    with (root) {
        const before = beforeBlock === undefined || beforeBlock === null ? null : beforeBlock
        if (!preserveHistory) {
            transferActivityHistory = []
        }
        const recipients = requestModule(inspectorModule, "indexerTransferRecipients", root.indexerArgs([before, transferActivityBlockBatch]), qsTr("Transfer activity"), false)
        if (!recipients.ok) {
            transferActivityError = recipients.error
            setResult(qsTr("Transfer activity"), transferActivityError, true)
            return
        }

        const page = recipients.value || {}
        const rows = Array.isArray(page.recipients) ? page.recipients : (Array.isArray(recipients.value) ? recipients.value : null)
        if (rows === null) {
            transferActivityBeforeBlock = before || 0
            transferActivityRows = []
            transferActivityNextBeforeBlock = 0
            transferActivityOverflowRows = []
            transferActivityOverflowNextBeforeBlock = 0
            transferActivityError = qsTr("Response shape unknown. Raw JSON remains available.")
            setResult(qsTr("Transfer activity"), BridgeHelpers.formatValue(recipients.value), false, recipients.value)
            return
        }
        transferActivityBeforeBlock = before || 0
        transferActivityRows = rows.slice(0, transferActivityLimit)
        const next = Number(page.next_before_block || 0)
        transferActivityOverflowRows = rows.slice(transferActivityLimit)
        transferActivityOverflowNextBeforeBlock = next > 0 ? next : nextTransferActivityBlock(rows)
        transferActivityNextBeforeBlock = transferActivityOverflowNextBeforeBlock
        transferActivityError = ""
        setResult(qsTr("Transfer activity"), BridgeHelpers.formatValue(transferActivityRows), false, transferActivityRows)
    }
}

function nextTransferActivityPage(root) {
    with (root) {
        if (Array.isArray(transferActivityOverflowRows) && transferActivityOverflowRows.length > 0) {
            const history = Array.isArray(transferActivityHistory) ? transferActivityHistory.slice(0) : []
            history.push(transferActivityBeforeBlock)
            transferActivityHistory = history
            transferActivityRows = transferActivityOverflowRows.slice(0, transferActivityLimit)
            transferActivityOverflowRows = transferActivityOverflowRows.slice(transferActivityLimit)
            transferActivityNextBeforeBlock = transferActivityOverflowRows.length > 0 ? transferActivityNextBeforeBlock : transferActivityOverflowNextBeforeBlock
            setResult(qsTr("Transfer activity"), BridgeHelpers.formatValue(transferActivityRows), false, transferActivityRows)
            return
        }
        const history = Array.isArray(transferActivityHistory) ? transferActivityHistory.slice(0) : []
        history.push(transferActivityBeforeBlock)
        transferActivityHistory = history
        refreshTransferActivityPage(transferActivityNextBeforeBlock, true)
    }
}

function previousTransferActivityPage(root) {
    with (root) {
        const history = Array.isArray(transferActivityHistory) ? transferActivityHistory.slice(0) : []
        if (!history.length) {
            return
        }
        const before = history.pop()
        transferActivityHistory = history
        refreshTransferActivityPage(before || null, true)
    }
}

function setTransferActivityPageLimit(root, limit) {
    with (root) {
        const value = Math.max(1, Number(limit || transferActivityLimit))
        if (transferActivityLimit === value) {
            return
        }
        transferActivityLimit = value
        refreshTransferActivityPage(transferActivityBeforeBlock || null, true)
    }
}

function nextTransferActivityBlock(root, recipients) {
    with (root) {
        const rows = Array.isArray(recipients) ? recipients : []
        let next = 0
        for (let i = 0; i < rows.length; ++i) {
            const slot = Number(rows[i].last_slot || 0)
            if (slot > 0 && (next === 0 || slot < next)) {
                next = slot
            }
        }
        return next
    }
}

function transferRecipientDetail(root, row) {
    with (root) {
        const recipient = row || {}
        return {
            type: "transfer_recipient",
            address: String(recipient.account_ref || recipient.recipient || recipient.address || ""),
            total_received: recipient.received,
            txs: recipient.txs || 0,
            outputs: recipient.outputs || 0,
            references: recipient.references || recipient.outputs || 0,
            last_slot: recipient.last_slot,
            source: String(recipient.source || ""),
            transfers: Array.isArray(recipient.transfers) ? recipient.transfers : [],
            raw: recipient
        }
    }
}

function transferRecipientDetailById(root, value) {
    with (root) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = transferActivityRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.recipient || row.address) === wanted) {
                return transferRecipientDetail(row)
            }
        }
        return null
    }
}

function refreshChannelsPage(root, anchorSlot) {
    with (root) {
        const node = requestModule(inspectorModule, "blockchainNode", root.blockchainArgs([]), qsTr("Channels node state"), false)
        if (!node.ok) {
            channelsPageError = node.error
            setResult(qsTr("Channels"), channelsPageError, true)
            return
        }

        const infoProbe = node.value ? node.value.cryptarchia_info : null
        const info = infoProbe && infoProbe.value ? infoProbe.value.cryptarchia_info : null
        const fallbackSlot = info ? (info.slot || info.lib_slot || 0) : 0
        const requestedSlot = Math.max(0, Number(anchorSlot === undefined || anchorSlot === null ? fallbackSlot : anchorSlot))
        const slotTo = fallbackSlot > 0 ? Math.min(requestedSlot, Number(fallbackSlot)) : requestedSlot
        const slotFrom = Math.max(0, slotTo - channelsPageWindow)
        const report = requestModule(inspectorModule, "channelScan", root.blockchainRpcArgs([slotFrom, slotTo]), qsTr("Channels"), false)
        if (!report.ok) {
            channelsPageError = report.error
            setResult(qsTr("Channels"), channelsPageError, true)
            return
        }

        if (!report.value || typeof report.value !== "object" || !Array.isArray(report.value.summaries)) {
            channelsPageSlotFrom = slotFrom
            channelsPageSlotTo = slotTo
            channelsPageRows = []
            channelsPageError = qsTr("Response shape unknown. Raw JSON remains available.")
            setResult(qsTr("Channels"), BridgeHelpers.formatValue(report.value), false, report.value)
            return
        }

        channelsPageSlotFrom = slotFrom
        channelsPageSlotTo = slotTo
        channelsPageRows = report.value.summaries.slice(0, channelsPageLimit)
        channelsPageError = ""
        setResult(qsTr("Channels"), BridgeHelpers.formatValue(report.value || {}), false, report.value || {})
    }
}

function olderChannelsPage(root) {
    with (root) {
        refreshChannelsPage(Math.max(0, channelsPageSlotFrom - 1))
    }
}

function newerChannelsPage(root) {
    with (root) {
        refreshChannelsPage(channelsPageSlotTo + channelsPageWindow + 1)
    }
}

function setChannelsPageLimit(root, limit) {
    with (root) {
        const value = Math.max(1, Number(limit || channelsPageLimit))
        if (channelsPageLimit === value) {
            return
        }
        channelsPageLimit = value
        refreshChannelsPage(channelsPageSlotTo > 0 ? channelsPageSlotTo : null)
    }
}

function channelDetail(root, row) {
    with (root) {
        const channel = row || {}
        const channelId = String(channel.channel || channel.channel_id || "")
        const lastTxHash = String(channel.last_tx_hash || channel.tx_hash || "")
        const lastBlockHash = String(channel.last_block_hash || channel.header || channel.block_hash || "")
        const keyValues = Array.isArray(channel.key_values)
            ? channel.key_values
            : (Array.isArray(channel.accredited_keys) ? channel.accredited_keys.map(function (key) { return String(key) }) : [])
        return {
            type: "channel",
            channel: channelId,
            channel_id: channelId,
            operation_type: String(channel.operation_type || channel.last_operation_type || ""),
            l1_slot: channel.last_slot || channel.l1_slot,
            header: lastBlockHash,
            l1_header_hash: lastBlockHash,
            tx_hash: lastTxHash,
            transaction_hash: lastTxHash,
            parent: String(channel.parent || channel.parent_hash || ""),
            signer: String(channel.signer || channel.author || ""),
            source_confidence: String(channel.source_confidence || channel.source || "scan"),
            label: channel.label,
            first_slot: channel.first_slot,
            first_tx_hash: channel.first_tx_hash,
            first_block_hash: channel.first_block_hash,
            last_slot: channel.last_slot || channel.tip_slot,
            last_tx_hash: lastTxHash,
            last_block_hash: lastBlockHash,
            tip: channel.tip || channel.tip_message,
            balance: channel.balance,
            withdraw_threshold: channel.withdraw_threshold,
            keys: channel.keys !== undefined && channel.keys !== null ? channel.keys : keyValues.length,
            key_values: keyValues,
            operations: channel.operations || 0,
            raw_json: channel.raw || channel,
            raw: channel
        }
    }
}

function channelDetailById(root, value) {
    with (root) {
        const wanted = normalizedHashOrValue(value)
        if (!wanted.length) {
            return null
        }
        const rows = channelsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.channel || row.channel_id) === wanted) {
                return channelDetail(row)
            }
        }
        return null
    }
}
