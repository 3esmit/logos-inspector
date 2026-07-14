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
        #[arg(long, default_value = "logoscore_cli")]
        source_mode: String,
        #[arg(long)]
        rest_url: Option<String>,
        #[arg(long)]
        metrics_url: Option<String>,
    },
    Messaging {
        #[arg(long, default_value = "logoscore_cli")]
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
    Backup {
        #[command(subcommand)]
        command: BackupCommand,
    },
}

#[derive(Debug, Subcommand)]
pub(super) enum BackupCommand {
    List,
    Download {
        cid: String,
        #[arg(long, default_value = "logoscore_cli")]
        source_mode: String,
        #[arg(long)]
        rest_url: Option<String>,
        #[arg(long)]
        local_only: bool,
    },
    Preview(BackupImportArgs),
    Apply(BackupImportArgs),
}

#[derive(Debug, Clone, ClapArgs)]
pub(super) struct BackupImportArgs {
    pub(super) backup_catalog_id: String,
    #[arg(long)]
    pub(super) options: String,
    #[arg(long)]
    pub(super) wallet_profile: Option<String>,
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

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};

    use super::*;

    #[test]
    fn backup_subcommands_parse_scriptable_workflow() -> Result<()> {
        let parsed = Args::try_parse_from([
            "logos-inspector",
            "cli",
            "backup",
            "download",
            "cid-1",
            "--source-mode",
            "rest",
            "--rest-url",
            "http://storage",
            "--local-only",
        ])?;
        let Some(Mode::Cli(cli)) = parsed.mode else {
            bail!("CLI backup mode was not parsed");
        };
        let CliCommand::Backup {
            command:
                BackupCommand::Download {
                    cid,
                    source_mode,
                    rest_url,
                    local_only,
                },
        } = cli.into_command()
        else {
            bail!("CLI backup download command was not parsed");
        };
        if cid != "cid-1"
            || source_mode != "rest"
            || rest_url.as_deref() != Some("http://storage")
            || !local_only
        {
            bail!("CLI backup download arguments drifted");
        }
        Ok(())
    }

    #[test]
    fn backup_apply_requires_explicit_options() {
        let parsed =
            Args::try_parse_from(["logos-inspector", "cli", "backup", "apply", "backup-1"]);
        assert!(parsed.is_err());
    }

    #[test]
    fn backup_preview_and_apply_parse_explicit_catalog_selection() -> Result<()> {
        for action in ["preview", "apply"] {
            let parsed = Args::try_parse_from([
                "logos-inspector",
                "cli",
                "backup",
                action,
                "backup-1",
                "--options",
                r#"{"settings":"replace"}"#,
                "--wallet-profile",
                r#"{"wallet_home":"/tmp/wallet"}"#,
            ])?;
            let Some(Mode::Cli(cli)) = parsed.mode else {
                bail!("CLI backup {action} mode was not parsed");
            };
            let CliCommand::Backup { command } = cli.into_command() else {
                bail!("CLI backup {action} command was not parsed");
            };
            let import = match command {
                BackupCommand::Preview(import) if action == "preview" => import,
                BackupCommand::Apply(import) if action == "apply" => import,
                _ => bail!("CLI backup {action} selected the wrong subcommand"),
            };
            if import.backup_catalog_id != "backup-1"
                || import.options != r#"{"settings":"replace"}"#
                || import.wallet_profile.as_deref() != Some(r#"{"wallet_home":"/tmp/wallet"}"#)
            {
                bail!("CLI backup {action} arguments drifted");
            }
        }
        Ok(())
    }
}
