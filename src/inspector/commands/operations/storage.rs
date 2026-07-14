use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;

use crate::{
    modules::logos_core::SharedModuleTransport,
    source_routing::storage_layer,
    support::backup_catalog::{attach_remote_backup_metadata, backup_payload_bytes},
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
            Self::UploadPayload | Self::UploadBackupCatalogEntry => None,
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
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::Arc,
        thread,
        time::Duration,
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
