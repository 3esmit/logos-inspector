use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    source_routing::Args,
    state_store::{
        load_idl_state, load_settings_state, load_wallet_state, save_idl_state,
        save_settings_state, save_wallet_state,
    },
};

pub(super) fn try_handle(method: &str, args: Value) -> Result<Option<Value>> {
    let value = match method {
        "loadIdlState" => load_idl_state()?,
        "saveIdlState" => {
            let args = Args::new(args)?;
            save_idl_state(args.value(0).context("IDL state is required")?)?
        }
        "loadWalletState" => load_wallet_state()?,
        "saveWalletState" => {
            let args = Args::new(args)?;
            save_wallet_state(args.value(0).context("wallet state is required")?)?
        }
        "loadSettingsState" => load_settings_state()?,
        "saveSettingsState" => {
            let args = Args::new(args)?;
            save_settings_state(args.value(0).context("settings state is required")?)?
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}
