use std::{
    env,
    io::ErrorKind,
    path::{Path, PathBuf},
    process::{Command, Output, Stdio},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{ProgramFileInfo, program_file_info, raw_http_json};

pub const LOCAL_WALLET_HOME_ENV: &str = "LEE_WALLET_HOME_DIR";
const LOCAL_WALLET_DEPLOY_TIMEOUT: Duration = Duration::from_secs(120);
const LOCAL_WALLET_SYNC_TIMEOUT: Duration = Duration::from_secs(120);
const LOCAL_WALLET_LIST_TIMEOUT: Duration = Duration::from_secs(30);
const LOCAL_WALLET_VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const LOCAL_WALLET_POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOCAL_WALLET_OUTPUT_LIMIT: usize = 4096;
const LOCAL_WALLET_ENV_ALLOWLIST: &[&str] = &[
    "PATH",
    "HOME",
    "USER",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "NIX_SSL_CERT_FILE",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalWalletProfileStatus {
    pub source: String,
    pub status: String,
    pub checked_at: String,
    pub detail: String,
    pub version: Option<String>,
    pub home_source: String,
    pub network_profile: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalWalletDeployReport {
    pub source: String,
    pub status: String,
    pub command: String,
    pub wallet_home_source: String,
    pub submitted_at: String,
    pub exit_status: String,
    #[serde(flatten)]
    pub program: ProgramFileInfo,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stdout: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalWalletSyncPrivateReport {
    pub source: String,
    pub status: String,
    pub command: String,
    pub wallet_home_source: String,
    pub submitted_at: String,
    pub exit_status: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stdout: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stderr: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalWalletAccountsReport {
    pub source: String,
    pub status: String,
    pub command: String,
    pub wallet_home_source: String,
    pub checked_at: String,
    pub accounts: Vec<LocalWalletAccountRow>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stdout: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub stderr: String,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LocalWalletAccountRow {
    pub typed_id: String,
    pub account_id: String,
    pub privacy: String,
    pub label: String,
    pub chain_index: String,
    pub state: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct LocalWalletProfileInput {
    #[serde(default, alias = "walletBinary")]
    wallet_binary: String,
    #[serde(default, alias = "walletHome")]
    wallet_home: String,
    #[serde(default, alias = "networkProfile")]
    network_profile: String,
}

pub fn local_wallet_profile_status(profile: Value) -> Result<LocalWalletProfileStatus> {
    local_wallet_profile_status_with_env(
        profile,
        env::var(LOCAL_WALLET_HOME_ENV).unwrap_or_default(),
    )
}

pub fn local_wallet_deploy_program(
    profile: Value,
    program_path: impl AsRef<Path>,
) -> Result<LocalWalletDeployReport> {
    let profile: LocalWalletProfileInput =
        serde_json::from_value(profile).context("failed to parse local wallet profile")?;
    let wallet_binary = profile.wallet_binary.trim();
    if wallet_binary.is_empty() {
        bail!("wallet binary is required to deploy program binary");
    }
    if local_wallet_binary_is_path_like(wallet_binary) && !Path::new(wallet_binary).is_file() {
        bail!("wallet binary is not reachable");
    }

    let explicit_home = profile.wallet_home.trim();
    let env_wallet_home = env::var(LOCAL_WALLET_HOME_ENV).unwrap_or_default();
    let (wallet_home, wallet_home_source) = if !explicit_home.is_empty() {
        (explicit_home.to_owned(), "profile")
    } else if !env_wallet_home.trim().is_empty() {
        (env_wallet_home.trim().to_owned(), LOCAL_WALLET_HOME_ENV)
    } else {
        (String::new(), "none")
    };
    if wallet_home.is_empty() {
        bail!("wallet home directory is required to deploy program binary");
    }
    if !Path::new(&wallet_home).is_dir() {
        bail!("wallet home directory is not reachable");
    }

    let path = program_path.as_ref();
    let program =
        program_file_info(path).context("failed to inspect program binary before deployment")?;
    let mut redactions = vec![wallet_home.as_str(), program.path.as_str()];
    if local_wallet_binary_is_path_like(wallet_binary) {
        redactions.push(wallet_binary);
    }
    let output =
        local_wallet_deploy_program_output(wallet_binary, &wallet_home, path, &redactions)?;

    Ok(LocalWalletDeployReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet deploy-program <program binary>".to_owned(),
        wallet_home_source: wallet_home_source.to_owned(),
        submitted_at: unix_time_text(),
        exit_status: output.status.to_string(),
        stdout: local_wallet_output_text(&output.stdout, &redactions),
        stderr: local_wallet_output_text(&output.stderr, &redactions),
        program,
    })
}

pub fn local_wallet_sync_private(profile: Value) -> Result<LocalWalletSyncPrivateReport> {
    let profile: LocalWalletProfileInput =
        serde_json::from_value(profile).context("failed to parse local wallet profile")?;
    let wallet_binary = profile.wallet_binary.trim();
    if wallet_binary.is_empty() {
        bail!("wallet binary is required to sync private wallet state");
    }
    if local_wallet_binary_is_path_like(wallet_binary) && !Path::new(wallet_binary).is_file() {
        bail!("wallet binary is not reachable");
    }

    let explicit_home = profile.wallet_home.trim();
    let env_wallet_home = env::var(LOCAL_WALLET_HOME_ENV).unwrap_or_default();
    let (wallet_home, wallet_home_source) = if !explicit_home.is_empty() {
        (explicit_home.to_owned(), "profile")
    } else if !env_wallet_home.trim().is_empty() {
        (env_wallet_home.trim().to_owned(), LOCAL_WALLET_HOME_ENV)
    } else {
        (String::new(), "none")
    };
    if wallet_home.is_empty() {
        bail!("wallet home directory is required to sync private wallet state");
    }
    if !Path::new(&wallet_home).is_dir() {
        bail!("wallet home directory is not reachable");
    }

    let mut redactions = vec![wallet_home.as_str()];
    if local_wallet_binary_is_path_like(wallet_binary) {
        redactions.push(wallet_binary);
    }
    let output = local_wallet_sync_private_output(wallet_binary, &wallet_home, &redactions)?;

    Ok(LocalWalletSyncPrivateReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet account sync-private".to_owned(),
        wallet_home_source: wallet_home_source.to_owned(),
        submitted_at: unix_time_text(),
        exit_status: output.status.to_string(),
        stdout: local_wallet_output_text(&output.stdout, &redactions),
        stderr: local_wallet_output_text(&output.stderr, &redactions),
    })
}

pub fn local_wallet_accounts(profile: Value) -> Result<LocalWalletAccountsReport> {
    let profile: LocalWalletProfileInput =
        serde_json::from_value(profile).context("failed to parse local wallet profile")?;
    let wallet_binary = profile.wallet_binary.trim();
    if wallet_binary.is_empty() {
        bail!("wallet binary is required to list wallet accounts");
    }
    if local_wallet_binary_is_path_like(wallet_binary) && !Path::new(wallet_binary).is_file() {
        bail!("wallet binary is not reachable");
    }

    let explicit_home = profile.wallet_home.trim();
    let env_wallet_home = env::var(LOCAL_WALLET_HOME_ENV).unwrap_or_default();
    let (wallet_home, wallet_home_source) = if !explicit_home.is_empty() {
        (explicit_home.to_owned(), "profile")
    } else if !env_wallet_home.trim().is_empty() {
        (env_wallet_home.trim().to_owned(), LOCAL_WALLET_HOME_ENV)
    } else {
        (String::new(), "none")
    };
    if wallet_home.is_empty() {
        bail!("wallet home directory is required to list wallet accounts");
    }
    if !Path::new(&wallet_home).is_dir() {
        bail!("wallet home directory is not reachable");
    }
    if !wallet_home_is_configured(Path::new(&wallet_home)) {
        bail!("wallet home missing wallet_config.json");
    }

    let mut redactions = vec![wallet_home.as_str()];
    if local_wallet_binary_is_path_like(wallet_binary) {
        redactions.push(wallet_binary);
    }
    let output = local_wallet_accounts_output(wallet_binary, &wallet_home, &redactions)?;
    if output.stdout.len() > LOCAL_WALLET_OUTPUT_LIMIT {
        bail!(
            "wallet account list output exceeded {} bytes; refusing to parse partial account data",
            LOCAL_WALLET_OUTPUT_LIMIT
        );
    }
    let stdout = local_wallet_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);
    let accounts = parse_local_wallet_accounts_output(&stdout);

    Ok(LocalWalletAccountsReport {
        source: "local_wallet_cli".to_owned(),
        status: "loaded".to_owned(),
        command: "wallet account list --long".to_owned(),
        wallet_home_source: wallet_home_source.to_owned(),
        checked_at: unix_time_text(),
        accounts,
        stdout,
        stderr,
    })
}

pub async fn bedrock_wallet_balance(
    endpoint: &str,
    public_key: &str,
    tip: Option<&str>,
) -> Result<Value> {
    let public_key = normalize_bedrock_wallet_public_key(public_key)?;
    let mut path = format!("/wallet/{public_key}/balance");
    if let Some(tip) = tip.map(str::trim).filter(|tip| !tip.is_empty()) {
        let tip = normalize_bedrock_hex_id(tip, "balance tip")?;
        path.push_str("?tip=");
        path.push_str(&tip);
    }
    raw_http_json(endpoint, &path).await
}

pub(crate) fn default_wallet_state() -> Value {
    json!({
        "version": 1,
        "profile": {
            "label": "Local wallet",
            "wallet_binary": "",
            "wallet_home": "",
            "network_profile": "",
            "public_key_probe": ""
        },
        "operations": []
    })
}

pub(crate) fn detected_wallet_profile() -> Value {
    json!({
        "label": "Local wallet",
        "wallet_binary": detect_wallet_binary()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        "wallet_home": detect_wallet_home()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    })
}

fn local_wallet_profile_status_with_env(
    profile: Value,
    nssa_env_home: String,
) -> Result<LocalWalletProfileStatus> {
    let profile: LocalWalletProfileInput =
        serde_json::from_value(profile).context("failed to parse local wallet profile")?;
    let wallet_binary = profile.wallet_binary.trim();
    let explicit_home = profile.wallet_home.trim();
    let (env_home, env_home_source) = if !nssa_env_home.trim().is_empty() {
        (nssa_env_home, Some(LOCAL_WALLET_HOME_ENV))
    } else {
        (String::new(), None)
    };
    let wallet_home = if !explicit_home.is_empty() {
        explicit_home.to_owned()
    } else if env_home_source.is_some() {
        env_home.trim().to_owned()
    } else {
        String::new()
    };
    let home_source = if !explicit_home.is_empty() {
        "profile"
    } else {
        env_home_source.unwrap_or("none")
    };

    if wallet_binary.is_empty() && wallet_home.is_empty() {
        return Ok(LocalWalletProfileStatus {
            source: "local_wallet_cli".to_owned(),
            status: "unknown".to_owned(),
            checked_at: unix_time_text(),
            detail: format!("wallet binary and {LOCAL_WALLET_HOME_ENV} not configured"),
            version: None,
            home_source: home_source.to_owned(),
            network_profile: profile.network_profile.trim().to_owned(),
        });
    }

    let mut status = "ok";
    let mut details = Vec::new();
    let mut version = None;

    if wallet_home.is_empty() {
        details.push("wallet home directory not configured".to_owned());
        status = local_wallet_worst_status(status, "degraded");
    } else if !PathBuf::from(&wallet_home).is_dir() {
        details.push("wallet home directory is not reachable".to_owned());
        status = local_wallet_worst_status(status, "down");
    } else if !wallet_home_is_configured(Path::new(&wallet_home)) {
        details.push("wallet home missing wallet_config.json".to_owned());
        status = local_wallet_worst_status(status, "degraded");
    } else if home_source == "profile" {
        details.push("wallet home configured".to_owned());
    } else {
        details.push(format!("{home_source} configured"));
    }

    if wallet_binary.is_empty() {
        details.push("wallet binary not configured".to_owned());
        status = local_wallet_worst_status(status, "degraded");
    } else if local_wallet_binary_is_path_like(wallet_binary) && !Path::new(wallet_binary).is_file()
    {
        details.push("wallet binary is not reachable".to_owned());
        status = local_wallet_worst_status(status, "down");
    } else if status != "down" {
        match local_wallet_binary_version(wallet_binary, &wallet_home) {
            Ok(value) => {
                details.push("wallet binary responded".to_owned());
                version = (!value.is_empty()).then_some(value);
            }
            Err(error) => {
                details.push(format!("wallet binary version check failed: {error:#}"));
                status = local_wallet_worst_status(status, "degraded");
            }
        }
    }

    Ok(LocalWalletProfileStatus {
        source: "local_wallet_cli".to_owned(),
        status: status.to_owned(),
        checked_at: unix_time_text(),
        detail: details.join("; "),
        version,
        home_source: home_source.to_owned(),
        network_profile: profile.network_profile.trim().to_owned(),
    })
}

fn local_wallet_binary_is_path_like(binary: &str) -> bool {
    let binary = binary.trim();
    Path::new(binary).is_absolute()
        || binary.contains(std::path::MAIN_SEPARATOR)
        || binary.contains('/')
        || binary.contains('\\')
}

fn local_wallet_binary_version(binary: &str, wallet_home: &str) -> Result<String> {
    let mut command = Command::new(binary);
    configure_local_wallet_command(&mut command, wallet_home);
    command.arg("--version");
    let mut redactions = Vec::new();
    if local_wallet_binary_is_path_like(binary) {
        redactions.push(binary);
    }
    if !wallet_home.trim().is_empty() {
        redactions.push(wallet_home);
    }
    let output = run_local_wallet_command(
        command,
        "wallet --version",
        LOCAL_WALLET_VERSION_TIMEOUT,
        &redactions,
    )?;
    let text = if output.stdout.is_empty() {
        local_wallet_output_text(&output.stderr, &redactions)
    } else {
        local_wallet_output_text(&output.stdout, &redactions)
    };
    let version = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default()
        .chars()
        .take(160)
        .collect::<String>();
    Ok(version)
}

fn local_wallet_deploy_program_output(
    binary: &str,
    wallet_home: &str,
    program_path: &Path,
    redactions: &[&str],
) -> Result<Output> {
    let mut command = Command::new(binary);
    configure_local_wallet_command(&mut command, wallet_home);
    command.arg("deploy-program").arg(program_path);
    run_local_wallet_command(
        command,
        "wallet deploy-program",
        LOCAL_WALLET_DEPLOY_TIMEOUT,
        redactions,
    )
}

fn local_wallet_sync_private_output(
    binary: &str,
    wallet_home: &str,
    redactions: &[&str],
) -> Result<Output> {
    let mut command = Command::new(binary);
    configure_local_wallet_command(&mut command, wallet_home);
    command.arg("account").arg("sync-private");
    run_local_wallet_command(
        command,
        "wallet account sync-private",
        LOCAL_WALLET_SYNC_TIMEOUT,
        redactions,
    )
}

fn local_wallet_accounts_output(
    binary: &str,
    wallet_home: &str,
    redactions: &[&str],
) -> Result<Output> {
    let mut command = Command::new(binary);
    configure_local_wallet_command(&mut command, wallet_home);
    command.arg("account").arg("list").arg("--long");
    run_local_wallet_command(
        command,
        "wallet account list --long",
        LOCAL_WALLET_LIST_TIMEOUT,
        redactions,
    )
}

fn configure_local_wallet_command(command: &mut Command, wallet_home: &str) {
    command.env_clear();
    for name in LOCAL_WALLET_ENV_ALLOWLIST {
        if let Some(value) = env::var_os(name) {
            command.env(name, value);
        }
    }
    if !wallet_home.trim().is_empty() {
        command.env(LOCAL_WALLET_HOME_ENV, wallet_home);
    }
}

fn run_local_wallet_command(
    mut command: Command,
    label: &str,
    timeout: Duration,
    redactions: &[&str],
) -> Result<Output> {
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .with_context(|| format!("failed to run {label}"))?;
    let started = Instant::now();
    loop {
        if child
            .try_wait()
            .with_context(|| format!("failed to poll {label}"))?
            .is_some()
        {
            break;
        }
        if started.elapsed() >= timeout {
            match child.kill() {
                Ok(()) => {}
                Err(error) if error.kind() == ErrorKind::InvalidInput => {}
                Err(error) => {
                    return Err(error).with_context(|| format!("failed to kill timed-out {label}"));
                }
            }
            let output = child
                .wait_with_output()
                .with_context(|| format!("failed to collect timed-out {label}"))?;
            let message = local_wallet_process_message(&output, redactions);
            bail!(
                "{label} timed out after {} ms: {message}",
                timeout.as_millis()
            );
        }
        thread::sleep(LOCAL_WALLET_POLL_INTERVAL);
    }
    let output = child
        .wait_with_output()
        .with_context(|| format!("failed to collect {label}"))?;
    if !output.status.success() {
        let message = local_wallet_process_message(&output, redactions);
        bail!("{label} exited with {}: {message}", output.status);
    }
    Ok(output)
}

fn local_wallet_process_message(output: &Output, redactions: &[&str]) -> String {
    let message = if output.stderr.is_empty() {
        local_wallet_output_text(&output.stdout, redactions)
    } else {
        local_wallet_output_text(&output.stderr, redactions)
    };
    if message.is_empty() {
        "no output".to_owned()
    } else {
        message
    }
}

fn local_wallet_output_text(output: &[u8], redactions: &[&str]) -> String {
    let text = String::from_utf8_lossy(output).trim().to_owned();
    let mut redacted = text;
    for value in redactions {
        let value = value.trim();
        if !value.is_empty() {
            redacted = redacted.replace(value, "...");
        }
    }
    redacted.chars().take(LOCAL_WALLET_OUTPUT_LIMIT).collect()
}

fn parse_local_wallet_accounts_output(text: &str) -> Vec<LocalWalletAccountRow> {
    let mut rows = Vec::new();
    let mut current: Option<LocalWalletAccountRow> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !line.chars().next().is_some_and(char::is_whitespace)
            && let Some(row) = parse_wallet_account_header(trimmed)
        {
            if let Some(row) = current.replace(row) {
                rows.push(row);
            }
            continue;
        }
        let Some(row) = current.as_mut() else {
            continue;
        };
        if trimmed.starts_with('{') {
            match serde_json::from_str::<Value>(trimmed) {
                Ok(value) => {
                    row.state = "loaded".to_owned();
                    row.data = Some(value);
                }
                Err(_) => push_wallet_account_detail(row, trimmed),
            }
        } else if trimmed.eq_ignore_ascii_case("uninitialized") {
            row.state = "uninitialized".to_owned();
            push_wallet_account_detail(row, trimmed);
        } else if trimmed.to_ascii_lowercase().starts_with("error ") {
            row.state = "error".to_owned();
            push_wallet_account_detail(row, trimmed);
        } else {
            push_wallet_account_detail(row, trimmed);
        }
    }

    if let Some(row) = current {
        rows.push(row);
    }
    rows
}

fn parse_wallet_account_header(line: &str) -> Option<LocalWalletAccountRow> {
    let public_index = line.find("Public/");
    let private_index = line.find("Private/");
    let id_index = match (public_index, private_index) {
        (Some(public), Some(private)) => public.min(private),
        (Some(public), None) => public,
        (None, Some(private)) => private,
        (None, None) => return None,
    };
    let chain_index = line[..id_index].trim();
    let rest = line[id_index..].trim();
    let label = rest
        .split_once('[')
        .and_then(|(_, suffix)| suffix.split_once(']').map(|(label, _)| label.trim()))
        .unwrap_or_default();
    let typed_id = rest
        .split_once('[')
        .map_or(rest, |(id, _)| id)
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .trim_end_matches(',');
    let (privacy, account_id) = typed_id.split_once('/')?;
    Some(LocalWalletAccountRow {
        typed_id: typed_id.to_owned(),
        account_id: account_id.to_owned(),
        privacy: privacy.to_ascii_lowercase(),
        label: label.to_owned(),
        chain_index: chain_index.to_owned(),
        state: "unknown".to_owned(),
        detail: String::new(),
        data: None,
    })
}

fn push_wallet_account_detail(row: &mut LocalWalletAccountRow, detail: &str) {
    if row.detail.is_empty() {
        row.detail = detail.to_owned();
    } else {
        row.detail.push_str("; ");
        row.detail.push_str(detail);
    }
}

fn local_wallet_worst_status(current: &str, candidate: &str) -> &'static str {
    match (
        local_wallet_status_rank(current),
        local_wallet_status_rank(candidate),
    ) {
        (left, right) if left >= right => local_wallet_status_name(left),
        (_, right) => local_wallet_status_name(right),
    }
}

fn local_wallet_status_rank(status: &str) -> u8 {
    match status {
        "down" => 3,
        "degraded" => 2,
        "unknown" => 1,
        _ => 0,
    }
}

fn local_wallet_status_name(rank: u8) -> &'static str {
    match rank {
        3 => "down",
        2 => "degraded",
        1 => "unknown",
        _ => "ok",
    }
}

fn normalize_bedrock_wallet_public_key(value: &str) -> Result<String> {
    normalize_bedrock_hex_id(value, "wallet public key")
}

fn normalize_bedrock_hex_id(value: &str, label: &str) -> Result<String> {
    let text = value.trim();
    let text = text.strip_prefix("0x").unwrap_or(text);
    if text.is_empty() {
        bail!("{label} is required")
    }
    if text.len() != 64 || !text.chars().all(|ch| ch.is_ascii_hexdigit()) {
        bail!("{label} must be 64 hex characters")
    }
    Ok(text.to_ascii_lowercase())
}

fn detect_wallet_binary() -> Option<PathBuf> {
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

fn detect_wallet_home() -> Option<PathBuf> {
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

fn wallet_home_is_configured(path: &Path) -> bool {
    path.is_dir() && path.join("wallet_config.json").is_file()
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

fn unix_time_text() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_wallet_profile_status_reports_missing_binary_without_path_leak() {
        let status = local_wallet_profile_status(serde_json::json!({
            "wallet_binary": "/definitely/not/a/logos/wallet",
            "wallet_home": ".",
            "network_profile": "local"
        }));

        assert!(status.is_ok(), "{status:?}");
        let Ok(status) = status else {
            return;
        };
        assert_eq!(status.status, "down");
        assert_eq!(status.source, "local_wallet_cli");
        assert_eq!(status.home_source, "profile");
        assert_eq!(status.network_profile, "local");
        assert!(!status.detail.contains("/definitely/not/a/logos/wallet"));
    }

    #[test]
    fn local_wallet_profile_status_keeps_blank_wallet_home_unconfigured() {
        let status = local_wallet_profile_status_with_env(
            serde_json::json!({
                "wallet_binary": "/definitely/not/a/logos/wallet",
                "wallet_home": "",
                "network_profile": "local"
            }),
            String::new(),
        );

        assert!(status.is_ok(), "{status:?}");
        let Ok(status) = status else {
            return;
        };
        assert_eq!(status.home_source, "none");
        assert!(
            status
                .detail
                .contains("wallet home directory not configured")
        );
    }

    #[test]
    fn local_wallet_profile_status_accepts_lee_wallet_home_env() {
        let status = local_wallet_profile_status_with_env(
            serde_json::json!({
                "wallet_binary": "",
                "wallet_home": "",
                "network_profile": "local"
            }),
            ".".to_owned(),
        );

        assert!(status.is_ok(), "{status:?}");
        let Ok(status) = status else {
            return;
        };
        assert_eq!(status.home_source, LOCAL_WALLET_HOME_ENV);
        assert!(
            status
                .detail
                .contains("wallet home missing wallet_config.json")
        );
    }

    #[test]
    fn parse_local_wallet_accounts_output_extracts_rows_and_json() {
        let rows = parse_local_wallet_accounts_output(
            r#"m/0 Public/7wHg9sbJwc6h3NP1S9bekfAzB8CHifEcxKswCKUt3YQo [main]
  Regular account
  {"balance":42,"program_owner":"owner","data":"","nonce":1}
Private/3oCG8gqdKLMegw4rRfyaMQvuPHpcASt7xwttsmnZLSkw
  Uninitialized"#,
        );

        assert_eq!(rows.len(), 2);
        let [public_account, private_account] = rows.as_slice() else {
            return;
        };
        assert_eq!(public_account.privacy, "public");
        assert_eq!(public_account.label, "main");
        assert_eq!(public_account.chain_index, "m/0");
        assert_eq!(public_account.state, "loaded");
        assert_eq!(
            public_account
                .data
                .as_ref()
                .and_then(|value| value.get("balance")),
            Some(&serde_json::json!(42))
        );
        assert_eq!(private_account.privacy, "private");
        assert_eq!(private_account.state, "uninitialized");
    }

    #[test]
    fn local_wallet_deploy_program_requires_wallet_binary() {
        let error_text = match local_wallet_deploy_program(
            serde_json::json!({
                "wallet_binary": "",
                "wallet_home": ".",
                "network_profile": "local"
            }),
            "program.bin",
        ) {
            Ok(report) => format!("{report:?}"),
            Err(error) => format!("{error:#}"),
        };

        assert!(
            error_text.contains("wallet binary is required"),
            "{error_text}"
        );
    }

    #[test]
    fn local_wallet_output_text_redacts_values_and_limits_output() {
        let input = format!(
            "failed at /sensitive/wallet/home {}",
            "x".repeat(LOCAL_WALLET_OUTPUT_LIMIT + 100)
        );
        let output = local_wallet_output_text(input.as_bytes(), &["/sensitive/wallet/home"]);

        assert!(!output.contains("/sensitive/wallet/home"));
        assert!(output.contains("..."));
        assert!(output.chars().count() <= LOCAL_WALLET_OUTPUT_LIMIT);
    }

    #[test]
    fn bedrock_wallet_public_key_normalization_requires_64_hex() {
        let normalized = normalize_bedrock_wallet_public_key(&format!("0x{}", "ab".repeat(32)));
        assert!(normalized.is_ok(), "{normalized:?}");
        assert_eq!(normalized.unwrap_or_default(), "ab".repeat(32));
        assert!(normalize_bedrock_wallet_public_key("Public/abc123").is_err());
        assert!(normalize_bedrock_wallet_public_key("abc123").is_err());
        assert!(normalize_bedrock_wallet_public_key("abc/123").is_err());
        assert!(normalize_bedrock_wallet_public_key("abc?tip=1").is_err());
    }
}
