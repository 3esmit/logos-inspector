use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::{
    indexer_block_by_hash, indexer_blocks, indexer_health, indexer_status,
    indexer_transfer_recipients, last_sequencer_block_id,
    lez::LezTargetResolver,
    local_wallet_deploy_program, local_wallet_instruction_submit, raw_json_rpc_optional_result,
    sequencer_block, sequencer_blocks, sequencer_health, sequencer_program_ids,
    sequencer_transaction,
    source_routing::{self, CoreEndpointMode, SourceEndpoint},
    support::args::Args,
    support::confirmation::ConfirmationPolicy,
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationDefinition, OperationDomain, OperationMethod};
use super::wallet_args::{confirmed_wallet_args, wallet_profile_arg};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationMethod::Health,
        "health",
        OperationDomain::Execution,
        "Execution health",
    ),
    OperationDefinition::new(
        OperationMethod::Head,
        "head",
        OperationDomain::Execution,
        "Execution head",
    ),
    OperationDefinition::new(
        OperationMethod::Programs,
        "programs",
        OperationDomain::Execution,
        "Programs",
    ),
    OperationDefinition::new(
        OperationMethod::Block,
        "block",
        OperationDomain::Execution,
        "Sequencer block",
    ),
    OperationDefinition::new(
        OperationMethod::SequencerBlocks,
        "sequencerBlocks",
        OperationDomain::Execution,
        "Sequencer blocks",
    ),
    OperationDefinition::new(
        OperationMethod::Transaction,
        "transaction",
        OperationDomain::Execution,
        "Transaction",
    ),
    OperationDefinition::new(
        OperationMethod::InspectTransaction,
        "inspectTransaction",
        OperationDomain::Execution,
        "Transaction inspection",
    ),
    OperationDefinition::new(
        OperationMethod::TraceTransaction,
        "traceTransaction",
        OperationDomain::Execution,
        "Transaction trace",
    ),
    OperationDefinition::new(
        OperationMethod::Account,
        "account",
        OperationDomain::Execution,
        "Account inspection",
    ),
    OperationDefinition::new(
        OperationMethod::ResolveLezTarget,
        "resolveLezTarget",
        OperationDomain::Execution,
        "LEZ lookup",
    ),
    OperationDefinition::new(
        OperationMethod::LocalWalletDeployProgram,
        "localWalletDeployProgram",
        OperationDomain::Execution,
        "Program deploy",
    ),
    OperationDefinition::new(
        OperationMethod::LocalWalletInstructionSubmit,
        "localWalletInstructionSubmit",
        OperationDomain::Execution,
        "IDL instruction",
    ),
    OperationDefinition::new(
        OperationMethod::IndexerHealth,
        "indexerHealth",
        OperationDomain::Indexer,
        "Indexer health",
    ),
    OperationDefinition::new(
        OperationMethod::IndexerStatus,
        "indexerStatus",
        OperationDomain::Indexer,
        "Indexer status",
    ),
    OperationDefinition::new(
        OperationMethod::IndexerFinalizedHead,
        "indexerFinalizedHead",
        OperationDomain::Indexer,
        "Indexer finalized head",
    ),
    OperationDefinition::new(
        OperationMethod::IndexerBlocks,
        "indexerBlocks",
        OperationDomain::Indexer,
        "Indexer blocks",
    ),
    OperationDefinition::new(
        OperationMethod::IndexerBlockByHash,
        "indexerBlockByHash",
        OperationDomain::Indexer,
        "Indexer block",
    ),
    OperationDefinition::new(
        OperationMethod::IndexerTransferRecipients,
        "indexerTransferRecipients",
        OperationDomain::Indexer,
        "Indexer transfer recipients",
    ),
];

pub(super) async fn execute(request: &RuntimeOperationRequest) -> Result<Value> {
    match request.method() {
        OperationMethod::Health => execute_execution_health(request).await,
        OperationMethod::Head => execute_execution_head(request).await,
        OperationMethod::Programs => execute_programs(request).await,
        OperationMethod::Block => execute_sequencer_block(request).await,
        OperationMethod::SequencerBlocks => execute_sequencer_blocks(request).await,
        OperationMethod::Transaction => execute_sequencer_transaction(request).await,
        OperationMethod::InspectTransaction => execute_inspect_transaction(request).await,
        OperationMethod::TraceTransaction => execute_trace_transaction(request).await,
        OperationMethod::Account => execute_account_operation(request).await,
        OperationMethod::ResolveLezTarget => execute_resolve_lez_target(request).await,
        OperationMethod::LocalWalletDeployProgram => execute_program_deployment(request).await,
        OperationMethod::LocalWalletInstructionSubmit => {
            execute_instruction_submission(request).await
        }
        OperationMethod::IndexerHealth => execute_indexer_health_operation(request).await,
        OperationMethod::IndexerStatus => execute_indexer_status_operation(request).await,
        OperationMethod::IndexerFinalizedHead => execute_indexer_finalized_head(request).await,
        OperationMethod::IndexerBlocks => execute_indexer_blocks_operation(request).await,
        OperationMethod::IndexerBlockByHash => {
            execute_indexer_block_by_hash_operation(request).await
        }
        OperationMethod::IndexerTransferRecipients => {
            execute_indexer_transfer_recipients_operation(request).await
        }
        _ => bail!(
            "`{}` is not a LEZ or indexer operation",
            request.method_name()
        ),
    }
}

pub(super) async fn execute_execution_health(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "health")?;
    sequencer_health(source.endpoint).await?;
    Ok(json!("ok"))
}

pub(super) async fn execute_execution_head(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "head")?;
    to_value(last_sequencer_block_id(source.endpoint).await?)
}

pub(super) async fn execute_programs(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "programs")?;
    to_value(sequencer_program_ids(source.endpoint).await?)
}

pub(super) async fn execute_sequencer_block(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "block")?;
    to_value(sequencer_block(source.endpoint, args.u64(source.next_index, "block id")?).await?)
}

pub(super) async fn execute_sequencer_blocks(request: &RuntimeOperationRequest) -> Result<Value> {
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

pub(super) async fn execute_sequencer_transaction(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
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

pub(super) async fn execute_inspect_transaction(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    let next_index = source.next_index;
    let hash = args.string(next_index, "transaction hash")?;
    let idl = args.optional_string(next_index + 1);
    let resolver = LezTargetResolver::from_execution_source(source);
    to_value(resolver.inspect_transaction(hash, idl).await?)
}

pub(super) async fn execute_trace_transaction(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    let next_index = source.next_index;
    let hash = args.string(next_index, "transaction hash")?;
    let idl = args.optional_string(next_index + 1);
    let resolver = LezTargetResolver::from_execution_source(source);
    to_value(resolver.trace_transaction(hash, idl).await?)
}

pub(super) async fn execute_account_operation(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let account_args = args.account_sources()?;
    let account = account_args.account;
    let idl = args.optional_string(account_args.next_index);
    let account_type = args.optional_string(account_args.next_index + 1);
    let resolver = LezTargetResolver::from_account_sources(account_args);
    resolver.inspect_account(account, idl, account_type).await
}

pub(super) async fn execute_resolve_lez_target(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let sources = args.account_sources()?;
    let target = sources.account;
    let session = LezTargetResolver::from_account_sources(sources);
    to_value(session.resolve_target(target).await?)
}

pub(super) async fn execute_program_deployment(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletDeployProgram)?;
    let profile = wallet_profile_arg(&args)?;
    let program_path = args.string(1, "program path")?.to_owned();
    blocking_value("program deployment", move || {
        to_value(local_wallet_deploy_program(profile, &program_path)?)
    })
    .await
}

pub(super) async fn execute_instruction_submission(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletInstructionSubmit)?;
    to_value(
        local_wallet_instruction_submit(
            wallet_profile_arg(&args)?,
            args.value(1)
                .cloned()
                .context("IDL instruction request is required")?,
        )
        .await?,
    )
}

pub(super) async fn execute_indexer_health_operation(
    request: &RuntimeOperationRequest,
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
    request: &RuntimeOperationRequest,
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
    request: &RuntimeOperationRequest,
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
    request: &RuntimeOperationRequest,
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
    request: &RuntimeOperationRequest,
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
    request: &RuntimeOperationRequest,
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
