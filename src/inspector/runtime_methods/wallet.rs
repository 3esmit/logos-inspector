use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    bedrock_wallet_balance, local_wallet_instruction_preview, local_wallet_profile_status,
    source_routing::Args, wallet::detected_wallet_profile,
};

use super::super::bridge::to_value;

pub(super) fn try_handle(runtime: &Runtime, method: &str, args: Value) -> Result<Option<Value>> {
    let value = match method {
        "localWalletProfileStatus" => {
            let args = Args::new(args)?;
            to_value(local_wallet_profile_status(
                args.value(0)
                    .cloned()
                    .context("local wallet profile is required")?,
            )?)?
        }
        "localWalletInstructionPreview" => {
            let args = Args::new(args)?;
            to_value(local_wallet_instruction_preview(
                args.value(0)
                    .cloned()
                    .context("IDL instruction request is required")?,
            )?)?
        }
        "bedrockWalletBalance" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(bedrock_wallet_balance(
                args.string(0, "node endpoint")?,
                args.string(1, "wallet public key")?,
                args.optional_string(2),
            ))?)?
        }
        "detectWalletProfile" => detected_wallet_profile(),
        _ => return Ok(None),
    };
    Ok(Some(value))
}
