use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    source_routing::Args,
    state_store::{
        load_idl_state as load_idl_state_store, load_settings_state as load_settings_state_store,
        load_wallet_state as load_wallet_state_store, save_idl_state as save_idl_state_store,
        save_settings_state as save_settings_state_store,
        save_wallet_state as save_wallet_state_store,
    },
};

pub(super) fn load_idl_state() -> Result<Value> {
    load_idl_state_store()
}

pub(super) fn save_idl_state(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    save_idl_state_store(args.value(0).context("IDL state is required")?)
}

pub(super) fn load_wallet_state() -> Result<Value> {
    load_wallet_state_store()
}

pub(super) fn save_wallet_state(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    save_wallet_state_store(args.value(0).context("wallet state is required")?)
}

pub(super) fn load_settings_state() -> Result<Value> {
    load_settings_state_store()
}

pub(super) fn save_settings_state(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    save_settings_state_store(args.value(0).context("settings state is required")?)
}
