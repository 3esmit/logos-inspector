use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};

use crate::borsh_decode::{
    DecodedValue, decode_borsh_shape, decode_borsh_type, decode_instruction_type, idl_type_label,
    parse_hex_bytes,
};
use crate::value_to_string;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DecodedField {
    pub path: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AccountIdlDecodeReport {
    pub account_id: Option<String>,
    pub account_type: String,
    pub consumed_bytes: usize,
    pub total_bytes: usize,
    pub remaining_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remaining_data_hex: Option<String>,
    pub decoded: Value,
    pub rows: Vec<DecodedField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventIdlDecodeReport {
    pub event: String,
    pub consumed_bytes: usize,
    pub total_bytes: usize,
    pub decoded: Value,
    pub rows: Vec<DecodedField>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InstructionDecodeReport {
    pub program_id: String,
    pub idl_name: Option<String>,
    pub instruction: String,
    pub variant_index: u32,
    pub accounts: Vec<DecodedField>,
    pub args: Vec<DecodedField>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decode_error: Option<String>,
    pub remaining_words: Vec<u32>,
}

pub fn decode_account_data_hex_with_idl(
    idl_json: &str,
    account_type: Option<&str>,
    data_hex: &str,
    account_id: Option<&str>,
) -> Result<AccountIdlDecodeReport> {
    let idl: Value = serde_json::from_str(idl_json).context("failed to parse IDL JSON")?;
    let bytes = parse_hex_bytes(data_hex).context("failed to parse account data hex")?;
    let accounts = idl
        .get("accounts")
        .and_then(Value::as_array)
        .context("IDL has no accounts array")?;
    let selected = account_type
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut attempted = Vec::new();
    let mut best_partial: Option<(String, DecodedValue)> = None;

    for account in accounts {
        let name = account
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        if selected.is_some_and(|selected| selected != name) {
            continue;
        }

        let Some(shape) = account.get("type") else {
            attempted.push(format!("{name}: missing type"));
            continue;
        };

        match decode_borsh_shape(shape, &bytes, 0, &idl, 0) {
            Ok(decoded) if decoded.consumed == bytes.len() => {
                return Ok(account_idl_decode_report(account_id, name, decoded, &bytes));
            }
            Ok(decoded) if selected.is_some() => {
                return Ok(account_idl_decode_report(account_id, name, decoded, &bytes));
            }
            Ok(decoded) => {
                if best_partial
                    .as_ref()
                    .is_none_or(|(_, best)| decoded.consumed > best.consumed)
                {
                    best_partial = Some((name.to_owned(), decoded));
                } else {
                    attempted.push(format!(
                        "{name}: decoded {} of {} bytes",
                        decoded.consumed,
                        bytes.len()
                    ));
                }
            }
            Err(err) if selected.is_some() => {
                return Err(err).with_context(|| format!("failed to decode as `{name}`"));
            }
            Err(err) => attempted.push(format!("{name}: {err:#}")),
        }
    }

    if let Some((name, decoded)) = best_partial {
        return Ok(account_idl_decode_report(
            account_id, &name, decoded, &bytes,
        ));
    }

    if let Some(selected) = selected {
        bail!("IDL account `{selected}` not found");
    }

    bail!(
        "no IDL account shape decoded the data: {}",
        attempted.join("; ")
    )
}

fn account_idl_decode_report(
    account_id: Option<&str>,
    account_type: &str,
    decoded: DecodedValue,
    bytes: &[u8],
) -> AccountIdlDecodeReport {
    let remaining = bytes.get(decoded.consumed..).unwrap_or_default();
    let remaining_data_hex = (!remaining.is_empty()).then(|| hex::encode(remaining));
    let mut rows = Vec::new();
    flatten_decoded_value(&decoded.value, "", &mut rows);
    if let Some(remaining_data_hex) = &remaining_data_hex {
        rows.push(DecodedField {
            path: "remaining_data_hex".to_owned(),
            value: remaining_data_hex.clone(),
        });
    }
    AccountIdlDecodeReport {
        account_id: account_id.map(ToOwned::to_owned),
        account_type: account_type.to_owned(),
        consumed_bytes: decoded.consumed,
        total_bytes: bytes.len(),
        remaining_bytes: remaining.len(),
        remaining_data_hex,
        decoded: decoded.value,
        rows,
    }
}

pub fn decode_event_data_hex_with_idl(
    idl_json: &str,
    event_name: Option<&str>,
    data_hex: &str,
) -> Result<EventIdlDecodeReport> {
    let bytes = parse_hex_bytes(data_hex).context("failed to parse event data hex")?;
    decode_event_data_with_idl(idl_json, event_name, &bytes)
}

pub fn decode_event_data_with_idl(
    idl_json: &str,
    event_name: Option<&str>,
    data: &[u8],
) -> Result<EventIdlDecodeReport> {
    let idl: Value = serde_json::from_str(idl_json).context("failed to parse IDL JSON")?;
    let event = select_idl_event(&idl, event_name)?;
    let event = decode_idl_event(&idl, event, data)?;
    Ok(event)
}

pub fn decode_instruction_words_with_idl(
    idl_json: &str,
    program_id: &str,
    instruction_words: &[u32],
    account_ids: &[String],
) -> Result<InstructionDecodeReport> {
    let idl: Value = serde_json::from_str(idl_json).context("failed to parse IDL JSON")?;
    if let Some(instruction_type) = idl
        .get("instruction_type")
        .filter(|value| !value.is_null() && !value.is_string())
    {
        bail!(
            "IDL uses external instruction_type `{}`; positional instruction decode is unsafe without explicit variant metadata",
            idl_type_label(instruction_type)
        );
    }
    let variant_index = *instruction_words
        .first()
        .context("instruction data is empty")?;
    let instructions = idl
        .get("instructions")
        .and_then(Value::as_array)
        .context("IDL has no instructions array")?;
    let instruction = instructions
        .get(variant_index as usize)
        .with_context(|| format!("IDL instruction variant {variant_index} not found"))?;
    let instruction_name = instruction
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_owned();

    let mut accounts = Vec::new();
    for (index, account) in instruction
        .get("accounts")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .enumerate()
    {
        let path = account
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| format!("account_{index}"));
        let value = account_ids
            .get(index)
            .cloned()
            .unwrap_or_else(|| "-".to_owned());
        accounts.push(DecodedField { path, value });
    }
    for (index, account_id) in account_ids.iter().enumerate().skip(accounts.len()) {
        accounts.push(DecodedField {
            path: format!("extra_{index}"),
            value: account_id.clone(),
        });
    }

    let mut offset = 1;
    let mut args = Vec::new();
    let mut decode_error = None;
    for arg in instruction
        .get("args")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
    {
        let name = arg.get("name").and_then(Value::as_str).unwrap_or("arg");
        let Some(ty) = arg.get("type") else {
            args.push(DecodedField {
                path: name.to_owned(),
                value: "missing type".to_owned(),
            });
            continue;
        };

        match decode_instruction_type(ty, instruction_words, offset, 0) {
            Ok(decoded) => {
                args.push(DecodedField {
                    path: format!("{name}: {}", decoded.type_label),
                    value: decoded.value,
                });
                offset += decoded.consumed;
            }
            Err(err) => {
                let error = format!("{err:#}");
                args.push(DecodedField {
                    path: format!("{name}: {}", idl_type_label(ty)),
                    value: format!(
                        "unsupported ({error}); raw words {}..{}",
                        offset,
                        instruction_words.len().saturating_sub(1)
                    ),
                });
                decode_error = Some(error);
                break;
            }
        }
    }

    Ok(InstructionDecodeReport {
        program_id: program_id.to_owned(),
        idl_name: idl
            .get("name")
            .and_then(Value::as_str)
            .map(ToOwned::to_owned),
        instruction: instruction_name,
        variant_index,
        accounts,
        args,
        decode_error,
        remaining_words: instruction_words.get(offset..).unwrap_or_default().to_vec(),
    })
}

fn select_idl_event<'a>(idl: &'a Value, event_name: Option<&str>) -> Result<&'a Value> {
    if !idl_event_extension_enabled(idl) {
        bail!(
            "event decode requires explicit nonstandard events extension; pinned SPEL IDL has no event schema"
        );
    }
    let events = idl
        .get("events")
        .and_then(Value::as_array)
        .context("IDL has no events array")?;
    if events.is_empty() {
        bail!("IDL events array is empty");
    }

    let selected = event_name.map(str::trim).filter(|value| !value.is_empty());
    if let Some(selected) = selected {
        return events
            .iter()
            .find(|event| event.get("name").and_then(Value::as_str) == Some(selected))
            .with_context(|| format!("IDL event `{selected}` not found"));
    }

    if events.len() == 1 {
        return events
            .first()
            .context("IDL events array is empty after length check");
    }

    let names = events
        .iter()
        .map(idl_event_name)
        .collect::<Vec<_>>()
        .join(", ");
    bail!(
        "event name required because IDL has {} events: {names}",
        events.len()
    )
}

fn idl_event_extension_enabled(idl: &Value) -> bool {
    idl.get("x-logos-inspector-events").and_then(Value::as_bool) == Some(true)
        || idl
            .get("extensions")
            .and_then(|extensions| extensions.get("logos_inspector_events"))
            .and_then(Value::as_bool)
            == Some(true)
}

fn decode_idl_event(idl: &Value, event: &Value, data: &[u8]) -> Result<EventIdlDecodeReport> {
    let name = idl_event_name(event).to_owned();
    let decoded = if let Some(ty) = event.get("type") {
        decode_borsh_type(ty, data, 0, idl, 0)
    } else if let Some(fields) = event.get("fields").and_then(Value::as_array) {
        let shape = json!({
            "kind": "struct",
            "fields": fields,
        });
        decode_borsh_shape(&shape, data, 0, idl, 0)
    } else {
        bail!("IDL event `{name}` must have a `type` shape or `fields` array")
    }
    .with_context(|| {
        format!(
            "failed to decode IDL event `{name}`; event data is assumed to be raw Borsh payload with no discriminator"
        )
    })?;

    if decoded.consumed != data.len() {
        bail!(
            "IDL event `{name}` decoded {} of {} bytes; event data is assumed to be raw Borsh payload with no discriminator",
            decoded.consumed,
            data.len()
        );
    }

    let mut rows = Vec::new();
    flatten_decoded_value(&decoded.value, "", &mut rows);
    Ok(EventIdlDecodeReport {
        event: name,
        consumed_bytes: decoded.consumed,
        total_bytes: data.len(),
        decoded: decoded.value,
        rows,
    })
}

fn idl_event_name(event: &Value) -> &str {
    event
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

fn flatten_decoded_value(value: &Value, prefix: &str, rows: &mut Vec<DecodedField>) {
    match value {
        Value::Object(object) => {
            if let Some(variant) = object.get("variant") {
                rows.push(DecodedField {
                    path: prefixed(prefix, "variant"),
                    value: value_to_string(variant),
                });
                if let Some(fields) = object.get("fields") {
                    flatten_decoded_value(fields, prefix, rows);
                }
                return;
            }

            if object.is_empty() {
                rows.push(DecodedField {
                    path: prefix_or_value(prefix),
                    value: "{}".to_owned(),
                });
                return;
            }

            for (key, child) in object {
                flatten_decoded_value(child, &prefixed(prefix, key), rows);
            }
        }
        Value::Array(items) => {
            if items.is_empty() {
                rows.push(DecodedField {
                    path: prefix_or_value(prefix),
                    value: "[]".to_owned(),
                });
                return;
            }

            for (index, child) in items.iter().enumerate() {
                flatten_decoded_value(
                    child,
                    &format!("{}[{index}]", prefix_or_value(prefix)),
                    rows,
                );
            }
        }
        _ => rows.push(DecodedField {
            path: prefix_or_value(prefix),
            value: value_to_string(value),
        }),
    }
}

fn prefixed(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_owned()
    } else {
        format!("{prefix}.{key}")
    }
}

fn prefix_or_value(prefix: &str) -> String {
    if prefix.is_empty() {
        "value".to_owned()
    } else {
        prefix.to_owned()
    }
}
