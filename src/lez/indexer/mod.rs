use anyhow::{Context as _, Result};
use serde_json::{Value, json};

mod normalizer;
mod transactions;

pub use normalizer::{IndexerBlockReport, IndexerStatusReport};
pub(crate) use normalizer::{
    summarize_indexer_status_response, validated_indexer_module_block_for_hash,
    validated_indexer_module_block_for_id, validated_indexer_module_block_report,
    verified_indexer_block_report,
};
pub use transactions::{AccountTransactionSummary, TransactionTransferOutputSummary};
pub(crate) use transactions::{
    summarize_indexer_transaction, summarize_transfer_outputs,
    validated_indexer_module_transaction_summary, verified_indexer_transaction_summary,
    with_account_direction,
};

use crate::{
    lez::{AccountReport, TransactionSummary, indexer_account_report},
    rpc::{raw_json_rpc, raw_json_rpc_optional_result},
};

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
    Ok(Some(verified_indexer_block_report(&result)?))
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
    Ok(Some(verified_indexer_block_report(&result)?))
}

pub async fn indexer_transaction(
    endpoint: &str,
    transaction_hash: &str,
) -> Result<Option<TransactionSummary>> {
    let parsed_hash = crate::parse_hash(transaction_hash, "transaction hash")?;
    let result =
        raw_json_rpc_optional_result(endpoint, "getTransaction", json!([parsed_hash.to_string()]))
            .await
            .with_context(|| format!("failed to fetch indexer transaction {parsed_hash}"))?;
    if result.is_null() {
        return Ok(None);
    }
    Ok(Some(verified_indexer_transaction_summary(
        &result,
        transaction_hash,
    )?))
}

pub async fn indexer_account_at_block(
    endpoint: &str,
    account_id: &str,
    block_id: u64,
) -> Result<AccountReport> {
    let account_id = crate::parse_account_id(account_id)?.to_string();
    let response = raw_json_rpc(
        endpoint,
        "getAccountAtBlock",
        json!([account_id.as_str(), block_id]),
    )
    .await
    .with_context(|| format!("failed to fetch indexer account {account_id} at block {block_id}"))?;
    if json_rpc_method_is_unavailable(&response) {
        return Err(super::evidence_capability_error(
            "Indexer does not expose historical account reads",
        ));
    }
    if response.get("error").is_some() {
        return Err(super::evidence_protocol_error(
            "Indexer historical account read returned an RPC error",
        ));
    }
    let result = response.get("result").cloned().ok_or_else(|| {
        super::evidence_protocol_error("Indexer historical account read omitted its result")
    })?;
    if result.is_null() {
        return Err(super::evidence_protocol_error(
            "Indexer historical account read returned no account",
        ));
    }
    indexer_account_report(&result, &account_id)
}

fn json_rpc_method_is_unavailable(response: &Value) -> bool {
    response
        .pointer("/error/code")
        .and_then(Value::as_i64)
        .is_some_and(|code| code == -32_601)
        || response
            .pointer("/error/message")
            .and_then(Value::as_str)
            .is_some_and(|message| message.to_ascii_lowercase().contains("method not found"))
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
    blocks.iter().map(verified_indexer_block_report).collect()
}

pub async fn indexer_health(endpoint: &str) -> Result<Value> {
    raw_json_rpc_optional_result(endpoint, "checkHealth", Value::Array(vec![]))
        .await
        .context("failed to check indexer health")
}
