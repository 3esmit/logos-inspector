use std::{
    env, fs,
    io::{self, Write as _},
    path::Path,
};

use anyhow::{Context as _, Result};
use clap::{Args as ClapArgs, Parser, Subcommand};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    inspector::{
        command_surface::{DispatchContext, dispatch_inspector_command},
        commands::operations::RuntimeOperationInterface,
        value::to_value,
    },
    local_nodes::{bootstrap_default_local_indexer, is_default_local_indexer_endpoint},
    logoscore,
    source_routing::{Args as SourceArgs, NetworkEndpoints, resolve_network_endpoints},
    support::confirmation::ConfirmationPolicy,
};

#[derive(Debug, Parser)]
#[command(name = "logos-inspector")]
#[command(about = "Inspect Logos Blockchain and Logos Execution Zone networks")]
pub struct Args {
    #[command(subcommand)]
    pub mode: Option<Mode>,
}

#[derive(Debug, Subcommand)]
pub enum Mode {
    Gui,
    Cli(Box<CliArgs>),
}

#[derive(Debug, ClapArgs)]
pub struct CliArgs {
    #[command(subcommand)]
    command: CliCommand,
}

#[derive(Debug, Subcommand)]
enum CliCommand {
    Overview(EndpointArgs),
    Health(SequencerArgs),
    Head(SequencerArgs),
    Programs(SequencerArgs),
    Block {
        block_id: u64,
        #[command(flatten)]
        endpoints: SequencerArgs,
    },
    Tx {
        hash: String,
        #[command(flatten)]
        endpoints: SequencerArgs,
    },
    InspectTx {
        hash: String,
        #[arg(long)]
        idl: Option<String>,
        #[command(flatten)]
        endpoints: SequencerArgs,
    },
    TraceTx {
        hash: String,
        #[arg(long)]
        idl: Option<String>,
        #[command(flatten)]
        endpoints: SequencerArgs,
    },
    Account {
        account_id: String,
        #[arg(long)]
        idl: Option<String>,
        #[arg(long)]
        idl_account: Option<String>,
        #[command(flatten)]
        endpoints: EndpointArgs,
    },
    DecodeAccount {
        #[arg(long)]
        data_hex: String,
        #[arg(long)]
        idl: String,
        #[arg(long)]
        idl_account: Option<String>,
    },
    DecodeInstruction {
        #[arg(long)]
        program_id: String,
        #[arg(long)]
        words: String,
        #[arg(long)]
        idl: String,
        #[arg(long)]
        accounts: Option<String>,
    },
    DecodeEvent {
        /// Decode a nonstandard Inspector event extension, not official SPEL IDL.
        #[arg(long)]
        data_hex: String,
        #[arg(long)]
        idl: String,
        #[arg(long)]
        event: Option<String>,
    },
    ProgramFile {
        path: String,
    },
    BlockchainNode(EndpointArgs),
    BlockchainBlocks {
        #[arg(long)]
        slot_from: u64,
        #[arg(long)]
        slot_to: u64,
        #[command(flatten)]
        endpoints: EndpointArgs,
    },
    LogoscoreStatus,
    SourcePolicy,
    Modules,
    BlockchainModule {
        #[arg(long)]
        address: Option<String>,
    },
    Storage {
        #[arg(long)]
        cid: Option<String>,
        #[arg(long, default_value = "rest")]
        source_mode: String,
        #[arg(long)]
        rest_url: Option<String>,
        #[arg(long)]
        metrics_url: Option<String>,
    },
    Messaging {
        #[arg(long, default_value = "rest")]
        source_mode: String,
        #[arg(long)]
        rest_url: Option<String>,
        #[arg(long)]
        metrics_url: Option<String>,
    },
    Capabilities,
    Channels {
        #[arg(long)]
        slot_from: u64,
        #[arg(long)]
        slot_to: u64,
        #[command(flatten)]
        endpoints: EndpointArgs,
    },
    SpelIdl {
        idl: String,
    },
    Rpc {
        endpoint: String,
        method: String,
        params: Option<String>,
    },
    Wallet {
        #[command(subcommand)]
        command: WalletCommand,
    },
}

#[derive(Debug, Subcommand)]
enum WalletCommand {
    Status(WalletProfileArgs),
    Accounts(WalletProfileArgs),
    CreateAccount {
        #[command(flatten)]
        profile: WalletProfileArgs,
        privacy: String,
        #[arg(long)]
        label: Option<String>,
    },
    Send {
        #[command(flatten)]
        profile: WalletProfileArgs,
        #[arg(long)]
        from: String,
        #[arg(long)]
        to: Option<String>,
        #[arg(long)]
        to_npk: Option<String>,
        #[arg(long)]
        to_vpk: Option<String>,
        #[arg(long)]
        to_keys: Option<String>,
        #[arg(long)]
        to_identifier: Option<String>,
        #[arg(long)]
        amount: String,
    },
    Incoming(WalletProfileArgs),
    Command {
        #[command(flatten)]
        profile: WalletProfileArgs,
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<String>,
    },
    BedrockBalance {
        public_key: String,
        #[arg(long)]
        tip: Option<String>,
        #[command(flatten)]
        endpoints: EndpointArgs,
    },
}

#[derive(Debug, Clone, ClapArgs)]
struct WalletProfileArgs {
    #[arg(long)]
    wallet_binary: Option<String>,
    #[arg(long)]
    wallet_home: Option<String>,
    #[arg(long)]
    network_profile: Option<String>,
}

#[derive(Debug, Clone, ClapArgs)]
pub struct EndpointArgs {
    #[arg(long, visible_alias = "network", value_name = "PROFILE")]
    profile: Option<String>,
    #[arg(long)]
    sequencer_url: Option<String>,
    #[arg(long)]
    indexer_url: Option<String>,
    #[arg(long)]
    node_url: Option<String>,
}

#[derive(Debug, Clone, ClapArgs)]
pub struct SequencerArgs {
    #[arg(long, visible_alias = "network", value_name = "PROFILE")]
    profile: Option<String>,
    #[arg(long)]
    sequencer_url: Option<String>,
}

impl EndpointArgs {
    fn endpoints(&self) -> Result<NetworkEndpoints> {
        let endpoints = resolve_network_endpoints(
            self.profile.as_deref(),
            self.sequencer_url.as_deref(),
            self.indexer_url.as_deref(),
            self.node_url.as_deref(),
        )?;
        Ok(endpoints)
    }
}

impl SequencerArgs {
    fn sequencer_url(&self) -> Result<String> {
        Ok(resolve_network_endpoints(
            self.profile.as_deref(),
            self.sequencer_url.as_deref(),
            None,
            None,
        )?
        .sequencer_endpoint)
    }
}

pub fn run(args: CliArgs) -> Result<()> {
    let invocation = args.command.invocation()?;
    if let Some(endpoint) = invocation.bootstrap_indexer_endpoint.as_deref() {
        maybe_bootstrap_default_local_indexer(endpoint)?;
    }
    let runtime = CliCommandRuntime::new()?;
    let value = runtime.call(invocation.method, invocation.args)?;
    match invocation.output {
        CliOutput::Json => print_json(&value),
        CliOutput::Text => print_text_value(&value),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CliOutput {
    Json,
    Text,
}

#[derive(Debug, Clone, PartialEq)]
struct CliInvocation {
    method: &'static str,
    args: Value,
    output: CliOutput,
    bootstrap_indexer_endpoint: Option<String>,
}

impl CliInvocation {
    fn json(method: &'static str, args: Value) -> Self {
        Self {
            method,
            args,
            output: CliOutput::Json,
            bootstrap_indexer_endpoint: None,
        }
    }

    fn text(method: &'static str, args: Value) -> Self {
        Self {
            output: CliOutput::Text,
            ..Self::json(method, args)
        }
    }

    fn with_indexer_bootstrap(mut self, endpoint: &str) -> Self {
        self.bootstrap_indexer_endpoint = Some(endpoint.to_owned());
        self
    }
}

struct CliCommandRuntime {
    runtime: Runtime,
    operations: RuntimeOperationInterface,
}

impl CliCommandRuntime {
    fn new() -> Result<Self> {
        Ok(Self {
            runtime: Runtime::new().context("failed to create tokio runtime")?,
            operations: RuntimeOperationInterface::default(),
        })
    }

    fn call(&self, method: &str, args: Value) -> Result<Value> {
        let call_core_module = |module: &str, method: &str, args: Value| {
            let args = SourceArgs::new(args)?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| value.to_string())
                })
                .collect::<Vec<_>>();
            to_value(logoscore::call(module, method, &args)?)
        };
        let context = DispatchContext {
            runtime: &self.runtime,
            operations: &self.operations,
            call_core_module: &call_core_module,
        };
        dispatch_inspector_command(&context, method, args)?
            .with_context(|| format!("unknown inspector method `{method}`"))
    }
}

impl CliCommand {
    fn invocation(self) -> Result<CliInvocation> {
        match self {
            CliCommand::Overview(endpoints) => {
                let endpoints = endpoints.endpoints()?;
                Ok(CliInvocation::json(
                    "overview",
                    json!([
                        endpoints.sequencer_endpoint,
                        endpoints.indexer_endpoint,
                        endpoints.node_endpoint
                    ]),
                )
                .with_indexer_bootstrap(&endpoints.indexer_endpoint))
            }
            CliCommand::Health(endpoints) => Ok(CliInvocation::text(
                "health",
                json!([endpoints.sequencer_url()?]),
            )),
            CliCommand::Head(endpoints) => Ok(CliInvocation::json(
                "head",
                json!([endpoints.sequencer_url()?]),
            )),
            CliCommand::Programs(endpoints) => Ok(CliInvocation::json(
                "programs",
                json!([endpoints.sequencer_url()?]),
            )),
            CliCommand::Block {
                block_id,
                endpoints,
            } => Ok(CliInvocation::json(
                "block",
                json!([endpoints.sequencer_url()?, block_id]),
            )),
            CliCommand::Tx { hash, endpoints } => Ok(CliInvocation::json(
                "transaction",
                json!([endpoints.sequencer_url()?, hash]),
            )),
            CliCommand::InspectTx {
                hash,
                idl,
                endpoints,
            } => Ok(CliInvocation::json(
                "inspectTransaction",
                json!([
                    endpoints.sequencer_url()?,
                    hash,
                    optional_idl_json(idl.as_deref())?
                ]),
            )),
            CliCommand::TraceTx {
                hash,
                idl,
                endpoints,
            } => Ok(CliInvocation::json(
                "traceTransaction",
                json!([
                    endpoints.sequencer_url()?,
                    hash,
                    optional_idl_json(idl.as_deref())?
                ]),
            )),
            CliCommand::Account {
                account_id,
                idl,
                idl_account,
                endpoints,
            } => {
                let endpoints = endpoints.endpoints()?;
                Ok(CliInvocation::json(
                    "account",
                    json!([
                        endpoints.sequencer_endpoint,
                        endpoints.indexer_endpoint,
                        account_id,
                        optional_idl_json(idl.as_deref())?,
                        idl_account.unwrap_or_default()
                    ]),
                )
                .with_indexer_bootstrap(&endpoints.indexer_endpoint))
            }
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

impl WalletProfileArgs {
    fn value(&self) -> Value {
        serde_json::json!({
            "wallet_binary": self.wallet_binary.as_deref().unwrap_or_default(),
            "wallet_home": self.wallet_home.as_deref().unwrap_or_default(),
            "network_profile": self.network_profile.as_deref().unwrap_or_default(),
        })
    }
}

fn maybe_bootstrap_default_local_indexer(endpoint: &str) -> Result<()> {
    if env::var_os("LOGOS_INSPECTOR_ENABLE_INDEXER_AUTO_BOOTSTRAP").is_some()
        && is_default_local_indexer_endpoint(endpoint)
    {
        bootstrap_default_local_indexer()?;
    }
    Ok(())
}

fn optional_idl_json(value: Option<&str>) -> Result<String> {
    match value {
        Some(value) => read_idl(value),
        None => Ok(String::new()),
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

fn print_json(value: &impl serde::Serialize) -> Result<()> {
    print_line(serde_json::to_string_pretty(value)?)
}

fn print_text_value(value: &Value) -> Result<()> {
    if let Some(value) = value.as_str() {
        return print_line(value);
    }
    print_line(value)
}

fn print_line(value: impl std::fmt::Display) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{value}")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use anyhow::ensure;

    use super::*;

    #[test]
    fn account_command_plans_shared_account_operation_with_bootstrap() -> Result<()> {
        let invocation = CliCommand::Account {
            account_id: "account-1".to_owned(),
            idl: None,
            idl_account: Some("Vault".to_owned()),
            endpoints: EndpointArgs {
                profile: None,
                sequencer_url: Some("https://sequencer.invalid".to_owned()),
                indexer_url: Some("http://127.0.0.1:8779".to_owned()),
                node_url: None,
            },
        }
        .invocation()?;

        ensure!(invocation.method == "account", "unexpected method");
        ensure!(
            invocation.output == CliOutput::Json,
            "unexpected output mode: {:?}",
            invocation.output
        );
        ensure!(
            invocation.bootstrap_indexer_endpoint.as_deref() == Some("http://127.0.0.1:8779"),
            "unexpected bootstrap endpoint"
        );
        ensure!(
            invocation.args
                == json!([
                    "https://sequencer.invalid",
                    "http://127.0.0.1:8779",
                    "account-1",
                    "",
                    "Vault"
                ]),
            "unexpected args: {}",
            invocation.args
        );
        Ok(())
    }

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
