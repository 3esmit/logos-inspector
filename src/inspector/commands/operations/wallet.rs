use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    local_wallet_accounts, local_wallet_command, local_wallet_create_account,
    local_wallet_send_transaction, local_wallet_sync_private, support::args::Args,
    support::confirmation::ConfirmationPolicy,
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationClass, OperationCommand, OperationDefinition, OperationMethod};
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
) -> Result<Value> {
    match command {
        WalletCommand::CreateAccount => execute_wallet_create_account(request).await,
        WalletCommand::SendTransaction => execute_wallet_send_transaction(request).await,
        WalletCommand::Command => execute_wallet_command(request).await,
        WalletCommand::SyncPrivate => execute_wallet_sync_private(request).await,
        WalletCommand::Accounts => execute_wallet_accounts(request).await,
    }
}

pub(super) async fn execute_wallet_create_account(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 3, ConfirmationPolicy::WalletCreateAccount)?;
    let profile = wallet_profile_arg(&args)?;
    let privacy = args.string(1, "account privacy")?.to_owned();
    let label = args.optional_string(2).map(ToOwned::to_owned);
    blocking_value("wallet account creation", move || {
        to_value(local_wallet_create_account(
            profile,
            &privacy,
            label.as_deref(),
        )?)
    })
    .await
}

pub(super) async fn execute_wallet_send_transaction(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletSendTransaction)?;
    let profile = wallet_profile_arg(&args)?;
    let send_request = args
        .value(1)
        .cloned()
        .context("wallet send request is required")?;
    blocking_value("wallet transaction send", move || {
        to_value(local_wallet_send_transaction(profile, send_request)?)
    })
    .await
}

pub(super) async fn execute_wallet_command(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletCommand)?;
    let command_args = serde_json::from_value::<Vec<String>>(
        args.value(1)
            .cloned()
            .context("wallet command arguments are required")?,
    )
    .context("wallet command arguments must be a string array")?;
    let profile = wallet_profile_arg(&args)?;
    blocking_value("wallet command", move || {
        to_value(local_wallet_command(profile, command_args)?)
    })
    .await
}

pub(super) async fn execute_wallet_sync_private(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 1, ConfirmationPolicy::WalletSyncPrivate)?;
    let profile = wallet_profile_arg(&args)?;
    blocking_value("private wallet sync", move || {
        to_value(local_wallet_sync_private(profile)?)
    })
    .await
}

pub(super) async fn execute_wallet_accounts(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let profile = wallet_profile_arg(&args)?;
    blocking_value("wallet accounts", move || {
        to_value(local_wallet_accounts(profile)?)
    })
    .await
}
