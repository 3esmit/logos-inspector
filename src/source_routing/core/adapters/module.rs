use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::{
    AccountReport, AccountTransactionSummary, IndexerBlockReport, IndexerStatusReport, ProbeReport,
    TransactionSummary,
    blockchain::BlockchainNodeReport,
    lez::{
        indexer_account_report, summarize_account_transaction, summarize_indexer_status_response,
        verified_indexer_block_report, verified_indexer_transaction_summary,
    },
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
};

pub(crate) const BLOCKCHAIN_MODULE: &str = "blockchain_module";
pub(crate) const INDEXER_MODULE: &str = "lez_indexer_module";
pub(crate) const LEZ_CORE_MODULE: &str = "lez_core";

pub(crate) async fn blockchain_node_report(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
) -> BlockchainNodeReport {
    BlockchainNodeReport {
        endpoint: BLOCKCHAIN_MODULE.to_owned(),
        cryptarchia_info: ProbeReport::from_result(
            "cryptarchia info",
            "blockchain_module.get_cryptarchia_info",
            transport_call_value(
                transport,
                transport_kind,
                BLOCKCHAIN_MODULE,
                "get_cryptarchia_info",
                Vec::new(),
            )
            .await,
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

pub(crate) async fn blockchain_blocks(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    slot_from: u64,
    slot_to: u64,
) -> Result<Value> {
    validate_slot_range(slot_from, slot_to)?;
    transport_call_value(
        transport,
        transport_kind,
        BLOCKCHAIN_MODULE,
        "get_blocks",
        vec![json!(slot_from), json!(slot_to)],
    )
    .await
}

pub(crate) async fn blockchain_recent_blocks(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<Value> {
    let blocks = blockchain_blocks(transport, transport_kind, slot_from, slot_to).await?;
    Ok(sort_and_limit_blocks(blocks, limit.clamp(1, 500)))
}

pub(crate) async fn blockchain_block(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    block_id: &str,
) -> Result<Value> {
    let block_id = required_text(block_id, "block id")?;
    transport_call_value(
        transport,
        transport_kind,
        BLOCKCHAIN_MODULE,
        "get_block",
        vec![json!(block_id)],
    )
    .await
}

pub(crate) async fn blockchain_transaction(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    transaction_id: &str,
) -> Result<Value> {
    let transaction_id = required_text(transaction_id, "transaction id")?;
    transport_call_value(
        transport,
        transport_kind,
        BLOCKCHAIN_MODULE,
        "get_transaction",
        vec![json!(transaction_id)],
    )
    .await
}

async fn transport_call_value(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    module: &str,
    method: &str,
    args: Vec<Value>,
) -> Result<Value> {
    crate::source_routing::shared::module_bridge::call_value(
        transport,
        transport_kind,
        module,
        method,
        args,
    )
    .await
    .map(|reply| reply.into_value())
}

pub(crate) async fn indexer_health(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
) -> Result<Value> {
    let status = indexer_status(transport, transport_kind).await?;
    Ok(json!({
        "status": "healthy",
        "health": status,
    }))
}

async fn indexer_status(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
) -> Result<IndexerStatusReport> {
    let value = transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getStatus",
        Vec::new(),
    )
    .await?;
    Ok(summarize_indexer_status_response(&json!({
        "result": value,
    })))
}

pub(crate) async fn indexer_finalized_head(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
) -> Result<Value> {
    transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getLastFinalizedBlockId",
        Vec::new(),
    )
    .await
}

pub(crate) async fn indexer_blocks(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    before: Option<u64>,
    limit: u64,
) -> Result<Vec<IndexerBlockReport>> {
    let before = before.map_or_else(String::new, |block_id| block_id.to_string());
    let value = transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getBlocks",
        vec![json!(before), json!(limit.to_string())],
    )
    .await?;
    let blocks = value
        .as_array()
        .context("getBlocks result was not an array")?;
    blocks.iter().map(verified_indexer_block_report).collect()
}

pub(crate) async fn indexer_block_by_hash(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    header_hash: &str,
) -> Result<Option<IndexerBlockReport>> {
    let header_hash = required_text(header_hash, "block header hash")?;
    let value = transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getBlockByHash",
        vec![json!(header_hash)],
    )
    .await?;
    if empty_module_lookup(&value) {
        return Ok(None);
    }
    Ok(Some(verified_indexer_block_report(&value)?))
}

pub(crate) async fn indexer_block_by_id(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    block_id: u64,
) -> Result<Option<IndexerBlockReport>> {
    let value = transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getBlockById",
        vec![json!(block_id.to_string())],
    )
    .await?;
    if empty_module_lookup(&value) {
        return Ok(None);
    }
    Ok(Some(verified_indexer_block_report(&value)?))
}

pub(crate) async fn indexer_transaction(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    transaction_hash: &str,
) -> Result<Option<TransactionSummary>> {
    let transaction_hash = required_text(transaction_hash, "transaction hash")?;
    let value = transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getTransaction",
        vec![json!(transaction_hash)],
    )
    .await?;
    if empty_module_lookup(&value) {
        return Ok(None);
    }
    Ok(Some(verified_indexer_transaction_summary(
        &value,
        transaction_hash,
    )?))
}

pub(crate) async fn indexer_account_at_block(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    account_id: &str,
    block_id: u64,
) -> Result<AccountReport> {
    let account_id = required_text(account_id, "account id")?;
    let value = transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getAccountAtBlock",
        vec![json!(account_id), json!(block_id.to_string())],
    )
    .await?;
    if empty_module_lookup(&value) {
        bail!("getAccountAtBlock returned no account");
    }
    indexer_account_report(&value, account_id)
}

pub(crate) async fn account_transactions_by_account(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    account_id: &str,
    offset: usize,
    limit: usize,
) -> Result<Vec<AccountTransactionSummary>> {
    let account_id = required_text(account_id, "account id")?;
    let value = transport_call_value(
        transport,
        transport_kind,
        INDEXER_MODULE,
        "getTransactionsByAccount",
        vec![
            json!(account_id),
            json!(offset.to_string()),
            json!(limit.to_string()),
        ],
    )
    .await?;
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

fn empty_module_lookup(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty())
}
