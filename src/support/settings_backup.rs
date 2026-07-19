use std::{
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
use serde_json::{Map, Value, json};
use sha2::Sha256;

use crate::{
    source_routing::channel_sources::settings_state_from_stored,
    support::{
        local_state::{
            LocalStateCommitCancellation, LocalStateCommitReport, LocalStateSession,
            LocalStateTransactionError, LocalStateWriteSet, StateFile, with_local_state_in,
        },
        state_store::{config_dir, idl_state_from_stored, wallet_state_from_stored},
    },
    wallet::LOCAL_WALLET_HOME_ENV,
};

#[cfg(test)]
use crate::support::local_state::{
    LOCAL_STATE_TRANSACTION_ID_HEX_LENGTH, LocalStateTestBoundary, LocalStateTestFault,
};

const BACKUP_KIND: &str = "logos-inspector-settings-backup";
const BACKUP_VERSION: u64 = 1;
const ENCRYPTION_SCHEME: &str = "xchacha20poly1305-wallet-config-v1";
const ENCRYPTION_TAG_BYTES: usize = 16;
const WALLET_CONFIG_FILE: &str = "wallet_config.json";

pub(crate) const SETTINGS_BACKUP_MAX_BYTES: usize = 16 * 1024 * 1024;

mod import_options;
mod import_plan;

pub(crate) use import_options::{
    BackupImportArea, BackupImportMode, BackupImportOptions, BackupImportSelection,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RestoreSummary {
    pub settings_restored: bool,
    pub idl_restored: bool,
    pub wallet_restored: bool,
    pub favorites_count: usize,
    pub idl_count: usize,
    pub encrypted: bool,
}

#[derive(Debug)]
pub(crate) enum SettingsBackupCommitResult {
    NoOp,
    Applied(LocalStateCommitReport),
}

#[derive(Debug)]
pub(crate) struct SettingsBackupRestoreReceipt {
    summary: Map<String, Value>,
    applied_areas: Vec<BackupImportArea>,
    commit: SettingsBackupCommitResult,
}

impl SettingsBackupRestoreReceipt {
    pub(crate) fn into_parts(
        self,
    ) -> (
        Map<String, Value>,
        Vec<BackupImportArea>,
        SettingsBackupCommitResult,
    ) {
        (self.summary, self.applied_areas, self.commit)
    }

    #[cfg(test)]
    fn commit_report(&self) -> Option<&LocalStateCommitReport> {
        match &self.commit {
            SettingsBackupCommitResult::NoOp => None,
            SettingsBackupCommitResult::Applied(report) => Some(report),
        }
    }
}

pub(crate) fn ensure_settings_backup_size(byte_len: usize) -> Result<()> {
    if byte_len > SETTINGS_BACKUP_MAX_BYTES {
        bail!(
            "settings backup payload exceeded {} byte limit",
            SETTINGS_BACKUP_MAX_BYTES
        );
    }
    Ok(())
}

pub(crate) fn validate_app_settings_backup_envelope(payload: &Value) -> Result<()> {
    validate_backup_identity(payload)?;
    if payload
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        encrypted_backup_material(payload).map(|_| ())
    } else {
        restorable_backup_state(payload).map(|_| ())
    }
}

pub(crate) fn export_app_settings_backup(
    encrypted: bool,
    wallet_profile: Option<&Value>,
    content_options: Option<&Value>,
) -> Result<Value> {
    export_app_settings_backup_in_dir(&config_dir()?, encrypted, wallet_profile, content_options)
}

fn export_app_settings_backup_in_dir(
    base_dir: &Path,
    encrypted: bool,
    wallet_profile: Option<&Value>,
    content_options: Option<&Value>,
) -> Result<Value> {
    let contents = BackupContentsSelection::from_value(content_options)?;
    with_local_state_in(base_dir, |session| {
        let settings = (contents.settings || contents.favorites)
            .then(|| {
                settings_state_from_stored(&session.read(StateFile::Settings)?)
                    .context("failed to load settings state for backup")
            })
            .transpose()?;
        let idl = contents
            .idl_registry
            .then(|| {
                idl_state_from_stored(&session.read(StateFile::Idl)?)
                    .context("failed to load IDL state for backup")
            })
            .transpose()?;
        let wallet = contents
            .wallet_profile
            .then(|| {
                wallet_state_from_stored(&session.read(StateFile::Wallet)?)
                    .context("failed to load wallet state for backup")
            })
            .transpose()?;
        backup_payload_from_optional_states(
            settings.as_ref(),
            idl.as_ref(),
            wallet.as_ref(),
            encrypted,
            wallet_profile,
            &contents,
        )
    })
    .context("failed to load selected local state for backup")
}

pub(crate) fn preview_app_settings_backup_import(
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
) -> Result<Value> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    preview_app_settings_backup_import_in_dir(&config_dir()?, &state, options)
}

fn preview_app_settings_backup_import_in_dir(
    base_dir: &Path,
    state: &RestoredState,
    options: &BackupImportOptions,
) -> Result<Value> {
    validate_selected_import_state(state, options)?;
    with_local_state_in(base_dir, |session| {
        let (current_settings, current_idl) = current_import_state(session, state, options)?;
        let plan = build_import_plan_with_current(
            state,
            options,
            false,
            current_settings.as_ref(),
            current_idl.as_ref(),
        )?;
        Ok(plan.summary)
    })
    .context("failed to load selected current local state for backup import preview")
}

#[cfg(test)]
pub(crate) fn preview_app_settings_backup_import_in_dir_for_test(
    base_dir: &Path,
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
) -> Result<Value> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    preview_app_settings_backup_import_in_dir(base_dir, &state, options)
}

pub(crate) fn restore_app_settings_backup_with_options(
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
) -> Result<SettingsBackupRestoreReceipt> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    restore_app_settings_backup_in_dir_with_cancellation(
        &config_dir()?,
        &state,
        options,
        cancellation,
    )
}

#[cfg(test)]
fn restore_app_settings_backup_in_dir(
    base_dir: &Path,
    state: &RestoredState,
    options: &BackupImportOptions,
) -> Result<SettingsBackupRestoreReceipt> {
    restore_app_settings_backup_in_dir_with_cancellation(
        base_dir,
        state,
        options,
        &LocalStateCommitCancellation::default(),
    )
}

fn restore_app_settings_backup_in_dir_with_cancellation(
    base_dir: &Path,
    state: &RestoredState,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
) -> Result<SettingsBackupRestoreReceipt> {
    restore_app_settings_backup_in_dir_with_commit(base_dir, state, options, |session, writes| {
        session.commit(writes, cancellation)
    })
}

fn restore_app_settings_backup_in_dir_with_commit(
    base_dir: &Path,
    state: &RestoredState,
    options: &BackupImportOptions,
    commit: impl FnOnce(&mut LocalStateSession, LocalStateWriteSet) -> Result<LocalStateCommitReport>,
) -> Result<SettingsBackupRestoreReceipt> {
    validate_selected_import_state(state, options)?;
    with_local_state_in(base_dir, |session| {
        let (current_settings, current_idl) = current_import_state(session, state, options)
            .context("failed to rebuild backup import plan from current local state")?;
        let plan = build_import_plan_with_current(
            state,
            options,
            true,
            current_settings.as_ref(),
            current_idl.as_ref(),
        )?;
        let mut writes = LocalStateWriteSet::new();
        if let Some(settings) = plan.settings.as_ref() {
            writes = writes.settings(
                serde_json::to_vec_pretty(settings)
                    .context("failed to serialize restored settings state")?,
            );
        }
        if let Some(idl) = plan.idl.as_ref() {
            writes = writes.idl(
                serde_json::to_vec_pretty(idl).context("failed to serialize restored IDL state")?,
            );
        }
        if let Some(wallet) = plan.wallet.as_ref() {
            writes = writes.wallet(
                serde_json::to_vec_pretty(wallet)
                    .context("failed to serialize restored wallet state")?,
            );
        }
        let summary = plan
            .summary
            .as_object()
            .cloned()
            .context("backup import summary must be a JSON object")?;
        let commit = if writes.is_empty() {
            if !plan.applied_areas.is_empty() {
                bail!("backup import plan applied areas without a local write set");
            }
            SettingsBackupCommitResult::NoOp
        } else {
            if plan.applied_areas.is_empty() {
                bail!("backup import plan produced a local write set without applied areas");
            }
            SettingsBackupCommitResult::Applied(
                commit(session, writes).map_err(local_state_commit_error)?,
            )
        };
        Ok(SettingsBackupRestoreReceipt {
            summary,
            applied_areas: plan.applied_areas,
            commit,
        })
    })
}

#[cfg(test)]
pub(crate) fn restore_app_settings_backup_in_dir_for_test(
    base_dir: &Path,
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
) -> Result<SettingsBackupRestoreReceipt> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    restore_app_settings_backup_in_dir_with_cancellation(base_dir, &state, options, cancellation)
}

#[cfg(test)]
pub(crate) fn restore_app_settings_backup_with_fault_in_dir_for_test(
    base_dir: &Path,
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
    fault: LocalStateTestFault,
) -> Result<SettingsBackupRestoreReceipt> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    restore_app_settings_backup_in_dir_with_commit(
        base_dir,
        &state,
        options,
        move |session, writes| session.commit_with_test_fault(writes, cancellation, fault),
    )
}

#[cfg(test)]
pub(crate) fn restore_app_settings_backup_at_boundary_in_dir_for_test(
    base_dir: &Path,
    payload: &Value,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
    boundary: LocalStateTestBoundary,
    mut at_boundary: impl FnMut() -> Result<()>,
) -> Result<SettingsBackupRestoreReceipt> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    restore_app_settings_backup_in_dir_with_commit(
        base_dir,
        &state,
        options,
        move |session, writes| {
            session.commit_with_test_boundary(writes, cancellation, boundary, &mut at_boundary)
        },
    )
}

fn current_import_state(
    session: &LocalStateSession,
    state: &RestoredState,
    options: &BackupImportOptions,
) -> Result<(Option<Value>, Option<Value>)> {
    let requirements = import_plan::current_state_requirements(state, options);
    let settings = requirements
        .settings
        .then(|| {
            settings_state_from_stored(&session.read(StateFile::Settings)?)
                .context("failed to load current settings for backup import")
        })
        .transpose()?;
    let idl = requirements
        .idl
        .then(|| {
            idl_state_from_stored(&session.read(StateFile::Idl)?)
                .context("failed to load current IDL state for backup import")
        })
        .transpose()?;
    Ok((settings, idl))
}

fn validate_selected_import_state(
    state: &RestoredState,
    options: &BackupImportOptions,
) -> Result<()> {
    if (options.mode(BackupImportArea::Settings).is_selected()
        || options.mode(BackupImportArea::Favorites).is_selected())
        && let Some(settings) = state.settings.as_ref()
    {
        validate_import_settings_state(settings, options)?;
    }
    if options.mode(BackupImportArea::IdlRegistry).is_selected()
        && let Some(idl) = state.idl.as_ref()
    {
        validate_import_idl_state(idl)?;
    }
    if options.mode(BackupImportArea::WalletProfile).is_selected()
        && let Some(wallet) = state.wallet.as_ref()
    {
        validate_import_wallet_state(wallet)?;
    }
    Ok(())
}

fn validate_import_settings_state(settings: &Value, options: &BackupImportOptions) -> Result<()> {
    let object = settings
        .as_object()
        .context("backup settings state must be an object")?;
    if options.mode(BackupImportArea::Favorites).is_selected()
        && let Some(favorites) = object.get("favorites")
    {
        let favorites = favorites
            .as_array()
            .context("backup favorites must be an array")?;
        for (index, favorite) in favorites.iter().enumerate() {
            if !favorite.is_object() {
                bail!("backup favorite at index {index} must be an object");
            }
        }
    }
    Ok(())
}

fn validate_import_idl_state(idl: &Value) -> Result<()> {
    let object = idl
        .as_object()
        .context("backup IDL registry must be an object")?;
    let entries = object
        .get("idls")
        .and_then(Value::as_array)
        .context("backup IDL registry must contain an `idls` array")?;
    for (index, entry) in entries.iter().enumerate() {
        if !entry.is_object() {
            bail!("backup IDL registry item at index {index} must be an object");
        }
    }
    if let Some(selections) = object.get("account_idl_selections") {
        let selections = selections
            .as_object()
            .context("backup IDL account selections must be an object")?;
        for (account, selection) in selections {
            if normalized_account_idl_selection_key(selection).is_none() {
                bail!(
                    "backup IDL account selection for `{account}` must be a non-empty IDL key string or object"
                );
            }
            if let Some(selection) = selection.as_object() {
                for field in ["accountType", "ownerProgram", "network"] {
                    if selection.get(field).is_some_and(|value| !value.is_string()) {
                        bail!(
                            "backup IDL account selection field `{field}` for `{account}` must be a string"
                        );
                    }
                }
            }
        }
    }
    Ok(())
}

fn normalized_account_idl_selection_key(selection: &Value) -> Option<String> {
    selection
        .as_str()
        .or_else(|| {
            selection
                .as_object()
                .and_then(|value| value.get("idlKey"))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_lowercase)
}

fn validate_import_wallet_state(wallet: &Value) -> Result<()> {
    let object = wallet
        .as_object()
        .context("backup wallet state must be an object")?;
    if let Some(profile) = object.get("profile")
        && !profile.is_object()
    {
        bail!("backup wallet profile must be an object");
    }
    if let Some(operations) = object.get("operations")
        && !operations.is_array()
    {
        bail!("backup wallet operations must be an array");
    }
    Ok(())
}

fn local_state_commit_error(error: anyhow::Error) -> anyhow::Error {
    let facts = error
        .downcast_ref::<LocalStateTransactionError>()
        .map(|transaction| {
            (
                transaction.status().as_str(),
                transaction.transaction_id().unwrap_or("unknown").to_owned(),
            )
        });
    match facts {
        Some((status, transaction_id)) => error.context(format!(
            "backup import local transaction `{transaction_id}` ended {status}"
        )),
        None => error,
    }
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

#[cfg(test)]
use import_plan::build_import_plan;
use import_plan::build_import_plan_with_current;
#[cfg(test)]
use import_plan::{planned_idl_state, planned_settings_state};

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

    validate_backup_identity(&plain)?;
    let state = restorable_backup_state(&plain)?;
    let settings = state.get("settings").cloned();
    let idl = state.get("idls").or_else(|| state.get("idl")).cloned();
    let wallet = state.get("wallet").cloned();
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

fn validate_backup_identity(payload: &Value) -> Result<()> {
    if payload.get("kind").and_then(Value::as_str) != Some(BACKUP_KIND) {
        bail!("backup payload kind is not supported");
    }
    if payload.get("version").and_then(Value::as_u64) != Some(BACKUP_VERSION) {
        bail!("backup payload version is not supported");
    }
    Ok(())
}

fn restorable_backup_state(payload: &Value) -> Result<&serde_json::Map<String, Value>> {
    let state = payload
        .get("state")
        .and_then(Value::as_object)
        .context("backup payload state is missing")?;
    if !state.contains_key("settings")
        && !state.contains_key("idls")
        && !state.contains_key("idl")
        && !state.contains_key("wallet")
    {
        bail!("backup payload does not contain restorable state");
    }
    Ok(state)
}

fn decrypt_backup_payload(payload: &Value, wallet_profile: Option<&Value>) -> Result<Value> {
    let (salt, nonce, ciphertext) = encrypted_backup_material(payload)?;
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

fn encrypted_backup_material(payload: &Value) -> Result<([u8; 16], [u8; 24], Vec<u8>)> {
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
    if ciphertext.len() < ENCRYPTION_TAG_BYTES {
        bail!("encrypted backup ciphertext has invalid length");
    }
    Ok((salt, nonce, ciphertext))
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
    fn settings_backup_size_contract_accepts_limit_and_rejects_overflow() -> Result<()> {
        ensure_settings_backup_size(SETTINGS_BACKUP_MAX_BYTES)?;
        let error = ensure_settings_backup_size(SETTINGS_BACKUP_MAX_BYTES.saturating_add(1))
            .err()
            .context("oversized settings backup should fail")?;

        if !error.to_string().contains(&format!(
            "settings backup payload exceeded {} byte limit",
            SETTINGS_BACKUP_MAX_BYTES
        )) {
            bail!("unexpected settings backup size error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn settings_backup_envelope_rejects_unrestorable_payloads_without_decrypting() -> Result<()> {
        let invalid = [
            (
                json!({
                    "kind": "other-backup",
                    "version": 1,
                    "encrypted": false,
                    "state": { "settings": {} }
                }),
                "backup payload kind is not supported",
            ),
            (
                json!({
                    "kind": BACKUP_KIND,
                    "version": 2,
                    "encrypted": false,
                    "state": { "settings": {} }
                }),
                "backup payload version is not supported",
            ),
            (
                json!({
                    "kind": BACKUP_KIND,
                    "version": BACKUP_VERSION,
                    "encrypted": false,
                    "state": {}
                }),
                "backup payload does not contain restorable state",
            ),
            (
                json!({
                    "kind": BACKUP_KIND,
                    "version": BACKUP_VERSION,
                    "encrypted": true,
                    "state": { "settings": {} }
                }),
                "encrypted backup metadata is missing",
            ),
        ];

        for (payload, expected) in invalid {
            let error = validate_app_settings_backup_envelope(&payload)
                .err()
                .context("invalid settings backup envelope should fail")?;
            if !error.to_string().contains(expected) {
                bail!("unexpected backup envelope error: {error:#}");
            }
        }

        validate_app_settings_backup_envelope(&json!({
            "kind": BACKUP_KIND,
            "version": BACKUP_VERSION,
            "encrypted": true,
            "encryption": {
                "scheme": ENCRYPTION_SCHEME,
                "salt": BASE64_STANDARD.encode([0_u8; 16]),
                "nonce": BASE64_STANDARD.encode([0_u8; 24]),
                "key_source": "wallet_config"
            },
            "ciphertext": BASE64_STANDARD.encode([0_u8; ENCRYPTION_TAG_BYTES])
        }))?;
        Ok(())
    }

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
    fn selected_restore_commits_settings_idl_and_wallet_as_one_local_transaction() -> Result<()> {
        let directory = tempfile::tempdir().context("failed to create restore test directory")?;
        fs::write(
            directory.path().join("settings.json"),
            br#"{"version":2,"theme":"old","channel_source_configs":[]}"#,
        )?;
        fs::write(
            directory.path().join("idls.json"),
            br#"{"version":1,"idls":[]}"#,
        )?;
        fs::write(
            directory.path().join("wallet.json"),
            br#"{"profile":{"label":"Old wallet"}}"#,
        )?;
        let payload = backup_payload_from_states(
            &json!({ "version": 2, "theme": "new", "channel_source_configs": [] }),
            &json!({ "version": 1, "idls": [{ "key": "idl-new", "json": "{}" }] }),
            &json!({ "profile": { "label": "New wallet" } }),
            false,
            None,
        )?;
        let restored = restored_state_from_payload(&payload, None)?;
        let options = BackupImportOptions::parse(Some(&json!({
            "settings": "replace",
            "favorites": "skip",
            "idl_registry": "replace",
            "wallet_profile": "replace"
        })))?;
        let receipt = restore_app_settings_backup_in_dir(directory.path(), &restored, &options)?;
        let report = receipt
            .commit_report()
            .context("restore omitted local transaction evidence")?;
        if report.transaction_id().len() != LOCAL_STATE_TRANSACTION_ID_HEX_LENGTH {
            bail!("restore returned an invalid transaction identity");
        }
        let settings: Value =
            serde_json::from_slice(&fs::read(directory.path().join("settings.json"))?)?;
        let idl: Value = serde_json::from_slice(&fs::read(directory.path().join("idls.json"))?)?;
        let wallet: Value =
            serde_json::from_slice(&fs::read(directory.path().join("wallet.json"))?)?;
        if settings.get("theme").and_then(Value::as_str) != Some("new")
            || idl.pointer("/idls/0/key").and_then(Value::as_str) != Some("idl-new")
            || wallet.pointer("/profile/label").and_then(Value::as_str) != Some("New wallet")
        {
            bail!("restore did not commit the selected triple");
        }
        Ok(())
    }

    #[test]
    fn favorites_only_restore_preserves_current_channel_config_and_revision() -> Result<()> {
        let directory = tempfile::tempdir().context("failed to create backup test directory")?;
        let current_config = backup_test_channel_config('1', '1', 7, "Current", 3040);
        let current_settings = json!({
            "version": 2,
            "theme": "old",
            "favorites": [{ "value": "old-favorite" }],
            "channel_source_configs": [current_config.clone()]
        });
        fs::write(
            directory.path().join("settings.json"),
            serde_json::to_vec(&current_settings)?,
        )?;
        let backup_settings = json!({
            "version": 2,
            "theme": "backup",
            "favorites": [{
                "kind": "transaction",
                "layer": "l1",
                "open_kind": "mantleTransaction",
                "value": "new-favorite",
                "navigation_context": {
                    "kind": "l1_transaction",
                    "slot": 41
                }
            }],
            "channel_source_configs": [backup_test_channel_config('1', '1', 99, "Backup", 4040)]
        });
        let payload = backup_payload_from_states(
            &backup_settings,
            &json!({ "version": 1, "idls": [] }),
            &json!({ "profile": { "label": "Ignored wallet" } }),
            false,
            None,
        )?;
        let restored = restored_state_from_payload(&payload, None)?;
        let options = BackupImportOptions::parse(Some(&json!({
            "settings": "skip",
            "favorites": "replace",
            "idl_registry": "skip",
            "wallet_profile": "skip"
        })))?;

        preview_app_settings_backup_import_in_dir(directory.path(), &restored, &options)?;
        restore_app_settings_backup_in_dir(directory.path(), &restored, &options)?;

        let saved: Value =
            serde_json::from_slice(&fs::read(directory.path().join("settings.json"))?)?;
        if saved.pointer("/favorites/0/value").and_then(Value::as_str) != Some("new-favorite")
            || saved
                .pointer("/favorites/0/navigation_context/kind")
                .and_then(Value::as_str)
                != Some("l1_transaction")
            || saved
                .pointer("/favorites/0/navigation_context/slot")
                .and_then(Value::as_u64)
                != Some(41)
            || saved.get("theme").and_then(Value::as_str) != Some("old")
            || saved.pointer("/channel_source_configs/0") != Some(&current_config)
            || saved
                .pointer("/channel_source_configs/0/config_revision")
                .and_then(Value::as_u64)
                != Some(7)
        {
            bail!("favorites-only restore changed current Channel source config: {saved}");
        }
        Ok(())
    }

    #[test]
    fn settings_replace_rebases_channel_config_revisions_on_disk() -> Result<()> {
        let directory = tempfile::tempdir().context("failed to create backup test directory")?;
        let current_a = backup_test_channel_config('3', '3', 7, "Same", 3040);
        let current_b = backup_test_channel_config('4', '4', 9, "Current", 3041);
        let current_c = backup_test_channel_config('5', '5', 3, "Removed", 3042);
        fs::write(
            directory.path().join("settings.json"),
            serde_json::to_vec(&json!({
                "version": 2,
                "theme": "old",
                "channel_source_configs": [current_a.clone(), current_b, current_c]
            }))?,
        )?;
        let mut archived_a = current_a;
        *archived_a
            .get_mut("config_revision")
            .context("archived config revision missing")? = json!(99);
        let archived_b = backup_test_channel_config('4', '4', 1, "Changed", 4041);
        let archived_d = backup_test_channel_config('6', '6', 88, "New", 4042);
        let payload = backup_payload_from_states(
            &json!({
                "version": 2,
                "theme": "backup",
                "channel_source_configs": [
                    archived_a,
                    archived_b.clone(),
                    archived_d.clone()
                ]
            }),
            &json!({ "version": 1, "idls": [] }),
            &json!({ "profile": { "label": "Ignored wallet" } }),
            false,
            None,
        )?;
        let restored = restored_state_from_payload(&payload, None)?;
        let options = BackupImportOptions::parse(Some(&json!({
            "settings": "replace",
            "favorites": "skip",
            "idl_registry": "skip",
            "wallet_profile": "skip"
        })))?;

        preview_app_settings_backup_import_in_dir(directory.path(), &restored, &options)?;
        restore_app_settings_backup_in_dir(directory.path(), &restored, &options)?;

        let saved: Value =
            serde_json::from_slice(&fs::read(directory.path().join("settings.json"))?)?;
        let configs = saved
            .get("channel_source_configs")
            .and_then(Value::as_array)
            .context("restored Channel source configs missing")?;
        let config = |channel_character: char| {
            let channel_id = channel_character.to_string().repeat(64);
            configs.iter().find(|config| {
                config.get("channel_id").and_then(Value::as_str) == Some(channel_id.as_str())
            })
        };
        if config('3').and_then(|value| value.get("config_revision")) != Some(&json!(7))
            || config('4').and_then(|value| value.get("config_revision")) != Some(&json!(10))
            || config('5').is_some()
            || config('6').and_then(|value| value.get("config_revision")) != Some(&json!(1))
            || config('4').and_then(|value| value.get("sequencer_sources"))
                != archived_b.get("sequencer_sources")
            || config('6').and_then(|value| value.get("sequencer_sources"))
                != archived_d.get("sequencer_sources")
            || saved.get("theme").and_then(Value::as_str) != Some("backup")
        {
            bail!("settings replacement revision matrix drifted: {saved}");
        }
        Ok(())
    }

    #[test]
    fn backup_revision_overflow_aborts_settings_idl_and_wallet_transaction() -> Result<()> {
        let directory = tempfile::tempdir().context("failed to create backup test directory")?;
        let current_settings = json!({
            "version": 2,
            "theme": "old",
            "channel_source_configs": [backup_test_channel_config('2', '2', u64::MAX, "Current", 3040)]
        });
        let current_settings_bytes = serde_json::to_vec(&current_settings)?;
        let current_idl_bytes = br#"{"version":1,"idls":[{"key":"old"}]}"#.to_vec();
        let current_wallet_bytes = br#"{"profile":{"label":"Old wallet"}}"#.to_vec();
        fs::write(
            directory.path().join("settings.json"),
            &current_settings_bytes,
        )?;
        fs::write(directory.path().join("idls.json"), &current_idl_bytes)?;
        fs::write(directory.path().join("wallet.json"), &current_wallet_bytes)?;
        let payload = backup_payload_from_states(
            &json!({
                "version": 2,
                "theme": "new",
                "channel_source_configs": [backup_test_channel_config('2', '2', 1, "Changed", 4040)]
            }),
            &json!({ "version": 1, "idls": [{ "key": "new" }] }),
            &json!({ "profile": { "label": "New wallet" } }),
            false,
            None,
        )?;
        let restored = restored_state_from_payload(&payload, None)?;
        let options = BackupImportOptions::parse(Some(&json!({
            "settings": "replace",
            "favorites": "skip",
            "idl_registry": "replace",
            "wallet_profile": "replace"
        })))?;

        let preview_error =
            preview_app_settings_backup_import_in_dir(directory.path(), &restored, &options)
                .err()
                .context("overflowing backup preview unexpectedly succeeded")?;
        let restore_error =
            restore_app_settings_backup_in_dir(directory.path(), &restored, &options)
                .err()
                .context("overflowing backup restore unexpectedly succeeded")?;
        if !format!("{preview_error:#}").contains("revision overflow during backup import")
            || !format!("{restore_error:#}").contains("revision overflow during backup import")
            || fs::read(directory.path().join("settings.json"))? != current_settings_bytes
            || fs::read(directory.path().join("idls.json"))? != current_idl_bytes
            || fs::read(directory.path().join("wallet.json"))? != current_wallet_bytes
        {
            bail!("revision overflow did not abort the complete local-state transaction");
        }
        Ok(())
    }

    #[test]
    fn malformed_selected_settings_idl_and_wallet_do_not_mutate_local_state() -> Result<()> {
        for (label, state, option_value, expected_error) in [
            (
                "favorites-settings-container",
                json!({ "settings": "not-an-object" }),
                json!({ "favorites": "replace" }),
                "backup settings state must be an object",
            ),
            (
                "favorites-container",
                json!({ "settings": { "favorites": "not-an-array" } }),
                json!({ "favorites": "replace" }),
                "backup favorites must be an array",
            ),
            (
                "favorite-row",
                json!({ "settings": { "favorites": ["not-an-object"] } }),
                json!({ "favorites": "replace" }),
                "backup favorite at index 0 must be an object",
            ),
            (
                "idl-container",
                json!({ "idls": "not-an-object" }),
                json!({ "idl_registry": "replace" }),
                "backup IDL registry must be an object",
            ),
            (
                "idl-rows",
                json!({ "idls": { "idls": "not-an-array" } }),
                json!({ "idl_registry": "replace" }),
                "backup IDL registry must contain an `idls` array",
            ),
            (
                "idl-account-selection",
                json!({
                    "idls": {
                        "idls": [],
                        "account_idl_selections": {
                            "account-a": { "accountType": "AccountA" }
                        }
                    }
                }),
                json!({ "idl_registry": "replace" }),
                "must be a non-empty IDL key string or object",
            ),
            (
                "idl-account-metadata",
                json!({
                    "idls": {
                        "idls": [],
                        "account_idl_selections": {
                            "account-a": { "idlKey": "idl-a", "network": 7 }
                        }
                    }
                }),
                json!({ "idl_registry": "replace" }),
                "field `network`",
            ),
            (
                "wallet-container",
                json!({ "wallet": "not-an-object" }),
                json!({ "wallet_profile": "replace" }),
                "backup wallet state must be an object",
            ),
            (
                "wallet-profile",
                json!({ "wallet": { "profile": "not-an-object" } }),
                json!({ "wallet_profile": "replace" }),
                "backup wallet profile must be an object",
            ),
        ] {
            let directory =
                tempfile::tempdir().context("failed to create malformed restore test directory")?;
            let old_settings = br#"{"version":2,"theme":"old","channel_source_configs":[]}"#;
            let old_idl = br#"{"version":1,"idls":[{"key":"old"}]}"#;
            let old_wallet = br#"{"profile":{"label":"Old wallet"}}"#;
            fs::write(directory.path().join("settings.json"), old_settings)?;
            fs::write(directory.path().join("idls.json"), old_idl)?;
            fs::write(directory.path().join("wallet.json"), old_wallet)?;
            let payload = json!({
                "kind": BACKUP_KIND,
                "version": BACKUP_VERSION,
                "encrypted": false,
                "state": state,
            });
            let restored = restored_state_from_payload(&payload, None)?;
            let options = BackupImportOptions::parse(Some(&option_value))?;

            let error = restore_app_settings_backup_in_dir(directory.path(), &restored, &options)
                .err()
                .with_context(|| format!("malformed selected {label} payload should fail"))?;

            if !error.to_string().contains(expected_error)
                || fs::read(directory.path().join("settings.json"))? != old_settings
                || fs::read(directory.path().join("idls.json"))? != old_idl
                || fs::read(directory.path().join("wallet.json"))? != old_wallet
            {
                bail!("malformed selected {label} payload mutated local state: {error:#}");
            }
        }
        Ok(())
    }

    #[test]
    fn selective_backup_flows_do_not_parse_unselected_local_state() -> Result<()> {
        let directory = tempfile::tempdir().context("failed to create backup test directory")?;
        let current_settings = br#"{"version":2,"theme":"old","channel_source_configs":[]}"#;
        let malformed_idl = b"{malformed-idl";
        let malformed_wallet = b"{malformed-wallet-secret";
        fs::write(directory.path().join("settings.json"), current_settings)?;
        fs::write(directory.path().join("idls.json"), malformed_idl)?;
        fs::write(directory.path().join("wallet.json"), malformed_wallet)?;

        let payload = backup_payload_from_states(
            &json!({
                "version": 2,
                "theme": "new",
                "favorites": [{ "value": "account-new" }],
                "channel_source_configs": []
            }),
            &json!({ "version": 1, "idls": [{ "key": "ignored" }] }),
            &json!({ "profile": { "label": "Ignored wallet" } }),
            false,
            None,
        )?;
        let restored = restored_state_from_payload(&payload, None)?;
        let selected_options = json!({
            "settings": "replace",
            "favorites": "replace",
            "idl_registry": "skip",
            "wallet_profile": "skip"
        });
        let selected_options = BackupImportOptions::parse(Some(&selected_options))?;

        let preview = preview_app_settings_backup_import_in_dir(
            directory.path(),
            &restored,
            &selected_options,
        )?;
        if preview.get("settings").and_then(Value::as_bool) != Some(true) {
            bail!("selective preview omitted selected settings: {preview}");
        }
        restore_app_settings_backup_in_dir(directory.path(), &restored, &selected_options)?;
        if fs::read(directory.path().join("idls.json"))? != malformed_idl
            || fs::read(directory.path().join("wallet.json"))? != malformed_wallet
        {
            bail!("selective restore modified unselected malformed state");
        }

        let exported = export_app_settings_backup_in_dir(
            directory.path(),
            false,
            None,
            Some(&json!({ "settings": true })),
        )?;
        let exported_state = exported
            .get("state")
            .and_then(Value::as_object)
            .context("selective export state is missing")?;
        if exported_state.get("settings").is_none()
            || exported_state.get("idls").is_some()
            || exported_state.get("wallet").is_some()
        {
            bail!("selective export included unselected state: {exported}");
        }

        fs::write(directory.path().join("settings.json"), current_settings)?;
        let skip_all = json!({
            "settings": "skip",
            "favorites": "skip",
            "idl_registry": "skip",
            "wallet_profile": "skip"
        });
        let skip_all = BackupImportOptions::parse(Some(&skip_all))?;
        preview_app_settings_backup_import_in_dir(directory.path(), &restored, &skip_all)?;
        restore_app_settings_backup_in_dir(directory.path(), &restored, &skip_all)?;
        if fs::read(directory.path().join("settings.json"))? != current_settings {
            bail!("all-skip restore parsed or modified current settings");
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
        validate_app_settings_backup_envelope(&payload)?;
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
            "settings": "replace",
            "favorites": "merge",
            "idl_registry": "merge",
            "wallet_profile": "skip",
            "conflicts": {
                "idl_registry": {
                    "idl-a": "replace_existing"
                }
            }
        });
        let options = BackupImportOptions::parse(Some(&options))?;
        let settings = planned_settings_state(
            restored.settings.as_ref(),
            Some(&current_settings),
            &options,
            true,
        )?
        .state
        .context("settings plan missing")?;
        let idl = planned_idl_state(restored.idl.as_ref(), Some(&current_idl), &options, true)?
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
        let selection = BackupImportOptions::parse(Some(&json!({
            "favorites": "merge"
        })))?;
        let preview = planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &selection,
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
            true,
        )
        .is_ok()
        {
            bail!("favorite conflict should require explicit decision before apply");
        }

        let replace_options = json!({
            "favorites": "merge",
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
        let replace_options = BackupImportOptions::parse(Some(&replace_options))?;
        let replaced = planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &replace_options,
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
            "favorites": "merge",
            "conflicts": {
                "favorites": {
                    "account:l2:account:account-1": "skip_backup_item"
                }
            }
        });
        let skip_options = BackupImportOptions::parse(Some(&skip_options))?;
        let skipped = planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &skip_options,
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
        let merge_options = BackupImportOptions::parse(Some(&json!({
            "idl_registry": "merge"
        })))?;
        let preview =
            planned_idl_state(Some(&backup_idl), Some(&current_idl), &merge_options, false)?;
        if preview.idl_registry.conflicts.len() != 1 {
            bail!("IDL conflict was not reported");
        }
        if planned_idl_state(Some(&backup_idl), Some(&current_idl), &merge_options, true).is_ok() {
            bail!("IDL conflict should require explicit decision before apply");
        }

        let replace_options = json!({
            "idl_registry": "merge",
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
        let replace_options = BackupImportOptions::parse(Some(&replace_options))?;
        let replaced = planned_idl_state(
            Some(&backup_idl),
            Some(&current_idl),
            &replace_options,
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
            "idl_registry": "merge",
            "conflicts": {
                "idl_registry": {
                    "idl-a": "skip_backup_item"
                }
            }
        });
        let skip_options = BackupImportOptions::parse(Some(&skip_options))?;
        let skipped =
            planned_idl_state(Some(&backup_idl), Some(&current_idl), &skip_options, true)?
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
    fn idl_item_selection_keeps_account_mappings_for_selected_idls_only() -> Result<()> {
        let backup_idl = json!({
            "idls": [
                { "key": "idl-a", "name": "Selected" },
                { "key": "idl-b", "name": "Skipped" }
            ],
            "account_idl_selections": {
                "account-a": {
                    "idlKey": "IDL-A",
                    "accountType": "AccountA",
                    "ownerProgram": "owner-a",
                    "network": "devnet"
                },
                "account-b": {
                    "idlKey": "idl-b",
                    "accountType": "AccountB",
                    "ownerProgram": "owner-b",
                    "network": "devnet"
                }
            }
        });
        let current_idl = json!({
            "idls": [],
            "account_idl_selections": {}
        });
        let options = BackupImportOptions::parse(Some(&json!({
            "idl_registry": "merge",
            "items": {
                "idl_registry": {
                    "idl-a": true,
                    "idl-b": false
                }
            }
        })))?;

        let planned = planned_idl_state(Some(&backup_idl), Some(&current_idl), &options, true)?
            .state
            .context("selected IDL plan is missing")?;
        let rows = planned
            .get("idls")
            .and_then(Value::as_array)
            .context("selected IDL rows are missing")?;
        let selections = planned
            .get("account_idl_selections")
            .and_then(Value::as_object)
            .context("selected IDL account mappings are missing")?;

        if rows.len() != 1
            || rows
                .first()
                .and_then(|row| row.get("key"))
                .and_then(Value::as_str)
                != Some("idl-a")
            || selections
                .get("account-a")
                .and_then(|value| value.get("idlKey"))
                .and_then(Value::as_str)
                != Some("IDL-A")
            || selections.contains_key("account-b")
        {
            bail!("IDL item selection drifted from account mappings: {planned}");
        }
        Ok(())
    }

    #[test]
    fn settings_replace_with_favorites_skip_never_imports_backup_favorites() -> Result<()> {
        let backup_settings = json!({
            "theme": "dark",
            "favorites": [{ "value": "backup-favorite" }]
        });
        let current_settings = json!({
            "theme": "light",
            "favorites": [{ "value": "current-favorite" }]
        });
        let options = BackupImportOptions::parse(Some(&json!({
            "settings": "replace",
            "favorites": "skip"
        })))?;

        let planned = planned_settings_state(
            Some(&backup_settings),
            Some(&current_settings),
            &options,
            true,
        )?
        .state
        .context("settings replacement plan is missing")?;

        if planned.get("theme").and_then(Value::as_str) != Some("dark")
            || planned
                .pointer("/favorites/0/value")
                .and_then(Value::as_str)
                != Some("current-favorite")
        {
            bail!("settings replacement imported skipped backup favorites: {planned}");
        }

        let planned_without_current_favorites = planned_settings_state(
            Some(&backup_settings),
            Some(&json!({ "theme": "light" })),
            &options,
            true,
        )?
        .state
        .context("settings replacement plan without current favorites is missing")?;
        if planned_without_current_favorites.get("favorites").is_some() {
            bail!(
                "settings replacement retained skipped backup favorites when current favorites were absent: {planned_without_current_favorites}"
            );
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
        let options = BackupImportOptions::parse(Some(&json!({
            "settings": "replace",
            "favorites": "skip",
            "wallet_profile": "skip"
        })))?;
        let plan = build_import_plan(&restored, &options, false)?;

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

        let options = BackupImportOptions::parse(Some(&json!({ "wallet_profile": "replace" })))?;
        let plan = build_import_plan(&restored, &options, false)?;
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

        let options = BackupImportOptions::parse(Some(&json!({ "wallet_profile": "replace" })))?;
        let plan = build_import_plan(&restored, &options, false)?;
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

        let options = BackupImportOptions::parse(Some(&json!({
            "settings": "replace",
            "wallet_profile": "skip"
        })))?;
        let plan = build_import_plan(&restored, &options, false)?;

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

        let plan = build_import_plan(&restored, &BackupImportOptions::default(), false)?;

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
    fn zero_favorite_replace_remains_an_applied_area() -> Result<()> {
        let restored = RestoredState {
            settings: Some(json!({ "favorites": [] })),
            idl: None,
            wallet: None,
            summary: RestoreSummary {
                settings_restored: true,
                idl_restored: false,
                wallet_restored: false,
                favorites_count: 0,
                idl_count: 0,
                encrypted: false,
            },
        };
        let options = BackupImportOptions::parse(Some(&json!({
            "favorites": "replace"
        })))?;

        let plan = build_import_plan_with_current(
            &restored,
            &options,
            true,
            Some(&json!({ "favorites": [{ "value": "remove-me" }] })),
            None,
        )?;

        if plan.summary.get("favorites").and_then(Value::as_u64) != Some(0)
            || !plan
                .summary
                .get("applied_areas")
                .and_then(Value::as_array)
                .is_some_and(|areas| areas.contains(&json!("favorites")))
        {
            bail!(
                "zero-favorite replace lost its applied area: {}",
                plan.summary
            );
        }
        Ok(())
    }

    #[test]
    fn import_plan_rejects_merge_for_replace_only_sections() -> Result<()> {
        if BackupImportOptions::parse(Some(&json!({ "settings": "merge" }))).is_ok() {
            bail!("settings merge should be rejected");
        }
        if BackupImportOptions::parse(Some(&json!({ "wallet_profile": "merge" }))).is_ok() {
            bail!("wallet profile merge should be rejected");
        }
        Ok(())
    }

    fn backup_test_channel_config(
        channel_character: char,
        source_character: char,
        revision: u64,
        label: &str,
        port: u16,
    ) -> Value {
        json!({
            "network_scope": {
                "kind": "genesis_id",
                "genesis_id": "a".repeat(64)
            },
            "channel_id": channel_character.to_string().repeat(64),
            "config_revision": revision,
            "sequencer_sources": [{
                "source_id": format!("src_{}", source_character.to_string().repeat(32)),
                "label": label,
                "target": {
                    "kind": "rpc",
                    "endpoint": format!("https://sequencer-{port}.example/")
                },
                "channel_attestation": { "state": "pending" }
            }],
            "selected_sequencer_source_id": null,
            "indexer_source": null
        })
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
