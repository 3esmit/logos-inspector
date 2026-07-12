use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::{
    ACCOUNT_TRANSACTION_LIMIT, AccountReport, AccountTransactionSummary, IndexerBlockReport,
    IndexerStatusReport, ProbeReport, TransferActivityPage,
    blockchain::BlockchainLiveBlocksReport,
    blockchain::BlockchainNodeReport,
    lez::{
        next_indexer_blocks_cursor, summarize_account_transaction, summarize_indexer_block,
        summarize_indexer_status_response, transfer_recipient_summaries_from_blocks,
    },
    modules::logos_core,
    response_excerpt,
};

pub(crate) const BLOCKCHAIN_MODULE: &str = "blockchain_module";
pub(crate) const INDEXER_MODULE: &str = "lez_indexer_module";
pub(crate) const LEZ_CORE_MODULE: &str = "lez_core";

pub(crate) fn blockchain_node_report() -> BlockchainNodeReport {
    BlockchainNodeReport {
        endpoint: BLOCKCHAIN_MODULE.to_owned(),
        cryptarchia_info: ProbeReport::from_result(
            "cryptarchia info",
            "blockchain_module.get_cryptarchia_info",
            call_value(BLOCKCHAIN_MODULE, "get_cryptarchia_info", &[]),
        ),
        headers: ProbeReport::err(
            "headers",
            "blockchain_module",
            "blockchain_module does not expose header-list reads",
        ),
        network_info: ProbeReport::err(
            "network info",
            "blockchain_module",
            "blockchain_module does not expose network info reads",
        ),
        mantle_metrics: ProbeReport::err(
            "mantle metrics",
            "blockchain_module",
            "blockchain_module does not expose Mantle metrics",
        ),
    }
}

pub(crate) fn blockchain_blocks(slot_from: u64, slot_to: u64) -> Result<Value> {
    validate_slot_range(slot_from, slot_to)?;
    call_value(
        BLOCKCHAIN_MODULE,
        "get_blocks",
        &[slot_from.to_string(), slot_to.to_string()],
    )
}

pub(crate) fn blockchain_recent_blocks(slot_from: u64, slot_to: u64, limit: u64) -> Result<Value> {
    let blocks = blockchain_blocks(slot_from, slot_to)?;
    Ok(sort_and_limit_blocks(blocks, limit.clamp(1, 500)))
}

pub(crate) fn blockchain_live_blocks_snapshot(
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<BlockchainLiveBlocksReport> {
    let blocks = blockchain_recent_blocks(slot_from, slot_to, limit)?;
    Ok(BlockchainLiveBlocksReport {
        endpoint: BLOCKCHAIN_MODULE.to_owned(),
        source: "module_range".to_owned(),
        blocks: value_array(blocks),
        unknown_events: Vec::new(),
    })
}

pub(crate) fn blockchain_block(block_id: &str) -> Result<Value> {
    let block_id = required_text(block_id, "block id")?;
    call_value(BLOCKCHAIN_MODULE, "get_block", &[block_id.to_owned()])
}

pub(crate) fn blockchain_transaction(transaction_id: &str) -> Result<Value> {
    let transaction_id = required_text(transaction_id, "transaction id")?;
    call_value(
        BLOCKCHAIN_MODULE,
        "get_transaction",
        &[transaction_id.to_owned()],
    )
}

pub(crate) fn indexer_health() -> Result<Value> {
    let status = indexer_status()?;
    Ok(json!({
        "status": "healthy",
        "health": status,
    }))
}

pub(crate) fn indexer_status() -> Result<IndexerStatusReport> {
    let value = call_value(INDEXER_MODULE, "getStatus", &[])?;
    Ok(summarize_indexer_status_response(&json!({
        "result": value,
    })))
}

pub(crate) fn indexer_finalized_head() -> Result<Value> {
    call_value(INDEXER_MODULE, "getLastFinalizedBlockId", &[])
}

pub(crate) fn indexer_blocks(before: Option<u64>, limit: u64) -> Result<Vec<IndexerBlockReport>> {
    let before = before.map_or_else(String::new, |block_id| block_id.to_string());
    let value = call_value(INDEXER_MODULE, "getBlocks", &[before, limit.to_string()])?;
    let blocks = value
        .as_array()
        .context("getBlocks result was not an array")?;
    Ok(blocks.iter().map(summarize_indexer_block).collect())
}

pub(crate) fn indexer_block_by_hash(header_hash: &str) -> Result<Option<IndexerBlockReport>> {
    let header_hash = required_text(header_hash, "block header hash")?;
    let value = call_value(INDEXER_MODULE, "getBlockByHash", &[header_hash.to_owned()])?;
    if empty_module_lookup(&value) {
        return Ok(None);
    }
    Ok(Some(summarize_indexer_block(&value)))
}

pub(crate) fn indexer_block_by_id(block_id: u64) -> Result<Option<IndexerBlockReport>> {
    let value = call_value(INDEXER_MODULE, "getBlockById", &[block_id.to_string()])?;
    if empty_module_lookup(&value) {
        return Ok(None);
    }
    Ok(Some(summarize_indexer_block(&value)))
}

pub(crate) fn indexer_transfer_recipients(
    before: Option<u64>,
    limit: u64,
) -> Result<TransferActivityPage> {
    let blocks = indexer_blocks(before, limit)?;
    Ok(TransferActivityPage {
        next_before_block: next_indexer_blocks_cursor(&blocks),
        recipients: transfer_recipient_summaries_from_blocks(&blocks),
    })
}

pub(crate) fn account_transactions_by_account(
    account_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<AccountTransactionSummary>> {
    let account_id = required_text(account_id, "account id")?;
    let value = call_value(
        INDEXER_MODULE,
        "getTransactionsByAccount",
        &[account_id.to_owned(), offset.to_string(), limit.to_string()],
    )?;
    let transactions = value
        .as_array()
        .context("getTransactionsByAccount result was not an array")?;
    Ok(transactions
        .iter()
        .enumerate()
        .map(|(index, transaction)| {
            summarize_account_transaction(transaction, offset + index, account_id)
        })
        .collect())
}

pub(crate) fn attach_module_account_transactions(account: &mut AccountReport) {
    match account_transactions_by_account(&account.account_id_base58, 0, ACCOUNT_TRANSACTION_LIMIT)
    {
        Ok(transactions) => {
            account.related_transactions = Some(transactions);
            account.related_transactions_error = None;
        }
        Err(error) => {
            account.related_transactions = Some(Vec::new());
            account.related_transactions_error = Some(format!("{error:#}"));
        }
    }
}

pub(crate) fn call_value(module: &str, method: &str, args: &[String]) -> Result<Value> {
    let output = logos_core::call(module, method, args)?;
    unwrap_call_output(module, method, output.value)
}

fn unwrap_call_output(module: &str, method: &str, value: Value) -> Result<Value> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !status.is_empty() && status != "ok" {
        bail!(
            "{module}.{method} returned status `{status}`: {}",
            response_excerpt(&value.to_string())
        );
    }

    let Some(result) = value.get("result") else {
        return Ok(parse_json_string(value));
    };
    if let Some(object) = result.as_object()
        && let Some(success) = object.get("success").and_then(Value::as_bool)
    {
        if !success {
            let error = object
                .get("error")
                .map(value_error_text)
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "module call failed".to_owned());
            bail!("{module}.{method} failed: {error}");
        }
        return Ok(object
            .get("value")
            .cloned()
            .map(parse_json_string)
            .unwrap_or(Value::Null));
    }
    Ok(parse_json_string(result.clone()))
}

fn parse_json_string(value: Value) -> Value {
    let Value::String(text) = value else {
        return value;
    };
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return Value::String(text);
    }
    serde_json::from_str(trimmed).unwrap_or(Value::String(text))
}

fn value_error_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

fn required_text<'a>(value: &'a str, label: &str) -> Result<&'a str> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required");
    }
    if value.contains('/') || value.contains('?') || value.contains('#') {
        bail!("{label} cannot contain path separators or query markers");
    }
    Ok(value)
}

fn validate_slot_range(slot_from: u64, slot_to: u64) -> Result<()> {
    if slot_from > slot_to {
        bail!("slot_from must be less than or equal to slot_to");
    }
    Ok(())
}

fn sort_and_limit_blocks(value: Value, limit: u64) -> Value {
    let Value::Array(mut blocks) = value else {
        return value;
    };
    blocks.sort_by_key(|block| std::cmp::Reverse(block_slot(block)));
    blocks.truncate(usize::try_from(limit).unwrap_or(usize::MAX));
    Value::Array(blocks)
}

fn block_slot(block: &Value) -> u64 {
    block
        .get("header")
        .and_then(|header| header.get("slot"))
        .and_then(Value::as_u64)
        .or_else(|| block.get("slot").and_then(Value::as_u64))
        .unwrap_or_default()
}

fn value_array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(values) => values,
        _ => Vec::new(),
    }
}

fn empty_module_lookup(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unwraps_logos_result_json_string_value() -> Result<()> {
        let value = unwrap_call_output(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": {
                    "success": true,
                    "value": "{\"slot\":7}",
                    "error": null
                }
            }),
        )?;

        if value.get("slot").and_then(Value::as_u64) != Some(7) {
            bail!("unexpected slot value: {value}");
        }
        Ok(())
    }

    #[test]
    fn unwraps_plain_qstring_json_result() -> Result<()> {
        let value = unwrap_call_output(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": "[{\"id\":1}]"
            }),
        )?;

        if value.as_array().map(Vec::len) != Some(1) {
            bail!("unexpected array value: {value}");
        }
        Ok(())
    }

    #[test]
    fn unwrap_reports_module_failure() {
        let result = unwrap_call_output(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": {
                    "success": false,
                    "value": null,
                    "error": "not started"
                }
            }),
        );

        assert!(result.is_err_and(|error| error.to_string().contains("not started")));
    }
}
