.import "../../services/BridgeHelpers.js" as BridgeHelpers

function openMantleTransaction(root, hash, navigationContext) {
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
            root.chainPages.invalidateOperationCaller("detail.transaction",
                qsTr("Transaction selection changed."))
            transactionDetailValue = detail
            transactionsPageError = ""
            shell.setResult(qsTr("Mantle transaction"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }
        if (root.chainPages.operationPending("detail.transaction")) {
            transactionDetailError = qsTr("A transaction lookup is already running.")
            shell.setResult(qsTr("Mantle transaction"), transactionDetailError,
                true, null, "transactionDetail")
            return null
        }
        const presentation = root.chainPages.beginPresentation(
            qsTr("Mantle transaction"), "transactionDetail")
        const slot = l1TransactionNavigationSlot(navigationContext)
        if (slot !== null) {
            return root.chainPages.startOperation("detail.transaction",
                "blockchainBlocks", [slot, slot, 10],
                qsTr("Mantle transaction block"), function (response) {
                    if (response && response.ok) {
                        const fetched = transactionDetailFromBlocks(root,
                            response.value, value)
                        if (fetched) {
                            completeMantleTransaction(root, presentation,
                                fetched)
                            return false
                        }
                    }
                    if (response && response.invalidated) {
                        failMantleTransaction(root, presentation,
                            String(response.error
                                || qsTr("Blockchain source changed.")))
                        return false
                    }
                    startMantleTransactionHashLookup(root, value,
                        presentation)
                    return false
                })
        }
        return startMantleTransactionHashLookup(root, value, presentation)
    }
}

function startMantleTransactionHashLookup(root, value, presentation) {
    with (root) {
        return root.chainPages.startOperation("detail.transaction", "blockchainTransaction",
            [root.chainPages.normalizedHashOrValue(value)], qsTr("Mantle transaction"),
            function (response) {
                if (response && response.ok) {
                    const fetched = root.blockchainTransactionDetail(response.value, value)
                    completeMantleTransaction(root, presentation, fetched)
                    return false
                }
                failMantleTransaction(root, presentation,
                    String(response && response.error
                        || qsTr("Mantle transaction %1 was not found.").arg(value)))
                return false
            })
    }
}

function completeMantleTransaction(root, presentation, detail) {
    with (root) {
        transactionDetailValue = detail
        transactionDetailError = ""
        transactionsPageError = ""
        root.chainPages.completePresentation(presentation,
            qsTr("Mantle transaction"), BridgeHelpers.formatValue(detail),
            false, detail)
    }
}

function failMantleTransaction(root, presentation, error) {
    with (root) {
        transactionDetailValue = null
        transactionDetailError = String(error || qsTr("Transaction lookup failed."))
        root.chainPages.completePresentation(presentation,
            qsTr("Mantle transaction"), transactionDetailError, true, null)
    }
}

function l1TransactionNavigationSlot(value) {
    if (!value || typeof value !== "object" || Array.isArray(value)
            || String(value.kind || "") !== "l1_transaction"
            || typeof value.slot !== "number"
            || !Number.isSafeInteger(value.slot) || value.slot < 0) {
        return null
    }
    return value.slot
}

function transactionDetailFromBlocks(root, blocks, hash) {
    const rows = root.chainPages.transactionRowsFromBlocks(blocks)
    const normalized = root.chainPages.normalizedHashOrValue(hash)
    for (let i = 0; i < rows.length; ++i) {
        const row = rows[i]
        if (root.chainPages.normalizedHashOrValue(row.hash) === normalized) {
            return transactionDetailFromRow(row)
        }
    }
    return null
}

function transactionDetailFromRow(row) {
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
        root.chainPages.invalidateOperationCaller("detail.block",
            qsTr("Block selection changed."))
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
        if (root.chainPages.operationPending("detail.block")) {
            blockDetailError = qsTr("A block lookup is already running.")
            shell.setResult(qsTr("Block"), blockDetailError, true, null, "blockDetail")
            return null
        }
        const presentation = root.chainPages.beginPresentation(qsTr("Block"), "blockDetail")
        const acceptBlock = function (response) {
            if (response && response.ok) {
                blockDetailValue = root.chainPages.blockchainBlockDetail(response.value)
                blockDetailError = ""
                blocksPageError = ""
                root.chainPages.completePresentation(presentation, qsTr("Block"),
                    BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return true
            }
            return false
        }
        const failBlock = function (response) {
            blockDetailValue = null
            blockDetailError = response && response.invalidated
                ? String(response.error || qsTr("Blockchain source changed."))
                : qsTr("L1 block %1 was not found.").arg(value)
            root.chainPages.completePresentation(presentation, qsTr("Block"),
                blockDetailError, true, null)
        }
        return root.chainPages.startOperation("detail.block", "blockchainBlock", [value],
            qsTr("Block lookup"), function (response) {
                if (acceptBlock(response)) {
                    return false
                }
                failBlock(response)
                return false
            })
    }
}

function loadBlockchainBlockBySlot(root, slot) {
    with (root) {
        const value = Math.max(0, Number(slot || 0))
        pushNavigationHistory()
        selectView("blockDetail", false)
        blockDetailValue = null
        blockDetailError = ""
        if (root.chainPages.operationPending("detail.block")) {
            blockDetailError = qsTr("A block lookup is already running.")
            shell.setResult(qsTr("Block"), blockDetailError, true, null, "blockDetail")
            return null
        }
        const presentation = root.chainPages.beginPresentation(qsTr("Block"), "blockDetail")
        return root.chainPages.startOperation("detail.block", "blockchainBlocks", [value, value],
            qsTr("Block lookup"), function (response) {
                if (response && response.ok && response.value.length > 0) {
                    blockDetailValue = root.chainPages.blockchainBlockDetail(response.value[0])
                    blockDetailError = ""
                    root.chainPages.completePresentation(presentation, qsTr("Block"),
                        BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                    return false
                }
                blockDetailValue = null
                blockDetailError = response && response.ok
                    ? qsTr("No block found at slot %1.").arg(value)
                    : String(response && response.error || qsTr("Block lookup failed."))
                root.chainPages.completePresentation(presentation, qsTr("Block"),
                    blockDetailError, true, null)
                return false
            })
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
        root.chainPages.invalidateOperationCaller("detail.transaction",
            qsTr("Transaction selection changed."))
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
                return transactionDetailFromRow(row)
            }
        }
        return null
    }
}

function blockchainTransactionDetail(root, value, fallbackHash) {
    with (root) {
        const tx = value || {}
        const hash = root.chainPages.transactionHash(tx)
            || String(tx.hash || tx.tx_hash || tx.transaction_hash || fallbackHash || "")
        const ops = root.chainPages.transactionOps(tx)
        return {
            type: "blockchain_transaction",
            hash: hash,
            block: String(tx.block || tx.block_hash || tx.header_hash || ""),
            slot: tx.slot,
            index: tx.index,
            ops: ops.map(function (op, index) {
                return root.chainPages.operationSummary(op, tx, index)
            }),
            raw: tx.raw || tx
        }
    }
}

function openLocalWallet(root, walletReference, tab) {
    with (root) {
        const target = String(walletReference || "").trim()
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
        if (!bedrockOnly) {
            const configured = walletProfileConfigured()
            const usable = walletProfileUsable()
            if (!configured || !usable) {
                const cachedDetail = String(root.localWalletStatusError || (root.localWalletStatus && root.localWalletStatus.detail) || "")
                shell.setResult(
                    qsTr("Local wallet"),
                    cachedDetail.length
                        ? cachedDetail
                        : (configured
                            ? qsTr("Local wallet profile is not usable.")
                            : qsTr("Configure wallet binary and wallet home before inspecting local wallet state.")),
                    true,
                    null
                )
                checkLocalWalletProfile(false)
                return
            }
        }
        if (localWalletTab === "bedrockNotes" && walletPublicKeyProbe !== target) {
            walletPublicKeyProbe = target
            root.metrics.setModuleReport("blockchain", null)
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
        if (!bedrockOnly) {
            checkLocalWalletProfile(false)
        }
    }
}

function showLocalWalletRequired(root, walletReference) {
    with (root) {
        openLocalWallet(root, walletReference, "profiles")
    }
}
