use std::{
    env,
    path::Path,
    process::{Command, Output},
    time::Duration,
};

use anyhow::Result;

use super::profile::local_wallet_binary_is_path_like;
use super::{
    LOCAL_WALLET_DEPLOY_TIMEOUT, LOCAL_WALLET_ENV_ALLOWLIST, LOCAL_WALLET_HOME_ENV,
    LOCAL_WALLET_LIST_TIMEOUT, LOCAL_WALLET_MUTATION_TIMEOUT, LOCAL_WALLET_OUTPUT_LIMIT,
    LOCAL_WALLET_POLL_INTERVAL, LOCAL_WALLET_SYNC_TIMEOUT, LOCAL_WALLET_VERSION_TIMEOUT,
};
use crate::support::command_runner::{CommandRunPolicy, output_text, run_command};

pub(super) enum LocalWalletInvocation<'a> {
    Version,
    DeployProgram { program_path: &'a Path },
    SyncPrivate,
    Accounts,
    Args { args: &'a [String], label: &'a str },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LocalWalletOutput {
    pub(super) exit_status: String,
    pub(super) stdout: Vec<u8>,
    pub(super) stderr: Vec<u8>,
}

pub(super) trait LocalWalletRunner {
    fn run(
        &self,
        binary: &str,
        wallet_home: &str,
        invocation: LocalWalletInvocation<'_>,
        redactions: &[&str],
    ) -> Result<LocalWalletOutput>;

    fn version(&self, binary: &str, wallet_home: &str) -> Result<String> {
        let mut redactions = Vec::new();
        if local_wallet_binary_is_path_like(binary) {
            redactions.push(binary);
        }
        if !wallet_home.trim().is_empty() {
            redactions.push(wallet_home);
        }
        let output = self.run(
            binary,
            wallet_home,
            LocalWalletInvocation::Version,
            &redactions,
        )?;
        let text = if output.stdout.is_empty() {
            local_wallet_output_text(&output.stderr, &redactions)
        } else {
            local_wallet_output_text(&output.stdout, &redactions)
        };
        Ok(text
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .chars()
            .take(160)
            .collect())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct CliLocalWalletRunner;

impl LocalWalletRunner for CliLocalWalletRunner {
    fn run(
        &self,
        binary: &str,
        wallet_home: &str,
        invocation: LocalWalletInvocation<'_>,
        redactions: &[&str],
    ) -> Result<LocalWalletOutput> {
        let mut command = Command::new(binary);
        configure_local_wallet_command(&mut command, wallet_home);
        let (label, timeout) = configure_invocation(&mut command, invocation);
        run_local_wallet_command(command, label, timeout, redactions)
    }
}

pub(super) fn local_wallet_output_text(output: &[u8], redactions: &[&str]) -> String {
    output_text(output, redactions, LOCAL_WALLET_OUTPUT_LIMIT)
}

fn configure_invocation<'a>(
    command: &mut Command,
    invocation: LocalWalletInvocation<'a>,
) -> (&'a str, Duration) {
    match invocation {
        LocalWalletInvocation::Version => {
            command.arg("--version");
            ("wallet --version", LOCAL_WALLET_VERSION_TIMEOUT)
        }
        LocalWalletInvocation::DeployProgram { program_path } => {
            command.arg("deploy-program").arg(program_path);
            ("wallet deploy-program", LOCAL_WALLET_DEPLOY_TIMEOUT)
        }
        LocalWalletInvocation::SyncPrivate => {
            command.arg("account").arg("sync-private");
            ("wallet account sync-private", LOCAL_WALLET_SYNC_TIMEOUT)
        }
        LocalWalletInvocation::Accounts => {
            command.arg("account").arg("list").arg("--long");
            ("wallet account list --long", LOCAL_WALLET_LIST_TIMEOUT)
        }
        LocalWalletInvocation::Args { args, label } => {
            command.args(args);
            (label, LOCAL_WALLET_MUTATION_TIMEOUT)
        }
    }
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
) -> Result<LocalWalletOutput> {
    let output = run_command(
        command,
        CommandRunPolicy {
            label,
            timeout,
            poll_interval: LOCAL_WALLET_POLL_INTERVAL,
            redactions,
            output_limit: LOCAL_WALLET_OUTPUT_LIMIT,
        },
    )?;
    Ok(normalize_output(output))
}

fn normalize_output(output: Output) -> LocalWalletOutput {
    LocalWalletOutput {
        exit_status: output.status.to_string(),
        stdout: output.stdout,
        stderr: output.stderr,
    }
}
