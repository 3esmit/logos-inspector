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
use serde_json::{Value, json};
use sha2::Sha256;

use crate::{
    state_store::{
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
) -> Result<Value> {
    let state = backup_payload_from_states(
        &load_settings_state().context("failed to load settings state for backup")?,
        &load_idl_state().context("failed to load IDL state for backup")?,
        &load_wallet_state().context("failed to load wallet state for backup")?,
        encrypted,
        wallet_profile,
    )?;
    Ok(state)
}

pub(crate) fn restore_app_settings_backup(
    payload: &Value,
    wallet_profile: Option<&Value>,
) -> Result<RestoreSummary> {
    let state = restored_state_from_payload(payload, wallet_profile)?;
    if let Some(settings) = state.settings.as_ref() {
        save_settings_state(settings).context("failed to restore settings state")?;
    }
    if let Some(idl) = state.idl.as_ref() {
        save_idl_state(idl).context("failed to restore IDL state")?;
    }
    if let Some(wallet) = state.wallet.as_ref() {
        save_wallet_state(wallet).context("failed to restore wallet state")?;
    }
    Ok(state.summary)
}

fn backup_payload_from_states(
    settings: &Value,
    idl: &Value,
    wallet: &Value,
    encrypted: bool,
    wallet_profile: Option<&Value>,
) -> Result<Value> {
    let state = json!({
        "settings": settings,
        "idls": idl,
        "wallet": wallet,
    });
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

struct RestoredState {
    settings: Option<Value>,
    idl: Option<Value>,
    wallet: Option<Value>,
    summary: RestoreSummary,
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
