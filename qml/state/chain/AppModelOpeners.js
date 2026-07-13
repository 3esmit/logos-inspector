.import "../../services/BridgeHelpers.js" as BridgeHelpers

function openMantleTransaction(root, hash) {
    with (root) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        const detail = transactionDetail(value)
        pushNavigationHistory()
        selectView("transactionDetail", false)
        transactionDetailError = ""
        if (detail) {
            transactionDetailValue = detail
            transactionsPageError = ""
            shell.setResult(qsTr("Mantle transaction"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        const response = requestModule(inspectorModule, "blockchainTransaction", root.blockchainArgs([root.chainPages.normalizedHashOrValue(value)]), qsTr("Mantle transaction"), false)
        if (response.ok) {
            const fetched = root.blockchainTransactionDetail(response.value, value)
            transactionDetailValue = fetched
            transactionsPageError = ""
            shell.setResult(qsTr("Mantle transaction"), BridgeHelpers.formatValue(fetched), false, fetched)
            return
        }

        transactionDetailValue = null
        transactionDetailError = response.error || qsTr("Mantle transaction %1 was not found.").arg(value)
        shell.setResult(qsTr("Mantle transaction"), transactionDetailError, true, null, "transactionDetail")
    }
}

function openPrivateAccountReference(root, account) {
    with (root) {
        const value = String(account || "").trim()
        pushNavigationHistory()
        selectView("localWallet", false)
        localWalletTab = "privateSync"
        localWalletLookupTarget = value.length && value.indexOf("Private/") !== 0 ? "Private/" + value : value
        const detail = {
            type: "private_account_reference",
            account_id: localWalletLookupTarget,
            source: "local_wallet_required"
        }
        root.shell.setResult(qsTr("Private account reference"), qsTr("Private account state is local wallet state. Sync the configured local wallet profile to inspect local private state."), !walletProfileConfigured(), detail)
        if (walletProfileConfigured()) {
            checkLocalWalletProfile(false)
        }
    }
}

function openBlockchainBlock(root, blockOrId) {
    with (root) {
        let detail = null
        if (blockOrId && typeof blockOrId === "object") {
            detail = root.chainPages.blockchainBlockDetail(blockOrId)
        } else {
            detail = root.chainPages.blockchainBlockDetailById(blockOrId)
        }
        if (!detail) {
            const fallback = blockOrId && typeof blockOrId === "object" ? root.chainPages.blockHash(blockOrId) : blockOrId
            if (/^[0-9]+$/.test(String(fallback || "").trim())) {
                loadBlockchainBlockBySlot(Number(fallback))
                return
            }
            loadBlockchainBlockById(String(fallback || ""))
            return
        }

        pushNavigationHistory()
        selectView("blockDetail", false)
        blockDetailValue = detail
        blockDetailError = ""
        shell.setResult(qsTr("Block"), BridgeHelpers.formatValue(detail), false, detail)
    }
}

function loadBlockchainBlockById(root, blockId) {
    with (root) {
        const value = String(blockId || "").trim()
        if (!value.length) {
            return
        }
        pushNavigationHistory()
        selectView("blockDetail", false)
        blockDetailValue = null
        blockDetailError = ""
        const response = requestModule(inspectorModule, "blockchainBlock", root.blockchainArgs([value]), qsTr("Block lookup"), false)
        if (response.ok) {
            blockDetailValue = root.chainPages.blockchainBlockDetail(response.value)
            blockDetailError = ""
            blocksPageError = ""
            shell.setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
            return
        }
        const normalized = root.chainPages.normalizedHashOrValue(value)
        const retryValue = normalized !== value ? normalized : ""
        if (retryValue.length) {
            const retry = requestModule(inspectorModule, "blockchainBlock", root.blockchainArgs([retryValue]), qsTr("Block lookup"), false)
            if (retry.ok) {
                blockDetailValue = root.chainPages.blockchainBlockDetail(retry.value)
                blockDetailError = ""
                blocksPageError = ""
                shell.setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
        }
        selectView("blockDetail", false)
        blockDetailValue = null
        blockDetailError = qsTr("L1 block %1 was not found.").arg(value)
        shell.setResult(qsTr("Block"), blockDetailError, true, null, "blockDetail")
    }
}

function loadBlockchainBlockBySlot(root, slot) {
    with (root) {
        const value = Math.max(0, Number(slot || 0))
        pushNavigationHistory()
        selectView("blockDetail", false)
        blockDetailValue = null
        blockDetailError = ""
        const response = requestModule(inspectorModule, "blockchainBlocks", root.blockchainArgs([value, value]), qsTr("Block lookup"), false)
        if (response.ok) {
            const blocks = Array.isArray(response.value) ? response.value : []
            if (blocks.length > 0) {
                blockDetailValue = root.chainPages.blockchainBlockDetail(blocks[0])
                blockDetailError = ""
                shell.setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
            blockDetailError = qsTr("No block found at slot %1.").arg(value)
            blockDetailValue = null
            shell.setResult(qsTr("Block"), blockDetailError, true, null, "blockDetail")
        } else {
            blockDetailError = response.error
            blockDetailValue = null
            shell.setResult(qsTr("Block"), response.error, true, null, "blockDetail")
        }
    }
}

function openBlockchainTransaction(root, transaction, block) {
    with (root) {
        const tx = transaction || {}
        const parentBlock = block || {}
        const detail = {
            type: "blockchain_transaction",
            hash: String(tx.hash || ""),
            block: String(parentBlock.hash || ""),
            slot: parentBlock.slot,
            index: tx.index,
            ops: Array.isArray(tx.operations) ? tx.operations : [],
            raw: tx.raw || null
        }
        pushNavigationHistory()
        selectView("transactionDetail", false)
        transactionDetailValue = detail
        transactionDetailError = ""
        shell.setResult(qsTr("Transaction"), BridgeHelpers.formatValue(detail), false, detail)
    }
}

function transactionDetail(root, hash) {
    with (root) {
        const normalized = root.chainPages.normalizedHashOrValue(hash)
        const rows = transactionsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (root.chainPages.normalizedHashOrValue(row.hash) === normalized) {
                return {
                    type: "blockchain_transaction",
                    hash: row.hash,
                    block: row.block,
                    slot: row.slot,
                    index: row.index,
                    ops: row.operations || [],
                    raw: row.raw
                }
            }
        }
        return null
    }
}

function blockchainTransactionDetail(root, value, fallbackHash) {
    with (root) {
        const tx = value || {}
        const hash = transactionHash(tx) || String(tx.hash || tx.tx_hash || tx.transaction_hash || fallbackHash || "")
        const ops = transactionOps(tx)
        return {
            type: "blockchain_transaction",
            hash: hash,
            block: String(tx.block || tx.block_hash || tx.header_hash || ""),
            slot: tx.slot,
            index: tx.index,
            ops: ops.map(function (op, index) {
                return operationSummary(op, tx, index)
            }),
            raw: tx.raw || tx
        }
    }
}

function openLocalWallet(root, wallet, tab) {
    with (root) {
        const target = String(wallet || "").trim()
        const targetTab = String(tab || "").length ? String(tab || "") : "profiles"
        const bedrockOnly = targetTab === "bedrockNotes"
        pushNavigationHistory()
        selectView("localWallet", false)
        localWalletTab = targetTab
        localWalletLookupTarget = target
        if (bedrockOnly && !bedrockWalletSourceConfigured()) {
            shell.setResult(qsTr("Bedrock wallet"), qsTr("Configure a Bedrock node endpoint before querying wallet balance."), true, null)
            return
        }
        if (!bedrockOnly && !walletProfileConfigured()) {
            shell.setResult(qsTr("Local wallet"), qsTr("Configure wallet binary and wallet home before inspecting local wallet state."), true, null)
            return
        }
        if (!bedrockOnly) {
            const status = root.checkedLocalWalletProfile()
            if (!status.ok) {
                shell.setResult(qsTr("Local wallet"), status.detail.length ? status.detail : qsTr("Local wallet profile is not usable."), true, null)
                return
            }
        }
        if (localWalletTab === "bedrockNotes" && walletPublicKeyProbe !== target) {
            walletPublicKeyProbe = target
            blockchainModuleReport = null
            bedrockWalletModuleError = ""
            bedrockWalletBalanceValue = null
            bedrockWalletBalanceError = ""
        }
        shell.setResult(
            bedrockOnly ? qsTr("Bedrock wallet") : qsTr("Local wallet"),
            target.length ? (bedrockOnly ? qsTr("Bedrock wallet context: %1").arg(target) : qsTr("Local wallet context: %1").arg(target)) : (bedrockOnly ? qsTr("Bedrock wallet source configured.") : qsTr("Local wallet profile configured.")),
            false,
            walletProfile()
        )
    }
}

function showLocalWalletRequired(root, wallet) {
    with (root) {
        openLocalWallet(root, wallet, "profiles")
    }
}
