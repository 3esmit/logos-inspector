use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    raw_http_json,
    source_routing::{
        self, require_mutating_diagnostics, storage_rest_download_bytes, storage_rest_source,
        storage_rest_upload_bytes,
    },
    support::args::Args,
    support::backup_catalog::{
        attach_remote_backup_metadata, backup_payload_bytes, record_remote_settings_backup_payload,
    },
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::with_runtime("storageExists", storage_exists),
    RuntimeMethodEntry::with_runtime("storageRestoreSettings", storage_restore_settings),
    RuntimeMethodEntry::with_runtime(
        "storageUploadBackupCatalogEntry",
        storage_upload_backup_catalog_entry,
    ),
    RuntimeMethodEntry::with_runtime("storageUploadPayload", storage_upload_payload),
];

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

pub(super) fn storage_upload_backup_catalog_entry(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    if source_routing::is_storage_module_source(&args) {
        bail!(
            "settings backup through storage_module needs storageUploadDone event correlation to return the final CID; use the Storage app upload flow or Direct REST source for synchronous settings backup"
        );
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "settings backup action")?;
    let backup_catalog_id = args.string(source.next_index + 1, "backup catalog id")?;
    let block_size = args
        .value(source.next_index + 2)
        .and_then(Value::as_u64)
        .unwrap_or(65_536);
    let bytes = backup_payload_bytes(backup_catalog_id)?;
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
    let entry = if cid.is_empty() {
        None
    } else {
        Some(attach_remote_backup_metadata(
            backup_catalog_id,
            &cid,
            Some("logos_storage"),
        )?)
    };
    Ok(json!({
        "cid": cid,
        "bytes": bytes.len(),
        "endpoint": source.endpoint,
        "backup_catalog_id": backup_catalog_id,
        "catalog_entry": entry,
        "upload": upload,
    }))
}

pub(super) fn storage_upload_payload(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    if source_routing::is_storage_module_source(&args) {
        bail!(
            "payload upload through storage_module needs storageUploadDone event correlation to return the final CID; use Direct REST source for synchronous payload upload"
        );
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "storage payload upload")?;
    let filename = args.string(source.next_index + 1, "payload filename")?;
    let payload = args
        .value(source.next_index + 2)
        .context("payload JSON is required")?;
    let block_size = args
        .value(source.next_index + 3)
        .and_then(Value::as_u64)
        .unwrap_or(65_536);
    let bytes =
        serde_json::to_vec_pretty(payload).context("failed to serialize storage payload")?;
    let upload = runtime
        .block_on(storage_rest_upload_bytes(
            source.endpoint,
            filename,
            &bytes,
            block_size,
        ))
        .context("failed to upload payload through storage REST")?;
    let cid = upload
        .get("cid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(json!({
        "cid": cid,
        "bytes": bytes.len(),
        "endpoint": source.endpoint,
        "filename": filename,
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
    let mut cid_index = source.next_index;
    if args.value(cid_index).is_some_and(Value::is_boolean) {
        cid_index += 1;
    }
    let cid = args.string(cid_index, "backup CID")?;
    let local_only = if args.value(cid_index + 1).is_some_and(Value::is_boolean) {
        args.optional_bool(cid_index + 1)
    } else {
        args.optional_bool(cid_index + 2)
    };
    let bytes = runtime
        .block_on(storage_rest_download_bytes(
            source.endpoint,
            cid,
            local_only,
        ))
        .with_context(|| format!("failed to download settings backup CID {cid}"))?;
    let payload: Value = serde_json::from_slice(&bytes)
        .with_context(|| format!("settings backup CID {cid} did not contain JSON"))?;
    let entry = record_remote_settings_backup_payload(
        Some(&format!("Remote backup {cid}")),
        &payload,
        cid,
        Some("logos_storage"),
    )
    .context("failed to record downloaded backup in local catalog")?;
    Ok(json!({
        "downloaded": true,
        "restored": false,
        "cid": cid,
        "backup_catalog_id": entry.backup_catalog_id,
        "payload_id": entry.payload_id,
        "catalog_entry": entry,
        "bytes": bytes.len(),
        "endpoint": source.endpoint,
        "source": if local_only { "local" } else { "network" },
        "encrypted": payload.get("encrypted").and_then(Value::as_bool).unwrap_or(false),
    }))
}
