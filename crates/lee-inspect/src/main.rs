use std::{collections::HashMap, env, fs, str::FromStr as _};

use anyhow::{Context as _, Result, bail};
use lee::{
    AccountId, privacy_preserving_transaction::circuit::ProgramWithDependencies, program::Program,
};
use token_core::Instruction;
use wallet::{AccountIdentity, WalletCore};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let args = env::args().collect::<Vec<_>>();
    match args.get(1).map(String::as_str) {
        Some("token-new-public") => token_new_public(&args[2..]).await,
        _ => usage(),
    }
}

async fn token_new_public(args: &[String]) -> Result<()> {
    if args.len() != 5 {
        usage();
    }

    let program_path = &args[0];
    let definition_account = parse_public_account(&args[1])?;
    let holding_account = parse_public_account(&args[2])?;
    let name = args[3].clone();
    let total_supply = args[4]
        .parse::<u128>()
        .with_context(|| format!("invalid total supply `{}`", args[4]))?;

    let bytecode = fs::read(program_path)
        .with_context(|| format!("failed to read program bytecode at {program_path}"))?;
    let program = Program::new(bytecode).context("failed to parse deployed program bytecode")?;
    let program_id = program.id();
    let program = ProgramWithDependencies::new(program, HashMap::new());

    let instruction = Instruction::NewFungibleDefinition { name, total_supply };
    let instruction_data =
        Program::serialize_instruction(instruction).context("failed to serialize instruction")?;

    let wallet =
        WalletCore::from_env().context("failed to load wallet from LEE_WALLET_HOME_DIR")?;
    let tx_hash = wallet
        .send_pub_tx(
            vec![
                AccountIdentity::Public(definition_account),
                AccountIdentity::Public(holding_account),
            ],
            instruction_data,
            &program,
        )
        .await
        .context("failed to submit public token transaction")?;

    println!("program_id={program_id:?}");
    println!("tx_hash={tx_hash}");
    Ok(())
}

fn parse_public_account(raw: &str) -> Result<AccountId> {
    let Some(account_id) = raw.strip_prefix("Public/") else {
        bail!("account `{raw}` must use Public/<account-id> format");
    };
    AccountId::from_str(account_id).with_context(|| format!("invalid account id `{account_id}`"))
}

fn usage() -> ! {
    eprintln!(
        "usage: lee-inspect token-new-public <program.bin> <definition-account> <holding-account> <name> <total-supply>"
    );
    std::process::exit(2);
}
