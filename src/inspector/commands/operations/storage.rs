use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::Map;

use anyhow::{Context as _, Result, bail};
use reqwest::Method;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;

use crate::{
    raw_http_json,
    source_routing::{
        self, require_mutating_diagnostics, rest_empty_request, rest_json_request, rest_url,
        storage_rest_source, storage_rest_upload,
    },
    support::args::Args,
    support::raw_source_transport::request_success,
};

use super::super::value::to_value;
use super::record::update_runtime_operation_progress;
use super::spec::{OperationDefinition, OperationDomain, OperationExclusiveGroup, OperationMethod};
use super::{
    RuntimeOperationRegistry, RuntimeOperationRequest, blocking_module_call,
    blocking_module_dispatch,
};

pub(super) const OPERATION_DEFINITIONS: &[OperationDefinition] = &[
    OperationDefinition::new(
        OperationMethod::StorageManifests,
        "storageManifests",
        OperationDomain::Storage,
        "Storage manifests",
    ),
    OperationDefinition::new(
        OperationMethod::StorageDownloadManifest,
        "storageDownloadManifest",
        OperationDomain::Storage,
        "Storage manifest",
    ),
    OperationDefinition::mutating(
        OperationMethod::StorageFetch,
        "storageFetch",
        OperationDomain::Storage,
        "Storage fetch",
    ),
    OperationDefinition::mutating(
        OperationMethod::StorageUploadUrl,
        "storageUploadUrl",
        OperationDomain::Storage,
        "Storage upload",
    ),
    OperationDefinition::cancellable(
        OperationMethod::StorageDownloadToUrl,
        "storageDownloadToUrl",
        OperationDomain::Storage,
        "Storage download",
        OperationExclusiveGroup::StorageDownload,
    ),
    OperationDefinition::mutating(
        OperationMethod::StorageRemove,
        "storageRemove",
        OperationDomain::Storage,
        "Storage remove",
    ),
];

pub(super) async fn execute(
    request: &RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    match request.method() {
        OperationMethod::StorageManifests => execute_storage_manifests(request).await,
        OperationMethod::StorageDownloadManifest => {
            execute_storage_download_manifest(request).await
        }
        OperationMethod::StorageFetch => execute_storage_fetch(request).await,
        OperationMethod::StorageUploadUrl => execute_storage_upload(request).await,
        OperationMethod::StorageDownloadToUrl => {
            execute_storage_download(request, registry, operation_id, cancel_requested).await
        }
        OperationMethod::StorageRemove => execute_storage_remove(request).await,
        _ => bail!("`{}` is not a Storage operation", request.method_name()),
    }
}

pub(super) fn add_operation_context(
    request: &RuntimeOperationRequest,
    context: &mut Map<String, Value>,
) {
    let Ok(args) = Args::new(request.args.clone()) else {
        return;
    };
    let Ok(source) = storage_rest_source(&args) else {
        return;
    };
    context.insert("endpoint".to_owned(), json!(source.endpoint));
    match request.method() {
        OperationMethod::StorageDownloadToUrl => {
            if let Some(cid) = args.optional_string(source.next_index + 1) {
                context.insert("cid".to_owned(), json!(cid));
            }
            if let Some(path) = args.optional_string(source.next_index + 2) {
                context.insert("path".to_owned(), json!(path));
            }
            context.insert(
                "source".to_owned(),
                json!(if args.optional_bool(source.next_index + 3) {
                    "local"
                } else {
                    "network"
                }),
            );
        }
        OperationMethod::StorageUploadUrl => {
            if let Some(path) = args.optional_string(source.next_index + 1) {
                context.insert("path".to_owned(), json!(path));
            }
        }
        OperationMethod::StorageFetch | OperationMethod::StorageRemove => {
            if let Some(cid) = args.optional_string(source.next_index + 1) {
                context.insert("cid".to_owned(), json!(cid));
            }
        }
        OperationMethod::StorageDownloadManifest => {
            let cid_index = if matches!(args.value(source.next_index), Some(Value::Bool(_))) {
                source.next_index + 1
            } else {
                source.next_index
            };
            if let Some(cid) = args.optional_string(cid_index) {
                context.insert("cid".to_owned(), json!(cid));
            }
        }
        _ => {}
    }
}

pub(super) async fn execute_storage_manifests(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = source_routing::storage_args(&args, false, "storage manifests")? {
        return blocking_module_call(
            "storage module manifests",
            source_routing::STORAGE_MODULE,
            "manifests",
            module_args.values,
        )
        .await;
    }
    let source = storage_rest_source(&args)?;
    to_value(raw_http_json(source.endpoint, "/data").await?)
}

pub(super) async fn execute_storage_download_manifest(
    request: &RuntimeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) =
        source_routing::storage_args(&args, false, "storage manifest download")?
    {
        let cid = module_args
            .values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        return blocking_module_dispatch(
            "storage module manifest download",
            source_routing::STORAGE_MODULE,
            "downloadManifest",
            module_args.values,
            vec![("cid", cid)],
        )
        .await;
    }
    let source = storage_rest_source(&args)?;
    let cid_index = if matches!(args.value(source.next_index), Some(Value::Bool(_))) {
        source.next_index + 1
    } else {
        source.next_index
    };
    let cid = args.string(cid_index, "CID")?;
    to_value(raw_http_json(source.endpoint, &format!("/data/{cid}/network/manifest")).await?)
}

pub(super) async fn execute_storage_fetch(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = source_routing::storage_args(&args, true, "storage network action")?
    {
        let cid = module_args
            .values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        return blocking_module_dispatch(
            "storage module fetch",
            source_routing::STORAGE_MODULE,
            "fetch",
            module_args.values,
            vec![("cid", cid)],
        )
        .await;
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "storage network action")?;
    let cid = args.string(source.next_index + 1, "CID")?;
    rest_json_request(
        Method::POST,
        source.endpoint,
        &format!("/data/{cid}/network"),
        None,
    )
    .await
    .with_context(|| format!("failed to start storage network fetch for {cid}"))
}

pub(super) async fn execute_storage_upload(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = source_routing::storage_args(&args, true, "storage upload action")? {
        let mut values = module_args.values;
        if values.len() < 2 {
            values.push(json!(65_536));
        }
        let path = values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        return blocking_module_dispatch(
            "storage module upload",
            source_routing::STORAGE_MODULE,
            "uploadUrl",
            values,
            vec![("path", path)],
        )
        .await;
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "storage upload action")?;
    let path = args.string(source.next_index + 1, "file path")?;
    if path.starts_with("http://") || path.starts_with("https://") {
        bail!("storage REST upload expects a local file path");
    }
    let block_size = args
        .value(source.next_index + 2)
        .and_then(Value::as_u64)
        .unwrap_or(65_536);
    storage_rest_upload(source.endpoint, path, block_size)
        .await
        .with_context(|| format!("failed to upload `{path}` through storage REST"))
}

pub(super) async fn execute_storage_download(
    request: &RuntimeOperationRequest,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = source_routing::storage_args(&args, true, "storage download action")?
    {
        let mut values = module_args.values;
        if values.len() < 3 {
            values.push(json!(false));
        }
        if values.len() < 4 {
            values.push(json!(65_536));
        }
        let cid = values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let path = values
            .get(1)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        return blocking_module_dispatch(
            "storage module download",
            source_routing::STORAGE_MODULE,
            "downloadToUrl",
            values,
            vec![("cid", cid), ("path", path)],
        )
        .await;
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "storage download action")?;
    let cid = args.string(source.next_index + 1, "CID")?;
    let path = args.string(source.next_index + 2, "download path")?;
    let local_only = args.optional_bool(source.next_index + 3);
    storage_rest_download_tracked(
        source.endpoint,
        cid,
        path,
        local_only,
        registry,
        operation_id,
        cancel_requested,
    )
    .await
    .with_context(|| format!("failed to download storage CID {cid} to `{path}`"))
}

pub(super) async fn execute_storage_remove(request: &RuntimeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = source_routing::storage_args(&args, true, "storage remove action")? {
        let cid = module_args
            .values
            .first()
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        return blocking_module_dispatch(
            "storage module remove",
            source_routing::STORAGE_MODULE,
            "remove",
            module_args.values,
            vec![("cid", cid)],
        )
        .await;
    }
    let source = storage_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "storage remove action")?;
    let cid = args.string(source.next_index + 1, "CID")?;
    rest_empty_request(
        Method::DELETE,
        source.endpoint,
        &format!("/data/{cid}"),
        None,
    )
    .await
    .with_context(|| format!("failed to remove storage CID {cid}"))?;
    Ok(json!({
        "removed": true,
        "cid": cid,
        "endpoint": source.endpoint,
    }))
}

pub(super) async fn storage_rest_download_tracked(
    endpoint: &str,
    cid: &str,
    path: &str,
    local_only: bool,
    registry: &RuntimeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    if cancel_requested.load(Ordering::Relaxed) {
        bail!("storage download canceled");
    }
    let route = if local_only {
        format!("/data/{cid}")
    } else {
        format!("/data/{cid}/network/stream")
    };
    let url = rest_url(endpoint, &route);
    let response = request_success(
        reqwest::Client::new().get(&url),
        &url,
        "storage download",
        "failed to read storage download error body",
    )
    .await?;
    update_runtime_operation_progress(registry, operation_id, 0, response.content_length());
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
        "cid": cid,
        "path": path,
        "bytes": bytes,
        "source": if local_only { "local" } else { "network" },
        "endpoint": endpoint,
    }))
}
