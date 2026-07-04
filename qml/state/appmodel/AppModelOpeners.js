.import "../../services/BridgeHelpers.js" as BridgeHelpers

function openReference(root, kind, value, payload) {
    with (root) {
        const target = root.valueToString(value).trim()
        if (!target.length && payload === undefined) {
            return
        }

        switch (kind) {
        case "block":
        case "blockHash":
        case "blockNumber":
        case "slot":
            openBlockchainBlock(payload === undefined ? target : payload)
            return
        case "indexerBlock":
            openIndexerBlock(target, payload)
            return
        case "lezBlock":
            openLezBlock(target)
            return
        case "transaction":
        case "transactionHash":
        case "tx":
            openTransaction(target)
            return
        case "mantleTransaction":
            openMantleTransaction(target)
            return
        case "wallet":
            openLocalWallet(target, "profiles")
            return
        case "private":
        case "privateAccount":
            openPrivateAccountReference(target)
            return
        case "bedrockWallet":
        case "note":
            openLocalWallet(target, "bedrockNotes")
            return
        case "recipient":
        case "transferRecipient":
            openRecipient(target)
            return
        case "channel":
            openChannel(payload === undefined ? target : payload)
            return
        case "account":
        case "signer":
            openAccount(target)
            return
        case "program":
            openProgram(target)
            return
        default:
            routeSearch(target)
        }
    }
}

function openMantleTransaction(root, hash) {
    with (root) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        const detail = transactionDetail(value)
        currentView = "transactionDetail"
        transactionDetailError = ""
        if (detail) {
            transactionDetailValue = detail
            transactionsPageError = ""
            setResult(qsTr("Mantle transaction"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        const response = requestModule(inspectorModule, "blockchainTransaction", root.blockchainArgs([root.normalizedHashOrValue(value)]), qsTr("Mantle transaction"), false)
        if (response.ok) {
            const fetched = root.blockchainTransactionDetail(response.value, value)
            transactionDetailValue = fetched
            transactionsPageError = ""
            setResult(qsTr("Mantle transaction"), BridgeHelpers.formatValue(fetched), false, fetched)
            return
        }

        transactionDetailValue = null
        transactionDetailError = response.error || qsTr("Mantle transaction %1 was not found.").arg(value)
        setResult(qsTr("Mantle transaction"), transactionDetailError, true, null, "transactionDetail")
    }
}

function openAccount(root, account) {
    with (root) {
        const value = String(account || "").trim()
        if (!value.length) {
            return
        }
        if (value.indexOf("Private/") === 0 || value.indexOf("private/") === 0) {
            openPrivateAccountReference(value)
            return
        }
        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        currentView = "accounts"
        accountTab = "lookup"
        statusText = qsTr("Account lookup")
        requestModuleAsync(inspectorModule, "account", root.accountLookupArgs(value), qsTr("Account lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok) {
                accountDetailValue = response.value || null
                setResult(qsTr("Account lookup"), response.text, false, response.value, "accounts")
            } else {
                accountDetailValue = null
                setResult(qsTr("Account lookup"), response.error, true, null, "accounts")
            }
        })
    }
}

function openPrivateAccountReference(root, account) {
    with (root) {
        const value = String(account || "").trim()
        currentView = "accounts"
        accountTab = "lookup"
        accountDetailValue = {
            type: "private_account_reference",
            account_id: value.length && value.indexOf("Private/") !== 0 ? "Private/" + value : value,
            source: "local_wallet_required"
        }
        setResult(qsTr("Private account reference"), qsTr("Private account state is local wallet state. Public RPC can only expose public effects, commitments, nullifiers, or proofs when available."), false, accountDetailValue)
    }
}

function openTransaction(root, hash) {
    with (root) {
        openLezTransaction(hash)
    }
}

function openLezSearchTarget(root, target) {
    with (root) {
        const value = String(target || "").trim()
        if (!value.length) {
            return
        }
        if (/^[0-9]+$/.test(value)) {
            openLezBlock(value)
            return
        }
        resolveLezHash(value)
    }
}

function openLezBlock(root, blockId) {
    with (root) {
        const value = String(blockId || "").trim()
        if (!value.length) {
            return
        }

        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        currentView = "l2BlockDetail"
        blockDetailValue = null
        blockDetailError = ""
        statusText = qsTr("LEZ block lookup")
        requestModuleAsync(inspectorModule, "block", root.executionArgs([value]), qsTr("LEZ block"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok && response.value !== null && response.value !== undefined) {
                blockDetailValue = root.indexerBlockDetail(response.value, "sequencer")
                blockDetailError = ""
                setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue, "l2BlockDetail")
            } else {
                blockDetailValue = null
                blockDetailError = response.error || qsTr("LEZ block %1 was not found.").arg(value)
                setResult(qsTr("LEZ block"), blockDetailError, true, null, "l2BlockDetail")
            }
        })
    }
}

function resolveLezHash(root, hash) {
    with (root) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        const serial = searchResolveSerial + 1
        searchResolveSerial = serial
        currentView = "l2BlockDetail"
        blockDetailValue = null
        blockDetailError = ""
        statusText = qsTr("L2 lookup")
        requestModuleAsync(inspectorModule, "indexerBlockByHash", root.indexerArgs([value]), qsTr("LEZ block lookup"), false, function (response) {
            if (serial !== searchResolveSerial) {
                return
            }
            if (response.ok && response.value !== null && response.value !== undefined) {
                const detail = root.indexerBlockDetail(response.value)
                blockDetailValue = detail
                blockDetailError = ""
                setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(detail), false, detail, "l2BlockDetail")
                return
            }
            root.openLezTransaction(value)
        })
    }
}

function openLezTransaction(root, hash) {
    with (root) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        searchResolveSerial += 1
        currentView = "l2TransactionDetail"
        inspectTransaction(value, "")
    }
}

function inspectTransaction(root, hash, idl) {
    with (root) {
        const value = String(hash || "").trim()
        if (!value.length) {
            return
        }

        currentView = "l2TransactionDetail"
        const trimmedIdl = String(idl || "").trim()
        const args = root.executionArgs(trimmedIdl.length ? [value, trimmedIdl] : [value])
        const serial = transactionAutoDecodeSerial + 1
        transactionAutoDecodeSerial = serial
        transactionDetailValue = null
        transactionDetailError = ""
        requestModuleAsync(inspectorModule, "inspectTransaction", args, qsTr("Transaction inspection"), false, function (response) {
            if (serial !== transactionAutoDecodeSerial) {
                return
            }
            if (response.ok) {
                transactionDetailValue = response.value
                transactionDetailError = ""
                lezTransactionsPageError = ""
                setResult(qsTr("Transaction"), response.text, false, response.value, "l2TransactionDetail")
                if (!trimmedIdl.length) {
                    root.autoDecodeTransactionDetail(response.value)
                }
            } else {
                transactionDetailValue = null
                transactionDetailError = response.error
                setResult(qsTr("Transaction"), response.error, true, null, "l2TransactionDetail")
            }
        })
    }
}

function openBlockchainBlock(root, blockOrId) {
    with (root) {
        let detail = null
        if (blockOrId && typeof blockOrId === "object") {
            detail = blockchainBlockDetail(blockOrId)
        } else {
            detail = blockchainBlockDetailById(blockOrId)
        }
        if (!detail) {
            const fallback = blockOrId && typeof blockOrId === "object" ? blockHash(blockOrId) : blockOrId
            if (/^[0-9]+$/.test(String(fallback || "").trim())) {
                loadBlockchainBlockBySlot(Number(fallback))
                return
            }
            loadBlockchainBlockById(String(fallback || ""))
            return
        }

        currentView = "blockDetail"
        blockDetailValue = detail
        blockDetailError = ""
        setResult(qsTr("Block"), BridgeHelpers.formatValue(detail), false, detail)
    }
}

function loadBlockchainBlockById(root, blockId) {
    with (root) {
        const value = String(blockId || "").trim()
        if (!value.length) {
            return
        }
        currentView = "blockDetail"
        blockDetailValue = null
        blockDetailError = ""
        const response = requestModule(inspectorModule, "blockchainBlock", root.blockchainArgs([value]), qsTr("Block lookup"), false)
        if (response.ok) {
            blockDetailValue = blockchainBlockDetail(response.value)
            blockDetailError = ""
            blocksPageError = ""
            setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
            return
        }
        const normalized = normalizedHashOrValue(value)
        const retryValue = normalized !== value ? normalized : ""
        if (retryValue.length) {
            const retry = requestModule(inspectorModule, "blockchainBlock", root.blockchainArgs([retryValue]), qsTr("Block lookup"), false)
            if (retry.ok) {
                blockDetailValue = blockchainBlockDetail(retry.value)
                blockDetailError = ""
                blocksPageError = ""
                setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
        }
        currentView = "blockDetail"
        blockDetailValue = null
        blockDetailError = qsTr("L1 block %1 was not found.").arg(value)
        setResult(qsTr("Block"), blockDetailError, true, null, "blockDetail")
    }
}

function loadBlockchainBlockBySlot(root, slot) {
    with (root) {
        const value = Math.max(0, Number(slot || 0))
        currentView = "blockDetail"
        blockDetailValue = null
        blockDetailError = ""
        const response = requestModule(inspectorModule, "blockchainBlocks", root.blockchainArgs([value, value]), qsTr("Block lookup"), false)
        if (response.ok) {
            const blocks = Array.isArray(response.value) ? response.value : []
            if (blocks.length > 0) {
                blockDetailValue = blockchainBlockDetail(blocks[0])
                blockDetailError = ""
                setResult(qsTr("Block"), BridgeHelpers.formatValue(blockDetailValue), false, blockDetailValue)
                return
            }
            blockDetailError = qsTr("No block found at slot %1.").arg(value)
            blockDetailValue = null
            setResult(qsTr("Block"), blockDetailError, true, null, "blockDetail")
        } else {
            blockDetailError = response.error
            blockDetailValue = null
            setResult(qsTr("Block"), response.error, true, null, "blockDetail")
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
        currentView = "transactionDetail"
        transactionDetailValue = detail
        transactionDetailError = ""
        setResult(qsTr("Transaction"), BridgeHelpers.formatValue(detail), false, detail)
    }
}

function transactionDetail(root, hash) {
    with (root) {
        const normalized = normalizedHashOrValue(hash)
        const rows = transactionsPageRows || []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i]
            if (normalizedHashOrValue(row.hash) === normalized) {
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

function openIndexerBlock(root, headerHash, payload) {
    with (root) {
        const value = String(headerHash || "").trim()
        if (!value.length && (payload === undefined || payload === null)) {
            return
        }

        currentView = "l2BlockDetail"
        blockDetailValue = null
        blockDetailError = ""

        if (payload !== undefined && payload !== null && typeof payload === "object") {
            lezBlocksPageError = ""
            const source = String(payload.source || "") === "sequencer" ? "sequencer" : ""
            const detail = root.indexerBlockDetail(payload, source)
            blockDetailValue = detail
            blockDetailError = ""
            setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        const response = requestModule(inspectorModule, "indexerBlockByHash", root.indexerArgs([value]), qsTr("Block lookup"), false)
        if (response.ok) {
            if (response.value === null || response.value === undefined) {
                blockDetailError = qsTr("No block found for %1.").arg(value)
                blockDetailValue = null
                setResult(qsTr("LEZ block"), blockDetailError, true, null, "l2BlockDetail")
                return
            }
            lezBlocksPageError = ""
            const detail = root.indexerBlockDetail(response.value)
            blockDetailValue = detail
            blockDetailError = ""
            setResult(qsTr("LEZ block"), BridgeHelpers.formatValue(detail), false, detail)
        } else {
            blockDetailError = response.error
            blockDetailValue = null
            setResult(qsTr("LEZ block"), blockDetailError, true, null, "l2BlockDetail")
        }
    }
}

function indexerBlockDetail(root, value, source) {
    with (root) {
        const block = value || {}
        const transactions = Array.isArray(block.transactions) ? block.transactions : []
        const fromSequencer = String(source || "") === "sequencer"
        return {
            type: fromSequencer ? "sequencer_block" : "indexer_block",
            hash: String(block.header_hash || ""),
            parent: String(block.parent_hash || ""),
            block_id: block.block_id,
            slot: block.block_id,
            height: block.block_id,
            status: fromSequencer ? String(block.status || block.bedrock_status || "") : String(block.bedrock_status || ""),
            version: "",
            block_root: "",
            voucher_cm: "",
            entropy: "",
            signature: "",
            leader_key: "",
            transactions: transactions.map(function (tx, index) {
                return {
                    index: tx.index !== undefined ? tx.index : index,
                    hash: String(tx.hash || ""),
                    ops: Array.isArray(tx.instruction_data) ? tx.instruction_data.length : 0,
                    operations: [],
                    raw: tx.raw || tx
                }
            }),
            raw: block.raw || block
        }
    }
}

function openLocalWallet(root, wallet, tab) {
    with (root) {
        const target = String(wallet || "").trim()
        const targetTab = String(tab || "").length ? String(tab || "") : "profiles"
        const bedrockOnly = targetTab === "bedrockNotes"
        currentView = "localWallet"
        localWalletTab = targetTab
        localWalletLookupTarget = target
        transferRecipientDetailValue = null
        if (bedrockOnly && !bedrockWalletSourceConfigured()) {
            setResult(
                qsTr("Bedrock wallet"),
                qsTr("Configure a Bedrock node endpoint before querying wallet notes."),
                true,
                null
            )
            return
        }
        if (!bedrockOnly && !walletProfileConfigured()) {
            setResult(
                qsTr("Local wallet"),
                qsTr("Configure an explicit local wallet profile. Transfer recipients use recipient:<id>; wallet:<id> is reserved for local wallet state."),
                true,
                null
            )
            return
        }
        const profileStatus = bedrockOnly ? { ok: true, detail: "" } : checkedLocalWalletProfile()
        if (!bedrockOnly && !profileStatus.ok) {
            setResult(
                qsTr("Local wallet"),
                profileStatus.detail.length ? profileStatus.detail : qsTr("Local wallet profile is not usable."),
                true,
                localWalletStatus
            )
            return
        }
        if (localWalletTab === "bedrockNotes" && walletPublicKeyProbe !== target) {
            walletPublicKeyProbe = target
            blockchainModuleReport = null
            bedrockWalletModuleError = ""
            bedrockWalletBalanceValue = null
            bedrockWalletBalanceError = ""
        }
        setResult(
            bedrockOnly ? qsTr("Bedrock wallet") : qsTr("Local wallet"),
            target.length ? (bedrockOnly ? qsTr("Bedrock wallet context: %1").arg(target) : qsTr("Local wallet context: %1").arg(target)) : (bedrockOnly ? qsTr("Bedrock wallet source configured.") : qsTr("Local wallet profile configured.")),
            false,
            walletProfile()
        )
    }
}

function showLocalWalletRequired(root, wallet) {
    with (root) {
        openLocalWallet(wallet, "profiles")
    }
}

function openProgram(root, programId) {
    with (root) {
        const value = String(programId || "").trim()
        if (!value.length) {
            selectView("programs")
            return
        }
        currentView = "programs"
        programTab = "programIds"
        const detail = root.programContextDetail(value)
        setResult(qsTr("Program"), BridgeHelpers.formatValue(detail), false, detail)
    }
}

function programContextDetail(root, programId) {
    with (root) {
        const input = String(programId || "").trim()
        const normalized = root.canonicalProgramIdHex(input) || root.normalizedHexText(input)
        const accountResponse = requestModule(inspectorModule, "account", root.accountLookupArgs(input), qsTr("Program account"), false, false)
        if (!root.knownProgramIdRows().length) {
            const response = requestModule(inspectorModule, "programs", root.executionRpcArgs([]), qsTr("Known programs"), false, false)
            if (!response.ok) {
                return root.programContextFromParts(input, normalized, null, accountResponse, response.error || qsTr("Sequencer known-program lookup failed."))
            }
        }
        return root.programContextFromParts(input, normalized, root.knownProgramRow(normalized), accountResponse, "")
    }
}

function programContextFromParts(root, input, normalized, knownRow, accountResponse, lookupError) {
    with (root) {
        const row = knownRow || {}
        const account = accountResponse && accountResponse.ok === true ? accountResponse.value : null
        const accountHex = account && typeof account === "object" ? String(account.account_id_hex || "") : ""
        const accountBase58 = account && typeof account === "object" ? String(account.account_id_base58 || account.account_id || "") : ""
        const hex = normalized || String(row.hex || "") || accountHex
        const base58 = String(row.base58 || accountBase58 || (root.looksLikeHexId(input) ? "" : input))
        const idls = root.idlEntriesForProgram(hex.length ? hex : input)
        const txs = root.programRecentTransactions(hex)
        return {
            type: "program",
            program_id: base58.length ? base58 : input,
            program_id_hex: hex,
            program_id_base58: base58,
            input: input,
            known_label: String(row.label || ""),
            in_chain: knownRow !== null && knownRow !== undefined,
            verification: knownRow ? "verified" : (lookupError.length ? "unavailable" : "not_found"),
            verification_detail: lookupError,
            account: account,
            account_error: accountResponse && accountResponse.ok !== true ? String(accountResponse.error || "") : "",
            idls: idls,
            recent_transactions: txs,
            source: "sequencer getProgramIds + getAccount"
        }
    }
}

function knownProgramRow(root, programId) {
    with (root) {
        const normalized = root.canonicalProgramIdHex(programId) || root.normalizedHexText(programId)
        if (!normalized.length) {
            return null
        }
        const rows = root.knownProgramIdRows()
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            const rowProgram = String(row.hex || row.programIdHex || "") || root.canonicalProgramIdHex(row.base58 || row.programId || row.program_id)
            if (rowProgram === normalized) {
                return row
            }
        }
        return null
    }
}

function programRecentTransactions(root, programId) {
    with (root) {
        const normalized = root.canonicalProgramIdHex(programId) || root.normalizedHexText(programId)
        if (!normalized.length) {
            return []
        }
        const matches = []
        const rows = Array.isArray(lezTransactionsPageRows) ? lezTransactionsPageRows : []
        for (let i = 0; i < rows.length; ++i) {
            const row = rows[i] || {}
            const txProgram = root.transactionProgramIdHex(row.raw || row)
            if (txProgram === normalized) {
                matches.push({
                    hash: String(row.hash || ""),
                    block_id: row.block_id,
                    kind: String(row.kind || ""),
                    ops: row.ops
                })
            }
        }
        return matches
    }
}

function looksLikeHexId(root, value) {
    with (root) {
        return /^(0x)?[0-9a-fA-F]{64}$/.test(String(value || "").trim())
    }
}

function openRecipient(root, recipient) {
    with (root) {
        const value = String(recipient || "").trim()
        if (!value.length) {
            return
        }

        const detail = transferRecipientDetailById(value)
        if (detail) {
            currentView = "transferActivity"
            transferRecipientDetailValue = detail
            setResult(qsTr("Transfer recipient"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }
        currentView = "transferActivity"
        transferRecipientDetailValue = null
        setResult(qsTr("Transfer recipient"), qsTr("No transfer recipient found for %1 in the loaded finalized L2 block window.").arg(value), true, null)
    }
}

function openChannel(root, channel) {
    with (root) {
        const detail = typeof channel === "object" ? channelDetail(channel) : channelDetailById(channel)
        if (detail) {
            currentView = "channels"
            channelDetailValue = detail
            channelDetailError = ""
            setResult(qsTr("Channel"), BridgeHelpers.formatValue(detail), false, detail)
            return
        }

        const channelId = String(channel || "").trim()
        const response = requestModule(inspectorModule, "channelState", root.blockchainRpcArgs([channelId]), qsTr("Channel"), false)
        if (response.ok) {
            const raw = response.value && typeof response.value === "object" ? response.value : {}
            const state = raw.channel && typeof raw.channel === "object" && !Array.isArray(raw.channel) ? raw.channel : raw
            const value = root.channelDetail(Object.assign({}, state, {
                channel: String(raw.channel_id || state.channel_id || channelId),
                channel_id: String(raw.channel_id || state.channel_id || channelId),
                raw: raw,
                source_confidence: "node"
            }))
            currentView = "channels"
            channelDetailValue = value
            channelDetailError = ""
            setResult(qsTr("Channel"), BridgeHelpers.formatValue(value), false, value)
            return
        }

        currentView = "channels"
        channelDetailValue = null
        channelDetailError = response.error || qsTr("Channel %1 was not found.").arg(channelId)
        setResult(qsTr("Channel"), channelDetailError, true, null, "channels")
    }
}
