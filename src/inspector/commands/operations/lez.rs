use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::{
    local_wallet_deploy_program, local_wallet_instruction_submit,
    support::confirmation::ConfirmationPolicy,
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationDefinition, OperationDomain, OperationMethod};
use super::wallet_args::{confirmed_wallet_args, wallet_profile_arg};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
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
];

pub(super) async fn execute(request: &RuntimeOperationRequest) -> Result<Value> {
    match request.method() {
        OperationMethod::LocalWalletDeployProgram => execute_program_deployment(request).await,
        OperationMethod::LocalWalletInstructionSubmit => {
            execute_instruction_submission(request).await
        }
        _ => bail!(
            "`{}` is not a program wallet operation",
            request.method_name()
        ),
    }
}

async fn execute_program_deployment(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletDeployProgram)?;
    let profile = wallet_profile_arg(&args)?;
    let program_path = args.string(1, "program path")?.to_owned();
    blocking_value("program deployment", move || {
        to_value(local_wallet_deploy_program(profile, &program_path)?)
    })
    .await
}

async fn execute_instruction_submission(request: &RuntimeOperationRequest) -> Result<Value> {
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
