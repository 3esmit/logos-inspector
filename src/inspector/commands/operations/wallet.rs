use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    local_wallet_accounts, local_wallet_command, local_wallet_create_account,
    local_wallet_deploy_program, local_wallet_instruction_submit, local_wallet_send_transaction,
    local_wallet_sync_private, source_routing::Args, support::confirmation::ConfirmationPolicy,
};

use super::super::value::{blocking_value, to_value};
use super::RuntimeOperationRequest;
use super::spec::{OperationCatalogEntry, OperationDomain, OperationMethod};

pub(super) const OPERATION_CATALOG: &[OperationCatalogEntry] = &[
    OperationCatalogEntry::new(
        OperationMethod::LocalWalletCreateAccount,
        "localWalletCreateAccount",
        OperationDomain::Wallet,
        "Wallet account",
    ),
    OperationCatalogEntry::new(
        OperationMethod::LocalWalletSendTransaction,
        "localWalletSendTransaction",
        OperationDomain::Wallet,
        "Wallet send",
    ),
    OperationCatalogEntry::new(
        OperationMethod::LocalWalletInstructionSubmit,
        "localWalletInstructionSubmit",
        OperationDomain::Wallet,
        "IDL instruction",
    ),
    OperationCatalogEntry::new(
        OperationMethod::LocalWalletCommand,
        "localWalletCommand",
        OperationDomain::Wallet,
        "Wallet command",
    ),
    OperationCatalogEntry::new(
        OperationMethod::LocalWalletDeployProgram,
        "localWalletDeployProgram",
        OperationDomain::Wallet,
        "Program deploy",
    ),
    OperationCatalogEntry::new(
        OperationMethod::LocalWalletSyncPrivate,
        "localWalletSyncPrivate",
        OperationDomain::Wallet,
        "Private sync",
    ),
    OperationCatalogEntry::new(
        OperationMethod::LocalWalletAccounts,
        "localWalletAccounts",
        OperationDomain::Wallet,
        "Wallet accounts",
    ),
];

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

pub(super) async fn execute_wallet_instruction_submit(
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

pub(super) async fn execute_wallet_deploy_program(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = confirmed_wallet_args(request, 2, ConfirmationPolicy::WalletDeployProgram)?;
    let profile = wallet_profile_arg(&args)?;
    let program_path = args.string(1, "program path")?.to_owned();
    blocking_value("program deployment", move || {
        to_value(local_wallet_deploy_program(profile, &program_path)?)
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

fn confirmed_wallet_args(
    request: &RuntimeOperationRequest,
    confirmation_index: usize,
    policy: ConfirmationPolicy,
) -> Result<Args> {
    let args = Args::new(request.args.clone())?;
    policy.require(args.optional_string(confirmation_index))?;
    Ok(args)
}

fn wallet_profile_arg(args: &Args) -> Result<Value> {
    args.value(0)
        .cloned()
        .context("local wallet profile is required")
}
