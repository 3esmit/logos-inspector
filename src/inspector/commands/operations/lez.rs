use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    support::{command_runner::CommandControl, confirmation::ConfirmationPolicy},
    wallet,
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::dispatch::{interruptible_remote, normalize_command_execution};
use super::spec::{OperationClass, OperationCommand, OperationDefinition, OperationMethod};
use super::supervisor::{OperationControl, TerminationEvidence};
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
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletInstructionSubmit)?;
    let submission = interruptible_remote(
        control,
        "wallet instruction submission stopped locally; remote transaction state is unknown",
        wallet::local_wallet_instruction_submit(
            wallet_profile_arg(&args)?,
            args.value(1)
                .cloned()
                .context("IDL instruction request is required")?,
        ),
    )
    .await??;
    to_value(submission)
}

fn command_control(control: &OperationControl) -> CommandControl {
    control.command_control()
}

#[cfg(test)]
mod tests {
    use std::{future::pending, time::Duration};

    use anyhow::{Context as _, Result};

    use super::*;
    use crate::inspector::commands::operations::supervisor::{
        OperationInterrupted, test_operation_control,
    };

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
