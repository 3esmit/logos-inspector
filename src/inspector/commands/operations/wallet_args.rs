use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{source_routing::Args, support::confirmation::ConfirmationPolicy};

use super::RuntimeOperationRequest;

pub(super) fn confirmed_wallet_args(
    request: &RuntimeOperationRequest,
    confirmation_index: usize,
    policy: ConfirmationPolicy,
) -> Result<Args> {
    let args = Args::new(request.args.clone())?;
    policy.require(args.optional_string(confirmation_index))?;
    Ok(args)
}

pub(super) fn wallet_profile_arg(args: &Args) -> Result<Value> {
    args.value(0)
        .cloned()
        .context("local wallet profile is required")
}
