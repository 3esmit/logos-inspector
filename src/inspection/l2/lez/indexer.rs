use anyhow::{Context as _, Result};
use serde::Serialize;
use serde_json::{Value, json};

use super::{
    transaction_facts::{AccountTransactionSummary, summarize_indexer_transaction},
    transfers::{TransferActivityPage, transfer_recipient_summaries_from_blocks},
};
use crate::{json_rpc_result, raw_json_rpc, raw_json_rpc_optional_result, value_to_string};

#[derive(Debug, Clone, Serialize)]
pub struct IndexerBlockReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub header_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bedrock_status: Option<String>,
    pub tx_count: usize,
    pub transactions: Vec<AccountTransactionSummary>,
    pub raw: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct IndexerStatusReport {
    pub state: String,
    #[serde(rename = "indexedBlockId", skip_serializing_if = "Option::is_none")]
    pub indexed_block_id: Option<String>,
    #[serde(rename = "lastError", skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    pub raw: Value,
}

pub async fn indexer_block_by_hash(
    endpoint: &str,
    header_hash: &str,
) -> Result<Option<IndexerBlockReport>> {
    let parsed_hash = crate::parse_hash(header_hash, "block header hash")?;
    let response = raw_json_rpc(endpoint, "getBlockByHash", json!([parsed_hash.to_string()]))
        .await
        .with_context(|| format!("failed to fetch indexer block {}", parsed_hash))?;
    let Some(result) = json_rpc_result(&response, "getBlockByHash")? else {
        return Ok(None);
    };
    Ok(Some(summarize_indexer_block(result)))
}

pub async fn indexer_blocks(
    endpoint: &str,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<IndexerBlockReport>> {
    let before = before.map_or(Value::Null, |block_id| json!(block_id));
    let response = raw_json_rpc(endpoint, "getBlocks", json!([before, limit]))
        .await
        .context("failed to fetch indexer blocks")?;
    let Some(result) = json_rpc_result(&response, "getBlocks")? else {
        return Ok(Vec::new());
    };
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

pub(crate) fn summarize_indexer_block(value: &Value) -> IndexerBlockReport {
    let empty = Value::Null;
    let header = value.get("header").unwrap_or(&empty);
    let body = value.get("body").unwrap_or(&empty);
    let transactions = body
        .get("transactions")
        .or_else(|| value.get("transactions"))
        .and_then(Value::as_array);
    let transaction_summaries = transactions
        .into_iter()
        .flatten()
        .enumerate()
        .map(|(index, transaction)| summarize_indexer_transaction(transaction, index))
        .collect::<Vec<_>>();

    IndexerBlockReport {
        block_id: value_u64_any(header, &["block_id", "id", "slot", "height"])
            .or_else(|| value_u64_any(value, &["block_id", "id", "slot", "height"])),
        header_hash: value_string_any(header, &["hash", "header_hash", "header_id"])
            .or_else(|| value_string_any(value, &["hash", "header_hash", "header_id"])),
        parent_hash: value_string_any(header, &["prev_block_hash", "parent_hash", "parent_id"])
            .or_else(|| value_string_any(value, &["prev_block_hash", "parent_hash", "parent_id"])),
        timestamp: value_u64_any(header, &["timestamp", "time"])
            .or_else(|| value_u64_any(value, &["timestamp", "time"])),
        bedrock_status: value_string_any(value, &["bedrock_status", "status"]),
        tx_count: transactions.map_or(transaction_summaries.len(), Vec::len),
        transactions: transaction_summaries,
        raw: value.clone(),
    }
}

pub(crate) fn next_indexer_blocks_cursor(blocks: &[IndexerBlockReport]) -> Option<u64> {
    blocks.iter().filter_map(|block| block.block_id).min()
}

pub(crate) fn summarize_indexer_status_response(response: &Value) -> IndexerStatusReport {
    if let Some(error) = response.get("error") {
        let error_text = value_to_string(error);
        return IndexerStatusReport {
            state: if indexer_status_error_is_method_not_found(error) {
                "unavailable".to_owned()
            } else {
                "error".to_owned()
            },
            indexed_block_id: None,
            last_error: Some(error_text),
            raw: response.clone(),
        };
    }

    let Some(result) = response.get("result") else {
        return IndexerStatusReport {
            state: "unavailable".to_owned(),
            indexed_block_id: None,
            last_error: Some("getStatus returned no result".to_owned()),
            raw: response.clone(),
        };
    };

    if result.is_null() {
        return IndexerStatusReport {
            state: "unavailable".to_owned(),
            indexed_block_id: None,
            last_error: Some("getStatus returned no result".to_owned()),
            raw: response.clone(),
        };
    }

    let indexed_block_id = value_string_any(
        result,
        &[
            "indexedBlockId",
            "indexed_block_id",
            "lastIndexedBlockId",
            "last_indexed_block_id",
            "lastFinalizedBlockId",
            "last_finalized_block_id",
        ],
    );
    let last_error = value_string_any(result, &["lastError", "last_error", "error"]);
    let state = value_string_any(result, &["state", "status", "phase"])
        .or_else(|| scalar_status_text(result))
        .or_else(|| last_error.as_ref().map(|_| "error".to_owned()))
        .unwrap_or_else(|| "unknown".to_owned());

    IndexerStatusReport {
        state,
        indexed_block_id,
        last_error,
        raw: response.clone(),
    }
}

fn scalar_status_text(value: &Value) -> Option<String> {
    match value {
        Value::String(_) | Value::Bool(_) | Value::Number(_) => Some(value_to_string(value)),
        Value::Null | Value::Array(_) | Value::Object(_) => None,
    }
}

fn indexer_status_error_is_method_not_found(error: &Value) -> bool {
    error
        .get("code")
        .and_then(Value::as_i64)
        .is_some_and(|code| code == -32601)
        || error
            .get("message")
            .map(value_to_string)
            .is_some_and(|message| message.to_ascii_lowercase().contains("method not found"))
}

fn value_u64_any(value: &Value, keys: &[&str]) -> Option<u64> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
        })
    })
}

fn value_string_any(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value.get(*key).and_then(|value| {
            let text = value_to_string(value);
            (!text.is_empty() && text != "null").then_some(text)
        })
    })
}
