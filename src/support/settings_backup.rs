use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead as _, KeyInit as _, Payload},
};
use hkdf::Hkdf;
use serde_json::{Value, json};
use sha2::Sha256;

use crate::{
    support::state_store::{
        load_idl_state, load_settings_state, load_wallet_state, save_idl_state,
        save_settings_state, save_wallet_state,
    },
    wallet::LOCAL_WALLET_HOME_ENV,
};

const BACKUP_KIND: &str = "logos-inspector-settings-backup";
const BACKUP_VERSION: u64 = 1;
const ENCRYPTION_SCHEME: &str = "xchacha20poly1305-wallet-config-v1";
const WALLET_CONFIG_FILE: &str = "wallet_config.json";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestoreSummary {
    pub settings_restored: bool,
    pub idl_restored: bool,
    pub wallet_restored: bool,
    pub favorites_count: usize,
    pub idl_count: usize,
    pub encrypted: bool,
}

pub(crate) fn export_app_settings_backup(
    encrypted: bool,
    wallet_profile: Option<&Value>,
    content_options: Option<&Value>,
) -> Result<Value> {
    let contents = BackupContentsSelection::from_value(content_options)?;
    let settings = (contents.settings || contents.favorites)
        .then(load_settings_state)
        .transpose()
        .context("failed to load settings state for backup")?;
    let idl = contents
        .idl_registry
        .then(load_idl_state)
        .transpose()
        .context("failed to load IDL state for backup")?;
    let wallet = contents
        .wallet_profile
        .then(load_wallet_state)
        .transpose()
        .context("failed to load wallet state for backup")?;
    let state = backup_payload_from_optional_states(
        settings.as_ref(),
        idl.as_ref(),
        wallet.as_ref(),
        encrypted,
        wallet_profile,
        &contents,
    )?;
    Ok(state)
}

pub(crate) fn preview_app_settings_backup_import(
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: Option<&Value>,
) -> Result<Value> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    let plan = build_import_plan(&state, options, false)?;
    Ok(plan.summary)
}

pub(crate) fn restore_app_settings_backup_with_options(
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: Option<&Value>,
) -> Result<Value> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    let plan = build_import_plan(&state, options, true)?;
    if let Some(settings) = plan.settings.as_ref() {
        save_settings_state(settings).context("failed to restore settings state")?;
    }
    if let Some(idl) = plan.idl.as_ref() {
        save_idl_state(idl).context("failed to restore IDL state")?;
    }
    if let Some(wallet) = plan.wallet.as_ref() {
        save_wallet_state(wallet).context("failed to restore wallet state")?;
    }
    Ok(plan.summary)
}

#[cfg(test)]
fn backup_payload_from_states(
    settings: &Value,
    idl: &Value,
    wallet: &Value,
    encrypted: bool,
    wallet_profile: Option<&Value>,
) -> Result<Value> {
    backup_payload_from_optional_states(
        Some(settings),
        Some(idl),
        Some(wallet),
        encrypted,
        wallet_profile,
        &BackupContentsSelection::all(),
    )
}

fn backup_payload_from_optional_states(
    settings: Option<&Value>,
    idl: Option<&Value>,
    wallet: Option<&Value>,
    encrypted: bool,
    wallet_profile: Option<&Value>,
    contents: &BackupContentsSelection,
) -> Result<Value> {
    let settings = selected_settings_state(settings, contents);
    let idl = contents.idl_registry.then(|| idl.cloned()).flatten();
    let wallet = contents.wallet_profile.then(|| wallet.cloned()).flatten();
    if settings.is_none() && idl.is_none() && wallet.is_none() {
        bail!("backup content selection is empty");
    }
    let mut state = serde_json::Map::new();
    if let Some(settings) = settings {
        state.insert("settings".to_owned(), settings);
    }
    if let Some(idl) = idl {
        state.insert("idls".to_owned(), idl);
    }
    if let Some(wallet) = wallet {
        state.insert("wallet".to_owned(), wallet);
    }
    let state = Value::Object(state);
    let plain = json!({
        "kind": BACKUP_KIND,
        "version": BACKUP_VERSION,
        "created_at": unix_time_text(),
        "encrypted": false,
        "state": state,
    });
    if !encrypted {
        return Ok(plain);
    }

    let mut salt = [0_u8; 16];
    let mut nonce = [0_u8; 24];
    getrandom::fill(&mut salt).context("failed to generate backup encryption salt")?;
    getrandom::fill(&mut nonce).context("failed to generate backup encryption nonce")?;
    let key = wallet_backup_key(wallet_profile, &salt)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key).context("invalid backup encryption key")?;
    let plaintext = serde_json::to_vec(&plain).context("failed to serialize backup payload")?;
    let aad = backup_encryption_aad();
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plaintext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| anyhow::anyhow!("failed to encrypt backup payload"))?;

    Ok(json!({
        "kind": BACKUP_KIND,
        "version": BACKUP_VERSION,
        "created_at": unix_time_text(),
        "encrypted": true,
        "encryption": {
            "scheme": ENCRYPTION_SCHEME,
            "salt": BASE64_STANDARD.encode(salt),
            "nonce": BASE64_STANDARD.encode(nonce),
            "key_source": "wallet_config"
        },
        "ciphertext": BASE64_STANDARD.encode(ciphertext),
    }))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BackupContentsSelection {
    settings: bool,
    favorites: bool,
    idl_registry: bool,
    wallet_profile: bool,
}

impl BackupContentsSelection {
    fn all() -> Self {
        Self {
            settings: true,
            favorites: true,
            idl_registry: true,
            wallet_profile: true,
        }
    }

    fn none() -> Self {
        Self {
            settings: false,
            favorites: false,
            idl_registry: false,
            wallet_profile: false,
        }
    }

    fn from_value(value: Option<&Value>) -> Result<Self> {
        let Some(value) = value else {
            return Ok(Self::all());
        };
        let source = value.get("contents").unwrap_or(value);
        let selection = match source {
            Value::Array(items) => Self::from_area_array(items)?,
            Value::Object(object) => Self {
                settings: content_area_enabled(object.get("settings"), "settings")?,
                favorites: content_area_enabled(object.get("favorites"), "favorites")?,
                idl_registry: content_area_enabled(
                    object
                        .get("idl_registry")
                        .or_else(|| object.get("idls"))
                        .or_else(|| object.get("idl")),
                    "idl_registry",
                )?,
                wallet_profile: content_area_enabled(
                    object
                        .get("wallet_profile")
                        .or_else(|| object.get("wallet")),
                    "wallet_profile",
                )?,
            },
            _ => bail!("backup content selection must be an object or array"),
        };
        if !selection.any() {
            bail!("backup content selection is empty");
        }
        Ok(selection)
    }

    fn from_area_array(items: &[Value]) -> Result<Self> {
        let mut selection = Self::none();
        for item in items {
            let area = item
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .context("backup content area must be a string")?;
            selection.enable_area(area)?;
        }
        Ok(selection)
    }

    fn enable_area(&mut self, area: &str) -> Result<()> {
        match normalized_area_name(area).as_str() {
            "settings" => self.settings = true,
            "favorites" => self.favorites = true,
            "idl_registry" => self.idl_registry = true,
            "wallet_profile" => self.wallet_profile = true,
            _ => bail!("unsupported backup content area `{area}`"),
        }
        Ok(())
    }

    fn any(self) -> bool {
        self.settings || self.favorites || self.idl_registry || self.wallet_profile
    }
}

fn selected_settings_state(
    settings: Option<&Value>,
    contents: &BackupContentsSelection,
) -> Option<Value> {
    if !contents.settings && !contents.favorites {
        return None;
    }
    let source = settings.cloned().unwrap_or_else(|| json!({}));
    let mut selected = if contents.settings {
        source.clone()
    } else {
        json!({})
    };
    if !selected.is_object() {
        selected = json!({});
    }
    if !contents.favorites {
        if let Some(object) = selected.as_object_mut() {
            object.remove("favorites");
        }
        return Some(selected);
    }
    let favorites = source
        .get("favorites")
        .cloned()
        .unwrap_or_else(|| json!([]));
    set_object_field(&mut selected, "favorites", favorites).ok()?;
    Some(selected)
}

fn content_area_enabled(value: Option<&Value>, key: &str) -> Result<bool> {
    let Some(value) = value else {
        return Ok(false);
    };
    match value {
        Value::Bool(enabled) => Ok(*enabled),
        Value::String(text) => match text.trim().to_lowercase().as_str() {
            "true" | "include" | "selected" | "yes" | "on" => Ok(true),
            "false" | "skip" | "none" | "no" | "off" | "" => Ok(false),
            other => bail!("unsupported backup content value `{other}` for `{key}`"),
        },
        _ => bail!("backup content value for `{key}` must be boolean or string"),
    }
}

fn normalized_area_name(area: &str) -> String {
    match area.trim().to_lowercase().replace('-', "_").as_str() {
        "idl" | "idls" => "idl_registry".to_owned(),
        "wallet" => "wallet_profile".to_owned(),
        other => other.to_owned(),
    }
}

struct RestoredState {
    settings: Option<Value>,
    idl: Option<Value>,
    wallet: Option<Value>,
    summary: RestoreSummary,
}

struct ImportPlan {
    settings: Option<Value>,
    idl: Option<Value>,
    wallet: Option<Value>,
    summary: Value,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportMode {
    Replace,
    Merge,
    Skip,
}

struct ImportSelection {
    settings: ImportMode,
    favorites: ImportMode,
    idl_registry: ImportMode,
    wallet_profile: ImportMode,
}

#[derive(Default)]
struct ImportItemReport {
    favorites: ItemAreaReport,
    idl_registry: ItemAreaReport,
}

#[derive(Default)]
struct ItemAreaReport {
    items: Vec<Value>,
    conflicts: Vec<Value>,
}

struct PlannedSettingsState {
    state: Option<Value>,
    favorites: ItemAreaReport,
}

struct PlannedIdlState {
    state: Option<Value>,
    idl_registry: ItemAreaReport,
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

fn restored_state_from_payload(
    payload: &Value,
    wallet_profile: Option<&Value>,
) -> Result<RestoredState> {
    let encrypted = payload
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let plain = if encrypted {
        decrypt_backup_payload(payload, wallet_profile)?
    } else {
        payload.clone()
    };

    if plain.get("kind").and_then(Value::as_str) != Some(BACKUP_KIND) {
        bail!("backup payload kind is not supported");
    }
    if plain.get("version").and_then(Value::as_u64) != Some(BACKUP_VERSION) {
        bail!("backup payload version is not supported");
    }
    let state = plain
        .get("state")
        .and_then(Value::as_object)
        .context("backup payload state is missing")?;
    let settings = state.get("settings").cloned();
    let idl = state.get("idls").or_else(|| state.get("idl")).cloned();
    let wallet = state.get("wallet").cloned();
    if settings.is_none() && idl.is_none() && wallet.is_none() {
        bail!("backup payload does not contain restorable state");
    }
    let favorites_count = settings
        .as_ref()
        .and_then(|value| value.get("favorites"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let idl_count = idl
        .as_ref()
        .and_then(|value| value.get("idls"))
        .and_then(Value::as_array)
        .map_or(0, Vec::len);
    let settings_restored = settings.is_some();
    let idl_restored = idl.is_some();
    let wallet_restored = wallet.is_some();
    Ok(RestoredState {
        summary: RestoreSummary {
            settings_restored,
            idl_restored,
            wallet_restored,
            favorites_count,
            idl_count,
            encrypted,
        },
        settings,
        idl,
        wallet,
    })
}

fn build_import_plan(
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

fn planned_settings_state(
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

fn planned_idl_state(
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

fn set_object_field(target: &mut Value, key: &str, value: Value) -> Result<()> {
    if !target.is_object() {
        *target = json!({});
    }
    let object = target
        .as_object_mut()
        .context("backup import target is not an object")?;
    object.insert(key.to_owned(), value);
    Ok(())
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

fn decrypt_backup_payload(payload: &Value, wallet_profile: Option<&Value>) -> Result<Value> {
    let encryption = payload
        .get("encryption")
        .and_then(Value::as_object)
        .context("encrypted backup metadata is missing")?;
    if encryption.get("scheme").and_then(Value::as_str) != Some(ENCRYPTION_SCHEME) {
        bail!("backup encryption scheme is not supported");
    }
    let salt = decode_fixed_base64::<16>(
        encryption.get("salt").and_then(Value::as_str),
        "backup encryption salt",
    )?;
    let nonce = decode_fixed_base64::<24>(
        encryption.get("nonce").and_then(Value::as_str),
        "backup encryption nonce",
    )?;
    let ciphertext = BASE64_STANDARD
        .decode(
            payload
                .get("ciphertext")
                .and_then(Value::as_str)
                .context("encrypted backup ciphertext is missing")?,
        )
        .context("encrypted backup ciphertext is not valid base64")?;
    let key = wallet_backup_key(wallet_profile, &salt)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(&key).context("invalid backup encryption key")?;
    let aad = backup_encryption_aad();
    let plaintext = cipher
        .decrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &ciphertext,
                aad: aad.as_bytes(),
            },
        )
        .map_err(|_| {
            anyhow::anyhow!("failed to decrypt backup payload with the configured wallet")
        })?;
    serde_json::from_slice(&plaintext).context("decrypted backup payload is not valid JSON")
}

fn decode_fixed_base64<const N: usize>(value: Option<&str>, label: &str) -> Result<[u8; N]> {
    let decoded = BASE64_STANDARD
        .decode(value.context(format!("{label} is missing"))?)
        .with_context(|| format!("{label} is not valid base64"))?;
    decoded
        .try_into()
        .map_err(|_| anyhow::anyhow!("{label} has invalid length"))
}

fn wallet_backup_key(wallet_profile: Option<&Value>, salt: &[u8]) -> Result<[u8; 32]> {
    let material = wallet_backup_material(wallet_profile)?;
    let hkdf = Hkdf::<Sha256>::new(Some(salt), &material);
    let mut key = [0_u8; 32];
    hkdf.expand(b"logos inspector settings backup wallet key", &mut key)
        .map_err(|_| anyhow::anyhow!("failed to derive wallet backup key"))?;
    Ok(key)
}

fn wallet_backup_material(wallet_profile: Option<&Value>) -> Result<Vec<u8>> {
    let home = wallet_home_from_profile(wallet_profile)?;
    let config_path = home.join(WALLET_CONFIG_FILE);
    let config = fs::read(&config_path)
        .with_context(|| format!("failed to read wallet config {}", config_path.display()))?;
    if config.is_empty() {
        bail!("wallet config is empty");
    }
    let mut material = Vec::with_capacity(config.len() + BACKUP_KIND.len());
    material.extend_from_slice(BACKUP_KIND.as_bytes());
    material.push(0);
    material.extend_from_slice(&config);
    Ok(material)
}

fn wallet_home_from_profile(wallet_profile: Option<&Value>) -> Result<PathBuf> {
    let profile = wallet_profile
        .map(|value| value.get("profile").unwrap_or(value))
        .filter(|value| value.is_object());
    let explicit = profile
        .and_then(|value| {
            value
                .get("wallet_home")
                .or_else(|| value.get("walletHome"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let value = match explicit {
        Some(value) => value.to_owned(),
        None => env::var(LOCAL_WALLET_HOME_ENV)
            .ok()
            .map(|value| value.trim().to_owned())
            .filter(|value| !value.is_empty())
            .context("wallet home is required for encrypted backups")?,
    };
    let path = Path::new(&value);
    if !path.is_dir() {
        bail!("wallet home directory is not reachable");
    }
    Ok(path.to_path_buf())
}

fn backup_encryption_aad() -> String {
    format!("{BACKUP_KIND}:{BACKUP_VERSION}:{ENCRYPTION_SCHEME}")
}

fn unix_time_text() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_backup_payload_contains_settings_idls_and_wallet() -> Result<()> {
        let payload = backup_payload_from_states(
            &json!({ "favorites": [{ "value": "account-1" }] }),
            &json!({ "idls": [{ "name": "token" }] }),
            &json!({ "profile": { "label": "Local wallet" } }),
            false,
            None,
        )?;
        let restored = restored_state_from_payload(&payload, None)?;

        if !restored.summary.settings_restored
            || !restored.summary.idl_restored
            || !restored.summary.wallet_restored
        {
            bail!("expected all state sections to restore");
        }
        if restored.summary.favorites_count != 1 || restored.summary.idl_count != 1 {
            bail!("unexpected restore counts");
        }
        Ok(())
    }

    #[test]
    fn plain_backup_payload_honors_selected_contents() -> Result<()> {
        let contents = BackupContentsSelection::from_value(Some(&json!({
            "settings": false,
            "favorites": true,
            "idl_registry": false,
            "wallet_profile": false
        })))?;

        let payload = backup_payload_from_optional_states(
            Some(&json!({
                "theme": "dark",
                "favorites": [{ "value": "account-1" }]
            })),
            Some(&json!({ "idls": [{ "name": "token" }] })),
            Some(&json!({ "profile": { "label": "Local wallet" } })),
            false,
            None,
            &contents,
        )?;

        let state = payload
            .get("state")
            .and_then(Value::as_object)
            .context("backup state missing")?;
        if state.get("idls").is_some() || state.get("wallet").is_some() {
            bail!("unselected backup content leaked into payload: {payload}");
        }
        let settings = state.get("settings").context("settings state missing")?;
        if settings.get("theme").is_some() || settings.get("favorites").is_none() {
            bail!("selected favorites did not isolate settings payload: {payload}");
        }
        if BackupContentsSelection::from_value(Some(&json!({}))).is_ok() {
            bail!("empty backup content selection should fail");
        }
        Ok(())
    }

    #[test]
    fn encrypted_backup_payload_round_trips_with_wallet_config() -> Result<()> {
        let temp = unique_test_dir("encrypted-backup")?;
        fs::create_dir_all(&temp)
            .with_context(|| format!("failed to create test directory {}", temp.display()))?;
        fs::write(temp.join(WALLET_CONFIG_FILE), br#"{"wallet":"test"}"#)
            .context("failed to write test wallet config")?;
        let wallet_profile = json!({ "wallet_home": temp.display().to_string() });
        let payload = backup_payload_from_states(
            &json!({ "favorites": [{ "value": "tx-1" }] }),
            &json!({ "idls": [] }),
            &json!({ "profile": { "label": "Local wallet" } }),
            true,
            Some(&wallet_profile),
        )?;

        if payload.get("encrypted").and_then(Value::as_bool) != Some(true) {
            bail!("expected encrypted backup payload");
        }
        let restored = restored_state_from_payload(&payload, Some(&wallet_profile))?;

        if !restored.summary.encrypted || restored.summary.favorites_count != 1 {
            bail!("encrypted restore summary was not populated");
        }
        fs::remove_dir_all(&temp)
            .with_context(|| format!("failed to remove test directory {}", temp.display()))?;
        Ok(())
    }

    #[test]
    fn import_plan_can_merge_favorites_and_idl_registry() -> Result<()> {
        let restored = RestoredState {
            settings: Some(json!({
                "theme": "dark",
                "favorites": [
                    { "kind": "account", "layer": "l2", "open_kind": "account", "value": "account-1" },
                    { "kind": "block", "layer": "l1", "open_kind": "block", "value": "10" }
                ]
            })),
            idl: Some(json!({
                "idls": [
                    { "key": "idl-a", "name": "Token" },
                    { "key": "idl-b", "name": "Vault" }
                ],
                "account_idl_selections": { "account-1": "idl-a" }
            })),
            wallet: Some(json!({ "profile": { "label": "Backup wallet" } })),
            summary: RestoreSummary {
                settings_restored: true,
                idl_restored: true,
                wallet_restored: true,
                favorites_count: 2,
                idl_count: 2,
                encrypted: false,
            },
        };
        let current_settings = json!({
            "theme": "light",
            "favorites": [
                { "kind": "account", "layer": "l2", "open_kind": "account", "value": "account-1" },
                { "kind": "transaction", "layer": "l2", "open_kind": "transaction", "value": "tx-1" }
            ]
        });
        let current_idl = json!({
            "idls": [
                { "key": "idl-a", "name": "Old Token" },
                { "key": "idl-local", "name": "Local" }
            ],
            "account_idl_selections": { "account-local": "idl-local" }
        });
        let options = json!({
            "conflicts": {
                "idl_registry": {
                    "idl-a": "replace_existing"
                }
            }
        });
        let settings = planned_settings_state(
            restored.settings.as_ref(),
            Some(&current_settings),
            &ImportSelection {
                settings: ImportMode::Merge,
                favorites: ImportMode::Merge,
                idl_registry: ImportMode::Merge,
                wallet_profile: ImportMode::Skip,
            },
            Some(&options),
            true,
        )?
        .state
        .context("settings plan missing")?;
        let idl = planned_idl_state(
            restored.idl.as_ref(),
            Some(&current_idl),
            ImportMode::Merge,
            Some(&options),
            true,
        )?
        .state
        .context("idl plan missing")?;

        if settings
            .get("favorites")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
            != 3
        {
            bail!("favorites were not merged and deduped: {settings}");
        }
        if idl
            .get("idls")
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
            != 3
        {
            bail!("IDL registry was not merged and deduped: {idl}");
        }
        if idl
            .get("account_idl_selections")
            .and_then(|value| value.get("account-1"))
            .and_then(Value::as_str)
            != Some("idl-a")
        {
            bail!("IDL account selections did not merge: {idl}");
        }
        Ok(())
    }

    #[test]
    fn favorite_import_items_skip_import_replace_and_skip_conflicts() -> Result<()> {
        let backup_settings = json!({
            "favorites": [
                { "kind": "account", "layer": "l2", "open_kind": "account", "value": "account-1", "label": "Backup Account" },
                { "kind": "block", "layer": "l1", "open_kind": "block", "value": "10", "label": "Backup Block" }
            ]
        });
        let current_settings = json!({
            "favorites": [
                { "kind": "account", "layer": "l2", "open_kind": "account", "value": "account-1", "label": "Local Account" },
                { "kind": "transaction", "layer": "l2", "open_kind": "transaction", "value": "tx-1" }
            ]
        });
        let selection = ImportSelection {
            settings: ImportMode::Skip,
            favorites: ImportMode::Merge,
            idl_registry: ImportMode::Skip,
            wallet_profile: ImportMode::Skip,
        };
        let preview = planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &selection,
            None,
            false,
        )?;

        if preview.favorites.conflicts.len() != 1 {
            bail!(
                "favorite conflict was not reported: {}",
                preview.favorites.conflicts.len()
            );
        }
        if planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &selection,
            None,
            true,
        )
        .is_ok()
        {
            bail!("favorite conflict should require explicit decision before apply");
        }

        let replace_options = json!({
            "items": {
                "favorites": {
                    "account:l2:account:account-1": true,
                    "block:l1:block:10": false
                }
            },
            "conflicts": {
                "favorites": {
                    "account:l2:account:account-1": "replace_existing"
                }
            }
        });
        let replaced = planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &selection,
            Some(&replace_options),
            true,
        )?
        .state
        .context("settings plan missing")?;
        let rows = replaced
            .get("favorites")
            .and_then(Value::as_array)
            .context("favorites missing")?;
        if rows.len() != 2
            || rows.first().and_then(|row| row.get("label")) != Some(&json!("Backup Account"))
        {
            bail!("favorite replace/skip decisions were not applied: {replaced}");
        }

        let skip_options = json!({
            "conflicts": {
                "favorites": {
                    "account:l2:account:account-1": "skip_backup_item"
                }
            }
        });
        let skipped = planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &selection,
            Some(&skip_options),
            true,
        )?
        .state
        .context("settings plan missing")?;
        let rows = skipped
            .get("favorites")
            .and_then(Value::as_array)
            .context("favorites missing")?;
        if rows.len() != 3
            || rows.first().and_then(|row| row.get("label")) != Some(&json!("Local Account"))
        {
            bail!(
                "favorite conflict skip did not keep existing and import non-conflict: {skipped}"
            );
        }
        Ok(())
    }

    #[test]
    fn idl_import_items_skip_import_replace_and_skip_conflicts() -> Result<()> {
        let backup_idl = json!({
            "idls": [
                { "key": "idl-a", "name": "Backup Token" },
                { "key": "idl-b", "name": "Backup Vault" }
            ],
            "account_idl_selections": {}
        });
        let current_idl = json!({
            "idls": [
                { "key": "idl-a", "name": "Local Token" },
                { "key": "idl-local", "name": "Local" }
            ],
            "account_idl_selections": {}
        });
        let preview = planned_idl_state(
            Some(&backup_idl),
            Some(&current_idl),
            ImportMode::Merge,
            None,
            false,
        )?;
        if preview.idl_registry.conflicts.len() != 1 {
            bail!("IDL conflict was not reported");
        }
        if planned_idl_state(
            Some(&backup_idl),
            Some(&current_idl),
            ImportMode::Merge,
            None,
            true,
        )
        .is_ok()
        {
            bail!("IDL conflict should require explicit decision before apply");
        }

        let replace_options = json!({
            "items": {
                "idl_registry": {
                    "idl-a": true,
                    "idl-b": false
                }
            },
            "conflicts": {
                "idl_registry": {
                    "idl-a": "replace_existing"
                }
            }
        });
        let replaced = planned_idl_state(
            Some(&backup_idl),
            Some(&current_idl),
            ImportMode::Merge,
            Some(&replace_options),
            true,
        )?
        .state
        .context("IDL plan missing")?;
        let rows = replaced
            .get("idls")
            .and_then(Value::as_array)
            .context("IDL rows missing")?;
        if rows.len() != 2
            || rows.first().and_then(|row| row.get("name")) != Some(&json!("Backup Token"))
        {
            bail!("IDL replace/skip decisions were not applied: {replaced}");
        }

        let skip_options = json!({
            "conflicts": {
                "idl_registry": {
                    "idl-a": "skip_backup_item"
                }
            }
        });
        let skipped = planned_idl_state(
            Some(&backup_idl),
            Some(&current_idl),
            ImportMode::Merge,
            Some(&skip_options),
            true,
        )?
        .state
        .context("IDL plan missing")?;
        let rows = skipped
            .get("idls")
            .and_then(Value::as_array)
            .context("IDL rows missing")?;
        if rows.len() != 3
            || rows.first().and_then(|row| row.get("name")) != Some(&json!("Local Token"))
        {
            bail!("IDL conflict skip did not keep existing and import non-conflict: {skipped}");
        }
        Ok(())
    }

    #[test]
    fn import_plan_can_skip_replace_only_wallet_profile() -> Result<()> {
        let restored = RestoredState {
            settings: Some(json!({ "favorites": [] })),
            idl: None,
            wallet: Some(json!({ "profile": { "label": "Backup wallet" } })),
            summary: RestoreSummary {
                settings_restored: true,
                idl_restored: false,
                wallet_restored: true,
                favorites_count: 0,
                idl_count: 0,
                encrypted: false,
            },
        };
        let plan = build_import_plan(
            &restored,
            Some(&json!({
                "settings": "replace",
                "favorites": "skip",
                "wallet_profile": "skip"
            })),
            false,
        )?;

        if plan.wallet.is_some() {
            bail!("wallet profile should be skipped");
        }
        if plan
            .summary
            .get("wallet")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            bail!("summary should report skipped wallet profile");
        }
        Ok(())
    }

    #[test]
    fn import_plan_warns_about_wallet_paths_that_do_not_exist_locally() -> Result<()> {
        let missing_home = unique_test_dir("missing-wallet-home")?;
        let missing_binary = missing_home.join("lee-wallet");
        let restored = RestoredState {
            settings: None,
            idl: None,
            wallet: Some(json!({
                "profile": {
                    "label": "Backup wallet",
                    "wallet_home": missing_home.display().to_string(),
                    "wallet_binary": missing_binary.display().to_string()
                }
            })),
            summary: RestoreSummary {
                settings_restored: false,
                idl_restored: false,
                wallet_restored: true,
                favorites_count: 0,
                idl_count: 0,
                encrypted: false,
            },
        };

        let plan = build_import_plan(
            &restored,
            Some(&json!({ "wallet_profile": "replace" })),
            false,
        )?;
        let warnings = plan
            .summary
            .get("warnings")
            .and_then(Value::as_array)
            .context("warning rows missing")?;

        if !warning_key_present(warnings, "wallet_home")
            || !warning_key_present(warnings, "wallet_binary")
        {
            bail!("wallet import plan should warn about missing local paths: {warnings:?}");
        }
        Ok(())
    }

    #[test]
    fn import_plan_does_not_warn_about_valid_wallet_paths() -> Result<()> {
        let temp = unique_test_dir("valid-wallet-paths")?;
        let wallet_home = temp.join("wallet-home");
        let bin_dir = temp.join("bin");
        let wallet_binary = bin_dir.join("lee-wallet");
        fs::create_dir_all(&wallet_home)
            .with_context(|| format!("failed to create wallet home {}", wallet_home.display()))?;
        fs::create_dir_all(&bin_dir)
            .with_context(|| format!("failed to create wallet bin dir {}", bin_dir.display()))?;
        fs::write(&wallet_binary, b"test-wallet").with_context(|| {
            format!("failed to write wallet binary {}", wallet_binary.display())
        })?;
        let restored = RestoredState {
            settings: None,
            idl: None,
            wallet: Some(json!({
                "profile": {
                    "label": "Backup wallet",
                    "wallet_home": wallet_home.display().to_string(),
                    "wallet_binary": wallet_binary.display().to_string()
                }
            })),
            summary: RestoreSummary {
                settings_restored: false,
                idl_restored: false,
                wallet_restored: true,
                favorites_count: 0,
                idl_count: 0,
                encrypted: false,
            },
        };

        let plan = build_import_plan(
            &restored,
            Some(&json!({ "wallet_profile": "replace" })),
            false,
        )?;
        let warning_count = plan
            .summary
            .get("warnings")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);

        fs::remove_dir_all(&temp)
            .with_context(|| format!("failed to remove test directory {}", temp.display()))?;
        if warning_count != 0 {
            bail!(
                "valid wallet paths should not emit warnings: {}",
                plan.summary
            );
        }
        Ok(())
    }

    #[test]
    fn import_plan_does_not_warn_about_skipped_wallet_paths() -> Result<()> {
        let restored = RestoredState {
            settings: Some(json!({ "favorites": [] })),
            idl: None,
            wallet: Some(json!({
                "profile": {
                    "wallet_home": "relative-wallet-home",
                    "wallet_binary": "relative-wallet-binary"
                }
            })),
            summary: RestoreSummary {
                settings_restored: true,
                idl_restored: false,
                wallet_restored: true,
                favorites_count: 0,
                idl_count: 0,
                encrypted: false,
            },
        };

        let plan = build_import_plan(
            &restored,
            Some(&json!({
                "settings": "replace",
                "wallet_profile": "skip"
            })),
            false,
        )?;

        let warning_count = plan
            .summary
            .get("warnings")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        if warning_count != 0 {
            bail!(
                "skipped wallet profile should not emit path warnings: {}",
                plan.summary
            );
        }
        Ok(())
    }

    #[test]
    fn import_plan_defaults_to_skip_without_explicit_modes() -> Result<()> {
        let restored = RestoredState {
            settings: Some(json!({ "theme": "dark", "favorites": [{ "value": "tx-1" }] })),
            idl: Some(json!({ "idls": [{ "key": "idl-a" }] })),
            wallet: Some(json!({ "profile": { "label": "Backup wallet" } })),
            summary: RestoreSummary {
                settings_restored: true,
                idl_restored: true,
                wallet_restored: true,
                favorites_count: 1,
                idl_count: 1,
                encrypted: false,
            },
        };

        let plan = build_import_plan(&restored, None, false)?;

        if plan.settings.is_some() || plan.idl.is_some() || plan.wallet.is_some() {
            bail!("implicit backup import modes should skip all sections");
        }
        if plan
            .summary
            .get("restored")
            .and_then(Value::as_bool)
            .unwrap_or(true)
        {
            bail!("summary should report no restored sections by default");
        }
        Ok(())
    }

    #[test]
    fn import_plan_rejects_merge_for_replace_only_sections() -> Result<()> {
        let restored = RestoredState {
            settings: Some(json!({ "theme": "dark" })),
            idl: None,
            wallet: Some(json!({ "profile": { "label": "Backup wallet" } })),
            summary: RestoreSummary {
                settings_restored: true,
                idl_restored: false,
                wallet_restored: true,
                favorites_count: 0,
                idl_count: 0,
                encrypted: false,
            },
        };

        if build_import_plan(&restored, Some(&json!({ "settings": "merge" })), false).is_ok() {
            bail!("settings merge should be rejected");
        }
        if build_import_plan(
            &restored,
            Some(&json!({ "wallet_profile": "merge" })),
            false,
        )
        .is_ok()
        {
            bail!("wallet profile merge should be rejected");
        }
        Ok(())
    }

    fn warning_key_present(warnings: &[Value], key: &str) -> bool {
        warnings
            .iter()
            .any(|warning| warning.get("key").and_then(Value::as_str) == Some(key))
    }

    fn unique_test_dir(label: &str) -> Result<PathBuf> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before UNIX epoch")?
            .as_nanos();
        Ok(env::temp_dir().join(format!(
            "logos-inspector-{label}-{}-{nanos}",
            std::process::id()
        )))
    }
}
