use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use serde::Deserialize;
use serde_json::Value;

use super::{LOCAL_WALLET_HOME_ENV, LocalWalletReadiness};

#[derive(Debug, Clone, Default, Deserialize)]
pub(super) struct LocalWalletProfileInput {
    #[serde(default, alias = "walletBinary")]
    pub(super) wallet_binary: String,
    #[serde(default, alias = "walletHome")]
    pub(super) wallet_home: String,
    #[serde(default, alias = "networkProfile")]
    pub(super) network_profile: String,
}

#[derive(Debug, Clone)]
pub(super) struct ResolvedLocalWalletProfile {
    pub(super) wallet_binary: String,
    pub(super) wallet_home: String,
    pub(super) wallet_home_source: String,
}

#[derive(Debug, Clone, Copy)]
struct WalletHomeRequirements {
    require_config: bool,
    require_storage: bool,
}

#[derive(Debug, Clone)]
struct ResolvedWalletHome {
    path: String,
    source: String,
}

pub(super) fn parse_local_wallet_profile(profile: Value) -> Result<LocalWalletProfileInput> {
    serde_json::from_value(profile).context("failed to parse local wallet profile")
}

pub(super) fn resolve_local_wallet_profile(
    profile: Value,
    action: &str,
    require_config: bool,
) -> Result<ResolvedLocalWalletProfile> {
    let profile = parse_local_wallet_profile(profile)?;
    let wallet_binary = profile.wallet_binary.trim();
    if wallet_binary.is_empty() {
        bail!("wallet binary is required to {action}");
    }
    if local_wallet_binary_is_path_like(wallet_binary) && !Path::new(wallet_binary).is_file() {
        bail!("wallet binary is not reachable");
    }

    let wallet_home = resolve_wallet_home_from_input(
        &profile,
        action,
        WalletHomeRequirements {
            require_config,
            require_storage: false,
        },
    )?;

    Ok(ResolvedLocalWalletProfile {
        wallet_binary: wallet_binary.to_owned(),
        wallet_home: wallet_home.path,
        wallet_home_source: wallet_home.source,
    })
}

pub(super) fn resolve_instruction_wallet_home(profile: Value) -> Result<PathBuf> {
    let profile = parse_local_wallet_profile(profile)?;
    let wallet_home = resolve_wallet_home_from_input(
        &profile,
        "send IDL instruction",
        WalletHomeRequirements {
            require_config: true,
            require_storage: true,
        },
    )?;
    Ok(PathBuf::from(wallet_home.path))
}

fn resolve_wallet_home_from_input(
    profile: &LocalWalletProfileInput,
    action: &str,
    requirements: WalletHomeRequirements,
) -> Result<ResolvedWalletHome> {
    let explicit_home = profile.wallet_home.trim();
    let env_wallet_home = env::var(LOCAL_WALLET_HOME_ENV).unwrap_or_default();
    let (wallet_home, wallet_home_source) = if !explicit_home.is_empty() {
        (explicit_home.to_owned(), "profile".to_owned())
    } else if !env_wallet_home.trim().is_empty() {
        (
            env_wallet_home.trim().to_owned(),
            LOCAL_WALLET_HOME_ENV.to_owned(),
        )
    } else {
        (String::new(), "none".to_owned())
    };
    if wallet_home.is_empty() {
        bail!("wallet home directory is required to {action}");
    }
    let wallet_home_path = Path::new(&wallet_home);
    if !wallet_home_path.is_dir() {
        bail!("wallet home directory is not reachable");
    }
    if requirements.require_config && !wallet_home_is_configured(wallet_home_path) {
        bail!("wallet home missing wallet_config.json");
    }
    if requirements.require_storage && !wallet_home_path.join("storage.json").is_file() {
        bail!("wallet home missing storage.json");
    }
    Ok(ResolvedWalletHome {
        path: wallet_home,
        source: wallet_home_source,
    })
}

pub(super) fn local_wallet_binary_is_path_like(binary: &str) -> bool {
    let binary = binary.trim();
    Path::new(binary).is_absolute()
        || binary.contains(std::path::MAIN_SEPARATOR)
        || binary.contains('/')
        || binary.contains('\\')
}

impl ResolvedLocalWalletProfile {
    pub(super) fn redactions(&self) -> Vec<&str> {
        let mut redactions = vec![self.wallet_home.as_str()];
        if local_wallet_binary_is_path_like(&self.wallet_binary) {
            redactions.push(self.wallet_binary.as_str());
        }
        redactions
    }
}

pub(super) fn detect_wallet_binary() -> Option<PathBuf> {
    if let Some(path) = env_path_if_file("LOGOS_WALLET_BINARY") {
        return Some(path);
    }

    if let Some(path) = find_binary_in_path("wallet") {
        return Some(path);
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    [
        home.join(".cargo").join("bin").join(binary_name("wallet")),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("logos-execution-zone")
            .join("target")
            .join("release")
            .join(binary_name("wallet")),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("logos-execution-zone")
            .join("target")
            .join("debug")
            .join(binary_name("wallet")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

pub(super) fn detect_wallet_home() -> Option<PathBuf> {
    if let Some(path) = env_path_if_wallet_home(LOCAL_WALLET_HOME_ENV) {
        return Some(path);
    }
    None
}

fn env_path_if_file(variable: &str) -> Option<PathBuf> {
    let path = env::var_os(variable).map(PathBuf::from)?;
    path.is_file().then_some(path)
}

fn env_path_if_wallet_home(variable: &str) -> Option<PathBuf> {
    let path = env::var_os(variable).map(PathBuf::from)?;
    wallet_home_is_configured(&path).then_some(path)
}

pub(super) fn wallet_home_is_configured(path: &Path) -> bool {
    path.is_dir() && path.join("wallet_config.json").is_file()
}

pub(super) fn local_wallet_readiness(
    profile: &LocalWalletProfileInput,
    env_wallet_home: &str,
) -> LocalWalletReadiness {
    let binary = profile.wallet_binary.trim();
    let wallet_binary_ready = !binary.is_empty()
        && (!local_wallet_binary_is_path_like(binary) || Path::new(binary).is_file());
    let explicit_home = profile.wallet_home.trim();
    let home = if explicit_home.is_empty() {
        env_wallet_home.trim()
    } else {
        explicit_home
    };
    let home_path = Path::new(home);
    let wallet_home_ready = !home.is_empty() && home_path.is_dir();
    let config_path = home_path.join("wallet_config.json");
    let wallet_config_ready = wallet_home_ready && config_path.is_file();
    let wallet_storage_ready = wallet_home_ready && home_path.join("storage.json").is_file();
    let backup_encryption_ready = wallet_config_ready
        && fs::metadata(config_path)
            .map(|metadata| metadata.len() > 0)
            .unwrap_or(false);

    LocalWalletReadiness {
        wallet_binary_ready,
        wallet_home_ready,
        wallet_config_ready,
        wallet_storage_ready,
        command_ready: wallet_binary_ready && wallet_home_ready,
        accounts_ready: wallet_binary_ready && wallet_config_ready,
        instruction_submit_ready: wallet_config_ready && wallet_storage_ready,
        backup_encryption_ready,
    }
}

fn find_binary_in_path(binary: &str) -> Option<PathBuf> {
    let binary = binary_name(binary);
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|path| path.join(&binary))
        .find(|path| path.is_file())
}

fn binary_name(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use std::{
        env, fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;
    use crate::support::time::now_millis;

    struct TempWalletHome {
        path: PathBuf,
    }

    static NEXT_TEMP_HOME: AtomicU64 = AtomicU64::new(1);

    impl TempWalletHome {
        fn new() -> Result<Self> {
            let path = env::temp_dir().join(format!(
                "logos-inspector-wallet-profile-{}-{}",
                now_millis(),
                NEXT_TEMP_HOME.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&path)?;
            fs::write(path.join("wallet_config.json"), "{}")?;
            fs::write(path.join("storage.json"), "{}")?;
            Ok(Self { path })
        }
    }

    impl Drop for TempWalletHome {
        fn drop(&mut self) {
            if fs::remove_dir_all(&self.path).is_err() {}
        }
    }

    #[test]
    fn instruction_wallet_home_does_not_require_wallet_binary() -> Result<()> {
        let home = TempWalletHome::new()?;
        let resolved = resolve_instruction_wallet_home(json!({
            "wallet_home": home.path.display().to_string()
        }))?;

        if resolved != home.path {
            bail!("unexpected wallet home: {}", resolved.display());
        }
        Ok(())
    }

    #[test]
    fn readiness_distinguishes_command_accounts_instruction_and_backup_requirements() -> Result<()>
    {
        let home = TempWalletHome::new()?;
        let binary = home.path.join("wallet");
        fs::write(&binary, "binary")?;
        let profile = parse_local_wallet_profile(json!({
            "wallet_binary": binary.display().to_string(),
            "wallet_home": home.path.display().to_string()
        }))?;

        let readiness = local_wallet_readiness(&profile, "");

        if !readiness.command_ready
            || !readiness.accounts_ready
            || !readiness.instruction_submit_ready
            || !readiness.backup_encryption_ready
        {
            bail!("fully configured wallet should be ready: {readiness:?}");
        }
        Ok(())
    }
}
