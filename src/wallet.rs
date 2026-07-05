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
const LOCAL_WALLET_MUTATION_TIMEOUT: Duration = Duration::from_secs(300);
const LOCAL_WALLET_SYNC_TIMEOUT: Duration = Duration::from_secs(120);
const LOCAL_WALLET_LIST_TIMEOUT: Duration = Duration::from_secs(30);
const LOCAL_WALLET_VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const LOCAL_WALLET_POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOCAL_WALLET_OUTPUT_LIMIT: usize = 4096;
const LOCAL_WALLET_MAX_COMMAND_ARGS: usize = 64;
const LOCAL_WALLET_MAX_COMMAND_ARG_LEN: usize = 2048;
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
pub struct LocalWalletCommandReport {
    pub source: String,
    pub status: String,
    pub command: String,
    pub wallet_home_source: String,
    pub submitted_at: String,
    pub exit_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub privacy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tx_hash: Option<String>,
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

#[derive(Debug, Clone, Default, Deserialize)]
struct LocalWalletSendNativeInput {
    #[serde(default)]
    from: String,
    #[serde(default)]
    to: String,
    #[serde(default, alias = "toNpk")]
    to_npk: String,
    #[serde(default, alias = "toVpk")]
    to_vpk: String,
    #[serde(default, alias = "toKeys")]
    to_keys: String,
    #[serde(default, alias = "toIdentifier")]
    to_identifier: String,
    #[serde(default)]
    amount: String,
}

#[derive(Debug, Clone)]
struct ResolvedLocalWalletProfile {
    wallet_binary: String,
    wallet_home: String,
    wallet_home_source: String,
}

pub fn local_wallet_profile_status(profile: Value) -> Result<LocalWalletProfileStatus> {
    local_wallet_profile_status_with_env(
        profile,
        env::var(LOCAL_WALLET_HOME_ENV).unwrap_or_default(),
    )
}

fn resolve_local_wallet_profile(
    profile: Value,
    action: &str,
    require_config: bool,
) -> Result<ResolvedLocalWalletProfile> {
    let profile: LocalWalletProfileInput =
        serde_json::from_value(profile).context("failed to parse local wallet profile")?;
    let wallet_binary = profile.wallet_binary.trim();
    if wallet_binary.is_empty() {
        bail!("wallet binary is required to {action}");
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
        bail!("wallet home directory is required to {action}");
    }
    if !Path::new(&wallet_home).is_dir() {
        bail!("wallet home directory is not reachable");
    }
    if require_config && !wallet_home_is_configured(Path::new(&wallet_home)) {
        bail!("wallet home missing wallet_config.json");
    }

    Ok(ResolvedLocalWalletProfile {
        wallet_binary: wallet_binary.to_owned(),
        wallet_home,
        wallet_home_source: wallet_home_source.to_owned(),
    })
}

pub fn local_wallet_deploy_program(
    profile: Value,
    program_path: impl AsRef<Path>,
) -> Result<LocalWalletDeployReport> {
    let wallet = resolve_local_wallet_profile(profile, "deploy program binary", false)?;
    let path = program_path.as_ref();
    let program =
        program_file_info(path).context("failed to inspect program binary before deployment")?;
    let mut redactions = wallet.redactions();
    redactions.push(program.path.as_str());
    let output = local_wallet_deploy_program_output(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        path,
        &redactions,
    )?;

    Ok(LocalWalletDeployReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet deploy-program <program binary>".to_owned(),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.status.to_string(),
        stdout: local_wallet_output_text(&output.stdout, &redactions),
        stderr: local_wallet_output_text(&output.stderr, &redactions),
        program,
    })
}

pub fn local_wallet_sync_private(profile: Value) -> Result<LocalWalletSyncPrivateReport> {
    let wallet = resolve_local_wallet_profile(profile, "sync private wallet state", false)?;
    let redactions = wallet.redactions();
    let output =
        local_wallet_sync_private_output(&wallet.wallet_binary, &wallet.wallet_home, &redactions)?;

    Ok(LocalWalletSyncPrivateReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet account sync-private".to_owned(),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.status.to_string(),
        stdout: local_wallet_output_text(&output.stdout, &redactions),
        stderr: local_wallet_output_text(&output.stderr, &redactions),
    })
}

pub fn local_wallet_accounts(profile: Value) -> Result<LocalWalletAccountsReport> {
    let wallet = resolve_local_wallet_profile(profile, "list wallet accounts", true)?;
    let redactions = wallet.redactions();
    let output =
        local_wallet_accounts_output(&wallet.wallet_binary, &wallet.wallet_home, &redactions)?;
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
        wallet_home_source: wallet.wallet_home_source.clone(),
        checked_at: unix_time_text(),
        accounts,
        stdout,
        stderr,
    })
}

pub fn local_wallet_create_account(
    profile: Value,
    privacy: &str,
    label: Option<&str>,
) -> Result<LocalWalletCommandReport> {
    let privacy = normalized_wallet_account_privacy(privacy)?;
    let label = label.map(str::trim).filter(|value| !value.is_empty());
    let wallet = resolve_local_wallet_profile(profile, "create wallet account", false)?;
    let mut args = vec!["account".to_owned(), "new".to_owned(), privacy.to_owned()];
    if let Some(label) = label {
        args.push("--label".to_owned());
        args.push(label.to_owned());
    }
    let redactions = wallet.redactions();
    let output = local_wallet_args_output(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        &args,
        "wallet account new",
        LOCAL_WALLET_MUTATION_TIMEOUT,
        &redactions,
    )?;
    let stdout = local_wallet_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);
    let account_id = extract_wallet_account_id(&format!("{stdout}\n{stderr}"));

    Ok(LocalWalletCommandReport {
        source: "local_wallet_cli".to_owned(),
        status: "created".to_owned(),
        command: format!("wallet account new {privacy}"),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.status.to_string(),
        privacy: Some(privacy.to_owned()),
        account_id,
        from: None,
        to: None,
        amount: None,
        tx_hash: None,
        stdout,
        stderr,
    })
}

pub fn local_wallet_send_transaction(
    profile: Value,
    request: Value,
) -> Result<LocalWalletCommandReport> {
    let request: LocalWalletSendNativeInput =
        serde_json::from_value(request).context("failed to parse wallet send request")?;
    let from = request.from.trim();
    if from.is_empty() {
        bail!("sender account is required");
    }
    let amount = normalized_wallet_amount(&request.amount)?;
    let to = request.to.trim();
    let to_keys = request.to_keys.trim();
    let to_npk = request.to_npk.trim();
    let to_vpk = request.to_vpk.trim();
    let to_identifier = request.to_identifier.trim();
    validate_wallet_send_recipient(to, to_keys, to_npk, to_vpk)?;

    let wallet = resolve_local_wallet_profile(profile, "send wallet transaction", false)?;
    let mut args = vec![
        "auth-transfer".to_owned(),
        "send".to_owned(),
        "--from".to_owned(),
        from.to_owned(),
        "--amount".to_owned(),
        amount.clone(),
    ];
    let report_to = if !to.is_empty() {
        args.push("--to".to_owned());
        args.push(to.to_owned());
        Some(to.to_owned())
    } else if !to_keys.is_empty() {
        args.push("--to-keys".to_owned());
        args.push(to_keys.to_owned());
        Some("<keys file>".to_owned())
    } else {
        args.push("--to-npk".to_owned());
        args.push(to_npk.to_owned());
        args.push("--to-vpk".to_owned());
        args.push(to_vpk.to_owned());
        Some("<recipient keys>".to_owned())
    };
    if !to_identifier.is_empty() {
        normalized_wallet_identifier(to_identifier)?;
        args.push("--to-identifier".to_owned());
        args.push(to_identifier.to_owned());
    }

    let mut redactions = wallet.redactions();
    if local_wallet_binary_is_path_like(to_keys) {
        redactions.push(to_keys);
    }
    let output = local_wallet_args_output(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        &args,
        "wallet auth-transfer send",
        LOCAL_WALLET_MUTATION_TIMEOUT,
        &redactions,
    )?;
    let stdout = local_wallet_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);
    let tx_hash = extract_wallet_tx_hash(&format!("{stdout}\n{stderr}"));

    Ok(LocalWalletCommandReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet auth-transfer send".to_owned(),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.status.to_string(),
        privacy: None,
        account_id: None,
        from: Some(from.to_owned()),
        to: report_to,
        amount: Some(amount),
        tx_hash,
        stdout,
        stderr,
    })
}

pub fn local_wallet_command(profile: Value, args: Vec<String>) -> Result<LocalWalletCommandReport> {
    let args = normalized_wallet_command_args(args)?;
    let wallet = resolve_local_wallet_profile(profile, "run wallet command", false)?;
    let redactions = wallet.redactions();
    let output = local_wallet_args_output(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        &args,
        "wallet command",
        LOCAL_WALLET_MUTATION_TIMEOUT,
        &redactions,
    )?;
    let stdout = local_wallet_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);

    Ok(LocalWalletCommandReport {
        source: "local_wallet_cli".to_owned(),
        status: "completed".to_owned(),
        command: wallet_command_label(&args),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.status.to_string(),
        privacy: None,
        account_id: extract_wallet_account_id(&format!("{stdout}\n{stderr}")),
        from: None,
        to: None,
        amount: None,
        tx_hash: extract_wallet_tx_hash(&format!("{stdout}\n{stderr}")),
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

impl ResolvedLocalWalletProfile {
    fn redactions(&self) -> Vec<&str> {
        let mut redactions = vec![self.wallet_home.as_str()];
        if local_wallet_binary_is_path_like(&self.wallet_binary) {
            redactions.push(self.wallet_binary.as_str());
        }
        redactions
    }
}

fn normalized_wallet_account_privacy(value: &str) -> Result<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "public" => Ok("public"),
        "private" => Ok("private"),
        _ => bail!("wallet account privacy must be public or private"),
    }
}

fn normalized_wallet_amount(value: &str) -> Result<String> {
    let amount = value.trim();
    if amount.is_empty() {
        bail!("transaction amount is required");
    }
    let parsed = amount
        .parse::<u128>()
        .context("transaction amount must be a positive integer")?;
    if parsed == 0 {
        bail!("transaction amount must be greater than zero");
    }
    Ok(parsed.to_string())
}

fn normalized_wallet_identifier(value: &str) -> Result<u128> {
    value
        .trim()
        .parse::<u128>()
        .context("recipient identifier must be an unsigned integer")
}

fn validate_wallet_send_recipient(
    to: &str,
    to_keys: &str,
    to_npk: &str,
    to_vpk: &str,
) -> Result<()> {
    let key_pair_present = !to_npk.is_empty() || !to_vpk.is_empty();
    let variants = usize::from(!to.is_empty())
        + usize::from(!to_keys.is_empty())
        + usize::from(key_pair_present);
    if variants != 1 {
        bail!("provide exactly one recipient form: to, to_keys, or to_npk/to_vpk");
    }
    if key_pair_present && (to_npk.is_empty() || to_vpk.is_empty()) {
        bail!("to_npk and to_vpk are both required when using recipient keys");
    }
    Ok(())
}

fn normalized_wallet_command_args(args: Vec<String>) -> Result<Vec<String>> {
    let mut args = args
        .into_iter()
        .map(|arg| arg.trim().to_owned())
        .filter(|arg| !arg.is_empty())
        .collect::<Vec<_>>();
    if args.first().is_some_and(|arg| arg == "wallet") {
        args.remove(0);
    }
    if args.is_empty() {
        bail!("wallet command requires at least one argument");
    }
    if args.len() > LOCAL_WALLET_MAX_COMMAND_ARGS {
        bail!(
            "wallet command accepts at most {} arguments",
            LOCAL_WALLET_MAX_COMMAND_ARGS
        );
    }
    for arg in &args {
        if arg.chars().count() > LOCAL_WALLET_MAX_COMMAND_ARG_LEN {
            bail!(
                "wallet command arguments must be at most {} characters",
                LOCAL_WALLET_MAX_COMMAND_ARG_LEN
            );
        }
        if arg.contains('\0') {
            bail!("wallet command arguments cannot contain NUL bytes");
        }
    }
    Ok(args)
}

fn wallet_command_label(args: &[String]) -> String {
    let mut label = String::from("wallet");
    for arg in args.iter().take(8) {
        label.push(' ');
        label.push_str(arg);
    }
    if args.len() > 8 {
        label.push_str(" ...");
    }
    label.chars().take(240).collect()
}

fn extract_wallet_account_id(text: &str) -> Option<String> {
    text.split_whitespace()
        .map(|token| token.trim_matches(|ch: char| matches!(ch, ',' | '.' | ';' | ')' | '(')))
        .find(|token| token.starts_with("Public/") || token.starts_with("Private/"))
        .map(ToOwned::to_owned)
}

fn extract_wallet_tx_hash(text: &str) -> Option<String> {
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if let Some((_, value)) = line.split_once("Transaction hash is ") {
            return Some(value.trim().to_owned());
        }
        if let Some((_, value)) = line.split_once("tx_hash=") {
            return Some(value.trim().to_owned());
        }
        if let Some((_, value)) = line.split_once("tx_hash:") {
            return Some(value.trim().to_owned());
        }
    }
    None
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

fn local_wallet_args_output(
    binary: &str,
    wallet_home: &str,
    args: &[String],
    label: &str,
    timeout: Duration,
    redactions: &[&str],
) -> Result<Output> {
    let mut command = Command::new(binary);
    configure_local_wallet_command(&mut command, wallet_home);
    command.args(args);
    run_local_wallet_command(command, label, timeout, redactions)
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
    fn local_wallet_send_validation_requires_single_recipient_form() {
        assert!(validate_wallet_send_recipient("Public/a", "", "", "").is_ok());
        assert!(validate_wallet_send_recipient("", "/tmp/recipient.keys", "", "").is_ok());
        assert!(validate_wallet_send_recipient("", "", "npk", "vpk").is_ok());
        assert!(validate_wallet_send_recipient("", "", "npk", "").is_err());
        assert!(validate_wallet_send_recipient("Public/a", "/tmp/recipient.keys", "", "").is_err());
    }

    #[test]
    fn normalized_wallet_command_args_drops_leading_wallet() {
        let args = normalized_wallet_command_args(vec![
            "wallet".to_owned(),
            "account".to_owned(),
            "list".to_owned(),
        ]);

        assert!(args.is_ok(), "{args:?}");
        assert_eq!(
            args.unwrap_or_default(),
            vec!["account".to_owned(), "list".to_owned()]
        );
        assert!(normalized_wallet_command_args(Vec::new()).is_err());
    }

    #[test]
    fn wallet_output_extractors_find_account_and_tx() {
        let text = "Generated new account with account_id Private/abc123 at path /1\nTransaction hash is deadbeef";

        assert_eq!(
            extract_wallet_account_id(text).as_deref(),
            Some("Private/abc123")
        );
        assert_eq!(extract_wallet_tx_hash(text).as_deref(), Some("deadbeef"));
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
