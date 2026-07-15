use std::{
    collections::BTreeSet,
    fs::{self, File, OpenOptions},
    io::{Read as _, Write as _},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::support::state_store::config_dir;

use super::local_state::LocalStateCommitCancellation;
use super::settings_backup::{
    BackupImportOptions, BackupImportSelection, SETTINGS_BACKUP_MAX_BYTES,
    SettingsBackupRestoreReceipt, ensure_settings_backup_size, export_app_settings_backup,
    preview_app_settings_backup_import, restore_app_settings_backup_with_options,
    validate_app_settings_backup_envelope,
};

#[cfg(test)]
use super::{
    local_state::{LocalStateTestBoundary, LocalStateTestFault},
    settings_backup::{
        preview_app_settings_backup_import_in_dir_for_test,
        restore_app_settings_backup_at_boundary_in_dir_for_test,
        restore_app_settings_backup_in_dir_for_test,
        restore_app_settings_backup_with_fault_in_dir_for_test,
    },
};

const CATALOG_VERSION: u64 = 1;
const BACKUP_PAYLOAD_DIR: &str = "backup-payloads";
const CATALOG_LOCK_FILE: &str = ".backup-catalog.lock";
const CATALOG_STAGE_PREFIX: &str = ".backup-catalog.stage.";
const PAYLOAD_STAGE_PREFIX: &str = ".backup-payload.stage.";
const BACKUP_CATALOG_ID_MAX_BYTES: usize = 128;
const BACKUP_CID_MAX_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct BackupCatalogId(String);

impl BackupCatalogId {
    pub(crate) fn parse(value: &str) -> Result<Self> {
        let value = value.trim();
        if value.is_empty() {
            bail!("backup catalog id is required");
        }
        if value.len() > BACKUP_CATALOG_ID_MAX_BYTES {
            bail!("backup catalog id exceeds {BACKUP_CATALOG_ID_MAX_BYTES} byte limit");
        }
        if value == "."
            || value == ".."
            || value.contains(['/', '\\'])
            || value.chars().any(char::is_control)
        {
            bail!("backup catalog id contains an unsafe path component");
        }
        Ok(Self(value.to_owned()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug)]
pub(crate) struct LocalBackupImportReceipt {
    backup_catalog_id: BackupCatalogId,
    selection: BackupImportSelection,
    restore: SettingsBackupRestoreReceipt,
}

impl LocalBackupImportReceipt {
    pub(crate) fn into_parts(
        self,
    ) -> (
        BackupCatalogId,
        BackupImportSelection,
        SettingsBackupRestoreReceipt,
    ) {
        (self.backup_catalog_id, self.selection, self.restore)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BackupCatalog {
    pub(crate) version: u64,
    pub(crate) entries: Vec<BackupCatalogEntry>,
}

impl Default for BackupCatalog {
    fn default() -> Self {
        Self {
            version: CATALOG_VERSION,
            entries: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BackupCatalogEntry {
    pub(crate) backup_catalog_id: String,
    pub(crate) payload_id: String,
    pub(crate) backup_version_label: String,
    pub(crate) created_at: String,
    pub(crate) contents: Vec<String>,
    pub(crate) encrypted: bool,
    pub(crate) local_payload_path: String,
    pub(crate) remote: Option<RemoteBackupMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RemoteBackupMetadata {
    pub(crate) cid: String,
    pub(crate) provider: String,
    pub(crate) uploaded_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BackupCatalogIoPoint {
    LockAttempt,
    LockAcquired,
    PayloadPersisted,
    CatalogPersisted,
}

#[derive(Debug, PartialEq, Eq)]
enum BackupCatalogCommitReceipt {
    Durable,
    VisibleDurabilityUnconfirmed { error: String },
}

impl BackupCatalogCommitReceipt {
    fn with_value<T>(self, value: T) -> CommittedBackupCatalog<T> {
        match self {
            Self::Durable => CommittedBackupCatalog::Durable(value),
            Self::VisibleDurabilityUnconfirmed { error } => {
                CommittedBackupCatalog::VisibleDurabilityUnconfirmed { value, error }
            }
        }
    }
}

#[derive(Debug)]
#[must_use = "catalog commit visibility must be acknowledged"]
enum CommittedBackupCatalog<T> {
    Durable(T),
    VisibleDurabilityUnconfirmed { value: T, error: String },
}

impl<T> CommittedBackupCatalog<T> {
    fn into_value(self) -> T {
        match self {
            Self::Durable(value) => value,
            Self::VisibleDurabilityUnconfirmed { value, error } => {
                drop(error);
                value
            }
        }
    }
}

trait BackupCatalogIoHook {
    fn at(&mut self, _point: BackupCatalogIoPoint) -> Result<()> {
        Ok(())
    }
}

struct NoopBackupCatalogIoHook;

impl BackupCatalogIoHook for NoopBackupCatalogIoHook {}

struct PreparedPayload {
    path: PathBuf,
    bytes: Vec<u8>,
}

struct BackupCatalogTransaction {
    base_dir: PathBuf,
    catalog_path: PathBuf,
    original_catalog: BackupCatalog,
    catalog: BackupCatalog,
    _lock_file: File,
}

pub(crate) fn load_backup_catalog() -> Result<BackupCatalog> {
    load_catalog_from_path(&backup_catalog_path()?)
}

pub(crate) fn load_backup_catalog_value() -> Result<Value> {
    serde_json::to_value(load_backup_catalog()?).context("failed to serialize backup catalog")
}

pub(crate) fn create_local_settings_backup(
    label: Option<&str>,
    encrypted: bool,
    wallet_profile: Option<&Value>,
    content_options: Option<&Value>,
) -> Result<BackupCatalogEntry> {
    let base_dir = config_dir()?;
    let payload = export_app_settings_backup(encrypted, wallet_profile, content_options)?;
    record_payload_in_dir(&base_dir, label, &payload)
}

pub(crate) fn record_remote_settings_backup_payload_in_dir(
    base_dir: &Path,
    label: Option<&str>,
    payload: &Value,
    cid: &str,
    provider: Option<&str>,
) -> Result<BackupCatalogEntry> {
    validate_app_settings_backup_envelope(payload)?;
    record_remote_payload_in_dir(base_dir, label, payload, cid, provider)
}

pub(crate) fn backup_payload_bytes(backup_catalog_id: &str) -> Result<Vec<u8>> {
    let base_dir = config_dir()?;
    backup_payload_bytes_in_dir(&base_dir, backup_catalog_id)
}

fn backup_payload_bytes_in_dir(base_dir: &Path, backup_catalog_id: &str) -> Result<Vec<u8>> {
    let catalog_id = BackupCatalogId::parse(backup_catalog_id)?;
    let catalog = load_catalog_from_path(&catalog_path_for_dir(base_dir))?;
    let entry = ensure_catalog_entry(&catalog, catalog_id.as_str())?;
    let path = payload_path(base_dir, catalog_id.as_str());
    let bytes = read_backup_payload_file(&path)?;
    let actual_payload_id = payload_identity(&bytes);
    if entry.payload_id != actual_payload_id {
        bail!(
            "backup catalog entry `{}` payload identity does not match the catalog",
            catalog_id.as_str()
        );
    }
    Ok(bytes)
}

pub(crate) fn preview_local_settings_restore_with_options(
    backup_catalog_id: &str,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
) -> Result<Value> {
    let (catalog_id, payload) = backup_payload_value(backup_catalog_id)?;
    let mut summary = preview_app_settings_backup_import(&payload, wallet_profile, options)?;
    set_summary_catalog_id(&mut summary, catalog_id.as_str())?;
    set_summary_import_areas(&mut summary, options)?;
    Ok(summary)
}

pub(crate) fn restore_local_settings_backup_with_options(
    backup_catalog_id: &str,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
) -> Result<LocalBackupImportReceipt> {
    if options.is_empty() {
        bail!("select at least one backup section to import");
    }
    let (catalog_id, payload) = backup_payload_value(backup_catalog_id)?;
    let restore =
        restore_app_settings_backup_with_options(&payload, wallet_profile, options, cancellation)?;
    Ok(LocalBackupImportReceipt {
        backup_catalog_id: catalog_id,
        selection: options.selection().clone(),
        restore,
    })
}

#[cfg(test)]
pub(crate) fn preview_local_settings_restore_with_options_in_dir_for_test(
    base_dir: &Path,
    backup_catalog_id: &str,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
) -> Result<Value> {
    let (catalog_id, payload) = backup_payload_value_in_dir(base_dir, backup_catalog_id)?;
    let mut summary = preview_app_settings_backup_import_in_dir_for_test(
        base_dir,
        &payload,
        wallet_profile,
        options,
    )?;
    set_summary_catalog_id(&mut summary, catalog_id.as_str())?;
    set_summary_import_areas(&mut summary, options)?;
    Ok(summary)
}

#[cfg(test)]
pub(crate) fn restore_local_settings_backup_with_options_in_dir_for_test(
    base_dir: &Path,
    backup_catalog_id: &str,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
    fault: Option<LocalStateTestFault>,
) -> Result<LocalBackupImportReceipt> {
    if options.is_empty() {
        bail!("select at least one backup section to import");
    }
    let (catalog_id, payload) = backup_payload_value_in_dir(base_dir, backup_catalog_id)?;
    let restore = match fault {
        Some(fault) => restore_app_settings_backup_with_fault_in_dir_for_test(
            base_dir,
            &payload,
            wallet_profile,
            options,
            cancellation,
            fault,
        )?,
        None => restore_app_settings_backup_in_dir_for_test(
            base_dir,
            &payload,
            wallet_profile,
            options,
            cancellation,
        )?,
    };
    Ok(LocalBackupImportReceipt {
        backup_catalog_id: catalog_id,
        selection: options.selection().clone(),
        restore,
    })
}

#[cfg(test)]
pub(crate) fn restore_local_settings_backup_at_boundary_in_dir_for_test(
    base_dir: &Path,
    backup_catalog_id: &str,
    wallet_profile: Option<&Value>,
    options: &BackupImportOptions,
    cancellation: &LocalStateCommitCancellation,
    boundary: LocalStateTestBoundary,
    at_boundary: impl FnMut() -> Result<()>,
) -> Result<LocalBackupImportReceipt> {
    if options.is_empty() {
        bail!("select at least one backup section to import");
    }
    let (catalog_id, payload) = backup_payload_value_in_dir(base_dir, backup_catalog_id)?;
    let restore = restore_app_settings_backup_at_boundary_in_dir_for_test(
        base_dir,
        &payload,
        wallet_profile,
        options,
        cancellation,
        boundary,
        at_boundary,
    )?;
    Ok(LocalBackupImportReceipt {
        backup_catalog_id: catalog_id,
        selection: options.selection().clone(),
        restore,
    })
}

fn record_payload_in_dir(
    base_dir: &Path,
    label: Option<&str>,
    payload: &Value,
) -> Result<BackupCatalogEntry> {
    let payload_text =
        serde_json::to_vec_pretty(payload).context("failed to serialize backup payload")?;
    ensure_settings_backup_size(payload_text.len())?;
    let payload_id = payload_identity(&payload_text);
    let encrypted = payload
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let mut hook = NoopBackupCatalogIoHook;
    let mut transaction = BackupCatalogTransaction::begin(base_dir, &mut hook)?;
    let (entry, prepared_payload) = create_entry_from_payload(
        base_dir,
        &mut transaction.catalog,
        label,
        payload,
        encrypted,
        payload_text,
        payload_id,
    )?;
    Ok(transaction
        .commit(Some(prepared_payload), &mut hook)?
        .with_value(entry)
        .into_value())
}

fn record_remote_payload_in_dir(
    base_dir: &Path,
    label: Option<&str>,
    payload: &Value,
    cid: &str,
    provider: Option<&str>,
) -> Result<BackupCatalogEntry> {
    let mut hook = NoopBackupCatalogIoHook;
    Ok(
        record_remote_payload_in_dir_with_hook(base_dir, label, payload, cid, provider, &mut hook)?
            .into_value(),
    )
}

fn record_remote_payload_in_dir_with_hook(
    base_dir: &Path,
    label: Option<&str>,
    payload: &Value,
    cid: &str,
    provider: Option<&str>,
    hook: &mut impl BackupCatalogIoHook,
) -> Result<CommittedBackupCatalog<BackupCatalogEntry>> {
    let cid = cid.trim();
    validate_remote_cid(cid)?;
    let payload_text =
        serde_json::to_vec_pretty(payload).context("failed to serialize backup payload")?;
    ensure_settings_backup_size(payload_text.len())?;
    let payload_id = payload_identity(&payload_text);
    let mut transaction = BackupCatalogTransaction::begin(base_dir, hook)?;
    if let Some(existing) = transaction
        .catalog
        .entries
        .iter()
        .find(|entry| {
            entry
                .remote
                .as_ref()
                .is_some_and(|remote| remote.cid == cid)
        })
        .cloned()
    {
        if existing.payload_id != payload_id {
            bail!("remote backup CID `{cid}` conflicts with existing payload identity");
        }
        let expected_payload_path = payload_path(base_dir, &existing.backup_catalog_id);
        let stored_payload_is_current = read_backup_payload_file(&expected_payload_path)
            .is_ok_and(|stored| payload_identity(&stored) == payload_id);
        let prepared_payload = (!stored_payload_is_current).then_some(PreparedPayload {
            path: expected_payload_path.clone(),
            bytes: payload_text,
        });
        let mut result = existing.clone();
        if Path::new(&existing.local_payload_path) != expected_payload_path {
            let stored = transaction
                .catalog
                .entries
                .iter_mut()
                .find(|entry| entry.backup_catalog_id == existing.backup_catalog_id)
                .context("accepted remote backup entry disappeared during repair")?;
            stored.local_payload_path = expected_payload_path.display().to_string();
            result = stored.clone();
        }
        return Ok(transaction
            .commit(prepared_payload, hook)?
            .with_value(result));
    }
    let encrypted = payload
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let remote = remote_backup_metadata(cid, provider);

    let (entry, prepared_payload) = if let Some(entry) = transaction
        .catalog
        .entries
        .iter_mut()
        .find(|entry| entry.payload_id == payload_id && entry.remote.is_none())
    {
        let local_payload_path = payload_path(base_dir, &entry.backup_catalog_id);
        entry.payload_id.clone_from(&payload_id);
        entry.contents = backup_contents(payload);
        entry.encrypted = encrypted;
        entry.local_payload_path = local_payload_path.display().to_string();
        if let Some(label) = label.map(str::trim).filter(|value| !value.is_empty()) {
            entry.backup_version_label = label.to_owned();
        }
        entry.remote = Some(remote);
        (
            entry.clone(),
            PreparedPayload {
                path: local_payload_path,
                bytes: payload_text,
            },
        )
    } else {
        let (entry, prepared_payload) = create_entry_from_payload(
            base_dir,
            &mut transaction.catalog,
            label,
            payload,
            encrypted,
            payload_text,
            payload_id,
        )?;
        let stored = transaction
            .catalog
            .entries
            .iter_mut()
            .find(|stored| stored.backup_catalog_id == entry.backup_catalog_id)
            .context("new remote backup entry disappeared before commit")?;
        stored.remote = Some(remote);
        (stored.clone(), prepared_payload)
    };

    Ok(transaction
        .commit(Some(prepared_payload), hook)?
        .with_value(entry))
}

pub(crate) fn attach_remote_backup_metadata(
    backup_catalog_id: &str,
    cid: &str,
    provider: Option<&str>,
) -> Result<BackupCatalogEntry> {
    let base_dir = config_dir()?;
    attach_remote_backup_metadata_in_dir(&base_dir, backup_catalog_id, cid, provider)
}

fn attach_remote_backup_metadata_in_dir(
    base_dir: &Path,
    backup_catalog_id: &str,
    cid: &str,
    provider: Option<&str>,
) -> Result<BackupCatalogEntry> {
    let mut hook = NoopBackupCatalogIoHook;
    Ok(attach_remote_backup_metadata_in_dir_with_hook(
        base_dir,
        backup_catalog_id,
        cid,
        provider,
        &mut hook,
    )?
    .into_value())
}

fn attach_remote_backup_metadata_in_dir_with_hook(
    base_dir: &Path,
    backup_catalog_id: &str,
    cid: &str,
    provider: Option<&str>,
    hook: &mut impl BackupCatalogIoHook,
) -> Result<CommittedBackupCatalog<BackupCatalogEntry>> {
    let catalog_id = BackupCatalogId::parse(backup_catalog_id)?;
    let cid = cid.trim();
    validate_remote_cid(cid)?;
    let provider = remote_backup_provider(provider);

    let mut transaction = BackupCatalogTransaction::begin(base_dir, hook)?;
    let target_index = transaction
        .catalog
        .entries
        .iter()
        .position(|entry| entry.backup_catalog_id == catalog_id.as_str())
        .with_context(|| {
            format!(
                "backup catalog entry `{}` was not found",
                catalog_id.as_str()
            )
        })?;
    let target_payload_id = transaction
        .catalog
        .entries
        .get(target_index)
        .context("backup catalog target disappeared before metadata validation")?
        .payload_id
        .clone();
    if let Some(existing) = transaction
        .catalog
        .entries
        .iter()
        .enumerate()
        .find(|(index, entry)| {
            *index != target_index
                && entry
                    .remote
                    .as_ref()
                    .is_some_and(|remote| remote.cid == cid)
        })
        .map(|(_, entry)| entry)
    {
        if existing.payload_id != target_payload_id {
            bail!("remote backup CID `{cid}` conflicts with existing payload identity");
        }
        bail!(
            "remote backup CID `{cid}` is already attached to backup catalog entry `{}`",
            existing.backup_catalog_id
        );
    }

    let entry = transaction
        .catalog
        .entries
        .get_mut(target_index)
        .context("backup catalog target disappeared before metadata attachment")?;
    match entry.remote.as_ref() {
        Some(remote) if remote.cid == cid && remote.provider == provider => {
            return Ok(CommittedBackupCatalog::Durable(entry.clone()));
        }
        Some(remote) => {
            bail!(
                "backup catalog entry `{}` already has remote metadata for CID `{}` from provider `{}`",
                catalog_id.as_str(),
                remote.cid,
                remote.provider
            );
        }
        None => {}
    }
    entry.remote = Some(RemoteBackupMetadata {
        cid: cid.to_owned(),
        provider,
        uploaded_at: unix_time_text(),
    });
    let result = entry.clone();
    Ok(transaction.commit(None, hook)?.with_value(result))
}

fn backup_payload_value(backup_catalog_id: &str) -> Result<(BackupCatalogId, Value)> {
    let base_dir = config_dir()?;
    backup_payload_value_in_dir(&base_dir, backup_catalog_id)
}

fn backup_payload_value_in_dir(
    base_dir: &Path,
    backup_catalog_id: &str,
) -> Result<(BackupCatalogId, Value)> {
    let catalog_id = BackupCatalogId::parse(backup_catalog_id)?;
    let bytes = backup_payload_bytes_in_dir(base_dir, catalog_id.as_str())?;
    let payload = serde_json::from_slice(&bytes).with_context(|| {
        format!(
            "backup catalog entry `{}` payload does not contain JSON",
            catalog_id.as_str()
        )
    })?;
    Ok((catalog_id, payload))
}

fn ensure_catalog_entry<'a>(
    catalog: &'a BackupCatalog,
    backup_catalog_id: &str,
) -> Result<&'a BackupCatalogEntry> {
    catalog
        .entries
        .iter()
        .find(|entry| entry.backup_catalog_id == backup_catalog_id)
        .with_context(|| format!("backup catalog entry `{backup_catalog_id}` was not found"))
}

fn set_summary_catalog_id(summary: &mut Value, backup_catalog_id: &str) -> Result<()> {
    let object = summary
        .as_object_mut()
        .context("backup restore summary is not an object")?;
    object.insert(
        "backup_catalog_id".to_owned(),
        Value::String(backup_catalog_id.to_owned()),
    );
    Ok(())
}

fn set_summary_import_areas(summary: &mut Value, options: &BackupImportOptions) -> Result<()> {
    let object = summary
        .as_object_mut()
        .context("backup restore summary is not an object")?;
    object.insert(
        "selected_areas".to_owned(),
        Value::Array(
            options
                .selected_areas()
                .into_iter()
                .map(|area| Value::String(area.as_str().to_owned()))
                .collect(),
        ),
    );
    object.insert(
        "affected_areas".to_owned(),
        Value::Array(
            options
                .affected_areas()
                .into_iter()
                .map(|area| Value::String(area.as_str().to_owned()))
                .collect(),
        ),
    );
    Ok(())
}

fn load_catalog_from_path(path: &Path) -> Result<BackupCatalog> {
    if !path.is_file() {
        return Ok(BackupCatalog::default());
    }
    let text = fs::read_to_string(path)
        .with_context(|| format!("failed to read backup catalog from {}", path.display()))?;
    let catalog: BackupCatalog = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse backup catalog from {}", path.display()))?;
    if catalog.version != CATALOG_VERSION {
        bail!("backup catalog version is not supported");
    }
    validate_loaded_catalog(path, &catalog)?;
    Ok(catalog)
}

fn validate_loaded_catalog(path: &Path, catalog: &BackupCatalog) -> Result<()> {
    let base_dir = path
        .parent()
        .context("backup catalog path has no parent directory")?;
    let mut catalog_ids = BTreeSet::new();
    let mut remote_cids = BTreeSet::new();
    for (index, entry) in catalog.entries.iter().enumerate() {
        let catalog_id = BackupCatalogId::parse(&entry.backup_catalog_id)
            .with_context(|| format!("backup catalog entry {index} has an invalid id"))?;
        if catalog_id.as_str() != entry.backup_catalog_id {
            bail!("backup catalog entry {index} has a noncanonical id");
        }
        if !catalog_ids.insert(entry.backup_catalog_id.as_str()) {
            bail!(
                "backup catalog contains duplicate id `{}`",
                entry.backup_catalog_id
            );
        }
        validate_payload_identity(&entry.payload_id)
            .with_context(|| format!("backup catalog entry {index} has an invalid payload id"))?;
        let expected_payload_path = payload_path(base_dir, catalog_id.as_str());
        if Path::new(&entry.local_payload_path) != expected_payload_path {
            bail!("backup catalog entry {index} has a noncanonical local payload path");
        }
        if let Some(remote) = &entry.remote {
            validate_remote_cid(&remote.cid).with_context(|| {
                format!("backup catalog entry {index} has an invalid remote CID")
            })?;
            if !remote_cids.insert(remote.cid.as_str()) {
                bail!(
                    "backup catalog contains duplicate remote CID `{}`",
                    remote.cid
                );
            }
        }
    }
    Ok(())
}

fn validate_payload_identity(value: &str) -> Result<()> {
    let Some(digest) = value.strip_prefix("sha256:") else {
        bail!("backup payload identity must use sha256");
    };
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        bail!("backup payload identity has an invalid sha256 digest");
    }
    Ok(())
}

fn validate_remote_cid(value: &str) -> Result<()> {
    if value.is_empty() {
        bail!("remote backup CID is required");
    }
    if value.len() > BACKUP_CID_MAX_BYTES {
        bail!("remote backup CID exceeds {BACKUP_CID_MAX_BYTES} byte limit");
    }
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        bail!("remote backup CID contains unsupported characters");
    }
    Ok(())
}

impl BackupCatalogTransaction {
    fn begin(base_dir: &Path, hook: &mut impl BackupCatalogIoHook) -> Result<Self> {
        fs::create_dir_all(base_dir).with_context(|| {
            format!(
                "failed to create backup catalog directory {}",
                base_dir.display()
            )
        })?;
        let lock_path = base_dir.join(CATALOG_LOCK_FILE);
        let lock_file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&lock_path)
            .with_context(|| {
                format!("failed to open backup catalog lock {}", lock_path.display())
            })?;
        hook.at(BackupCatalogIoPoint::LockAttempt)?;
        lock_file
            .lock()
            .with_context(|| format!("failed to lock backup catalog at {}", lock_path.display()))?;
        hook.at(BackupCatalogIoPoint::LockAcquired)?;

        cleanup_staged_files(base_dir, CATALOG_STAGE_PREFIX)?;
        let payload_dir = base_dir.join(BACKUP_PAYLOAD_DIR);
        cleanup_staged_files(&payload_dir, PAYLOAD_STAGE_PREFIX)?;
        let catalog_path = catalog_path_for_dir(base_dir);
        let catalog = load_catalog_from_path(&catalog_path)?;
        cleanup_orphan_payloads(base_dir, &catalog)?;

        Ok(Self {
            base_dir: base_dir.to_owned(),
            catalog_path,
            original_catalog: catalog.clone(),
            catalog,
            _lock_file: lock_file,
        })
    }

    fn commit(
        self,
        prepared_payload: Option<PreparedPayload>,
        hook: &mut impl BackupCatalogIoHook,
    ) -> Result<BackupCatalogCommitReceipt> {
        let catalog_changed = self.catalog != self.original_catalog;
        if prepared_payload.is_none() && !catalog_changed {
            return Ok(BackupCatalogCommitReceipt::Durable);
        }

        let payload_referenced_before = prepared_payload.as_ref().is_some_and(|prepared| {
            self.original_catalog.entries.iter().any(|entry| {
                payload_path(&self.base_dir, &entry.backup_catalog_id) == prepared.path
            })
        });
        let payload_existed_before = prepared_payload
            .as_ref()
            .map(|prepared| path_exists(&prepared.path))
            .transpose()?
            .unwrap_or(false);
        let payload_target = prepared_payload
            .as_ref()
            .map(|prepared| prepared.path.clone());
        let mut staged_payload = prepared_payload
            .as_ref()
            .map(|prepared| {
                stage_replacement(
                    &prepared.path,
                    &prepared.bytes,
                    PAYLOAD_STAGE_PREFIX,
                    "backup payload",
                )
            })
            .transpose()?;
        let mut staged_catalog = if catalog_changed {
            let catalog_bytes = serde_json::to_vec_pretty(&self.catalog)
                .context("failed to serialize backup catalog")?;
            Some(stage_replacement(
                &self.catalog_path,
                &catalog_bytes,
                CATALOG_STAGE_PREFIX,
                "backup catalog",
            )?)
        } else {
            None
        };

        let mut payload_installed = false;
        let mut catalog_installed = false;
        let commit_result = (|| -> Result<()> {
            if let (Some(staged), Some(target)) = (staged_payload.take(), payload_target.as_ref()) {
                persist_replacement(staged, target, "backup payload")?;
                payload_installed = true;
                hook.at(BackupCatalogIoPoint::PayloadPersisted)?;
                let parent = target
                    .parent()
                    .context("backup payload path has no parent directory")?;
                sync_directory(parent, "backup payload")?;
                sync_directory(&self.base_dir, "backup catalog")?;
            }
            if let Some(staged) = staged_catalog.take() {
                persist_replacement(staged, &self.catalog_path, "backup catalog")?;
                catalog_installed = true;
                hook.at(BackupCatalogIoPoint::CatalogPersisted)?;
                sync_directory(&self.base_dir, "backup catalog")?;
            }
            Ok(())
        })();

        if let Err(error) = commit_result {
            if catalog_installed
                || (payload_installed && payload_referenced_before && !catalog_changed)
            {
                return Ok(BackupCatalogCommitReceipt::VisibleDurabilityUnconfirmed {
                    error: format!("{error:#}"),
                });
            }
            let new_uncommitted_payload = payload_installed
                && !catalog_installed
                && !payload_referenced_before
                && !payload_existed_before;
            if new_uncommitted_payload
                && let Some(target) = payload_target.as_ref()
                && let Err(rollback_error) = remove_uncommitted_payload(&self.base_dir, target)
            {
                bail!("{error:#}; failed to remove uncommitted backup payload: {rollback_error:#}");
            }
            return Err(error);
        }
        Ok(BackupCatalogCommitReceipt::Durable)
    }
}

fn create_entry_from_payload(
    base_dir: &Path,
    catalog: &mut BackupCatalog,
    label: Option<&str>,
    payload: &Value,
    encrypted: bool,
    payload_text: Vec<u8>,
    payload_id: String,
) -> Result<(BackupCatalogEntry, PreparedPayload)> {
    ensure_settings_backup_size(payload_text.len())?;
    let created_at = unix_time_text();
    let backup_catalog_id = unique_catalog_id(catalog, &created_at, &payload_id)?;
    let local_payload_path = payload_path(base_dir, &backup_catalog_id);

    let entry = BackupCatalogEntry {
        backup_catalog_id,
        payload_id,
        backup_version_label: label
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(created_at.as_str())
            .to_owned(),
        created_at,
        contents: backup_contents(payload),
        encrypted,
        local_payload_path: local_payload_path.display().to_string(),
        remote: None,
    };
    catalog.entries.push(entry.clone());
    Ok((
        entry,
        PreparedPayload {
            path: local_payload_path,
            bytes: payload_text,
        },
    ))
}

fn remote_backup_metadata(cid: &str, provider: Option<&str>) -> RemoteBackupMetadata {
    RemoteBackupMetadata {
        cid: cid.to_owned(),
        provider: remote_backup_provider(provider),
        uploaded_at: unix_time_text(),
    }
}

fn remote_backup_provider(provider: Option<&str>) -> String {
    provider
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("logos_storage")
        .to_owned()
}

fn stage_replacement(
    target: &Path,
    bytes: &[u8],
    prefix: &str,
    label: &str,
) -> Result<tempfile::NamedTempFile> {
    let parent = target
        .parent()
        .with_context(|| format!("{label} path has no parent directory"))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create {label} directory {}", parent.display()))?;
    let mut staged = tempfile::Builder::new()
        .prefix(prefix)
        .suffix(".tmp")
        .tempfile_in(parent)
        .with_context(|| format!("failed to stage {label} in {}", parent.display()))?;
    staged
        .write_all(bytes)
        .with_context(|| format!("failed to write staged {label}"))?;
    staged
        .as_file()
        .sync_all()
        .with_context(|| format!("failed to sync staged {label}"))?;
    Ok(staged)
}

fn persist_replacement(staged: tempfile::NamedTempFile, target: &Path, label: &str) -> Result<()> {
    staged
        .persist(target)
        .map_err(|error| error.error)
        .with_context(|| {
            format!(
                "failed to atomically replace {label} at {}",
                target.display()
            )
        })?;
    Ok(())
}

fn cleanup_staged_files(directory: &Path, prefix: &str) -> Result<()> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect backup transaction directory {}",
                    directory.display()
                )
            });
        }
    };
    let mut removed = false;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to inspect backup transaction entry in {}",
                directory.display()
            )
        })?;
        if !entry
            .file_type()
            .context("failed to inspect backup transaction entry type")?
            .is_file()
            || !entry.file_name().to_string_lossy().starts_with(prefix)
        {
            continue;
        }
        fs::remove_file(entry.path()).with_context(|| {
            format!(
                "failed to remove stale backup transaction file {}",
                entry.path().display()
            )
        })?;
        removed = true;
    }
    if removed {
        sync_directory(directory, "backup transaction")?;
    }
    Ok(())
}

fn cleanup_orphan_payloads(base_dir: &Path, catalog: &BackupCatalog) -> Result<()> {
    let payload_dir = base_dir.join(BACKUP_PAYLOAD_DIR);
    let entries = match fs::read_dir(&payload_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to inspect backup payload directory {}",
                    payload_dir.display()
                )
            });
        }
    };
    let mut removed = false;
    for entry in entries {
        let entry = entry.with_context(|| {
            format!(
                "failed to inspect backup payload entry in {}",
                payload_dir.display()
            )
        })?;
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        let is_payload_file = entry
            .file_type()
            .context("failed to inspect backup payload entry type")?
            .is_file()
            && file_name.starts_with("backup_")
            && file_name.ends_with(".json");
        let is_referenced = catalog.entries.iter().any(|catalog_entry| {
            payload_path(base_dir, &catalog_entry.backup_catalog_id) == entry.path()
        });
        if !is_payload_file || is_referenced {
            continue;
        }
        fs::remove_file(entry.path()).with_context(|| {
            format!(
                "failed to remove orphaned backup payload {}",
                entry.path().display()
            )
        })?;
        removed = true;
    }
    if removed {
        sync_directory(&payload_dir, "backup payload")?;
        sync_directory(base_dir, "backup catalog")?;
    }
    Ok(())
}

fn remove_uncommitted_payload(base_dir: &Path, target: &Path) -> Result<()> {
    match fs::remove_file(target) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "failed to remove uncommitted backup payload {}",
                    target.display()
                )
            });
        }
    }
    let parent = target
        .parent()
        .context("backup payload path has no parent directory")?;
    sync_directory(parent, "backup payload")?;
    sync_directory(base_dir, "backup catalog")
}

fn path_exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("failed to inspect backup path {}", path.display()))
        }
    }
}

#[cfg(unix)]
fn sync_directory(directory: &Path, label: &str) -> Result<()> {
    File::open(directory)
        .with_context(|| format!("failed to open {label} directory {}", directory.display()))?
        .sync_all()
        .with_context(|| format!("failed to sync {label} directory {}", directory.display()))
}

#[cfg(not(unix))]
fn sync_directory(_directory: &Path, _label: &str) -> Result<()> {
    Ok(())
}

fn read_backup_payload_file(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path)
        .with_context(|| format!("failed to open backup payload from {}", path.display()))?;
    let length = file
        .metadata()
        .with_context(|| format!("failed to inspect backup payload from {}", path.display()))?
        .len();
    if length > SETTINGS_BACKUP_MAX_BYTES as u64 {
        ensure_settings_backup_size(SETTINGS_BACKUP_MAX_BYTES.saturating_add(1))?;
    }
    let capacity =
        usize::try_from(length).context("backup payload length does not fit in memory")?;
    let mut bytes = Vec::with_capacity(capacity);
    file.take(SETTINGS_BACKUP_MAX_BYTES.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .with_context(|| format!("failed to read backup payload from {}", path.display()))?;
    ensure_settings_backup_size(bytes.len())?;
    Ok(bytes)
}

fn payload_identity(payload_text: &[u8]) -> String {
    let digest = Sha256::digest(payload_text);
    format!("sha256:{}", hex::encode(digest))
}

fn catalog_id(created_at: &str, payload_id: &str) -> String {
    let short_hash = payload_id
        .strip_prefix("sha256:")
        .map(|value| value.chars().take(12).collect::<String>())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "unknown".to_owned());
    format!("backup_{created_at}_{short_hash}")
}

fn unique_catalog_id(
    catalog: &BackupCatalog,
    created_at: &str,
    payload_id: &str,
) -> Result<String> {
    let base = catalog_id(created_at, payload_id);
    if !catalog
        .entries
        .iter()
        .any(|entry| entry.backup_catalog_id == base)
    {
        return Ok(base);
    }
    (2..=catalog.entries.len().saturating_add(2))
        .map(|suffix| format!("{base}_{suffix}"))
        .find(|candidate| {
            !catalog
                .entries
                .iter()
                .any(|entry| entry.backup_catalog_id == *candidate)
        })
        .context("backup catalog id space is exhausted")
}

fn backup_contents(payload: &Value) -> Vec<String> {
    let state = payload.get("state").and_then(Value::as_object);
    let encrypted = payload
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if encrypted {
        return vec!["encrypted_payload".to_owned()];
    }
    let mut contents = Vec::new();
    if let Some(settings) = state.and_then(|state| state.get("settings")) {
        let has_settings = settings
            .as_object()
            .is_none_or(|object| object.keys().any(|key| key != "favorites"));
        if has_settings {
            contents.push("settings".to_owned());
        }
        if settings.get("favorites").is_some() {
            contents.push("favorites".to_owned());
        }
    }
    if state.and_then(|state| state.get("idls")).is_some() {
        contents.push("idl_registry".to_owned());
    }
    if state.and_then(|state| state.get("wallet")).is_some() {
        contents.push("wallet_profile".to_owned());
    }
    contents
}

fn backup_catalog_path() -> Result<PathBuf> {
    Ok(catalog_path_for_dir(&config_dir()?))
}

fn catalog_path_for_dir(base_dir: &Path) -> PathBuf {
    base_dir.join("backup_catalog.json")
}

fn payload_path(base_dir: &Path, backup_catalog_id: &str) -> PathBuf {
    base_dir
        .join(BACKUP_PAYLOAD_DIR)
        .join(format!("{backup_catalog_id}.json"))
}

fn unix_time_text() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs().to_string())
        .unwrap_or_else(|_| "0".to_owned())
}

#[cfg(test)]
mod tests {
    use std::{
        env,
        process::{Child, Command, Output, Stdio},
        thread,
        time::{Duration, Instant},
    };

    use anyhow::Result;
    use serde_json::json;

    use super::*;

    const CHILD_BASE_DIR_ENV: &str = "BACKUP_CATALOG_TEST_BASE_DIR";
    const CHILD_CID_ENV: &str = "BACKUP_CATALOG_TEST_CID";
    const CHILD_THEME_ENV: &str = "BACKUP_CATALOG_TEST_THEME";
    const CHILD_CATALOG_ID_ENV: &str = "BACKUP_CATALOG_TEST_ID";
    const CHILD_LOCK_ATTEMPT_ENV: &str = "BACKUP_CATALOG_TEST_LOCK_ATTEMPT";
    const CHILD_LOCK_ACQUIRED_ENV: &str = "BACKUP_CATALOG_TEST_LOCK_ACQUIRED";
    const CHILD_LOCK_RELEASE_ENV: &str = "BACKUP_CATALOG_TEST_LOCK_RELEASE";

    struct FailAfterPayloadPersist {
        fired: bool,
    }

    impl BackupCatalogIoHook for FailAfterPayloadPersist {
        fn at(&mut self, point: BackupCatalogIoPoint) -> Result<()> {
            if point == BackupCatalogIoPoint::PayloadPersisted && !self.fired {
                self.fired = true;
                bail!("injected backup catalog failure after payload persistence");
            }
            Ok(())
        }
    }

    struct FailAfterCatalogPersist {
        fired: bool,
    }

    impl BackupCatalogIoHook for FailAfterCatalogPersist {
        fn at(&mut self, point: BackupCatalogIoPoint) -> Result<()> {
            if point == BackupCatalogIoPoint::CatalogPersisted && !self.fired {
                self.fired = true;
                bail!("injected backup catalog durability uncertainty after catalog install");
            }
            Ok(())
        }
    }

    struct SubprocessLockHook {
        attempt_marker: Option<PathBuf>,
        acquired_marker: Option<PathBuf>,
        release_marker: Option<PathBuf>,
    }

    impl SubprocessLockHook {
        fn from_environment() -> Self {
            Self {
                attempt_marker: env::var_os(CHILD_LOCK_ATTEMPT_ENV).map(PathBuf::from),
                acquired_marker: env::var_os(CHILD_LOCK_ACQUIRED_ENV).map(PathBuf::from),
                release_marker: env::var_os(CHILD_LOCK_RELEASE_ENV).map(PathBuf::from),
            }
        }
    }

    impl BackupCatalogIoHook for SubprocessLockHook {
        fn at(&mut self, point: BackupCatalogIoPoint) -> Result<()> {
            if point == BackupCatalogIoPoint::LockAttempt
                && let Some(marker) = self.attempt_marker.as_ref()
            {
                fs::write(marker, b"attempted")?;
            }
            if point != BackupCatalogIoPoint::LockAcquired {
                return Ok(());
            }
            if let Some(marker) = self.acquired_marker.as_ref() {
                fs::write(marker, b"acquired")?;
            }
            let Some(release_marker) = self.release_marker.as_ref() else {
                return Ok(());
            };
            let deadline = Instant::now() + Duration::from_secs(10);
            while !release_marker.is_file() {
                if Instant::now() >= deadline {
                    bail!("timed out waiting to release backup catalog test lock");
                }
                thread::sleep(Duration::from_millis(5));
            }
            Ok(())
        }
    }

    struct TestChild(Option<Child>);

    impl TestChild {
        fn try_wait(&mut self) -> Result<Option<std::process::ExitStatus>> {
            self.0
                .as_mut()
                .context("backup catalog test child was already consumed")?
                .try_wait()
                .context("failed to poll backup catalog test child")
        }

        fn finish(mut self) -> Result<Output> {
            self.0
                .take()
                .context("backup catalog test child was already consumed")?
                .wait_with_output()
                .context("failed to wait for backup catalog test child")
        }
    }

    impl Drop for TestChild {
        fn drop(&mut self) {
            if let Some(child) = self.0.as_mut() {
                drop(child.kill());
                drop(child.wait());
            }
        }
    }

    #[test]
    fn local_backup_entry_splits_catalog_and_payload_identity() -> Result<()> {
        let base = unique_test_dir("catalog-identity")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": {
                "settings": { "favorites": [] },
                "idls": { "idls": [] },
                "wallet": { "profile": {} }
            }
        });

        let entry = record_payload_in_dir(&base, Some("Release candidate"), &payload)?;

        if entry.backup_catalog_id == entry.payload_id {
            bail!("catalog id and payload id must be separate");
        }
        if entry.backup_version_label != "Release candidate" {
            bail!("backup label was not preserved");
        }
        if !Path::new(&entry.local_payload_path).is_file() {
            bail!("backup payload file was not written");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn local_backup_entry_records_plain_contents() -> Result<()> {
        let base = unique_test_dir("catalog-contents")?;
        let payload = json!({
            "encrypted": false,
            "state": {
                "settings": { "theme": "dark", "favorites": [{ "value": "account-1" }] },
                "idls": { "idls": [{ "name": "token" }] },
                "wallet": { "profile": { "label": "Local wallet" } }
            }
        });

        let entry = record_payload_in_dir(&base, None, &payload)?;

        for content in ["settings", "favorites", "idl_registry", "wallet_profile"] {
            if !entry.contents.iter().any(|value| value == content) {
                bail!("backup entry missing content `{content}`: {entry:?}");
            }
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn local_backup_entry_defaults_blank_label_to_created_at_timestamp() -> Result<()> {
        let base = unique_test_dir("catalog-default-label")?;
        let payload = json!({
            "encrypted": false,
            "state": {
                "settings": { "favorites": [] }
            }
        });

        let entry = record_payload_in_dir(&base, Some("   "), &payload)?;

        if entry.backup_version_label.is_empty() {
            bail!("backup label default must not be empty");
        }
        if entry.backup_version_label == "Manual backup" {
            bail!("backup label default must not use the legacy manual label");
        }
        if entry.backup_version_label != entry.created_at {
            bail!(
                "backup label default should match created_at timestamp: {:?}",
                entry
            );
        }
        entry
            .backup_version_label
            .parse::<u64>()
            .context("backup label default is not a unix timestamp")?;
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn remote_upload_metadata_attaches_once_and_retries_idempotently() -> Result<()> {
        let base = unique_test_dir("remote-upload-attach-once")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let local = record_payload_in_dir(&base, Some("Local"), &payload)?;

        let first = attach_remote_backup_metadata_in_dir(
            &base,
            &local.backup_catalog_id,
            " z-cid ",
            Some(" logos_storage "),
        )?;
        let catalog_path = catalog_path_for_dir(&base);
        let catalog_before = fs::read(&catalog_path)?;
        let second = attach_remote_backup_metadata_in_dir(
            &base,
            &local.backup_catalog_id,
            "z-cid",
            Some("logos_storage"),
        )?;

        anyhow::ensure!(
            first == second
                && first.remote.as_ref().map(|remote| remote.cid.as_str()) == Some("z-cid")
                && fs::read(&catalog_path)? == catalog_before,
            "idempotent remote metadata retry changed accepted identity"
        );
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn remote_upload_metadata_rejects_replacement_without_mutation() -> Result<()> {
        let base = unique_test_dir("remote-upload-replacement")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let local = record_payload_in_dir(&base, None, &payload)?;
        attach_remote_backup_metadata_in_dir(
            &base,
            &local.backup_catalog_id,
            "z-first",
            Some("logos_storage"),
        )?;
        let catalog_path = catalog_path_for_dir(&base);
        let catalog_before = fs::read(&catalog_path)?;

        for (cid, provider) in [("z-first", "other_storage"), ("z-second", "logos_storage")] {
            let error = attach_remote_backup_metadata_in_dir(
                &base,
                &local.backup_catalog_id,
                cid,
                Some(provider),
            )
            .err()
            .context("different remote identity should be rejected")?;

            anyhow::ensure!(
                error
                    .to_string()
                    .contains("already has remote metadata for CID `z-first`")
                    && fs::read(&catalog_path)? == catalog_before,
                "remote metadata replacement changed durable catalog state: {error:#}"
            );
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn remote_upload_cid_cannot_bind_to_different_payload_identity() -> Result<()> {
        let base = unique_test_dir("remote-upload-cid-payload-conflict")?;
        let first_payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "light", "favorites": [] } }
        });
        let second_payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "dark", "favorites": [] } }
        });
        let first = record_payload_in_dir(&base, None, &first_payload)?;
        let second = record_payload_in_dir(&base, None, &second_payload)?;
        attach_remote_backup_metadata_in_dir(&base, &first.backup_catalog_id, "z-shared", None)?;
        let catalog_path = catalog_path_for_dir(&base);
        let catalog_before = fs::read(&catalog_path)?;

        let error = attach_remote_backup_metadata_in_dir(
            &base,
            &second.backup_catalog_id,
            "z-shared",
            None,
        )
        .err()
        .context("one CID should not bind different payload identities")?;

        anyhow::ensure!(
            error.to_string()
                == "remote backup CID `z-shared` conflicts with existing payload identity"
                && fs::read(&catalog_path)? == catalog_before,
            "CID payload conflict changed durable catalog state: {error:#}"
        );
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn remote_download_payload_records_local_catalog_entry() -> Result<()> {
        let base = unique_test_dir("remote-download")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": {
                "settings": { "favorites": [] }
            }
        });

        let entry = record_remote_payload_in_dir(
            &base,
            Some("Remote z-cid"),
            &payload,
            "z-cid",
            Some("logos_storage"),
        )?;
        let catalog = load_catalog_from_path(&catalog_path_for_dir(&base))?;
        let payload_file = payload_path(&base, &entry.backup_catalog_id);

        if catalog.entries.len() != 1
            || !payload_file.is_file()
            || entry.remote.as_ref().map(|remote| remote.cid.as_str()) != Some("z-cid")
        {
            bail!("remote payload was not recorded in local catalog: {catalog:?}");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn identical_remote_download_is_stable_and_idempotent() -> Result<()> {
        let base = unique_test_dir("remote-download-update")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": {
                "settings": { "favorites": [] }
            }
        });

        let first = record_remote_payload_in_dir(
            &base,
            Some("Remote z-cid"),
            &payload,
            "z-cid",
            Some("logos_storage"),
        )?;
        let catalog_path = catalog_path_for_dir(&base);
        let payload_file = payload_path(&base, &first.backup_catalog_id);
        let catalog_before = fs::read(&catalog_path)?;
        let payload_before = fs::read(&payload_file)?;
        let second = record_remote_payload_in_dir(
            &base,
            Some("Replacement label must not mutate an accepted CID"),
            &payload,
            "z-cid",
            Some("logos_storage"),
        )?;
        let catalog = load_catalog_from_path(&catalog_path)?;

        if catalog.entries.len() != 1
            || first != second
            || fs::read(&catalog_path)? != catalog_before
            || fs::read(&payload_file)? != payload_before
        {
            bail!("identical remote download was not stable and idempotent: {catalog:?}");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn identical_remote_download_repairs_missing_or_corrupt_local_payload() -> Result<()> {
        let base = unique_test_dir("remote-download-repair")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": {
                "settings": { "favorites": [] }
            }
        });
        let first = record_remote_payload_in_dir(
            &base,
            Some("Remote repair"),
            &payload,
            "z-repair-cid",
            Some("logos_storage"),
        )?;
        let catalog_path = catalog_path_for_dir(&base);
        let payload_file = payload_path(&base, &first.backup_catalog_id);
        let catalog_before = fs::read(&catalog_path)?;
        let expected_payload = serde_json::to_vec_pretty(&payload)?;

        fs::remove_file(&payload_file)?;
        let repaired_missing = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-repair-cid",
            Some("logos_storage"),
        )?;
        if repaired_missing != first
            || fs::read(&payload_file)? != expected_payload
            || fs::read(&catalog_path)? != catalog_before
        {
            bail!("repeat download did not repair a missing payload idempotently");
        }

        fs::write(&payload_file, b"corrupt")?;
        let repaired_corrupt = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-repair-cid",
            Some("logos_storage"),
        )?;
        if repaired_corrupt != first
            || fs::read(&payload_file)? != expected_payload
            || fs::read(&catalog_path)? != catalog_before
        {
            bail!("repeat download did not repair a corrupt payload idempotently");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn backup_payload_reader_rejects_oversized_file_before_allocation() -> Result<()> {
        let base = unique_test_dir("payload-read-bound")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let entry = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-oversized-local",
            Some("logos_storage"),
        )?;
        let payload_file = payload_path(&base, &entry.backup_catalog_id);
        fs::OpenOptions::new()
            .write(true)
            .open(&payload_file)?
            .set_len(SETTINGS_BACKUP_MAX_BYTES.saturating_add(1) as u64)?;

        let error = backup_payload_bytes_in_dir(&base, &entry.backup_catalog_id)
            .err()
            .context("oversized local backup payload should fail")?;
        if !error.to_string().contains(&format!(
            "settings backup payload exceeded {} byte limit",
            SETTINGS_BACKUP_MAX_BYTES
        )) {
            bail!("unexpected oversized local backup error: {error:#}");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn backup_payload_reader_rejects_valid_json_with_wrong_payload_identity() -> Result<()> {
        let base = unique_test_dir("payload-identity-mismatch")?;
        let original = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "light", "favorites": [] } }
        });
        let changed = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "dark", "favorites": [] } }
        });
        let entry = record_remote_payload_in_dir(
            &base,
            None,
            &original,
            "z-payload-identity",
            Some("logos_storage"),
        )?;
        fs::write(
            payload_path(&base, &entry.backup_catalog_id),
            serde_json::to_vec_pretty(&changed)?,
        )?;

        let error = backup_payload_bytes_in_dir(&base, &entry.backup_catalog_id)
            .err()
            .context("changed local payload identity should fail")?;
        if !error
            .to_string()
            .contains("payload identity does not match the catalog")
        {
            bail!("unexpected payload identity error: {error:#}");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn backup_catalog_id_normalizes_whitespace_and_rejects_path_components() -> Result<()> {
        let base = unique_test_dir("catalog-id-normalization")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let entry = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-catalog-id",
            Some("logos_storage"),
        )?;

        let (catalog_id, resolved) =
            backup_payload_value_in_dir(&base, &format!("  {}  ", entry.backup_catalog_id))?;
        if catalog_id.as_str() != entry.backup_catalog_id || resolved != payload {
            bail!("backup catalog id did not resolve to canonical identity");
        }
        for unsafe_id in ["../backup", "folder/backup", "folder\\backup", ".", ".."] {
            if backup_payload_bytes_in_dir(&base, unsafe_id).is_ok() {
                bail!("unsafe backup catalog id was accepted: {unsafe_id}");
            }
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn catalog_load_rejects_traversal_id_before_existing_cid_repair() -> Result<()> {
        let sandbox = tempfile::tempdir()?;
        let base = sandbox.path().join("catalog").join("base");
        fs::create_dir_all(&base)?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let payload_bytes = serde_json::to_vec_pretty(&payload)?;
        let victim = sandbox.path().join("victim.json");
        fs::write(&victim, b"sentinel")?;
        let catalog = BackupCatalog {
            version: CATALOG_VERSION,
            entries: vec![BackupCatalogEntry {
                backup_catalog_id: "../../../victim".to_owned(),
                payload_id: payload_identity(&payload_bytes),
                backup_version_label: "crafted".to_owned(),
                created_at: "1".to_owned(),
                contents: vec!["settings".to_owned()],
                encrypted: false,
                local_payload_path: victim.display().to_string(),
                remote: Some(RemoteBackupMetadata {
                    cid: "z-crafted-cid".to_owned(),
                    provider: "logos_storage".to_owned(),
                    uploaded_at: "1".to_owned(),
                }),
            }],
        };
        let catalog_path = catalog_path_for_dir(&base);
        let catalog_before = serde_json::to_vec_pretty(&catalog)?;
        fs::write(&catalog_path, &catalog_before)?;

        let error = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-crafted-cid",
            Some("logos_storage"),
        )
        .err()
        .context("crafted catalog id should fail before repair")?;

        if !error.to_string().contains("invalid id")
            || fs::read(&victim)? != b"sentinel"
            || fs::read(&catalog_path)? != catalog_before
        {
            bail!("crafted catalog repair escaped fail-closed validation: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn catalog_load_rejects_noncanonical_and_duplicate_identity_fields() -> Result<()> {
        let base = unique_test_dir("catalog-load-integrity")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let first = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-integrity-first",
            Some("logos_storage"),
        )?;
        let catalog_path = catalog_path_for_dir(&base);
        let valid = load_catalog_from_path(&catalog_path)?;

        let mut invalid_payload = valid.clone();
        invalid_payload
            .entries
            .first_mut()
            .context("valid catalog has no entry")?
            .payload_id = "sha256:not-a-digest".to_owned();
        fs::write(&catalog_path, serde_json::to_vec_pretty(&invalid_payload)?)?;
        if load_catalog_from_path(&catalog_path).is_ok() {
            bail!("invalid persisted payload identity was accepted");
        }

        let mut invalid_path = valid.clone();
        invalid_path
            .entries
            .first_mut()
            .context("valid catalog has no entry")?
            .local_payload_path = base.join("elsewhere.json").display().to_string();
        fs::write(&catalog_path, serde_json::to_vec_pretty(&invalid_path)?)?;
        if load_catalog_from_path(&catalog_path).is_ok() {
            bail!("noncanonical persisted payload path was accepted");
        }

        let mut duplicate = valid.clone();
        let mut second = duplicate
            .entries
            .first()
            .cloned()
            .context("valid catalog has no entry")?;
        second.backup_version_label = "duplicate".to_owned();
        duplicate.entries.push(second);
        fs::write(&catalog_path, serde_json::to_vec_pretty(&duplicate)?)?;
        if load_catalog_from_path(&catalog_path).is_ok() {
            bail!("duplicate persisted catalog and remote identities were accepted");
        }

        let mut duplicate_cid = valid.clone();
        let mut second = duplicate_cid
            .entries
            .first()
            .cloned()
            .context("valid catalog has no entry")?;
        second.backup_catalog_id = "backup_duplicate_remote_cid".to_owned();
        second.local_payload_path = payload_path(&base, &second.backup_catalog_id)
            .display()
            .to_string();
        duplicate_cid.entries.push(second);
        fs::write(&catalog_path, serde_json::to_vec_pretty(&duplicate_cid)?)?;
        if load_catalog_from_path(&catalog_path).is_ok() {
            bail!("duplicate persisted remote CID was accepted");
        }

        fs::write(&catalog_path, serde_json::to_vec_pretty(&valid)?)?;
        let restored = load_catalog_from_path(&catalog_path)?;
        let restored_id = restored
            .entries
            .first()
            .map(|entry| &entry.backup_catalog_id);
        if restored.entries.len() != 1 || restored_id != Some(&first.backup_catalog_id) {
            bail!("valid catalog did not reload after integrity checks");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn remote_download_rejects_changed_payload_for_existing_cid_without_mutation() -> Result<()> {
        let base = unique_test_dir("remote-download-cid-conflict")?;
        let first_payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": {
                "settings": { "theme": "light", "favorites": [] }
            }
        });
        let changed_payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": {
                "settings": { "theme": "dark", "favorites": [] }
            }
        });
        let first = record_remote_payload_in_dir(
            &base,
            Some("Remote immutable CID"),
            &first_payload,
            "z-immutable-cid",
            Some("logos_storage"),
        )?;
        let catalog_path = catalog_path_for_dir(&base);
        let payload_file = payload_path(&base, &first.backup_catalog_id);
        let catalog_before = fs::read(&catalog_path)?;
        let payload_before = fs::read(&payload_file)?;

        let error = record_remote_payload_in_dir(
            &base,
            Some("Conflicting payload"),
            &changed_payload,
            "z-immutable-cid",
            Some("logos_storage"),
        )
        .err()
        .context("changed payload for accepted CID should fail")?;

        if error.to_string()
            != "remote backup CID `z-immutable-cid` conflicts with existing payload identity"
            || fs::read(&catalog_path)? != catalog_before
            || fs::read(&payload_file)? != payload_before
        {
            bail!("CID conflict changed durable catalog state: {error:#}");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn equal_payload_under_second_cid_preserves_both_remote_identities() -> Result<()> {
        let base = unique_test_dir("remote-download-equal-content-cids")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "light", "favorites": [] } }
        });
        let changed = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "dark", "favorites": [] } }
        });
        let first = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-first-cid",
            Some("logos_storage"),
        )?;
        let second = record_remote_payload_in_dir(
            &base,
            None,
            &payload,
            "z-second-cid",
            Some("logos_storage"),
        )?;
        if first.backup_catalog_id == second.backup_catalog_id
            || first.payload_id != second.payload_id
        {
            bail!("equal payload CIDs did not retain distinct catalog identities");
        }
        let catalog_path = catalog_path_for_dir(&base);
        let catalog_before = fs::read(&catalog_path)?;
        let first_payload_before = fs::read(payload_path(&base, &first.backup_catalog_id))?;
        let second_payload_before = fs::read(payload_path(&base, &second.backup_catalog_id))?;
        let catalog = load_catalog_from_path(&catalog_path)?;
        let remote_cids = catalog
            .entries
            .iter()
            .filter_map(|entry| entry.remote.as_ref().map(|remote| remote.cid.as_str()))
            .collect::<Vec<_>>();
        if catalog.entries.len() != 2
            || !remote_cids.contains(&"z-first-cid")
            || !remote_cids.contains(&"z-second-cid")
        {
            bail!("equal payload CIDs collapsed remote identity: {catalog:?}");
        }

        let error = record_remote_payload_in_dir(
            &base,
            None,
            &changed,
            "z-first-cid",
            Some("logos_storage"),
        )
        .err()
        .context("changed payload for first CID should remain rejected")?;
        if error.to_string()
            != "remote backup CID `z-first-cid` conflicts with existing payload identity"
            || fs::read(&catalog_path)? != catalog_before
            || fs::read(payload_path(&base, &first.backup_catalog_id))? != first_payload_before
            || fs::read(payload_path(&base, &second.backup_catalog_id))? != second_payload_before
        {
            bail!("second equal-content CID weakened first-CID immutability: {error:#}");
        }
        fs::remove_dir_all(&base)
            .with_context(|| format!("failed to remove test directory {}", base.display()))?;
        Ok(())
    }

    #[test]
    fn failed_catalog_commit_rolls_back_payload_and_next_writer_cleans_crash_orphans() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let base = directory.path();
        let first_payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "light", "favorites": [] } }
        });
        let second_payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "dark", "favorites": [] } }
        });
        let first = record_remote_payload_in_dir(
            base,
            None,
            &first_payload,
            "z-first-durable",
            Some("logos_storage"),
        )?;
        let catalog_path = catalog_path_for_dir(base);
        let catalog_before = fs::read(&catalog_path)?;
        let first_payload_before = fs::read(payload_path(base, &first.backup_catalog_id))?;
        let mut hook = FailAfterPayloadPersist { fired: false };

        let error = record_remote_payload_in_dir_with_hook(
            base,
            None,
            &second_payload,
            "z-interrupted",
            Some("logos_storage"),
            &mut hook,
        )
        .err()
        .context("injected backup catalog commit failure should fail")?;

        if !error
            .to_string()
            .contains("injected backup catalog failure after payload persistence")
            || fs::read(&catalog_path)? != catalog_before
            || fs::read(payload_path(base, &first.backup_catalog_id))? != first_payload_before
            || load_catalog_from_path(&catalog_path)?.entries.len() != 1
        {
            bail!("failed catalog commit changed durable state: {error:#}");
        }
        let payload_dir = base.join(BACKUP_PAYLOAD_DIR);
        let payload_files = fs::read_dir(&payload_dir)?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<std::io::Result<Vec<_>>>()?;
        if payload_files.len() != 1
            || payload_files
                .iter()
                .any(|name| name.to_string_lossy().starts_with(PAYLOAD_STAGE_PREFIX))
        {
            bail!("failed catalog commit left payload artifacts: {payload_files:?}");
        }

        let orphan = payload_dir.join("backup_crash_orphan.json");
        let payload_stage = payload_dir.join(format!("{PAYLOAD_STAGE_PREFIX}crash.tmp"));
        let catalog_stage = base.join(format!("{CATALOG_STAGE_PREFIX}crash.tmp"));
        fs::write(&orphan, b"orphan")?;
        fs::write(&payload_stage, b"staged")?;
        fs::write(&catalog_stage, b"staged")?;

        let repeated = record_remote_payload_in_dir(
            base,
            None,
            &first_payload,
            "z-first-durable",
            Some("logos_storage"),
        )?;

        if repeated != first
            || orphan.exists()
            || payload_stage.exists()
            || catalog_stage.exists()
            || fs::read(&catalog_path)? != catalog_before
        {
            bail!("next catalog writer did not clean crash artifacts idempotently");
        }
        Ok(())
    }

    #[test]
    fn payload_install_failure_does_not_claim_uninstalled_remote_metadata() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let base = directory.path();
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "dark", "favorites": [] } }
        });
        let local = record_payload_in_dir(base, Some("Local"), &payload)?;
        let mut hook = FailAfterPayloadPersist { fired: false };

        let error = record_remote_payload_in_dir_with_hook(
            base,
            Some("Remote"),
            &payload,
            "z-promotion-retry",
            Some("logos_storage"),
            &mut hook,
        )
        .err()
        .context("uninstalled remote metadata should not be reported as committed")?;
        let after_failure = load_catalog_from_path(&catalog_path_for_dir(base))?;

        anyhow::ensure!(
            error
                .to_string()
                .contains("injected backup catalog failure after payload persistence"),
            "local-to-remote promotion returned the wrong failure: {error:#}"
        );
        anyhow::ensure!(
            after_failure.entries.as_slice() == [local.clone()]
                && after_failure
                    .entries
                    .first()
                    .is_some_and(|entry| entry.remote.is_none())
                && !backup_payload_bytes_in_dir(base, &local.backup_catalog_id)?.is_empty(),
            "failed local-to-remote promotion exposed uninstalled metadata: {after_failure:?}"
        );

        let retry = record_remote_payload_in_dir(
            base,
            Some("Remote"),
            &payload,
            "z-promotion-retry",
            Some("logos_storage"),
        )?;
        let after_retry = load_catalog_from_path(&catalog_path_for_dir(base))?;
        anyhow::ensure!(
            retry.backup_catalog_id == local.backup_catalog_id
                && retry
                    .remote
                    .as_ref()
                    .is_some_and(|remote| remote.cid == "z-promotion-retry")
                && after_retry.entries.as_slice() == [retry],
            "local-to-remote retry did not commit exactly once: {after_retry:?}"
        );
        Ok(())
    }

    #[test]
    fn catalog_install_uncertainty_returns_committed_receipt_without_surprise_entry() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let base = directory.path();
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": "dark", "favorites": [] } }
        });
        let mut hook = FailAfterCatalogPersist { fired: false };

        let committed = record_remote_payload_in_dir_with_hook(
            base,
            None,
            &payload,
            "z-installed-uncertain",
            Some("logos_storage"),
            &mut hook,
        )?;
        let entry = match committed {
            CommittedBackupCatalog::VisibleDurabilityUnconfirmed { value, error } => {
                anyhow::ensure!(
                    error.contains(
                        "injected backup catalog durability uncertainty after catalog install"
                    ),
                    "catalog install lost uncertainty evidence: {error}"
                );
                value
            }
            CommittedBackupCatalog::Durable(_) => {
                bail!("injected post-install uncertainty was reported as durable")
            }
        };

        let immediately_visible = load_catalog_from_path(&catalog_path_for_dir(base))?;
        anyhow::ensure!(
            immediately_visible.entries.as_slice() == [entry.clone()],
            "post-install uncertainty hid a catalog entry until later recovery"
        );
        let retry = spawn_catalog_writer(base, "z-installed-uncertain", "dark", None, None, None)?;
        assert_child_success(
            retry.finish()?,
            "cross-process retry after uncertain durability",
        )?;
        let after_retry = load_catalog_from_path(&catalog_path_for_dir(base))?;
        anyhow::ensure!(
            after_retry.entries.as_slice() == [entry],
            "cross-process retry duplicated or revealed a surprise catalog entry: {after_retry:?}"
        );
        Ok(())
    }

    #[test]
    fn cross_process_writers_reload_catalog_under_exclusive_lock() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let base = directory.path();
        let acquired_marker = base.join("first-lock-acquired");
        let release_marker = base.join("release-first-lock");
        let second_attempt_marker = base.join("second-lock-attempted");
        let first = spawn_catalog_writer(
            base,
            "z-concurrent-first",
            "light",
            None,
            Some(&acquired_marker),
            Some(&release_marker),
        )?;
        wait_for_file(&acquired_marker)?;
        let mut second = spawn_catalog_writer(
            base,
            "z-concurrent-second",
            "dark",
            Some(&second_attempt_marker),
            None,
            None,
        )?;
        wait_for_file(&second_attempt_marker)?;
        if let Some(status) = second.try_wait()? {
            bail!("second catalog writer bypassed exclusive lock: {status}");
        }

        fs::write(&release_marker, b"release")?;
        assert_child_success(first.finish()?, "first catalog writer")?;
        assert_child_success(second.finish()?, "second catalog writer")?;

        let catalog_bytes = fs::read(catalog_path_for_dir(base))?;
        let catalog: BackupCatalog = serde_json::from_slice(&catalog_bytes)
            .context("concurrent backup catalog is not valid JSON")?;
        let remote_cids = catalog
            .entries
            .iter()
            .filter_map(|entry| entry.remote.as_ref().map(|remote| remote.cid.as_str()))
            .collect::<Vec<_>>();
        if catalog.entries.len() != 2
            || !remote_cids.contains(&"z-concurrent-first")
            || !remote_cids.contains(&"z-concurrent-second")
        {
            bail!("concurrent catalog writers lost an entry: {catalog:?}");
        }
        for entry in &catalog.entries {
            backup_payload_bytes_in_dir(base, &entry.backup_catalog_id)?;
        }
        Ok(())
    }

    #[test]
    fn cross_process_remote_metadata_attach_is_first_writer_wins() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let base = directory.path();
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let local = record_payload_in_dir(base, None, &payload)?;
        let acquired_marker = base.join("first-attach-lock-acquired");
        let release_marker = base.join("release-first-attach-lock");
        let second_attempt_marker = base.join("second-attach-lock-attempted");
        let first = spawn_catalog_attacher(
            base,
            &local.backup_catalog_id,
            "z-attach-first",
            None,
            Some(&acquired_marker),
            Some(&release_marker),
        )?;
        wait_for_file(&acquired_marker)?;
        let mut second = spawn_catalog_attacher(
            base,
            &local.backup_catalog_id,
            "z-attach-second",
            Some(&second_attempt_marker),
            None,
            None,
        )?;
        wait_for_file(&second_attempt_marker)?;
        if let Some(status) = second.try_wait()? {
            bail!("second metadata attacher bypassed exclusive lock: {status}");
        }

        fs::write(&release_marker, b"release")?;
        assert_child_success(first.finish()?, "first metadata attacher")?;
        let second_output = second.finish()?;
        anyhow::ensure!(
            !second_output.status.success()
                && String::from_utf8_lossy(&second_output.stderr)
                    .contains("already has remote metadata for CID `z-attach-first`"),
            "second metadata attacher did not reject replacement: status={}, stderr={}",
            second_output.status,
            String::from_utf8_lossy(&second_output.stderr)
        );

        let catalog = load_catalog_from_path(&catalog_path_for_dir(base))?;
        let stored = ensure_catalog_entry(&catalog, &local.backup_catalog_id)?;
        anyhow::ensure!(
            stored.remote.as_ref().map(|remote| remote.cid.as_str()) == Some("z-attach-first"),
            "cross-process metadata race replaced the first identity: {stored:?}"
        );
        Ok(())
    }

    #[test]
    fn backup_catalog_subprocess_writer() -> Result<()> {
        let Some(base_dir) = env::var_os(CHILD_BASE_DIR_ENV) else {
            return Ok(());
        };
        let cid = env::var(CHILD_CID_ENV).context("backup catalog child CID is missing")?;
        let theme = env::var(CHILD_THEME_ENV).context("backup catalog child theme is missing")?;
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "theme": theme, "favorites": [] } }
        });
        let mut hook = SubprocessLockHook::from_environment();
        record_remote_payload_in_dir_with_hook(
            Path::new(&base_dir),
            None,
            &payload,
            &cid,
            Some("logos_storage"),
            &mut hook,
        )?
        .into_value();
        Ok(())
    }

    #[test]
    fn backup_catalog_subprocess_attacher() -> Result<()> {
        let Some(base_dir) = env::var_os(CHILD_BASE_DIR_ENV) else {
            return Ok(());
        };
        let catalog_id =
            env::var(CHILD_CATALOG_ID_ENV).context("backup catalog child ID is missing")?;
        let cid = env::var(CHILD_CID_ENV).context("backup catalog child CID is missing")?;
        let mut hook = SubprocessLockHook::from_environment();
        attach_remote_backup_metadata_in_dir_with_hook(
            Path::new(&base_dir),
            &catalog_id,
            &cid,
            Some("logos_storage"),
            &mut hook,
        )?
        .into_value();
        Ok(())
    }

    fn spawn_catalog_writer(
        base_dir: &Path,
        cid: &str,
        theme: &str,
        attempt_marker: Option<&Path>,
        acquired_marker: Option<&Path>,
        release_marker: Option<&Path>,
    ) -> Result<TestChild> {
        let mut command = Command::new(env::current_exe()?);
        command
            .arg("--exact")
            .arg("support::backup_catalog::tests::backup_catalog_subprocess_writer")
            .arg("--nocapture")
            .env(CHILD_BASE_DIR_ENV, base_dir)
            .env(CHILD_CID_ENV, cid)
            .env(CHILD_THEME_ENV, theme)
            .env_remove(CHILD_LOCK_ATTEMPT_ENV)
            .env_remove(CHILD_LOCK_ACQUIRED_ENV)
            .env_remove(CHILD_LOCK_RELEASE_ENV)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(marker) = attempt_marker {
            command.env(CHILD_LOCK_ATTEMPT_ENV, marker);
        }
        if let Some(marker) = acquired_marker {
            command.env(CHILD_LOCK_ACQUIRED_ENV, marker);
        }
        if let Some(marker) = release_marker {
            command.env(CHILD_LOCK_RELEASE_ENV, marker);
        }
        Ok(TestChild(Some(
            command
                .spawn()
                .context("failed to spawn backup catalog test writer")?,
        )))
    }

    fn spawn_catalog_attacher(
        base_dir: &Path,
        catalog_id: &str,
        cid: &str,
        attempt_marker: Option<&Path>,
        acquired_marker: Option<&Path>,
        release_marker: Option<&Path>,
    ) -> Result<TestChild> {
        let mut command = Command::new(env::current_exe()?);
        command
            .arg("--exact")
            .arg("support::backup_catalog::tests::backup_catalog_subprocess_attacher")
            .arg("--nocapture")
            .env(CHILD_BASE_DIR_ENV, base_dir)
            .env(CHILD_CATALOG_ID_ENV, catalog_id)
            .env(CHILD_CID_ENV, cid)
            .env_remove(CHILD_LOCK_ATTEMPT_ENV)
            .env_remove(CHILD_LOCK_ACQUIRED_ENV)
            .env_remove(CHILD_LOCK_RELEASE_ENV)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(marker) = attempt_marker {
            command.env(CHILD_LOCK_ATTEMPT_ENV, marker);
        }
        if let Some(marker) = acquired_marker {
            command.env(CHILD_LOCK_ACQUIRED_ENV, marker);
        }
        if let Some(marker) = release_marker {
            command.env(CHILD_LOCK_RELEASE_ENV, marker);
        }
        Ok(TestChild(Some(command.spawn().context(
            "failed to spawn backup catalog metadata attacher",
        )?)))
    }

    fn wait_for_file(path: &Path) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(10);
        while !path.is_file() {
            if Instant::now() >= deadline {
                bail!("timed out waiting for test marker {}", path.display());
            }
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    fn assert_child_success(output: Output, label: &str) -> Result<()> {
        if output.status.success() {
            return Ok(());
        }
        bail!(
            "{label} failed with {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
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
