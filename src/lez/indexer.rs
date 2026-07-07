use anyhow::{Context as _, Result};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::Serialize;
use serde_json::{Value, json};

use super::{
    accounts::AccountTransactionSummary,
    transfers::{TransferActivityPage, transfer_recipient_summaries_from_blocks},
};
use crate::{
    enum_payload, json_rpc_result, normalize_program_id_hex, raw_json_rpc,
    raw_json_rpc_optional_result, value_list_strings, value_to_string,
};

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

pub(crate) fn summarize_indexer_transaction(
    value: &Value,
    index: usize,
) -> AccountTransactionSummary {
    if let Some(kind) = compact_transaction_kind(value) {
        return AccountTransactionSummary {
            index,
            hash: value
                .get("hash")
                .map(value_to_string)
                .unwrap_or_else(|| "-".to_owned()),
            kind,
            direction: None,
            program_id_hex: value
                .get("program_id")
                .map(value_to_string)
                .map(|program_id| normalize_program_id_hex(&program_id).unwrap_or(program_id)),
            account_ids: compact_transaction_account_field_strings(value, "account_id"),
            nonces: compact_transaction_account_field_strings(value, "nonce"),
            instruction_data: value_list_u32(value.get("instruction_data")),
            bytecode_len: value_usize(value.get("bytecode_size")),
            raw: value.clone(),
        };
    }

    let (kind, payload) = enum_payload(value);
    let empty = Value::Null;
    let message = payload.get("message").unwrap_or(&empty);
    let bytecode_len = message.get("bytecode").and_then(|bytecode| match bytecode {
        Value::Array(items) => Some(items.len()),
        Value::String(value) => BASE64_STANDARD.decode(value).ok().map(|bytes| bytes.len()),
        _ => None,
    });
    AccountTransactionSummary {
        index,
        hash: payload
            .get("hash")
            .map(value_to_string)
            .unwrap_or_else(|| "-".to_owned()),
        kind: kind.to_owned(),
        direction: None,
        program_id_hex: message
            .get("program_id")
            .map(value_to_string)
            .map(|program_id| normalize_program_id_hex(&program_id).unwrap_or(program_id)),
        account_ids: indexer_transaction_account_ids(message),
        nonces: value_list_strings(message.get("nonces")),
        instruction_data: value_list_u32(message.get("instruction_data")),
        bytecode_len,
        raw: value.clone(),
    }
}

fn indexer_transaction_account_ids(message: &Value) -> Vec<String> {
    let mut account_ids = value_list_strings(message.get("account_ids"));
    for account_id in value_list_strings(message.get("public_account_ids")) {
        if !account_ids.iter().any(|value| value == &account_id) {
            account_ids.push(account_id);
        }
    }
    account_ids
}

fn compact_transaction_kind(value: &Value) -> Option<String> {
    value
        .get("type")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|kind| !kind.is_empty())
        .map(ToOwned::to_owned)
}

fn compact_transaction_account_field_strings(value: &Value, field: &str) -> Vec<String> {
    value
        .get("accounts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|account| account.get(field))
        .map(value_to_string)
        .collect()
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

fn value_list_u32(value: Option<&Value>) -> Vec<u32> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| {
                item.as_u64()
                    .and_then(|value| u32::try_from(value).ok())
                    .or_else(|| item.as_str().and_then(parse_u32_text))
            })
            .collect(),
        Some(Value::String(value)) => value.split(',').filter_map(parse_u32_text).collect(),
        Some(value) => value
            .as_u64()
            .and_then(|value| u32::try_from(value).ok())
            .into_iter()
            .collect(),
        None => Vec::new(),
    }
}

fn value_usize(value: Option<&Value>) -> Option<usize> {
    value
        .and_then(|value| {
            value
                .as_u64()
                .or_else(|| value.as_str().and_then(|value| value.trim().parse().ok()))
        })
        .and_then(|value| usize::try_from(value).ok())
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

fn parse_u32_text(value: &str) -> Option<u32> {
    value.trim().parse().ok()
}
