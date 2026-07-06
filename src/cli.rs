use std::{
    env, fs,
    io::{self, Write as _},
    path::Path,
};

use anyhow::{Context as _, Result};
use clap::{Args as ClapArgs, Parser, Subcommand};
use logos_inspector::{
    account_lookup, account_lookup_with_idl, bedrock_wallet_balance,
    blockchain::blockchain_blocks,
    blockchain::blockchain_node_report,
    channels::channel_scan,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl,
    decode_instruction_words_with_idl, last_sequencer_block_id,
    local_indexer::{bootstrap_default_local_indexer, is_default_local_indexer_endpoint},
    local_wallet_accounts, local_wallet_command, local_wallet_create_account,
    local_wallet_profile_status, local_wallet_send_transaction, local_wallet_sync_private,
    modules::blockchain_module_report,
    modules::capabilities_report,
    modules::delivery_source_report,
    modules::logoscore_status_report,
    modules::modules_report,
    modules::storage_source_report,
    network_profiles, overview, program_file_info, raw_rpc_report, resolve_network_endpoints,
    sequencer_block, sequencer_health, sequencer_program_ids, sequencer_transaction,
    sequencer_transaction_inspection, sequencer_transaction_inspection_with_idl,
    sequencer_transaction_trace, sequencer_transaction_trace_with_idl,
    source_policy::source_policy_report,
    spel::spel_idl_report,
};
use serde_json::Value;

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
    fn endpoints(&self) -> Result<logos_inspector::NetworkEndpoints> {
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
    let runtime = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    match args.command {
        CliCommand::Overview(endpoints) => {
            let endpoints = endpoints.endpoints()?;
            maybe_bootstrap_default_local_indexer(&endpoints.indexer_endpoint)?;
            let report = runtime.block_on(overview(
                &endpoints.sequencer_endpoint,
                &endpoints.indexer_endpoint,
                &endpoints.node_endpoint,
            ));
            print_json(&report)
        }
        CliCommand::Health(endpoints) => {
            let sequencer_url = endpoints.sequencer_url()?;
            runtime.block_on(sequencer_health(&sequencer_url))?;
            print_line("ok")
        }
        CliCommand::Head(endpoints) => {
            let sequencer_url = endpoints.sequencer_url()?;
            let head = runtime.block_on(last_sequencer_block_id(&sequencer_url))?;
            print_line(head)
        }
        CliCommand::Programs(endpoints) => {
            let sequencer_url = endpoints.sequencer_url()?;
            let programs = runtime.block_on(sequencer_program_ids(&sequencer_url))?;
            print_json(&programs)
        }
        CliCommand::Block {
            block_id,
            endpoints,
        } => {
            let sequencer_url = endpoints.sequencer_url()?;
            let block = runtime.block_on(sequencer_block(&sequencer_url, block_id))?;
            print_json(&block)
        }
        CliCommand::Tx { hash, endpoints } => {
            let sequencer_url = endpoints.sequencer_url()?;
            let tx = runtime.block_on(sequencer_transaction(&sequencer_url, &hash))?;
            print_json(&tx)
        }
        CliCommand::InspectTx {
            hash,
            idl,
            endpoints,
        } => {
            let sequencer_url = endpoints.sequencer_url()?;
            if let Some(idl) = idl {
                let idl_json = read_idl(&idl)?;
                let tx = runtime.block_on(sequencer_transaction_inspection_with_idl(
                    &sequencer_url,
                    &hash,
                    &idl_json,
                ))?;
                print_json(&tx)
            } else {
                let tx =
                    runtime.block_on(sequencer_transaction_inspection(&sequencer_url, &hash))?;
                print_json(&tx)
            }
        }
        CliCommand::TraceTx {
            hash,
            idl,
            endpoints,
        } => {
            let sequencer_url = endpoints.sequencer_url()?;
            if let Some(idl) = idl {
                let idl_json = read_idl(&idl)?;
                let tx = runtime.block_on(sequencer_transaction_trace_with_idl(
                    &sequencer_url,
                    &hash,
                    &idl_json,
                ))?;
                print_json(&tx)
            } else {
                let tx = runtime.block_on(sequencer_transaction_trace(&sequencer_url, &hash))?;
                print_json(&tx)
            }
        }
        CliCommand::Account {
            account_id,
            idl,
            idl_account,
            endpoints,
        } => {
            let endpoints = endpoints.endpoints()?;
            maybe_bootstrap_default_local_indexer(&endpoints.indexer_endpoint)?;
            if let Some(idl) = idl {
                let idl_json = read_idl(&idl)?;
                let account = runtime.block_on(account_lookup_with_idl(
                    &endpoints.sequencer_endpoint,
                    &endpoints.indexer_endpoint,
                    &account_id,
                    &idl_json,
                    idl_account.as_deref(),
                ))?;
                print_json(&account)
            } else {
                let account = runtime.block_on(account_lookup(
                    &endpoints.sequencer_endpoint,
                    &endpoints.indexer_endpoint,
                    &account_id,
                ))?;
                print_json(&account)
            }
        }
        CliCommand::DecodeAccount {
            data_hex,
            idl,
            idl_account,
        } => {
            let idl_json = read_idl(&idl)?;
            print_json(&decode_account_data_hex_with_idl(
                &idl_json,
                idl_account.as_deref(),
                &data_hex,
                None,
            )?)
        }
        CliCommand::DecodeInstruction {
            program_id,
            words,
            idl,
            accounts,
        } => {
            let idl_json = read_idl(&idl)?;
            let words = parse_words(&words)?;
            let accounts = parse_accounts(accounts.as_deref())?;
            print_json(&decode_instruction_words_with_idl(
                &idl_json,
                &program_id,
                &words,
                &accounts,
            )?)
        }
        CliCommand::DecodeEvent {
            data_hex,
            idl,
            event,
        } => {
            let idl_json = read_idl(&idl)?;
            print_json(&decode_event_data_hex_with_idl(
                &idl_json,
                event.as_deref(),
                &data_hex,
            )?)
        }
        CliCommand::ProgramFile { path } => print_json(&program_file_info(path)?),
        CliCommand::BlockchainNode(endpoints) => {
            let endpoints = endpoints.endpoints()?;
            let report = runtime.block_on(blockchain_node_report(&endpoints.node_endpoint));
            print_json(&report)
        }
        CliCommand::BlockchainBlocks {
            slot_from,
            slot_to,
            endpoints,
        } => {
            let endpoints = endpoints.endpoints()?;
            let blocks = runtime.block_on(blockchain_blocks(
                &endpoints.node_endpoint,
                slot_from,
                slot_to,
            ))?;
            print_json(&blocks)
        }
        CliCommand::LogoscoreStatus => print_json(&logoscore_status_report()),
        CliCommand::SourcePolicy => print_json(&source_policy_report(network_profiles())),
        CliCommand::Modules => print_json(&modules_report()),
        CliCommand::BlockchainModule { address } => {
            print_json(&blockchain_module_report(address.as_deref()))
        }
        CliCommand::Storage {
            cid,
            source_mode,
            rest_url,
            metrics_url,
        } => {
            let report = runtime.block_on(storage_source_report(
                &source_mode,
                rest_url.as_deref(),
                metrics_url.as_deref(),
                cid.as_deref(),
                false,
            ));
            print_json(&report)
        }
        CliCommand::Messaging {
            source_mode,
            rest_url,
            metrics_url,
        } => {
            let report = runtime.block_on(delivery_source_report(
                &source_mode,
                rest_url.as_deref(),
                metrics_url.as_deref(),
            ));
            print_json(&report)
        }
        CliCommand::Capabilities => print_json(&capabilities_report()),
        CliCommand::Channels {
            slot_from,
            slot_to,
            endpoints,
        } => {
            let endpoints = endpoints.endpoints()?;
            let report =
                runtime.block_on(channel_scan(&endpoints.node_endpoint, slot_from, slot_to))?;
            print_json(&report)
        }
        CliCommand::SpelIdl { idl } => {
            let idl_json = read_idl(&idl)?;
            print_json(&spel_idl_report(&idl_json)?)
        }
        CliCommand::Rpc {
            endpoint,
            method,
            params,
        } => {
            let params = parse_rpc_params(params)?;
            let report = runtime.block_on(raw_rpc_report(&endpoint, &method, params))?;
            print_json(&report)
        }
        CliCommand::Wallet { command } => match command {
            WalletCommand::Status(profile) => {
                print_json(&local_wallet_profile_status(profile.value())?)
            }
            WalletCommand::Accounts(profile) => {
                print_json(&local_wallet_accounts(profile.value())?)
            }
            WalletCommand::CreateAccount {
                profile,
                privacy,
                label,
            } => print_json(&local_wallet_create_account(
                profile.value(),
                &privacy,
                label.as_deref(),
            )?),
            WalletCommand::Send {
                profile,
                from,
                to,
                to_npk,
                to_vpk,
                to_keys,
                to_identifier,
                amount,
            } => print_json(&local_wallet_send_transaction(
                profile.value(),
                serde_json::json!({
                    "from": from,
                    "to": to.unwrap_or_default(),
                    "to_npk": to_npk.unwrap_or_default(),
                    "to_vpk": to_vpk.unwrap_or_default(),
                    "to_keys": to_keys.unwrap_or_default(),
                    "to_identifier": to_identifier.unwrap_or_default(),
                    "amount": amount,
                }),
            )?),
            WalletCommand::Incoming(profile) => {
                print_json(&local_wallet_sync_private(profile.value())?)
            }
            WalletCommand::Command { profile, args } => {
                print_json(&local_wallet_command(profile.value(), args)?)
            }
            WalletCommand::BedrockBalance {
                public_key,
                tip,
                endpoints,
            } => {
                let endpoints = endpoints.endpoints()?;
                let balance = runtime.block_on(bedrock_wallet_balance(
                    &endpoints.node_endpoint,
                    &public_key,
                    tip.as_deref(),
                ))?;
                print_json(&balance)
            }
        },
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

fn print_line(value: impl std::fmt::Display) -> Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    writeln!(stdout, "{value}")?;
    Ok(())
}
