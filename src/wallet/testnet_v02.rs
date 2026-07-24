use std::{
    env,
    ffi::OsString,
    io::Write as _,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result, bail};
use logos_inspector_testnet_v02_helper_protocol::{
    HELPER_MODE, HelperRequest, HelperResponse, HelperSuccess,
};

pub(super) use logos_inspector_testnet_v02_helper_protocol::{
    HelperAccount, HelperAccountPrivacy, SubmitPrivateIdlRequest,
};

use super::{LOCAL_WALLET_MUTATION_TIMEOUT, LOCAL_WALLET_OUTPUT_LIMIT};
use crate::support::command_runner::{
    CommandRunPolicy, DEFAULT_COMMAND_CAPTURE_LIMIT, output_text, run_command,
};

const PACKAGED_HELPER_ENV: &str = "LOGOS_INSPECTOR_TESTNET_V02_HELPER";
const PACKAGED_HELPER_NAME: &str = "logos-inspector-testnet-v02-helper";

#[cfg(all(debug_assertions, feature = "development-testnet-v02-helper"))]
const DEVELOPMENT_HELPER_MANIFEST: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/crates/testnet-v02-helper/Cargo.toml"
);

pub(super) async fn submit_private_idl(
    request: SubmitPrivateIdlRequest,
) -> Result<(String, usize)> {
    let profile_request = HelperRequest::CheckProfile {
        wallet_home: request.wallet_home.clone(),
        sequencer_endpoint: request.sequencer_endpoint.clone(),
    };
    let profile = invoke_helper_async(profile_request).await?;
    match profile {
        HelperResponse::Success {
            result: HelperSuccess::Profile { .. },
        } => {}
        HelperResponse::Success {
            result: HelperSuccess::Submitted { .. },
        } => bail!("private Testnet helper returned an unexpected submission response"),
        HelperResponse::Failure { error } => {
            bail!("private Testnet helper rejected the Sequencer profile: {error}");
        }
    }

    invoke_helper_async(HelperRequest::SubmitPrivateIdl(request))
        .await
        .and_then(|response| match response {
            HelperResponse::Success {
                result:
                    HelperSuccess::Submitted {
                        tx_hash,
                        shared_secret_count,
                    },
            } => Ok((tx_hash, shared_secret_count)),
            HelperResponse::Success {
                result: HelperSuccess::Profile { .. },
            } => bail!("private Testnet helper returned an unexpected profile response"),
            HelperResponse::Failure { error } => {
                bail!("private Testnet helper rejected the instruction: {error}");
            }
        })
}

async fn invoke_helper_async(request: HelperRequest) -> Result<HelperResponse> {
    tokio::task::spawn_blocking(move || invoke_helper(request))
        .await
        .context("private Testnet helper task failed")?
}

fn invoke_helper(request: HelperRequest) -> Result<HelperResponse> {
    let wallet_home = request_wallet_home(&request);
    let wallet_home_text = wallet_home.to_string_lossy().into_owned();
    let mut request_file = tempfile::NamedTempFile::new()
        .context("failed to create private Testnet helper request")?;
    serde_json::to_writer(&mut request_file, &request)
        .context("failed to encode private Testnet helper request")?;
    request_file
        .flush()
        .context("failed to flush private Testnet helper request")?;

    let helper = resolve_helper_command()?;
    let mut command = Command::new(&helper.program);
    command.args(&helper.args);
    command
        .arg(HELPER_MODE)
        .arg("--request")
        .arg(request_file.path());
    let redactions = [wallet_home_text.as_str()];
    let output = run_command(
        command,
        CommandRunPolicy {
            label: "Testnet v0.2 private wallet helper",
            timeout: LOCAL_WALLET_MUTATION_TIMEOUT,
            poll_interval: super::LOCAL_WALLET_POLL_INTERVAL,
            redactions: &redactions,
            output_limit: LOCAL_WALLET_OUTPUT_LIMIT,
            capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
        },
    )?;
    parse_helper_response(&output.stdout, &redactions)
}

fn request_wallet_home(request: &HelperRequest) -> &Path {
    match request {
        HelperRequest::CheckProfile { wallet_home, .. }
        | HelperRequest::SubmitPrivateIdl(SubmitPrivateIdlRequest { wallet_home, .. }) => {
            wallet_home
        }
    }
}

struct HelperCommand {
    program: PathBuf,
    args: Vec<OsString>,
}

fn resolve_helper_command() -> Result<HelperCommand> {
    if let Some(path) = env::var_os(PACKAGED_HELPER_ENV).map(PathBuf::from) {
        if path.is_file() {
            return Ok(HelperCommand {
                program: path,
                args: Vec::new(),
            });
        }
        bail!("configured private Testnet helper is unavailable");
    }

    let compiled_helper = option_env!("LOGOS_INSPECTOR_TESTNET_V02_HELPER").map(PathBuf::from);
    if let Some(path) = compiled_helper.as_ref()
        && path.is_file()
    {
        return Ok(HelperCommand {
            program: path.clone(),
            args: Vec::new(),
        });
    }

    if let Some(path) = env::current_exe()
        .ok()
        .as_deref()
        .and_then(installed_helper_path)
        .filter(|path| path.is_file())
    {
        return Ok(HelperCommand {
            program: path,
            args: Vec::new(),
        });
    }

    if compiled_helper.is_some() {
        bail!("packaged private Testnet helper is unavailable");
    }

    #[cfg(all(debug_assertions, feature = "development-testnet-v02-helper"))]
    if let Some(command) = development_helper_command() {
        return Ok(command);
    }

    bail!(
        "private instruction targets a different circuit profile, but no source-attested Testnet helper is installed"
    );
}

fn installed_helper_path(current_exe: &Path) -> Option<PathBuf> {
    let bin_dir = current_exe.parent()?;
    let prefix = bin_dir.parent()?;
    Some(
        prefix
            .join("libexec")
            .join(format!("{PACKAGED_HELPER_NAME}{}", env::consts::EXE_SUFFIX)),
    )
}

#[cfg(all(debug_assertions, feature = "development-testnet-v02-helper"))]
fn development_helper_command() -> Option<HelperCommand> {
    let cargo = option_env!("CARGO")?;
    let cargo = PathBuf::from(cargo);
    let manifest = PathBuf::from(DEVELOPMENT_HELPER_MANIFEST);
    if !cargo.is_file() || !manifest.is_file() {
        return None;
    }
    Some(HelperCommand {
        program: cargo,
        args: vec![
            OsString::from("run"),
            OsString::from("--quiet"),
            OsString::from("--locked"),
            OsString::from("--manifest-path"),
            manifest.into_os_string(),
            OsString::from("--"),
        ],
    })
}

fn parse_helper_response(output: &[u8], redactions: &[&str]) -> Result<HelperResponse> {
    let text = String::from_utf8_lossy(output);
    for line in text.lines().rev() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Ok(response) = serde_json::from_str(line) {
            return Ok(response);
        }
    }
    let detail = output_text(output, redactions, LOCAL_WALLET_OUTPUT_LIMIT);
    bail!("private Testnet helper did not return a valid response: {detail}");
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::*;

    #[test]
    fn parses_terminal_response_after_wallet_output() -> Result<()> {
        let response = HelperResponse::Success {
            result: HelperSuccess::Profile {
                protocol: "testnet_v0_2".to_owned(),
            },
        };
        let encoded = serde_json::to_string(&response)?;
        let output = format!("legacy wallet log\n{encoded}\n");
        let parsed = parse_helper_response(output.as_bytes(), &[])?;
        let HelperResponse::Success {
            result: HelperSuccess::Profile { protocol },
        } = parsed
        else {
            bail!("unexpected helper response");
        };
        if protocol != "testnet_v0_2" {
            bail!("unexpected helper profile: {protocol}");
        }
        Ok(())
    }

    #[test]
    fn request_keeps_private_account_bytes_without_text_conversion() -> Result<()> {
        let request = HelperRequest::SubmitPrivateIdl(SubmitPrivateIdlRequest {
            wallet_home: PathBuf::from("/wallet"),
            sequencer_endpoint: Some("https://sequencer.example.test".to_owned()),
            expected_program_id: [7; 8],
            program_binary: PathBuf::from("/program.bin"),
            dependency_binaries: Vec::new(),
            accounts: vec![HelperAccount {
                account_id: [9; 32],
                privacy: HelperAccountPrivacy::Private,
                signer: false,
            }],
            instruction_words: vec![1, 2],
        });
        let decoded: HelperRequest = serde_json::from_value(serde_json::to_value(request)?)?;
        let HelperRequest::SubmitPrivateIdl(decoded) = decoded else {
            bail!("unexpected helper request");
        };
        let Some(account) = decoded.accounts.first() else {
            bail!("private account was removed across helper protocol");
        };
        if account.account_id != [9; 32]
            || !matches!(account.privacy, HelperAccountPrivacy::Private)
        {
            bail!("private account bytes changed across helper protocol");
        }
        Ok(())
    }

    #[test]
    fn packaged_helper_uses_installed_layout() -> Result<()> {
        let current_exe = Path::new("/opt/logos-inspector/bin/logos-inspector-standalone-gui");
        let Some(helper) = installed_helper_path(current_exe) else {
            bail!("installed helper path did not resolve");
        };
        if helper
            != Path::new("/opt/logos-inspector/libexec")
                .join(format!("{PACKAGED_HELPER_NAME}{}", env::consts::EXE_SUFFIX))
        {
            bail!("unexpected installed helper path: {}", helper.display());
        }
        Ok(())
    }

    #[cfg(all(debug_assertions, feature = "development-testnet-v02-helper"))]
    #[test]
    fn development_helper_uses_the_pinned_manifest() -> Result<()> {
        let Some(command) = development_helper_command() else {
            bail!("development helper command was not available");
        };
        if !command
            .args
            .iter()
            .any(|argument| argument == DEVELOPMENT_HELPER_MANIFEST)
        {
            bail!("development helper command did not use the pinned manifest");
        }
        Ok(())
    }

    #[cfg(all(debug_assertions, feature = "development-testnet-v02-helper"))]
    #[test]
    #[ignore = "builds and starts the isolated legacy helper"]
    fn development_helper_executes_as_a_separate_process() -> Result<()> {
        let wallet = tempfile::tempdir()?;
        let response = invoke_helper(HelperRequest::CheckProfile {
            wallet_home: wallet.path().to_path_buf(),
            sequencer_endpoint: None,
        })?;
        let HelperResponse::Failure { error } = response else {
            bail!("development helper unexpectedly accepted an empty wallet");
        };
        if !error.contains("failed to open local wallet state") {
            bail!("development helper returned an unexpected failure: {error}");
        }
        Ok(())
    }
}
