use std::collections::BTreeMap;

use anyhow::{Context as _, Result, bail};
use lee::AccountId;
use lee_core::program::ProgramId;
use serde_json::Value;
use sha2::{Digest as _, Sha256};

use crate::parse_account_id;

use super::{
    LocalWalletInstructionRequest,
    model::{AccountPrivacy, PreparedAccount},
    values::{ParsedValue, named_value},
};

pub(super) fn resolve_accounts(
    instruction: &Value,
    request: &LocalWalletInstructionRequest,
    program_id: &ProgramId,
    parsed_args: &BTreeMap<String, ParsedValue>,
) -> Result<Vec<PreparedAccount>> {
    let idl_accounts = instruction
        .get("accounts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut account_map = BTreeMap::<String, AccountId>::new();
    let mut resolved = Vec::new();

    for account in idl_accounts
        .iter()
        .filter(|account| !account_has_pda(account))
    {
        let name = account
            .get("name")
            .and_then(Value::as_str)
            .context("IDL account missing name")?;
        let signer = account.get("signer").and_then(Value::as_bool) == Some(true);
        let rest = account.get("rest").and_then(Value::as_bool) == Some(true);
        if rest {
            if let Some(raw) = named_value(&request.accounts, name) {
                for (index, item) in raw
                    .split(',')
                    .map(str::trim)
                    .filter(|item| !item.is_empty())
                    .enumerate()
                {
                    let (account_id, privacy) =
                        parse_account_reference(item).with_context(|| {
                            format!("failed to parse rest account `{name}` entry {index}")
                        })?;
                    account_map.insert(format!("{name}[{index}]"), account_id);
                    resolved.push(PreparedAccount {
                        name: format!("{name}[{index}]"),
                        account_id,
                        privacy,
                        signer,
                        rest: true,
                        pda: false,
                    });
                }
            }
            continue;
        }
        let raw = named_value(&request.accounts, name)
            .with_context(|| format!("account `{name}` is required"))?;
        let (account_id, privacy) = parse_account_reference(raw)
            .with_context(|| format!("failed to parse account `{name}`"))?;
        account_map.insert(name.to_owned(), account_id);
        resolved.push(PreparedAccount {
            name: name.to_owned(),
            account_id,
            privacy,
            signer,
            rest: false,
            pda: false,
        });
    }

    resolve_external_pda_seed_accounts(&idl_accounts, request, &mut account_map)?;

    for account in idl_accounts
        .iter()
        .filter(|account| account_has_pda(account))
    {
        let name = account
            .get("name")
            .and_then(Value::as_str)
            .context("IDL account missing name")?;
        let pda = account
            .get("pda")
            .context("PDA account missing pda field after filter")?;
        if pda.get("private").and_then(Value::as_bool) == Some(true) {
            bail!("private PDA accounts are not supported by direct Inspector interaction");
        }
        let account_id = compute_pda(pda, program_id, &account_map, parsed_args)
            .with_context(|| format!("failed to compute PDA `{name}`"))?;
        account_map.insert(name.to_owned(), account_id);
        resolved.push(PreparedAccount {
            name: name.to_owned(),
            account_id,
            privacy: AccountPrivacy::Public,
            signer: account.get("signer").and_then(Value::as_bool) == Some(true),
            rest: false,
            pda: true,
        });
    }

    Ok(resolved)
}

fn resolve_external_pda_seed_accounts(
    idl_accounts: &[Value],
    request: &LocalWalletInstructionRequest,
    account_map: &mut BTreeMap<String, AccountId>,
) -> Result<()> {
    for account in idl_accounts {
        let Some(pda) = account.get("pda") else {
            continue;
        };
        for seed in pda
            .get("seeds")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
        {
            if seed.get("kind").and_then(Value::as_str) != Some("account") {
                continue;
            }
            let path = seed
                .get("path")
                .and_then(Value::as_str)
                .context("account PDA seed missing path")?;
            if account_map.contains_key(path) {
                continue;
            }
            let raw = named_value(&request.accounts, path)
                .with_context(|| format!("PDA seed account `{path}` is required"))?;
            let (account_id, _) = parse_account_reference(raw)
                .with_context(|| format!("failed to parse PDA seed account `{path}`"))?;
            account_map.insert(path.to_owned(), account_id);
        }
    }
    Ok(())
}

fn account_has_pda(account: &Value) -> bool {
    account.get("pda").is_some()
}

fn compute_pda(
    pda: &Value,
    program_id: &ProgramId,
    account_map: &BTreeMap<String, AccountId>,
    parsed_args: &BTreeMap<String, ParsedValue>,
) -> Result<AccountId> {
    let seeds = pda
        .get("seeds")
        .and_then(Value::as_array)
        .context("PDA has no seeds array")?;
    if seeds.is_empty() {
        bail!("PDA requires at least one seed");
    }
    let resolved = seeds
        .iter()
        .map(|seed| resolve_seed(seed, account_map, parsed_args))
        .collect::<Result<Vec<_>>>()?;
    let combined = if resolved.len() == 1 {
        *resolved
            .first()
            .context("PDA seeds unexpectedly empty after length check")?
    } else {
        let mut hasher = Sha256::new();
        for seed in &resolved {
            hasher.update(seed);
        }
        hasher.finalize().into()
    };
    Ok(AccountId::for_public_pda(
        program_id,
        &lee_core::program::PdaSeed::new(combined),
    ))
}

fn resolve_seed(
    seed: &Value,
    account_map: &BTreeMap<String, AccountId>,
    parsed_args: &BTreeMap<String, ParsedValue>,
) -> Result<[u8; 32]> {
    match seed.get("kind").and_then(Value::as_str) {
        Some("const") => {
            let value = seed
                .get("value")
                .and_then(Value::as_str)
                .context("const PDA seed missing value")?;
            let bytes = value.as_bytes();
            if bytes.len() > 32 {
                bail!("const PDA seed `{value}` exceeds 32 bytes");
            }
            let mut seed = [0_u8; 32];
            let Some(seed_prefix) = seed.get_mut(..bytes.len()) else {
                bail!("const PDA seed `{value}` exceeds 32 bytes");
            };
            seed_prefix.copy_from_slice(bytes);
            Ok(seed)
        }
        Some("account") => {
            let path = seed
                .get("path")
                .and_then(Value::as_str)
                .context("account PDA seed missing path")?;
            account_map
                .get(path)
                .map(|account_id| *account_id.value())
                .with_context(|| format!("PDA seed account `{path}` is not resolved"))
        }
        Some("arg") => {
            let path = seed
                .get("path")
                .and_then(Value::as_str)
                .context("arg PDA seed missing path")?;
            parsed_args
                .get(path)
                .and_then(|value| value.seed_bytes)
                .with_context(|| format!("PDA seed arg `{path}` cannot be converted to 32 bytes"))
        }
        Some(kind) => bail!("unsupported PDA seed kind `{kind}`"),
        None => bail!("PDA seed missing kind"),
    }
}

fn parse_account_reference(value: &str) -> Result<(AccountId, AccountPrivacy)> {
    let value = value.trim();
    let (trimmed, privacy) = if let Some(value) = value
        .strip_prefix("Private/")
        .or_else(|| value.strip_prefix("private/"))
    {
        (value.trim(), AccountPrivacy::Private)
    } else if let Some(value) = value
        .strip_prefix("Public/")
        .or_else(|| value.strip_prefix("public/"))
    {
        (value.trim(), AccountPrivacy::Public)
    } else {
        (value, AccountPrivacy::Public)
    };
    Ok((parse_account_id(trimmed)?, privacy))
}
