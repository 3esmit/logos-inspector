use anyhow::Result;
use clap::{Args as ClapArgs, Parser, Subcommand};
use serde_json::Value;

use crate::source_routing::{NetworkEndpoints, resolve_network_endpoints};

#[derive(Debug, Parser)]
#[command(name = "logos-inspector")]
#[command(about = "Inspect Logos networks")]
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

impl CliArgs {
    pub(super) fn into_command(self) -> CliCommand {
        self.command
    }
}

#[derive(Debug, Subcommand)]
pub(super) enum CliCommand {
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
pub(super) enum WalletCommand {
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
pub(super) struct WalletProfileArgs {
    #[arg(long)]
    pub(super) wallet_binary: Option<String>,
    #[arg(long)]
    pub(super) wallet_home: Option<String>,
    #[arg(long)]
    pub(super) network_profile: Option<String>,
}

#[derive(Debug, Clone, ClapArgs)]
pub struct EndpointArgs {
    #[arg(long, visible_alias = "network", value_name = "PROFILE")]
    pub(super) profile: Option<String>,
    #[arg(long)]
    pub(super) node_url: Option<String>,
}

impl EndpointArgs {
    pub(super) fn endpoints(&self) -> Result<NetworkEndpoints> {
        resolve_network_endpoints(self.profile.as_deref(), self.node_url.as_deref())
    }
}

impl WalletProfileArgs {
    pub(super) fn value(&self) -> Value {
        serde_json::json!({
            "wallet_binary": self.wallet_binary.as_deref().unwrap_or_default(),
            "wallet_home": self.wallet_home.as_deref().unwrap_or_default(),
            "network_profile": self.network_profile.as_deref().unwrap_or_default(),
        })
    }
}
