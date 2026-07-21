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
            .await
            .map(crate::blockchain::normalize_cryptarchia_info),
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
    Ok(
        blockchain_blocks_read(transport, transport_kind, slot_from, slot_to, None)
            .await?
            .value,
    )
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
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport,
    };

    struct TipParentTransport {
        calls: Mutex<Vec<ModuleCall>>,
    }

    impl TipParentTransport {
        const fn new() -> Self {
            Self {
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
    }

    impl ModuleTransport for TipParentTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::LogoscoreCli
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
            } else if method == "get_cryptarchia_info" && args.is_empty() {
                Ok(json!({
                    "genesis_id": test_hash('0'),
                    "tip": test_hash('a'),
                    "slot": 130,
                }))
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
    async fn cli_node_report_uses_the_canonical_cryptarchia_shape() -> Result<()> {
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
        assert_eq!(harness.call_methods(), vec!["get_cryptarchia_info"]);
        Ok(())
    }
}
