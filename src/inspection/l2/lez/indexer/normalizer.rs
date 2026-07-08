use serde::Serialize;
use serde_json::Value;

use super::transactions::{AccountTransactionSummary, summarize_indexer_transaction};
use crate::value_to_string;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inspection::l2::lez::programs::{program_id_base58, program_id_hex};

    #[test]
    fn summarize_indexer_status_response_maps_status_object() {
        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {
                "state": "syncing",
                "indexedBlockId": "42",
                "lastError": "behind tip"
            }
        });

        let summary = summarize_indexer_status_response(&raw);

        assert_eq!(summary.state, "syncing");
        assert_eq!(summary.indexed_block_id.as_deref(), Some("42"));
        assert_eq!(summary.last_error.as_deref(), Some("behind tip"));
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_indexer_status_response_maps_string_result() {
        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "caught up"
        });

        let summary = summarize_indexer_status_response(&raw);

        assert_eq!(summary.state, "caught up");
        assert_eq!(summary.indexed_block_id, None);
        assert_eq!(summary.last_error, None);
    }

    #[test]
    fn summarize_indexer_status_response_marks_method_not_found_unavailable() {
        let raw = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32601,
                "message": "Method not found"
            }
        });

        let summary = summarize_indexer_status_response(&raw);

        assert_eq!(summary.state, "unavailable");
        assert_eq!(summary.indexed_block_id, None);
        assert!(
            summary
                .last_error
                .as_deref()
                .is_some_and(|error| error.contains("Method not found"))
        );
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_indexer_block_maps_header_hash_and_transactions() {
        let header_hash = "ab".repeat(32);
        let parent_hash = "cd".repeat(32);
        let tx_hash = "ef".repeat(32);
        let program_id = [1_u32; 8];
        let program_id_base58 = program_id_base58(program_id);
        let program_id_hex = program_id_hex(program_id);
        let raw = serde_json::json!({
            "header": {
                "block_id": 44,
                "hash": header_hash.clone(),
                "prev_block_hash": parent_hash.clone(),
                "timestamp": 1000
            },
            "body": {
                "transactions": [{
                    "Public": {
                        "hash": tx_hash.clone(),
                        "message": {
                            "program_id": program_id_base58,
                            "account_ids": ["acct-a"],
                            "instruction_data": [1, 2]
                        }
                    }
                }]
            },
            "bedrock_status": "Finalized"
        });

        let summary = summarize_indexer_block(&raw);

        assert_eq!(summary.block_id, Some(44));
        assert_eq!(summary.header_hash.as_deref(), Some(header_hash.as_str()));
        assert_eq!(summary.parent_hash.as_deref(), Some(parent_hash.as_str()));
        assert_eq!(summary.timestamp, Some(1000));
        assert_eq!(summary.bedrock_status.as_deref(), Some("Finalized"));
        assert_eq!(summary.tx_count, 1);
        assert_eq!(
            summary.transactions.first().map(|tx| tx.hash.as_str()),
            Some(tx_hash.as_str())
        );
        assert_eq!(
            summary
                .transactions
                .first()
                .and_then(|tx| tx.program_id_hex.as_deref()),
            Some(program_id_hex.as_str())
        );
        assert_eq!(summary.raw, raw);
    }

    #[test]
    fn summarize_indexer_block_maps_compact_top_level_transactions() {
        let header_hash = "ab".repeat(32);
        let parent_hash = "cd".repeat(32);
        let raw = serde_json::json!({
            "block_id": "45",
            "hash": header_hash.clone(),
            "prev_block_hash": parent_hash.clone(),
            "timestamp": "1001",
            "bedrock_status": "Finalized",
            "transactions": [{
                "type": "Public",
                "hash": "tx-public",
                "accounts": [{ "account_id": "acct-a", "nonce": 1 }],
                "instruction_data": [1, 2]
            }]
        });

        let summary = summarize_indexer_block(&raw);

        assert_eq!(summary.block_id, Some(45));
        assert_eq!(summary.header_hash.as_deref(), Some(header_hash.as_str()));
        assert_eq!(summary.parent_hash.as_deref(), Some(parent_hash.as_str()));
        assert_eq!(summary.timestamp, Some(1001));
        assert_eq!(summary.tx_count, 1);
        assert_eq!(
            summary.transactions.first().map(|tx| tx.kind.as_str()),
            Some("Public")
        );
        assert_eq!(
            summary.transactions.first().map(|tx| tx.hash.as_str()),
            Some("tx-public")
        );
    }

    #[test]
    fn summarize_indexer_block_does_not_treat_bedrock_parent_as_lez_parent() {
        let raw = serde_json::json!({
            "header": {
                "block_id": 44,
                "hash": "ab".repeat(32)
            },
            "bedrock_parent_id": "cd".repeat(32)
        });

        let summary = summarize_indexer_block(&raw);

        assert_eq!(summary.parent_hash, None);
    }

    #[test]
    fn next_indexer_blocks_cursor_uses_oldest_fetched_block() {
        let blocks = vec![
            IndexerBlockReport {
                block_id: Some(100),
                header_hash: None,
                parent_hash: None,
                timestamp: None,
                bedrock_status: None,
                tx_count: 0,
                transactions: Vec::new(),
                raw: serde_json::json!({}),
            },
            IndexerBlockReport {
                block_id: Some(51),
                header_hash: None,
                parent_hash: None,
                timestamp: None,
                bedrock_status: None,
                tx_count: 0,
                transactions: Vec::new(),
                raw: serde_json::json!({}),
            },
        ];

        assert_eq!(next_indexer_blocks_cursor(&blocks), Some(51));
    }
}
