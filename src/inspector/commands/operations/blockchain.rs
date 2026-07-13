use anyhow::{Result, bail};
use serde_json::Value;

use crate::{source_routing::bedrock_layer, support::args::Args};

use super::super::value::to_value;
use super::RuntimeOperationRequest;
use super::spec::{OperationClass, OperationDefinition, OperationDomain, OperationMethod};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationMethod::BlockchainNode,
        "blockchainNode",
        OperationDomain::Blockchain,
        "Blockchain node",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainBlocks,
        "blockchainBlocks",
        OperationDomain::Blockchain,
        "Blockchain blocks",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainLiveBlocks,
        "blockchainLiveBlocks",
        OperationDomain::Blockchain,
        "Blockchain live blocks",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainBlock,
        "blockchainBlock",
        OperationDomain::Blockchain,
        "Blockchain block",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationMethod::BlockchainTransaction,
        "blockchainTransaction",
        OperationDomain::Blockchain,
        "Blockchain transaction",
        OperationClass::ReadPoll,
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
    to_value(bedrock_layer::node_report(source.adapter()).await?)
}

async fn execute_blockchain_blocks(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let slot_from = args.u64(source.next_index, "slot from")?;
    let slot_to = args.u64(source.next_index + 1, "slot to")?;
    let limit = args.value(source.next_index + 2).and_then(Value::as_u64);
    to_value(bedrock_layer::blocks(source.adapter(), slot_from, slot_to, limit).await?)
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
    to_value(bedrock_layer::live_blocks(source.adapter(), slot_from, slot_to, limit).await?)
}

async fn execute_blockchain_block(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(
        bedrock_layer::block(
            source.adapter(),
            args.string(source.next_index, "block id")?,
        )
        .await?,
    )
}

async fn execute_blockchain_transaction(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(
        bedrock_layer::transaction(
            source.adapter(),
            args.string(source.next_index, "transaction id")?,
        )
        .await?,
    )
}
