use serde::Serialize;
use serde_json::Value;

use super::transactions::{
    AccountTransactionSummary, summarize_indexer_transaction, validated_compact_indexer_transaction,
};
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

pub(crate) fn verified_indexer_block_report(value: &Value) -> anyhow::Result<IndexerBlockReport> {
    let indexed = serde_json::from_value::<indexer_service_protocol::Block>(value.clone())
        .map_err(|_| crate::lez::evidence_protocol_error("Indexer block has invalid layout"))?;
    let block: common::block::Block = indexed
        .try_into()
        .map_err(|_| crate::lez::evidence_protocol_error("Indexer block conversion failed"))?;
    crate::lez::block::verify_block_content_hash(&block)?;
    let verified = crate::lez::summarize_block(&block);
    let mut report = summarize_indexer_block(value);
    if report.transactions.len() != verified.transactions.len()
        || report
            .transactions
            .iter()
            .zip(&verified.transactions)
            .any(|(indexed, canonical)| {
                crate::parse_hash(&indexed.hash, "indexed transaction hash")
                    .map(|hash| hash.to_string() != canonical.hash)
                    .unwrap_or(true)
            })
    {
        return Err(crate::lez::evidence_protocol_error(
            "Indexer block transaction evidence is inconsistent",
        ));
    }
    report.block_id = Some(verified.block_id);
    report.header_hash = Some(verified.header_hash);
    report.parent_hash = Some(verified.parent_hash);
    report.timestamp = Some(verified.timestamp);
    report.bedrock_status = Some(verified.bedrock_status);
    report.tx_count = verified.tx_count;
    Ok(report)
}

/// Validates the intentionally compact block schema exposed by the pinned
/// local Indexer module. That schema omits transaction witnesses, proofs, and
/// deployed bytecode, so it cannot support the full content-hash recomputation
/// required for RPC evidence. Keep this boundary module-only; RPC responses
/// continue through `verified_indexer_block_report` above.
pub(crate) fn validated_indexer_module_block_report(
    value: &Value,
) -> anyhow::Result<IndexerBlockReport> {
    let block_id = value_u64_any(value, &["block_id"]).ok_or_else(compact_block_error)?;
    let timestamp = value_u64_any(value, &["timestamp"]).ok_or_else(compact_block_error)?;
    let header_hash = value
        .get("hash")
        .and_then(Value::as_str)
        .and_then(|hash| crate::parse_hash(hash, "Indexer module block hash").ok())
        .ok_or_else(compact_block_error)?
        .to_string();
    let parent_hash = value
        .get("prev_block_hash")
        .and_then(Value::as_str)
        .and_then(|hash| crate::parse_hash(hash, "Indexer module parent hash").ok())
        .ok_or_else(compact_block_error)?
        .to_string();
    let signature_is_valid = value
        .get("signature")
        .and_then(Value::as_str)
        .and_then(|signature| hex::decode(signature).ok())
        .is_some_and(|signature| signature.len() == 64);
    let bedrock_status = value
        .get("bedrock_status")
        .and_then(Value::as_str)
        .filter(|status| matches!(*status, "Pending" | "Safe" | "Finalized"))
        .ok_or_else(compact_block_error)?
        .to_owned();
    let transactions = value
        .get("transactions")
        .and_then(Value::as_array)
        .ok_or_else(compact_block_error)?;
    if !signature_is_valid {
        return Err(compact_block_error());
    }
    let transaction_summaries = transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| validated_compact_indexer_transaction(transaction, index))
        .collect::<anyhow::Result<Vec<_>>>()?;

    Ok(IndexerBlockReport {
        block_id: Some(block_id),
        header_hash: Some(header_hash),
        parent_hash: Some(parent_hash),
        timestamp: Some(timestamp),
        bedrock_status: Some(bedrock_status),
        tx_count: transaction_summaries.len(),
        transactions: transaction_summaries,
        raw: value.clone(),
    })
}

pub(crate) fn validated_indexer_module_block_for_hash(
    value: &Value,
    expected_hash: &str,
) -> anyhow::Result<IndexerBlockReport> {
    let report = validated_indexer_module_block_report(value)?;
    let expected_hash = crate::parse_hash(expected_hash, "block header hash")?.to_string();
    if report.header_hash.as_deref() != Some(expected_hash.as_str()) {
        return Err(crate::lez::evidence_protocol_error(
            "Indexer module block hash does not match requested hash",
        ));
    }
    Ok(report)
}

pub(crate) fn validated_indexer_module_block_for_id(
    value: &Value,
    expected_block_id: u64,
) -> anyhow::Result<IndexerBlockReport> {
    let report = validated_indexer_module_block_report(value)?;
    if report.block_id != Some(expected_block_id) {
        return Err(crate::lez::evidence_protocol_error(
            "Indexer module block ID does not match requested ID",
        ));
    }
    Ok(report)
}

fn compact_block_error() -> anyhow::Error {
    crate::lez::evidence_protocol_error("Indexer module block has invalid compact evidence")
}

#[cfg(test)]
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
    use crate::lez::programs::{program_id_base58, program_id_hex};

    use super::*;

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
    fn validates_official_module_compact_block_contract() -> anyhow::Result<()> {
        let raw = serde_json::json!({
            "bedrock_status": "Finalized",
            "block_id": "2700",
            "hash": "d6a534f93985c0de29510a3483e1153a62df4e3f18ea0d490b1e8167639006b7",
            "prev_block_hash": "1cedbb9fa154bd92fdc1805a481d1657318666384c0ac6ac34f86aea4130624a",
            "signature": "86b04766151e17a0fd07d6e27f28e97eb36c588a6f326865b348370bb060ecf4c94321bf3428cee964344c9cd376df21bb240495d2629d9e9c9b7a57cd786d54",
            "timestamp": "1782986224098",
            "transactions": [{
                "accounts": [
                    {"account_id": "4BdcjoXkq786TMWcBGGHqcxeLYMZmn17rL4eM9ZyRWNU", "nonce": "0"},
                    {"account_id": "4BdcjoXkq786TMWcBGGHqcxeLYMZmn17rL4eM9ZyRWSs", "nonce": "0"},
                    {"account_id": "4BdcjoXkq786TMWcBGGHqcxeLYMZmn17rL4eM9ZyRWkX", "nonce": "0"}
                ],
                "hash": "e2c0e3a95fda8489754a5994d350abcb96df5f231eb11d16fb64a63f0630bdae",
                "instruction_data": [574796258, 415],
                "program_id": "884e693a302d57de1ac4c405ca5bea1df707d1de11d9f87de51b78845aa98e63",
                "signature_count": 0,
                "type": "Public"
            }]
        });

        let report = validated_indexer_module_block_report(&raw)?;

        anyhow::ensure!(report.block_id == Some(2700));
        anyhow::ensure!(report.tx_count == 1);
        anyhow::ensure!(
            report
                .transactions
                .first()
                .map(|transaction| transaction.hash.as_str())
                == Some("e2c0e3a95fda8489754a5994d350abcb96df5f231eb11d16fb64a63f0630bdae")
        );
        Ok(())
    }

    #[test]
    fn rejects_malformed_official_module_compact_block_evidence() -> anyhow::Result<()> {
        let valid = serde_json::json!({
            "bedrock_status": "Safe",
            "block_id": "7",
            "hash": "ab".repeat(32),
            "prev_block_hash": "cd".repeat(32),
            "signature": "ef".repeat(64),
            "timestamp": "1000",
            "transactions": [{
                "type": "ProgramDeployment",
                "hash": "12".repeat(32),
                "bytecode_size": 42
            }]
        });
        anyhow::ensure!(validated_indexer_module_block_report(&valid).is_ok());

        for (pointer, replacement) in [
            ("/signature", serde_json::json!("00")),
            ("/bedrock_status", serde_json::json!("Unknown")),
            ("/transactions/0/hash", serde_json::json!("not-a-hash")),
            ("/transactions/0/bytecode_size", serde_json::json!(-1)),
        ] {
            let mut malformed = valid.clone();
            let Some(field) = malformed.pointer_mut(pointer) else {
                anyhow::bail!("missing compact fixture field {pointer}");
            };
            *field = replacement;
            anyhow::ensure!(
                validated_indexer_module_block_report(&malformed).is_err(),
                "malformed compact block field {pointer} was accepted"
            );
        }
        Ok(())
    }

    #[test]
    fn rejects_compact_block_detail_that_does_not_match_request() -> anyhow::Result<()> {
        let hash = "ab".repeat(32);
        let raw = serde_json::json!({
            "bedrock_status": "Safe",
            "block_id": "7",
            "hash": hash,
            "prev_block_hash": "cd".repeat(32),
            "signature": "ef".repeat(64),
            "timestamp": "1000",
            "transactions": []
        });

        anyhow::ensure!(validated_indexer_module_block_for_hash(&raw, &"ab".repeat(32)).is_ok());
        anyhow::ensure!(validated_indexer_module_block_for_id(&raw, 7).is_ok());
        anyhow::ensure!(
            validated_indexer_module_block_for_hash(&raw, &"12".repeat(32)).is_err(),
            "compact module block accepted a mismatched requested hash"
        );
        anyhow::ensure!(
            validated_indexer_module_block_for_id(&raw, 8).is_err(),
            "compact module block accepted a mismatched requested ID"
        );
        Ok(())
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
