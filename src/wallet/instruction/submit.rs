use std::{
    collections::HashMap,
    fs::{self, File},
    io::BufReader,
    path::{Path, PathBuf},
};

use ::wallet::{
    AccountIdentity, WalletCore,
    config::{WalletConfig, WalletConfigOverrides},
};
use anyhow::{Context as _, Result, bail};
use common::config::BasicAuth;
use lee::program::Program;
use url::Url;

use super::{
    LocalWalletInstructionRequest,
    model::{AccountPrivacy, InstructionMode, PreparedAccount, PreparedInstruction},
};

pub(super) async fn submit_instruction(
    wallet_home: PathBuf,
    request: &LocalWalletInstructionRequest,
    prepared: &PreparedInstruction,
    sequencer_endpoint: Option<&str>,
) -> Result<(String, Option<usize>)> {
    let config_path = wallet_home.join("wallet_config.json");
    let storage_path = wallet_home.join("storage.json");
    let targeted_config = sequencer_endpoint
        .map(|endpoint| targeted_wallet_config(&config_path, endpoint))
        .transpose()?;
    let config_overrides = targeted_config
        .as_ref()
        .map(|targeted| targeted.overrides.clone());
    let wallet = WalletCore::new_update_chain(config_path, storage_path, config_overrides)
        .context("failed to open local wallet state")?;
    if let Some(targeted) = &targeted_config {
        verify_effective_basic_auth(
            wallet.config().basic_auth.as_ref(),
            targeted.expected_basic_auth.as_ref(),
        )?;
    }

    match prepared.mode {
        InstructionMode::Public => {
            let accounts = prepared
                .accounts
                .iter()
                .map(public_account_identity)
                .collect::<Vec<_>>();
            let tx_hash = wallet
                .send_pub_tx(
                    accounts,
                    prepared.instruction_words.clone(),
                    prepared.program_id,
                )
                .await
                .map_err(|error| anyhow::anyhow!("failed to submit public transaction: {error}"))?;
            Ok((tx_hash.to_string(), None))
        }
        InstructionMode::Private => {
            let program = load_program_with_dependencies(
                Path::new(&prepared.program_binary),
                &request.dependency_binaries,
            )?;
            let accounts = prepared
                .accounts
                .iter()
                .map(private_account_identity)
                .collect::<Vec<_>>();
            let (tx_hash, shared_secrets) = wallet
                .send_privacy_preserving_tx(accounts, prepared.instruction_words.clone(), &program)
                .await
                .map_err(|error| {
                    anyhow::anyhow!("failed to submit privacy-preserving transaction: {error}")
                })?;
            Ok((tx_hash.to_string(), Some(shared_secrets.len())))
        }
    }
}

struct TargetedWalletConfig {
    overrides: WalletConfigOverrides,
    expected_basic_auth: Option<BasicAuth>,
}

fn targeted_wallet_config(
    config_path: &Path,
    sequencer_endpoint: &str,
) -> Result<TargetedWalletConfig> {
    let target = validated_sequencer_endpoint(sequencer_endpoint)?;
    let file = File::open(config_path).context("failed to open local wallet configuration")?;
    let config: WalletConfig = serde_json::from_reader(BufReader::new(file))
        .context("failed to parse local wallet configuration")?;
    let same_origin = config.sequencer_addr.origin() == target.origin();
    let overrides = config_overrides_for_target(&config, target);
    Ok(TargetedWalletConfig {
        overrides,
        expected_basic_auth: if same_origin {
            config.basic_auth.clone()
        } else {
            None
        },
    })
}

fn verify_effective_basic_auth(
    effective: Option<&BasicAuth>,
    expected: Option<&BasicAuth>,
) -> Result<()> {
    let matches = match (effective, expected) {
        (None, None) => true,
        (Some(effective), Some(expected)) => {
            effective.username == expected.username && effective.password == expected.password
        }
        _ => false,
    };
    if !matches {
        bail!("wallet configuration changed while binding the Sequencer target");
    }
    Ok(())
}

fn config_overrides_for_target(config: &WalletConfig, target: Url) -> WalletConfigOverrides {
    let basic_auth = if config.sequencer_addr.origin() == target.origin() {
        None
    } else {
        Some(None)
    };

    WalletConfigOverrides {
        sequencer_addr: Some(target),
        basic_auth,
        ..WalletConfigOverrides::default()
    }
}

fn validated_sequencer_endpoint(value: &str) -> Result<Url> {
    let endpoint = Url::parse(value).context("verified Sequencer endpoint is not a valid URL")?;
    if endpoint.cannot_be_a_base() {
        bail!("verified Sequencer endpoint must be a hierarchical URL");
    }
    if endpoint.scheme() != "http" && endpoint.scheme() != "https" {
        bail!("verified Sequencer endpoint scheme must be http or https");
    }
    if endpoint.host().is_none() {
        bail!("verified Sequencer endpoint must include a host");
    }
    if !endpoint.username().is_empty() || endpoint.password().is_some() {
        bail!("verified Sequencer endpoint cannot contain authentication");
    }
    if endpoint.query().is_some() || endpoint.fragment().is_some() {
        bail!("verified Sequencer endpoint cannot contain a query or fragment");
    }
    Ok(endpoint)
}

fn public_account_identity(account: &PreparedAccount) -> AccountIdentity {
    if account.signer {
        AccountIdentity::Public(account.account_id)
    } else {
        AccountIdentity::PublicNoSign(account.account_id)
    }
}

fn private_account_identity(account: &PreparedAccount) -> AccountIdentity {
    match account.privacy {
        AccountPrivacy::Private => AccountIdentity::PrivateOwned(account.account_id),
        AccountPrivacy::Public if account.signer => AccountIdentity::Public(account.account_id),
        AccountPrivacy::Public => AccountIdentity::PublicNoSign(account.account_id),
    }
}

fn load_program_with_dependencies(
    program_path: &Path,
    dependency_paths: &[String],
) -> Result<lee::privacy_preserving_transaction::circuit::ProgramWithDependencies> {
    let program = load_program(program_path)?;
    let mut dependencies = HashMap::new();
    for path in dependency_paths {
        let dependency = load_program(Path::new(path))?;
        dependencies.insert(dependency.id(), dependency);
    }
    Ok(
        lee::privacy_preserving_transaction::circuit::ProgramWithDependencies::new(
            program,
            dependencies,
        ),
    )
}

fn load_program(path: &Path) -> Result<Program> {
    let bytes = fs::read(path)
        .with_context(|| format!("failed to read program binary at {}", path.display()))?;
    Program::new(bytes.into())
        .map_err(|error| anyhow::anyhow!("failed to parse program binary: {error:?}"))
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use tempfile::tempdir;

    use super::*;

    fn config_with_auth(endpoint: &str) -> Result<WalletConfig> {
        Ok(WalletConfig {
            sequencer_addr: Url::parse(endpoint)?,
            basic_auth: Some(BasicAuth {
                username: "wallet-user".to_owned(),
                password: Some("test-password".to_owned()),
            }),
            ..WalletConfig::default()
        })
    }

    fn write_config(path: &Path, config: &WalletConfig) -> Result<()> {
        let file = File::create(path)?;
        serde_json::to_writer(file, config)?;
        Ok(())
    }

    #[test]
    fn target_override_preserves_basic_auth_for_same_origin() -> Result<()> {
        let mut config = config_with_auth("https://sequencer.example.test/api")?;
        let target = validated_sequencer_endpoint("https://sequencer.example.test/rpc")?;
        let overrides = config_overrides_for_target(&config, target);

        if overrides.basic_auth.is_some() {
            bail!("same-origin target unexpectedly overrides basic authentication");
        }
        config.apply_overrides(overrides);
        let auth = config
            .basic_auth
            .as_ref()
            .context("same-origin target cleared basic authentication")?;
        if auth.username != "wallet-user" || auth.password.as_deref() != Some("test-password") {
            bail!("same-origin target changed basic authentication");
        }
        if config.sequencer_addr.as_str() != "https://sequencer.example.test/rpc" {
            bail!("same-origin target was not applied");
        }
        Ok(())
    }

    #[test]
    fn target_override_clears_basic_auth_for_different_origin() -> Result<()> {
        let mut config = config_with_auth("https://sequencer.example.test/api")?;
        let target = validated_sequencer_endpoint("https://other-sequencer.example.test/rpc")?;
        let overrides = config_overrides_for_target(&config, target);

        if !matches!(overrides.basic_auth, Some(None)) {
            bail!("cross-origin target did not explicitly clear basic authentication");
        }
        config.apply_overrides(overrides);
        if config.basic_auth.is_some() {
            bail!("cross-origin target retained basic authentication");
        }
        if config.sequencer_addr.as_str() != "https://other-sequencer.example.test/rpc" {
            bail!("cross-origin target was not applied");
        }
        Ok(())
    }

    #[test]
    fn target_override_rejects_invalid_url_without_exposing_input() -> Result<()> {
        let directory = tempdir()?;
        let config_path = directory.path().join("wallet_config.json");
        let config = config_with_auth("https://sequencer.example.test")?;
        write_config(&config_path, &config)?;
        let before = fs::read(&config_path)?;
        let invalid_target = "private-value-not-a-url";
        let result = targeted_wallet_config(&config_path, invalid_target);

        if result.is_ok() {
            bail!("invalid target was accepted");
        }
        let error = result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        if !error.contains("not a valid URL") {
            bail!("unexpected invalid-target error: {error}");
        }
        if error.contains(invalid_target) || error.contains("test-password") {
            bail!("invalid-target error exposed sensitive input");
        }
        if fs::read(&config_path)? != before {
            bail!("invalid target changed wallet configuration file");
        }
        Ok(())
    }

    #[test]
    fn targeted_config_rejects_reloaded_credentials() -> Result<()> {
        let directory = tempdir()?;
        let config_path = directory.path().join("wallet_config.json");
        let original = config_with_auth("https://sequencer.example.test/api")?;
        write_config(&config_path, &original)?;

        let targeted = targeted_wallet_config(&config_path, "https://sequencer.example.test/rpc")?;

        let mut replacement = config_with_auth("https://other-sequencer.example.test/api")?;
        let replacement_auth = replacement
            .basic_auth
            .as_mut()
            .context("replacement config has no basic authentication")?;
        replacement_auth.username = "replacement-user".to_owned();
        replacement_auth.password = Some("replacement-password".to_owned());
        write_config(&config_path, &replacement)?;
        let replacement_file = File::open(&config_path)?;
        let mut reloaded: WalletConfig = serde_json::from_reader(BufReader::new(replacement_file))?;
        reloaded.apply_overrides(targeted.overrides);

        let result = verify_effective_basic_auth(
            reloaded.basic_auth.as_ref(),
            targeted.expected_basic_auth.as_ref(),
        );
        if result.is_ok() {
            bail!("reloaded credentials were accepted after origin policy decision");
        }
        let error = result
            .err()
            .map(|error| error.to_string())
            .unwrap_or_default();
        if !error.contains("configuration changed") {
            bail!("unexpected reloaded-credentials error: {error}");
        }
        if error.contains("test-password") || error.contains("replacement-password") {
            bail!("reloaded-credentials error exposed a password");
        }
        Ok(())
    }
}
