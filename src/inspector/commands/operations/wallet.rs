use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::{
    local_wallet_accounts, local_wallet_command, local_wallet_create_account,
    local_wallet_deploy_program, local_wallet_instruction_submit, local_wallet_send_transaction,
    local_wallet_sync_private, source_routing::Args,
};

use super::super::value::{blocking_value, to_value};
use super::NodeOperationRequest;

pub(super) async fn execute_wallet_create_account(request: &NodeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(
        request,
        3,
        "confirm-create-account",
        "wallet account creation requires explicit confirmation",
    )?;
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
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = confirmed_wallet_args(
        request,
        2,
        "confirm-send-transaction",
        "wallet transaction send requires explicit confirmation",
    )?;
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
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = confirmed_wallet_args(
        request,
        2,
        "confirm-idl-instruction",
        "IDL instruction send requires explicit confirmation",
    )?;
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

pub(super) async fn execute_wallet_command(request: &NodeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(
        request,
        2,
        "confirm-wallet-command",
        "wallet command requires explicit confirmation",
    )?;
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

pub(super) async fn execute_wallet_deploy_program(request: &NodeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(
        request,
        2,
        "confirm-deploy-program",
        "program deployment requires explicit confirmation",
    )?;
    let profile = wallet_profile_arg(&args)?;
    let program_path = args.string(1, "program path")?.to_owned();
    blocking_value("program deployment", move || {
        to_value(local_wallet_deploy_program(profile, &program_path)?)
    })
    .await
}

pub(super) async fn execute_wallet_sync_private(request: &NodeOperationRequest) -> Result<Value> {
    let args = confirmed_wallet_args(
        request,
        1,
        "confirm-sync-private",
        "private wallet sync requires explicit confirmation",
    )?;
    let profile = wallet_profile_arg(&args)?;
    blocking_value("private wallet sync", move || {
        to_value(local_wallet_sync_private(profile)?)
    })
    .await
}

pub(super) async fn execute_wallet_accounts(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let profile = wallet_profile_arg(&args)?;
    blocking_value("wallet accounts", move || {
        to_value(local_wallet_accounts(profile)?)
    })
    .await
}

fn confirmed_wallet_args(
    request: &NodeOperationRequest,
    confirmation_index: usize,
    token: &str,
    error: &str,
) -> Result<Args> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(confirmation_index) != Some(token) {
        bail!("{error}");
    }
    Ok(args)
}

fn wallet_profile_arg(args: &Args) -> Result<Value> {
    args.value(0)
        .cloned()
        .context("local wallet profile is required")
}
