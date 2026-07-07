use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::{
    bridge::{blocking_value, to_value},
    local_wallet_accounts, local_wallet_command, local_wallet_create_account,
    local_wallet_deploy_program, local_wallet_instruction_submit, local_wallet_send_transaction,
    local_wallet_sync_private,
    source_routing::Args,
};

use super::NodeOperationRequest;

pub(super) async fn execute_wallet_create_account(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(3) != Some("confirm-create-account") {
        bail!("wallet account creation requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
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
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-send-transaction") {
        bail!("wallet transaction send requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
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
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-idl-instruction") {
        bail!("IDL instruction send requires explicit confirmation");
    }
    to_value(
        local_wallet_instruction_submit(
            args.value(0)
                .cloned()
                .context("local wallet profile is required")?,
            args.value(1)
                .cloned()
                .context("IDL instruction request is required")?,
        )
        .await?,
    )
}

pub(super) async fn execute_wallet_command(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-wallet-command") {
        bail!("wallet command requires explicit confirmation");
    }
    let command_args = serde_json::from_value::<Vec<String>>(
        args.value(1)
            .cloned()
            .context("wallet command arguments are required")?,
    )
    .context("wallet command arguments must be a string array")?;
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    blocking_value("wallet command", move || {
        to_value(local_wallet_command(profile, command_args)?)
    })
    .await
}

pub(super) async fn execute_wallet_deploy_program(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-deploy-program") {
        bail!("program deployment requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    let program_path = args.string(1, "program path")?.to_owned();
    blocking_value("program deployment", move || {
        to_value(local_wallet_deploy_program(profile, &program_path)?)
    })
    .await
}

pub(super) async fn execute_wallet_sync_private(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.string(1, "private sync confirmation")? != "confirm-sync-private" {
        bail!("private wallet sync requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    blocking_value("private wallet sync", move || {
        to_value(local_wallet_sync_private(profile)?)
    })
    .await
}

pub(super) async fn execute_wallet_accounts(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    blocking_value("wallet accounts", move || {
        to_value(local_wallet_accounts(profile)?)
    })
    .await
}
