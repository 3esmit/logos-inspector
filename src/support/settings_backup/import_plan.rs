use std::path::Path;

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use crate::source_routing::channel_sources::normalized_settings_state_from_backup;

#[cfg(test)]
use crate::support::state_store::{load_idl_state, load_settings_state};

use super::{
    BackupImportArea, BackupImportMode, BackupImportOptions, BackupImportSelection, RestoredState,
    import_options::ItemAreaReport, set_object_field,
};

mod merge_engine;

use merge_engine::{
    favorite_entry_key, favorite_entry_label, idl_entry_key, idl_entry_label,
    item_report_for_array, merge_array_by_key, merge_object_field,
};

pub(super) struct ImportPlan {
    pub(super) settings: Option<Value>,
    pub(super) idl: Option<Value>,
    pub(super) wallet: Option<Value>,
    pub(super) applied_areas: Vec<BackupImportArea>,
    pub(super) summary: Value,
}

pub(super) struct ImportCurrentRequirements {
    pub(super) settings: bool,
    pub(super) idl: bool,
}

#[derive(Default)]
pub(super) struct ImportItemReport {
    pub(super) favorites: ItemAreaReport,
    pub(super) idl_registry: ItemAreaReport,
}

pub(super) struct PlannedSettingsState {
    pub(super) state: Option<Value>,
    pub(super) favorites: ItemAreaReport,
}

pub(super) struct PlannedIdlState {
    pub(super) state: Option<Value>,
    pub(super) idl_registry: ItemAreaReport,
}

#[cfg(test)]
pub(super) fn build_import_plan(
    state: &RestoredState,
    options: &BackupImportOptions,
    require_conflict_decisions: bool,
) -> Result<ImportPlan> {
    let selection = options.selection();
    let current_settings = needs_current_settings(selection, state)
        .then(load_settings_state)
        .transpose()
        .context("failed to load current settings for backup import")?;
    let current_idl = (selection.mode(BackupImportArea::IdlRegistry) == BackupImportMode::Merge)
        .then(load_idl_state)
        .transpose()
        .context("failed to load current IDL state for backup import")?;

    build_import_plan_with_current(
        state,
        options,
        require_conflict_decisions,
        current_settings.as_ref(),
        current_idl.as_ref(),
    )
}

pub(super) fn build_import_plan_with_current(
    state: &RestoredState,
    options: &BackupImportOptions,
    require_conflict_decisions: bool,
    current_settings: Option<&Value>,
    current_idl: Option<&Value>,
) -> Result<ImportPlan> {
    let selection = options.selection();

    let planned_settings = planned_settings_state(
        state.settings.as_ref(),
        current_settings,
        options,
        require_conflict_decisions,
    )?;
    let planned_idl = planned_idl_state(
        state.idl.as_ref(),
        current_idl,
        options,
        require_conflict_decisions,
    )?;
    let wallet = if selection.mode(BackupImportArea::WalletProfile) != BackupImportMode::Skip {
        state.wallet.clone()
    } else {
        None
    };
    let warnings = wallet_import_warnings(
        state.wallet.as_ref(),
        selection.mode(BackupImportArea::WalletProfile),
    );
    let settings = planned_settings
        .state
        .as_ref()
        .map(|settings| {
            normalized_settings_state_from_backup(
                settings,
                current_settings.context(
                    "current settings are required to rebase Channel source configuration revisions",
                )?,
            )
        })
        .transpose()?;
    let idl = planned_idl.state;

    let settings_applied = settings.is_some();
    let idl_applied = idl.is_some();
    let wallet_applied = wallet.is_some();
    let favorites_count = planned_favorites_count(
        settings.as_ref(),
        state.summary.favorites_count,
        selection.mode(BackupImportArea::Favorites),
    );
    let idl_count = planned_idl_count(
        idl.as_ref(),
        state.summary.idl_count,
        selection.mode(BackupImportArea::IdlRegistry),
    );
    let applied_areas = selection.applied_areas(
        state.settings.is_some() && selection.mode(BackupImportArea::Settings).is_selected(),
        state.settings.is_some() && selection.mode(BackupImportArea::Favorites).is_selected(),
        idl_applied,
        wallet_applied,
    );
    let item_report = ImportItemReport {
        favorites: planned_settings.favorites,
        idl_registry: planned_idl.idl_registry,
    };
    Ok(ImportPlan {
        settings,
        idl,
        wallet,
        applied_areas: applied_areas.clone(),
        summary: json!({
            "restored": settings_applied || idl_applied || wallet_applied,
            "encrypted": state.summary.encrypted,
            "settings": settings_applied,
            "idls": idl_applied,
            "wallet": wallet_applied,
            "favorites": favorites_count,
            "idl_count": idl_count,
            "modes": {
                "settings": selection.mode(BackupImportArea::Settings).as_str(),
                "favorites": selection.mode(BackupImportArea::Favorites).as_str(),
                "idl_registry": selection.mode(BackupImportArea::IdlRegistry).as_str(),
                "wallet_profile": selection.mode(BackupImportArea::WalletProfile).as_str()
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
            "applied_areas": applied_areas.into_iter().map(BackupImportArea::as_str).collect::<Vec<_>>(),
            "operation_policy": {
                "restart_policy": "safe_read_poll_only",
                "auto_restart_classes": ["read", "poll", "probe"],
                "manual_restart_classes": ["mutating", "signing", "submission", "lifecycle", "destructive", "backup"]
            }
        }),
    })
}

pub(super) fn current_state_requirements(
    state: &RestoredState,
    options: &BackupImportOptions,
) -> ImportCurrentRequirements {
    let selection = options.selection();
    ImportCurrentRequirements {
        settings: needs_current_settings(selection, state),
        idl: selection.mode(BackupImportArea::IdlRegistry) == BackupImportMode::Merge,
    }
}

fn wallet_import_warnings(wallet: Option<&Value>, mode: BackupImportMode) -> Vec<Value> {
    if mode == BackupImportMode::Skip {
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
    options: &BackupImportOptions,
    require_conflict_decisions: bool,
) -> Result<PlannedSettingsState> {
    let selection = options.selection();
    let Some(backup_settings) = backup_settings else {
        return Ok(PlannedSettingsState {
            state: None,
            favorites: ItemAreaReport::default(),
        });
    };
    let mut target = match selection.mode(BackupImportArea::Settings) {
        BackupImportMode::Replace => Some(backup_settings.clone()),
        BackupImportMode::Merge => Some(merge_settings(current_settings, backup_settings)?),
        BackupImportMode::Skip => None,
    };
    let mut favorites_report = match selection.mode(BackupImportArea::Favorites) {
        BackupImportMode::Replace | BackupImportMode::Merge => item_report_for_array(
            BackupImportArea::Favorites,
            backup_settings.get("favorites"),
            favorite_entry_key,
            favorite_entry_label,
        ),
        BackupImportMode::Skip => ItemAreaReport::default(),
    };
    match selection.mode(BackupImportArea::Favorites) {
        BackupImportMode::Replace => {
            if selection.mode(BackupImportArea::Settings) == BackupImportMode::Skip {
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
        BackupImportMode::Merge => {
            let plan = merge_array_by_key(
                current_settings.and_then(|value| value.get("favorites")),
                backup_settings.get("favorites"),
                favorite_entry_key,
                favorite_entry_label,
                BackupImportArea::Favorites,
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
        BackupImportMode::Skip => {
            if let Some(value) = target.as_mut() {
                if let Some(current_favorites) =
                    current_settings.and_then(|value| value.get("favorites"))
                {
                    set_object_field(value, "favorites", current_favorites.clone())?;
                } else {
                    value
                        .as_object_mut()
                        .context("backup settings state must be an object")?
                        .remove("favorites");
                }
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
    options: &BackupImportOptions,
    require_conflict_decisions: bool,
) -> Result<PlannedIdlState> {
    let mode = options.mode(BackupImportArea::IdlRegistry);
    let Some(backup_idl) = backup_idl else {
        return Ok(PlannedIdlState {
            state: None,
            idl_registry: ItemAreaReport::default(),
        });
    };
    let mut idl_report = match mode {
        BackupImportMode::Replace | BackupImportMode::Merge => item_report_for_array(
            BackupImportArea::IdlRegistry,
            backup_idl.get("idls"),
            idl_entry_key,
            idl_entry_label,
        ),
        BackupImportMode::Skip => ItemAreaReport::default(),
    };
    let state = match mode {
        BackupImportMode::Replace => Some(backup_idl.clone()),
        BackupImportMode::Merge => {
            let mut value = current_idl.cloned().unwrap_or_else(|| json!({}));
            let plan = merge_array_by_key(
                current_idl.and_then(|value| value.get("idls")),
                backup_idl.get("idls"),
                idl_entry_key,
                idl_entry_label,
                BackupImportArea::IdlRegistry,
                options,
                require_conflict_decisions,
            )?;
            idl_report = plan.report;
            set_object_field(&mut value, "idls", Value::Array(plan.rows))?;
            if let Some(backup_selections) = backup_idl.get("account_idl_selections") {
                let merged = merge_object_field(
                    current_idl.and_then(|value| value.get("account_idl_selections")),
                    backup_selections,
                    BackupImportArea::IdlRegistry,
                    options,
                    require_conflict_decisions,
                    &mut idl_report,
                )?;
                set_object_field(&mut value, "account_idl_selections", merged)?;
            }
            Some(value)
        }
        BackupImportMode::Skip => None,
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

fn needs_current_settings(selection: &BackupImportSelection, state: &RestoredState) -> bool {
    state.settings.is_some()
        && (selection.mode(BackupImportArea::Settings).is_selected()
            || selection.mode(BackupImportArea::Favorites).is_selected())
}

fn planned_favorites_count(
    settings: Option<&Value>,
    fallback_count: usize,
    favorites_mode: BackupImportMode,
) -> usize {
    if favorites_mode == BackupImportMode::Skip {
        return 0;
    }
    settings
        .and_then(|value| value.get("favorites"))
        .and_then(Value::as_array)
        .map_or(fallback_count, Vec::len)
}

fn planned_idl_count(
    idl: Option<&Value>,
    fallback_count: usize,
    idl_mode: BackupImportMode,
) -> usize {
    if idl_mode == BackupImportMode::Skip {
        return 0;
    }
    idl.and_then(|value| value.get("idls"))
        .and_then(Value::as_array)
        .map_or(fallback_count, Vec::len)
}
