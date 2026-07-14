use std::collections::HashMap;

use anyhow::{Result, bail};
use serde_json::{Value, json};

use super::super::{
    BackupImportArea, BackupImportOptions,
    import_options::{ConflictDecision, ItemAreaReport},
    normalized_account_idl_selection_key,
};

pub(super) struct MergeArrayPlan {
    pub(super) rows: Vec<Value>,
    pub(super) report: ItemAreaReport,
}

pub(super) fn merge_array_by_key(
    current: Option<&Value>,
    incoming: Option<&Value>,
    key_fn: fn(&Value) -> String,
    label_fn: fn(&Value) -> String,
    area: BackupImportArea,
    options: &BackupImportOptions,
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
    let mut report = ItemAreaReport::default();
    for value in incoming.and_then(Value::as_array).into_iter().flatten() {
        let key = key_fn(value);
        if key.is_empty() {
            continue;
        }
        let selected = options.item_is_selected(area, &key);
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
        match options.conflict_decision(area, &key) {
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
                    bail!(
                        "backup import conflict decision is required for {} item `{key}`",
                        area.as_str()
                    );
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
    area: BackupImportArea,
    options: &BackupImportOptions,
    require_conflict_decisions: bool,
    report: &mut ItemAreaReport,
) -> Result<Value> {
    let mut result = current.cloned().unwrap_or_else(|| json!({}));
    let Some(result_object) = result.as_object_mut() else {
        return Ok(incoming.clone());
    };
    if let Some(incoming_object) = incoming.as_object() {
        for (key, value) in incoming_object {
            let selected_item_key =
                normalized_account_idl_selection_key(value).unwrap_or_else(|| key.to_lowercase());
            let selected = options.item_is_selected(area, &selected_item_key);
            let label = format!("Account selection {key}");
            let conflict = result_object
                .get(key)
                .is_some_and(|current| current != value);
            if !selected {
                continue;
            }
            if conflict {
                match options.conflict_decision(area, key) {
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
                                "backup import conflict decision is required for {} item `{key}`",
                                area.as_str()
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
    area: BackupImportArea,
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

fn item_identity(
    area: BackupImportArea,
    key: &str,
    label: &str,
    selected: bool,
    exists: bool,
    conflict: bool,
    decision: &str,
) -> Value {
    json!({
        "area": area.as_str(),
        "key": key,
        "label": if label.trim().is_empty() { key } else { label },
        "selected": selected,
        "exists": exists,
        "conflict": conflict,
        "decision": decision
    })
}

fn conflict_identity(area: BackupImportArea, key: &str, label: &str, decision: &str) -> Value {
    json!({
        "area": area.as_str(),
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
