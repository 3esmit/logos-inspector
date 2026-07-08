use anyhow::{Result, bail};
use serde_json::{Value, json};

use crate::{
    account_lookup, account_lookup_with_idl, blockchain, indexer_block_by_hash, indexer_blocks,
    indexer_health, indexer_status, indexer_transfer_recipients, last_sequencer_block_id,
    lez::{LezInspectionSession, RegisteredIdlResolver},
    raw_json_rpc_optional_result, sequencer_account, sequencer_block, sequencer_blocks,
    sequencer_program_ids, sequencer_transaction, sequencer_transaction_inspection,
    sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
    sequencer_transaction_trace_with_idl,
    source_routing::{self, Args, CoreEndpointMode, SourceEndpoint},
    support::state_store::registered_idl_entries,
};

use super::super::value::{blocking_value, to_value};
use super::NodeOperationRequest;

const EXECUTION_MODULE: &str = source_routing::LEZ_CORE_MODULE;

pub(super) async fn execute_blockchain_node(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    if source.mode == CoreEndpointMode::Module {
        return blocking_value("blockchain module node", move || {
            to_value(source_routing::blockchain_node_report())
        })
        .await;
    }
    to_value(blockchain::blockchain_node_report(source.endpoint).await)
}

pub(super) async fn execute_blockchain_blocks(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let slot_from = args.u64(source.next_index, "slot from")?;
    let slot_to = args.u64(source.next_index + 1, "slot to")?;
    if source.mode == CoreEndpointMode::Module {
        let limit = args.value(source.next_index + 2).and_then(Value::as_u64);
        return blocking_value("blockchain module blocks", move || {
            if let Some(limit) = limit {
                to_value(source_routing::blockchain_recent_blocks(
                    slot_from, slot_to, limit,
                )?)
            } else {
                to_value(source_routing::blockchain_blocks(slot_from, slot_to)?)
            }
        })
        .await;
    }
    if let Some(limit) = args.value(source.next_index + 2).and_then(Value::as_u64) {
        to_value(
            blockchain::blockchain_recent_blocks(source.endpoint, slot_from, slot_to, limit)
                .await?,
        )
    } else {
        to_value(blockchain::blockchain_blocks(source.endpoint, slot_from, slot_to).await?)
    }
}

pub(super) async fn execute_blockchain_live_blocks(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let slot_from = args.u64(source.next_index, "slot from")?;
    let slot_to = args.u64(source.next_index + 1, "slot to")?;
    let limit = args
        .value(source.next_index + 2)
        .and_then(Value::as_u64)
        .unwrap_or(50);
    if source.mode == CoreEndpointMode::Module {
        return blocking_value("blockchain module live blocks", move || {
            to_value(source_routing::blockchain_live_blocks_snapshot(
                slot_from, slot_to, limit,
            )?)
        })
        .await;
    }
    to_value(
        blockchain::blockchain_live_blocks_snapshot(source.endpoint, slot_from, slot_to, limit)
            .await?,
    )
}

pub(super) async fn execute_blockchain_block(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    if source.mode == CoreEndpointMode::Module {
        let block_id = args.string(source.next_index, "block id")?.to_owned();
        return blocking_value("blockchain module block", move || {
            to_value(source_routing::blockchain_block(&block_id)?)
        })
        .await;
    }
    to_value(
        blockchain::blockchain_block(source.endpoint, args.string(source.next_index, "block id")?)
            .await?,
    )
}

pub(super) async fn execute_blockchain_transaction(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    if source.mode == CoreEndpointMode::Module {
        let transaction_id = args.string(source.next_index, "transaction id")?.to_owned();
        return blocking_value("blockchain module transaction", move || {
            to_value(source_routing::blockchain_transaction(&transaction_id)?)
        })
        .await;
    }
    to_value(
        blockchain::blockchain_transaction(
            source.endpoint,
            args.string(source.next_index, "transaction id")?,
        )
        .await?,
    )
}

pub(super) async fn execute_execution_head(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "head")?;
    to_value(last_sequencer_block_id(source.endpoint).await?)
}

pub(super) async fn execute_programs(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "programs")?;
    to_value(sequencer_program_ids(source.endpoint).await?)
}

pub(super) async fn execute_sequencer_block(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "block")?;
    to_value(sequencer_block(source.endpoint, args.u64(source.next_index, "block id")?).await?)
}

pub(super) async fn execute_sequencer_blocks(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "sequencerBlocks")?;
    let before = args.value(source.next_index).and_then(Value::as_u64);
    let limit = args
        .value(source.next_index + 1)
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .min(50);
    to_value(sequencer_blocks(source.endpoint, before, limit).await?)
}

pub(super) async fn execute_sequencer_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "transaction")?;
    to_value(
        sequencer_transaction(
            source.endpoint,
            args.string(source.next_index, "transaction hash")?,
        )
        .await?,
    )
}

pub(super) async fn execute_inspect_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "inspectTransaction")?;
    let endpoint = source.endpoint;
    let hash = args.string(source.next_index, "transaction hash")?;
    let idl = args.optional_string(source.next_index + 1);
    if let Some(idl) = idl {
        return to_value(sequencer_transaction_inspection_with_idl(endpoint, hash, idl).await?);
    }
    let inspection = sequencer_transaction_inspection(endpoint, hash).await?;
    let Some(inspection) = inspection else {
        return Ok(Value::Null);
    };
    let idl_entries = registered_idl_entries()?;
    if let Some(report) =
        RegisteredIdlResolver::new(&idl_entries).transaction_inspection(&inspection.raw_summary)
    {
        return to_value(Some(report));
    }
    to_value(Some(inspection))
}

pub(super) async fn execute_trace_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "traceTransaction")?;
    let endpoint = source.endpoint;
    let hash = args.string(source.next_index, "transaction hash")?;
    if let Some(idl) = args.optional_string(source.next_index + 1) {
        to_value(sequencer_transaction_trace_with_idl(endpoint, hash, idl).await?)
    } else {
        let trace = sequencer_transaction_trace(endpoint, hash).await?;
        let Some(trace) = trace else {
            return Ok(Value::Null);
        };
        let idl_entries = registered_idl_entries()?;
        if let Some(report) = RegisteredIdlResolver::new(&idl_entries)
            .transaction_trace(&trace.inspection.raw_summary)
        {
            return to_value(Some(report));
        }
        to_value(Some(trace))
    }
}

pub(super) async fn execute_account_operation(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let account_args = args.account_sources()?;
    if account_args.execution_mode == CoreEndpointMode::Module {
        bail!(
            "{EXECUTION_MODULE} does not expose Inspector account reads; use sequencer RPC for account inspection"
        );
    }
    let idl = args
        .optional_string(account_args.next_index)
        .map(ToOwned::to_owned);
    let account_type = args
        .optional_string(account_args.next_index + 1)
        .map(ToOwned::to_owned);
    let mut value = if account_args.indexer_mode == CoreEndpointMode::Module {
        let mut account =
            sequencer_account(account_args.sequencer_endpoint, account_args.account).await?;
        blocking_value("indexer module account transactions", move || {
            source_routing::attach_module_account_transactions(&mut account);
            if let Some(idl) = idl.as_deref() {
                to_value(crate::lez::account_report_with_optional_idl_decode(
                    account,
                    idl,
                    account_type.as_deref(),
                ))
            } else {
                to_value(account)
            }
        })
        .await?
    } else if let Some(idl) = idl.as_deref() {
        to_value(
            account_lookup_with_idl(
                account_args.sequencer_endpoint,
                account_args.indexer_endpoint,
                account_args.account,
                idl,
                account_type.as_deref(),
            )
            .await?,
        )?
    } else {
        to_value(
            account_lookup(
                account_args.sequencer_endpoint,
                account_args.indexer_endpoint,
                account_args.account,
            )
            .await?,
        )?
    };
    let idl_entries = registered_idl_entries()?;
    RegisteredIdlResolver::new(&idl_entries)
        .enrich_account_related_transaction_decodes(&mut value)?;
    Ok(value)
}

pub(super) async fn execute_resolve_lez_target(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let sources = args.account_sources()?;
    let target = sources.account;
    let session = LezInspectionSession::new(sources);
    to_value(session.resolve_target(target).await?)
}

pub(super) async fn execute_indexer_health_operation(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    if source.mode == CoreEndpointMode::Module {
        return blocking_value("indexer module health", move || {
            to_value(source_routing::indexer_health()?)
        })
        .await;
    }
    let health = indexer_health(source.endpoint).await?;
    Ok(json!({
        "status": "healthy",
        "health": health,
    }))
}

pub(super) async fn execute_indexer_status_operation(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    if source.mode == CoreEndpointMode::Module {
        return blocking_value("indexer module status", move || {
            to_value(source_routing::indexer_status()?)
        })
        .await;
    }
    to_value(indexer_status(source.endpoint).await?)
}

pub(super) async fn execute_indexer_finalized_head(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    if source.mode == CoreEndpointMode::Module {
        return blocking_value("indexer module finalized head", move || {
            to_value(source_routing::indexer_finalized_head()?)
        })
        .await;
    }
    to_value(
        raw_json_rpc_optional_result(
            source.endpoint,
            "getLastFinalizedBlockId",
            Value::Array(vec![]),
        )
        .await?,
    )
}

pub(super) async fn execute_indexer_blocks_operation(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    let before = args.value(source.next_index).and_then(Value::as_u64);
    let limit = args
        .value(source.next_index + 1)
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .min(50);
    if source.mode == CoreEndpointMode::Module {
        return blocking_value("indexer module blocks", move || {
            to_value(source_routing::indexer_blocks(before, limit)?)
        })
        .await;
    }
    to_value(indexer_blocks(source.endpoint, before, limit).await?)
}

pub(super) async fn execute_indexer_block_by_hash_operation(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    if source.mode == CoreEndpointMode::Module {
        let header_hash = args
            .string(source.next_index, "block header hash")?
            .to_owned();
        return blocking_value("indexer module block by hash", move || {
            to_value(source_routing::indexer_block_by_hash(&header_hash)?)
        })
        .await;
    }
    to_value(
        indexer_block_by_hash(
            source.endpoint,
            args.string(source.next_index, "block header hash")?,
        )
        .await?,
    )
}

pub(super) async fn execute_indexer_transfer_recipients_operation(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    let before = args.value(source.next_index).and_then(Value::as_u64);
    let limit = args
        .value(source.next_index + 1)
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .min(50);
    if source.mode == CoreEndpointMode::Module {
        return blocking_value("indexer module transfer recipients", move || {
            to_value(source_routing::indexer_transfer_recipients(before, limit)?)
        })
        .await;
    }
    to_value(indexer_transfer_recipients(source.endpoint, before, limit).await?)
}

fn require_rpc_operation_source(source: &SourceEndpoint<'_>, method: &str) -> Result<()> {
    if source.mode == CoreEndpointMode::Rpc {
        return Ok(());
    }
    bail!(
        "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
        source.module
    )
}
