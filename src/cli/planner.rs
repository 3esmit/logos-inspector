use std::{fs, path::Path};

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use super::args::{BackupCommand, BackupImportArgs, CliCommand, WalletCommand};
use crate::source_routing::{
    DEFAULT_DELIVERY_METRICS_ENDPOINT, DEFAULT_DELIVERY_REST_ENDPOINT,
    DEFAULT_STORAGE_METRICS_ENDPOINT, DEFAULT_STORAGE_REST_ENDPOINT, SourceFamily,
    source_mode_is_token, source_mode_policy,
};
use crate::support::confirmation::ConfirmationPolicy;

#[derive(Debug, Clone, PartialEq)]
pub(super) struct CliInvocation {
    pub(super) method: &'static str,
    pub(super) args: Value,
    completion: CliCompletionPolicy,
}

impl CliInvocation {
    fn json(method: &'static str, args: Value) -> Self {
        Self {
            method,
            args,
            completion: CliCompletionPolicy::Any,
        }
    }

    fn with_completion(mut self, completion: CliCompletionPolicy) -> Self {
        self.completion = completion;
        self
    }

    pub(super) fn into_parts(self) -> (&'static str, Value, CliCompletionPolicy) {
        (self.method, self.args, self.completion)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) enum CliCompletionPolicy {
    Any,
    BackupCatalog,
    BackupDownload { cid: String },
    BackupPreview { backup_catalog_id: String },
    BackupApply { backup_catalog_id: String },
}

impl CliCompletionPolicy {
    pub(super) const fn requires_signal_aware_shutdown(&self) -> bool {
        matches!(self, Self::BackupDownload { .. })
    }

    pub(super) fn validate(&self, value: &Value) -> Result<()> {
        match self {
            Self::Any => Ok(()),
            Self::BackupCatalog => {
                if value.get("version").and_then(Value::as_u64).is_none()
                    || value
                        .get("entries")
                        .is_none_or(|entries| !entries.is_array())
                {
                    anyhow::bail!("backup catalog response is malformed");
                }
                Ok(())
            }
            Self::BackupDownload { cid } => validate_backup_download(value, cid),
            Self::BackupPreview { backup_catalog_id } => {
                if value.get("outcome").and_then(Value::as_str) != Some("preview")
                    || value.get("terminal").and_then(Value::as_bool) != Some(false)
                    || value.get("import_plan").and_then(Value::as_bool) != Some(true)
                    || value.get("backupCatalogId").and_then(Value::as_str)
                        != Some(backup_catalog_id)
                {
                    anyhow::bail!("backup import preview did not return a valid import plan");
                }
                Ok(())
            }
            Self::BackupApply { backup_catalog_id } => {
                if value.get("outcome").and_then(Value::as_str) != Some("applied")
                    || value.get("terminal").and_then(Value::as_bool) != Some(true)
                    || value.get("applied").and_then(Value::as_bool) != Some(true)
                    || value.get("recoveryRequired").and_then(Value::as_bool) != Some(false)
                    || value.get("backupCatalogId").and_then(Value::as_str)
                        != Some(backup_catalog_id)
                {
                    let outcome = value
                        .get("outcome")
                        .and_then(Value::as_str)
                        .unwrap_or("malformed");
                    anyhow::bail!("backup import ended with outcome `{outcome}`");
                }
                Ok(())
            }
        }
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
                json!([storage_report_initialization(
                    &source_mode,
                    rest_url.as_deref(),
                    metrics_url.as_deref(),
                    cid.as_deref(),
                )]),
            )),
            CliCommand::Messaging {
                source_mode,
                rest_url,
                metrics_url,
            } => Ok(CliInvocation::json(
                "deliverySourceReport",
                json!([delivery_report_initialization(
                    &source_mode,
                    rest_url.as_deref(),
                    metrics_url.as_deref(),
                )]),
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
            CliCommand::Backup { command } => command.invocation(),
        }
    }
}

impl BackupCommand {
    fn invocation(self) -> Result<CliInvocation> {
        match self {
            Self::List => Ok(CliInvocation::json("loadBackupCatalog", json!([]))
                .with_completion(CliCompletionPolicy::BackupCatalog)),
            Self::Download {
                cid,
                source_mode,
                rest_url,
                local_only,
            } => {
                let cid = required_cli_text(cid, "backup CID")?;
                let adapter = backup_storage_initialization(&source_mode, rest_url.as_deref())?;
                Ok(CliInvocation::json(
                    "storageDownloadBackupCatalogEntry",
                    json!([{
                        "adapter": adapter,
                        "payload": {
                            "cid": cid,
                            "local_only": local_only,
                        },
                        "mutating_enabled": false,
                    }]),
                )
                .with_completion(CliCompletionPolicy::BackupDownload { cid }))
            }
            Self::Preview(args) => backup_import_invocation(
                "settingsBackupImportPreview",
                args,
                BackupImportCompletion::Preview,
            ),
            Self::Apply(args) => backup_import_invocation(
                "settingsBackupImportApply",
                args,
                BackupImportCompletion::Apply,
            ),
        }
    }
}

fn backup_import_invocation(
    method: &'static str,
    args: BackupImportArgs,
    completion: BackupImportCompletion,
) -> Result<CliInvocation> {
    let backup_catalog_id = required_cli_text(args.backup_catalog_id, "backup catalog id")?;
    let options = parse_json_or_path(&args.options, "backup import options")?;
    if !options.is_object() {
        anyhow::bail!("backup import options must be a JSON object");
    }
    let wallet_profile = match args.wallet_profile {
        Some(value) => {
            let profile = parse_json_or_path(&value, "wallet profile")?;
            if !profile.is_object() {
                anyhow::bail!("wallet profile must be a JSON object");
            }
            profile
        }
        None => json!({}),
    };
    let completion = match completion {
        BackupImportCompletion::Preview => CliCompletionPolicy::BackupPreview {
            backup_catalog_id: backup_catalog_id.clone(),
        },
        BackupImportCompletion::Apply => CliCompletionPolicy::BackupApply {
            backup_catalog_id: backup_catalog_id.clone(),
        },
    };
    Ok(
        CliInvocation::json(method, json!([backup_catalog_id, wallet_profile, options]))
            .with_completion(completion),
    )
}

#[derive(Debug, Clone, Copy)]
enum BackupImportCompletion {
    Preview,
    Apply,
}

fn backup_storage_initialization(source_mode: &str, rest_url: Option<&str>) -> Result<Value> {
    let source_mode = canonical_source_mode(SourceFamily::Storage, source_mode);
    match source_mode.as_str() {
        "logoscore_cli" => {
            if rest_url.is_some_and(|value| !value.trim().is_empty()) {
                anyhow::bail!("--rest-url is only valid with --source-mode rest");
            }
            Ok(json!({ "source_mode": source_mode, "inputs": {} }))
        }
        "rest" => Ok(json!({
            "source_mode": source_mode,
            "inputs": {
                "rest_endpoint": rest_url.unwrap_or(DEFAULT_STORAGE_REST_ENDPOINT),
            }
        })),
        _ => anyhow::bail!(
            "backup download source must be `logoscore_cli` or `rest`, not `{source_mode}`"
        ),
    }
}

fn validate_backup_download(value: &Value, cid: &str) -> Result<()> {
    let catalog_id = value
        .get("backup_catalog_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let payload_id = value
        .get("payload_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let entry_catalog_id = value
        .pointer("/catalog_entry/backup_catalog_id")
        .and_then(Value::as_str);
    let entry_payload_id = value
        .pointer("/catalog_entry/payload_id")
        .and_then(Value::as_str);
    let encrypted = value.get("encrypted").and_then(Value::as_bool);
    let entry_encrypted = value
        .pointer("/catalog_entry/encrypted")
        .and_then(Value::as_bool);
    if value.get("downloaded").and_then(Value::as_bool) != Some(true)
        || value.get("restored").and_then(Value::as_bool) != Some(false)
        || value.get("cid").and_then(Value::as_str) != Some(cid)
        || value
            .pointer("/catalog_entry/remote/cid")
            .and_then(Value::as_str)
            != Some(cid)
        || catalog_id.is_none()
        || payload_id.is_none()
        || catalog_id != entry_catalog_id
        || payload_id != entry_payload_id
        || value
            .pointer("/catalog_entry/remote/provider")
            .and_then(Value::as_str)
            != Some("logos_storage")
        || encrypted.is_none()
        || encrypted != entry_encrypted
    {
        anyhow::bail!("backup download did not produce a verified download-only catalog entry");
    }
    Ok(())
}

fn required_cli_text(value: String, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        anyhow::bail!("{label} is required");
    }
    Ok(value.to_owned())
}

fn parse_json_or_path(value: &str, label: &str) -> Result<Value> {
    let path = Path::new(value);
    let text = if path.exists() {
        fs::read_to_string(path)
            .with_context(|| format!("failed to read {label} at {}", path.display()))?
    } else {
        value.to_owned()
    };
    serde_json::from_str(&text).with_context(|| format!("failed to parse {label} JSON"))
}

fn delivery_report_initialization(
    source_mode: &str,
    rest_url: Option<&str>,
    metrics_url: Option<&str>,
) -> Value {
    let source_mode = canonical_source_mode(SourceFamily::Delivery, source_mode);
    let inputs = match source_mode.as_str() {
        "rest" | "network-monitor" => json!({
            "rest_endpoint": rest_url.unwrap_or(DEFAULT_DELIVERY_REST_ENDPOINT),
            "metrics_endpoint": metrics_url.unwrap_or_default(),
        }),
        "metrics" => json!({
            "metrics_endpoint": metrics_url.unwrap_or(DEFAULT_DELIVERY_METRICS_ENDPOINT),
        }),
        _ => json!({}),
    };
    json!({
        "source_mode": source_mode,
        "inputs": inputs,
    })
}

fn storage_report_initialization(
    source_mode: &str,
    rest_url: Option<&str>,
    metrics_url: Option<&str>,
    cid: Option<&str>,
) -> Value {
    let source_mode = canonical_source_mode(SourceFamily::Storage, source_mode);
    let inputs = match source_mode.as_str() {
        "rest" => json!({
            "rest_endpoint": rest_url.unwrap_or(DEFAULT_STORAGE_REST_ENDPOINT),
            "metrics_endpoint": metrics_url.unwrap_or_default(),
        }),
        "metrics" => json!({
            "metrics_endpoint": metrics_url.unwrap_or(DEFAULT_STORAGE_METRICS_ENDPOINT),
        }),
        _ => json!({}),
    };
    json!({
        "source_mode": source_mode,
        "inputs": inputs,
        "options": {
            "cid": cid.unwrap_or_default(),
            "privileged_debug_enabled": false,
        },
    })
}

fn canonical_source_mode(family: SourceFamily, value: &str) -> String {
    if source_mode_is_token(family, value) {
        source_mode_policy(family, value).key.to_owned()
    } else {
        value.trim().to_ascii_lowercase()
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

    #[test]
    fn messaging_command_plans_one_structured_adapter_initialization() -> Result<()> {
        let invocation = CliCommand::Messaging {
            source_mode: "logoscore_cli".to_owned(),
            rest_url: None,
            metrics_url: None,
        }
        .invocation()?;

        ensure!(
            invocation.method == "deliverySourceReport",
            "unexpected method"
        );
        ensure!(
            invocation.args
                == json!([{
                    "source_mode": "logoscore_cli",
                    "inputs": {}
                }]),
            "unexpected args: {}",
            invocation.args
        );
        Ok(())
    }

    #[test]
    fn storage_rest_command_supplies_default_endpoint_in_structured_adapter() -> Result<()> {
        let invocation = CliCommand::Storage {
            cid: Some("cid-a".to_owned()),
            source_mode: "rest".to_owned(),
            rest_url: None,
            metrics_url: None,
        }
        .invocation()?;

        ensure!(
            invocation.method == "storageSourceReport",
            "unexpected method"
        );
        ensure!(
            invocation.args
                == json!([{
                    "source_mode": "rest",
                    "inputs": {
                        "rest_endpoint": crate::source_routing::DEFAULT_STORAGE_REST_ENDPOINT,
                        "metrics_endpoint": ""
                    },
                    "options": {
                        "cid": "cid-a",
                        "privileged_debug_enabled": false
                    }
                }]),
            "unexpected args: {}",
            invocation.args
        );
        Ok(())
    }

    #[test]
    fn backup_download_command_plans_one_structured_logoscore_request() -> Result<()> {
        let invocation = BackupCommand::Download {
            cid: " cid-backup ".to_owned(),
            source_mode: "logoscore_cli".to_owned(),
            rest_url: None,
            local_only: true,
        }
        .invocation()?;

        ensure!(
            invocation.method == "storageDownloadBackupCatalogEntry",
            "unexpected backup download method"
        );
        ensure!(
            invocation.args
                == json!([{
                    "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
                    "payload": { "cid": "cid-backup", "local_only": true },
                    "mutating_enabled": false
                }]),
            "unexpected backup download args: {}",
            invocation.args
        );
        invocation.completion.validate(&json!({
            "downloaded": true,
            "restored": false,
            "cid": "cid-backup",
            "backup_catalog_id": "backup-1",
            "payload_id": "sha256:1",
            "encrypted": false,
            "catalog_entry": {
                "backup_catalog_id": "backup-1",
                "payload_id": "sha256:1",
                "encrypted": false,
                "remote": { "cid": "cid-backup", "provider": "logos_storage" }
            }
        }))
    }

    #[test]
    fn backup_rest_download_and_catalog_list_use_shared_methods() -> Result<()> {
        let download = BackupCommand::Download {
            cid: "cid-rest".to_owned(),
            source_mode: "rest".to_owned(),
            rest_url: Some("http://storage.example".to_owned()),
            local_only: false,
        }
        .invocation()?;
        let list = BackupCommand::List.invocation()?;

        ensure!(
            download.args.pointer("/0/adapter")
                == Some(&json!({
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://storage.example" }
                })),
            "REST backup adapter drifted: {}",
            download.args
        );
        ensure!(
            list.method == "loadBackupCatalog" && list.args == json!([]),
            "backup list bypassed catalog method"
        );
        list.completion
            .validate(&json!({ "version": 1, "entries": [] }))
    }

    #[test]
    fn backup_preview_and_apply_preserve_full_json_or_path_options() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let options_path = directory.path().join("options.json");
        fs::write(
            &options_path,
            r#"{
                "settings":"replace",
                "favorites":"merge",
                "items":{"favorites":["favorite-a"]},
                "conflicts":{"favorites":{"favorite-a":"replace_existing"}}
            }"#,
        )?;
        let preview = BackupCommand::Preview(BackupImportArgs {
            backup_catalog_id: "backup-1".to_owned(),
            options: options_path.display().to_string(),
            wallet_profile: None,
        })
        .invocation()?;
        let apply = BackupCommand::Apply(BackupImportArgs {
            backup_catalog_id: "backup-1".to_owned(),
            options: r#"{"idl_registry":"merge"}"#.to_owned(),
            wallet_profile: Some(r#"{"wallet_home":"/tmp/wallet"}"#.to_owned()),
        })
        .invocation()?;

        ensure!(
            preview.method == "settingsBackupImportPreview"
                && preview
                    .args
                    .pointer("/2/items/favorites/0")
                    .and_then(Value::as_str)
                    == Some("favorite-a")
                && preview.args.pointer("/1") == Some(&json!({})),
            "backup preview lost shared import options: {}",
            preview.args
        );
        ensure!(
            apply.method == "settingsBackupImportApply"
                && apply.args.pointer("/1/wallet_home").and_then(Value::as_str)
                    == Some("/tmp/wallet")
                && apply
                    .args
                    .pointer("/2/idl_registry")
                    .and_then(Value::as_str)
                    == Some("merge"),
            "backup apply lost shared import inputs: {}",
            apply.args
        );
        Ok(())
    }

    #[test]
    fn backup_cli_completion_policies_reject_direct_restore_and_unapplied_outcomes() -> Result<()> {
        let download = CliCompletionPolicy::BackupDownload {
            cid: "cid-1".to_owned(),
        };
        ensure!(
            download
                .validate(&json!({
                    "downloaded": true,
                    "restored": true,
                    "cid": "cid-1",
                    "backup_catalog_id": "backup-1",
                    "payload_id": "sha256:1",
                    "encrypted": false,
                    "catalog_entry": {
                        "backup_catalog_id": "backup-1",
                        "payload_id": "sha256:1",
                        "encrypted": false,
                        "remote": { "cid": "cid-1", "provider": "logos_storage" }
                    }
                }))
                .is_err(),
            "CLI accepted one-step remote restore"
        );
        for outcome in ["blocked", "rolled_back", "recovery_required"] {
            ensure!(
                CliCompletionPolicy::BackupApply {
                    backup_catalog_id: "backup-1".to_owned(),
                }
                .validate(&json!({
                    "outcome": outcome,
                    "terminal": true,
                    "applied": false,
                    "recoveryRequired": outcome == "recovery_required",
                    "backupCatalogId": "backup-1"
                }))
                .is_err(),
                "CLI accepted non-applied outcome `{outcome}`"
            );
        }
        CliCompletionPolicy::BackupApply {
            backup_catalog_id: "backup-1".to_owned(),
        }
        .validate(&json!({
            "outcome": "applied",
            "terminal": true,
            "applied": true,
            "recoveryRequired": false,
            "backupCatalogId": "backup-1"
        }))
    }

    #[test]
    fn backup_cli_completion_correlates_catalog_and_payload_identity() -> Result<()> {
        let policy = CliCompletionPolicy::BackupDownload {
            cid: "cid-1".to_owned(),
        };
        let valid = json!({
            "downloaded": true,
            "restored": false,
            "cid": "cid-1",
            "backup_catalog_id": "backup-1",
            "payload_id": "sha256:1",
            "encrypted": false,
            "catalog_entry": {
                "backup_catalog_id": "backup-1",
                "payload_id": "sha256:1",
                "encrypted": false,
                "remote": { "cid": "cid-1", "provider": "logos_storage" }
            }
        });
        policy.validate(&valid)?;

        for pointer in [
            "/catalog_entry/backup_catalog_id",
            "/catalog_entry/payload_id",
            "/catalog_entry/remote/cid",
            "/catalog_entry/remote/provider",
            "/catalog_entry/encrypted",
        ] {
            let mut mismatched = valid.clone();
            *mismatched
                .pointer_mut(pointer)
                .with_context(|| format!("missing completion fixture field {pointer}"))? =
                if pointer == "/catalog_entry/encrypted" {
                    json!(true)
                } else {
                    json!("foreign")
                };
            ensure!(
                policy.validate(&mismatched).is_err(),
                "CLI accepted mismatched backup identity at {pointer}"
            );
        }
        for policy in [
            CliCompletionPolicy::BackupPreview {
                backup_catalog_id: "backup-1".to_owned(),
            },
            CliCompletionPolicy::BackupApply {
                backup_catalog_id: "backup-1".to_owned(),
            },
        ] {
            let value = match policy {
                CliCompletionPolicy::BackupPreview { .. } => json!({
                    "outcome": "preview",
                    "terminal": false,
                    "import_plan": true,
                    "backupCatalogId": "foreign"
                }),
                CliCompletionPolicy::BackupApply { .. } => json!({
                    "outcome": "applied",
                    "terminal": true,
                    "applied": true,
                    "recoveryRequired": false,
                    "backupCatalogId": "foreign"
                }),
                _ => unreachable!(),
            };
            ensure!(
                policy.validate(&value).is_err(),
                "CLI accepted a foreign selected catalog ID"
            );
        }
        Ok(())
    }
}
