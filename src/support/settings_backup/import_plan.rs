use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::support::state_store::{load_idl_state, load_settings_state};

use super::{RestoredState, content_area_enabled, set_object_field};

pub(super) struct ImportPlan {
    pub(super) settings: Option<Value>,
    pub(super) idl: Option<Value>,
    pub(super) wallet: Option<Value>,
    pub(super) summary: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ImportMode {
    Replace,
    Merge,
    Skip,
}

pub(super) struct ImportSelection {
    pub(super) settings: ImportMode,
    pub(super) favorites: ImportMode,
    pub(super) idl_registry: ImportMode,
    pub(super) wallet_profile: ImportMode,
}

#[derive(Default)]
pub(super) struct ImportItemReport {
    pub(super) favorites: ItemAreaReport,
    pub(super) idl_registry: ItemAreaReport,
}

#[derive(Default)]
pub(super) struct ItemAreaReport {
    pub(super) items: Vec<Value>,
    pub(super) conflicts: Vec<Value>,
}

pub(super) struct PlannedSettingsState {
    pub(super) state: Option<Value>,
    pub(super) favorites: ItemAreaReport,
}

pub(super) struct PlannedIdlState {
    pub(super) state: Option<Value>,
    pub(super) idl_registry: ItemAreaReport,
}

struct MergeArrayPlan {
    rows: Vec<Value>,
    report: ItemAreaReport,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConflictDecision {
    ReplaceExisting,
    SkipBackupItem,
}

impl ImportSelection {
    fn from_value(options: Option<&Value>) -> Result<Self> {
        let selection = Self {
            settings: replace_only_mode_setting(
                options,
                &["settings", "app_settings"],
                ImportMode::Skip,
            )?,
            favorites: mode_setting(options, &["favorites"], ImportMode::Skip)?,
            idl_registry: mode_setting(
                options,
                &["idl_registry", "idls", "idl"],
                ImportMode::Skip,
            )?,
            wallet_profile: replace_only_mode_setting(
                options,
                &["wallet_profile", "wallet"],
                ImportMode::Skip,
            )?,
        };
        Ok(selection)
    }
}

pub(super) fn build_import_plan(
    state: &RestoredState,
    options: Option<&Value>,
    require_conflict_decisions: bool,
) -> Result<ImportPlan> {
    let selection = ImportSelection::from_value(options)?;
    let current_settings = needs_current_settings(&selection, state)
        .then(load_settings_state)
        .transpose()
        .context("failed to load current settings for backup import")?;
    let current_idl = (selection.idl_registry == ImportMode::Merge)
        .then(load_idl_state)
        .transpose()
        .context("failed to load current IDL state for backup import")?;

    let planned_settings = planned_settings_state(
        state.settings.as_ref(),
        current_settings.as_ref(),
        &selection,
        options,
        require_conflict_decisions,
    )?;
    let planned_idl = planned_idl_state(
        state.idl.as_ref(),
        current_idl.as_ref(),
        selection.idl_registry,
        options,
        require_conflict_decisions,
    )?;
    let wallet = if selection.wallet_profile != ImportMode::Skip {
        state.wallet.clone()
    } else {
        None
    };
    let warnings = wallet_import_warnings(state.wallet.as_ref(), selection.wallet_profile);
    let settings = planned_settings.state;
    let idl = planned_idl.state;

    let settings_applied = settings.is_some();
    let idl_applied = idl.is_some();
    let wallet_applied = wallet.is_some();
    let favorites_count = planned_favorites_count(
        settings.as_ref(),
        state.summary.favorites_count,
        selection.favorites,
    );
    let idl_count = planned_idl_count(
        idl.as_ref(),
        state.summary.idl_count,
        selection.idl_registry,
    );
    let item_report = ImportItemReport {
        favorites: planned_settings.favorites,
        idl_registry: planned_idl.idl_registry,
    };
    Ok(ImportPlan {
        settings,
        idl,
        wallet,
        summary: json!({
            "restored": settings_applied || idl_applied || wallet_applied,
            "encrypted": state.summary.encrypted,
            "settings": settings_applied,
            "idls": idl_applied,
            "wallet": wallet_applied,
            "favorites": favorites_count,
            "idl_count": idl_count,
            "modes": {
                "settings": mode_name(selection.settings),
                "favorites": mode_name(selection.favorites),
                "idl_registry": mode_name(selection.idl_registry),
                "wallet_profile": mode_name(selection.wallet_profile)
            },
            "items": {
                "favorites": item_report.favorites.items,
                "idl_registry": item_report.idl_registry.items
            },
            "conflicts": {
                "favorites": item_report.favorites.conflicts,
                "idl_registry": item_report.idl_registry.conflicts
            },
            "warnings": warnings,
            "applied_areas": applied_areas(settings_applied, idl_applied, wallet_applied, favorites_count),
            "operation_policy": {
                "restart_policy": "safe_read_poll_only",
                "auto_restart_classes": ["read", "poll", "probe"],
                "manual_restart_classes": ["mutating", "signing", "submission", "lifecycle", "destructive", "backup"]
            }
        }),
    })
}

fn wallet_import_warnings(wallet: Option<&Value>, mode: ImportMode) -> Vec<Value> {
    if mode == ImportMode::Skip {
        return Vec::new();
    }
    let Some(wallet) = wallet else {
        return vec![warning_value(
            "wallet_profile",
            "profile",
            "Backup does not contain a wallet profile.",
        )];
    };
    let Some(profile) = wallet_profile_payload(wallet) else {
        return vec![warning_value(
            "wallet_profile",
            "profile",
            "Backup wallet profile is not an object.",
        )];
    };

    let mut warnings = Vec::new();
    append_wallet_path_warning(
        &mut warnings,
        profile,
        &["wallet_home", "walletHome"],
        "wallet_home",
        "Wallet home path from backup",
        true,
    );
    append_wallet_path_warning(
        &mut warnings,
        profile,
        &["wallet_binary", "walletBinary"],
        "wallet_binary",
        "Wallet binary path from backup",
        false,
    );
    warnings
}

fn wallet_profile_payload(wallet: &Value) -> Option<&Value> {
    wallet
        .get("profile")
        .filter(|value| value.is_object())
        .or_else(|| wallet.is_object().then_some(wallet))
}

fn append_wallet_path_warning(
    warnings: &mut Vec<Value>,
    profile: &Value,
    keys: &[&str],
    key: &str,
    label: &str,
    expect_directory: bool,
) {
    let Some(path_text) = first_non_empty_string(profile, keys) else {
        return;
    };
    let path = Path::new(&path_text);
    if path.is_relative() {
        warnings.push(warning_value(
            "wallet_profile",
            key,
            &format!("{label} is relative and may resolve differently."),
        ));
        return;
    }
    let exists = if expect_directory {
        path.is_dir()
    } else {
        path.is_file()
    };
    if !exists {
        warnings.push(warning_value(
            "wallet_profile",
            key,
            &format!("{label} does not exist locally."),
        ));
    }
}

fn first_non_empty_string(value: &Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

fn warning_value(area: &str, key: &str, message: &str) -> Value {
    json!({
        "area": area,
        "key": key,
        "severity": "warning",
        "message": message
    })
}

pub(super) fn planned_settings_state(
    backup_settings: Option<&Value>,
    current_settings: Option<&Value>,
    selection: &ImportSelection,
    options: Option<&Value>,
    require_conflict_decisions: bool,
) -> Result<PlannedSettingsState> {
    let Some(backup_settings) = backup_settings else {
        return Ok(PlannedSettingsState {
            state: None,
            favorites: ItemAreaReport::default(),
        });
    };
    let mut target = match selection.settings {
        ImportMode::Replace => Some(backup_settings.clone()),
        ImportMode::Merge => Some(merge_settings(current_settings, backup_settings)?),
        ImportMode::Skip => None,
    };
    let mut favorites_report = match selection.favorites {
        ImportMode::Replace | ImportMode::Merge => item_report_for_array(
            "favorites",
            backup_settings.get("favorites"),
            favorite_entry_key,
            favorite_entry_label,
        ),
        ImportMode::Skip => ItemAreaReport::default(),
    };
    match selection.favorites {
        ImportMode::Replace => {
            if selection.settings == ImportMode::Skip {
                let mut value = current_settings.cloned().unwrap_or_else(|| json!({}));
                set_object_field(
                    &mut value,
                    "favorites",
                    backup_settings
                        .get("favorites")
                        .cloned()
                        .unwrap_or_else(|| json!([])),
                )?;
                target = Some(value);
            }
        }
        ImportMode::Merge => {
            let plan = merge_array_by_key(
                current_settings.and_then(|value| value.get("favorites")),
                backup_settings.get("favorites"),
                favorite_entry_key,
                favorite_entry_label,
                "favorites",
                options,
                require_conflict_decisions,
            )?;
            favorites_report = plan.report;
            let mut value = target
                .take()
                .or_else(|| current_settings.cloned())
                .unwrap_or_else(|| json!({}));
            set_object_field(&mut value, "favorites", Value::Array(plan.rows))?;
            target = Some(value);
        }
        ImportMode::Skip => {
            if let Some(value) = target.as_mut()
                && let Some(current_favorites) =
                    current_settings.and_then(|value| value.get("favorites"))
            {
                set_object_field(value, "favorites", current_favorites.clone())?;
            }
        }
    }
    Ok(PlannedSettingsState {
        state: target,
        favorites: favorites_report,
    })
}

pub(super) fn planned_idl_state(
    backup_idl: Option<&Value>,
    current_idl: Option<&Value>,
    mode: ImportMode,
    options: Option<&Value>,
    require_conflict_decisions: bool,
) -> Result<PlannedIdlState> {
    let Some(backup_idl) = backup_idl else {
        return Ok(PlannedIdlState {
            state: None,
            idl_registry: ItemAreaReport::default(),
        });
    };
    let mut idl_report = match mode {
        ImportMode::Replace | ImportMode::Merge => item_report_for_array(
            "idl_registry",
            backup_idl.get("idls"),
            idl_entry_key,
            idl_entry_label,
        ),
        ImportMode::Skip => ItemAreaReport::default(),
    };
    let state = match mode {
        ImportMode::Replace => Some(backup_idl.clone()),
        ImportMode::Merge => {
            let mut value = current_idl.cloned().unwrap_or_else(|| json!({}));
            let plan = merge_array_by_key(
                current_idl.and_then(|value| value.get("idls")),
                backup_idl.get("idls"),
                idl_entry_key,
                idl_entry_label,
                "idl_registry",
                options,
                require_conflict_decisions,
            )?;
            idl_report = plan.report;
            set_object_field(&mut value, "idls", Value::Array(plan.rows))?;
            if let Some(backup_selections) = backup_idl.get("account_idl_selections") {
                let merged = merge_object_field(
                    current_idl.and_then(|value| value.get("account_idl_selections")),
                    backup_selections,
                    "idl_registry",
                    options,
                    require_conflict_decisions,
                    &mut idl_report,
                )?;
                set_object_field(&mut value, "account_idl_selections", merged)?;
            }
            Some(value)
        }
        ImportMode::Skip => None,
    };
    Ok(PlannedIdlState {
        state,
        idl_registry: idl_report,
    })
}

fn merge_settings(current: Option<&Value>, backup: &Value) -> Result<Value> {
    let mut value = current.cloned().unwrap_or_else(|| json!({}));
    if let Some(object) = backup.as_object() {
        for (key, item) in object {
            if key != "favorites" {
                set_object_field(&mut value, key, item.clone())?;
            }
        }
    }
    Ok(value)
}

fn merge_array_by_key(
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

fn merge_object_field(
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

fn item_report_for_array(
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

fn favorite_entry_key(value: &Value) -> String {
    let kind = value_string(value, &["kind"]).to_lowercase();
    let layer = value_string(value, &["layer"]).to_lowercase();
    let open_kind = value_string(value, &["open_kind", "openKind"]).to_lowercase();
    let item_value = value_string(value, &["value"]).to_lowercase();
    if kind.is_empty() || item_value.is_empty() {
        return json_value_key("favorite", value);
    }
    format!("{kind}:{layer}:{open_kind}:{item_value}")
}

fn favorite_entry_label(value: &Value) -> String {
    value_string(value, &["label", "title", "name", "value"])
}

fn idl_entry_key(value: &Value) -> String {
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

fn idl_entry_label(value: &Value) -> String {
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

fn needs_current_settings(selection: &ImportSelection, state: &RestoredState) -> bool {
    state.settings.is_some()
        && (selection.settings == ImportMode::Merge
            || selection.favorites == ImportMode::Merge
            || selection.favorites == ImportMode::Skip
            || selection.settings == ImportMode::Skip)
}

fn planned_favorites_count(
    settings: Option<&Value>,
    fallback_count: usize,
    favorites_mode: ImportMode,
) -> usize {
    if favorites_mode == ImportMode::Skip {
        return 0;
    }
    settings
        .and_then(|value| value.get("favorites"))
        .and_then(Value::as_array)
        .map_or(fallback_count, Vec::len)
}

fn planned_idl_count(idl: Option<&Value>, fallback_count: usize, idl_mode: ImportMode) -> usize {
    if idl_mode == ImportMode::Skip {
        return 0;
    }
    idl.and_then(|value| value.get("idls"))
        .and_then(Value::as_array)
        .map_or(fallback_count, Vec::len)
}

fn applied_areas(
    settings_applied: bool,
    idl_applied: bool,
    wallet_applied: bool,
    favorites_count: usize,
) -> Vec<&'static str> {
    let mut areas = Vec::new();
    if settings_applied {
        areas.push("settings");
    }
    if favorites_count > 0 {
        areas.push("favorites");
    }
    if idl_applied {
        areas.push("idl_registry");
    }
    if wallet_applied {
        areas.push("wallet_profile");
    }
    areas
}

fn mode_setting(
    options: Option<&Value>,
    keys: &[&str],
    default_mode: ImportMode,
) -> Result<ImportMode> {
    let Some(options) = options.and_then(Value::as_object) else {
        return Ok(default_mode);
    };
    for key in keys {
        if let Some(value) = options.get(*key) {
            return parse_import_mode(value, key);
        }
    }
    Ok(default_mode)
}

fn replace_only_mode_setting(
    options: Option<&Value>,
    keys: &[&str],
    default_mode: ImportMode,
) -> Result<ImportMode> {
    let mode = mode_setting(options, keys, default_mode)?;
    if mode == ImportMode::Merge {
        bail!(
            "backup import mode `merge` is not supported for `{}`",
            keys.first().copied().unwrap_or("area")
        );
    }
    Ok(mode)
}

fn parse_import_mode(value: &Value, key: &str) -> Result<ImportMode> {
    match value
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_lowercase()
        .as_str()
    {
        "" | "replace" => Ok(ImportMode::Replace),
        "merge" => Ok(ImportMode::Merge),
        "skip" | "none" | "not_import" | "not import" => Ok(ImportMode::Skip),
        other => bail!("unsupported backup import mode `{other}` for `{key}`"),
    }
}

fn mode_name(mode: ImportMode) -> &'static str {
    match mode {
        ImportMode::Replace => "replace",
        ImportMode::Merge => "merge",
        ImportMode::Skip => "skip",
    }
}
