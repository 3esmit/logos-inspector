use anyhow::{Result, bail};
use serde_json::Value;

use crate::{
    blockchain,
    source_routing::{self, Args, CoreEndpointMode},
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationDefinition, OperationDomain, OperationMethod};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationMethod::BlockchainNode,
        "blockchainNode",
        OperationDomain::Blockchain,
        "Blockchain node",
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainBlocks,
        "blockchainBlocks",
        OperationDomain::Blockchain,
        "Blockchain blocks",
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainLiveBlocks,
        "blockchainLiveBlocks",
        OperationDomain::Blockchain,
        "Blockchain live blocks",
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainBlock,
        "blockchainBlock",
        OperationDomain::Blockchain,
        "Blockchain block",
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainTransaction,
        "blockchainTransaction",
        OperationDomain::Blockchain,
        "Blockchain transaction",
    ),
];

pub(super) async fn execute(request: &RuntimeOperationRequest) -> Result<Value> {
    match request.method() {
        OperationMethod::BlockchainNode => execute_blockchain_node(request).await,
        OperationMethod::BlockchainBlocks => execute_blockchain_blocks(request).await,
        OperationMethod::BlockchainLiveBlocks => execute_blockchain_live_blocks(request).await,
        OperationMethod::BlockchainBlock => execute_blockchain_block(request).await,
        OperationMethod::BlockchainTransaction => execute_blockchain_transaction(request).await,
        _ => bail!("`{}` is not a Blockchain operation", request.method_name()),
    }
}

async fn execute_blockchain_node(request: &RuntimeOperationRequest) -> Result<Value> {
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

async fn execute_blockchain_blocks(request: &RuntimeOperationRequest) -> Result<Value> {
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

async fn execute_blockchain_live_blocks(request: &RuntimeOperationRequest) -> Result<Value> {
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

async fn execute_blockchain_block(request: &RuntimeOperationRequest) -> Result<Value> {
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

async fn execute_blockchain_transaction(request: &RuntimeOperationRequest) -> Result<Value> {
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
