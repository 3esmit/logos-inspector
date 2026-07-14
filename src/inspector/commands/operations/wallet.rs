use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    support::{args::Args, command_runner::CommandControl, confirmation::ConfirmationPolicy},
    wallet::{
        local_wallet_accounts_controlled, local_wallet_command_controlled,
        local_wallet_create_account_controlled, local_wallet_send_transaction_controlled,
        local_wallet_sync_private_controlled,
    },
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::dispatch::normalize_command_execution;
use super::spec::{OperationClass, OperationCommand, OperationDefinition, OperationMethod};
use super::supervisor::{OperationControl, TerminationEvidence};
use super::wallet_args::{confirmed_wallet_args, wallet_profile_arg};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum WalletCommand {
    CreateAccount,
    SendTransaction,
    Command,
    SyncPrivate,
    Accounts,
}

impl WalletCommand {
    pub(super) const fn method(self) -> OperationMethod {
        match self {
            Self::CreateAccount => OperationMethod::LocalWalletCreateAccount,
            Self::SendTransaction => OperationMethod::LocalWalletSendTransaction,
            Self::Command => OperationMethod::LocalWalletCommand,
            Self::SyncPrivate => OperationMethod::LocalWalletSyncPrivate,
            Self::Accounts => OperationMethod::LocalWalletAccounts,
        }
    }
}

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationCommand::Wallet(WalletCommand::CreateAccount),
        "localWalletCreateAccount",
        "Wallet account",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationCommand::Wallet(WalletCommand::SendTransaction),
        "localWalletSendTransaction",
        "Wallet send",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationCommand::Wallet(WalletCommand::Command),
        "localWalletCommand",
        "Wallet command",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationCommand::Wallet(WalletCommand::SyncPrivate),
        "localWalletSyncPrivate",
        "Private sync",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationCommand::Wallet(WalletCommand::Accounts),
        "localWalletAccounts",
        "Wallet accounts",
        OperationClass::ReadPoll,
    ),
];

pub(super) async fn execute(
    command: WalletCommand,
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    match command {
        WalletCommand::CreateAccount => execute_wallet_create_account(request, control).await,
        WalletCommand::SendTransaction => execute_wallet_send_transaction(request, control).await,
        WalletCommand::Command => execute_wallet_command(request, control).await,
        WalletCommand::SyncPrivate => execute_wallet_sync_private(request, control).await,
        WalletCommand::Accounts => execute_wallet_accounts(request, control).await,
    }
}

pub(super) async fn execute_wallet_create_account(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 3, ConfirmationPolicy::WalletCreateAccount)?;
    let profile = wallet_profile_arg(&args)?;
    let privacy = args.string(1, "account privacy")?.to_owned();
    let label = args.optional_string(2).map(ToOwned::to_owned);
    let command_control = command_control(control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("wallet account creation", move || {
        let _worker_guard = worker_guard;
        to_value(local_wallet_create_account_controlled(
            profile,
            &privacy,
            label.as_deref(),
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

pub(super) async fn execute_wallet_send_transaction(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletSendTransaction)?;
    let profile = wallet_profile_arg(&args)?;
    let send_request = args
        .value(1)
        .cloned()
        .context("wallet send request is required")?;
    let command_control = command_control(control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("wallet transaction send", move || {
        let _worker_guard = worker_guard;
        to_value(local_wallet_send_transaction_controlled(
            profile,
            send_request,
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

pub(super) async fn execute_wallet_command(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletCommand)?;
    let command_args = serde_json::from_value::<Vec<String>>(
        args.value(1)
            .cloned()
            .context("wallet command arguments are required")?,
    )
    .context("wallet command arguments must be a string array")?;
    let profile = wallet_profile_arg(&args)?;
    let command_control = command_control(control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("wallet command", move || {
        let _worker_guard = worker_guard;
        to_value(local_wallet_command_controlled(
            profile,
            command_args,
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

pub(super) async fn execute_wallet_sync_private(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 1, ConfirmationPolicy::WalletSyncPrivate)?;
    let profile = wallet_profile_arg(&args)?;
    let command_control = command_control(control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("private wallet sync", move || {
        let _worker_guard = worker_guard;
        to_value(local_wallet_sync_private_controlled(
            profile,
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

pub(super) async fn execute_wallet_accounts(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let profile = wallet_profile_arg(&args)?;
    let command_control = command_control(control);
    let worker_guard = control.blocking_worker_guard()?;
    let result = blocking_value("wallet accounts", move || {
        let _worker_guard = worker_guard;
        to_value(local_wallet_accounts_controlled(profile, command_control)?)
    })
    .await;
    normalize_command_execution(
        result,
        control,
        TerminationEvidence::Confirmed,
        TerminationEvidence::Confirmed,
    )
}

fn command_control(control: &OperationControl) -> CommandControl {
    control.command_control()
}
