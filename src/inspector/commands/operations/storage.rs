use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;

use crate::{modules::logos_core::SharedModuleTransport, source_routing::storage_layer};

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
            Self::DownloadToUrl => OperationMethod::StorageDownloadToUrl,
            Self::Remove => OperationMethod::StorageRemove,
        }
    }

    const fn operation(self) -> storage_layer::StorageOperation {
        match self {
            Self::Manifests => storage_layer::StorageOperation::Manifests,
            Self::DownloadManifest => storage_layer::StorageOperation::DownloadManifest,
            Self::Fetch => storage_layer::StorageOperation::Fetch,
            Self::UploadUrl => storage_layer::StorageOperation::Upload,
            Self::DownloadToUrl => storage_layer::StorageOperation::Download,
            Self::Remove => storage_layer::StorageOperation::Remove,
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
    let request = storage_layer::StorageOperationRequest::parse(
        request.node_request()?,
        command.operation(),
    )?;
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
    let operation_request = storage_layer::StorageOperationRequest::parse(
        request.node_request()?,
        command.operation(),
    )?;
    context.extend(operation_request.context().clone());
    Ok(())
}

pub(super) fn validate(command: StorageCommand, request: &RuntimeOperationRequest) -> Result<()> {
    storage_layer::StorageOperationRequest::parse(request.node_request()?, command.operation())
        .map(|_| ())
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
