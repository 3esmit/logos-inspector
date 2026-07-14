use std::{
    io::Write as _,
    path::{Path, PathBuf},
};

#[cfg(test)]
use anyhow::bail;
use anyhow::{Context as _, Result};
use serde_json::{Value, json};

#[cfg(test)]
use crate::support::settings_backup::SETTINGS_BACKUP_MAX_BYTES;
use crate::{
    modules::logos_core::SharedModuleTransport,
    source_routing::storage_layer,
    support::backup_catalog::{
        attach_remote_backup_metadata, backup_payload_bytes,
        record_remote_settings_backup_payload_in_dir,
    },
    support::command_runner::CommandControl,
    support::settings_backup::ensure_settings_backup_size,
    support::state_store::config_dir,
    support::work_tracker::BlockingWorkGuard,
};

use super::spec::{
    AffectedContextField, AffectedContextKey, OperationClass, OperationCommand,
    OperationDefinition, OperationExclusiveGroup, OperationMethod,
};
use super::{
    RuntimeOperationRegistry, RuntimeOperationRequest,
    dispatch::normalize_command_execution,
    identity::RuntimeOperationId,
    outcome::RuntimeOperationOutcome,
    supervisor::{
        OperationCommitGuard, OperationControl, OperationInterrupted, OperationStopReason,
        TerminationEvidence,
    },
    transition::RuntimeOperationTransition,
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
    control: &OperationControl,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    match command {
        StorageCommand::UploadPayload => {
            return execute_payload_upload(request, control, module_transport).await;
        }
        StorageCommand::UploadBackupCatalogEntry => {
            return execute_backup_catalog_upload(request, control, module_transport).await;
        }
        StorageCommand::DownloadBackupCatalogEntry => {
            return execute_backup_catalog_download(request, control).await;
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
            storage_rest_download_tracked(&download, registry, operation_id, control)
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
    control: &OperationControl,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    let request =
        storage_layer::StoragePayloadUploadRequest::parse_request(request.node_request()?)?;
    request
        .client()
        .ensure_managed_byte_upload_supported(&module_transport)?;
    ensure_not_interrupted(control)?;
    let payload = request.payload().clone();
    let worker_guard = control.blocking_worker_guard()?;
    let bytes = tokio::task::spawn_blocking(move || {
        let _worker_guard = worker_guard;
        serde_json::to_vec_pretty(&payload)
    })
    .await
    .context("storage payload serialization worker failed")?
    .context("failed to serialize storage payload")?;
    ensure_not_interrupted(control)?;
    let result = request
        .client()
        .upload_bytes_controlled(
            &module_transport,
            request.filename(),
            &bytes,
            request.block_size(),
            command_control(control),
        )
        .await;
    let upload = normalize_command_execution(
        result,
        control,
        TerminationEvidence::LocalOnly,
        TerminationEvidence::LocalOnly,
    )
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
    control: &OperationControl,
    module_transport: SharedModuleTransport,
) -> Result<RuntimeOperationOutcome> {
    let request =
        storage_layer::StorageBackupUploadRequest::parse_request(request.node_request()?)?;
    request
        .client()
        .ensure_managed_byte_upload_supported(&module_transport)?;
    ensure_not_interrupted(control)?;
    let backup_catalog_id = request.backup_catalog_id().to_owned();
    let payload_catalog_id = backup_catalog_id.clone();
    let worker_guard = control.blocking_worker_guard()?;
    let bytes = tokio::task::spawn_blocking(move || {
        let _worker_guard = worker_guard;
        backup_payload_bytes(&payload_catalog_id)
    })
    .await
    .context("backup payload worker failed")??;
    ensure_settings_backup_size(bytes.len())?;
    ensure_not_interrupted(control)?;
    let result = request
        .client()
        .upload_bytes_controlled(
            &module_transport,
            "logos-inspector-settings-backup.json",
            &bytes,
            request.block_size(),
            command_control(control),
        )
        .await;
    let upload = normalize_command_execution(
        result,
        control,
        TerminationEvidence::LocalOnly,
        TerminationEvidence::LocalOnly,
    )
    .context("failed to upload settings backup through Storage")?;
    let cid = required_backup_upload_cid(&upload)?;
    let metadata_catalog_id = backup_catalog_id.clone();
    let metadata_cid = cid.clone();
    let catalog_entry =
        non_cancellable_file_commit(control, "backup catalog metadata commit", move || {
            attach_remote_backup_metadata(
                &metadata_catalog_id,
                &metadata_cid,
                Some("logos_storage"),
            )
        })
        .await
        .context("backup catalog metadata commit failed")?;
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
    control: &OperationControl,
) -> Result<RuntimeOperationOutcome> {
    execute_backup_catalog_download_in_dir(request, control, None).await
}

async fn execute_backup_catalog_download_in_dir(
    request: &RuntimeOperationRequest,
    control: &OperationControl,
    catalog_base_dir: Option<PathBuf>,
) -> Result<RuntimeOperationOutcome> {
    let request =
        storage_layer::StorageBackupDownloadRequest::parse_request(request.node_request()?)?;
    let cid = request.cid().to_owned();
    let local_only = request.local_only();
    let result = request
        .client()
        .download_backup_bytes_controlled(&cid, local_only, command_control(control))
        .await;
    let bytes = normalize_command_execution(
        result,
        control,
        TerminationEvidence::Confirmed,
        TerminationEvidence::LocalOnly,
    )
    .with_context(|| format!("failed to download settings backup CID {cid}"))?;
    let endpoint = request
        .client()
        .endpoint()
        .context("settings backup download requires storage REST source")?
        .to_owned();
    ensure_not_interrupted(control)?;
    let result =
        non_cancellable_file_commit(control, "backup download catalog commit", move || {
            let base_dir = match catalog_base_dir {
                Some(base_dir) => base_dir,
                None => config_dir()?,
            };
            downloaded_backup_result(&base_dir, &bytes, &cid, &endpoint, local_only)
        })
        .await
        .context("backup download catalog commit failed")?;
    Ok(RuntimeOperationOutcome::Completed(result))
}

fn command_control(control: &OperationControl) -> CommandControl {
    control.command_control()
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
    control: &OperationControl,
) -> Result<Value> {
    let response = interruptible(control, storage_layer::download_response(request)).await??;
    registry.transition(
        operation_id,
        RuntimeOperationTransition::Progress {
            bytes_written: 0,
            content_length: response.content_length(),
        },
    )?;
    let path = request.path();
    let temp_path = control
        .disposable_file_path()
        .context("storage download has no supervised staging file")?;
    ensure_not_interrupted(control)?;
    let open_path = temp_path.to_owned();
    let open_control = control.clone();
    let lifetime_guard = control.blocking_worker_guard()?;
    let mut file = interruptible(
        control,
        blocking_file_work("storage download staging-file open", move || {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&open_path)
                .with_context(|| {
                    format!(
                        "failed to create download staging file `{}`",
                        open_path.display()
                    )
                })?;
            open_control.mark_disposable_file_created();
            Ok(TrackedStagingFile {
                file,
                _lifetime_guard: lifetime_guard,
            })
        }),
    )
    .await??;
    let mut response = response;
    let mut bytes = 0_u64;
    while let Some(chunk) = interruptible(control, response.chunk())
        .await?
        .context("failed to read storage download response chunk")?
    {
        let chunk_len = chunk.len();
        let write_path = temp_path.to_owned();
        file = interruptible(
            control,
            blocking_file_work("storage download staging-file write", move || {
                file.file.write_all(&chunk).with_context(|| {
                    format!("failed to write download file `{}`", write_path.display())
                })?;
                Ok(file)
            }),
        )
        .await??;
        bytes = bytes.saturating_add(u64::try_from(chunk_len).unwrap_or(u64::MAX));
        registry.transition(
            operation_id,
            RuntimeOperationTransition::Progress {
                bytes_written: bytes,
                content_length: None,
            },
        )?;
    }
    let flush_path = temp_path.to_owned();
    file = interruptible(
        control,
        blocking_file_work("storage download staging-file flush", move || {
            file.file.flush().with_context(|| {
                format!("failed to flush download file `{}`", flush_path.display())
            })?;
            Ok(file)
        }),
    )
    .await??;
    drop(file);
    ensure_not_interrupted(control)?;
    let rename_from = temp_path.to_owned();
    let rename_to = PathBuf::from(path);
    non_cancellable_file_commit(control, "storage download staging-file commit", move || {
        std::fs::rename(&rename_from, &rename_to).with_context(|| {
            format!(
                "failed to move `{}` to `{}`",
                rename_from.display(),
                rename_to.display()
            )
        })
    })
    .await?;
    Ok(json!({
        "cid": request.cid(),
        "path": path,
        "bytes": bytes,
        "source": if request.local_only() { "local" } else { "network" },
        "endpoint": request.endpoint(),
    }))
}

struct TrackedStagingFile {
    // Field order is intentional: close the file before releasing the lifetime barrier.
    file: std::fs::File,
    _lifetime_guard: BlockingWorkGuard,
}

struct TrackedCommitOutput<T> {
    // Field order is intentional: dispose commit output before releasing either barrier.
    result: Result<T>,
    _worker_guard: BlockingWorkGuard,
    _commit_guard: OperationCommitGuard,
}

async fn blocking_file_work<T, F>(label: &'static str, work: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(work)
        .await
        .with_context(|| format!("{label} worker failed"))?
}

async fn non_cancellable_file_commit<T, F>(
    control: &OperationControl,
    label: &'static str,
    commit: F,
) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    let commit_guard = control.begin_non_cancellable_commit()?;
    let worker_guard = control.blocking_worker_guard()?;
    let output = tokio::task::spawn_blocking(move || TrackedCommitOutput {
        result: commit(),
        _worker_guard: worker_guard,
        _commit_guard: commit_guard,
    })
    .await
    .with_context(|| format!("{label} worker failed"))?;
    let TrackedCommitOutput {
        result,
        _worker_guard,
        _commit_guard,
    } = output;
    result
}

pub(super) fn operation_disposable_file(
    request: &RuntimeOperationRequest,
    operation_id: &RuntimeOperationId,
) -> Result<Option<PathBuf>> {
    if request.command() != OperationCommand::Storage(StorageCommand::DownloadToUrl) {
        return Ok(None);
    }
    let payload: Value = request.node_request()?.payload("storage download")?;
    let path = payload
        .get("path")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .context("storage download path is required")?;
    let target = PathBuf::from(path);
    let mut staging = target.as_os_str().to_os_string();
    staging.push(format!(".part.{}", operation_id.as_str()));
    Ok(Some(PathBuf::from(staging)))
}

async fn interruptible<F, T>(control: &OperationControl, future: F) -> Result<T>
where
    F: std::future::Future<Output = T>,
{
    tokio::select! {
        biased;
        value = future => Ok(value),
        () = control.cancellation().cancelled() => Err(interruption(control).into()),
        () = tokio::time::sleep_until(control.deadline()) => {
            Err(OperationInterrupted::confirmed(
                OperationStopReason::DeadlineExceeded,
                "storage download deadline elapsed",
            ).into())
        }
    }
}

fn ensure_not_interrupted(control: &OperationControl) -> Result<()> {
    if control.cancellation().is_cancelled() {
        return Err(interruption(control).into());
    }
    if tokio::time::Instant::now() >= control.deadline() {
        return Err(OperationInterrupted::confirmed(
            OperationStopReason::DeadlineExceeded,
            "storage operation deadline elapsed",
        )
        .into());
    }
    Ok(())
}

fn interruption(control: &OperationControl) -> OperationInterrupted {
    let reason = control
        .stop_reason()
        .unwrap_or(OperationStopReason::CancelRequested);
    let message = match reason {
        OperationStopReason::CancelRequested => "storage operation cancellation observed",
        OperationStopReason::DeadlineExceeded => "storage operation deadline elapsed",
        OperationStopReason::Shutdown => "storage operation stopped during shutdown",
    };
    OperationInterrupted::confirmed(reason, message)
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        sync::{Arc, mpsc},
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Context as _, Result};
    use serde_json::json;

    use super::*;
    use crate::{
        inspector::commands::operations::{
            RuntimeOperations, runtime_operation_request_from_value,
            supervisor::test_operation_control,
        },
        modules::logos_core::UnavailableModuleTransport,
    };

    const SUPERVISOR_TEST_TIMEOUT: Duration = Duration::from_secs(5);

    fn operation_control() -> OperationControl {
        test_operation_control(Duration::from_secs(30))
    }

    #[test]
    fn canceled_download_removes_owned_staging_file_before_terminal_status() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("canceled-download.bin");
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let (release_server, await_release) = mpsc::channel();
        let server = thread::spawn(move || -> Result<Vec<u8>> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(SUPERVISOR_TEST_TIMEOUT))?;
            let request = read_http_headers(&mut stream)?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 1024\r\nConnection: close\r\n\r\n"
            )?;
            stream.flush()?;
            await_release
                .recv_timeout(SUPERVISOR_TEST_TIMEOUT)
                .context("storage download test server was not released")?;
            Ok(request)
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let operations = RuntimeOperations::default();
        let request = storage_download_runtime_request(&endpoint, &target, "cid-cancel-cleanup")?;

        let started = operations.start(&runtime, request.clone())?;
        let operation_id = started
            .get("operationId")
            .and_then(Value::as_str)
            .context("started storage download has no operation id")?
            .to_owned();
        let staging =
            operation_disposable_file(&request, &RuntimeOperationId::parse(&operation_id)?)?
                .context("storage download has no staging path")?;
        runtime.block_on(wait_for_path_state(&staging, true, SUPERVISOR_TEST_TIMEOUT))?;

        let canceling = operations.cancel(&operation_id)?;

        anyhow::ensure!(
            canceling.get("status").and_then(Value::as_str) == Some("canceling"),
            "cancel request did not expose canceling state: {canceling}"
        );
        let canceled = runtime.block_on(wait_for_canceled_after_cleanup(
            &operations,
            &operation_id,
            &staging,
            &target,
            SUPERVISOR_TEST_TIMEOUT,
        ))?;
        anyhow::ensure!(
            !staging.exists() && !target.exists(),
            "canceled status became visible before staging cleanup: operation={canceled}, staging={}, target={}",
            staging.exists(),
            target.exists()
        );

        release_server
            .send(())
            .map_err(|_| anyhow::anyhow!("failed to release storage download test server"))?;
        let request_bytes = server
            .join()
            .map_err(|_| anyhow::anyhow!("storage download cleanup test server panicked"))??;
        anyhow::ensure!(
            std::str::from_utf8(&request_bytes)?
                .starts_with("GET /data/cid-cancel-cleanup/network/stream HTTP/1.1\r\n"),
            "storage download used unexpected request route"
        );
        operations.shutdown(&runtime)?;
        Ok(())
    }

    #[test]
    fn failed_download_preserves_preexisting_staging_file() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let target = directory.path().join("preexisting-staging.bin");
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<Vec<u8>> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(SUPERVISOR_TEST_TIMEOUT))?;
            let request = read_http_headers(&mut stream)?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            )?;
            stream.flush()?;
            Ok(request)
        });
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let operations = RuntimeOperations::default();
        let request =
            storage_download_runtime_request(&endpoint, &target, "cid-preexisting-staging")?;

        let started = operations.start(&runtime, request.clone())?;
        let operation_id = started
            .get("operationId")
            .and_then(Value::as_str)
            .context("started storage download has no operation id")?
            .to_owned();
        let staging =
            operation_disposable_file(&request, &RuntimeOperationId::parse(&operation_id)?)?
                .context("storage download has no staging path")?;
        fs::write(&staging, b"preexisting")?;

        let failed = runtime.block_on(wait_for_terminal_status(
            &operations,
            &operation_id,
            "failed",
            SUPERVISOR_TEST_TIMEOUT,
        ))?;

        anyhow::ensure!(
            fs::read(&staging)? == b"preexisting",
            "supervisor deleted or replaced staging file it did not create"
        );
        anyhow::ensure!(
            !target.exists(),
            "failed download unexpectedly created target: {failed}"
        );
        let request_bytes = server
            .join()
            .map_err(|_| anyhow::anyhow!("preexisting staging test server panicked"))??;
        anyhow::ensure!(
            std::str::from_utf8(&request_bytes)?
                .starts_with("GET /data/cid-preexisting-staging/network/stream HTTP/1.1\r\n"),
            "storage download used unexpected request route"
        );
        operations.shutdown(&runtime)?;
        Ok(())
    }

    async fn wait_for_canceled_after_cleanup(
        operations: &RuntimeOperations,
        operation_id: &str,
        staging: &Path,
        target: &Path,
        timeout: Duration,
    ) -> Result<Value> {
        let deadline = tokio::time::Instant::now()
            .checked_add(timeout)
            .context("canceled-operation wait deadline overflow")?;
        loop {
            let staging_existed_before_status = staging.exists();
            let target_existed_before_status = target.exists();
            let operation = operations.status(operation_id)?;
            let status = operation
                .get("status")
                .and_then(Value::as_str)
                .context("runtime operation status is missing")?;
            if status == "canceled" {
                anyhow::ensure!(
                    !staging_existed_before_status && !target_existed_before_status,
                    "canceled status was observable before cleanup: {operation}"
                );
                return Ok(operation);
            }
            if matches!(status, "completed" | "dispatched" | "failed" | "timed_out") {
                bail!("runtime operation reached `{status}` instead of `canceled`: {operation}");
            }
            if tokio::time::Instant::now() >= deadline {
                bail!(
                    "timed out waiting for runtime operation `{operation_id}` to cancel: {operation}"
                );
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    fn storage_download_runtime_request(
        endpoint: &str,
        target: &Path,
        cid: &str,
    ) -> Result<RuntimeOperationRequest> {
        runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageDownloadToUrl",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": endpoint }
            },
            "mutating_enabled": true,
            "payload": {
                "cid": cid,
                "path": target.to_string_lossy(),
                "local_only": false
            }
        }))
    }

    async fn wait_for_path_state(path: &Path, expected: bool, timeout: Duration) -> Result<()> {
        let deadline = tokio::time::Instant::now()
            .checked_add(timeout)
            .context("path-state wait deadline overflow")?;
        loop {
            if path.exists() == expected {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                bail!(
                    "timed out waiting for {} to {}",
                    path.display(),
                    if expected { "exist" } else { "be removed" }
                );
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

    async fn wait_for_terminal_status(
        operations: &RuntimeOperations,
        operation_id: &str,
        expected: &str,
        timeout: Duration,
    ) -> Result<Value> {
        let deadline = tokio::time::Instant::now()
            .checked_add(timeout)
            .context("operation-status wait deadline overflow")?;
        loop {
            let operation = operations.status(operation_id)?;
            let status = operation
                .get("status")
                .and_then(Value::as_str)
                .context("runtime operation status is missing")?;
            if status == expected {
                return Ok(operation);
            }
            if matches!(
                status,
                "completed" | "dispatched" | "failed" | "canceled" | "timed_out"
            ) {
                bail!("runtime operation reached `{status}` instead of `{expected}`: {operation}");
            }
            if tokio::time::Instant::now() >= deadline {
                bail!(
                    "timed out waiting for runtime operation `{operation_id}` to reach `{expected}`: {operation}"
                );
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    }

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
            Arc::new(UnavailableModuleTransport::basecamp_host_not_configured());

        let error = execute_backup_catalog_upload(&request, &operation_control(), transport)
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

        let outcome = execute_backup_catalog_download_in_dir(
            &request,
            &operation_control(),
            Some(base_dir.clone()),
        )
        .await?;
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

        let outcome = execute_backup_catalog_download_in_dir(
            &request,
            &operation_control(),
            Some(base_dir.clone()),
        )
        .await?;
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

        let error = execute_backup_catalog_download_in_dir(
            &request,
            &operation_control(),
            Some(base_dir.clone()),
        )
        .await
        .err()
        .context("invalid backup JSON should fail")?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("invalid JSON test server panicked"))??;

        anyhow::ensure!(
            format!("{error:#}").contains("settings backup CID cid-invalid did not contain JSON"),
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

            let error = execute_backup_catalog_download_in_dir(
                &request,
                &operation_control(),
                Some(base_dir.clone()),
            )
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

        let error = execute_backup_catalog_download_in_dir(
            &request,
            &operation_control(),
            Some(base_dir.clone()),
        )
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

            let error = execute_backup_catalog_download_in_dir(
                &request,
                &operation_control(),
                Some(base_dir.clone()),
            )
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
            Arc::new(UnavailableModuleTransport::basecamp_host_not_configured());

        let error = execute_payload_upload(&request, &operation_control(), transport)
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
    async fn payload_upload_observes_control_before_serialization_or_transport() -> Result<()> {
        let request = runtime_operation_request_from_value(json!({
            "domain": "storage",
            "method": "storageUploadPayload",
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://127.0.0.1:1" }
            },
            "mutating_enabled": true,
            "payload": {
                "filename": "must-not-upload.json",
                "payload": { "kind": "must-not-serialize" },
                "block_size": 65536
            }
        }))?;
        let control = operation_control();
        control.cancellation().cancel();
        let transport: SharedModuleTransport =
            Arc::new(UnavailableModuleTransport::basecamp_host_not_configured());

        let error = execute_payload_upload(&request, &control, transport)
            .await
            .err()
            .context("canceled payload upload unexpectedly started")?;

        anyhow::ensure!(
            error.downcast_ref::<OperationInterrupted>().is_some(),
            "payload upload lost typed pre-effect interruption: {error:#}"
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
            Arc::new(UnavailableModuleTransport::basecamp_host_not_configured());

        let outcome = execute_payload_upload(&request, &operation_control(), transport).await?;
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
