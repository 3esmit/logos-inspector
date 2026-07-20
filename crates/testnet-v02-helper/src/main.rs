use std::{
    collections::HashMap,
    env,
    ffi::OsStr,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use logos_inspector_testnet_v02_helper_protocol::{
    HELPER_MODE, HelperAccount, HelperAccountPrivacy, HelperRequest, HelperResponse, HelperSuccess,
    SubmitPrivateIdlRequest,
};
use sequencer_service_rpc_testnet_v02::RpcClient as _;
use wallet_testnet_v02::{AccountIdentity, WalletCore};

fn main() -> Result<()> {
    let request_path = request_path_from_args()?;
    let response = match helper_response_from_path(&request_path) {
        Ok(response) => response,
        Err(error) => HelperResponse::Failure {
            error: error.to_string(),
        },
    };
    write_helper_response(&response)
}

fn request_path_from_args() -> Result<PathBuf> {
    let mut args = env::args_os();
    let _program = args.next();
    let Some(mode) = args.next() else {
        bail!("private Testnet helper requires {HELPER_MODE}");
    };
    if mode != OsStr::new(HELPER_MODE) {
        bail!("private Testnet helper requires {HELPER_MODE}");
    }
    let Some(flag) = args.next() else {
        bail!("private Testnet helper requires --request");
    };
    if flag != OsStr::new("--request") {
        bail!("private Testnet helper requires --request");
    }
    let request = args
        .next()
        .map(PathBuf::from)
        .context("private Testnet helper requires a request path")?;
    if args.next().is_some() {
        bail!("private Testnet helper received unexpected arguments");
    }
    Ok(request)
}

fn helper_response_from_path(request_path: &Path) -> Result<HelperResponse> {
    let request_file = std::fs::File::open(request_path)
        .context("failed to open private Testnet helper request")?;
    let request = serde_json::from_reader(std::io::BufReader::new(request_file))
        .context("failed to parse private Testnet helper request")?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("failed to start private Testnet helper runtime")?;
    match runtime.block_on(handle_helper_request(request)) {
        Ok(result) => Ok(HelperResponse::Success { result }),
        Err(error) => Ok(HelperResponse::Failure {
            error: error.to_string(),
        }),
    }
}

fn write_helper_response(response: &HelperResponse) -> Result<()> {
    let encoded =
        serde_json::to_vec(response).context("failed to encode private Testnet response")?;
    let mut stdout = std::io::stdout().lock();
    stdout
        .write_all(&encoded)
        .context("failed to write private Testnet response")?;
    stdout
        .write_all(b"\n")
        .context("failed to finish private Testnet response")?;
    stdout
        .flush()
        .context("failed to flush private Testnet response")
}

async fn handle_helper_request(request: HelperRequest) -> Result<HelperSuccess> {
    match request {
        HelperRequest::CheckProfile {
            wallet_home,
            sequencer_endpoint,
        } => {
            let wallet = open_legacy_wallet(wallet_home, sequencer_endpoint.as_deref())?;
            verify_testnet_v02_profile(&wallet).await?;
            Ok(HelperSuccess::Profile {
                protocol: "testnet_v0_2".to_owned(),
            })
        }
        HelperRequest::SubmitPrivateIdl(request) => submit_private_idl(request).await,
    }
}

async fn submit_private_idl(request: SubmitPrivateIdlRequest) -> Result<HelperSuccess> {
    let mut wallet = open_legacy_wallet(
        request.wallet_home.clone(),
        request.sequencer_endpoint.as_deref(),
    )?;
    verify_testnet_v02_profile(&wallet).await?;
    if requires_private_state_sync(&request.accounts) {
        wallet
            .sync_to_latest_block()
            .await
            .context("failed to synchronize private wallet state before building the instruction")?;
    }
    let program =
        load_program_with_dependencies(&request.program_binary, &request.dependency_binaries)?;
    if program.program.id() != request.expected_program_id {
        bail!("private instruction program binary does not match the selected IDL program ID");
    }

    let private_account_count = request
        .accounts
        .iter()
        .filter(|account| matches!(account.privacy, HelperAccountPrivacy::Private))
        .count();
    let accounts = request
        .accounts
        .iter()
        .map(account_identity)
        .collect::<Vec<_>>();
    let (tx_hash, shared_secrets) = wallet
        .send_privacy_preserving_tx(accounts, request.instruction_words, &program)
        .await
        .map_err(|error| {
            anyhow::anyhow!("failed to submit private Testnet instruction: {error}")
        })?;
    let tx_hash_text = tx_hash.to_string();
    if shared_secrets.len() != private_account_count {
        bail!("private Testnet helper produced an unexpected number of account secrets");
    }

    // The RPC accepted this exact transaction once it returned its hash.
    // Inspector owns the exact-source readback; private wallet state is
    // reconciled through the explicit local-wallet sync workflow after
    // inclusion. Waiting for the legacy wallet poll here can hide a submitted
    // transaction for its full polling window and encourages an unsafe retry.

    Ok(accepted_submission(tx_hash_text, shared_secrets.len()))
}

fn accepted_submission(tx_hash: String, shared_secret_count: usize) -> HelperSuccess {
    HelperSuccess::Submitted {
        tx_hash,
        shared_secret_count,
    }
}

fn requires_private_state_sync(accounts: &[HelperAccount]) -> bool {
    accounts
        .iter()
        .any(|account| matches!(account.privacy, HelperAccountPrivacy::Private))
}

fn account_identity(account: &HelperAccount) -> AccountIdentity {
    let account_id = lee_testnet_v02::AccountId::new(account.account_id);
    match account.privacy {
        HelperAccountPrivacy::Private => AccountIdentity::PrivateOwned(account_id),
        HelperAccountPrivacy::Public if account.signer => AccountIdentity::Public(account_id),
        HelperAccountPrivacy::Public => AccountIdentity::PublicNoSign(account_id),
    }
}

async fn verify_testnet_v02_profile(wallet: &WalletCore) -> Result<()> {
    let program_ids = wallet
        .sequencer_client
        .get_program_ids()
        .await
        .context("failed to query the Sequencer program profile")?;
    let privacy_circuit = program_ids
        .get("privacy_preserving_circuit")
        .context("Sequencer did not report a privacy circuit program")?;
    if privacy_circuit != &lee_testnet_v02::PRIVACY_PRESERVING_CIRCUIT_ID {
        bail!("Sequencer privacy circuit is not compatible with Testnet v0.2 private execution");
    }
    let authenticated_transfer = program_ids
        .get("authenticated_transfer")
        .context("Sequencer did not report an authenticated transfer program")?;
    if authenticated_transfer != &programs_testnet_v02::authenticated_transfer().id() {
        bail!(
            "Sequencer authenticated transfer program is not compatible with Testnet v0.2 private execution"
        );
    }
    Ok(())
}

fn open_legacy_wallet(
    wallet_home: PathBuf,
    sequencer_endpoint: Option<&str>,
) -> Result<WalletCore> {
    use wallet_testnet_v02::config::{WalletConfig, WalletConfigOverrides};

    let config_path = wallet_home.join("wallet_config.json");
    let storage_path = wallet_home.join("storage.json");
    let targeted = sequencer_endpoint
        .map(|endpoint| {
            let target = validated_sequencer_endpoint(endpoint)?;
            let file = std::fs::File::open(&config_path)
                .context("failed to open local wallet configuration")?;
            let config: WalletConfig = serde_json::from_reader(std::io::BufReader::new(file))
                .context("failed to parse local wallet configuration")?;
            let same_origin = config.sequencer_addr.origin() == target.origin();
            let basic_auth = if same_origin {
                config.basic_auth.clone()
            } else {
                None
            };
            let overrides = WalletConfigOverrides {
                sequencer_addr: Some(target),
                basic_auth: if same_origin { None } else { Some(None) },
                ..WalletConfigOverrides::default()
            };
            Ok::<_, anyhow::Error>((overrides, basic_auth))
        })
        .transpose()?;
    let overrides = targeted.as_ref().map(|(overrides, _)| overrides.clone());
    let wallet = WalletCore::new_update_chain(config_path, storage_path, overrides)
        .context("failed to open local wallet state")?;
    if let Some((_, expected_auth)) = targeted {
        verify_effective_basic_auth(wallet.config().basic_auth.as_ref(), expected_auth.as_ref())?;
    }
    Ok(wallet)
}

fn verify_effective_basic_auth(
    effective: Option<&common_testnet_v02::config::BasicAuth>,
    expected: Option<&common_testnet_v02::config::BasicAuth>,
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

fn load_program_with_dependencies(
    program_path: &Path,
    dependency_paths: &[PathBuf],
) -> Result<lee_testnet_v02::privacy_preserving_transaction::circuit::ProgramWithDependencies> {
    let program = load_program(program_path)?;
    let mut dependencies = HashMap::new();
    for path in dependency_paths {
        let dependency = load_program(path)?;
        dependencies.insert(dependency.id(), dependency);
    }
    Ok(
        lee_testnet_v02::privacy_preserving_transaction::circuit::ProgramWithDependencies::new(
            program,
            dependencies,
        ),
    )
}

fn load_program(path: &Path) -> Result<lee_testnet_v02::program::Program> {
    let bytes = std::fs::read(path)
        .with_context(|| format!("failed to read program binary at {}", path.display()))?;
    lee_testnet_v02::program::Program::new(bytes.into())
        .map_err(|error| anyhow::anyhow!("failed to parse program binary: {error:?}"))
}

fn validated_sequencer_endpoint(value: &str) -> Result<url::Url> {
    let endpoint =
        url::Url::parse(value).context("verified Sequencer endpoint is not a valid URL")?;
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

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};

    use super::*;

    #[test]
    fn accepted_submission_preserves_transaction_receipt() -> Result<()> {
        let response = accepted_submission("abc123".to_owned(), 2);
        let HelperSuccess::Submitted {
            tx_hash,
            shared_secret_count,
        } = response
        else {
            bail!("accepted private submission did not produce a receipt");
        };

        if tx_hash != "abc123" || shared_secret_count != 2 {
            bail!("accepted private submission changed its receipt");
        }
        Ok(())
    }

    #[test]
    fn private_accounts_require_state_sync_before_submission() {
        let public = HelperAccount {
            account_id: [1; 32],
            privacy: HelperAccountPrivacy::Public,
            signer: true,
        };
        let private = HelperAccount {
            account_id: [2; 32],
            privacy: HelperAccountPrivacy::Private,
            signer: false,
        };

        assert!(!requires_private_state_sync(&[public]));
        assert!(requires_private_state_sync(&[private]));
    }
}
