use anyhow::{Context as _, Result};
use serde_json::{Value, json};

mod normalizer;
mod transactions;

pub use normalizer::{IndexerBlockReport, IndexerStatusReport};
pub(crate) use normalizer::{
    next_indexer_blocks_cursor, summarize_indexer_block, summarize_indexer_status_response,
};
pub use transactions::{AccountTransactionSummary, TransactionTransferOutputSummary};
pub(crate) use transactions::{
    summarize_indexer_transaction, summarize_transfer_outputs, with_account_direction,
};

use super::transfers::{TransferActivityPage, transfer_recipient_summaries_from_blocks};
use crate::{raw_json_rpc, raw_json_rpc_optional_result};

pub async fn indexer_block_by_hash(
    endpoint: &str,
    header_hash: &str,
) -> Result<Option<IndexerBlockReport>> {
    let parsed_hash = crate::parse_hash(header_hash, "block header hash")?;
    let result =
        raw_json_rpc_optional_result(endpoint, "getBlockByHash", json!([parsed_hash.to_string()]))
            .await
            .with_context(|| format!("failed to fetch indexer block {}", parsed_hash))?;
    if result.is_null() {
        return Ok(None);
    }
    Ok(Some(summarize_indexer_block(&result)))
}

pub async fn indexer_block_by_id(
    endpoint: &str,
    block_id: u64,
) -> Result<Option<IndexerBlockReport>> {
    let result = raw_json_rpc_optional_result(endpoint, "getBlockById", json!([block_id]))
        .await
        .with_context(|| format!("failed to fetch indexer block {block_id}"))?;
    if result.is_null() {
        return Ok(None);
    }
    Ok(Some(summarize_indexer_block(&result)))
}

pub async fn indexer_finalized_block_id(endpoint: &str) -> Result<Option<u64>> {
    let result =
        raw_json_rpc_optional_result(endpoint, "getLastFinalizedBlockId", Value::Array(vec![]))
            .await
            .context("failed to fetch indexer finalized block id")?;
    if result.is_null() {
        return Ok(None);
    }
    result
        .as_u64()
        .or_else(|| result.as_str().and_then(|value| value.parse().ok()))
        .map(Some)
        .context("indexer finalized block id was not an unsigned integer")
}

pub async fn indexer_blocks(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<IndexerBlockReport>> {
    let before = before.map_or(Value::Null, |block_id| json!(block_id));
    let result = raw_json_rpc_optional_result(endpoint, "getBlocks", json!([before, limit]))
        .await
        .context("failed to fetch indexer blocks")?;
    if result.is_null() {
        return Ok(Vec::new());
    }
    let blocks = result
        .as_array()
        .context("getBlocks result was not an array")?;
    Ok(blocks.iter().map(summarize_indexer_block).collect())
}

pub async fn indexer_health(endpoint: &str) -> Result<Value> {
    raw_json_rpc_optional_result(endpoint, "checkHealth", Value::Array(vec![]))
        .await
        .context("failed to check indexer health")
}

pub async fn indexer_status(endpoint: &str) -> Result<IndexerStatusReport> {
    let response = raw_json_rpc(endpoint, "getStatus", Value::Array(vec![]))
        .await
        .context("failed to fetch indexer status")?;
    Ok(summarize_indexer_status_response(&response))
}

pub async fn indexer_transfer_recipients(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<TransferActivityPage> {
    let blocks = indexer_blocks(endpoint, before, limit).await?;
    Ok(TransferActivityPage {
        next_before_block: next_indexer_blocks_cursor(&blocks),
        recipients: transfer_recipient_summaries_from_blocks(&blocks),
    })
}
