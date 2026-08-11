use std::collections::HashSet;

use anyhow::{Context as _, Result, bail};
use lb_core::mantle::{MantleTx, Transaction as _};
use serde_json::{Value, json};

use crate::{
    AccountReport, AccountTransactionSummary, IndexerBlockReport, IndexerStatusReport, ProbeReport,
    TransactionSummary,
    blockchain::BlockchainNodeReport,
    lez::{
        indexer_account_report, summarize_account_transaction, summarize_indexer_status_response,
        validated_indexer_module_block_for_hash, validated_indexer_module_block_for_id,
        validated_indexer_module_block_report, validated_indexer_module_transaction_summary,
    },
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
    support::entity_id::normalize_block_id_text,
};

pub(crate) const BLOCKCHAIN_MODULE: &str = "blockchain_module";
pub(crate) const INDEXER_MODULE: &str = "lez_indexer_module";
pub(crate) const LEZ_CORE_MODULE: &str = "lez_core";

const CLI_TIP_PARENT_WALK_MAX_BLOCKS: usize = 500;

#[derive(Debug)]
pub(crate) struct BlockchainBlocksRead {
    pub(crate) value: Value,
    pub(crate) used_tip_parent_walk: bool,
}

pub(crate) async fn blockchain_node_report(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
) -> BlockchainNodeReport {
    let (cryptarchia_info, headers, network_info, mantle_metrics) =
        blockchain_diagnostic_reads(transport, transport_kind).await;

    BlockchainNodeReport {
        endpoint: BLOCKCHAIN_MODULE.to_owned(),
        cryptarchia_info: ProbeReport::from_result(
            "cryptarchia info",
            "blockchain_module.get_cryptarchia_info",
            cryptarchia_info.map(crate::blockchain::normalize_cryptarchia_info),
        ),
        headers: ProbeReport::from_result(
            "headers",
            "blockchain_module.get_cryptarchia_headers",
            headers,
        ),
        network_info: ProbeReport::from_result(
            "network info",
            "blockchain_module.get_network_info",
            network_info,
        ),
        mantle_metrics: ProbeReport::from_result(
            "mantle metrics",
            "blockchain_module.get_mantle_metrics",
            mantle_metrics,
        ),
    }
}

async fn blockchain_diagnostic_reads(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
) -> (Result<Value>, Result<Value>, Result<Value>, Result<Value>) {
    let cryptarchia_info = || {
        transport_call_value(
            transport,
            transport_kind,
            BLOCKCHAIN_MODULE,
            "get_cryptarchia_info",
            Vec::new(),
        )
    };
    let headers = || {
        transport_call_value(
            transport,
            transport_kind,
            BLOCKCHAIN_MODULE,
            "get_cryptarchia_headers",
            Vec::new(),
        )
    };
    let network_info = || {
        transport_call_value(
            transport,
            transport_kind,
            BLOCKCHAIN_MODULE,
            "get_network_info",
            Vec::new(),
        )
    };
    let mantle_metrics = || {
        transport_call_value(
            transport,
            transport_kind,
            BLOCKCHAIN_MODULE,
            "get_mantle_metrics",
            Vec::new(),
        )
    };

    match transport_kind {
        ModuleTransportKind::LogoscoreCli => (
            cryptarchia_info().await,
            headers().await,
            network_info().await,
            mantle_metrics().await,
        ),
        ModuleTransportKind::Module => tokio::join!(
            cryptarchia_info(),
            headers(),
            network_info(),
            mantle_metrics(),
        ),
    }
}

pub(crate) async fn blockchain_blocks(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    slot_from: u64,
    slot_to: u64,
) -> Result<Value> {
    Ok(
        blockchain_blocks_read(transport, transport_kind, slot_from, slot_to, None)
            .await?
            .value,
    )
}

pub(crate) async fn blockchain_finalized_blocks(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<Value> {
    crate::blockchain::validate_blockchain_slot_range(slot_from, slot_to)?;
    let limit = limit.clamp(1, 500);
    let mut blocks = transport_call_value(
        transport,
        transport_kind,
        BLOCKCHAIN_MODULE,
        "get_finalized_blocks_range",
        vec![json!(slot_from), json!(slot_to), json!(limit)],
    )
    .await?;
    blocks = crate::blockchain::bedrock::normalize_finalized_blocks_range(blocks)
        .context("blockchain_module.get_finalized_blocks_range returned an invalid response")?;
    if transport_kind == ModuleTransportKind::LogoscoreCli {
        blocks = enrich_cli_mantle_transaction_hashes(blocks);
    }
    Ok(blocks)
}

async fn blockchain_blocks_read(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    slot_from: u64,
    slot_to: u64,
    tip_parent_limit: Option<usize>,
) -> Result<BlockchainBlocksRead> {
    crate::blockchain::validate_blockchain_slot_range(slot_from, slot_to)?;
    let mut blocks = transport_call_value(
        transport,
        transport_kind,
        BLOCKCHAIN_MODULE,
        "get_blocks",
        vec![json!(slot_from), json!(slot_to)],
    )
    .await?;
    if transport_kind == ModuleTransportKind::LogoscoreCli {
        blocks = enrich_cli_mantle_transaction_hashes(blocks);
    }
    if transport_kind != ModuleTransportKind::LogoscoreCli
        || slot_to == 0
        || !blocks.as_array().is_some_and(Vec::is_empty)
    {
        return Ok(BlockchainBlocksRead {
            value: blocks,
            used_tip_parent_walk: false,
        });
    }

    Ok(BlockchainBlocksRead {
        value: cli_tip_parent_blocks(transport, slot_from, slot_to, tip_parent_limit).await?,
        used_tip_parent_walk: true,
    })
}

pub(crate) async fn blockchain_recent_blocks(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<Value> {
    Ok(
        blockchain_recent_blocks_read(transport, transport_kind, slot_from, slot_to, limit)
            .await?
            .value,
    )
}

pub(crate) async fn blockchain_recent_blocks_read(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    slot_from: u64,
    slot_to: u64,
    limit: u64,
) -> Result<BlockchainBlocksRead> {
    let limit = limit.clamp(1, CLI_TIP_PARENT_WALK_MAX_BLOCKS as u64);
    let tip_parent_limit =
        usize::try_from(limit).context("recent block limit does not fit the current platform")?;
    let mut blocks = blockchain_blocks_read(
        transport,
        transport_kind,
        slot_from,
        slot_to,
        Some(tip_parent_limit),
    )
    .await?;
    blocks.value = sort_and_limit_blocks(blocks.value, limit);
    Ok(blocks)
}

pub(crate) async fn blockchain_block(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    block_id: &str,
) -> Result<Value> {
    let block_id = normalize_block_id_text(block_id)?;
    let block = transport_call_value(
        transport,
        transport_kind,
        BLOCKCHAIN_MODULE,
        "get_block",
        vec![json!(block_id)],
    )
    .await?;
    if transport_kind != ModuleTransportKind::LogoscoreCli {
        return Ok(block);
    }
    Ok(enrich_cli_mantle_transaction_hashes(
        normalize_tip_parent_block(block, &block_id)?,
    ))
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

pub(crate) async fn indexer_status(
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
    blocks
        .iter()
        .map(validated_indexer_module_block_report)
        .collect()
}

pub(crate) async fn indexer_block_by_hash(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    header_hash: &str,
) -> Result<Option<IndexerBlockReport>> {
    let header_hash = required_text(header_hash, "block header hash")?;
    let header_hash = crate::parse_hash(header_hash, "block header hash")?.to_string();
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
    Ok(Some(validated_indexer_module_block_for_hash(
        &value,
        &header_hash,
    )?))
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
    Ok(Some(validated_indexer_module_block_for_id(
        &value, block_id,
    )?))
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
    Ok(Some(validated_indexer_module_transaction_summary(
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

async fn cli_tip_parent_blocks(
    transport: &SharedModuleTransport,
    slot_from: u64,
    slot_to: u64,
    result_limit: Option<usize>,
) -> Result<Value> {
    let info = transport_call_value(
        transport,
        ModuleTransportKind::LogoscoreCli,
        BLOCKCHAIN_MODULE,
        "get_cryptarchia_info",
        Vec::new(),
    )
    .await?;
    let tip = info
        .get("cryptarchia_info")
        .filter(|value| value.is_object())
        .unwrap_or(&info);
    let tip_slot = tip
        .get("slot")
        .and_then(Value::as_u64)
        .context("blockchain_module.get_cryptarchia_info did not include a numeric tip slot")?;
    if tip_slot < slot_from
        || tip_slot.saturating_sub(slot_to) >= CLI_TIP_PARENT_WALK_MAX_BLOCKS as u64
    {
        return Ok(Value::Array(Vec::new()));
    }
    let mut block_id = cli_tip_block_id(&info)?;
    let mut visited = HashSet::new();
    let mut blocks = Vec::new();

    for _ in 0..CLI_TIP_PARENT_WALK_MAX_BLOCKS {
        if !visited.insert(block_id.clone()) {
            bail!(
                "blockchain_module tip-parent traversal encountered a repeated block id `{block_id}`"
            );
        }
        let block = transport_call_value(
            transport,
            ModuleTransportKind::LogoscoreCli,
            BLOCKCHAIN_MODULE,
            "get_block",
            vec![json!(block_id)],
        )
        .await?;
        let block =
            enrich_cli_mantle_transaction_hashes(normalize_tip_parent_block(block, &block_id)?);
        let slot = tip_parent_block_slot(&block)?;
        if slot < slot_from {
            return Ok(sort_and_limit_blocks(
                Value::Array(blocks),
                CLI_TIP_PARENT_WALK_MAX_BLOCKS as u64,
            ));
        }
        let parent = tip_parent_id(&block)?;
        if slot <= slot_to {
            blocks.push(block);
            if result_limit.is_some_and(|limit| blocks.len() >= limit) {
                return Ok(sort_and_limit_blocks(
                    Value::Array(blocks),
                    CLI_TIP_PARENT_WALK_MAX_BLOCKS as u64,
                ));
            }
        }
        if slot == 0 {
            return Ok(sort_and_limit_blocks(
                Value::Array(blocks),
                CLI_TIP_PARENT_WALK_MAX_BLOCKS as u64,
            ));
        }
        let Some(parent) = parent else {
            return Ok(sort_and_limit_blocks(
                Value::Array(blocks),
                CLI_TIP_PARENT_WALK_MAX_BLOCKS as u64,
            ));
        };
        block_id = parent;
    }

    bail!(
        "blockchain_module.get_blocks returned an empty array and tip-parent traversal reached its {CLI_TIP_PARENT_WALK_MAX_BLOCKS}-block safety limit"
    )
}

fn cli_tip_block_id(info: &Value) -> Result<String> {
    let source = info
        .get("cryptarchia_info")
        .filter(|value| value.is_object())
        .unwrap_or(info);
    let tip = source
        .get("tip")
        .or_else(|| source.get("tip_hash"))
        .and_then(Value::as_str)
        .context("blockchain_module.get_cryptarchia_info did not include a tip block id")?;
    normalize_block_id_text(tip)
        .context("blockchain_module.get_cryptarchia_info returned an invalid tip block id")
}

fn normalize_tip_parent_block(mut block: Value, requested_id: &str) -> Result<Value> {
    let header = block
        .get_mut("header")
        .and_then(Value::as_object_mut)
        .context("blockchain_module.get_block did not return a header object")?;
    if let Some(actual_id) = header
        .get("id")
        .or_else(|| header.get("hash"))
        .and_then(Value::as_str)
    {
        let actual_id = normalize_block_id_text(actual_id)
            .context("blockchain_module.get_block returned an invalid header id")?;
        anyhow::ensure!(
            actual_id == requested_id,
            "blockchain_module.get_block returned header id `{actual_id}` for requested block `{requested_id}`"
        );
    } else {
        header.insert("id".to_owned(), json!(requested_id));
    }
    Ok(block)
}

/// Restore the canonical Mantle transaction identity omitted by the current
/// `blockchain_module` CLI serializer. The module still supplies the exact
/// Mantle transaction payload, so derive only absent hashes with the same
/// protocol implementation that defines the serialized HTTP API.
fn enrich_cli_mantle_transaction_hashes(mut value: Value) -> Value {
    match &mut value {
        Value::Array(blocks) => {
            for block in blocks {
                enrich_cli_block_mantle_transaction_hashes(block);
            }
        }
        Value::Object(_) => enrich_cli_block_mantle_transaction_hashes(&mut value),
        _ => {}
    }
    value
}

fn enrich_cli_block_mantle_transaction_hashes(block: &mut Value) {
    let Some(transactions) = block.get_mut("transactions").and_then(Value::as_array_mut) else {
        return;
    };
    for transaction in transactions {
        let Some(mantle_transaction) = transaction.get_mut("mantle_tx") else {
            continue;
        };
        let hash_is_present = mantle_transaction
            .get("hash")
            .and_then(Value::as_str)
            .is_some_and(|hash| !hash.trim().is_empty());
        if hash_is_present {
            continue;
        }
        let Some(hash) = canonical_mantle_transaction_hash(mantle_transaction) else {
            continue;
        };
        let Some(mantle_transaction) = mantle_transaction.as_object_mut() else {
            continue;
        };
        mantle_transaction.insert("hash".to_owned(), Value::String(hash));
    }
}

fn canonical_mantle_transaction_hash(mantle_transaction: &Value) -> Option<String> {
    let transaction = serde_json::from_value::<MantleTx>(mantle_transaction.clone()).ok()?;
    let hash = transaction.hash();
    Some(hex::encode(hash.0))
}

fn tip_parent_block_slot(block: &Value) -> Result<u64> {
    block
        .get("header")
        .and_then(|header| header.get("slot"))
        .and_then(Value::as_u64)
        .or_else(|| block.get("slot").and_then(Value::as_u64))
        .context("blockchain_module.get_block did not return a numeric slot")
}

fn tip_parent_id(block: &Value) -> Result<Option<String>> {
    let Some(parent) = block.get("header").and_then(|header| {
        header
            .get("parent_block")
            .or_else(|| header.get("parent_hash"))
    }) else {
        return Ok(None);
    };
    let parent = parent
        .as_str()
        .context("blockchain_module.get_block returned a non-text parent block id")?;
    let parent = normalize_block_id_text(parent)
        .context("blockchain_module.get_block returned an invalid parent block id")?;
    Ok((parent != "0".repeat(64)).then_some(parent))
}

fn empty_module_lookup(value: &Value) -> bool {
    value.is_null() || value.as_str().is_some_and(|value| value.trim().is_empty())
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::panic_in_result_fn)]
mod tests {
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport,
    };

    struct TipParentTransport {
        kind: ModuleTransportKind,
        calls: Mutex<Vec<ModuleCall>>,
    }

    struct PeakConcurrentCliTransport {
        active_calls: AtomicUsize,
        peak_calls: AtomicUsize,
    }

    impl PeakConcurrentCliTransport {
        const fn new() -> Self {
            Self {
                active_calls: AtomicUsize::new(0),
                peak_calls: AtomicUsize::new(0),
            }
        }

        fn peak_calls(&self) -> usize {
            self.peak_calls.load(Ordering::SeqCst)
        }
    }

    impl TipParentTransport {
        const fn new() -> Self {
            Self::with_kind(ModuleTransportKind::LogoscoreCli)
        }

        const fn with_kind(kind: ModuleTransportKind) -> Self {
            Self {
                kind,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn call_methods(&self) -> Vec<String> {
            let calls = match self.calls.lock() {
                Ok(calls) => calls,
                Err(poisoned) => poisoned.into_inner(),
            };
            calls.iter().map(|call| call.method().to_owned()).collect()
        }

        fn call_arguments(&self) -> Vec<Vec<Value>> {
            let calls = match self.calls.lock() {
                Ok(calls) => calls,
                Err(poisoned) => poisoned.into_inner(),
            };
            calls.iter().map(|call| call.args().to_vec()).collect()
        }
    }

    impl ModuleTransport for TipParentTransport {
        fn kind(&self) -> ModuleTransportKind {
            self.kind
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let transport = call.transport();
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args = call.args().to_vec();
            match self.calls.lock() {
                Ok(mut calls) => calls.push(call),
                Err(poisoned) => poisoned.into_inner().push(call),
            }
            let reply = if module != BLOCKCHAIN_MODULE {
                Err(anyhow::anyhow!("unexpected module `{module}`"))
            } else if method == "get_blocks" && args == vec![json!(100_u64), json!(130_u64)] {
                Ok(json!([]))
            } else if method == "get_finalized_blocks_range" {
                match args.as_slice() {
                    [_, slot_to, _] => match slot_to.as_u64() {
                        Some(slot_to) => Ok(json!([
                            {
                                "block": {
                                    "header": { "id": test_hash('p'), "slot": slot_to },
                                    "transactions": []
                                },
                                "tip": test_hash('p'),
                                "tip_slot": slot_to,
                                "lib": test_hash('f'),
                                "lib_slot": slot_to.saturating_sub(1)
                            },
                            {
                                "block": {
                                    "header": {
                                        "id": test_hash('f'),
                                        "slot": slot_to.saturating_sub(1)
                                    },
                                    "transactions": []
                                },
                                "tip": test_hash('p'),
                                "tip_slot": slot_to,
                                "lib": test_hash('f'),
                                "lib_slot": slot_to.saturating_sub(1)
                            }
                        ])),
                        None => Err(anyhow::anyhow!(
                            "unexpected finalized block range end {slot_to:?}"
                        )),
                    },
                    _ => Err(anyhow::anyhow!(
                        "unexpected finalized block range arguments {args:?}"
                    )),
                }
            } else if method == "get_cryptarchia_info" && args.is_empty() {
                Ok(json!({
                    "genesis_id": test_hash('0'),
                    "tip": test_hash('a'),
                    "slot": 130,
                }))
            } else if method == "get_cryptarchia_headers" && args.is_empty() {
                Ok(json!([{"slot": 130, "id": test_hash('a')}]))
            } else if method == "get_network_info" && args.is_empty() {
                Ok(json!({"peers": 3}))
            } else if method == "get_mantle_metrics" && args.is_empty() {
                Ok(json!({"transactions": 1}))
            } else if method == "get_block" && args == vec![json!(test_hash('a'))] {
                Ok(test_block(130, test_hash('b'), 2))
            } else if method == "get_block" && args == vec![json!(test_hash('b'))] {
                Ok(test_block(115, test_hash('c'), 1))
            } else if method == "get_block" && args == vec![json!(test_hash('c'))] {
                Ok(test_block(90, "0".repeat(64), 0))
            } else {
                Err(anyhow::anyhow!(
                    "unexpected CLI tip-parent call {method} with {args:?}"
                ))
            };
            Box::pin(async move { reply.map(|value| ModuleCallReply::new(transport, value)) })
        }
    }

    struct FarTipTransport {
        calls: Mutex<Vec<ModuleCall>>,
    }

    impl FarTipTransport {
        fn call_methods(&self) -> Vec<String> {
            let calls = match self.calls.lock() {
                Ok(calls) => calls,
                Err(poisoned) => poisoned.into_inner(),
            };
            calls.iter().map(|call| call.method().to_owned()).collect()
        }
    }

    impl ModuleTransport for FarTipTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::LogoscoreCli
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let transport = call.transport();
            let method = call.method().to_owned();
            let args = call.args().to_vec();
            match self.calls.lock() {
                Ok(mut calls) => calls.push(call),
                Err(poisoned) => poisoned.into_inner().push(call),
            }
            let reply = match method.as_str() {
                "get_blocks"
                    if args == vec![json!(100_u64), json!(100_u64)]
                        || args == vec![json!(100_u64), json!(700_u64)] =>
                {
                    Ok(json!([]))
                }
                "get_cryptarchia_info" if args.is_empty() => Ok(json!({
                    "tip": test_hash('a'),
                    "slot": 700,
                })),
                "get_block" if args == vec![json!(test_hash('a'))] => {
                    Ok(test_block(700, test_hash('b'), 0))
                }
                "get_block" if args == vec![json!(test_hash('b'))] => {
                    Ok(test_block(690, test_hash('c'), 0))
                }
                "get_block" if args == vec![json!(test_hash('c'))] => {
                    Ok(test_block(680, "0".repeat(64), 0))
                }
                _ => Err(anyhow::anyhow!(
                    "unexpected far-tip call {method} with {args:?}"
                )),
            };
            Box::pin(async move { reply.map(|value| ModuleCallReply::new(transport, value)) })
        }
    }

    impl ModuleTransport for PeakConcurrentCliTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::LogoscoreCli
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args = call.args().to_vec();
            Box::pin(async move {
                let active = self.active_calls.fetch_add(1, Ordering::SeqCst) + 1;
                self.peak_calls.fetch_max(active, Ordering::SeqCst);
                tokio::task::yield_now().await;
                self.active_calls.fetch_sub(1, Ordering::SeqCst);
                let value = if module != BLOCKCHAIN_MODULE || !args.is_empty() {
                    Err(anyhow::anyhow!(
                        "unexpected CLI diagnostic call {module}.{method}"
                    ))
                } else {
                    match method.as_str() {
                        "get_cryptarchia_info" => Ok(json!({
                            "genesis_id": test_hash('0'),
                            "tip": test_hash('a'),
                            "slot": 130,
                        })),
                        "get_cryptarchia_headers" => {
                            Ok(json!([{"slot": 130, "id": test_hash('a')}]))
                        }
                        "get_network_info" => Ok(json!({"peers": 3})),
                        "get_mantle_metrics" => Ok(json!({"transactions": 1})),
                        _ => Err(anyhow::anyhow!("unexpected CLI diagnostic method {method}")),
                    }
                };
                value.map(|value| ModuleCallReply::new(ModuleTransportKind::LogoscoreCli, value))
            })
        }
    }

    fn test_hash(character: char) -> String {
        character.to_string().repeat(64)
    }

    fn test_block(slot: u64, parent: String, transaction_count: u64) -> Value {
        json!({
            "header": {
                "slot": slot,
                "parent_block": parent,
            },
            "transactions": (0..transaction_count)
                .map(|index| json!({ "id": format!("transaction-{slot}-{index}") }))
                .collect::<Vec<_>>(),
        })
    }

    fn unhashed_cli_mantle_transaction() -> Value {
        json!({
            "mantle_tx": {
                "ops": [{
                    "opcode": 17,
                    "payload": {
                        "channel_id": "01".repeat(32),
                        "inscription": "00",
                        "parent": "00".repeat(32),
                        "signer": "00".repeat(32),
                    }
                }]
            },
            "ops_proofs": []
        })
    }

    #[test]
    fn cli_blocks_restore_missing_canonical_mantle_transaction_hashes() -> Result<()> {
        let transaction = unhashed_cli_mantle_transaction();
        let mantle = transaction
            .get("mantle_tx")
            .cloned()
            .context("test transaction did not include mantle_tx")?;
        let mantle_transaction = serde_json::from_value::<MantleTx>(mantle)
            .context("test transaction did not deserialize as MantleTx")?;
        let expected = hex::encode(mantle_transaction.hash().0);
        let supplied_hash = "f".repeat(64);
        let enriched = enrich_cli_mantle_transaction_hashes(json!([{
            "header": { "slot": 42 },
            "transactions": [
                transaction,
                { "mantle_tx": { "hash": supplied_hash, "ops": [] } }
            ]
        }]));

        assert_eq!(
            enriched.pointer("/0/transactions/0/mantle_tx/hash"),
            Some(&json!(expected))
        );
        assert_eq!(
            enriched.pointer("/0/transactions/1/mantle_tx/hash"),
            Some(&json!(supplied_hash))
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_empty_block_range_falls_back_to_tip_parent_chain() -> Result<()> {
        let harness = Arc::new(TipParentTransport::new());
        let transport: SharedModuleTransport = harness.clone();

        let value =
            blockchain_blocks(&transport, ModuleTransportKind::LogoscoreCli, 100, 130).await?;
        let blocks = value
            .as_array()
            .context("CLI tip-parent range did not return an array")?;

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].pointer("/header/slot"), Some(&json!(130)));
        assert_eq!(blocks[1].pointer("/header/slot"), Some(&json!(115)));
        assert_eq!(
            blocks[0].pointer("/header/id"),
            Some(&json!(test_hash('a')))
        );
        assert_eq!(
            blocks[1].pointer("/header/id"),
            Some(&json!(test_hash('b')))
        );
        assert_eq!(
            harness.call_methods(),
            vec![
                "get_blocks",
                "get_cryptarchia_info",
                "get_block",
                "get_block",
                "get_block",
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_empty_block_range_does_not_walk_when_tip_is_beyond_module_limit() -> Result<()> {
        let harness = Arc::new(FarTipTransport {
            calls: Mutex::new(Vec::new()),
        });
        let transport: SharedModuleTransport = harness.clone();

        let value =
            blockchain_blocks(&transport, ModuleTransportKind::LogoscoreCli, 100, 100).await?;

        assert_eq!(value, json!([]));
        assert_eq!(
            harness.call_methods(),
            vec!["get_blocks", "get_cryptarchia_info"]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_recent_range_walks_when_upper_bound_contains_tip() -> Result<()> {
        let harness = Arc::new(FarTipTransport {
            calls: Mutex::new(Vec::new()),
        });
        let transport: SharedModuleTransport = harness.clone();

        let value =
            blockchain_recent_blocks(&transport, ModuleTransportKind::LogoscoreCli, 100, 700, 2)
                .await?;
        let blocks = value
            .as_array()
            .context("wide CLI tip-parent range did not return an array")?;

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].pointer("/header/slot"), Some(&json!(700)));
        assert_eq!(blocks[1].pointer("/header/slot"), Some(&json!(690)));
        assert_eq!(
            harness.call_methods(),
            vec![
                "get_blocks",
                "get_cryptarchia_info",
                "get_block",
                "get_block"
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_finalized_block_range_uses_module_api_without_parent_walk() -> Result<()> {
        let harness = Arc::new(TipParentTransport::new());
        let transport: SharedModuleTransport = harness.clone();

        let value = blockchain_finalized_blocks(
            &transport,
            ModuleTransportKind::LogoscoreCli,
            100,
            130,
            20,
        )
        .await?;

        assert_eq!(value.pointer("/0/header/id"), Some(&json!(test_hash('f'))));
        assert_eq!(value.pointer("/0/_chain/status"), Some(&json!("finalized")));
        assert_eq!(value.as_array().map(Vec::len), Some(1));
        assert_eq!(harness.call_methods(), vec!["get_finalized_blocks_range"]);
        Ok(())
    }

    #[tokio::test]
    async fn finalized_block_range_preserves_u64_slots() -> Result<()> {
        let harness = Arc::new(TipParentTransport::with_kind(ModuleTransportKind::Module));
        let transport: SharedModuleTransport = harness.clone();
        let first_slot = (i32::MAX as u64) + 1;

        let value = blockchain_finalized_blocks(
            &transport,
            ModuleTransportKind::Module,
            first_slot,
            first_slot + 30,
            20,
        )
        .await?;

        assert_eq!(
            value.pointer("/0/header/slot"),
            Some(&json!(first_slot + 29))
        );
        assert_eq!(harness.call_methods(), vec!["get_finalized_blocks_range"]);
        assert_eq!(
            harness.call_arguments(),
            vec![vec![
                json!(first_slot),
                json!(first_slot + 30),
                json!(20_u64)
            ]]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_recent_block_range_stops_tip_parent_walk_at_requested_limit() -> Result<()> {
        let harness = Arc::new(TipParentTransport::new());
        let transport: SharedModuleTransport = harness.clone();

        let value =
            blockchain_recent_blocks(&transport, ModuleTransportKind::LogoscoreCli, 100, 130, 2)
                .await?;
        let blocks = value
            .as_array()
            .context("CLI recent tip-parent range did not return an array")?;

        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].pointer("/header/slot"), Some(&json!(130)));
        assert_eq!(blocks[1].pointer("/header/slot"), Some(&json!(115)));
        assert_eq!(
            harness.call_methods(),
            vec![
                "get_blocks",
                "get_cryptarchia_info",
                "get_block",
                "get_block",
            ]
        );
        Ok(())
    }

    #[tokio::test]
    async fn cli_node_report_relays_all_bedrock_diagnostic_reads() -> Result<()> {
        let harness = Arc::new(TipParentTransport::new());
        let transport: SharedModuleTransport = harness.clone();

        let report = blockchain_node_report(&transport, ModuleTransportKind::LogoscoreCli).await;
        assert!(report.cryptarchia_info.ok);
        assert_eq!(
            report
                .cryptarchia_info
                .value
                .as_ref()
                .and_then(|value| value.pointer("/cryptarchia_info/slot")),
            Some(&json!(130))
        );
        assert_eq!(
            report
                .cryptarchia_info
                .value
                .as_ref()
                .and_then(|value| value.pointer("/cryptarchia_info/tip")),
            Some(&json!(test_hash('a')))
        );
        assert_eq!(
            report
                .cryptarchia_info
                .value
                .as_ref()
                .and_then(|value| value.pointer("/cryptarchia_info/genesis_id")),
            Some(&json!(test_hash('0')))
        );
        assert!(report.headers.ok);
        assert_eq!(
            report.headers.value.as_ref(),
            Some(&json!([{"slot": 130, "id": test_hash('a')}]))
        );
        assert!(report.network_info.ok);
        assert_eq!(
            report.network_info.value.as_ref(),
            Some(&json!({"peers": 3}))
        );
        assert!(report.mantle_metrics.ok);
        assert_eq!(
            report.mantle_metrics.value.as_ref(),
            Some(&json!({"transactions": 1}))
        );

        let methods = harness.call_methods();
        assert_eq!(methods.len(), 4);
        for method in [
            "get_cryptarchia_info",
            "get_cryptarchia_headers",
            "get_network_info",
            "get_mantle_metrics",
        ] {
            assert!(methods.iter().any(|called| called == method));
        }
        Ok(())
    }

    #[tokio::test]
    async fn cli_node_report_serializes_diagnostic_reads_for_cli_gate() -> Result<()> {
        let harness = Arc::new(PeakConcurrentCliTransport::new());
        let transport: SharedModuleTransport = harness.clone();

        let report = blockchain_node_report(&transport, ModuleTransportKind::LogoscoreCli).await;

        assert!(report.cryptarchia_info.ok);
        assert!(report.headers.ok);
        assert!(report.network_info.ok);
        assert!(report.mantle_metrics.ok);
        assert_eq!(harness.peak_calls(), 1);
        Ok(())
    }
}
