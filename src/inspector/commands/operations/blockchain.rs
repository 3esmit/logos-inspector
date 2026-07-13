use anyhow::Result;
use serde_json::Value;

use crate::{source_routing::bedrock_layer, support::args::Args};

use super::super::value::to_value;
use super::RuntimeOperationRequest;
use super::spec::{OperationClass, OperationCommand, OperationDefinition, OperationMethod};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BlockchainCommand {
    Node,
    Blocks,
    LiveBlocks,
    Block,
    Transaction,
}

impl BlockchainCommand {
    pub(super) const fn method(self) -> OperationMethod {
        match self {
            Self::Node => OperationMethod::BlockchainNode,
            Self::Blocks => OperationMethod::BlockchainBlocks,
            Self::LiveBlocks => OperationMethod::BlockchainLiveBlocks,
            Self::Block => OperationMethod::BlockchainBlock,
            Self::Transaction => OperationMethod::BlockchainTransaction,
        }
    }
}

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::Node),
        "blockchainNode",
        "Blockchain node",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::Blocks),
        "blockchainBlocks",
        "Blockchain blocks",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::LiveBlocks),
        "blockchainLiveBlocks",
        "Blockchain live blocks",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::Block),
        "blockchainBlock",
        "Blockchain block",
        OperationClass::ReadPoll,
    ),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::Transaction),
        "blockchainTransaction",
        "Blockchain transaction",
        OperationClass::ReadPoll,
    ),
];

pub(super) async fn execute(
    command: BlockchainCommand,
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    match command {
        BlockchainCommand::Node => execute_blockchain_node(request).await,
        BlockchainCommand::Blocks => execute_blockchain_blocks(request).await,
        BlockchainCommand::LiveBlocks => execute_blockchain_live_blocks(request).await,
        BlockchainCommand::Block => execute_blockchain_block(request).await,
        BlockchainCommand::Transaction => execute_blockchain_transaction(request).await,
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
