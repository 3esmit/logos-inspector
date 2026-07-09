use std::collections::{HashMap, HashSet};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use super::super::content_area_enabled;
use super::ItemAreaReport;

pub(super) struct MergeArrayPlan {
    pub(super) rows: Vec<Value>,
    pub(super) report: ItemAreaReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConflictDecision {
    ReplaceExisting,
    SkipBackupItem,
}

pub(super) fn merge_array_by_key(
    current: Option<&Value>,
    incoming: Option<&Value>,
    key_fn: fn(&Value) -> String,
    label_fn: fn(&Value) -> String,
    area: &str,
    options: Option<&Value>,
    require_conflict_decisions: bool,
) -> Result<MergeArrayPlan> {
    let mut rows = Vec::new();
    let mut index_by_key = HashMap::new();
    for value in current.and_then(Value::as_array).into_iter().flatten() {
        let key = key_fn(value);
        if !key.is_empty() {
            index_by_key.entry(key).or_insert(rows.len());
        }
        rows.push(value.clone());
    }
    let selected_keys = selected_item_keys(options, area)?;
    let mut report = ItemAreaReport::default();
    for value in incoming.and_then(Value::as_array).into_iter().flatten() {
        let key = key_fn(value);
        if key.is_empty() {
            continue;
        }
        let selected = item_is_selected(selected_keys.as_ref(), &key);
        let label = label_fn(value);
        let Some(index) = index_by_key.get(&key).copied() else {
            report.items.push(item_identity(
                area,
                &key,
                &label,
                selected,
                false,
                false,
                if selected {
                    "import"
                } else {
                    "skip_backup_item"
                },
            ));
            if selected {
                index_by_key.insert(key, rows.len());
                rows.push(value.clone());
            }
            continue;
        };
        let conflict = rows.get(index).is_some_and(|current| current != value);
        if !conflict {
            report.items.push(item_identity(
                area,
                &key,
                &label,
                selected,
                true,
                false,
                if selected {
                    "keep_existing"
                } else {
                    "skip_backup_item"
                },
            ));
            continue;
        }
        if !selected {
            report.items.push(item_identity(
                area,
                &key,
                &label,
                false,
                true,
                true,
                "skip_backup_item",
            ));
            continue;
        }
        match conflict_decision(options, area, &key)? {
            Some(ConflictDecision::ReplaceExisting) => {
                if let Some(row) = rows.get_mut(index) {
                    *row = value.clone();
                }
                report.items.push(item_identity(
                    area,
                    &key,
                    &label,
                    true,
                    true,
                    true,
                    "replace_existing",
                ));
                report
                    .conflicts
                    .push(conflict_identity(area, &key, &label, "replace_existing"));
            }
            Some(ConflictDecision::SkipBackupItem) => {
                report.items.push(item_identity(
                    area,
                    &key,
                    &label,
                    true,
                    true,
                    true,
                    "skip_backup_item",
                ));
                report
                    .conflicts
                    .push(conflict_identity(area, &key, &label, "skip_backup_item"));
            }
            None => {
                if require_conflict_decisions {
                    bail!("backup import conflict decision is required for {area} item `{key}`");
                }
                report.items.push(item_identity(
                    area, &key, &label, true, true, true, "required",
                ));
                report
                    .conflicts
                    .push(conflict_identity(area, &key, &label, "required"));
            }
        }
    }
    Ok(MergeArrayPlan { rows, report })
}

pub(super) fn merge_object_field(
    current: Option<&Value>,
    incoming: &Value,
    area: &str,
    options: Option<&Value>,
    require_conflict_decisions: bool,
    report: &mut ItemAreaReport,
) -> Result<Value> {
    let mut result = current.cloned().unwrap_or_else(|| json!({}));
    let Some(result_object) = result.as_object_mut() else {
        return Ok(incoming.clone());
    };
    if let Some(incoming_object) = incoming.as_object() {
        for (key, value) in incoming_object {
            let selected = item_is_selected(selected_item_keys(options, area)?.as_ref(), key);
            let label = format!("Account selection {key}");
            let conflict = result_object
                .get(key)
                .is_some_and(|current| current != value);
            if !selected {
                continue;
            }
            if conflict {
                match conflict_decision(options, area, key)? {
                    Some(ConflictDecision::ReplaceExisting) => {
                        report.conflicts.push(conflict_identity(
                            area,
                            key,
                            &label,
                            "replace_existing",
                        ));
                    }
                    Some(ConflictDecision::SkipBackupItem) => {
                        report.conflicts.push(conflict_identity(
                            area,
                            key,
                            &label,
                            "skip_backup_item",
                        ));
                        continue;
                    }
                    None => {
                        if require_conflict_decisions {
                            bail!(
                                "backup import conflict decision is required for {area} item `{key}`"
                            );
                        }
                        report
                            .conflicts
                            .push(conflict_identity(area, key, &label, "required"));
                        continue;
                    }
                }
            }
            result_object.insert(key.clone(), value.clone());
        }
    }
    Ok(result)
}

pub(super) fn item_report_for_array(
    area: &str,
    incoming: Option<&Value>,
    key_fn: fn(&Value) -> String,
    label_fn: fn(&Value) -> String,
) -> ItemAreaReport {
    let mut report = ItemAreaReport::default();
    for value in incoming.and_then(Value::as_array).into_iter().flatten() {
        let key = key_fn(value);
        if key.is_empty() {
            continue;
        }
        report.items.push(item_identity(
            area,
            &key,
            &label_fn(value),
            true,
            false,
            false,
            "import",
        ));
    }
    report
}

fn selected_item_keys(options: Option<&Value>, area: &str) -> Result<Option<HashSet<String>>> {
    let Some(items) = options
        .and_then(|value| value.get("items").or_else(|| value.get("selected_items")))
        .and_then(|value| value.get(area))
    else {
        return Ok(None);
    };
    match items {
        Value::Array(values) => {
            let mut keys = HashSet::new();
            for value in values {
                let key = item_option_key(value)?;
                if !key.is_empty() {
                    keys.insert(key);
                }
            }
            Ok(Some(keys))
        }
        Value::Object(object) => {
            let mut keys = HashSet::new();
            for (key, value) in object {
                if content_area_enabled(Some(value), key)? {
                    keys.insert(key.to_owned());
                }
            }
            Ok(Some(keys))
        }
        _ => bail!("backup import item selection for `{area}` must be an array or object"),
    }
}

fn item_option_key(value: &Value) -> Result<String> {
    if let Some(key) = value.as_str() {
        return Ok(key.trim().to_owned());
    }
    if let Some(key) = value.get("key").and_then(Value::as_str) {
        return Ok(key.trim().to_owned());
    }
    bail!("backup import selected item must be a key string or object")
}

fn item_is_selected(selected_keys: Option<&HashSet<String>>, key: &str) -> bool {
    selected_keys.is_none_or(|keys| keys.contains(key))
}

fn conflict_decision(
    options: Option<&Value>,
    area: &str,
    key: &str,
) -> Result<Option<ConflictDecision>> {
    let Some(value) = options
        .and_then(|value| {
            value
                .get("conflicts")
                .or_else(|| value.get("conflict_decisions"))
                .or_else(|| value.get("conflictDecisions"))
        })
        .and_then(|value| value.get(area))
        .and_then(|value| value.get(key))
    else {
        return Ok(None);
    };
    let text = value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("backup import conflict decision must be a string")?;
    match text.to_lowercase().replace('-', "_").as_str() {
        "replace" | "replace_existing" | "use_backup" => {
            Ok(Some(ConflictDecision::ReplaceExisting))
        }
        "skip" | "skip_backup_item" | "keep_existing" => Ok(Some(ConflictDecision::SkipBackupItem)),
        other => bail!("unsupported backup import conflict decision `{other}`"),
    }
}

fn item_identity(
    area: &str,
    key: &str,
    label: &str,
    selected: bool,
    exists: bool,
    conflict: bool,
    decision: &str,
) -> Value {
    json!({
        "area": area,
        "key": key,
        "label": if label.trim().is_empty() { key } else { label },
        "selected": selected,
        "exists": exists,
        "conflict": conflict,
        "decision": decision
    })
}

fn conflict_identity(area: &str, key: &str, label: &str, decision: &str) -> Value {
    json!({
        "area": area,
        "key": key,
        "label": if label.trim().is_empty() { key } else { label },
        "decision": decision,
        "reason": "same_key_different_value"
    })
}

pub(super) fn favorite_entry_key(value: &Value) -> String {
    let kind = value_string(value, &["kind"]).to_lowercase();
    let layer = value_string(value, &["layer"]).to_lowercase();
    let open_kind = value_string(value, &["open_kind", "openKind"]).to_lowercase();
    let item_value = value_string(value, &["value"]).to_lowercase();
    if kind.is_empty() || item_value.is_empty() {
        return json_value_key("favorite", value);
    }
    format!("{kind}:{layer}:{open_kind}:{item_value}")
}

pub(super) fn favorite_entry_label(value: &Value) -> String {
    value_string(value, &["label", "title", "name", "value"])
}

pub(super) fn idl_entry_key(value: &Value) -> String {
    for keys in [
        &["key", "id"][..],
        &["programIdHex", "program_id_hex"][..],
        &["programId", "program_id"][..],
        &["name", "programName"][..],
    ] {
        let key = value_string(value, keys);
        if !key.is_empty() {
            return key.to_lowercase();
        }
    }
    json_value_key("idl", value)
}

pub(super) fn idl_entry_label(value: &Value) -> String {
    value_string(
        value,
        &[
            "name",
            "programName",
            "program_id_hex",
            "programIdHex",
            "key",
        ],
    )
}

fn value_string(value: &Value, keys: &[&str]) -> String {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(Value::as_str))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or_default()
        .to_owned()
}

fn json_value_key(prefix: &str, value: &Value) -> String {
    serde_json::to_string(value)
        .map(|text| format!("{prefix}:{text}"))
        .unwrap_or_else(|_| format!("{prefix}:unserializable"))
}
