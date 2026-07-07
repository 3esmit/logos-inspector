use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use super::bridge::to_value;
use crate::{
    TransactionSummary, bedrock_wallet_balance, channel_scan, channel_state,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl,
    idl_decode::spel_idl_report,
    inspect_transaction_summary_with_idl,
    inspection::l2::lez::{
        ProgramDecodeCandidate, resolve_account_decode_session, resolve_transaction_decode_session,
    },
    local_devnet_list, local_nodes_status, local_wallet_instruction_preview,
    local_wallet_profile_status,
    modules::{
        blockchain_module_report, delivery_report, logoscore_status_report, modules_report,
        storage_report,
    },
    network_profiles, normalize_program_id_hex, overview, program_file_info, raw_http_json,
    raw_rpc_report,
    settings_backup::{export_app_settings_backup, restore_app_settings_backup},
    social::social_messages_from_store as decode_social_messages,
    source_routing::{
        self, Args, CoreEndpointMode, SourceEndpoint, delivery_source_report,
        require_mutating_diagnostics, source_policy_report, storage_rest_download_bytes,
        storage_rest_source, storage_rest_upload_bytes, storage_source_report,
    },
    state_store::{
        load_idl_state, load_settings_state, load_wallet_state, save_idl_state,
        save_settings_state, save_wallet_state,
    },
    wallet::detected_wallet_profile,
};

pub(super) fn try_handle(runtime: &Runtime, method: &str, args: Value) -> Result<Option<Value>> {
    let value = match method {
        "overview" => {
            let args = Args::new(args)?;
            let value = runtime.block_on(overview(
                args.string(0, "sequencer endpoint")?,
                args.string(1, "indexer endpoint")?,
                args.string(2, "node endpoint")?,
            ));
            to_value(value)?
        }
        "decodeTransactionSummary" => {
            let args = Args::new(args)?;
            let summary: TransactionSummary = serde_json::from_value(
                args.value(0)
                    .cloned()
                    .context("transaction summary is required")?,
            )
            .context("failed to parse transaction summary")?;
            to_value(inspect_transaction_summary_with_idl(
                &summary,
                args.string(1, "IDL JSON")?,
            )?)?
        }
        "decodeAccount" => {
            let args = Args::new(args)?;
            to_value(decode_account_data_hex_with_idl(
                args.string(1, "IDL JSON")?,
                args.optional_string(2),
                args.string(0, "account data hex")?,
                None,
            )?)?
        }
        "resolveAccountDecodeSession" => {
            let args = Args::new(args)?;
            let candidates: Vec<ProgramDecodeCandidate> = serde_json::from_value(
                args.value(2)
                    .cloned()
                    .unwrap_or_else(|| Value::Array(Vec::new())),
            )
            .context("failed to parse decode candidates")?;
            to_value(resolve_account_decode_session(
                args.optional_string(1),
                args.string(0, "account data hex")?,
                &candidates,
            ))?
        }
        "resolveTransactionDecodeSession" => {
            let args = Args::new(args)?;
            let summary: TransactionSummary = serde_json::from_value(
                args.value(0)
                    .cloned()
                    .context("transaction summary is required")?,
            )
            .context("failed to parse transaction summary")?;
            let candidates: Vec<ProgramDecodeCandidate> = serde_json::from_value(
                args.value(1)
                    .cloned()
                    .unwrap_or_else(|| Value::Array(Vec::new())),
            )
            .context("failed to parse decode candidates")?;
            to_value(resolve_transaction_decode_session(&summary, &candidates))?
        }
        "decodeEvent" => {
            let args = Args::new(args)?;
            to_value(decode_event_data_hex_with_idl(
                args.string(1, "IDL JSON")?,
                args.optional_string(2),
                args.string(0, "event data hex")?,
            )?)?
        }
        "channelScan" => {
            let args = Args::new(args)?;
            let source = args.source_endpoint(0, "node endpoint")?;
            require_rpc_source(&source, "channelScan")?;
            to_value(runtime.block_on(channel_scan(
                source.endpoint,
                args.u64(source.next_index, "slot from")?,
                args.u64(source.next_index + 1, "slot to")?,
            ))?)?
        }
        "channelState" => {
            let args = Args::new(args)?;
            let source = args.source_endpoint(0, "node endpoint")?;
            require_rpc_source(&source, "channelState")?;
            to_value(runtime.block_on(channel_state(
                source.endpoint,
                args.string(source.next_index, "channel id")?,
            ))?)?
        }
        "rawRpc" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(raw_rpc_report(
                args.string(0, "RPC endpoint")?,
                args.string(1, "RPC method")?,
                args.json_or_empty_array(2)?,
            ))?)?
        }
        "spelIdl" => {
            let args = Args::new(args)?;
            to_value(spel_idl_report(args.string(0, "IDL JSON")?)?)?
        }
        "programFile" => {
            let args = Args::new(args)?;
            to_value(program_file_info(args.string(0, "program path")?)?)?
        }
        "normalizeProgramId" => {
            let args = Args::new(args)?;
            to_value(normalize_program_id_hex(args.string(0, "program id")?)?)?
        }
        "sourcePolicy" => to_value(source_policy_report(network_profiles()))?,
        "localWalletProfileStatus" => {
            let args = Args::new(args)?;
            to_value(local_wallet_profile_status(
                args.value(0)
                    .cloned()
                    .context("local wallet profile is required")?,
            )?)?
        }
        "localWalletInstructionPreview" => {
            let args = Args::new(args)?;
            to_value(local_wallet_instruction_preview(
                args.value(0)
                    .cloned()
                    .context("IDL instruction request is required")?,
            )?)?
        }
        "localNodesStatus" => {
            let args = Args::new(args)?;
            to_value(local_nodes_status(
                args.optional_string(0).unwrap_or("default"),
            )?)?
        }
        "localDevnetList" => {
            let args = Args::new(args)?;
            to_value(local_devnet_list(
                args.optional_string(0).unwrap_or("default"),
            )?)?
        }
        "bedrockWalletBalance" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(bedrock_wallet_balance(
                args.string(0, "node endpoint")?,
                args.string(1, "wallet public key")?,
                args.optional_string(2),
            ))?)?
        }
        "loadIdlState" => load_idl_state()?,
        "saveIdlState" => {
            let args = Args::new(args)?;
            save_idl_state(args.value(0).context("IDL state is required")?)?
        }
        "loadWalletState" => load_wallet_state()?,
        "detectWalletProfile" => detected_wallet_profile(),
        "saveWalletState" => {
            let args = Args::new(args)?;
            save_wallet_state(args.value(0).context("wallet state is required")?)?
        }
        "loadSettingsState" => load_settings_state()?,
        "saveSettingsState" => {
            let args = Args::new(args)?;
            save_settings_state(args.value(0).context("settings state is required")?)?
        }
        "modules" => to_value(modules_report())?,
        "logoscoreStatus" => to_value(logoscore_status_report())?,
        "blockchainModuleReport" => {
            let args = Args::new(args)?;
            to_value(blockchain_module_report(args.optional_string(0)))?
        }
        "storageReport" => {
            let args = Args::new(args)?;
            to_value(storage_report(
                args.optional_string(0),
                args.optional_bool(1),
            ))?
        }
        "storageSourceReport" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(storage_source_report(
                args.optional_string(0).unwrap_or("rest"),
                args.optional_string(1),
                args.optional_string(2),
                args.optional_string(3),
                args.optional_bool(4),
            )))?
        }
        "deliveryReport" => {
            let args = Args::new(args)?;
            to_value(delivery_report(args.optional_string(0)))?
        }
        "deliverySourceReport" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(delivery_source_report(
                args.optional_string(0).unwrap_or("rest"),
                args.optional_string(1),
                args.optional_string(2),
            )))?
        }
        "storageExists" => storage_exists(runtime, args)?,
        "storageBackupSettings" => storage_backup_settings(runtime, args)?,
        "storageRestoreSettings" => storage_restore_settings(runtime, args)?,
        "socialMessagesFromStore" => social_messages_from_store(args)?,
        _ => return Ok(None),
    };
    Ok(Some(value))
}

fn require_rpc_source(source: &SourceEndpoint<'_>, method: &str) -> Result<()> {
    if source.mode == CoreEndpointMode::Rpc {
        return Ok(());
    }
    bail!(
        "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
        source.module
    )
}

fn storage_exists(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    if source_routing::is_storage_module_source(&args) {
        let cid = args.string(2, "CID")?;
        return to_value(source_routing::call_value(
            source_routing::STORAGE_MODULE,
            "exists",
            &[json!(cid)],
        )?);
    }
    let source = storage_rest_source(&args)?;
    let cid = args.string(source.next_index, "CID")?;
    to_value(runtime.block_on(raw_http_json(
        source.endpoint,
        &format!("/data/{cid}/exists"),
    ))?)
}

fn storage_backup_settings(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    if source_routing::is_storage_module_source(&args) {
        bail!(
            "settings backup through storage_module needs storageUploadDone event correlation to return the final CID; use the Storage app upload flow or Direct REST source for synchronous settings backup"
        );
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "settings backup action")?;
    let encrypted = args.optional_bool(source.next_index + 1);
    let wallet_profile = args.value(source.next_index + 2);
    let block_size = args
        .value(source.next_index + 3)
        .and_then(Value::as_u64)
        .unwrap_or(65_536);
    let payload = export_app_settings_backup(encrypted, wallet_profile)?;
    let bytes =
        serde_json::to_vec_pretty(&payload).context("failed to serialize settings backup")?;
    let upload = runtime
        .block_on(storage_rest_upload_bytes(
            source.endpoint,
            "logos-inspector-settings-backup.json",
            &bytes,
            block_size,
        ))
        .context("failed to upload settings backup through storage REST")?;
    let cid = upload
        .get("cid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(json!({
        "cid": cid,
        "bytes": bytes.len(),
        "endpoint": source.endpoint,
        "encrypted": encrypted,
        "upload": upload,
    }))
}

fn storage_restore_settings(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    if source_routing::is_storage_module_source(&args) {
        bail!(
            "settings restore through storage_module needs storageDownloadDone chunk correlation; use Direct REST source for synchronous settings restore"
        );
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "settings restore action")?;
    let cid = args.string(source.next_index + 1, "backup CID")?;
    let wallet_profile = args.value(source.next_index + 2);
    let local_only = args.optional_bool(source.next_index + 3);
    let bytes = runtime
        .block_on(storage_rest_download_bytes(
            source.endpoint,
            cid,
            local_only,
        ))
        .with_context(|| format!("failed to download settings backup CID {cid}"))?;
    let payload: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("settings backup CID {cid} did not contain JSON"))?;
    let summary = restore_app_settings_backup(&payload, wallet_profile)?;
    Ok(json!({
        "restored": true,
        "cid": cid,
        "bytes": bytes.len(),
        "endpoint": source.endpoint,
        "source": if local_only { "local" } else { "network" },
        "encrypted": summary.encrypted,
        "settings": summary.settings_restored,
        "idls": summary.idl_restored,
        "wallet": summary.wallet_restored,
        "favorites": summary.favorites_count,
        "idl_count": summary.idl_count,
    }))
}

fn social_messages_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    let expected_account = args.optional_string(2);
    to_value(decode_social_messages(topic, value, expected_account))
}
