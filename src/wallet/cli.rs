use std::{
    env,
    path::Path,
    process::{Command, Output},
    time::Duration,
};

use anyhow::Result;

use super::{
    LOCAL_WALLET_DEPLOY_TIMEOUT, LOCAL_WALLET_ENV_ALLOWLIST, LOCAL_WALLET_HOME_ENV,
    LOCAL_WALLET_LIST_TIMEOUT, LOCAL_WALLET_OUTPUT_LIMIT, LOCAL_WALLET_POLL_INTERVAL,
    LOCAL_WALLET_SYNC_TIMEOUT, LOCAL_WALLET_VERSION_TIMEOUT, local_wallet_binary_is_path_like,
};
use crate::command_runner::{CommandRunPolicy, output_text, run_command};

pub(super) fn local_wallet_binary_version(binary: &str, wallet_home: &str) -> Result<String> {
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

pub(super) fn local_wallet_deploy_program_output(
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

pub(super) fn local_wallet_sync_private_output(
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

pub(super) fn local_wallet_args_output(
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

pub(super) fn local_wallet_accounts_output(
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

pub(super) fn local_wallet_output_text(output: &[u8], redactions: &[&str]) -> String {
    output_text(output, redactions, LOCAL_WALLET_OUTPUT_LIMIT)
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
    command: Command,
    label: &str,
    timeout: Duration,
    redactions: &[&str],
) -> Result<Output> {
    run_command(
        command,
        CommandRunPolicy {
            label,
            timeout,
            poll_interval: LOCAL_WALLET_POLL_INTERVAL,
            redactions,
            output_limit: LOCAL_WALLET_OUTPUT_LIMIT,
        },
    )
}
