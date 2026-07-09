use std::path::Path;

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};

use crate::support::state_store::{load_idl_state, load_settings_state};

use super::{RestoredState, set_object_field};

mod merge_engine;

use merge_engine::{
    favorite_entry_key, favorite_entry_label, idl_entry_key, idl_entry_label,
    item_report_for_array, merge_array_by_key, merge_object_field,
};

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
