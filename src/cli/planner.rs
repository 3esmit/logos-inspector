use std::{fs, path::Path};

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use super::args::{CliCommand, WalletCommand};
use crate::support::confirmation::ConfirmationPolicy;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CliInvocation {
    pub(super) method: &'static str,
    pub(super) args: Value,
}

impl CliInvocation {
    fn json(method: &'static str, args: Value) -> Self {
        Self { method, args }
    }
}

impl CliCommand {
    pub(super) fn invocation(self) -> Result<CliInvocation> {
        match self {
            CliCommand::DecodeAccount {
                data_hex,
                idl,
                idl_account,
            } => Ok(CliInvocation::json(
                "decodeAccount",
                json!([data_hex, read_idl(&idl)?, idl_account.unwrap_or_default()]),
            )),
            CliCommand::DecodeInstruction {
                program_id,
                words,
                idl,
                accounts,
            } => Ok(CliInvocation::json(
                "decodeInstruction",
                json!([
                    program_id,
                    parse_words(&words)?,
                    read_idl(&idl)?,
                    parse_accounts(accounts.as_deref())?
                ]),
            )),
            CliCommand::DecodeEvent {
                data_hex,
                idl,
                event,
            } => Ok(CliInvocation::json(
                "decodeEvent",
                json!([data_hex, read_idl(&idl)?, event.unwrap_or_default()]),
            )),
            CliCommand::ProgramFile { path } => {
                Ok(CliInvocation::json("programFile", json!([path])))
            }
            CliCommand::BlockchainNode(endpoints) => {
                let endpoints = endpoints.endpoints()?;
                Ok(CliInvocation::json(
                    "blockchainNode",
                    json!([endpoints.node_endpoint]),
                ))
            }
            CliCommand::BlockchainBlocks {
                slot_from,
                slot_to,
                endpoints,
            } => {
                let endpoints = endpoints.endpoints()?;
                Ok(CliInvocation::json(
                    "blockchainBlocks",
                    json!([endpoints.node_endpoint, slot_from, slot_to]),
                ))
            }
            CliCommand::LogoscoreStatus => Ok(CliInvocation::json("logoscoreStatus", json!([]))),
            CliCommand::SourcePolicy => Ok(CliInvocation::json("sourcePolicy", json!([]))),
            CliCommand::Modules => Ok(CliInvocation::json("modules", json!([]))),
            CliCommand::BlockchainModule { address } => Ok(CliInvocation::json(
                "blockchainModuleReport",
                json!([address.unwrap_or_default()]),
            )),
            CliCommand::Storage {
                cid,
                source_mode,
                rest_url,
                metrics_url,
            } => Ok(CliInvocation::json(
                "storageSourceReport",
                json!([
                    source_mode,
                    rest_url.unwrap_or_default(),
                    metrics_url.unwrap_or_default(),
                    cid.unwrap_or_default(),
                    false
                ]),
            )),
            CliCommand::Messaging {
                source_mode,
                rest_url,
                metrics_url,
            } => Ok(CliInvocation::json(
                "deliverySourceReport",
                json!([
                    source_mode,
                    rest_url.unwrap_or_default(),
                    metrics_url.unwrap_or_default()
                ]),
            )),
            CliCommand::Capabilities => Ok(CliInvocation::json("capabilitiesReport", json!([]))),
            CliCommand::Channels {
                slot_from,
                slot_to,
                endpoints,
            } => {
                let endpoints = endpoints.endpoints()?;
                Ok(CliInvocation::json(
                    "channelScan",
                    json!([endpoints.node_endpoint, slot_from, slot_to]),
                ))
            }
            CliCommand::SpelIdl { idl } => {
                Ok(CliInvocation::json("spelIdl", json!([read_idl(&idl)?])))
            }
            CliCommand::Rpc {
                endpoint,
                method,
                params,
            } => Ok(CliInvocation::json(
                "rawRpc",
                json!([endpoint, method, parse_rpc_params(params)?]),
            )),
            CliCommand::Wallet { command } => command.invocation(),
        }
    }
}

impl WalletCommand {
    fn invocation(self) -> Result<CliInvocation> {
        match self {
            WalletCommand::Status(profile) => Ok(CliInvocation::json(
                "localWalletProfileStatus",
                json!([profile.value()]),
            )),
            WalletCommand::Accounts(profile) => Ok(CliInvocation::json(
                "localWalletAccounts",
                json!([profile.value()]),
            )),
            WalletCommand::CreateAccount {
                profile,
                privacy,
                label,
            } => Ok(CliInvocation::json(
                "localWalletCreateAccount",
                json!([
                    profile.value(),
                    privacy,
                    label.unwrap_or_default(),
                    ConfirmationPolicy::WalletCreateAccount.token()
                ]),
            )),
            WalletCommand::Send {
                profile,
                from,
                to,
                to_npk,
                to_vpk,
                to_keys,
                to_identifier,
                amount,
            } => Ok(CliInvocation::json(
                "localWalletSendTransaction",
                json!([
                    profile.value(),
                    {
                        "from": from,
                        "to": to.unwrap_or_default(),
                        "to_npk": to_npk.unwrap_or_default(),
                        "to_vpk": to_vpk.unwrap_or_default(),
                        "to_keys": to_keys.unwrap_or_default(),
                        "to_identifier": to_identifier.unwrap_or_default(),
                        "amount": amount,
                    },
                    ConfirmationPolicy::WalletSendTransaction.token()
                ]),
            )),
            WalletCommand::Incoming(profile) => Ok(CliInvocation::json(
                "localWalletSyncPrivate",
                json!([
                    profile.value(),
                    ConfirmationPolicy::WalletSyncPrivate.token()
                ]),
            )),
            WalletCommand::Command { profile, args } => Ok(CliInvocation::json(
                "localWalletCommand",
                json!([
                    profile.value(),
                    args,
                    ConfirmationPolicy::WalletCommand.token()
                ]),
            )),
            WalletCommand::BedrockBalance {
                public_key,
                tip,
                endpoints,
            } => {
                let endpoints = endpoints.endpoints()?;
                Ok(CliInvocation::json(
                    "bedrockWalletBalance",
                    json!([endpoints.node_endpoint, public_key, tip.unwrap_or_default()]),
                ))
            }
        }
    }
}

fn parse_rpc_params(params: Option<String>) -> Result<Value> {
    match params {
        Some(raw) => serde_json::from_str(&raw)
            .with_context(|| format!("failed to parse rpc params JSON `{raw}`")),
        None => Ok(Value::Array(vec![])),
    }
}

fn read_idl(value: &str) -> Result<String> {
    let path = Path::new(value);
    if path.exists() {
        fs::read_to_string(path)
            .with_context(|| format!("failed to read IDL at {}", path.display()))
    } else {
        Ok(value.to_owned())
    }
}

fn parse_words(value: &str) -> Result<Vec<u32>> {
    let raw = value.trim();
    if raw.starts_with('[') {
        return serde_json::from_str(raw).context("failed to parse instruction words JSON array");
    }

    raw.split([',', ' ', '\n', '\t'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            word.parse::<u32>()
                .with_context(|| format!("invalid instruction word `{word}`"))
        })
        .collect()
}

fn parse_accounts(value: Option<&str>) -> Result<Vec<String>> {
    let Some(raw) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(vec![]);
    };
    if raw.starts_with('[') {
        return serde_json::from_str(raw).context("failed to parse accounts JSON array");
    }
    Ok(raw
        .split([',', ' ', '\n', '\t'])
        .filter(|account| !account.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, ensure};

    use super::*;
    use crate::cli::args::WalletProfileArgs;

    #[test]
    fn decode_instruction_command_plans_runtime_decode_method() -> Result<()> {
        let invocation = CliCommand::DecodeInstruction {
            program_id: "program-1".to_owned(),
            words: "1, 2 3".to_owned(),
            idl: "{\"name\":\"Demo\"}".to_owned(),
            accounts: Some("acct-1, acct-2".to_owned()),
        }
        .invocation()?;

        ensure!(
            invocation.method == "decodeInstruction",
            "unexpected method"
        );
        ensure!(
            invocation.args
                == json!([
                    "program-1",
                    [1, 2, 3],
                    "{\"name\":\"Demo\"}",
                    ["acct-1", "acct-2"]
                ]),
            "unexpected args: {}",
            invocation.args
        );
        Ok(())
    }

    #[test]
    fn wallet_send_command_uses_shared_confirmation_token() -> Result<()> {
        let invocation = WalletCommand::Send {
            profile: WalletProfileArgs {
                wallet_binary: Some("wallet".to_owned()),
                wallet_home: Some("/tmp/wallet".to_owned()),
                network_profile: Some("local".to_owned()),
            },
            from: "acct-1".to_owned(),
            to: Some("acct-2".to_owned()),
            to_npk: None,
            to_vpk: None,
            to_keys: None,
            to_identifier: None,
            amount: "1".to_owned(),
        }
        .invocation()?;

        ensure!(
            invocation.method == "localWalletSendTransaction",
            "unexpected method"
        );
        ensure!(
            invocation.args.pointer("/2").and_then(Value::as_str)
                == Some(ConfirmationPolicy::WalletSendTransaction.token()),
            "unexpected confirmation token"
        );
        ensure!(
            invocation.args.pointer("/1/from").and_then(Value::as_str) == Some("acct-1"),
            "unexpected sender"
        );
        Ok(())
    }
}
