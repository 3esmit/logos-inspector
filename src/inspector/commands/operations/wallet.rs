use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::{
    local_wallet_accounts, local_wallet_command, local_wallet_create_account,
    local_wallet_send_transaction, local_wallet_sync_private, support::args::Args,
    support::confirmation::ConfirmationPolicy,
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationClass, OperationDefinition, OperationDomain, OperationMethod};
use super::wallet_args::{confirmed_wallet_args, wallet_profile_arg};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationMethod::LocalWalletCreateAccount,
        "localWalletCreateAccount",
        OperationDomain::Wallet,
        "Wallet account",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationMethod::LocalWalletSendTransaction,
        "localWalletSendTransaction",
        OperationDomain::Wallet,
        "Wallet send",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationMethod::LocalWalletCommand,
        "localWalletCommand",
        OperationDomain::Wallet,
        "Wallet command",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationMethod::LocalWalletSyncPrivate,
        "localWalletSyncPrivate",
        OperationDomain::Wallet,
        "Private sync",
        OperationClass::SigningSubmission,
    ),
    OperationDefinition::new(
        OperationMethod::LocalWalletAccounts,
        "localWalletAccounts",
        OperationDomain::Wallet,
        "Wallet accounts",
        OperationClass::ReadPoll,
    ),
];

pub(super) async fn execute(request: &RuntimeOperationRequest) -> Result<Value> {
    match request.method() {
        OperationMethod::LocalWalletCreateAccount => execute_wallet_create_account(request).await,
        OperationMethod::LocalWalletSendTransaction => {
            execute_wallet_send_transaction(request).await
        }
        OperationMethod::LocalWalletCommand => execute_wallet_command(request).await,
        OperationMethod::LocalWalletSyncPrivate => execute_wallet_sync_private(request).await,
        OperationMethod::LocalWalletAccounts => execute_wallet_accounts(request).await,
        _ => bail!(
            "`{}` is not a Wallet Operations operation",
            request.method_name()
        ),
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
