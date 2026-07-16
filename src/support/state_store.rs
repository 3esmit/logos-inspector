use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::{
    source_routing::channel_sources::save_user_settings_state,
    support::entity_id::normalize_program_id_hex, wallet::default_wallet_state,
};

pub(crate) use super::config_path::config_dir;
use super::local_state::{LocalStateSession, StateFile, StoredBytes, with_local_state};

#[derive(Debug, Clone)]
pub(crate) struct RegisteredIdlEntry {
    pub(crate) key: String,
    pub(crate) name: String,
    pub(crate) program_id_hex: String,
    pub(crate) source: String,
    pub(crate) json: String,
}

pub(crate) fn load_idl_state() -> Result<Value> {
    with_local_state(|session| load_idl_state_locked(session))
}

pub(crate) fn save_idl_state(state: &Value) -> Result<Value> {
    with_local_state(|session| write_state(session, StateFile::Idl, "IDL", state))
}

pub(crate) fn registered_idl_entries() -> Result<Vec<RegisteredIdlEntry>> {
    let state = load_idl_state()?;
    Ok(registered_idl_entries_from_state(&state))
}

fn registered_idl_entries_from_state(state: &Value) -> Vec<RegisteredIdlEntry> {
    let mut entries = state
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
                key: entry
                    .get("key")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                name: entry
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                program_id_hex,
                source: entry
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                json: json.to_owned(),
            })
        })
        .collect::<Vec<_>>();
    entries.sort_by(registered_idl_entry_order);
    entries
}

fn registered_idl_entry_order(
    left: &RegisteredIdlEntry,
    right: &RegisteredIdlEntry,
) -> std::cmp::Ordering {
    registered_idl_source_order(&left.source)
        .cmp(&registered_idl_source_order(&right.source))
        .then_with(|| left.name.cmp(&right.name))
        .then_with(|| left.key.cmp(&right.key))
        .then_with(|| left.json.cmp(&right.json))
}

fn registered_idl_source_order(source: &str) -> u8 {
    if source == "shared" { 1 } else { 0 }
}

pub(crate) fn load_wallet_state() -> Result<Value> {
    with_local_state(|session| load_wallet_state_locked(session))
}

pub(crate) fn save_wallet_state(state: &Value) -> Result<Value> {
    with_local_state(|session| write_state(session, StateFile::Wallet, "wallet", state))
}

pub(crate) use crate::source_routing::channel_sources::load_settings_state;

pub(crate) fn save_settings_state(state: &Value) -> Result<Value> {
    save_user_settings_state(state)
}

pub(crate) fn load_idl_state_locked(session: &LocalStateSession) -> Result<Value> {
    idl_state_from_stored(&session.read(StateFile::Idl)?)
}

pub(crate) fn load_wallet_state_locked(session: &LocalStateSession) -> Result<Value> {
    wallet_state_from_stored(&session.read(StateFile::Wallet)?)
}

pub(crate) fn idl_state_from_stored(stored: &StoredBytes) -> Result<Value> {
    json_state_from_stored(stored, default_idl_state, "IDL")
}

pub(crate) fn wallet_state_from_stored(stored: &StoredBytes) -> Result<Value> {
    json_state_from_stored(stored, default_wallet_state, "wallet")
}

fn json_state_from_stored(
    stored: &StoredBytes,
    default_value: fn() -> Value,
    label: &str,
) -> Result<Value> {
    match stored {
        StoredBytes::Missing => Ok(default_value()),
        StoredBytes::Present(bytes) => {
            serde_json::from_slice(bytes).with_context(|| format!("failed to parse {label} state"))
        }
    }
}

fn write_state(
    session: &mut LocalStateSession,
    file: StateFile,
    label: &str,
    state: &Value,
) -> Result<Value> {
    let bytes = serde_json::to_vec_pretty(state)
        .with_context(|| format!("failed to serialize {label} state"))?;
    let durability = session.atomic_replace(file, &bytes)?;
    Ok(json!({
        "saved": true,
        "path": session.path_text(file),
        "directory_durability": durability.as_str(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_idl_entries_prefer_local_entries_before_shared_entries() {
        let program_id = "ab".repeat(32);
        let entries = registered_idl_entries_from_state(&json!({
            "idls": [{
                "key": "shared-first",
                "name": "Shared",
                "programIdHex": program_id,
                "source": "shared",
                "json": "{\"name\":\"shared\"}"
            }, {
                "key": "local-z",
                "name": "Zulu",
                "programIdHex": "ab".repeat(32),
                "source": "local",
                "json": "{\"name\":\"local-z\"}"
            }, {
                "key": "local-a",
                "name": "Alpha",
                "programIdHex": "ab".repeat(32),
                "source": "local",
                "json": "{\"name\":\"local-a\"}"
            }]
        }));

        let names = entries
            .iter()
            .map(|entry| entry.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Alpha", "Zulu", "Shared"]);
    }
}
