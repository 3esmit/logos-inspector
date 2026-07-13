use anyhow::{Context as _, Result};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    modules::logos_core::SharedModuleTransport,
    source_routing::storage_layer,
    support::args::Args,
    support::backup_catalog::{
        attach_remote_backup_metadata, backup_payload_bytes, record_remote_settings_backup_payload,
    },
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::with_module_transport("storageExists", storage_exists),
    RuntimeMethodEntry::with_runtime("storageRestoreSettings", storage_restore_settings),
    RuntimeMethodEntry::with_module_transport(
        "storageUploadBackupCatalogEntry",
        storage_upload_backup_catalog_entry,
    ),
    RuntimeMethodEntry::with_module_transport("storageUploadPayload", storage_upload_payload),
];

pub(super) fn storage_exists(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let request = storage_layer::StorageExistsRequest::parse(&args)?;
    to_value(runtime.block_on(request.execute(&module_transport))?)
}

pub(super) fn storage_upload_backup_catalog_entry(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let request = storage_layer::StorageBackupUploadRequest::parse(&args)?;
    let backup_catalog_id = request.backup_catalog_id();
    let bytes = backup_payload_bytes(backup_catalog_id)?;
    let upload = runtime
        .block_on(request.client().upload_bytes(
            &module_transport,
            "logos-inspector-settings-backup.json",
            &bytes,
            request.block_size(),
        ))
        .context("failed to upload settings backup through Storage")?;
    let endpoint = request.client().source();
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
        "endpoint": endpoint,
        "backup_catalog_id": backup_catalog_id,
        "catalog_entry": entry,
        "upload": upload,
    }))
}

pub(super) fn storage_upload_payload(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let request = storage_layer::StoragePayloadUploadRequest::parse(&args)?;
    let bytes = serde_json::to_vec_pretty(request.payload())
        .context("failed to serialize storage payload")?;
    let upload = runtime
        .block_on(request.client().upload_bytes(
            &module_transport,
            request.filename(),
            &bytes,
            request.block_size(),
        ))
        .context("failed to upload payload through Storage")?;
    let endpoint = request.client().source();
    let cid = upload
        .get("cid")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    Ok(json!({
        "cid": cid,
        "bytes": bytes.len(),
        "endpoint": endpoint,
        "filename": request.filename(),
        "upload": upload,
    }))
}

pub(super) fn storage_restore_settings(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let request = storage_layer::StorageRestoreRequest::parse(&args)?;
    let cid = request.cid();
    let local_only = request.local_only();
    let bytes = runtime
        .block_on(request.client().download_bytes(
            cid,
            local_only,
            "settings restore through storage_module needs storageDownloadDone chunk correlation; use Direct REST source for synchronous settings restore",
        ))
        .with_context(|| format!("failed to download settings backup CID {cid}"))?;
    let endpoint = request
        .client()
        .endpoint()
        .context("settings restore requires storage REST source")?;
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
        "endpoint": endpoint,
        "source": if local_only { "local" } else { "network" },
        "encrypted": payload.get("encrypted").and_then(Value::as_bool).unwrap_or(false),
    }))
}
