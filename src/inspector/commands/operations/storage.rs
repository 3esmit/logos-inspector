use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;

#[cfg(test)]
use crate::support::settings_backup::SETTINGS_BACKUP_MAX_BYTES;
use crate::{
    modules::logos_core::SharedModuleTransport,
    source_routing::storage_layer,
    support::backup_catalog::{
        attach_remote_backup_metadata, backup_payload_bytes,
        record_remote_settings_backup_payload_in_dir,
    },
    support::settings_backup::ensure_settings_backup_size,
    support::state_store::config_dir,
};

use super::spec::{
    AffectedContextField, AffectedContextKey, OperationClass, OperationCommand,
    OperationDefinition, OperationExclusiveGroup, OperationMethod,
};
use super::{
    RuntimeOperationRegistry, RuntimeOperationRequest, identity::RuntimeOperationId,
    outcome::RuntimeOperationOutcome, transition::RuntimeOperationTransition,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StorageCommand {
    Manifests,
    DownloadManifest,
    Fetch,
    UploadUrl,
    UploadPayload,
    UploadBackupCatalogEntry,
    DownloadBackupCatalogEntry,
    DownloadToUrl,
    Remove,
}

impl StorageCommand {
    pub(super) const fn method(self) -> OperationMethod {
        match self {
            Self::Manifests => OperationMethod::StorageManifests,
            Self::DownloadManifest => OperationMethod::StorageDownloadManifest,
            Self::Fetch => OperationMethod::StorageFetch,
            Self::UploadUrl => OperationMethod::StorageUploadUrl,
            Self::UploadPayload => OperationMethod::StorageUploadPayload,
            Self::UploadBackupCatalogEntry => OperationMethod::StorageUploadBackupCatalogEntry,
            Self::DownloadBackupCatalogEntry => OperationMethod::StorageDownloadBackupCatalogEntry,
            Self::DownloadToUrl => OperationMethod::StorageDownloadToUrl,
            Self::Remove => OperationMethod::StorageRemove,
        }
    }

    const fn operation(self) -> Option<storage_layer::StorageOperation> {
        match self {
            Self::Manifests => Some(storage_layer::StorageOperation::Manifests),
            Self::DownloadManifest => Some(storage_layer::StorageOperation::DownloadManifest),
            Self::Fetch => Some(storage_layer::StorageOperation::Fetch),
            Self::UploadUrl => Some(storage_layer::StorageOperation::Upload),
            Self::UploadPayload
            | Self::UploadBackupCatalogEntry
            | Self::DownloadBackupCatalogEntry => None,
            Self::DownloadToUrl => Some(storage_layer::StorageOperation::Download),
            Self::Remove => Some(storage_layer::StorageOperation::Remove),
        }
    }
}

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::Manifests),
        "storageManifests",
        "Storage manifests",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
    ]),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::DownloadManifest),
        "storageDownloadManifest",
        "Storage manifest",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::Cid),
    ]),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::Fetch),
        "storageFetch",
        "Storage fetch",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::Cid),
    ]),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::UploadUrl),
        "storageUploadUrl",
        "Storage upload",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::Path),
    ]),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::UploadPayload),
        "storageUploadPayload",
        "Storage payload upload",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::Filename),
    ]),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::UploadBackupCatalogEntry),
        "storageUploadBackupCatalogEntry",
        "Backup upload",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::BackupCatalogId),
    ]),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::DownloadBackupCatalogEntry),
        "storageDownloadBackupCatalogEntry",
        "Backup download",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::Cid),
        AffectedContextField::required(AffectedContextKey::DownloadScope),
    ]),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::DownloadToUrl),
        "storageDownloadToUrl",
        "Storage download",
        OperationClass::Mutating,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::Cid),
        AffectedContextField::required(AffectedContextKey::Path),
    ])
    .cancellable(OperationExclusiveGroup::StorageDownload),
    OperationDefinition::new(
        OperationCommand::Storage(StorageCommand::Remove),
        "storageRemove",
        "Storage remove",
        OperationClass::Destructive,
    )
    .with_context_inputs(&[
        AffectedContextField::required(AffectedContextKey::Source),
        AffectedContextField::optional(AffectedContextKey::Endpoint),
        AffectedContextField::required(AffectedContextKey::Cid),
    ]),
];

pub(super) async fn execute(
    command: StorageCommand,
    request: &RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &RuntimeOperationId,
    cancel_requested: &AtomicBool,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    match command {
        StorageCommand::UploadPayload => {
            return execute_payload_upload(request, module_transport).await;
        }
        StorageCommand::UploadBackupCatalogEntry => {
            return execute_backup_catalog_upload(request, module_transport).await;
        }
        StorageCommand::DownloadBackupCatalogEntry => {
            return execute_backup_catalog_download(request).await;
        }
        _ => {}
    }
    let operation = command
        .operation()
        .context("storage command has no standard operation")?;
    let request =
        storage_layer::StorageOperationRequest::parse(request.node_request()?, operation)?;
    match storage_layer::execute_operation(request, module_transport).await? {
        storage_layer::StorageOperationOutput::Outcome(outcome) => Ok(outcome.into()),
        storage_layer::StorageOperationOutput::Download(download) => {
            let cid = download.cid().to_owned();
            let path = download.path().to_owned();
            storage_rest_download_tracked(&download, registry, operation_id, cancel_requested)
                .await
                .with_context(|| format!("failed to download storage CID {cid} to `{path}`"))
                .map(RuntimeOperationOutcome::Completed)
        }
    }
}

pub(super) fn add_operation_context(
    command: StorageCommand,
    request: &RuntimeOperationRequest,
    context: &mut serde_json::Map<String, Value>,
) -> Result<()> {
    match command {
        StorageCommand::UploadPayload => {
            let upload =
                storage_layer::StoragePayloadUploadRequest::parse_request(request.node_request()?)?;
            context.insert("filename".to_owned(), json!(upload.filename()));
            return Ok(());
        }
        StorageCommand::UploadBackupCatalogEntry => {
            let upload =
                storage_layer::StorageBackupUploadRequest::parse_request(request.node_request()?)?;
            context.insert(
                "backupCatalogId".to_owned(),
                json!(upload.backup_catalog_id()),
            );
            return Ok(());
        }
        StorageCommand::DownloadBackupCatalogEntry => {
            let download = storage_layer::StorageBackupDownloadRequest::parse_request(
                request.node_request()?,
            )?;
            context.insert("cid".to_owned(), json!(download.cid()));
            context.insert("downloadScope".to_owned(), json!(download.download_scope()));
            return Ok(());
        }
        _ => {}
    }
    let operation = command
        .operation()
        .context("storage command has no standard operation")?;
    let operation_request =
        storage_layer::StorageOperationRequest::parse(request.node_request()?, operation)?;
    context.extend(operation_request.context().clone());
    Ok(())
}

pub(super) fn validate(command: StorageCommand, request: &RuntimeOperationRequest) -> Result<()> {
    match command {
        StorageCommand::UploadPayload => {
            return storage_layer::StoragePayloadUploadRequest::parse_request(
                request.node_request()?,
            )
            .map(|_| ());
        }
        StorageCommand::UploadBackupCatalogEntry => {
            return storage_layer::StorageBackupUploadRequest::parse_request(
                request.node_request()?,
            )
            .map(|_| ());
        }
        StorageCommand::DownloadBackupCatalogEntry => {
            return storage_layer::StorageBackupDownloadRequest::parse_request(
                request.node_request()?,
            )
            .map(|_| ());
        }
        _ => {}
    }
    let operation = command
        .operation()
        .context("storage command has no standard operation")?;
    storage_layer::StorageOperationRequest::parse(request.node_request()?, operation).map(|_| ())
}

async fn execute_payload_upload(
    request: &RuntimeOperationRequest,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    let request =
        storage_layer::StoragePayloadUploadRequest::parse_request(request.node_request()?)?;
    request
        .client()
        .ensure_managed_byte_upload_supported(&module_transport)?;
    let payload = request.payload().clone();
    let bytes = tokio::task::spawn_blocking(move || serde_json::to_vec_pretty(&payload))
        .await
        .context("storage payload serialization worker failed")?
        .context("failed to serialize storage payload")?;
    let upload = request
        .client()
        .upload_bytes(
            &module_transport,
            request.filename(),
            &bytes,
            request.block_size(),
        )
        .await
        .context("failed to upload payload through Storage")?;
    let cid = required_payload_upload_cid(&upload)?;
    Ok(RuntimeOperationOutcome::Completed(json!({
        "cid": cid,
        "bytes": bytes.len(),
        "endpoint": request.client().source(),
        "filename": request.filename(),
        "upload": upload,
    })))
}

fn required_payload_upload_cid(upload: &Value) -> Result<String> {
    upload
        .get("cid")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .context("Storage payload upload returned no CID")
}

async fn execute_backup_catalog_upload(
    request: &RuntimeOperationRequest,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    let request =
        storage_layer::StorageBackupUploadRequest::parse_request(request.node_request()?)?;
    request
        .client()
        .ensure_managed_byte_upload_supported(&module_transport)?;
    let backup_catalog_id = request.backup_catalog_id().to_owned();
    let payload_catalog_id = backup_catalog_id.clone();
    let bytes = tokio::task::spawn_blocking(move || backup_payload_bytes(&payload_catalog_id))
        .await
        .context("backup payload worker failed")??;
    ensure_settings_backup_size(bytes.len())?;
    let upload = request
        .client()
        .upload_bytes(
            &module_transport,
            "logos-inspector-settings-backup.json",
            &bytes,
            request.block_size(),
        )
        .await
        .context("failed to upload settings backup through Storage")?;
    let cid = required_backup_upload_cid(&upload)?;
    let metadata_catalog_id = backup_catalog_id.clone();
    let metadata_cid = cid.clone();
    let catalog_entry = tokio::task::spawn_blocking(move || {
        attach_remote_backup_metadata(&metadata_catalog_id, &metadata_cid, Some("logos_storage"))
    })
    .await
    .context("backup catalog metadata worker failed")??;
    Ok(RuntimeOperationOutcome::Completed(json!({
        "cid": cid,
        "bytes": bytes.len(),
        "endpoint": request.client().source(),
        "backup_catalog_id": backup_catalog_id,
        "catalog_entry": catalog_entry,
        "upload": upload,
    })))
}

async fn execute_backup_catalog_download(
    request: &RuntimeOperationRequest,
) -> Result<RuntimeOperationOutcome> {
    execute_backup_catalog_download_in_dir(request, None).await
}

async fn execute_backup_catalog_download_in_dir(
    request: &RuntimeOperationRequest,
    catalog_base_dir: Option<PathBuf>,
) -> Result<RuntimeOperationOutcome> {
    let request =
        storage_layer::StorageBackupDownloadRequest::parse_request(request.node_request()?)?;
    let cid = request.cid().to_owned();
    let local_only = request.local_only();
    let bytes = request
        .client()
        .download_backup_bytes(&cid, local_only)
        .await
        .with_context(|| format!("failed to download settings backup CID {cid}"))?;
    let endpoint = request
        .client()
        .endpoint()
        .context("settings backup download requires storage REST source")?
        .to_owned();
    let result = tokio::task::spawn_blocking(move || {
        let base_dir = match catalog_base_dir {
            Some(base_dir) => base_dir,
            None => config_dir()?,
        };
        downloaded_backup_result(&base_dir, &bytes, &cid, &endpoint, local_only)
    })
    .await
    .context("backup download recorder worker failed")??;
    Ok(RuntimeOperationOutcome::Completed(result))
}

fn downloaded_backup_result(
    base_dir: &Path,
    bytes: &[u8],
    cid: &str,
    endpoint: &str,
    local_only: bool,
) -> Result<Value> {
    let payload: Value = serde_json::from_slice(bytes)
        .with_context(|| format!("settings backup CID {cid} did not contain JSON"))?;
    let entry = record_remote_settings_backup_payload_in_dir(
        base_dir,
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

fn required_backup_upload_cid(upload: &Value) -> Result<String> {
    upload
        .get("cid")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .context("Storage backup upload returned no CID")
}

pub(super) async fn storage_rest_download_tracked(
    request: &storage_layer::StorageDownloadRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &RuntimeOperationId,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    if cancel_requested.load(Ordering::Relaxed) {
        bail!("storage download canceled");
    }
    let response = storage_layer::download_response(request).await?;
    registry.transition(
        operation_id,
        RuntimeOperationTransition::Progress {
            bytes_written: 0,
            content_length: response.content_length(),
        },
    )?;
    let path = request.path();
    let temp_path = format!("{path}.part");
    let mut file = tokio::fs::File::create(&temp_path)
        .await
        .with_context(|| format!("failed to create download file `{temp_path}`"))?;
    let mut response = response;
    let mut bytes = 0_u64;
    let result = async {
        while let Some(chunk) = response
            .chunk()
            .await
            .context("failed to read storage download response chunk")?
        {
            if cancel_requested.load(Ordering::Relaxed) {
                bail!("storage download canceled");
            }
            file.write_all(&chunk)
                .await
                .with_context(|| format!("failed to write download file `{temp_path}`"))?;
            bytes = bytes.saturating_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX));
            registry.transition(
                operation_id,
                RuntimeOperationTransition::Progress {
                    bytes_written: bytes,
                    content_length: None,
                },
            )?;
        }
        file.flush()
            .await
            .with_context(|| format!("failed to flush download file `{temp_path}`"))?;
        Ok::<(), anyhow::Error>(())
    }
    .await;
    drop(file);
    if let Err(error) = result {
        let _ignored = tokio::fs::remove_file(&temp_path).await;
        return Err(error);
    }
    if cancel_requested.load(Ordering::Relaxed) {
        let _ignored = tokio::fs::remove_file(&temp_path).await;
        bail!("storage download canceled");
    }
    tokio::fs::rename(&temp_path, path)
        .await
        .with_context(|| format!("failed to move `{temp_path}` to `{path}`"))?;
    Ok(json!({
        "cid": request.cid(),
        "path": path,
        "bytes": bytes,
        "source": if request.local_only() { "local" } else { "network" },
        "endpoint": request.endpoint(),
    }))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::Arc,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Context as _, Result};
    use serde_json::json;

    use super::*;
    use crate::{
        inspector::commands::operations::runtime_operation_request_from_value,
        modules::logos_core::UnavailableModuleTransport,
    };

    #[tokio::test]
    async fn backup_upload_executor_rejects_basecamp_before_catalog_read() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadBackupCatalogEntry",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": {
                "backup_catalog_id": "missing-backup-must-not-be-read",
                "block_size": 65536
            }
        }))?;
        let transport: SharedModuleTransport =
            Arc::new(UnavailableModuleTransport::basecamp_protocol_gate());

        let error = execute_backup_catalog_upload(&request, transport)
            .await
            .err()
            .context("Basecamp backup upload should fail")?;

        anyhow::ensure!(
            error.to_string()
                == "Basecamp module source does not support Inspector-managed byte staging",
            "unexpected executor error: {error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn backup_download_executor_records_rest_payload_with_download_only_result() -> Result<()>
    {
        let payload = json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        });
        let body = serde_json::to_vec(&payload)?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let response_body = body.clone();
        let server = thread::spawn(move || -> Result<Vec<u8>> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let request = read_http_headers(&mut stream)?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            )?;
            stream.write_all(&response_body)?;
            Ok(request)
        });
        let base_dir = unique_backup_dir("download-valid")?;
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": endpoint }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-restore", "local_only": false }
        }))?;

        let outcome =
            execute_backup_catalog_download_in_dir(&request, Some(base_dir.clone())).await?;
        let RuntimeOperationOutcome::Completed(result) = outcome else {
            bail!("backup download should complete");
        };
        let request_bytes = server
            .join()
            .map_err(|_| anyhow::anyhow!("storage download test server panicked"))??;
        let request_text = std::str::from_utf8(&request_bytes)?;
        let catalog: Value =
            serde_json::from_slice(&fs::read(base_dir.join("backup_catalog.json"))?)?;

        anyhow::ensure!(
            request_text.starts_with("GET /data/cid-restore/network/stream HTTP/1.1\r\n"),
            "unexpected backup download request: {request_text}"
        );
        anyhow::ensure!(
            result.get("downloaded") == Some(&json!(true))
                && result.get("restored") == Some(&json!(false))
                && result.get("cid") == Some(&json!("cid-restore"))
                && result.get("encrypted") == Some(&json!(false))
                && result.get("bytes") == Some(&json!(body.len()))
                && result.pointer("/catalog_entry/remote/cid") == Some(&json!("cid-restore")),
            "backup download result drifted: {result:?}"
        );
        anyhow::ensure!(
            catalog
                .get("entries")
                .and_then(Value::as_array)
                .is_some_and(|entries| entries.len() == 1),
            "downloaded payload was not recorded exactly once: {catalog:?}"
        );
        fs::remove_dir_all(&base_dir)?;
        Ok(())
    }

    #[tokio::test]
    async fn backup_download_executor_preserves_local_only_route_and_result_scope() -> Result<()> {
        let body = serde_json::to_vec(&json!({
            "kind": "logos-inspector-settings-backup",
            "version": 1,
            "encrypted": false,
            "state": { "settings": { "favorites": [] } }
        }))?;
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let response_body = body.clone();
        let server = thread::spawn(move || -> Result<Vec<u8>> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let request = read_http_headers(&mut stream)?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                response_body.len()
            )?;
            stream.write_all(&response_body)?;
            Ok(request)
        });
        let base_dir = unique_backup_dir("download-local")?;
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": endpoint }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-local", "local_only": true }
        }))?;

        let outcome =
            execute_backup_catalog_download_in_dir(&request, Some(base_dir.clone())).await?;
        let RuntimeOperationOutcome::Completed(result) = outcome else {
            bail!("local-only backup download should complete");
        };
        let request_bytes = server
            .join()
            .map_err(|_| anyhow::anyhow!("local-only download test server panicked"))??;
        let request_text = std::str::from_utf8(&request_bytes)?;

        anyhow::ensure!(
            request_text.starts_with("GET /data/cid-local HTTP/1.1\r\n"),
            "unexpected local-only backup request: {request_text}"
        );
        anyhow::ensure!(
            result.get("source") == Some(&json!("local"))
                && result.get("restored") == Some(&json!(false))
                && result.get("cid") == Some(&json!("cid-local")),
            "local-only backup result lost request scope: {result:?}"
        );
        fs::remove_dir_all(&base_dir)?;
        Ok(())
    }

    #[tokio::test]
    async fn invalid_backup_download_json_does_not_record_catalog_entry() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            let _request = read_http_headers(&mut stream)?;
            let body = b"{not-json";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )?;
            stream.write_all(body)?;
            Ok(())
        });
        let base_dir = unique_backup_dir("download-invalid")?;
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": endpoint }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-invalid", "local_only": false }
        }))?;

        let error = execute_backup_catalog_download_in_dir(&request, Some(base_dir.clone()))
            .await
            .err()
            .context("invalid backup JSON should fail")?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("invalid JSON test server panicked"))??;

        anyhow::ensure!(
            error
                .to_string()
                .contains("settings backup CID cid-invalid did not contain JSON"),
            "unexpected invalid backup error: {error:#}"
        );
        anyhow::ensure!(
            !base_dir.join("backup_catalog.json").exists(),
            "invalid backup JSON created a catalog"
        );
        Ok(())
    }

    #[tokio::test]
    async fn invalid_backup_envelopes_do_not_record_catalog_entries() -> Result<()> {
        let cases = [
            (
                "wrong-kind",
                json!({
                    "kind": "other-backup",
                    "version": 1,
                    "encrypted": false,
                    "state": { "settings": {} }
                }),
                "backup payload kind is not supported",
            ),
            (
                "wrong-version",
                json!({
                    "kind": "logos-inspector-settings-backup",
                    "version": 2,
                    "encrypted": false,
                    "state": { "settings": {} }
                }),
                "backup payload version is not supported",
            ),
            (
                "malformed-encrypted",
                json!({
                    "kind": "logos-inspector-settings-backup",
                    "version": 1,
                    "encrypted": true,
                    "state": { "settings": {} }
                }),
                "encrypted backup metadata is missing",
            ),
        ];

        for (label, payload, expected_error) in cases {
            let body = serde_json::to_vec(&payload)?;
            let listener = TcpListener::bind("127.0.0.1:0")?;
            let endpoint = format!("http://{}", listener.local_addr()?);
            let server = thread::spawn(move || -> Result<()> {
                let (mut stream, _) = listener.accept()?;
                let _request = read_http_headers(&mut stream)?;
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                )?;
                stream.write_all(&body)?;
                Ok(())
            });
            let base_dir = unique_backup_dir(label)?;
            let request = runtime_operation_request_from_value(json!({
                "domain": "storage",
                "method": "storageDownloadBackupCatalogEntry",
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": endpoint }
                },
                "mutating_enabled": false,
                "payload": { "cid": format!("cid-{label}"), "local_only": false }
            }))?;

            let error = execute_backup_catalog_download_in_dir(&request, Some(base_dir.clone()))
                .await
                .err()
                .context("invalid backup envelope should fail")?;
            server
                .join()
                .map_err(|_| anyhow::anyhow!("invalid envelope test server panicked"))??;

            anyhow::ensure!(
                format!("{error:#}").contains(expected_error),
                "unexpected invalid backup envelope error: {error:#}"
            );
            anyhow::ensure!(
                !base_dir.join("backup_catalog.json").exists(),
                "invalid backup envelope created a catalog"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn oversized_backup_download_does_not_record_catalog_entry() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            let _request = read_http_headers(&mut stream)?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                SETTINGS_BACKUP_MAX_BYTES.saturating_add(1)
            )?;
            Ok(())
        });
        let base_dir = unique_backup_dir("download-oversized")?;
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadBackupCatalogEntry",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": endpoint }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-oversized", "local_only": false }
        }))?;

        let error = execute_backup_catalog_download_in_dir(&request, Some(base_dir.clone()))
            .await
            .err()
            .context("oversized backup download should fail")?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("oversized download test server panicked"))??;

        anyhow::ensure!(
            format!("{error:#}").contains(&format!(
                "http response body exceeded {} byte limit",
                SETTINGS_BACKUP_MAX_BYTES
            )),
            "unexpected oversized backup error: {error:#}"
        );
        anyhow::ensure!(
            !base_dir.join("backup_catalog.json").exists(),
            "oversized backup download created a catalog"
        );
        Ok(())
    }

    #[tokio::test]
    async fn module_backup_download_fails_closed_without_cli_fallback() -> Result<()> {
        for (source_mode, expected_error) in [
            (
                "module",
                "Basecamp Storage module does not expose backup read-by-CID bytes",
            ),
            (
                "logoscore_cli",
                "LogosCore CLI Storage adapter does not support backup read-by-CID bytes",
            ),
        ] {
            let base_dir = unique_backup_dir(source_mode)?;
            let request = runtime_operation_request_from_value(json!({
                "domain": "storage",
                "method": "storageDownloadBackupCatalogEntry",
                "adapter": { "source_mode": source_mode, "inputs": {} },
                "mutating_enabled": false,
                "payload": { "cid": "cid-module", "local_only": false }
            }))?;

            let error = execute_backup_catalog_download_in_dir(&request, Some(base_dir.clone()))
                .await
                .err()
                .context("module backup download should fail")?;
            anyhow::ensure!(
                format!("{error:#}").contains(expected_error),
                "unexpected module backup download error: {error:#}"
            );
            anyhow::ensure!(
                !base_dir.join("backup_catalog.json").exists(),
                "failed module download wrote local catalog state"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn payload_upload_executor_rejects_basecamp_before_serialization() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadPayload",
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": {
                "filename": "shared-idl.json",
                "payload": { "kind": "shared-idl" },
                "block_size": 65536
            }
        }))?;
        let transport: SharedModuleTransport =
            Arc::new(UnavailableModuleTransport::basecamp_protocol_gate());

        let error = execute_payload_upload(&request, transport)
            .await
            .err()
            .context("Basecamp payload upload should fail")?;

        anyhow::ensure!(
            error.to_string()
                == "Basecamp module source does not support Inspector-managed byte staging",
            "unexpected executor error: {error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn payload_upload_executor_preserves_rest_compatibility_result() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<Vec<u8>> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let request = read_http_request(&mut stream)?;
            let response_body = "  cid-rest  \n";
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{response_body}",
                response_body.len()
            )?;
            Ok(request)
        });
        let payload = json!({ "kind": "shared-idl", "version": 1 });
        let expected_bytes = serde_json::to_vec_pretty(&payload)?;
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadPayload",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": endpoint }
            },
            "mutating_enabled": true,
            "payload": {
                "filename": "shared-idl.json",
                "payload": payload,
                "block_size": 32768
            }
        }))?;
        let transport: SharedModuleTransport =
            Arc::new(UnavailableModuleTransport::basecamp_protocol_gate());

        let outcome = execute_payload_upload(&request, transport).await?;
        let RuntimeOperationOutcome::Completed(result) = outcome else {
            bail!("payload upload should complete");
        };
        let request_bytes = server
            .join()
            .map_err(|_| anyhow::anyhow!("storage upload test server panicked"))??;
        let (headers, body) = split_http_request(&request_bytes)?;

        anyhow::ensure!(
            headers.starts_with("POST /data?blockSize=32768 HTTP/1.1\r\n"),
            "unexpected storage upload request: {headers}"
        );
        anyhow::ensure!(
            headers.contains("content-type: application/json\r\n")
                && headers
                    .contains("content-disposition: attachment; filename=\"shared-idl.json\"\r\n"),
            "storage upload headers lost compatibility fields: {headers}"
        );
        anyhow::ensure!(
            body == expected_bytes,
            "storage upload payload bytes drifted"
        );
        anyhow::ensure!(
            result
                == json!({
                    "cid": "cid-rest",
                    "bytes": expected_bytes.len(),
                    "endpoint": endpoint,
                    "filename": "shared-idl.json",
                    "upload": {
                        "cid": "cid-rest",
                        "filename": "shared-idl.json",
                        "bytes": expected_bytes.len(),
                        "endpoint": endpoint,
                    }
                }),
            "payload upload compatibility result drifted: {result:?}"
        );
        Ok(())
    }

    fn read_http_request(stream: &mut std::net::TcpStream) -> Result<Vec<u8>> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let bytes_read = stream.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            let chunk = buffer
                .get(..bytes_read)
                .context("HTTP request read exceeded its buffer")?;
            request.extend_from_slice(chunk);
            if let Ok((headers, body)) = split_http_request(&request)
                && let Some(content_length) = http_content_length(headers)
                && body.len() >= content_length
            {
                break;
            }
        }
        Ok(request)
    }

    fn read_http_headers(stream: &mut std::net::TcpStream) -> Result<Vec<u8>> {
        let mut request = Vec::new();
        let mut buffer = [0_u8; 4096];
        loop {
            let bytes_read = stream.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            request.extend_from_slice(
                buffer
                    .get(..bytes_read)
                    .context("HTTP header read exceeded its buffer")?,
            );
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                return Ok(request);
            }
        }
        bail!("HTTP request headers were incomplete")
    }

    fn unique_backup_dir(label: &str) -> Result<PathBuf> {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .context("system clock is before Unix epoch")?
            .as_nanos();
        Ok(std::env::temp_dir().join(format!(
            "logos-inspector-backup-operation-{label}-{}-{nanos}",
            std::process::id()
        )))
    }

    fn split_http_request(request: &[u8]) -> Result<(&str, &[u8])> {
        let header_end = request
            .windows(4)
            .position(|window| window == b"\r\n\r\n")
            .context("HTTP request headers were incomplete")?;
        let header_stop = header_end
            .checked_add(2)
            .context("HTTP request header boundary overflowed")?;
        let body_start = header_end
            .checked_add(4)
            .context("HTTP request body boundary overflowed")?;
        let headers = std::str::from_utf8(
            request
                .get(..header_stop)
                .context("HTTP request header boundary was invalid")?,
        )
        .context("HTTP request headers were not UTF-8")?;
        let body = request
            .get(body_start..)
            .context("HTTP request body boundary was invalid")?;
        Ok((headers, body))
    }

    fn http_content_length(headers: &str) -> Option<usize> {
        headers.lines().find_map(|line| {
            let (name, value) = line.split_once(':')?;
            name.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse().ok())
                .flatten()
        })
    }

    #[test]
    fn payload_upload_completion_requires_nonempty_string_cid() -> Result<()> {
        let cases = [json!({}), json!({ "cid": null }), json!({ "cid": "  " })];
        for upload in cases {
            anyhow::ensure!(
                required_payload_upload_cid(&upload).is_err(),
                "malformed upload CID was accepted: {upload}"
            );
        }
        anyhow::ensure!(
            required_payload_upload_cid(&json!({ "cid": "  cid-1  " }))? == "cid-1",
            "upload CID was not normalized"
        );
        Ok(())
    }

    #[test]
    fn backup_upload_completion_requires_nonempty_string_cid() -> Result<()> {
        let cases = [json!({}), json!({ "cid": null }), json!({ "cid": "  " })];
        for upload in cases {
            anyhow::ensure!(
                required_backup_upload_cid(&upload).is_err(),
                "malformed upload CID was accepted: {upload}"
            );
        }
        anyhow::ensure!(
            required_backup_upload_cid(&json!({ "cid": "  cid-1  " }))? == "cid-1",
            "upload CID was not normalized"
        );
        Ok(())
    }
}
