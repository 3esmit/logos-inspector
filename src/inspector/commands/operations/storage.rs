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
            Self::UploadBackupCatalogEntry => None,
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
    if command == StorageCommand::UploadBackupCatalogEntry {
        return execute_backup_catalog_upload(request, module_transport).await;
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
    if command == StorageCommand::UploadBackupCatalogEntry {
        let upload =
            storage_layer::StorageBackupUploadRequest::parse_request(request.node_request()?)?;
        context.insert(
            "backupCatalogId".to_owned(),
            json!(upload.backup_catalog_id()),
        );
        return Ok(());
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
    if command == StorageCommand::UploadBackupCatalogEntry {
        return storage_layer::StorageBackupUploadRequest::parse_request(request.node_request()?)
            .map(|_| ());
    }
    let operation = command
        .operation()
        .context("storage command has no standard operation")?;
    storage_layer::StorageOperationRequest::parse(request.node_request()?, operation).map(|_| ())
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
    use std::sync::Arc;

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
