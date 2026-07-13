use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    support::args::Args,
    support::backup_catalog::{
        attach_remote_backup_metadata, create_local_settings_backup, load_backup_catalog_value,
    },
    support::state_store::{
        load_idl_state as load_idl_state_store, load_settings_state as load_settings_state_store,
        load_wallet_state as load_wallet_state_store, save_idl_state as save_idl_state_store,
        save_settings_state as save_settings_state_store,
        save_wallet_state as save_wallet_state_store,
    },
};

use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::no_args("loadIdlState", load_idl_state),
    RuntimeMethodEntry::sync("saveIdlState", save_idl_state),
    RuntimeMethodEntry::no_args("loadWalletState", load_wallet_state),
    RuntimeMethodEntry::sync("saveWalletState", save_wallet_state),
    RuntimeMethodEntry::no_args("loadSettingsState", load_settings_state),
    RuntimeMethodEntry::sync("saveSettingsState", save_settings_state),
    RuntimeMethodEntry::no_args("loadBackupCatalog", load_backup_catalog),
    RuntimeMethodEntry::sync("createLocalSettingsBackup", create_local_backup),
    RuntimeMethodEntry::sync("attachBackupRemote", attach_backup_remote),
];

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

pub(super) fn load_backup_catalog() -> Result<Value> {
    load_backup_catalog_value()
}

pub(super) fn create_local_backup(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let entry = create_local_settings_backup(
        args.optional_string(0),
        args.optional_bool(1),
        args.value(2),
        args.value(3),
    )?;
    serde_json::to_value(entry).context("failed to serialize backup catalog entry")
}

pub(super) fn attach_backup_remote(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let entry = attach_remote_backup_metadata(
        args.string(0, "backup catalog id")?,
        args.string(1, "remote backup CID")?,
        args.optional_string(2),
    )?;
    serde_json::to_value(entry).context("failed to serialize backup catalog entry")
}
