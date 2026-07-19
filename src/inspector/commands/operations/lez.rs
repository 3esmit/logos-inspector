use std::sync::Arc;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use tokio::runtime::Runtime;

use crate::{
    inspection::{
        NetworkScope, ZoneSourceRole,
        l2::{ActiveZoneContext, L2SourceDescriptor, ResolvedActiveZoneContext},
    },
    inspector::commands::zone_catalog::ZoneCatalogCommandInterface,
    source_routing::channel_sources::ChannelSourceTarget,
    support::{command_runner::CommandControl, confirmation::ConfirmationPolicy},
    wallet,
};

use super::super::value::{blocking_value, to_value};
use super::dispatch::{interruptible_remote, normalize_command_execution};
use super::spec::{
    AffectedContextField, AffectedContextKey, OperationClass, OperationCommand,
    OperationDefinition, OperationMethod,
};
use super::supervisor::{OperationControl, TerminationEvidence};
use super::wallet_args::{confirmed_wallet_args, wallet_profile_arg};
use super::{InstructionTargetResolver, RuntimeOperationRequest};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ExecutionCommand {
    DeployProgram,
    SubmitInstruction,
}

impl ExecutionCommand {
    pub(super) const fn method(self) -> OperationMethod {
        match self {
            Self::DeployProgram => OperationMethod::LocalWalletDeployProgram,
            Self::SubmitInstruction => OperationMethod::LocalWalletInstructionSubmit,
        }
    }
}

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationCommand::Execution(ExecutionCommand::DeployProgram),
        "localWalletDeployProgram",
        "Program deploy",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationCommand::Execution(ExecutionCommand::SubmitInstruction),
        "localWalletInstructionSubmit",
        "IDL instruction",
        OperationClass::SigningSubmission,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::required(AffectedContextKey::Endpoint),
    ]),
];

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
struct InstructionTargetRequest {
    context: ActiveZoneContext,
    request_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct BoundInstructionTarget {
    network_scope: NetworkScope,
    channel_id: String,
    source_id: String,
    source_config_revision: u64,
    context_revision: u64,
    request_revision: u64,
    endpoint: String,
}

pub(super) struct ZoneCatalogInstructionTargetResolver {
    catalog: Arc<ZoneCatalogCommandInterface>,
}

impl ZoneCatalogInstructionTargetResolver {
    pub(super) fn new(catalog: Arc<ZoneCatalogCommandInterface>) -> Self {
        Self { catalog }
    }
}

impl InstructionTargetResolver for ZoneCatalogInstructionTargetResolver {
    fn resolve(
        &self,
        runtime: &Runtime,
        context: &ActiveZoneContext,
        request_revision: u64,
    ) -> Result<BoundInstructionTarget> {
        let facts = self
            .catalog
            .context_snapshot(runtime)
            .map_err(|_| anyhow::anyhow!("Active Zone state could not be verified"))?;
        let resolved = ResolvedActiveZoneContext::resolve(&facts, context, request_revision)
            .map_err(|error| anyhow::anyhow!(error.message))?;
        let source = resolved
            .selected_source(ZoneSourceRole::Sequencer)
            .map_err(|error| anyhow::anyhow!(error.message))?
            .context("Active Zone has no selected Sequencer source")?;
        bound_instruction_target_from_source(context, request_revision, source)
    }
}

fn bound_instruction_target_from_source(
    context: &ActiveZoneContext,
    request_revision: u64,
    source: L2SourceDescriptor,
) -> Result<BoundInstructionTarget> {
    if source.role != ZoneSourceRole::Sequencer {
        bail!("Selected source is not a Sequencer source");
    }
    let ChannelSourceTarget::Rpc { endpoint } = source.target else {
        bail!("Selected Sequencer source cannot submit wallet instructions");
    };
    if endpoint.trim().is_empty() {
        bail!("Selected Sequencer endpoint is unavailable");
    }
    Ok(BoundInstructionTarget {
        network_scope: context.network_scope.clone(),
        channel_id: context.channel_id.clone(),
        source_id: source.source_id,
        source_config_revision: source.source_config_revision,
        context_revision: context.context_revision,
        request_revision,
        endpoint,
    })
}

pub(super) fn bind_instruction_target(
    runtime: &Runtime,
    resolver: &dyn InstructionTargetResolver,
    request: &mut RuntimeOperationRequest,
) -> Result<()> {
    if request.command() != OperationCommand::Execution(ExecutionCommand::SubmitInstruction) {
        return Ok(());
    }
    let args = confirmed_wallet_args(request, 3, ConfirmationPolicy::WalletInstructionSubmit)?;
    if args.iter().count() != 4 {
        bail!("IDL instruction submission requires exactly four arguments");
    }
    let target: InstructionTargetRequest = serde_json::from_value(
        args.value(2)
            .cloned()
            .context("Active Zone target request is required")?,
    )
    .map_err(|_| anyhow::anyhow!("Active Zone target request is invalid"))?;
    let bound = resolver.resolve(runtime, &target.context, target.request_revision)?;
    let values = request
        .args
        .as_array_mut()
        .context("bridge args must be a JSON array")?;
    *values
        .get_mut(2)
        .context("Active Zone target request is required")? =
        serde_json::to_value(bound).context("failed to preserve verified Active Zone target")?;
    Ok(())
}

pub(super) fn add_operation_context(
    request: &RuntimeOperationRequest,
    context: &mut Map<String, Value>,
) -> Result<()> {
    if request.command() != OperationCommand::Execution(ExecutionCommand::SubmitInstruction) {
        return Ok(());
    }
    let target = bound_instruction_target(request)?;
    context.insert("source".to_owned(), json!(target.source_id));
    context.insert("endpoint".to_owned(), json!(target.endpoint));
    context.insert("channelId".to_owned(), json!(target.channel_id));
    context.insert(
        "networkScope".to_owned(),
        serde_json::to_value(target.network_scope)
            .context("failed to serialize instruction target network")?,
    );
    context.insert(
        "sourceConfigRevision".to_owned(),
        json!(target.source_config_revision),
    );
    context.insert("contextRevision".to_owned(), json!(target.context_revision));
    context.insert("requestRevision".to_owned(), json!(target.request_revision));
    Ok(())
}

pub(super) async fn execute(
    command: ExecutionCommand,
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    match command {
        ExecutionCommand::DeployProgram => execute_program_deployment(request, control).await,
        ExecutionCommand::SubmitInstruction => {
            execute_instruction_submission(request, control).await
        }
    }
}

async fn execute_program_deployment(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletDeployProgram)?;
    let profile = wallet_profile_arg(&args)?;
    let program_path = args.string(1, "program path")?.to_owned();
    let command_control = command_control(control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("program deployment", move || {
        let _worker_guard = worker_guard;
        to_value(wallet::local_wallet_deploy_program_controlled(
            profile,
            &program_path,
            command_control,
        )?)
    })
    .await;
    normalize_command_execution(
        result,
        control,
        TerminationEvidence::LocalOnly,
        TerminationEvidence::Confirmed,
    )
}

async fn execute_instruction_submission(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 3, ConfirmationPolicy::WalletInstructionSubmit)?;
    let target = bound_instruction_target(request)?;
    let submission = interruptible_remote(
        control,
        "wallet instruction submission stopped locally; remote transaction state is unknown",
        wallet::local_wallet_instruction_submit_to(
            wallet_profile_arg(&args)?,
            args.value(1)
                .cloned()
                .context("IDL instruction request is required")?,
            target.endpoint.clone(),
        ),
    )
    .await??;
    let mut report = to_value(submission)?;
    report
        .as_object_mut()
        .context("wallet instruction report must be an object")?
        .insert(
            "target".to_owned(),
            serde_json::to_value(target)
                .context("failed to serialize verified instruction target")?,
        );
    Ok(report)
}

fn bound_instruction_target(request: &RuntimeOperationRequest) -> Result<BoundInstructionTarget> {
    let args = confirmed_wallet_args(request, 3, ConfirmationPolicy::WalletInstructionSubmit)?;
    serde_json::from_value(
        args.value(2)
            .cloned()
            .context("verified Active Zone target is required")?,
    )
    .map_err(|_| anyhow::anyhow!("verified Active Zone target is invalid"))
}

fn command_control(control: &OperationControl) -> CommandControl {
    control.command_control()
}

#[cfg(test)]
mod tests {
    use std::{
        future::pending,
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use anyhow::{Context as _, Result, bail};
    use serde_json::json;

    use super::*;
    use crate::inspection::ZoneKind;
    use crate::inspector::commands::operations::supervisor::{
        OperationInterrupted, test_operation_control,
    };

    struct RecordingTargetResolver {
        calls: AtomicUsize,
        fail: bool,
    }

    impl RecordingTargetResolver {
        fn successful() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fail: false,
            }
        }

        fn failing() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                fail: true,
            }
        }
    }

    impl InstructionTargetResolver for RecordingTargetResolver {
        fn resolve(
            &self,
            _runtime: &Runtime,
            context: &ActiveZoneContext,
            request_revision: u64,
        ) -> Result<BoundInstructionTarget> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if self.fail {
                bail!("Active Zone source is stale");
            }
            Ok(BoundInstructionTarget {
                network_scope: context.network_scope.clone(),
                channel_id: context.channel_id.clone(),
                source_id: "src_verified".to_owned(),
                source_config_revision: context.source_config_revision,
                context_revision: context.context_revision,
                request_revision,
                endpoint: "https://verified-sequencer.example.test/".to_owned(),
            })
        }
    }

    fn active_zone_context() -> ActiveZoneContext {
        ActiveZoneContext {
            network_scope: NetworkScope::GenesisId {
                genesis_id: "11".repeat(32),
            },
            channel_id: "22".repeat(32),
            zone_kind: ZoneKind::SequencerZone,
            selected_sequencer_source_id: Some("src_requested".to_owned()),
            indexer_source_id: None,
            source_config_revision: 7,
            context_revision: 9,
        }
    }

    fn instruction_request(confirmation: Option<&str>) -> Result<RuntimeOperationRequest> {
        let mut args = vec![
            json!({ "wallet_home": "/wallet" }),
            json!({ "instruction": "transfer" }),
            json!({
                "context": active_zone_context(),
                "request_revision": 11
            }),
        ];
        if let Some(confirmation) = confirmation {
            args.push(json!(confirmation));
        }
        RuntimeOperationRequest::from_call(
            OperationMethod::LocalWalletInstructionSubmit,
            Value::Array(args),
            "IDL instruction",
        )
    }

    #[test]
    fn instruction_target_is_bound_before_operation_context_is_recorded() -> Result<()> {
        let runtime = Runtime::new()?;
        let resolver = RecordingTargetResolver::successful();
        let mut request =
            instruction_request(Some(ConfirmationPolicy::WalletInstructionSubmit.token()))?;

        bind_instruction_target(&runtime, &resolver, &mut request)?;

        if resolver.calls.load(Ordering::SeqCst) != 1 {
            bail!("instruction target resolver was not called exactly once");
        }
        let context = super::super::request::runtime_operation_context(&request)?;
        if context
            != json!({
                "source": "src_verified",
                "endpoint": "https://verified-sequencer.example.test/",
                "channelId": "22".repeat(32),
                "networkScope": {
                    "kind": "genesis_id",
                    "genesis_id": "11".repeat(32)
                },
                "sourceConfigRevision": 7,
                "contextRevision": 9,
                "requestRevision": 11
            })
        {
            bail!("operation context did not preserve verified target: {context}");
        }
        let bound = bound_instruction_target(&request)?;
        if bound.source_id != "src_verified"
            || bound.endpoint != "https://verified-sequencer.example.test/"
            || bound.request_revision != 11
        {
            bail!("verified instruction target was not frozen in operation args");
        }
        Ok(())
    }

    #[test]
    fn resolved_rpc_source_becomes_exact_bound_instruction_target() -> Result<()> {
        let context = active_zone_context();
        let target = bound_instruction_target_from_source(
            &context,
            11,
            L2SourceDescriptor {
                network_scope: context.network_scope.clone(),
                channel_id: context.channel_id.clone(),
                source_id: "src_resolved".to_owned(),
                role: ZoneSourceRole::Sequencer,
                target: ChannelSourceTarget::Rpc {
                    endpoint: "https://resolved.example.test/rpc".to_owned(),
                },
                source_config_revision: 7,
            },
        )?;

        if target.source_id != "src_resolved"
            || target.endpoint != "https://resolved.example.test/rpc"
            || target.channel_id != context.channel_id
            || target.source_config_revision != 7
            || target.context_revision != 9
            || target.request_revision != 11
        {
            bail!("resolved RPC source was not bound exactly");
        }
        Ok(())
    }

    #[test]
    fn resolved_module_source_cannot_enter_direct_wallet_submission() -> Result<()> {
        let context = active_zone_context();
        let error = bound_instruction_target_from_source(
            &context,
            11,
            L2SourceDescriptor {
                network_scope: context.network_scope.clone(),
                channel_id: context.channel_id.clone(),
                source_id: "src_module".to_owned(),
                role: ZoneSourceRole::Sequencer,
                target: ChannelSourceTarget::Module {
                    module_id: "sequencer_module".to_owned(),
                },
                source_config_revision: 7,
            },
        )
        .err()
        .context("module Sequencer target entered direct wallet submission")?;

        if error.to_string() != "Selected Sequencer source cannot submit wallet instructions" {
            bail!("unexpected module target rejection: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn missing_confirmation_is_rejected_before_target_resolution() -> Result<()> {
        let runtime = Runtime::new()?;
        let resolver = RecordingTargetResolver::successful();
        let mut request = instruction_request(None)?;

        let error = bind_instruction_target(&runtime, &resolver, &mut request)
            .err()
            .context("unconfirmed instruction target was accepted")?;

        if !error
            .to_string()
            .contains("IDL instruction send requires explicit confirmation")
            || resolver.calls.load(Ordering::SeqCst) != 0
        {
            bail!("confirmation was not enforced before target resolution: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn failed_target_resolution_does_not_rewrite_unverified_request() -> Result<()> {
        let runtime = Runtime::new()?;
        let resolver = RecordingTargetResolver::failing();
        let mut request =
            instruction_request(Some(ConfirmationPolicy::WalletInstructionSubmit.token()))?;
        let original = request.args.clone();

        let error = bind_instruction_target(&runtime, &resolver, &mut request)
            .err()
            .context("stale Active Zone target was accepted")?;

        if error.to_string() != "Active Zone source is stale"
            || request.args != original
            || resolver.calls.load(Ordering::SeqCst) != 1
        {
            bail!("failed target resolution mutated operation request: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn caller_supplied_endpoint_is_rejected_before_target_resolution() -> Result<()> {
        let runtime = Runtime::new()?;
        let resolver = RecordingTargetResolver::successful();
        let mut request =
            instruction_request(Some(ConfirmationPolicy::WalletInstructionSubmit.token()))?;
        request
            .args
            .as_array_mut()
            .and_then(|args| args.get_mut(2))
            .and_then(Value::as_object_mut)
            .context("instruction target request fixture is invalid")?
            .insert(
                "endpoint".to_owned(),
                json!("https://caller-controlled.example.test/"),
            );

        let error = bind_instruction_target(&runtime, &resolver, &mut request)
            .err()
            .context("caller-supplied Sequencer endpoint was accepted")?;

        if error.to_string() != "Active Zone target request is invalid"
            || resolver.calls.load(Ordering::SeqCst) != 0
        {
            bail!("raw endpoint reached target resolver: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn extra_instruction_submission_arguments_are_rejected() -> Result<()> {
        let runtime = Runtime::new()?;
        let resolver = RecordingTargetResolver::successful();
        let mut request =
            instruction_request(Some(ConfirmationPolicy::WalletInstructionSubmit.token()))?;
        request
            .args
            .as_array_mut()
            .context("instruction args fixture is invalid")?
            .push(json!("untrusted-extra"));

        let error = bind_instruction_target(&runtime, &resolver, &mut request)
            .err()
            .context("extra instruction submission argument was accepted")?;

        if error.to_string() != "IDL instruction submission requires exactly four arguments"
            || resolver.calls.load(Ordering::SeqCst) != 0
        {
            bail!("extra args reached target resolver: {error:#}");
        }
        Ok(())
    }

    #[tokio::test]
    async fn instruction_submission_stop_keeps_remote_effect_unconfirmed() -> Result<()> {
        let control = test_operation_control(Duration::from_secs(5));
        control.cancellation().cancel();

        let error = interruptible_remote(
            &control,
            "wallet instruction submission stopped locally; remote transaction state is unknown",
            pending::<Result<serde_json::Value>>(),
        )
        .await
        .err()
        .context("canceled instruction submission wrapper unexpectedly completed")?;

        anyhow::ensure!(
            error.downcast_ref::<OperationInterrupted>().is_some()
                && error
                    .to_string()
                    .contains("remote transaction state is unknown"),
            "instruction submission stop claimed unsupported remote evidence: {error:#}"
        );
        Ok(())
    }
}
