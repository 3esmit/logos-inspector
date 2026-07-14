.import "../../services/BridgeHelpers.js" as BridgeHelpers
.import "ChainPageQuery.js" as ChainPageQuery
.import "ChainPageQuerySession.js" as ChainPageQuerySession

function refreshBlocksPageRequest(root, anchorSlot, onComplete) {
    with (root) {
        if (root.operationPending("blocks.page.node")
                || root.operationPending("blocks.page.range")
                || root.operationPending("blocks.live.node")
                || root.operationPending("blocks.live.range")) {
            return null
        }
        const presentation = root.beginPresentation(qsTr("Blocks"), "blocks")
        return root.startOperation("blocks.page.node", "blockchainNode", [],
            qsTr("Blocks node state"), function (node) {
                if (!node || !node.ok) {
                    blocksPageError = String(node && node.error || qsTr("Blocks node query failed."))
                    const presented = root.completePresentation(presentation, qsTr("Blocks"),
                        blocksPageError, true, null)
                    if (onComplete) {
                        onComplete(node, presented)
                    }
                    return false
                }
                const window = ChainPageQuery.slotWindow(anchorSlot,
                    ChainPageQuery.slotTip(node.value, false), blocksPageWindow)
                const slotFrom = window.slotFrom
                const slotTo = window.slotTo
                const blockLimit = Math.max(5, Number(blocksPageLimit || 5))
                dashboardNode = node.value
                root.startOperation("blocks.page.range", "blockchainBlocks",
                    [slotFrom, slotTo, blockLimit], qsTr("Blocks"), function (blocks) {
                        if (!blocks || !blocks.ok) {
                            blocksPageError = String(blocks && blocks.error || qsTr("Blocks query failed."))
                            const presented = root.completePresentation(presentation, qsTr("Blocks"),
                                blocksPageError, true, null)
                            if (onComplete) {
                                onComplete(blocks, presented)
                            }
                            return false
                        }
                        blocksPageSlotFrom = slotFrom
                        blocksPageSlotTo = slotTo
                        blocksPageRows = sortedBlocks(blocks.value).slice(0, blocksPageLimit)
                        blocksPageError = ""
                        const presented = root.completePresentation(presentation, qsTr("Blocks"),
                            BridgeHelpers.formatValue(blocksPageRows), false, blocksPageRows)
                        if (onComplete) {
                            onComplete(blocks, presented)
                        }
                        return false
                    })
                return false
            })
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
            refreshBlocksPageRequest(root, undefined, function (response, presented) {
                if (blocksLiveEnabled && response && response.ok && presented === true) {
                    refreshBlocksLivePage(root)
                } else if (blocksLiveEnabled && presented === true) {
                    blocksLiveError = String(response && response.error
                        || qsTr("Blocks page refresh failed."))
                    blocksLiveEnabled = false
                } else if (blocksLiveEnabled) {
                    blocksLiveEnabled = false
                }
            })
            return
        }
        refreshBlocksLivePage(root)
    }
}

function stopBlocksLiveMode(root) {
    with (root) {
        root.invalidateOperationCaller("blocks.live.node", qsTr("Live blocks stopped."))
        root.invalidateOperationCaller("blocks.live.range", qsTr("Live blocks stopped."))
        blocksLiveEnabled = false
        blocksLiveError = ""
        blocksLiveSource = ""
        blocksLiveUnknownEvents = 0
        blocksLiveCheckedAt = ""
    }
}

function refreshBlocksLivePage(root) {
    with (root) {
        if (root.operationPending("blocks.live.node")
                || root.operationPending("blocks.live.range")
                || root.operationPending("blocks.page.node")
                || root.operationPending("blocks.page.range")) {
            return null
        }
        const presentation = root.beginPresentation(qsTr("Live blocks"), "blocks")
        return root.startOperation("blocks.live.node", "blockchainNode", [],
            qsTr("Live blocks node state"), function (node) {
                if (!node || !node.ok) {
                    blocksLiveError = String(node && node.error || qsTr("Live blocks node query failed."))
                    root.completePresentation(presentation, qsTr("Live blocks"),
                        blocksLiveError, true, null)
                    return false
                }
                dashboardNode = node.value
                const window = ChainPageQuery.liveSlotWindow(
                    ChainPageQuery.slotTip(node.value, false), blocksPageSlotTo, blocksPageWindow)
                const slotTo = window.slotTo
                if (slotTo <= 0) {
                    blocksLiveError = qsTr("No L1 tip available.")
                    root.completePresentation(presentation, qsTr("Live blocks"),
                        blocksLiveError, true, null)
                    return false
                }
                const slotFrom = window.slotFrom
                const limit = Math.max(5, Number(blocksPageLimit || 5))
                root.startOperation("blocks.live.range", "blockchainLiveBlocks",
                    [slotFrom, slotTo, limit], qsTr("Live blocks"), function (response) {
                        if (!response || !response.ok) {
                            blocksLiveError = String(response && response.error || qsTr("Live blocks query failed."))
                            root.completePresentation(presentation, qsTr("Live blocks"),
                                blocksLiveError, true, null)
                            return false
                        }
                        const liveReport = applyLiveBlockReport(root,
                            normalizedLiveBlockReport(response.value || {}, "blocks_range"), {
                                slotFrom: slotFrom,
                                slotTo: slotTo,
                                updateResult: false
                            })
                        root.completePresentation(presentation, qsTr("Live blocks"),
                            BridgeHelpers.formatValue(liveReport), false, liveReport)
                        return false
                    })
                return false
            })
    }
}

function normalizedLiveBlockReport(value, fallbackSource) {
    const report = value && typeof value === "object" && !Array.isArray(value) ? value : ({})
    if (Array.isArray(report.blocks)) {
        return {
            endpoint: String(report.endpoint || ""),
            source: String(report.source || fallbackSource || ""),
            blocks: report.blocks,
            unknown_events: Array.isArray(report.unknown_events) ? report.unknown_events : []
        }
    }
    const block = liveBlockFromPayload(value)
    return {
        endpoint: String(report.endpoint || ""),
        source: String(report.source || fallbackSource || ""),
        blocks: block ? [block] : [],
        unknown_events: []
    }
}

function liveBlockFromPayload(value) {
    const payload = livePayload(value)
    if (!payload || typeof payload !== "object" || Array.isArray(payload)) {
        return null
    }
    if (payload.header) {
        return payload
    }
    if (payload.block !== undefined) {
        return liveBlockFromPayload(payload.block)
    }
    if (payload.newBlock !== undefined) {
        return liveBlockFromPayload(payload.newBlock)
    }
    if (payload.new_block !== undefined) {
        return liveBlockFromPayload(payload.new_block)
    }
    return null
}

function livePayload(value) {
    if (value === undefined || value === null) {
        return null
    }
    if (typeof value === "object") {
        return value
    }
    const text = String(value || "").trim()
    if (!text.length) {
        return null
    }
    const parsed = BridgeHelpers.parseJson(text)
    return parsed.ok ? parsed.value : null
}

function applyLiveBlockReport(root, report, options) {
    with (root) {
        const opts = options || {}
        const liveReport = normalizedLiveBlockReport(report || {}, String(opts.source || ""))
        const liveBlocks = Array.isArray(liveReport.blocks) ? liveReport.blocks : []
        const merged = root.mergeLiveBlocks(liveBlocks, blocksPageRows, blocksPageLimit)
        const slotTo = Number(opts.slotTo !== undefined ? opts.slotTo : blocksPageSlotTo)
        blocksPageRows = merged
        blocksPageSlotTo = Math.max(Number(blocksPageSlotTo || 0), slotTo, maxBlockSlot(root, merged))
        blocksPageSlotFrom = merged.length ? minBlockSlot(root, merged) : Number(opts.slotFrom !== undefined ? opts.slotFrom : blocksPageSlotFrom)
        blocksLiveSource = String(liveReport.source || "")
        blocksLiveUnknownEvents = Array.isArray(liveReport.unknown_events) ? liveReport.unknown_events.length : 0
        blocksLiveCheckedAt = String(opts.checkedAt || new Date().toLocaleTimeString(Qt.locale(), "hh:mm:ss"))
        blocksLiveError = ""
        blocksPageError = ""
        if (opts.updateResult === true) {
            const title = String(opts.resultTitle || qsTr("Live blocks"))
            root.setResult(title, BridgeHelpers.formatValue(liveReport), false, liveReport, "blocks")
        }
        return liveReport
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
        if (blocksWorkflowRunning) {
            return false
        }
        const value = Math.max(1, Number(limit || blocksPageLimit))
        if (blocksPageLimit === value) {
            return false
        }
        blocksPageLimit = value
        refreshBlocksPage(blocksPageSlotTo > 0 ? blocksPageSlotTo : null)
        return true
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
        if (sourceName === "blockchain") {
            return blockchainSourceState(root)
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
        const info = root.cryptarchiaInfo() || {}
        const state = String(info.mode || info.sync_state || info.syncState || "").toLowerCase()
        if (state.indexOf("sync") >= 0 || state.indexOf("catch") >= 0 || state.indexOf("start") >= 0) {
            return "syncing"
        }
        return "ready"
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
    ChainPageQuerySession.refreshTransactionsPage(root, beforeBlock)
}

function olderTransactionsPage(root) {
    ChainPageQuerySession.olderTransactionsPage(root)
}

function newerTransactionsPage(root) {
    ChainPageQuerySession.newerTransactionsPage(root)
}

function setTransactionsPageLimit(root, limit) {
    ChainPageQuerySession.setTransactionsPageLimit(root, limit)
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
