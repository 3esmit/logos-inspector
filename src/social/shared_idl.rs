use serde::Serialize;
use serde_json::Value;

use super::{SocialPayload, social_messages_from_store};
use crate::{
    normalize_program_id_hex,
    program_decode::{ProgramDecodeCandidate, resolve_account_decode_session},
};

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedSharedIdlEntry {
    pub key: String,
    pub name: String,
    pub program_id: String,
    pub program_id_hex: String,
    pub program_binary: String,
    pub json: String,
    pub source: String,
    pub shared_topic: String,
    pub shared_identity: Value,
    pub shared_account_id: String,
    pub account_type: String,
}

#[must_use]
pub fn accepted_shared_idl_entries_from_store(
    topic: &str,
    store_value: &Value,
    account_id: &str,
    data_hex: &str,
    owner_program_id: Option<&str>,
) -> Vec<AcceptedSharedIdlEntry> {
    social_messages_from_store(topic, store_value, Some(account_id))
        .into_iter()
        .filter_map(|message| match message.payload {
            SocialPayload::LezAccountIdl {
                identity,
                account_id: shared_account_id,
                program_id,
                idl_name,
                idl_json,
                ..
            } => accepted_shared_idl_entry(SharedIdlInput {
                topic,
                account_id,
                data_hex,
                owner_program_id,
                identity,
                shared_account_id,
                program_id,
                idl_name,
                idl_json,
            }),
            SocialPayload::Comment { .. } => None,
        })
        .collect()
}

struct SharedIdlInput<'a> {
    topic: &'a str,
    account_id: &'a str,
    data_hex: &'a str,
    owner_program_id: Option<&'a str>,
    identity: Value,
    shared_account_id: String,
    program_id: String,
    idl_name: String,
    idl_json: String,
}

fn accepted_shared_idl_entry(input: SharedIdlInput<'_>) -> Option<AcceptedSharedIdlEntry> {
    let program_id_hex = canonical_program_id(&input.program_id);
    let owner = input.owner_program_id.and_then(canonical_program_id_opt);
    if let Some(owner) = owner
        && !program_id_hex.is_empty()
        && owner != program_id_hex
    {
        return None;
    }
    let account_types = idl_account_types(&input.idl_json);
    if account_types.is_empty() {
        return None;
    }
    let candidates = account_types
        .into_iter()
        .map(|account_type| ProgramDecodeCandidate {
            key: String::new(),
            name: input.idl_name.clone(),
            program_id_hex: program_id_hex.clone(),
            json: input.idl_json.clone(),
            account_type: Some(account_type),
            source: Some("shared".to_owned()),
            cached: false,
            shared: true,
            owner_matched: false,
        })
        .collect::<Vec<_>>();
    let session =
        resolve_account_decode_session(Some(input.account_id), input.data_hex, &candidates);
    let selected = session.selected?;
    let account_type = selected.evidence.account_type?;
    let name = if input.idl_name.trim().is_empty() {
        idl_name(&input.idl_json).unwrap_or_else(|| "Shared IDL".to_owned())
    } else {
        input.idl_name
    };
    Some(AcceptedSharedIdlEntry {
        key: idl_key(&name, &program_id_hex, &input.idl_json),
        name,
        program_id: input.program_id,
        program_id_hex,
        program_binary: String::new(),
        json: input.idl_json,
        source: "shared".to_owned(),
        shared_topic: input.topic.to_owned(),
        shared_identity: input.identity,
        shared_account_id: input.shared_account_id,
        account_type,
    })
}

fn idl_account_types(idl_json: &str) -> Vec<String> {
    let Ok(value) = serde_json::from_str::<Value>(idl_json) else {
        return Vec::new();
    };
    value
        .get("accounts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|account| account.get("name").and_then(Value::as_str))
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn idl_name(idl_json: &str) -> Option<String> {
    serde_json::from_str::<Value>(idl_json)
        .ok()
        .and_then(|value| {
            value
                .get("name")
                .and_then(Value::as_str)
                .map(ToOwned::to_owned)
        })
}

fn canonical_program_id(value: &str) -> String {
    canonical_program_id_opt(value).unwrap_or_else(|| value.trim().to_owned())
}

fn canonical_program_id_opt(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }
    normalize_program_id_hex(trimmed).ok().or_else(|| {
        let without_prefix = trimmed.strip_prefix("0x").unwrap_or(trimmed);
        without_prefix
            .chars()
            .all(|ch| ch.is_ascii_hexdigit())
            .then(|| without_prefix.to_ascii_lowercase())
    })
}

fn idl_key(name: &str, program_id: &str, json: &str) -> String {
    let text = format!("{name}\n{program_id}\n{json}");
    let mut hash = 2_166_136_261_u32;
    for code_unit in text.encode_utf16() {
        hash ^= u32::from(code_unit);
        hash = hash.wrapping_mul(16_777_619);
    }
    format!("{hash:x}")
}
