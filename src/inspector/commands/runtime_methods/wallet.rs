use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    bedrock_wallet_balance as inspect_bedrock_wallet_balance,
    local_wallet_instruction_preview as inspect_local_wallet_instruction_preview,
    local_wallet_profile_status as inspect_local_wallet_profile_status, source_routing::Args,
    wallet::detected_wallet_profile,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::sync("localWalletProfileStatus", local_wallet_profile_status),
    RuntimeMethodEntry::sync(
        "localWalletInstructionPreview",
        local_wallet_instruction_preview,
    ),
    RuntimeMethodEntry::with_runtime("bedrockWalletBalance", bedrock_wallet_balance),
    RuntimeMethodEntry::no_args("detectWalletProfile", detect_wallet_profile),
];

pub(super) fn local_wallet_profile_status(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(inspect_local_wallet_profile_status(
        args.value(0)
            .cloned()
            .context("local wallet profile is required")?,
    )?)
}

pub(super) fn local_wallet_instruction_preview(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(inspect_local_wallet_instruction_preview(
        args.value(0)
            .cloned()
            .context("IDL instruction request is required")?,
    )?)
}

pub(super) fn bedrock_wallet_balance(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(runtime.block_on(inspect_bedrock_wallet_balance(
        args.string(0, "node endpoint")?,
        args.string(1, "wallet public key")?,
        args.optional_string(2),
    ))?)
}

pub(super) fn detect_wallet_profile() -> Result<Value> {
    Ok(detected_wallet_profile())
}
