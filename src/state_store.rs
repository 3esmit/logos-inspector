use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::{
    normalize_program_id_hex,
    wallet::{default_wallet_state, wallet_state_with_detected_profile},
};

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
        return Ok(wallet_state_with_detected_profile(default_wallet_state()));
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read wallet state from {}", path.display()))?;
    let state: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse wallet state from {}", path.display()))?;
    Ok(wallet_state_with_detected_profile(state))
}

pub(crate) fn save_wallet_state(state: &Value) -> Result<Value> {
    let path = wallet_state_path()?;
    write_state("wallet", &path, state)
}

pub(crate) fn load_settings_state() -> Result<Value> {
    let path = settings_state_path()?;
    if !path.is_file() {
        return Ok(default_settings_state());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read settings state from {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse settings state from {}", path.display()))
}

pub(crate) fn save_settings_state(state: &Value) -> Result<Value> {
    let path = settings_state_path()?;
    write_state("settings", &path, state)
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

fn default_settings_state() -> Value {
    json!({
        "version": 1
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

fn settings_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

fn config_dir() -> Result<PathBuf> {
    if let Some(value) = env::var_os("LOGOS_INSPECTOR_CONFIG_DIR") {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(value).join("logos-inspector"));
    }
    if cfg!(windows)
        && let Some(value) = env::var_os("APPDATA")
    {
        return Ok(PathBuf::from(value).join("Logos Inspector"));
    }
    if cfg!(target_os = "macos")
        && let Some(value) = env::var_os("HOME")
    {
        return Ok(PathBuf::from(value)
            .join("Library")
            .join("Application Support")
            .join("Logos Inspector"));
    }
    if let Some(value) = env::var_os("HOME") {
        return Ok(PathBuf::from(value).join(".config").join("logos-inspector"));
    }
    bail!("could not determine config directory")
}
