function lezTransactionRowsFromBlocks(root, blocks) {
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
                hash: lezTransactionHash(root, tx),
                index: tx && tx.index !== undefined ? tx.index : j,
                kind: String(tx && tx.kind ? tx.kind : ""),
                program_id_hex: transactionProgramIdHex(root, tx),
                ops: lezTransactionOpCount(root, tx),
                raw: tx
            })
        }
    }
    return rows
}

function lezTransactionHash(root, tx) {
    return String((tx && (tx.hash || tx.tx_hash || tx.transaction_hash)) || "")
}

function transactionProgramIdHex(root, tx) {
    const value = tx || {}
    const message = value.message && typeof value.message === "object" ? value.message : {}
    const programId = String(value.program_id_hex || value.programIdHex || value.program_id || value.programId
        || message.program_id_hex || message.programIdHex || message.program_id || message.programId || "")
    return root.canonicalProgramIdHex(programId) || root.normalizedHexText(programId)
}

function lezTransactionOpCount(root, tx) {
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

function transferRecipientDetail(root, row) {
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

function transferRecipientDetailById(root, value) {
    const wanted = root.normalizedHashOrValue(value)
    if (!wanted.length) {
        return null
    }
    const rows = (root.transferActivityRows || []).concat(root.transferActivityOverflowRows || [])
    for (let i = 0; i < rows.length; ++i) {
        const row = rows[i]
        if (root.normalizedHashOrValue(row.recipient || row.address) === wanted) {
            return transferRecipientDetail(root, row)
        }
    }
    return null
}

function channelDetail(root, row) {
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

function channelDetailById(root, value) {
    const wanted = root.normalizedHashOrValue(value)
    if (!wanted.length) {
        return null
    }
    const rows = root.channelsPageRows || []
    for (let i = 0; i < rows.length; ++i) {
        const row = rows[i]
        if (root.normalizedHashOrValue(row.channel || row.channel_id) === wanted) {
            return channelDetail(root, row)
        }
    }
    return null
}
