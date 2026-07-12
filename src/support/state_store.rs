use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::{
    source_routing::channel_sources::save_user_settings_state,
    support::entity_id::normalize_program_id_hex, wallet::default_wallet_state,
};

pub(crate) use super::config_path::config_dir;

#[derive(Debug, Clone)]
pub(crate) struct RegisteredIdlEntry {
    pub(crate) program_id_hex: String,
    pub(crate) json: String,
}

pub(crate) fn load_idl_state() -> Result<Value> {
    let path = idl_state_path()?;
    if !path.is_file() {
        return Ok(default_idl_state());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read IDL state from {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse IDL state from {}", path.display()))
}

pub(crate) fn save_idl_state(state: &Value) -> Result<Value> {
    let path = idl_state_path()?;
    write_state("IDL", &path, state)
}

pub(crate) fn registered_idl_entries() -> Result<Vec<RegisteredIdlEntry>> {
    let state = load_idl_state()?;
    Ok(state
        .get("idls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let json = entry.get("json").and_then(Value::as_str)?.trim();
            if json.is_empty() {
                return None;
            }
            let program_id_hex = registered_idl_program_id_hex(entry);
            if program_id_hex.is_empty() {
                return None;
            }
            Some(RegisteredIdlEntry {
                program_id_hex,
                json: json.to_owned(),
            })
        })
        .collect())
}

pub(crate) fn load_wallet_state() -> Result<Value> {
    let path = wallet_state_path()?;
    if !path.is_file() {
        return Ok(default_wallet_state());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read wallet state from {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse wallet state from {}", path.display()))
}

pub(crate) fn save_wallet_state(state: &Value) -> Result<Value> {
    let path = wallet_state_path()?;
    write_state("wallet", &path, state)
}

pub(crate) use crate::source_routing::channel_sources::load_settings_state;

pub(crate) fn save_settings_state(state: &Value) -> Result<Value> {
    save_user_settings_state(state)
}

fn write_state(label: &str, path: &Path, state: &Value) -> Result<Value> {
    let parent = path
        .parent()
        .with_context(|| format!("{label} state path has no parent directory"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    let text = serde_json::to_string_pretty(state)
        .with_context(|| format!("failed to serialize {label} state"))?;
    fs::write(path, text)
        .with_context(|| format!("failed to write {label} state to {}", path.display()))?;
    Ok(json!({
        "saved": true,
        "path": path.display().to_string(),
    }))
}

fn default_idl_state() -> Value {
    json!({
        "version": 1,
        "idls": [],
        "account_idl_selections": {},
    })
}

fn registered_idl_program_id_hex(entry: &Value) -> String {
    entry
        .get("programIdHex")
        .or_else(|| entry.get("program_id_hex"))
        .and_then(Value::as_str)
        .and_then(normalized_program_id_hex_text)
        .or_else(|| {
            entry
                .get("programId")
                .or_else(|| entry.get("program_id"))
                .and_then(Value::as_str)
                .and_then(normalized_program_id_hex_text)
        })
        .unwrap_or_default()
}

fn normalized_program_id_hex_text(value: &str) -> Option<String> {
    normalize_program_id_hex(value)
        .ok()
        .filter(|text| !text.is_empty())
}

fn idl_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("idls.json"))
}

fn wallet_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("wallet.json"))
}
