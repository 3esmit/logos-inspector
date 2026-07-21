use std::{
    fs::{self, File},
    io::Write as _,
    path::Path,
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use super::paths::path_is_inside;

const NODE_KEY_FIELD: &str = "nodekey";
const NODE_KEY_BYTES: usize = 32;
const NODE_KEY_GENERATION_ATTEMPTS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IdentityPreparation {
    Existing,
    Generated,
}

pub(super) fn prepare_existing_config(
    workspace: &Path,
    config_path: &Path,
) -> Result<IdentityPreparation> {
    validate_config_path(workspace, config_path)?;
    let mut config = read_config(config_path)?;
    let preparation = match validated_nodekey(&config)? {
        Some(_) => IdentityPreparation::Existing,
        None => {
            insert_nodekey(&mut config, generate_nodekey()?)?;
            IdentityPreparation::Generated
        }
    };
    write_private_config(config_path, &config)?;
    Ok(preparation)
}

pub(super) fn write_generated_config(
    workspace: &Path,
    config_path: &Path,
    mut generated: Value,
) -> Result<()> {
    validate_config_path(workspace, config_path)?;
    let nodekey = match read_optional_config(config_path)? {
        Some(existing) => validated_nodekey(&existing)?.map(ToOwned::to_owned),
        None => None,
    }
    .map_or_else(generate_nodekey, Ok)?;
    insert_nodekey(&mut generated, nodekey)?;
    write_private_config(config_path, &generated)
}

pub(super) fn harden_existing_config_if_keyed(workspace: &Path, config_path: &Path) -> Result<()> {
    validate_config_path(workspace, config_path)?;
    let Some(config) = read_optional_config(config_path)? else {
        return Ok(());
    };
    if validated_nodekey(&config)?.is_some() {
        set_owner_only(config_path)?;
    }
    Ok(())
}

pub(super) fn identity_is_missing(workspace: &Path, config_path: &Path) -> Result<bool> {
    validate_config_path(workspace, config_path)?;
    let config = read_config(config_path)?;
    Ok(validated_nodekey(&config)?.is_none())
}

pub(super) fn redact_config_for_editor(config: &Value) -> Result<Value> {
    let mut redacted = config.clone();
    redacted
        .as_object_mut()
        .context("managed Messaging config must be a JSON object")?
        .remove(NODE_KEY_FIELD);
    Ok(redacted)
}

pub(super) fn has_persisted_identity(config: &Value) -> Result<bool> {
    Ok(validated_nodekey(config)?.is_some())
}

pub(super) fn write_editor_config(
    workspace: &Path,
    config_path: &Path,
    editor_config: Value,
    existing_config: &Value,
) -> Result<()> {
    validate_config_path(workspace, config_path)?;
    if editor_config
        .as_object()
        .context("managed Messaging config must be a JSON object")?
        .contains_key(NODE_KEY_FIELD)
    {
        bail!("Messaging peer identity is protected and cannot be edited here");
    }
    let nodekey = validated_nodekey(existing_config)?
        .context("managed Messaging config has no persisted peer identity")?
        .to_owned();
    let mut replacement = editor_config;
    insert_nodekey(&mut replacement, nodekey)?;
    write_private_config(config_path, &replacement)
}

fn read_optional_config(path: &Path) -> Result<Option<Value>> {
    match fs::read_to_string(path) {
        Ok(text) => serde_json::from_str(&text)
            .context("managed Messaging config is not valid JSON")
            .map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error).context("failed to read managed Messaging config"),
    }
}

fn read_config(path: &Path) -> Result<Value> {
    read_optional_config(path)?.context("managed Messaging config is missing")
}

fn validated_nodekey(config: &Value) -> Result<Option<&str>> {
    let object = config
        .as_object()
        .context("managed Messaging config must be a JSON object")?;
    let Some(value) = object.get(NODE_KEY_FIELD) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let key = value
        .as_str()
        .context("managed Messaging nodekey must be a 64-character hex string")?;
    if key.len() != NODE_KEY_BYTES * 2 || !key.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        bail!("managed Messaging nodekey must be a 64-character hex string");
    }
    let bytes = hex::decode(key).context("managed Messaging nodekey is not valid hex")?;
    k256::SecretKey::from_slice(&bytes)
        .context("managed Messaging nodekey is not a valid secp256k1 secret")?;
    Ok(Some(key))
}

fn generate_nodekey() -> Result<String> {
    for _attempt in 0..NODE_KEY_GENERATION_ATTEMPTS {
        let mut bytes = [0_u8; NODE_KEY_BYTES];
        getrandom::fill(&mut bytes).context("failed to generate Messaging peer identity")?;
        if k256::SecretKey::from_slice(&bytes).is_ok() {
            return Ok(hex::encode(bytes));
        }
    }
    bail!("failed to generate a valid Messaging peer identity")
}

fn insert_nodekey(config: &mut Value, nodekey: String) -> Result<()> {
    config
        .as_object_mut()
        .context("managed Messaging config must be a JSON object")?
        .insert(NODE_KEY_FIELD.to_owned(), Value::String(nodekey));
    Ok(())
}

fn validate_config_path(workspace: &Path, config_path: &Path) -> Result<()> {
    if !path_is_inside(workspace, config_path) {
        bail!("managed Messaging config is outside its topology workspace");
    }
    let parent = config_path
        .parent()
        .context("managed Messaging config path has no parent directory")?;
    let canonical_workspace = fs::canonicalize(workspace)
        .context("failed to resolve managed Messaging topology workspace")?;
    let canonical_parent =
        fs::canonicalize(parent).context("failed to resolve managed Messaging config directory")?;
    if canonical_parent != canonical_workspace
        && !canonical_parent.starts_with(&canonical_workspace)
    {
        bail!("managed Messaging config directory escapes its topology workspace");
    }
    match fs::symlink_metadata(config_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            bail!("managed Messaging config must not be a symbolic link")
        }
        Ok(metadata) if !metadata.is_file() => {
            bail!("managed Messaging config must be a regular file")
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error).context("failed to inspect managed Messaging config"),
    }
}

fn write_private_config(path: &Path, config: &Value) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(config)
        .context("failed to serialize managed Messaging config")?;
    let parent = path
        .parent()
        .context("managed Messaging config path has no parent directory")?;
    let mut staged = tempfile::Builder::new()
        .prefix(".messaging-config-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .context("failed to stage managed Messaging config")?;
    set_file_owner_only(staged.as_file())?;
    staged
        .write_all(&bytes)
        .context("failed to write staged managed Messaging config")?;
    staged
        .as_file_mut()
        .flush()
        .context("failed to flush staged managed Messaging config")?;
    staged
        .as_file()
        .sync_all()
        .context("failed to sync staged managed Messaging config")?;
    staged
        .persist(path)
        .map_err(|error| error.error)
        .context("failed to atomically replace managed Messaging config")?;
    set_owner_only(path)?;
    sync_directory(parent)
}

#[cfg(unix)]
fn set_file_owner_only(file: &File) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    file.set_permissions(fs::Permissions::from_mode(0o600))
        .context("failed to protect staged managed Messaging config")
}

#[cfg(not(unix))]
fn set_file_owner_only(_file: &File) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn set_owner_only(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600))
        .context("failed to protect managed Messaging config")
}

#[cfg(not(unix))]
fn set_owner_only(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .context("failed to open managed Messaging config directory")?
        .sync_all()
        .context("failed to sync managed Messaging config directory")
}

#[cfg(not(unix))]
fn sync_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_nodekey_is_rejected_without_rewriting_config() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let workspace = directory.path().join("network");
        let config_dir = workspace.join("configs");
        fs::create_dir_all(&config_dir)?;
        let config_path = config_dir.join("messaging.json");
        let original = br#"{"nodekey":"invalid","rest":true}"#;
        fs::write(&config_path, original)?;

        let error = prepare_existing_config(&workspace, &config_path)
            .err()
            .context("malformed Messaging nodekey was accepted")?;

        anyhow::ensure!(
            error.to_string().contains("64-character hex string")
                && fs::read(&config_path)? == original,
            "malformed Messaging config was not rejected without mutation"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn symbolic_link_config_is_rejected_without_touching_target() -> Result<()> {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir()?;
        let workspace = directory.path().join("network");
        let config_dir = workspace.join("configs");
        fs::create_dir_all(&config_dir)?;
        let target = directory.path().join("external.json");
        let original = br#"{"rest":true}"#;
        fs::write(&target, original)?;
        let config_path = config_dir.join("messaging.json");
        symlink(&target, &config_path)?;

        let error = prepare_existing_config(&workspace, &config_path)
            .err()
            .context("symbolic-link Messaging config was accepted")?;

        anyhow::ensure!(
            error.to_string().contains("must not be a symbolic link")
                && fs::read(&target)? == original,
            "symbolic-link rejection changed its target"
        );
        Ok(())
    }
}
