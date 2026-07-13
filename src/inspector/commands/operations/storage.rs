use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;

use crate::source_routing::storage_layer;

use super::record::update_runtime_operation_progress;
use super::spec::{
    OperationClass, OperationDefinition, OperationDomain, OperationExclusiveGroup, OperationMethod,
};
use super::{RuntimeOperationRegistry, RuntimeOperationRequest};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationMethod::StorageManifests,
        "storageManifests",
        OperationDomain::Storage,
        "Storage manifests",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&["source", "endpoint"]),
    OperationDefinition::new(
        OperationMethod::StorageDownloadManifest,
        "storageDownloadManifest",
        OperationDomain::Storage,
        "Storage manifest",
        OperationClass::ReadPoll,
    )
    .with_context_inputs(&["source", "endpoint", "cid"]),
    OperationDefinition::new(
        OperationMethod::StorageFetch,
        "storageFetch",
        OperationDomain::Storage,
        "Storage fetch",
        OperationClass::Mutating,
    )
    .with_context_inputs(&["source", "endpoint", "cid"]),
    OperationDefinition::new(
        OperationMethod::StorageUploadUrl,
        "storageUploadUrl",
        OperationDomain::Storage,
        "Storage upload",
        OperationClass::Mutating,
    )
    .with_context_inputs(&["source", "endpoint", "path"]),
    OperationDefinition::new(
        OperationMethod::StorageDownloadToUrl,
        "storageDownloadToUrl",
        OperationDomain::Storage,
        "Storage download",
        OperationClass::Mutating,
    )
    .with_context_inputs(&["source", "endpoint", "cid", "path"])
    .cancellable(OperationExclusiveGroup::StorageDownload),
    OperationDefinition::new(
        OperationMethod::StorageRemove,
        "storageRemove",
        OperationDomain::Storage,
        "Storage remove",
        OperationClass::Destructive,
    )
    .with_context_inputs(&["source", "endpoint", "cid"]),
];

pub(super) async fn execute(
    request: &RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    let operation = storage_operation(request)?;
    let request =
        storage_layer::StorageOperationRequest::parse(request.node_request()?, operation)?;
    match storage_layer::execute_operation(request).await? {
        storage_layer::StorageOperationOutput::Complete(value) => Ok(value),
        storage_layer::StorageOperationOutput::Download(download) => {
            let cid = download.cid().to_owned();
            let path = download.path().to_owned();
            storage_rest_download_tracked(&download, registry, operation_id, cancel_requested)
                .await
                .with_context(|| format!("failed to download storage CID {cid} to `{path}`"))
        }
    }
}

pub(super) fn add_operation_context(
    request: &RuntimeOperationRequest,
    context: &mut serde_json::Map<String, Value>,
) {
    let Ok(operation) = storage_operation(request) else {
        return;
    };
    if let Ok(node_request) = request.node_request()
        && let Ok(operation_request) =
            storage_layer::StorageOperationRequest::parse(node_request, operation)
    {
        context.extend(operation_request.context().clone());
    }
}

pub(super) fn validate(request: &RuntimeOperationRequest) -> Result<()> {
    let operation = storage_operation(request)?;
    storage_layer::StorageOperationRequest::parse(request.node_request()?, operation).map(|_| ())
}

fn storage_operation(request: &RuntimeOperationRequest) -> Result<storage_layer::StorageOperation> {
    match request.method() {
        OperationMethod::StorageManifests => Ok(storage_layer::StorageOperation::Manifests),
        OperationMethod::StorageDownloadManifest => {
            Ok(storage_layer::StorageOperation::DownloadManifest)
        }
        OperationMethod::StorageFetch => Ok(storage_layer::StorageOperation::Fetch),
        OperationMethod::StorageUploadUrl => Ok(storage_layer::StorageOperation::Upload),
        OperationMethod::StorageDownloadToUrl => Ok(storage_layer::StorageOperation::Download),
        OperationMethod::StorageRemove => Ok(storage_layer::StorageOperation::Remove),
        _ => bail!("`{}` is not a Storage operation", request.method_name()),
    }
}

pub(super) async fn storage_rest_download_tracked(
    request: &storage_layer::StorageDownloadRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    if cancel_requested.load(Ordering::Relaxed) {
        bail!("storage download canceled");
    }
    let response = storage_layer::download_response(request).await?;
    update_runtime_operation_progress(registry, operation_id, 0, response.content_length());
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
            update_runtime_operation_progress(registry, operation_id, bytes, None);
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
