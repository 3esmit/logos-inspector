use serde::{Deserialize, Serialize};

use crate::{
    inspection::l2::{
        L2AccountActivityRow, L2AccountExistence, L2AccountValue, L2BlockSummary, L2SourceError,
    },
    lez::{
        AccountReport, AccountTransactionSummary, BlockSummary, IndexerBlockReport,
        TransactionSummary,
    },
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct NormalizedL2Block {
    pub summary: L2BlockSummary,
    pub transactions: Vec<TransactionSummary>,
}

pub(crate) fn normalize_sequencer_block(
    block: BlockSummary,
) -> Result<NormalizedL2Block, L2SourceError> {
    let block_hash = canonical_hash(&block.header_hash)?;
    let parent_hash = canonical_hash(&block.parent_hash)?;
    let transactions = normalize_transactions(block.transactions)?;
    Ok(NormalizedL2Block {
        summary: L2BlockSummary {
            block_id: block.block_id,
            block_hash,
            parent_hash,
            timestamp: block.timestamp,
            bedrock_status: block.bedrock_status,
            transaction_count: block.tx_count,
        },
        transactions,
    })
}

pub(crate) fn normalize_indexer_block(
    block: IndexerBlockReport,
) -> Result<NormalizedL2Block, L2SourceError> {
    let block_id = block.block_id.ok_or_else(L2SourceError::protocol_error)?;
    let block_hash = canonical_hash(
        block
            .header_hash
            .as_deref()
            .ok_or_else(L2SourceError::protocol_error)?,
    )?;
    let parent_hash = canonical_hash(
        block
            .parent_hash
            .as_deref()
            .ok_or_else(L2SourceError::protocol_error)?,
    )?;
    let timestamp = block.timestamp.ok_or_else(L2SourceError::protocol_error)?;
    let transactions = normalize_transactions(
        block
            .transactions
            .iter()
            .map(TransactionSummary::from)
            .collect(),
    )?;
    Ok(NormalizedL2Block {
        summary: L2BlockSummary {
            block_id,
            block_hash,
            parent_hash,
            timestamp,
            bedrock_status: block.bedrock_status.unwrap_or_else(|| "Unknown".to_owned()),
            transaction_count: block.tx_count,
        },
        transactions,
    })
}

pub(crate) fn normalize_account(report: AccountReport) -> L2AccountValue {
    L2AccountValue {
        account_id: report.account_id,
        account_id_base58: report.account_id_base58,
        account_id_hex: report.account_id_hex,
        balance: report.balance,
        nonce: report.nonce,
        owner_program_base58: report.owner_base58,
        owner_program_hex: report.owner_hex,
        data_hex: report.data_hex,
        existence: L2AccountExistence::Unknown,
    }
}

pub(crate) fn normalize_activity_row(row: AccountTransactionSummary) -> L2AccountActivityRow {
    L2AccountActivityRow {
        index: row.index,
        transaction_id: row.hash,
        kind: row.kind,
        direction: row.direction,
        program_id_hex: row.program_id_hex,
        account_ids: row.account_ids,
        signer_account_ids: row.signer_account_ids,
        nonces: row.nonces,
        instruction_data: row.instruction_data,
        transfer_outputs: row.transfer_outputs,
        bytecode_len: row.bytecode_len,
    }
}

pub(crate) fn canonical_hash(value: &str) -> Result<String, L2SourceError> {
    crate::parse_hash(value, "L2 hash")
        .map(|hash| hash.to_string())
        .map_err(|_| L2SourceError::protocol_error())
}

fn normalize_transactions(
    transactions: Vec<TransactionSummary>,
) -> Result<Vec<TransactionSummary>, L2SourceError> {
    transactions
        .into_iter()
        .map(|mut transaction| {
            transaction.hash = canonical_hash(&transaction.hash)?;
            Ok(transaction)
        })
        .collect()
}
