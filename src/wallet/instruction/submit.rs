use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};

use ::wallet::{AccountIdentity, WalletCore};
use anyhow::{Context as _, Result};
use lee::program::Program;

use super::{
    LocalWalletInstructionRequest,
    model::{AccountPrivacy, InstructionMode, PreparedAccount, PreparedInstruction},
};

pub(super) async fn submit_instruction(
    wallet_home: PathBuf,
    request: &LocalWalletInstructionRequest,
    prepared: &PreparedInstruction,
) -> Result<(String, Option<usize>)> {
    let config_path = wallet_home.join("wallet_config.json");
    let storage_path = wallet_home.join("storage.json");
    let wallet = WalletCore::new_update_chain(config_path, storage_path, None)
        .context("failed to open local wallet state")?;

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
