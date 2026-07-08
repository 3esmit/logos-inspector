use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    raw_http_json,
    social::social_messages_from_store as decode_social_messages,
    source_routing::{
        self, Args, require_mutating_diagnostics, storage_rest_download_bytes, storage_rest_source,
        storage_rest_upload_bytes,
    },
    support::settings_backup::{export_app_settings_backup, restore_app_settings_backup},
};

use super::super::value::to_value;

pub(super) fn storage_exists(runtime: &Runtime, args: Value) -> Result<Value> {
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

pub(super) fn storage_backup_settings(runtime: &Runtime, args: Value) -> Result<Value> {
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

pub(super) fn storage_restore_settings(runtime: &Runtime, args: Value) -> Result<Value> {
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

pub(super) fn social_messages_from_store(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let topic = args.string(0, "social topic")?;
    let value = args
        .value(1)
        .context("Delivery Store response is required")?;
    let expected_account = args.optional_string(2);
    to_value(decode_social_messages(topic, value, expected_account))
}
