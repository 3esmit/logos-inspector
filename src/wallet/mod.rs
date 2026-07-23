use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{
    lez::{ProgramFileInfo, program_file_info},
    source_routing::bedrock_layer,
};

mod instruction;
mod profile;
mod runner;
#[cfg(feature = "local-wallet-runtime")]
mod testnet_v02;

use crate::support::command_runner::CommandControl;
pub(crate) use instruction::local_wallet_instruction_submit_to;
pub use instruction::{
    InstructionPlanField, LocalWalletInstructionPlanReport, LocalWalletInstructionReport,
    LocalWalletInstructionRequest, ResolvedInstructionAccount, ResolvedInstructionArg,
    local_wallet_instruction_plan, local_wallet_instruction_preview,
    local_wallet_instruction_submit,
};
use profile::{
    LocalWalletProfileInput, detect_wallet_binary, detect_wallet_home,
    local_wallet_binary_is_path_like, local_wallet_readiness, parse_local_wallet_profile,
    resolve_local_wallet_accounts_profile, resolve_local_wallet_command_profile,
    resolve_local_wallet_create_profile, resolve_local_wallet_profile,
    resolve_local_wallet_send_profile, wallet_home_from_environment, wallet_home_is_configured,
};
use runner::{
    CliLocalWalletRunner, ControlledCliLocalWalletRunner, LocalWalletInvocation, LocalWalletRunner,
    local_wallet_accounts_output_text, local_wallet_output_text,
};
pub const LOCAL_WALLET_HOME_ENV: &str = "LEE_WALLET_HOME_DIR";
const LEGACY_LOCAL_WALLET_HOME_ENV: &str = "NSSA_WALLET_HOME_DIR";
const LOCAL_WALLET_DEPLOY_TIMEOUT: Duration = Duration::from_secs(120);
const LOCAL_WALLET_MUTATION_TIMEOUT: Duration = Duration::from_secs(300);
const LOCAL_WALLET_SYNC_TIMEOUT: Duration = Duration::from_secs(120);
const LOCAL_WALLET_LIST_TIMEOUT: Duration = Duration::from_secs(30);
const LOCAL_WALLET_PROFILE_TIMEOUT: Duration = Duration::from_secs(10);
const LOCAL_WALLET_VERSION_TIMEOUT: Duration = Duration::from_secs(5);
const LOCAL_WALLET_POLL_INTERVAL: Duration = Duration::from_millis(50);
const LOCAL_WALLET_OUTPUT_LIMIT: usize = 4096;
const LOCAL_WALLET_ACCOUNTS_OUTPUT_LIMIT: usize = 1024 * 1024;
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
    pub readiness: LocalWalletReadiness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LocalWalletReadiness {
    pub wallet_binary_ready: bool,
    pub wallet_home_ready: bool,
    pub wallet_config_ready: bool,
    pub wallet_storage_ready: bool,
    pub command_ready: bool,
    pub accounts_ready: bool,
    pub instruction_submit_ready: bool,
    pub backup_encryption_ready: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalWalletDeployReport {
    pub source: String,
    pub status: String,
    pub command: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub operation_detail: String,
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
    #[serde(skip_serializing_if = "String::is_empty")]
    pub operation_detail: String,
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
    #[serde(skip_serializing_if = "String::is_empty")]
    pub operation_detail: String,
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
    #[serde(skip_serializing_if = "String::is_empty")]
    pub operation_detail: String,
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

pub fn local_wallet_profile_status(profile: Value) -> Result<LocalWalletProfileStatus> {
    let (env_home, env_home_source) = wallet_home_from_environment().map_or_else(
        || (String::new(), None),
        |(home, source)| (home, Some(source)),
    );
    local_wallet_profile_status_with_runner(
        &CliLocalWalletRunner,
        profile,
        env_home,
        env_home_source,
    )
}

pub fn local_wallet_deploy_program(
    profile: Value,
    program_path: impl AsRef<Path>,
) -> Result<LocalWalletDeployReport> {
    local_wallet_deploy_program_with_runner(&CliLocalWalletRunner, profile, program_path)
}

pub(crate) fn local_wallet_deploy_program_controlled(
    profile: Value,
    program_path: impl AsRef<Path>,
    control: CommandControl,
) -> Result<LocalWalletDeployReport> {
    local_wallet_deploy_program_with_runner(
        &ControlledCliLocalWalletRunner::new(control),
        profile,
        program_path,
    )
}

fn local_wallet_deploy_program_with_runner<R: LocalWalletRunner>(
    runner: &R,
    profile: Value,
    program_path: impl AsRef<Path>,
) -> Result<LocalWalletDeployReport> {
    let wallet = resolve_local_wallet_profile(profile, "deploy program binary", false)?;
    let path = program_path.as_ref();
    let program =
        program_file_info(path).context("failed to inspect program binary before deployment")?;
    let mut redactions = wallet.redactions();
    redactions.push(program.path.as_str());
    let output = runner.run(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        LocalWalletInvocation::DeployProgram { program_path: path },
        &redactions,
    )?;

    Ok(LocalWalletDeployReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet deploy-program <program binary>".to_owned(),
        operation_detail: "submitted".to_owned(),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.exit_status,
        stdout: local_wallet_output_text(&output.stdout, &redactions),
        stderr: local_wallet_output_text(&output.stderr, &redactions),
        program,
    })
}

pub fn local_wallet_sync_private(profile: Value) -> Result<LocalWalletSyncPrivateReport> {
    local_wallet_sync_private_with_runner(&CliLocalWalletRunner, profile)
}

pub(crate) fn local_wallet_sync_private_controlled(
    profile: Value,
    control: CommandControl,
) -> Result<LocalWalletSyncPrivateReport> {
    local_wallet_sync_private_with_runner(&ControlledCliLocalWalletRunner::new(control), profile)
}

fn local_wallet_sync_private_with_runner<R: LocalWalletRunner>(
    runner: &R,
    profile: Value,
) -> Result<LocalWalletSyncPrivateReport> {
    let wallet = resolve_local_wallet_profile(profile, "sync private wallet state", false)?;
    let redactions = wallet.redactions();
    let output = runner.run(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        LocalWalletInvocation::SyncPrivate,
        &redactions,
    )?;

    Ok(LocalWalletSyncPrivateReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet account sync-private".to_owned(),
        operation_detail: local_wallet_status_detail("submitted", &wallet.wallet_home_source),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.exit_status,
        stdout: local_wallet_output_text(&output.stdout, &redactions),
        stderr: local_wallet_output_text(&output.stderr, &redactions),
    })
}

pub fn local_wallet_accounts(profile: Value) -> Result<LocalWalletAccountsReport> {
    local_wallet_accounts_with_runner(&CliLocalWalletRunner, profile)
}

pub(crate) fn local_wallet_accounts_controlled(
    profile: Value,
    control: CommandControl,
) -> Result<LocalWalletAccountsReport> {
    local_wallet_accounts_with_runner(&ControlledCliLocalWalletRunner::new(control), profile)
}

fn local_wallet_accounts_with_runner<R: LocalWalletRunner>(
    runner: &R,
    profile: Value,
) -> Result<LocalWalletAccountsReport> {
    let wallet = resolve_local_wallet_accounts_profile(profile)?;
    let redactions = wallet.redactions();
    let output = runner.run(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        LocalWalletInvocation::Accounts,
        &redactions,
    )?;
    if output.stdout.len() > LOCAL_WALLET_ACCOUNTS_OUTPUT_LIMIT {
        bail!(
            "wallet account list output exceeded {} bytes; refusing to parse partial account data",
            LOCAL_WALLET_ACCOUNTS_OUTPUT_LIMIT
        );
    }
    let stdout = local_wallet_accounts_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);
    let accounts = parse_local_wallet_accounts_output(&stdout);

    Ok(LocalWalletAccountsReport {
        source: "local_wallet_cli".to_owned(),
        status: "loaded".to_owned(),
        command: "wallet account list --long".to_owned(),
        operation_detail: format!("{} accounts", accounts.len()),
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
    local_wallet_create_account_with_runner(&CliLocalWalletRunner, profile, privacy, label)
}

pub(crate) fn local_wallet_create_account_controlled(
    profile: Value,
    privacy: &str,
    label: Option<&str>,
    control: CommandControl,
) -> Result<LocalWalletCommandReport> {
    local_wallet_create_account_with_runner(
        &ControlledCliLocalWalletRunner::new(control),
        profile,
        privacy,
        label,
    )
}

fn local_wallet_create_account_with_runner<R: LocalWalletRunner>(
    runner: &R,
    profile: Value,
    privacy: &str,
    label: Option<&str>,
) -> Result<LocalWalletCommandReport> {
    let privacy = normalized_wallet_account_privacy(privacy)?;
    let label = label.map(str::trim).filter(|value| !value.is_empty());
    let wallet = resolve_local_wallet_create_profile(profile)?;
    let mut args = vec!["account".to_owned(), "new".to_owned(), privacy.to_owned()];
    if let Some(label) = label {
        args.push("--label".to_owned());
        args.push(label.to_owned());
    }
    let redactions = wallet.redactions();
    let output = runner.run(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        LocalWalletInvocation::Args {
            args: &args,
            label: "wallet account new",
        },
        &redactions,
    )?;
    let stdout = local_wallet_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);
    let account_id = extract_wallet_account_id(&format!("{stdout}\n{stderr}"));

    Ok(LocalWalletCommandReport {
        source: "local_wallet_cli".to_owned(),
        status: "created".to_owned(),
        command: format!("wallet account new {privacy}"),
        operation_detail: local_wallet_command_detail(
            "created",
            "wallet account new",
            account_id.as_deref(),
            None,
        ),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.exit_status,
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
    local_wallet_send_transaction_with_runner(&CliLocalWalletRunner, profile, request)
}

pub(crate) fn local_wallet_send_transaction_controlled(
    profile: Value,
    request: Value,
    control: CommandControl,
) -> Result<LocalWalletCommandReport> {
    local_wallet_send_transaction_with_runner(
        &ControlledCliLocalWalletRunner::new(control),
        profile,
        request,
    )
}

fn local_wallet_send_transaction_with_runner<R: LocalWalletRunner>(
    runner: &R,
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

    let wallet = resolve_local_wallet_send_profile(profile)?;
    let from = resolve_owned_public_wallet_sender(runner, &wallet, from)?;
    let mut args = vec![
        "auth-transfer".to_owned(),
        "send".to_owned(),
        "--from".to_owned(),
        from.clone(),
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
    let output = runner.run(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        LocalWalletInvocation::Args {
            args: &args,
            label: "wallet auth-transfer send",
        },
        &redactions,
    )?;
    let stdout = local_wallet_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);
    let tx_hash = extract_wallet_tx_hash(&format!("{stdout}\n{stderr}"));

    Ok(LocalWalletCommandReport {
        source: "local_wallet_cli".to_owned(),
        status: "submitted".to_owned(),
        command: "wallet auth-transfer send".to_owned(),
        operation_detail: local_wallet_command_detail(
            "submitted",
            "wallet auth-transfer send",
            None,
            tx_hash.as_deref(),
        ),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.exit_status,
        privacy: None,
        account_id: None,
        from: Some(from),
        to: report_to,
        amount: Some(amount),
        tx_hash,
        stdout,
        stderr,
    })
}

fn resolve_owned_public_wallet_sender<R: LocalWalletRunner>(
    runner: &R,
    wallet: &profile::ResolvedLocalWalletProfile,
    requested_from: &str,
) -> Result<String> {
    let redactions = wallet.redactions();
    let output = runner.run(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        LocalWalletInvocation::ProfileHomeProbe,
        &redactions,
    )?;
    if output.stdout.len() > LOCAL_WALLET_ACCOUNTS_OUTPUT_LIMIT {
        bail!(
            "wallet account list output exceeded {} bytes; refusing to parse partial account data",
            LOCAL_WALLET_ACCOUNTS_OUTPUT_LIMIT
        );
    }
    let stdout = local_wallet_accounts_output_text(&output.stdout, &redactions);
    let accounts = parse_local_wallet_accounts_output(&stdout);
    if let Some(account) = accounts
        .iter()
        .find(|account| account.privacy == "public" && account.typed_id == requested_from)
    {
        return Ok(account.typed_id.clone());
    }

    let matching_labels = accounts
        .iter()
        .filter(|account| account.privacy == "public" && account.label == requested_from)
        .collect::<Vec<_>>();
    match matching_labels.as_slice() {
        [account] => Ok(account.typed_id.clone()),
        [] => bail!("sender must be an owned Public wallet account"),
        _ => bail!(
            "sender label matches multiple owned Public wallet accounts; use a Public/... account id"
        ),
    }
}

pub fn local_wallet_command(profile: Value, args: Vec<String>) -> Result<LocalWalletCommandReport> {
    local_wallet_command_with_runner(&CliLocalWalletRunner, profile, args)
}

pub(crate) fn local_wallet_command_controlled(
    profile: Value,
    args: Vec<String>,
    control: CommandControl,
) -> Result<LocalWalletCommandReport> {
    local_wallet_command_with_runner(&ControlledCliLocalWalletRunner::new(control), profile, args)
}

fn local_wallet_command_with_runner<R: LocalWalletRunner>(
    runner: &R,
    profile: Value,
    args: Vec<String>,
) -> Result<LocalWalletCommandReport> {
    let args = normalized_wallet_command_args(args)?;
    let wallet = resolve_local_wallet_command_profile(profile)?;
    let redactions = wallet.redactions();
    let output = runner.run(
        &wallet.wallet_binary,
        &wallet.wallet_home,
        LocalWalletInvocation::Args {
            args: &args,
            label: "wallet command",
        },
        &redactions,
    )?;
    let stdout = local_wallet_output_text(&output.stdout, &redactions);
    let stderr = local_wallet_output_text(&output.stderr, &redactions);

    Ok(LocalWalletCommandReport {
        source: "local_wallet_cli".to_owned(),
        status: "completed".to_owned(),
        command: wallet_command_label(&args),
        operation_detail: local_wallet_command_detail(
            "completed",
            &wallet_command_label(&args),
            extract_wallet_account_id(&format!("{stdout}\n{stderr}")).as_deref(),
            extract_wallet_tx_hash(&format!("{stdout}\n{stderr}")).as_deref(),
        ),
        wallet_home_source: wallet.wallet_home_source.clone(),
        submitted_at: unix_time_text(),
        exit_status: output.exit_status,
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

fn local_wallet_status_detail(status: &str, wallet_home_source: &str) -> String {
    if wallet_home_source.is_empty() {
        status.to_owned()
    } else {
        format!("{status}, home {wallet_home_source}")
    }
}

fn local_wallet_command_detail(
    status: &str,
    command: &str,
    account_id: Option<&str>,
    tx_hash: Option<&str>,
) -> String {
    if let Some(tx_hash) = tx_hash.filter(|value| !value.is_empty()) {
        return format!("tx {}", short_wallet_value(tx_hash));
    }
    if let Some(account_id) = account_id.filter(|value| !value.is_empty()) {
        return short_wallet_value(account_id);
    }
    if !command.is_empty() {
        return command.to_owned();
    }
    status.to_owned()
}

fn short_wallet_value(value: &str) -> String {
    let value = value.trim();
    if value.len() <= 18 {
        return value.to_owned();
    }
    format!("{}...{}", &value[..10], &value[value.len() - 6..])
}

pub(crate) async fn bedrock_wallet_balance(
    endpoint: &str,
    public_key: &str,
    tip: Option<&str>,
) -> Result<Value> {
    let public_key = normalize_bedrock_wallet_public_key(public_key)?;
    let tip = tip
        .map(str::trim)
        .filter(|tip| !tip.is_empty())
        .map(|tip| normalize_bedrock_hex_id(tip, "balance tip"))
        .transpose()?;
    bedrock_layer::wallet_balance(endpoint, &public_key, tip.as_deref()).await
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

#[cfg(test)]
fn local_wallet_profile_status_with_env(
    profile: Value,
    env_home: String,
) -> Result<LocalWalletProfileStatus> {
    let env_home_source = (!env_home.trim().is_empty()).then_some(LOCAL_WALLET_HOME_ENV);
    local_wallet_profile_status_with_runner(
        &CliLocalWalletRunner,
        profile,
        env_home,
        env_home_source,
    )
}

fn local_wallet_profile_status_with_runner<R: LocalWalletRunner>(
    runner: &R,
    profile: Value,
    env_home: String,
    env_home_source: Option<&'static str>,
) -> Result<LocalWalletProfileStatus> {
    let profile: LocalWalletProfileInput = parse_local_wallet_profile(profile)?;
    let wallet_binary = profile.wallet_binary.trim();
    let explicit_home = profile.wallet_home.trim();
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
    let mut readiness = local_wallet_readiness(&profile, &env_home);

    if wallet_binary.is_empty() && wallet_home.is_empty() {
        return Ok(LocalWalletProfileStatus {
            source: "local_wallet_cli".to_owned(),
            status: "unknown".to_owned(),
            checked_at: unix_time_text(),
            detail: format!("wallet binary and {LOCAL_WALLET_HOME_ENV} not configured"),
            version: None,
            home_source: home_source.to_owned(),
            network_profile: profile.network_profile.trim().to_owned(),
            readiness,
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
        readiness.command_ready = false;
        readiness.accounts_ready = false;
    } else if !readiness.wallet_storage_ready {
        details.push("wallet home missing storage.json".to_owned());
        status = local_wallet_worst_status(status, "degraded");
        readiness.command_ready = false;
        readiness.accounts_ready = false;
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
        match runner.version(wallet_binary, &wallet_home) {
            Ok(value) => {
                details.push("wallet binary responded".to_owned());
                version = (!value.is_empty()).then_some(value);
                if readiness.accounts_ready {
                    let mut redactions = vec![wallet_home.as_str()];
                    if local_wallet_binary_is_path_like(wallet_binary) {
                        redactions.push(wallet_binary);
                    }
                    if let Err(error) = runner.run(
                        wallet_binary,
                        &wallet_home,
                        LocalWalletInvocation::ProfileHomeProbe,
                        &redactions,
                    ) {
                        details.push(format!(
                            "wallet binary cannot read configured wallet home: {error:#}"
                        ));
                        status = local_wallet_worst_status(status, "down");
                        readiness.command_ready = false;
                        readiness.accounts_ready = false;
                    }
                }
            }
            Err(error) => {
                details.push(format!("wallet binary version check failed: {error:#}"));
                status = local_wallet_worst_status(status, "degraded");
                readiness.wallet_binary_ready = false;
                readiness.command_ready = false;
                readiness.accounts_ready = false;
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
        readiness,
    })
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

pub(crate) fn unix_time_text() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use std::cell::RefCell;

    use anyhow::{Result, bail};
    use serde_json::json;

    use super::runner::LocalWalletOutput;
    use super::*;

    #[cfg(unix)]
    #[test]
    fn wallet_accounts_require_storage_before_invoking_cli() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        let marker = directory.path().join("wallet-cli-invoked");
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            format!(
                "#!/bin/sh\ntouch '{}'\necho 'Public/test-account'\n",
                marker.display()
            ),
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;

        let error = local_wallet_accounts(json!({
            "wallet_binary": wallet_binary,
            "wallet_home": wallet_home,
            "network_profile": "testnet"
        }))
        .err()
        .context("account listing unexpectedly accepted missing wallet storage")?;

        anyhow::ensure!(
            format!("{error:#}") == "wallet home missing storage.json",
            "missing storage returned the wrong account-list error: {error:#}"
        );
        anyhow::ensure!(
            !marker.exists(),
            "account listing invoked the wallet CLI before validating storage"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wallet_account_create_requires_storage_before_invoking_cli() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        let marker = directory.path().join("wallet-cli-invoked");
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            format!(
                "#!/bin/sh\ntouch '{}'\necho 'Generated new account with account_id Public/test-account'\n",
                marker.display()
            ),
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;

        let error = local_wallet_create_account(
            json!({
                "wallet_binary": wallet_binary,
                "wallet_home": wallet_home,
                "network_profile": "testnet"
            }),
            "public",
            Some("test"),
        )
        .err()
        .context("account creation unexpectedly accepted missing wallet storage")?;

        anyhow::ensure!(
            format!("{error:#}") == "wallet home missing storage.json",
            "missing storage returned the wrong account-create error: {error:#}"
        );
        anyhow::ensure!(
            !marker.exists(),
            "account creation invoked the wallet CLI before validating storage"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wallet_advanced_command_requires_storage_before_invoking_cli() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        let marker = directory.path().join("wallet-cli-invoked");
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;

        let error = local_wallet_command(
            json!({
                "wallet_binary": wallet_binary,
                "wallet_home": wallet_home,
                "network_profile": "testnet"
            }),
            vec![
                "auth-transfer".to_owned(),
                "init".to_owned(),
                "--account-id".to_owned(),
                "Public/test-account".to_owned(),
            ],
        )
        .err()
        .context("advanced command unexpectedly accepted missing wallet storage")?;

        anyhow::ensure!(
            format!("{error:#}") == "wallet home missing storage.json",
            "missing storage returned the wrong advanced-command error: {error:#}"
        );
        anyhow::ensure!(
            !marker.exists(),
            "advanced command invoked the wallet CLI before validating storage"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wallet_send_requires_storage_before_invoking_cli() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        let marker = directory.path().join("wallet-cli-invoked");
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            format!("#!/bin/sh\ntouch '{}'\n", marker.display()),
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;

        let error = local_wallet_send_transaction(
            json!({
                "wallet_binary": wallet_binary,
                "wallet_home": wallet_home,
                "network_profile": "testnet"
            }),
            json!({
                "from": "Public/sender",
                "to": "Public/recipient",
                "amount": "1"
            }),
        )
        .err()
        .context("wallet send unexpectedly accepted missing wallet storage")?;

        anyhow::ensure!(
            format!("{error:#}") == "wallet home missing storage.json",
            "missing storage returned the wrong wallet-send error: {error:#}"
        );
        anyhow::ensure!(
            !marker.exists(),
            "wallet send invoked the wallet CLI before validating storage"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_wallet_accounts_reaps_cli_at_absolute_deadline() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        use crate::support::command_runner::{
            CommandStopReason, CommandTerminated, CommandTerminationScope,
        };

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        std::fs::write(wallet_home.join("storage.json"), b"{}")?;
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(&wallet_binary, b"#!/bin/sh\nwhile :; do :; done\n")?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;
        let deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_millis(250))
            .context("wallet test deadline overflow")?;

        let error = local_wallet_accounts_controlled(
            json!({
                "wallet_binary": wallet_binary,
                "wallet_home": wallet_home,
                "network_profile": "local"
            }),
            CommandControl::new(tokio_util::sync::CancellationToken::new(), deadline),
        )
        .err()
        .context("deadline-bound wallet command unexpectedly completed")?;
        let terminated = error
            .downcast_ref::<CommandTerminated>()
            .context("wallet deadline lost typed command termination")?;

        anyhow::ensure!(
            terminated.reason() == CommandStopReason::DeadlineExceeded
                && terminated.scope() == CommandTerminationScope::ProcessGroup,
            "wallet deadline returned wrong termination evidence: {terminated}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wallet_profile_and_accounts_bind_both_home_environment_contracts() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        std::fs::write(wallet_home.join("storage.json"), b"{}")?;
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            br#"#!/bin/sh
if [ -z "$LEE_WALLET_HOME_DIR" ] || [ "$LEE_WALLET_HOME_DIR" != "$NSSA_WALLET_HOME_DIR" ]; then
    echo "wallet home environment mismatch" >&2
    exit 9
fi
if [ ! -f "$LEE_WALLET_HOME_DIR/wallet_config.json" ]; then
    echo "wrong wallet home" >&2
    exit 10
fi
case "$*" in
    "--version")
        echo "wallet 0.1.0-test"
        ;;
    "account list")
        echo "Public/7wHg9sbJwc6h3NP1S9bekfAzB8CHifEcxKswCKUt3YQo"
        ;;
    "account list --long")
        echo "Public/7wHg9sbJwc6h3NP1S9bekfAzB8CHifEcxKswCKUt3YQo [main]"
        echo "  Account"
        echo "  {\"balance\":42}"
        ;;
    *)
        exit 11
        ;;
esac
"#,
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;
        let profile = json!({
            "wallet_binary": wallet_binary,
            "wallet_home": wallet_home,
            "network_profile": "testnet"
        });

        let status = local_wallet_profile_status(profile.clone())?;
        anyhow::ensure!(status.status == "ok", "wallet profile did not become ready");
        anyhow::ensure!(
            status.version.as_deref() == Some("wallet 0.1.0-test"),
            "wallet profile reported the wrong version"
        );
        anyhow::ensure!(
            status.readiness.command_ready,
            "wallet commands remained unavailable"
        );
        anyhow::ensure!(
            status.readiness.accounts_ready,
            "wallet accounts remained unavailable"
        );

        let report = local_wallet_accounts(profile)?;
        anyhow::ensure!(
            report.accounts.len() == 1,
            "unexpected wallet account count"
        );
        let account = report
            .accounts
            .first()
            .context("wallet account report was empty")?;
        anyhow::ensure!(
            account.label == "main",
            "wallet account label was not parsed"
        );
        anyhow::ensure!(
            account.state == "loaded",
            "wallet account state was not parsed"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wallet_profile_rejects_binary_that_cannot_read_configured_home() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        std::fs::write(wallet_home.join("storage.json"), b"{}")?;
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            br#"#!/bin/sh
if [ "$1" = "--version" ]; then
    echo "wallet 0.1.0-stale"
    exit 0
fi
echo "incompatible wallet storage schema at $LEE_WALLET_HOME_DIR/storage.json" >&2
exit 12
"#,
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;

        let status = local_wallet_profile_status(json!({
            "wallet_binary": wallet_binary,
            "wallet_home": wallet_home,
            "network_profile": "testnet"
        }))?;

        anyhow::ensure!(status.status == "down", "incompatible wallet stayed ready");
        anyhow::ensure!(
            status.version.as_deref() == Some("wallet 0.1.0-stale"),
            "wallet profile lost the detected version"
        );
        anyhow::ensure!(
            status.readiness.wallet_binary_ready,
            "wallet binary was not recognized"
        );
        anyhow::ensure!(
            !status.readiness.command_ready,
            "incompatible wallet commands remained enabled"
        );
        anyhow::ensure!(
            !status.readiness.accounts_ready,
            "incompatible wallet accounts remained enabled"
        );
        anyhow::ensure!(
            status
                .detail
                .contains("wallet binary cannot read configured wallet home"),
            "wallet profile did not explain the compatibility failure"
        );
        anyhow::ensure!(
            !status
                .detail
                .contains(&directory.path().display().to_string()),
            "wallet profile exposed the configured local path"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wallet_profile_requires_storage_before_enabling_accounts() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            b"#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'wallet 0.1.0-test'; exit 0; fi\nexit 13\n",
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;

        let status = local_wallet_profile_status(json!({
            "wallet_binary": wallet_binary,
            "wallet_home": wallet_home,
            "network_profile": "testnet"
        }))?;

        anyhow::ensure!(
            status.status == "degraded",
            "missing storage did not degrade the wallet profile"
        );
        anyhow::ensure!(
            !status.readiness.command_ready,
            "missing storage left wallet commands enabled"
        );
        anyhow::ensure!(
            !status.readiness.accounts_ready,
            "missing storage left wallet account listing enabled"
        );
        anyhow::ensure!(
            status.detail.contains("wallet home missing storage.json"),
            "missing storage detail was not reported"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn wallet_profile_does_not_probe_home_without_config() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("storage.json"), b"{}")?;
        let marker = directory.path().join("unexpected-probe");
        let wallet_binary = directory.path().join("wallet-test");
        std::fs::write(
            &wallet_binary,
            format!(
                "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo 'wallet 0.1.0-test'; exit 0; fi\ntouch '{}'\nexit 14\n",
                marker.display()
            ),
        )?;
        let mut permissions = std::fs::metadata(&wallet_binary)?.permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&wallet_binary, permissions)?;

        let status = local_wallet_profile_status(json!({
            "wallet_binary": wallet_binary,
            "wallet_home": wallet_home,
            "network_profile": "testnet"
        }))?;

        anyhow::ensure!(
            status.status == "degraded",
            "missing config did not degrade the wallet profile"
        );
        anyhow::ensure!(
            !status.readiness.command_ready && !status.readiness.accounts_ready,
            "missing config left wallet actions enabled"
        );
        anyhow::ensure!(
            status
                .detail
                .contains("wallet home missing wallet_config.json"),
            "missing config detail was not reported"
        );
        anyhow::ensure!(
            !marker.exists(),
            "profile check invoked the wallet against an incomplete home"
        );
        Ok(())
    }

    #[derive(Debug)]
    struct FakeRunner {
        output: LocalWalletOutput,
        expected_wallet_home: String,
    }

    impl LocalWalletRunner for FakeRunner {
        fn run(
            &self,
            binary: &str,
            wallet_home: &str,
            invocation: LocalWalletInvocation<'_>,
            _redactions: &[&str],
        ) -> Result<LocalWalletOutput> {
            if binary != "wallet" {
                bail!("unexpected wallet binary: {binary}");
            }
            if wallet_home != self.expected_wallet_home {
                bail!("unexpected wallet home: {wallet_home}");
            }
            match invocation {
                LocalWalletInvocation::Args { args, label } => {
                    if label != "wallet account new" {
                        bail!("unexpected label: {label}");
                    }
                    if args
                        != [
                            "account".to_owned(),
                            "new".to_owned(),
                            "private".to_owned(),
                            "--label".to_owned(),
                            "main".to_owned(),
                        ]
                    {
                        bail!("unexpected args: {args:?}");
                    }
                }
                _ => bail!("unexpected wallet invocation"),
            }
            Ok(self.output.clone())
        }
    }

    #[derive(Debug)]
    struct AccountsRunner {
        output: LocalWalletOutput,
    }

    impl LocalWalletRunner for AccountsRunner {
        fn run(
            &self,
            binary: &str,
            _wallet_home: &str,
            invocation: LocalWalletInvocation<'_>,
            _redactions: &[&str],
        ) -> Result<LocalWalletOutput> {
            if binary != "wallet" {
                bail!("unexpected wallet binary: {binary}");
            }
            if !matches!(invocation, LocalWalletInvocation::Accounts) {
                bail!("unexpected wallet invocation");
            }
            Ok(self.output.clone())
        }
    }

    #[derive(Debug)]
    struct WalletSendRunner {
        accounts_output: LocalWalletOutput,
        send_output: LocalWalletOutput,
        expected_wallet_home: String,
        expected_send_args: Vec<String>,
        invocations: RefCell<Vec<&'static str>>,
    }

    impl LocalWalletRunner for WalletSendRunner {
        fn run(
            &self,
            binary: &str,
            wallet_home: &str,
            invocation: LocalWalletInvocation<'_>,
            _redactions: &[&str],
        ) -> Result<LocalWalletOutput> {
            if binary != "wallet" {
                bail!("unexpected wallet binary: {binary}");
            }
            if wallet_home != self.expected_wallet_home {
                bail!("unexpected wallet home: {wallet_home}");
            }
            match invocation {
                LocalWalletInvocation::ProfileHomeProbe => {
                    self.invocations.borrow_mut().push("account list");
                    Ok(self.accounts_output.clone())
                }
                LocalWalletInvocation::Args { args, label } => {
                    self.invocations.borrow_mut().push("auth-transfer send");
                    if label != "wallet auth-transfer send" {
                        bail!("unexpected label: {label}");
                    }
                    if args != self.expected_send_args {
                        bail!("unexpected args: {args:?}");
                    }
                    Ok(self.send_output.clone())
                }
                _ => bail!("unexpected wallet invocation"),
            }
        }
    }

    #[test]
    fn local_wallet_accounts_accepts_complete_output_larger_than_command_detail_limit() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        std::fs::write(directory.path().join("wallet_config.json"), b"{}")?;
        std::fs::write(directory.path().join("storage.json"), b"{}")?;
        let padding = "x".repeat(180);
        let stdout = (0..49)
            .map(|index| {
                format!(
                    "m/{index} Public/TestAccount{index} [asset-{index}]\n  Regular account\n  {{\"balance\":{index},\"padding\":\"{padding}\"}}\n"
                )
            })
            .collect::<String>();
        anyhow::ensure!(
            stdout.len() > LOCAL_WALLET_OUTPUT_LIMIT,
            "account-list fixture did not exceed the command-detail limit"
        );
        let runner = AccountsRunner {
            output: LocalWalletOutput {
                exit_status: "exit status: 0".to_owned(),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
            },
        };

        let report = local_wallet_accounts_with_runner(
            &runner,
            json!({
                "wallet_binary": "wallet",
                "wallet_home": directory.path(),
                "network_profile": "testnet"
            }),
        )?;

        anyhow::ensure!(report.accounts.len() == 49, "account rows were truncated");
        let last = report.accounts.last().context("last account row missing")?;
        anyhow::ensure!(
            last.typed_id == "Public/TestAccount48" && last.label == "asset-48",
            "last account row was not preserved: {last:?}"
        );
        Ok(())
    }

    #[test]
    fn local_wallet_accounts_rejects_output_above_dedicated_limit() -> Result<()> {
        let directory = tempfile::tempdir()?;
        std::fs::write(directory.path().join("wallet_config.json"), b"{}")?;
        std::fs::write(directory.path().join("storage.json"), b"{}")?;
        let runner = AccountsRunner {
            output: LocalWalletOutput {
                exit_status: "exit status: 0".to_owned(),
                stdout: vec![b'x'; LOCAL_WALLET_ACCOUNTS_OUTPUT_LIMIT + 1],
                stderr: Vec::new(),
            },
        };

        let error = local_wallet_accounts_with_runner(
            &runner,
            json!({
                "wallet_binary": "wallet",
                "wallet_home": directory.path(),
                "network_profile": "testnet"
            }),
        )
        .err()
        .context("oversized account output unexpectedly succeeded")?;

        anyhow::ensure!(
            format!("{error:#}")
                == "wallet account list output exceeded 1048576 bytes; refusing to parse partial account data",
            "oversized account output returned the wrong error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn local_wallet_profile_status_reports_missing_binary_without_path_leak() {
        let status = local_wallet_profile_status(json!({
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
        assert!(!status.readiness.wallet_binary_ready);
        assert!(!status.readiness.command_ready);
        assert!(!status.detail.contains("/definitely/not/a/logos/wallet"));
    }

    #[test]
    fn local_wallet_profile_status_keeps_blank_wallet_home_unconfigured() {
        let status = local_wallet_profile_status_with_env(
            json!({
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
        assert!(!status.readiness.wallet_home_ready);
        assert!(!status.readiness.command_ready);
        assert!(
            status
                .detail
                .contains("wallet home directory not configured")
        );
    }

    #[test]
    fn local_wallet_profile_status_accepts_lee_wallet_home_env() {
        let status = local_wallet_profile_status_with_env(
            json!({
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
        assert!(status.readiness.wallet_home_ready);
        assert!(!status.readiness.accounts_ready);
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
            json!({
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
    fn local_wallet_create_account_uses_runner_boundary() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        std::fs::write(wallet_home.join("storage.json"), b"{}")?;
        let runner = FakeRunner {
            output: LocalWalletOutput {
                exit_status: "exit status: 0".to_owned(),
                stdout: b"Generated new account with account_id Private/abc123".to_vec(),
                stderr: Vec::new(),
            },
            expected_wallet_home: wallet_home.display().to_string(),
        };

        let report = local_wallet_create_account_with_runner(
            &runner,
            json!({
                "wallet_binary": "wallet",
                "wallet_home": wallet_home,
                "network_profile": "local"
            }),
            "private",
            Some("main"),
        )?;

        if report.status != "created" {
            bail!("unexpected status: {}", report.status);
        }
        if report.command != "wallet account new private" {
            bail!("unexpected command: {}", report.command);
        }
        if report.account_id.as_deref() != Some("Private/abc123") {
            bail!("unexpected account id: {:?}", report.account_id);
        }
        if report.exit_status != "exit status: 0" {
            bail!("unexpected exit status: {}", report.exit_status);
        }
        Ok(())
    }

    #[test]
    fn local_wallet_send_resolves_an_owned_public_label_before_mutation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        std::fs::write(wallet_home.join("storage.json"), b"{}")?;
        let runner = WalletSendRunner {
            accounts_output: LocalWalletOutput {
                exit_status: "exit status: 0".to_owned(),
                stdout: b"m/0 Public/owned-sender [primary]\n".to_vec(),
                stderr: Vec::new(),
            },
            send_output: LocalWalletOutput {
                exit_status: "exit status: 0".to_owned(),
                stdout: b"Transaction hash is submitted-tx".to_vec(),
                stderr: Vec::new(),
            },
            expected_wallet_home: wallet_home.display().to_string(),
            expected_send_args: vec![
                "auth-transfer".to_owned(),
                "send".to_owned(),
                "--from".to_owned(),
                "Public/owned-sender".to_owned(),
                "--amount".to_owned(),
                "1".to_owned(),
                "--to".to_owned(),
                "Public/recipient".to_owned(),
            ],
            invocations: RefCell::new(Vec::new()),
        };

        let report = local_wallet_send_transaction_with_runner(
            &runner,
            json!({
                "wallet_binary": "wallet",
                "wallet_home": wallet_home,
                "network_profile": "testnet"
            }),
            json!({
                "from": "primary",
                "to": "Public/recipient",
                "amount": "1"
            }),
        )?;

        assert_eq!(report.from.as_deref(), Some("Public/owned-sender"));
        assert_eq!(report.tx_hash.as_deref(), Some("submitted-tx"));
        assert_eq!(
            runner.invocations.into_inner(),
            vec!["account list", "auth-transfer send"]
        );
        Ok(())
    }

    #[test]
    fn local_wallet_send_rejects_unowned_sender_before_mutation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let wallet_home = directory.path().join("wallet-home");
        std::fs::create_dir(&wallet_home)?;
        std::fs::write(wallet_home.join("wallet_config.json"), b"{}")?;
        std::fs::write(wallet_home.join("storage.json"), b"{}")?;
        let runner = WalletSendRunner {
            accounts_output: LocalWalletOutput {
                exit_status: "exit status: 0".to_owned(),
                stdout: b"m/0 Public/owned-sender [primary]\n".to_vec(),
                stderr: Vec::new(),
            },
            send_output: LocalWalletOutput {
                exit_status: "exit status: 0".to_owned(),
                stdout: Vec::new(),
                stderr: Vec::new(),
            },
            expected_wallet_home: wallet_home.display().to_string(),
            expected_send_args: Vec::new(),
            invocations: RefCell::new(Vec::new()),
        };

        let error = local_wallet_send_transaction_with_runner(
            &runner,
            json!({
                "wallet_binary": "wallet",
                "wallet_home": wallet_home,
                "network_profile": "testnet"
            }),
            json!({
                "from": "Public/unowned-sender",
                "to": "Public/recipient",
                "amount": "1"
            }),
        )
        .err()
        .context("unowned wallet sender unexpectedly submitted")?;

        assert_eq!(
            format!("{error:#}"),
            "sender must be an owned Public wallet account"
        );
        assert_eq!(runner.invocations.into_inner(), vec!["account list"]);
        Ok(())
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
