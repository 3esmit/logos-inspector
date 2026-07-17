use anyhow::Result;
use serde_json::{Map, Value, json};

use crate::{
    modules::logos_core::SharedModuleTransport, source_routing::bedrock_layer, support::args::Args,
    support::entity_id::normalize_block_id_text,
};

use super::super::value::to_value;
use super::RuntimeOperationRequest;
use super::spec::{
    AffectedContextField, AffectedContextKey, OperationClass, OperationCommand,
    OperationDefinition, OperationMethod,
};

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
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
    ]),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::Blocks),
        "blockchainBlocks",
        "Blockchain blocks",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::SlotRange),
    ]),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::LiveBlocks),
        "blockchainLiveBlocks",
        "Blockchain live blocks",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::SlotRange),
    ]),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::Block),
        "blockchainBlock",
        "Blockchain block",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::BlockId),
    ]),
    OperationDefinition::new(
        OperationCommand::Blockchain(BlockchainCommand::Transaction),
        "blockchainTransaction",
        "Blockchain transaction",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::TransactionId),
    ]),
];

pub(super) fn validate(
    command: BlockchainCommand,
    request: &RuntimeOperationRequest,
) -> Result<()> {
    operation_context(command, request).map(|_| ())
}

pub(super) fn add_operation_context(
    command: BlockchainCommand,
    request: &RuntimeOperationRequest,
    context: &mut Map<String, Value>,
) -> Result<()> {
    context.extend(operation_context(command, request)?);
    Ok(())
}

fn operation_context(
    command: BlockchainCommand,
    request: &RuntimeOperationRequest,
) -> Result<Map<String, Value>> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let mut context = Map::new();
    context.insert("source".to_owned(), json!(source.mode.as_str()));
    if !source.endpoint.is_empty() {
        context.insert("endpoint".to_owned(), json!(source.endpoint));
    }
    match command {
        BlockchainCommand::Node => {}
        BlockchainCommand::Blocks => {
            let slot_from = args.canonical_decimal_u64(source.next_index, "slot from")?;
            let slot_to = args.canonical_decimal_u64(source.next_index + 1, "slot to")?;
            crate::blockchain::validate_blockchain_slot_range(slot_from, slot_to)?;
            context.insert("slotFrom".to_owned(), json!(slot_from));
            context.insert("slotTo".to_owned(), json!(slot_to));
            context.insert(
                "slotRange".to_owned(),
                json!(format!("{slot_from}:{slot_to}")),
            );
            if let Some(limit) = args.value(source.next_index + 2).and_then(Value::as_u64) {
                context.insert("limit".to_owned(), json!(limit));
            }
        }
        BlockchainCommand::LiveBlocks => {
            let slot_from = args.canonical_decimal_u64(source.next_index, "slot from")?;
            let slot_to = args.canonical_decimal_u64(source.next_index + 1, "slot to")?;
            crate::blockchain::validate_blockchain_slot_range(slot_from, slot_to)?;
            context.insert("slotFrom".to_owned(), json!(slot_from));
            context.insert("slotTo".to_owned(), json!(slot_to));
            context.insert(
                "slotRange".to_owned(),
                json!(format!("{slot_from}:{slot_to}")),
            );
            context.insert(
                "limit".to_owned(),
                json!(
                    args.value(source.next_index + 2)
                        .and_then(Value::as_u64)
                        .unwrap_or(50)
                ),
            );
        }
        BlockchainCommand::Block => {
            let block_id = normalize_block_id_text(args.string(source.next_index, "block id")?)?;
            context.insert("blockId".to_owned(), json!(block_id));
        }
        BlockchainCommand::Transaction => {
            context.insert(
                "transactionId".to_owned(),
                json!(args.string(source.next_index, "transaction id")?),
            );
        }
    }
    Ok(context)
}

pub(super) async fn execute(
    command: BlockchainCommand,
    request: &RuntimeOperationRequest,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    match command {
        BlockchainCommand::Node => execute_blockchain_node(request, &module_transport).await,
        BlockchainCommand::Blocks => execute_blockchain_blocks(request, &module_transport).await,
        BlockchainCommand::LiveBlocks => {
            execute_blockchain_live_blocks(request, &module_transport).await
        }
        BlockchainCommand::Block => execute_blockchain_block(request, &module_transport).await,
        BlockchainCommand::Transaction => {
            execute_blockchain_transaction(request, &module_transport).await
        }
    }
}

async fn execute_blockchain_node(
    request: &RuntimeOperationRequest,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(bedrock_layer::node_report(source.adapter(), module_transport).await?)
}

async fn execute_blockchain_blocks(
    request: &RuntimeOperationRequest,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let slot_from = args.canonical_decimal_u64(source.next_index, "slot from")?;
    let slot_to = args.canonical_decimal_u64(source.next_index + 1, "slot to")?;
    let limit = args.value(source.next_index + 2).and_then(Value::as_u64);
    to_value(
        bedrock_layer::blocks(
            source.adapter(),
            slot_from,
            slot_to,
            limit,
            module_transport,
        )
        .await?,
    )
}

async fn execute_blockchain_live_blocks(
    request: &RuntimeOperationRequest,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let slot_from = args.canonical_decimal_u64(source.next_index, "slot from")?;
    let slot_to = args.canonical_decimal_u64(source.next_index + 1, "slot to")?;
    let limit = args
        .value(source.next_index + 2)
        .and_then(Value::as_u64)
        .unwrap_or(50);
    to_value(
        bedrock_layer::live_blocks(
            source.adapter(),
            slot_from,
            slot_to,
            limit,
            module_transport,
        )
        .await?,
    )
}

async fn execute_blockchain_block(
    request: &RuntimeOperationRequest,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(
        bedrock_layer::block(
            source.adapter(),
            args.string(source.next_index, "block id")?,
            module_transport,
        )
        .await?,
    )
}

async fn execute_blockchain_transaction(
    request: &RuntimeOperationRequest,
    module_transport: &SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(
        bedrock_layer::transaction(
            source.adapter(),
            args.string(source.next_index, "transaction id")?,
            module_transport,
        )
        .await?,
    )
}
