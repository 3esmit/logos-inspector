use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{support::confirmation::ConfirmationPolicy, wallet};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationClass, OperationCommand, OperationDefinition, OperationMethod};
use super::wallet_args::{confirmed_wallet_args, wallet_profile_arg};

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
    ),
];

pub(super) async fn execute(
    command: ExecutionCommand,
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    match command {
        ExecutionCommand::DeployProgram => execute_program_deployment(request).await,
        ExecutionCommand::SubmitInstruction => execute_instruction_submission(request).await,
    }
}

async fn execute_program_deployment(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletDeployProgram)?;
    let profile = wallet_profile_arg(&args)?;
    let program_path = args.string(1, "program path")?.to_owned();
    blocking_value("program deployment", move || {
        to_value(wallet::local_wallet_deploy_program(profile, &program_path)?)
    })
    .await
}

async fn execute_instruction_submission(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletInstructionSubmit)?;
    to_value(
        wallet::local_wallet_instruction_submit(
            wallet_profile_arg(&args)?,
            args.value(1)
                .cloned()
                .context("IDL instruction request is required")?,
        )
        .await?,
    )
}
