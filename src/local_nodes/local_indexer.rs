use std::{
    env, fs,
    net::{SocketAddr, TcpStream},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::{DEFAULT_INDEXER_ENDPOINT, support::state_store::load_settings_state};

const LEZ_DIR_ENV: &str = "LOGOS_EXECUTION_ZONE_DIR";
const DISABLE_AUTO_BUILD_ENV: &str = "LOGOS_INSPECTOR_DISABLE_INDEXER_AUTO_BUILD";
const INDEXER_SERVICE: &str = "logos-lez-indexer.service";
const INDEXER_PACKAGE: &str = "indexer_service";
const INDEXER_BINARY_RELATIVE_PATH: &str = "target/release/indexer_service";
const INDEXER_DATA_DIR_RELATIVE_PATH: &str = ".logos-inspector-indexer";
const INDEXER_CARGO_TOML_RELATIVE_PATHS: &[&str] = &[
    "lez/indexer/service/Cargo.toml",
    "indexer/service/Cargo.toml",
];
const INDEXER_CONFIG_RELATIVE_PATHS: &[&str] = &[
    "lez/indexer/service/configs/debug/indexer_config.json",
    "indexer/service/configs/debug/indexer_config.json",
];
const LOCAL_INDEXER_ADDR: &str = "127.0.0.1:8779";

pub fn bootstrap_default_local_indexer() -> Result<()> {
    if env::var_os(DISABLE_AUTO_BUILD_ENV).is_some() || local_indexer_is_reachable() {
        return Ok(());
    }

    let Some(workspace) = find_lez_workspace() else {
        return Ok(());
    };

    let binary = workspace.join(INDEXER_BINARY_RELATIVE_PATH);
    if !binary.is_file() {
        build_indexer_service(&workspace)?;
    }

    if restart_user_service_if_available(INDEXER_SERVICE)? {
        wait_for_local_indexer(Duration::from_secs(20))?;
    } else {
        spawn_indexer_service(&workspace, &binary)?;
        wait_for_local_indexer(Duration::from_secs(20))?;
    }

    Ok(())
}

pub fn bootstrap_default_local_indexer_for_saved_settings() -> Result<()> {
    if default_local_indexer_requested_by_saved_settings()? {
        bootstrap_default_local_indexer()?;
    }
    Ok(())
}

pub fn default_local_indexer_requested_by_saved_settings() -> Result<bool> {
    let settings = load_settings_state()?;
    Ok(is_default_local_indexer_endpoint(
        saved_settings_indexer_endpoint(&settings),
    ))
}

#[must_use]
pub fn is_default_local_indexer_endpoint(endpoint: &str) -> bool {
    let endpoint = endpoint.trim().trim_end_matches('/');
    endpoint == DEFAULT_INDEXER_ENDPOINT.trim_end_matches('/')
        || endpoint == "http://localhost:8779"
        || endpoint == "http://[::1]:8779"
}

fn saved_settings_indexer_endpoint(settings: &Value) -> &str {
    settings
        .get("indexer_url")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_INDEXER_ENDPOINT)
}

fn local_indexer_is_reachable() -> bool {
    let Ok(addr) = LOCAL_INDEXER_ADDR.parse::<SocketAddr>() else {
        return false;
    };
    TcpStream::connect_timeout(&addr, Duration::from_millis(250)).is_ok()
}

fn build_indexer_service(workspace: &Path) -> Result<()> {
    let mut command = Command::new("cargo");
    command
        .current_dir(workspace)
        .args(["build", "--release", "-p", INDEXER_PACKAGE])
        .env("RISC0_DEV_MODE", "1");
    apply_user_service_environment(&mut command, INDEXER_SERVICE);

    let status = command
        .status()
        .with_context(|| format!("failed to build {INDEXER_PACKAGE}"))?;
    if !status.success() {
        bail!("{INDEXER_PACKAGE} build exited with {status}");
    }
    Ok(())
}

fn restart_user_service_if_available(unit: &str) -> Result<bool> {
    if !user_service_exists(unit) {
        return Ok(false);
    }

    let status = Command::new("systemctl")
        .args(["--user", "restart", unit])
        .status()
        .with_context(|| format!("failed to restart {unit}"))?;
    if !status.success() {
        bail!("{unit} restart exited with {status}");
    }
    Ok(true)
}

fn wait_for_local_indexer(timeout: Duration) -> Result<()> {
    let started = Instant::now();
    while started.elapsed() < timeout {
        if local_indexer_is_reachable() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(250));
    }

    bail!("local indexer did not listen on {LOCAL_INDEXER_ADDR} after restart");
}

fn spawn_indexer_service(workspace: &Path, binary: &Path) -> Result<()> {
    let config = indexer_config_path(workspace)
        .with_context(|| format!("{INDEXER_PACKAGE} debug config not found"))?;
    let data_dir = workspace.join(INDEXER_DATA_DIR_RELATIVE_PATH);
    fs::create_dir_all(&data_dir).with_context(|| {
        format!(
            "failed to create indexer data directory {}",
            data_dir.display()
        )
    })?;

    let mut command = Command::new(binary);
    command
        .arg(config)
        .arg("--data-dir")
        .arg(&data_dir)
        .args(["--port", "8779"])
        .current_dir(workspace)
        .env("RISC0_DEV_MODE", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let _child = command
        .spawn()
        .with_context(|| format!("failed to start {}", binary.display()))?;
    Ok(())
}

fn user_service_exists(unit: &str) -> bool {
    let Ok(output) = Command::new("systemctl")
        .args(["--user", "show", unit, "-p", "FragmentPath", "--value"])
        .output()
    else {
        return false;
    };
    output.status.success() && !String::from_utf8_lossy(&output.stdout).trim().is_empty()
}

fn apply_user_service_environment(command: &mut Command, unit: &str) {
    let Ok(output) = Command::new("systemctl")
        .args(["--user", "show", unit, "-p", "Environment", "--value"])
        .output()
    else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    for (key, value) in parse_service_environment(&raw) {
        if env::var_os(&key).is_none() {
            command.env(key, value);
        }
    }
}

fn parse_service_environment(raw: &str) -> Vec<(String, String)> {
    raw.split_ascii_whitespace()
        .filter_map(|assignment| {
            let (key, value) = assignment.split_once('=')?;
            (!key.is_empty()).then(|| (key.to_owned(), value.to_owned()))
        })
        .collect()
}

fn find_lez_workspace() -> Option<PathBuf> {
    lez_workspace_candidates()
        .into_iter()
        .find(|path| workspace_has_indexer_service(path))
}

fn workspace_has_indexer_service(path: &Path) -> bool {
    INDEXER_CARGO_TOML_RELATIVE_PATHS
        .iter()
        .any(|relative_path| path.join(relative_path).is_file())
}

fn indexer_config_path(workspace: &Path) -> Option<PathBuf> {
    INDEXER_CONFIG_RELATIVE_PATHS
        .iter()
        .map(|relative_path| workspace.join(relative_path))
        .find(|path| path.is_file())
}

fn lez_workspace_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Some(path) = env::var_os(LEZ_DIR_ENV) {
        push_unique(&mut candidates, PathBuf::from(path));
    }
    if let Ok(current_dir) = env::current_dir() {
        push_workspace_neighbors(&mut candidates, &current_dir);
    }
    push_workspace_neighbors(&mut candidates, Path::new(env!("CARGO_MANIFEST_DIR")));
    candidates
}

fn push_workspace_neighbors(candidates: &mut Vec<PathBuf>, base: &Path) {
    if base
        .file_name()
        .is_some_and(|name| name == "logos-execution-zone")
    {
        push_unique(candidates, base.to_path_buf());
    }
    if let Some(parent) = base.parent() {
        push_unique(candidates, parent.join("logos-execution-zone"));
    }
}

fn push_unique(candidates: &mut Vec<PathBuf>, path: PathBuf) {
    if !candidates.iter().any(|candidate| candidate == &path) {
        candidates.push(path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_local_indexer_endpoint_accepts_loopback_forms() {
        assert!(is_default_local_indexer_endpoint("http://127.0.0.1:8779/"));
        assert!(is_default_local_indexer_endpoint(
            "  http://localhost:8779  "
        ));
        assert!(is_default_local_indexer_endpoint("http://[::1]:8779"));
    }

    #[test]
    fn default_local_indexer_endpoint_rejects_non_local_endpoints() {
        assert!(!is_default_local_indexer_endpoint(
            "https://testnet.lez.logos.co/"
        ));
        assert!(!is_default_local_indexer_endpoint("http://127.0.0.1:8080/"));
    }

    #[test]
    fn saved_settings_indexer_endpoint_defaults_to_local() {
        let settings = serde_json::json!({ "version": 1 });

        assert!(is_default_local_indexer_endpoint(
            saved_settings_indexer_endpoint(&settings)
        ));
    }

    #[test]
    fn saved_settings_indexer_endpoint_preserves_remote_override() {
        let settings = serde_json::json!({
            "version": 1,
            "indexer_url": "https://indexer.example/"
        });

        assert_eq!(
            saved_settings_indexer_endpoint(&settings),
            "https://indexer.example/"
        );
    }

    #[test]
    fn service_environment_parser_keeps_simple_assignments() {
        assert_eq!(
            parse_service_environment("A=1 B=two EMPTY= RUST_LOG=info\n"),
            vec![
                ("A".to_owned(), "1".to_owned()),
                ("B".to_owned(), "two".to_owned()),
                ("EMPTY".to_owned(), String::new()),
                ("RUST_LOG".to_owned(), "info".to_owned()),
            ]
        );
    }

    #[test]
    fn service_environment_parser_ignores_invalid_assignments() {
        assert_eq!(
            parse_service_environment("NOPE =bad OK=value"),
            vec![("OK".to_owned(), "value".to_owned())]
        );
    }

    #[test]
    fn workspace_has_indexer_service_accepts_canonical_layout() -> Result<()> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let workspace = env::temp_dir().join(format!(
            "logos-inspector-indexer-layout-{}-{nonce}",
            std::process::id()
        ));
        let service_dir = workspace.join("lez/indexer/service");
        std::fs::create_dir_all(&service_dir)?;
        std::fs::write(
            service_dir.join("Cargo.toml"),
            "[package]\nname = \"indexer_service\"\n",
        )?;

        if !workspace_has_indexer_service(&workspace) {
            bail!("canonical indexer service layout was not detected");
        }

        std::fs::remove_dir_all(&workspace)?;
        Ok(())
    }
}
