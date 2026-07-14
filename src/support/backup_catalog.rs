use std::{
    fs,
    io::Read as _,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::support::state_store::config_dir;

use super::settings_backup::{
    SETTINGS_BACKUP_MAX_BYTES, ensure_settings_backup_size, export_app_settings_backup,
    preview_app_settings_backup_import, restore_app_settings_backup_with_options,
    validate_app_settings_backup_envelope,
};

const CATALOG_VERSION: u64 = 1;
const BACKUP_PAYLOAD_DIR: &str = "backup-payloads";

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
    let catalog_id = backup_catalog_id.trim();
    if catalog_id.is_empty() {
        bail!("backup catalog id is required");
    }
    let catalog = load_catalog_from_path(&catalog_path_for_dir(base_dir))?;
    ensure_catalog_entry(&catalog, catalog_id)?;
    let path = payload_path(base_dir, catalog_id);
    read_backup_payload_file(&path)
}

pub(crate) fn preview_local_settings_restore(
    backup_catalog_id: &str,
    wallet_profile: Option<&Value>,
    options: Option<&Value>,
) -> Result<Value> {
    let payload = backup_payload_value(backup_catalog_id)?;
    preview_app_settings_backup_import(&payload, wallet_profile, options)
}

pub(crate) fn restore_local_settings_backup(
    backup_catalog_id: &str,
    wallet_profile: Option<&Value>,
    options: Option<&Value>,
) -> Result<Value> {
    let payload = backup_payload_value(backup_catalog_id)?;
    let mut summary = restore_app_settings_backup_with_options(&payload, wallet_profile, options)?;
    set_summary_catalog_id(&mut summary, backup_catalog_id)?;
    Ok(summary)
}

fn record_payload_in_dir(
    base_dir: &Path,
    label: Option<&str>,
    payload: &Value,
) -> Result<BackupCatalogEntry> {
    let mut catalog = load_catalog_from_path(&catalog_path_for_dir(base_dir))?;
    let encrypted = payload
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let entry = create_entry_from_payload(base_dir, &mut catalog, label, payload, encrypted)?;
    save_catalog_to_path(&catalog_path_for_dir(base_dir), &catalog)?;
    Ok(entry)
}

fn record_remote_payload_in_dir(
    base_dir: &Path,
    label: Option<&str>,
    payload: &Value,
    cid: &str,
    provider: Option<&str>,
) -> Result<BackupCatalogEntry> {
    let cid = cid.trim();
    if cid.is_empty() {
        bail!("remote backup CID is required");
    }
    let mut catalog = load_catalog_from_path(&catalog_path_for_dir(base_dir))?;
    let payload_text =
        serde_json::to_vec_pretty(payload).context("failed to serialize backup payload")?;
    ensure_settings_backup_size(payload_text.len())?;
    let payload_id = payload_identity(&payload_text);
    if let Some(existing) = catalog
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
        if !stored_payload_is_current {
            write_payload_file(&expected_payload_path, &payload_text)?;
        }
        if Path::new(&existing.local_payload_path) != expected_payload_path {
            let stored = catalog
                .entries
                .iter_mut()
                .find(|entry| entry.backup_catalog_id == existing.backup_catalog_id)
                .context("accepted remote backup entry disappeared during repair")?;
            stored.local_payload_path = expected_payload_path.display().to_string();
            let repaired = stored.clone();
            save_catalog_to_path(&catalog_path_for_dir(base_dir), &catalog)?;
            return Ok(repaired);
        }
        return Ok(existing);
    }
    let encrypted = payload
        .get("encrypted")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let remote = remote_backup_metadata(cid, provider);

    let entry = if let Some(entry) = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.payload_id == payload_id && entry.remote.is_none())
    {
        let local_payload_path = payload_path(base_dir, &entry.backup_catalog_id);
        write_payload_file(&local_payload_path, &payload_text)?;
        entry.payload_id = payload_id;
        entry.contents = backup_contents(payload);
        entry.encrypted = encrypted;
        entry.local_payload_path = local_payload_path.display().to_string();
        if let Some(label) = label.map(str::trim).filter(|value| !value.is_empty()) {
            entry.backup_version_label = label.to_owned();
        }
        entry.remote = Some(remote);
        entry.clone()
    } else {
        let mut entry =
            create_entry_from_payload(base_dir, &mut catalog, label, payload, encrypted)?;
        entry.remote = Some(remote);
        if let Some(stored) = catalog
            .entries
            .iter_mut()
            .find(|stored| stored.backup_catalog_id == entry.backup_catalog_id)
        {
            stored.remote.clone_from(&entry.remote);
        }
        entry
    };

    save_catalog_to_path(&catalog_path_for_dir(base_dir), &catalog)?;
    Ok(entry)
}

pub(crate) fn attach_remote_backup_metadata(
    backup_catalog_id: &str,
    cid: &str,
    provider: Option<&str>,
) -> Result<BackupCatalogEntry> {
    let catalog_id = backup_catalog_id.trim();
    if catalog_id.is_empty() {
        bail!("backup catalog id is required");
    }
    let cid = cid.trim();
    if cid.is_empty() {
        bail!("remote backup CID is required");
    }

    let path = backup_catalog_path()?;
    let mut catalog = load_catalog_from_path(&path)?;
    let Some(entry) = catalog
        .entries
        .iter_mut()
        .find(|entry| entry.backup_catalog_id == catalog_id)
    else {
        bail!("backup catalog entry `{catalog_id}` was not found");
    };
    entry.remote = Some(remote_backup_metadata(cid, provider));
    let result = entry.clone();
    save_catalog_to_path(&path, &catalog)?;
    Ok(result)
}

fn backup_payload_value(backup_catalog_id: &str) -> Result<Value> {
    let catalog_id = backup_catalog_id.trim();
    if catalog_id.is_empty() {
        bail!("backup catalog id is required");
    }
    let bytes = backup_payload_bytes(catalog_id)?;
    serde_json::from_slice(&bytes).with_context(|| {
        format!("backup catalog entry `{catalog_id}` payload does not contain JSON")
    })
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
    Ok(catalog)
}

fn save_catalog_to_path(path: &Path, catalog: &BackupCatalog) -> Result<()> {
    let parent = path
        .parent()
        .context("backup catalog path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create backup catalog directory {}",
            parent.display()
        )
    })?;
    let text =
        serde_json::to_string_pretty(catalog).context("failed to serialize backup catalog")?;
    fs::write(path, text)
        .with_context(|| format!("failed to write backup catalog to {}", path.display()))
}

fn create_entry_from_payload(
    base_dir: &Path,
    catalog: &mut BackupCatalog,
    label: Option<&str>,
    payload: &Value,
    encrypted: bool,
) -> Result<BackupCatalogEntry> {
    let payload_text =
        serde_json::to_vec_pretty(payload).context("failed to serialize backup payload")?;
    ensure_settings_backup_size(payload_text.len())?;
    let payload_id = payload_identity(&payload_text);
    let created_at = unix_time_text();
    let backup_catalog_id = unique_catalog_id(catalog, &created_at, &payload_id)?;
    let local_payload_path = payload_path(base_dir, &backup_catalog_id);
    let parent = local_payload_path
        .parent()
        .context("backup payload path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create backup payload directory {}",
            parent.display()
        )
    })?;
    write_payload_file(&local_payload_path, &payload_text)?;

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
    Ok(entry)
}

fn remote_backup_metadata(cid: &str, provider: Option<&str>) -> RemoteBackupMetadata {
    RemoteBackupMetadata {
        cid: cid.to_owned(),
        provider: provider
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("logos_storage")
            .to_owned(),
        uploaded_at: unix_time_text(),
    }
}

fn write_payload_file(path: &Path, payload_text: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("backup payload path has no parent directory")?;
    fs::create_dir_all(parent).with_context(|| {
        format!(
            "failed to create backup payload directory {}",
            parent.display()
        )
    })?;
    fs::write(path, payload_text)
        .with_context(|| format!("failed to write backup payload to {}", path.display()))
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
    use std::env;

    use anyhow::Result;
    use serde_json::json;

    use super::*;

    #[test]
    fn local_backup_entry_splits_catalog_and_payload_identity() -> Result<()> {
        let base = unique_test_dir("catalog-identity")?;
        let mut catalog = BackupCatalog::default();
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

        let entry = create_entry_from_payload(
            &base,
            &mut catalog,
            Some("Release candidate"),
            &payload,
            false,
        )?;

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
        let mut catalog = BackupCatalog::default();
        let payload = json!({
            "encrypted": false,
            "state": {
                "settings": { "theme": "dark", "favorites": [{ "value": "account-1" }] },
                "idls": { "idls": [{ "name": "token" }] },
                "wallet": { "profile": { "label": "Local wallet" } }
            }
        });

        let entry = create_entry_from_payload(&base, &mut catalog, None, &payload, false)?;

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
        let mut catalog = BackupCatalog::default();
        let payload = json!({
            "encrypted": false,
            "state": {
                "settings": { "favorites": [] }
            }
        });

        let entry = create_entry_from_payload(&base, &mut catalog, Some("   "), &payload, false)?;

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
    fn remote_upload_metadata_attaches_to_existing_identity() -> Result<()> {
        let mut catalog = BackupCatalog {
            version: CATALOG_VERSION,
            entries: vec![BackupCatalogEntry {
                backup_catalog_id: "backup-1".to_owned(),
                payload_id: "sha256:abc".to_owned(),
                backup_version_label: "Manual".to_owned(),
                created_at: "1".to_owned(),
                contents: vec!["settings".to_owned()],
                encrypted: false,
                local_payload_path: "/tmp/backup-1.json".to_owned(),
                remote: None,
            }],
        };

        let entry = catalog
            .entries
            .iter_mut()
            .find(|entry| entry.backup_catalog_id == "backup-1")
            .context("test entry missing")?;
        entry.remote = Some(RemoteBackupMetadata {
            cid: "z-cid".to_owned(),
            provider: "logos_storage".to_owned(),
            uploaded_at: "2".to_owned(),
        });

        if catalog.entries.len() != 1 {
            bail!("remote metadata must not create a second catalog entry");
        }
        let entry = catalog.entries.first().context("catalog entry missing")?;
        if entry.payload_id != "sha256:abc"
            || entry.remote.as_ref().map(|remote| remote.cid.as_str()) != Some("z-cid")
        {
            bail!("remote metadata did not attach to existing entry: {catalog:?}");
        }
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
