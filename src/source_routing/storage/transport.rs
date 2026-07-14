use std::{
    collections::HashSet,
    fmt,
    path::Path,
    sync::{Mutex, MutexGuard, OnceLock, TryLockError},
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use reqwest::{Method, Response, header};
use serde::Deserialize;
use serde_json::{Value, json};
use tokio_util::io::ReaderStream;
use tokio_util::sync::CancellationToken;

use crate::{
    modules::logos_core::{ModuleTransportKind, SharedModuleTransport},
    source_routing::shared::{http, module_bridge},
    source_routing::{ModuleDispatchIdentityRole, ModuleDispatchReceipt},
    support::{
        command_runner::{
            CommandCleanupUnconfirmed, CommandControl, CommandStopReason, CommandTerminated,
            CommandTerminationScope,
        },
        raw_source_transport::{request_bytes_bounded, request_success},
    },
};

const BACKUP_DOWNLOAD_FILENAME: &str = "settings-backup.json";
const BACKUP_DOWNLOAD_CHUNK_SIZE: i64 = 65_536;
const STORAGE_DOWNLOAD_CANCEL_TIMEOUT_MS: u64 = 15_000;
const STORAGE_DOWNLOAD_CANCEL_COMMAND_TIMEOUT_MS: u64 = 16_000;
const STORAGE_DOWNLOAD_TERMINATION_HANDSHAKE_GRACE_MS: u64 = 18_000;
pub(super) const BACKUP_DOWNLOAD_CANCEL_COMMAND_TIMEOUT: Duration =
    Duration::from_millis(STORAGE_DOWNLOAD_CANCEL_COMMAND_TIMEOUT_MS);
pub(super) const BACKUP_DOWNLOAD_TERMINATION_HANDSHAKE_GRACE: Duration =
    Duration::from_millis(STORAGE_DOWNLOAD_TERMINATION_HANDSHAKE_GRACE_MS);
const BACKUP_DOWNLOAD_CANCEL_RETRY_INTERVAL: Duration = Duration::from_millis(25);
const BACKUP_DOWNLOAD_SIZE_POLL_INTERVAL: Duration = Duration::from_millis(25);
const MAX_UNRELATED_DOWNLOAD_EVENTS: usize = 64;
const STORAGE_DOWNLOAD_PROTOCOL: &str = "logos.storage.download";
const STORAGE_DOWNLOAD_PROTOCOL_VERSION: u64 = 2;
const STORAGE_DOWNLOAD_PROTOCOL_METHOD: &str = "downloadProtocol";
const STORAGE_DOWNLOAD_METHOD: &str = "downloadToUrlV2";
const STORAGE_DOWNLOAD_METHOD_SIGNATURE: &str =
    "downloadToUrlV2(QString,QString,bool,int,QString,int)";
const STORAGE_DOWNLOAD_CANCEL_METHOD: &str = "downloadCancelV2";
const STORAGE_DOWNLOAD_DONE_EVENT: &str = "storageDownloadDoneV2";

const _: () =
    assert!(STORAGE_DOWNLOAD_CANCEL_TIMEOUT_MS < STORAGE_DOWNLOAD_CANCEL_COMMAND_TIMEOUT_MS);
const _: () = assert!(
    STORAGE_DOWNLOAD_CANCEL_COMMAND_TIMEOUT_MS < STORAGE_DOWNLOAD_TERMINATION_HANDSHAKE_GRACE_MS
);

static CLI_BACKUP_DOWNLOAD_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug, PartialEq, Eq)]
enum DownloadTerminalEvent {
    Unrelated,
    Succeeded,
    Canceled,
    Failed(String),
}

#[derive(Debug, Deserialize)]
struct DownloadDonePayload {
    protocol: String,
    version: u64,
    #[serde(rename = "moduleOperationId")]
    operation_id: String,
    cid: String,
    outcome: DownloadTerminalOutcome,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DownloadTerminalOutcome {
    Succeeded,
    Failed,
    Canceled,
}

#[derive(Debug, Deserialize)]
struct DownloadDispatchAcknowledgement {
    protocol: String,
    version: u64,
    accepted: bool,
    #[serde(rename = "moduleOperationId")]
    operation_id: String,
    cid: String,
}

#[derive(Debug, Deserialize)]
struct DownloadCancelAcknowledgement {
    protocol: String,
    version: u64,
    #[serde(rename = "moduleOperationId")]
    operation_id: String,
    #[serde(rename = "cancelStatus")]
    status: DownloadCancelDisposition,
    #[serde(default)]
    cid: Option<String>,
    #[serde(default, rename = "terminalOutcome")]
    terminal_outcome: Option<DownloadTerminalOutcome>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum DownloadCancelDisposition {
    Canceled,
    AlreadyTerminal,
    NotFound,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadCancelExpectation {
    EffectUnknown,
    Accepted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DownloadCancelSettlement {
    Settled,
    RetryNotFound,
}

#[derive(Debug)]
pub(crate) struct BackupDownloadCleanupUnconfirmed {
    stop_reason: Option<CommandStopReason>,
    message: String,
}

impl BackupDownloadCleanupUnconfirmed {
    fn new(primary: &anyhow::Error, message: String) -> Self {
        Self {
            stop_reason: primary
                .downcast_ref::<CommandTerminated>()
                .map(CommandTerminated::reason)
                .or_else(|| {
                    primary
                        .downcast_ref::<CommandCleanupUnconfirmed>()
                        .and_then(CommandCleanupUnconfirmed::reason)
                }),
            message,
        }
    }

    fn append(&self, suffix: impl fmt::Display) -> Self {
        Self {
            stop_reason: self.stop_reason,
            message: format!("{}; {suffix}", self.message),
        }
    }

    #[must_use]
    pub(crate) const fn stop_reason(&self) -> Option<CommandStopReason> {
        self.stop_reason
    }
}

impl fmt::Display for BackupDownloadCleanupUnconfirmed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BackupDownloadCleanupUnconfirmed {}

#[derive(Debug, Deserialize)]
struct DownloadProtocolContract {
    protocol: String,
    version: u64,
    #[serde(rename = "moduleOperationIdOwner")]
    operation_id_owner: String,
    #[serde(rename = "cancelTimeoutMs")]
    cancel_timeout_ms: u64,
    #[serde(rename = "maxDownloadBytes")]
    max_download_bytes: u64,
}

pub(super) async fn module_call(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    method: &'static str,
    args: Vec<Value>,
) -> Result<Value> {
    module_bridge::call_value(
        transport,
        transport_kind,
        super::layer::module_id(),
        method,
        args,
    )
    .await
    .map(|reply| reply.into_value())
}

pub(super) async fn module_dispatch(
    transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    method: &'static str,
    args: Vec<Value>,
    context: &[(&'static str, String)],
    identity_role: ModuleDispatchIdentityRole,
) -> Result<ModuleDispatchReceipt> {
    let reply = module_bridge::call_value(
        transport,
        transport_kind,
        super::layer::module_id(),
        method,
        args,
    )
    .await?;
    Ok(module_bridge::dispatch_result(
        super::layer::module_id(),
        method,
        reply,
        context,
        identity_role,
    ))
}

pub(super) async fn manifests(endpoint: &str) -> Result<Value> {
    crate::rpc::raw_http_json(endpoint, "/data").await
}

pub(super) async fn manifest(endpoint: &str, cid: &str) -> Result<Value> {
    crate::rpc::raw_http_json(endpoint, &format!("/data/{cid}/network/manifest")).await
}

pub(super) async fn exists(endpoint: &str, cid: &str) -> Result<Value> {
    crate::rpc::raw_http_json(endpoint, &format!("/data/{cid}/exists")).await
}

pub(super) async fn probe_value(endpoint: &str, path: &str) -> Result<Value> {
    let url = http::rest_url(endpoint, path);
    let text = http::raw_http_text_url(&url).await?;
    Ok(parse_probe_text(&text))
}

pub(super) async fn probe_metrics(endpoint: &str) -> Result<String> {
    http::raw_http_text_url(endpoint).await
}

pub(super) async fn fetch(endpoint: &str, cid: &str) -> Result<Value> {
    http::rest_json_request(
        Method::POST,
        endpoint,
        &format!("/data/{cid}/network"),
        None,
    )
    .await
}

pub(super) async fn upload(endpoint: &str, path: &str, block_size: u64) -> Result<Value> {
    let file = tokio::fs::File::open(path)
        .await
        .with_context(|| format!("failed to open upload file `{path}`"))?;
    let bytes = file
        .metadata()
        .await
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let filename = Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty());
    let body = reqwest::Body::wrap_stream(ReaderStream::new(file));
    let mut request = reqwest::Client::new()
        .post(http::rest_url(
            endpoint,
            &format!("/data?blockSize={block_size}"),
        ))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(body);
    if let Some(filename) = filename {
        request = request.header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                filename.replace(['\\', '"'], "_")
            ),
        );
    }
    let text = http::send_text(request, "storage upload").await?;
    Ok(json!({
        "cid": text.trim(),
        "path": path,
        "bytes": bytes,
        "endpoint": endpoint,
    }))
}

pub(super) async fn upload_bytes(
    endpoint: &str,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
) -> Result<Value> {
    let text = http::send_text(
        reqwest::Client::new()
            .post(http::rest_url(
                endpoint,
                &format!("/data?blockSize={block_size}"),
            ))
            .header(header::CONTENT_TYPE, "application/json")
            .header(
                header::CONTENT_DISPOSITION,
                format!(
                    "attachment; filename=\"{}\"",
                    filename.replace(['\\', '"'], "_")
                ),
            )
            .body(bytes.to_vec()),
        "storage settings backup upload",
    )
    .await?;
    Ok(json!({
        "cid": text.trim(),
        "filename": filename,
        "bytes": bytes.len(),
        "endpoint": endpoint,
    }))
}

pub(super) async fn upload_bytes_controlled(
    endpoint: &str,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
    control: CommandControl,
) -> Result<Value> {
    controlled_remote(control, upload_bytes(endpoint, filename, bytes, block_size)).await
}

pub(super) async fn module_upload_bytes_controlled(
    runtime: crate::modules::logos_core::LogoscoreCliRuntime,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
    control: CommandControl,
) -> Result<Value> {
    let filename = filename.to_owned();
    let bytes = bytes.to_vec();
    let worker_guard = control.blocking_worker_guard()?;
    blocking_module_call("Storage module payload upload", move || {
        let _worker_guard = worker_guard;
        module_upload_bytes_blocking_controlled(&runtime, &filename, &bytes, block_size, control)
    })
    .await
}

fn module_upload_bytes_blocking_controlled(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
    control: CommandControl,
) -> Result<Value> {
    let block_size = i64::try_from(block_size).context("storage upload block size is too large")?;
    let safe_filename = Path::new(filename)
        .file_name()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .context("storage upload filename is invalid")?;
    control.check_active()?;
    let staged = runtime.stage_shared_file(safe_filename, bytes)?;
    let path = staged
        .path()
        .to_str()
        .context("temporary storage upload path is not UTF-8")?
        .to_owned();

    runtime.require_module_method_controlled(
        super::layer::module_id(),
        "uploadUrl",
        "uploadUrl(QString,int)",
        control.clone(),
    )?;
    runtime.require_module_method_controlled(
        super::layer::module_id(),
        "manifests",
        "manifests()",
        control.clone(),
    )?;
    let manifests_before = logoscore_cli_call_value_controlled_with_runtime(
        runtime,
        super::layer::module_id(),
        "manifests",
        &[],
        control.clone(),
    )?;
    let baseline_cids = manifest_cids(&manifests_before);
    let session = logoscore_cli_call_value_controlled_with_runtime(
        runtime,
        super::layer::module_id(),
        "uploadUrl",
        &[path, block_size.to_string()],
        control.clone(),
    )?;
    let session_id = session
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("storage_module.uploadUrl returned no session ID")?
        .to_owned();
    let cid = loop {
        control.check_active()?;
        let manifests = logoscore_cli_call_value_controlled_with_runtime(
            runtime,
            super::layer::module_id(),
            "manifests",
            &[],
            control.clone(),
        )?;
        if let Some(cid) = new_manifest_cid(&manifests, safe_filename, bytes.len(), &baseline_cids)
        {
            break cid;
        }
        controlled_thread_sleep(&control, Duration::from_millis(100))?;
    };
    Ok(json!({
        "cid": cid,
        "filename": safe_filename,
        "bytes": bytes.len(),
        "endpoint": "logoscore call storage_module.uploadUrl",
        "completion": "manifest_poll",
        "sessionId": session_id,
    }))
}

fn manifest_cids(manifests: &Value) -> HashSet<String> {
    manifests
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|manifest| manifest.get("cid").and_then(Value::as_str))
        .map(ToOwned::to_owned)
        .collect()
}

fn new_manifest_cid(
    manifests: &Value,
    filename: &str,
    bytes: usize,
    baseline_cids: &HashSet<String>,
) -> Option<String> {
    let bytes = u64::try_from(bytes).ok()?;
    manifests.as_array()?.iter().find_map(|manifest| {
        let cid = manifest.get("cid")?.as_str()?.trim();
        let candidate_filename = manifest.get("filename")?.as_str()?;
        let candidate_bytes = manifest
            .get("datasetSize")
            .or_else(|| manifest.get("dataset_size"))?
            .as_u64()?;
        (candidate_filename == filename
            && candidate_bytes == bytes
            && !cid.is_empty()
            && !baseline_cids.contains(cid))
        .then(|| cid.to_owned())
    })
}

pub(super) async fn module_download_backup_bytes_controlled(
    runtime: crate::modules::logos_core::LogoscoreCliRuntime,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
    control: CommandControl,
) -> Result<Vec<u8>> {
    let cid = cid.to_owned();
    let worker_guard = control.blocking_worker_guard()?;
    blocking_module_call("Storage module backup download", move || {
        let _worker_guard = worker_guard;
        module_download_backup_bytes_blocking_controlled(
            &runtime, &cid, local_only, max_bytes, control,
        )
    })
    .await
}

fn module_download_backup_bytes_blocking_controlled(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
    control: CommandControl,
) -> Result<Vec<u8>> {
    module_download_backup_bytes_blocking_controlled_with_ready_hook(
        runtime,
        cid,
        local_only,
        max_bytes,
        control,
        |_| Ok(()),
    )
}

fn module_download_backup_bytes_blocking_controlled_with_ready_hook<F>(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
    control: CommandControl,
    after_watch_ready: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&Path) -> Result<()>,
{
    anyhow::ensure!(
        max_bytes > 0,
        "storage download byte limit must be positive"
    );
    let _download_permit = acquire_cli_backup_download(&control)?;
    runtime.require_module_contract_controlled(
        super::layer::module_id(),
        &[
            (STORAGE_DOWNLOAD_PROTOCOL_METHOD, "downloadProtocol()"),
            (STORAGE_DOWNLOAD_METHOD, STORAGE_DOWNLOAD_METHOD_SIGNATURE),
            (STORAGE_DOWNLOAD_CANCEL_METHOD, "downloadCancelV2(QString)"),
        ],
        &[(
            STORAGE_DOWNLOAD_DONE_EVENT,
            "storageDownloadDoneV2(QString)",
        )],
        control.clone(),
    )?;
    let protocol = logoscore_cli_call_value_controlled_with_runtime(
        runtime,
        super::layer::module_id(),
        STORAGE_DOWNLOAD_PROTOCOL_METHOD,
        &[],
        control.clone(),
    )?;
    validate_storage_download_protocol(&protocol, max_bytes)?;
    control.check_active()?;
    let staged = runtime.stage_shared_download(BACKUP_DOWNLOAD_FILENAME)?;
    let result = execute_staged_cli_backup_download(
        runtime,
        cid,
        local_only,
        max_bytes,
        &control,
        &staged,
        after_watch_ready,
    );
    let cleanup = staged.close();
    match (result, cleanup) {
        (Ok(bytes), Ok(())) => Ok(bytes),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(cleanup)) => {
            let primary = cleanup.context("logoscore download staging cleanup failed");
            let message = primary.to_string();
            Err(BackupDownloadCleanupUnconfirmed::new(&primary, message).into())
        }
        (Err(error), Err(cleanup)) => {
            if let Some(unconfirmed) = error.downcast_ref::<BackupDownloadCleanupUnconfirmed>() {
                return Err(unconfirmed
                    .append(format_args!(
                        "logoscore download staging cleanup failed: {cleanup:#}"
                    ))
                    .into());
            }
            let message =
                format!("{error}; logoscore download staging cleanup failed: {cleanup:#}");
            Err(BackupDownloadCleanupUnconfirmed::new(&error, message).into())
        }
    }
}

fn execute_staged_cli_backup_download<F>(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
    control: &CommandControl,
    staged: &crate::modules::logos_core::LogoscoreSharedDownload,
    after_watch_ready: F,
) -> Result<Vec<u8>>
where
    F: FnOnce(&Path) -> Result<()>,
{
    let path = staged
        .path()
        .to_str()
        .context("temporary storage download path is not UTF-8")?
        .to_owned();
    let operation_id = new_storage_download_operation_id()?;
    let mut watch = match runtime.start_event_watch(
        super::layer::module_id(),
        STORAGE_DOWNLOAD_DONE_EVENT,
        control,
    ) {
        Ok(watch) => watch,
        Err(error)
            if error
                .downcast_ref::<crate::modules::logos_core::LogoscoreWatchCleanupUnconfirmed>()
                .is_some() =>
        {
            let message = error.to_string();
            return Err(BackupDownloadCleanupUnconfirmed::new(&error, message).into());
        }
        Err(error) => return Err(error),
    };
    if let Err(error) = watch.wait_ready(control) {
        return cleanup_watch_error(error, &mut watch);
    }
    if let Err(error) = after_watch_ready(staged.path()) {
        return cleanup_watch_error(error, &mut watch);
    }
    let acknowledgement = match logoscore_cli_call_value_controlled_with_runtime(
        runtime,
        super::layer::module_id(),
        STORAGE_DOWNLOAD_METHOD,
        &[
            cid.to_owned(),
            path,
            local_only.to_string(),
            BACKUP_DOWNLOAD_CHUNK_SIZE.to_string(),
            operation_id.clone(),
            max_bytes.to_string(),
        ],
        control.clone(),
    ) {
        Ok(acknowledgement) => acknowledgement,
        Err(error) => {
            if error
                .downcast_ref::<CommandTerminated>()
                .is_some_and(|terminated| terminated.scope() == CommandTerminationScope::NoProcess)
            {
                return cleanup_watch_error(error, &mut watch);
            }
            return cleanup_active_download_error(
                runtime,
                &operation_id,
                cid,
                DownloadCancelExpectation::EffectUnknown,
                error,
                &mut watch,
            );
        }
    };
    if let Err(error) =
        decode_download_dispatch_acknowledgement(&acknowledgement, &operation_id, cid)
    {
        return cleanup_active_download_error(
            runtime,
            &operation_id,
            cid,
            DownloadCancelExpectation::EffectUnknown,
            error,
            &mut watch,
        );
    }

    let terminal = match wait_for_download_terminal(
        &mut watch,
        &operation_id,
        cid,
        staged.path(),
        max_bytes,
        control,
    ) {
        Ok(terminal) => terminal,
        Err(error) => {
            return cleanup_active_download_error(
                runtime,
                &operation_id,
                cid,
                DownloadCancelExpectation::Accepted,
                error,
                &mut watch,
            );
        }
    };
    match terminal {
        DownloadTerminalEvent::Succeeded => {
            complete_terminal_download_watch(&mut watch, Ok(()))?;
            staged.read_bounded(max_bytes)
        }
        DownloadTerminalEvent::Canceled => complete_terminal_download_watch(
            &mut watch,
            Err(anyhow::anyhow!(
                "storage_module download was canceled for CID `{cid}`"
            )),
        ),
        DownloadTerminalEvent::Failed(error) => complete_terminal_download_watch(
            &mut watch,
            Err(anyhow::anyhow!(
                "storage_module download failed for CID `{cid}`: {error}"
            )),
        ),
        DownloadTerminalEvent::Unrelated => {
            bail!("storage download terminal wait returned an unrelated event")
        }
    }
}

fn complete_terminal_download_watch<T>(
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
    terminal_result: Result<T>,
) -> Result<T> {
    match (terminal_result, watch.stop()) {
        (result, Ok(())) => result,
        (Ok(_), Err(cleanup)) => {
            let primary = anyhow::anyhow!("storage download reached a terminal event");
            let message =
                format!("{primary}; logoscore download terminal watch cleanup failed: {cleanup:#}");
            Err(BackupDownloadCleanupUnconfirmed::new(&primary, message).into())
        }
        (Err(primary), Err(cleanup)) => {
            let message =
                format!("{primary}; logoscore download terminal watch cleanup failed: {cleanup:#}");
            Err(BackupDownloadCleanupUnconfirmed::new(&primary, message).into())
        }
    }
}

fn new_storage_download_operation_id() -> Result<String> {
    let mut random = [0_u8; 24];
    getrandom::fill(&mut random).context("failed to generate storage download operation ID")?;
    Ok(format!("storage-download-{}", hex::encode(random)))
}

fn validate_storage_download_protocol(value: &Value, requested_max_bytes: usize) -> Result<()> {
    let contract: DownloadProtocolContract = serde_json::from_value(value.clone())
        .context("storage_module.downloadProtocol returned an invalid contract")?;
    let requested_max_bytes =
        u64::try_from(requested_max_bytes).context("storage download byte limit is too large")?;
    anyhow::ensure!(
        contract.protocol == STORAGE_DOWNLOAD_PROTOCOL
            && contract.version == STORAGE_DOWNLOAD_PROTOCOL_VERSION
            && contract.operation_id_owner == "caller"
            && contract.cancel_timeout_ms == STORAGE_DOWNLOAD_CANCEL_TIMEOUT_MS
            && requested_max_bytes > 0
            && contract.max_download_bytes >= requested_max_bytes,
        "storage_module download protocol is incompatible"
    );
    Ok(())
}

fn decode_download_dispatch_acknowledgement(
    value: &Value,
    expected_operation_id: &str,
    expected_cid: &str,
) -> Result<()> {
    let acknowledgement: DownloadDispatchAcknowledgement = serde_json::from_value(value.clone())
        .context("storage_module.downloadToUrlV2 returned an invalid acknowledgement")?;
    anyhow::ensure!(
        acknowledgement.protocol == STORAGE_DOWNLOAD_PROTOCOL
            && acknowledgement.version == STORAGE_DOWNLOAD_PROTOCOL_VERSION,
        "storage_module download acknowledgement has an incompatible protocol"
    );
    anyhow::ensure!(
        acknowledgement.accepted,
        "storage_module download acknowledgement was not accepted"
    );
    anyhow::ensure!(
        acknowledgement.operation_id == expected_operation_id,
        "storage_module download acknowledgement returned the wrong operation ID"
    );
    anyhow::ensure!(
        acknowledgement.cid == expected_cid,
        "storage_module download acknowledgement returned the wrong CID"
    );
    anyhow::ensure!(
        acknowledgement.operation_id != acknowledgement.cid,
        "storage_module download operation ID must differ from its CID"
    );
    Ok(())
}

fn acquire_cli_backup_download(control: &CommandControl) -> Result<MutexGuard<'static, ()>> {
    let lock = CLI_BACKUP_DOWNLOAD_LOCK.get_or_init(|| Mutex::new(()));
    loop {
        control.check_active()?;
        match lock.try_lock() {
            Ok(permit) => return Ok(permit),
            Err(TryLockError::Poisoned(_)) => {
                bail!("Storage CLI backup download lock is poisoned")
            }
            Err(TryLockError::WouldBlock) => {
                controlled_thread_sleep(control, Duration::from_millis(25))?;
            }
        }
    }
}

fn wait_for_download_terminal(
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
    operation_id: &str,
    cid: &str,
    staged_path: &Path,
    max_bytes: usize,
    control: &CommandControl,
) -> Result<DownloadTerminalEvent> {
    let mut unrelated = 0_usize;
    loop {
        ensure_staged_download_within_limit(staged_path, max_bytes)?;
        let Some(value) = watch.next_value_within(control, BACKUP_DOWNLOAD_SIZE_POLL_INTERVAL)?
        else {
            continue;
        };
        match decode_download_terminal_event(&value, operation_id, cid)? {
            DownloadTerminalEvent::Unrelated => {
                unrelated = unrelated.saturating_add(1);
                if unrelated > MAX_UNRELATED_DOWNLOAD_EVENTS {
                    bail!(
                        "storage download received more than {MAX_UNRELATED_DOWNLOAD_EVENTS} unrelated terminal events"
                    );
                }
            }
            terminal => return Ok(terminal),
        }
    }
}

fn ensure_staged_download_within_limit(path: &Path, max_bytes: usize) -> Result<()> {
    let max_bytes = u64::try_from(max_bytes).context("storage download byte limit is too large")?;
    let bytes = match std::fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => {
            return Err(error).with_context(|| {
                format!("failed to inspect storage download `{}`", path.display())
            });
        }
    };
    anyhow::ensure!(
        bytes <= max_bytes,
        "logoscore download exceeded {max_bytes} byte limit before terminal completion"
    );
    Ok(())
}

fn decode_download_terminal_event(
    value: &Value,
    expected_operation_id: &str,
    expected_cid: &str,
) -> Result<DownloadTerminalEvent> {
    if value.get("module").and_then(Value::as_str) != Some(super::layer::module_id()) {
        bail!("logoscore download watcher returned an event for the wrong module");
    }
    if value.get("event").and_then(Value::as_str) != Some(STORAGE_DOWNLOAD_DONE_EVENT) {
        bail!("logoscore download watcher returned the wrong event type");
    }
    let data = value
        .get("data")
        .and_then(Value::as_object)
        .context("logoscore download terminal event has no data object")?;
    if data.len() != 1 || !data.contains_key("arg0") {
        bail!("logoscore download terminal event must contain exactly one payload argument");
    }
    let payload_text = data
        .get("arg0")
        .and_then(Value::as_str)
        .context("logoscore download terminal payload must be a JSON string")?;
    let payload: DownloadDonePayload = serde_json::from_str(payload_text)
        .context("logoscore download terminal payload is invalid JSON")?;
    anyhow::ensure!(
        payload.protocol == STORAGE_DOWNLOAD_PROTOCOL
            && payload.version == STORAGE_DOWNLOAD_PROTOCOL_VERSION,
        "logoscore download terminal payload has an incompatible protocol"
    );
    let operation_id = payload.operation_id.trim();
    let cid = payload.cid.trim();
    if operation_id.is_empty() {
        bail!("logoscore download terminal payload has no operation ID");
    }
    if cid.is_empty() {
        bail!("logoscore download terminal payload has no CID");
    }
    if operation_id != expected_operation_id {
        return Ok(DownloadTerminalEvent::Unrelated);
    }
    anyhow::ensure!(
        cid == expected_cid,
        "logoscore download terminal payload returned the wrong CID for its operation ID"
    );
    anyhow::ensure!(
        operation_id != cid,
        "logoscore download terminal operation ID must differ from its CID"
    );
    let error = payload.error.as_deref().map(str::trim).unwrap_or_default();
    match payload.outcome {
        DownloadTerminalOutcome::Succeeded => {
            if !error.is_empty() {
                bail!("successful logoscore download terminal payload contains an error");
            }
            Ok(DownloadTerminalEvent::Succeeded)
        }
        DownloadTerminalOutcome::Failed => {
            if error.is_empty() {
                bail!("failed logoscore download terminal payload contains no error");
            }
            Ok(DownloadTerminalEvent::Failed(error.to_owned()))
        }
        DownloadTerminalOutcome::Canceled => {
            if !error.is_empty() {
                bail!("canceled logoscore download terminal payload contains an error");
            }
            Ok(DownloadTerminalEvent::Canceled)
        }
    }
}

fn cleanup_watch_error<T>(
    primary: anyhow::Error,
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
) -> Result<T> {
    match watch.stop() {
        Ok(()) => Err(primary),
        Err(cleanup) => {
            let message =
                format!("{primary}; logoscore download watch cleanup failed: {cleanup:#}");
            Err(BackupDownloadCleanupUnconfirmed::new(&primary, message).into())
        }
    }
}

fn cleanup_active_download_error<T>(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    operation_id: &str,
    cid: &str,
    expectation: DownloadCancelExpectation,
    primary: anyhow::Error,
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
) -> Result<T> {
    let cancel = cancel_module_download(runtime, operation_id, cid, expectation);
    let stop = watch.stop();
    match (cancel, stop) {
        (Ok(()), Ok(())) => Err(primary),
        (cancel, stop) => Err(BackupDownloadCleanupUnconfirmed::new(
            &primary,
            format!(
                "{primary}; storage download cleanup was not confirmed: cancel={}, watch={}",
                cleanup_result_text(cancel),
                cleanup_result_text(stop)
            ),
        )
        .into()),
    }
}

fn cancel_module_download(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    operation_id: &str,
    expected_cid: &str,
    expectation: DownloadCancelExpectation,
) -> Result<()> {
    cancel_module_download_with_timeout(
        runtime,
        operation_id,
        expected_cid,
        expectation,
        BACKUP_DOWNLOAD_CANCEL_COMMAND_TIMEOUT,
    )
}

fn cancel_module_download_with_timeout(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    operation_id: &str,
    expected_cid: &str,
    expectation: DownloadCancelExpectation,
    timeout: Duration,
) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("storage download cleanup deadline overflow")?;
    let control = CommandControl::new(CancellationToken::new(), deadline);
    cancel_module_download_controlled(runtime, operation_id, expected_cid, expectation, control)
}

fn cancel_module_download_controlled(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    operation_id: &str,
    expected_cid: &str,
    expectation: DownloadCancelExpectation,
    control: CommandControl,
) -> Result<()> {
    loop {
        let acknowledgement = logoscore_cli_call_value_controlled_with_runtime(
            runtime,
            super::layer::module_id(),
            STORAGE_DOWNLOAD_CANCEL_METHOD,
            &[operation_id.to_owned()],
            control.clone(),
        )?;
        match validate_download_cancel_acknowledgement(
            acknowledgement,
            operation_id,
            expected_cid,
            expectation,
        )? {
            DownloadCancelSettlement::Settled => return Ok(()),
            DownloadCancelSettlement::RetryNotFound => {
                controlled_thread_sleep(&control, BACKUP_DOWNLOAD_CANCEL_RETRY_INTERVAL)?;
            }
        }
    }
}

fn validate_download_cancel_acknowledgement(
    value: Value,
    operation_id: &str,
    expected_cid: &str,
    expectation: DownloadCancelExpectation,
) -> Result<DownloadCancelSettlement> {
    let acknowledgement: DownloadCancelAcknowledgement = serde_json::from_value(value)
        .context("storage_module.downloadCancelV2 returned an invalid acknowledgement")?;
    anyhow::ensure!(
        acknowledgement.protocol == STORAGE_DOWNLOAD_PROTOCOL
            && acknowledgement.version == STORAGE_DOWNLOAD_PROTOCOL_VERSION,
        "storage_module cancellation acknowledgement has an incompatible protocol"
    );
    anyhow::ensure!(
        acknowledgement.operation_id == operation_id,
        "storage_module cancellation acknowledgement returned the wrong operation ID"
    );
    match acknowledgement.status {
        DownloadCancelDisposition::Canceled => {
            anyhow::ensure!(
                acknowledgement.cid.as_deref() == Some(expected_cid)
                    && acknowledgement.terminal_outcome.is_none(),
                "storage_module canceled acknowledgement has invalid operation context"
            );
            Ok(DownloadCancelSettlement::Settled)
        }
        DownloadCancelDisposition::AlreadyTerminal => {
            anyhow::ensure!(
                acknowledgement.cid.as_deref() == Some(expected_cid)
                    && acknowledgement.terminal_outcome.is_some(),
                "storage_module terminal cancellation acknowledgement has invalid operation context"
            );
            Ok(DownloadCancelSettlement::Settled)
        }
        DownloadCancelDisposition::NotFound => {
            anyhow::ensure!(
                expectation == DownloadCancelExpectation::EffectUnknown
                    && acknowledgement.cid.is_none()
                    && acknowledgement.terminal_outcome.is_none(),
                "storage_module lost an accepted download during cancellation"
            );
            Ok(DownloadCancelSettlement::RetryNotFound)
        }
    }
}

fn cleanup_result_text(result: Result<()>) -> String {
    match result {
        Ok(()) => "ok".to_owned(),
        Err(error) => format!("{error:#}"),
    }
}

fn logoscore_cli_call_value_controlled_with_runtime(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    module: &str,
    method: &str,
    args: &[String],
    control: CommandControl,
) -> Result<Value> {
    let output = runtime.call_controlled(module, method, args, control)?;
    crate::modules::logos_core::normalize_module_call_value(module, method, output.value)
}

pub(super) async fn download_bytes(
    endpoint: &str,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
) -> Result<Vec<u8>> {
    let route = download_route(cid, local_only)?;
    let url = http::rest_url(endpoint, &route);
    request_bytes_bounded(
        reqwest::Client::new().get(&url),
        &url,
        "failed to read storage download body",
        max_bytes,
    )
    .await
}

pub(super) async fn download_bytes_controlled(
    endpoint: &str,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
    control: CommandControl,
) -> Result<Vec<u8>> {
    controlled_remote(
        control,
        download_bytes(endpoint, cid, local_only, max_bytes),
    )
    .await
}

pub(super) async fn download_response(
    endpoint: &str,
    cid: &str,
    local_only: bool,
) -> Result<Response> {
    let url = http::rest_url(endpoint, &download_route(cid, local_only)?);
    request_success(
        reqwest::Client::new().get(&url),
        &url,
        "storage download",
        "failed to read storage download error body",
    )
    .await
}

pub(super) async fn remove(endpoint: &str, cid: &str) -> Result<Value> {
    http::rest_empty_request(Method::DELETE, endpoint, &format!("/data/{cid}"), None).await?;
    Ok(json!({
        "removed": true,
        "cid": cid,
        "endpoint": endpoint,
    }))
}

fn download_route(cid: &str, local_only: bool) -> Result<String> {
    super::validate_backup_cid(cid)?;
    Ok(if local_only {
        format!("/data/{cid}")
    } else {
        format!("/data/{cid}/network/stream")
    })
}

async fn blocking_module_call<T, F>(label: &'static str, call: F) -> Result<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T> + Send + 'static,
{
    tokio::task::spawn_blocking(call)
        .await
        .with_context(|| format!("{label} worker failed"))?
}

async fn controlled_remote<T, F>(control: CommandControl, future: F) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    tokio::select! {
        biased;
        result = future => result,
        () = control.cancellation().cancelled() => command_interruption(&control),
        () = tokio::time::sleep_until(tokio::time::Instant::from_std(control.deadline())) => {
            command_interruption(&control)
        },
    }
}

fn controlled_thread_sleep(control: &CommandControl, duration: Duration) -> Result<()> {
    control.check_active()?;
    let remaining = control
        .deadline()
        .saturating_duration_since(std::time::Instant::now());
    std::thread::sleep(duration.min(remaining));
    control.check_active().map_err(Into::into)
}

fn command_interruption<T>(control: &CommandControl) -> Result<T> {
    match control.check_active() {
        Err(error) => Err(error.into()),
        Ok(()) => bail!("controlled storage transfer stopped without cancellation evidence"),
    }
}

fn parse_probe_text(text: &str) -> Value {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    serde_json::from_str(trimmed).unwrap_or_else(|_| Value::String(trimmed.to_owned()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::{Read as _, Write as _},
        net::TcpListener,
        path::PathBuf,
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[cfg(unix)]
    static TEST_CLI_BACKUP_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[cfg(unix)]
    const SUCCEEDED_TERMINAL: &str = r#"{"protocol":"logos.storage.download","version":2,"moduleOperationId":"__OPERATION_ID__","cid":"__CID__","outcome":"succeeded"}"#;

    #[cfg(unix)]
    const FAILED_TERMINAL: &str = r#"{"protocol":"logos.storage.download","version":2,"moduleOperationId":"__OPERATION_ID__","cid":"__CID__","outcome":"failed","error":"not found"}"#;

    #[cfg(unix)]
    fn serialize_cli_backup_test() -> Result<std::sync::MutexGuard<'static, ()>> {
        TEST_CLI_BACKUP_LOCK
            .lock()
            .map_err(|_| anyhow::anyhow!("CLI backup test lock is poisoned"))
    }

    #[cfg(unix)]
    struct FakeDownloadRuntime {
        _directory: tempfile::TempDir,
        socket: PathBuf,
        runtime: crate::modules::logos_core::LogoscoreCliRuntime,
        trigger: PathBuf,
        canceled: PathBuf,
        cancel_attempts: PathBuf,
        cancel_not_found_once: PathBuf,
        cancel_always_not_found: PathBuf,
        cancel_failure: PathBuf,
        cancel_delay: PathBuf,
        cid_path: PathBuf,
        staging_path: PathBuf,
    }

    #[cfg(unix)]
    impl FakeDownloadRuntime {
        fn new(
            payload: &[u8],
            terminal_payload: Option<&str>,
            module_info: &Value,
        ) -> Result<Self> {
            Self::with_call_behavior(payload, terminal_payload, module_info, None, false)
        }

        fn with_call_behavior(
            payload: &[u8],
            terminal_payload: Option<&str>,
            module_info: &Value,
            call_reply: Option<&str>,
            block_call: bool,
        ) -> Result<Self> {
            use std::os::unix::fs::PermissionsExt as _;

            let directory = tempfile::tempdir()?;
            let root = directory.path();
            let payload_path = root.join("payload.json");
            let terminal_path = root.join("terminal.json");
            let module_info_path = root.join("module-info.json");
            let call_reply_path = root.join("call-reply.json");
            let block_call_path = root.join("block-download-call");
            let trigger = root.join("download-started");
            let canceled = root.join("download-canceled");
            let cancel_attempts = root.join("cancel-attempts");
            let cancel_not_found_once = root.join("cancel-not-found-once");
            let cancel_always_not_found = root.join("cancel-always-not-found");
            let cancel_failure = root.join("cancel-failure");
            let cancel_delay = root.join("cancel-delay");
            let cancel_first_seen = root.join("cancel-first-seen");
            let staging_path = root.join("staging-path");
            let operation_id_path = root.join("operation-id");
            let cid_path = root.join("cid");
            let program = root.join("logoscore-test");
            fs::write(&payload_path, payload)?;
            if let Some(terminal_payload) = terminal_payload {
                fs::write(
                    &terminal_path,
                    serde_json::to_vec(&json!({
                        "type": "event",
                        "protocol": "logoscore.watch",
                        "version": 1,
                        "timestamp": "2026-07-14T12:00:00Z",
                        "module": "storage_module",
                        "event": "storageDownloadDoneV2",
                        "data": { "arg0": terminal_payload },
                    }))?,
                )?;
            }
            fs::write(&module_info_path, serde_json::to_vec(module_info)?)?;
            if let Some(call_reply) = call_reply {
                fs::write(&call_reply_path, call_reply)?;
            }
            if block_call {
                fs::write(&block_call_path, b"block")?;
            }
            let script = format!(
                "#!/bin/sh\n\
                 if [ \"$1\" = \"--config-dir\" ]; then shift 2; fi\n\
                 case \"$1\" in\n\
                   list-modules) printf '%s\\n' '[{{\"name\":\"storage_module\",\"status\":\"loaded\"}}]' ;;\n\
                   module-info) cat {module_info} ;;\n\
                   watch)\n\
                     printf '%s\\n' '{{\"type\":\"subscription_ready\",\"protocol\":\"logoscore.watch\",\"version\":1,\"module\":\"storage_module\",\"event\":\"storageDownloadDoneV2\"}}'\n\
                     while [ ! -f {trigger} ]; do sleep 0.01; done\n\
                     if [ -f {terminal} ]; then\n\
                       operation_id=$(cat {operation_id}); cid=$(cat {cid})\n\
                       sed -e \"s/__OPERATION_ID__/$operation_id/g\" -e \"s/__CID__/$cid/g\" {terminal}; printf '\\n'\n\
                     fi\n\
                     while :; do sleep 1; done ;;\n\
                   call)\n\
                     case \"$3\" in\n\
                       downloadProtocol)\n\
                         printf '%s\\n' '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"moduleOperationIdOwner\":\"caller\",\"cancelTimeoutMs\":15000,\"maxDownloadBytes\":1073741824}},\"error\":null}}}}' ;;\n\
                       downloadToUrlV2)\n\
                         printf '%s' \"$5\" > {staging}\n\
                         cp {payload} \"$5\"\n\
                         printf '%s' \"$8\" > {operation_id}\n\
                         printf '%s' \"$4\" > {cid}\n\
                         touch {trigger}\n\
                         if [ -f {block_call} ]; then while :; do sleep 1; done; fi\n\
                         if [ -f {call_reply} ]; then cat {call_reply}; else\n\
                         printf '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"accepted\":true,\"moduleOperationId\":\"%s\",\"cid\":\"%s\"}},\"error\":null}}}}\\n' \"$8\" \"$4\"; fi ;;\n\
                       downloadCancelV2)\n\
                         printf x >> {cancel_attempts}\n\
                         if [ -f {cancel_failure} ]; then exit 10; fi\n\
                         if [ -f {cancel_delay} ]; then sleep \"$(cat {cancel_delay})\"; fi\n\
                         cancel_not_found=false\n\
                         if [ -f {cancel_always_not_found} ]; then cancel_not_found=true; fi\n\
                         if [ -f {cancel_not_found_once} ] && [ ! -f {cancel_first_seen} ]; then\n\
                           touch {cancel_first_seen}; cancel_not_found=true\n\
                         fi\n\
                         if [ \"$cancel_not_found\" = true ]; then\n\
                           printf '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"moduleOperationId\":\"%s\",\"cancelStatus\":\"not_found\"}},\"error\":null}}}}\\n' \"$4\"\n\
                         else\n\
                           touch {canceled}\n\
                           cid=$(cat {cid})\n\
                           printf '{{\"status\":\"ok\",\"result\":{{\"success\":true,\"value\":{{\"protocol\":\"logos.storage.download\",\"version\":2,\"moduleOperationId\":\"%s\",\"cid\":\"%s\",\"cancelStatus\":\"canceled\"}},\"error\":null}}}}\\n' \"$4\" \"$cid\"\n\
                         fi ;;\n\
                       *) exit 9 ;;\n\
                     esac ;;\n\
                   *) exit 8 ;;\n\
                 esac\n",
                module_info = shell_path(&module_info_path),
                call_reply = shell_path(&call_reply_path),
                block_call = shell_path(&block_call_path),
                trigger = shell_path(&trigger),
                terminal = shell_path(&terminal_path),
                operation_id = shell_path(&operation_id_path),
                cid = shell_path(&cid_path),
                staging = shell_path(&staging_path),
                payload = shell_path(&payload_path),
                canceled = shell_path(&canceled),
                cancel_attempts = shell_path(&cancel_attempts),
                cancel_not_found_once = shell_path(&cancel_not_found_once),
                cancel_always_not_found = shell_path(&cancel_always_not_found),
                cancel_failure = shell_path(&cancel_failure),
                cancel_delay = shell_path(&cancel_delay),
                cancel_first_seen = shell_path(&cancel_first_seen),
            );
            fs::write(&program, script)?;
            let mut permissions = fs::metadata(&program)?.permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&program, permissions)?;

            let instance_id = format!(
                "backup-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            );
            let config_dir = root.join("logoscore-config");
            fs::create_dir_all(config_dir.join("client"))?;
            fs::write(
                config_dir.join("client/config.json"),
                serde_json::to_vec(&json!({
                    "instance_id": instance_id,
                    "daemon": { "core_service": { "transport": "local" } }
                }))?,
            )?;
            let socket = std::env::temp_dir().join(format!("logos_core_service_{instance_id}"));
            fs::write(&socket, b"test socket identity")?;
            let runtime = crate::modules::logos_core::LogoscoreCliRuntime::managed(
                program.display().to_string(),
                config_dir.display().to_string(),
            );
            Ok(Self {
                _directory: directory,
                socket,
                runtime,
                trigger,
                canceled,
                cancel_attempts,
                cancel_not_found_once,
                cancel_always_not_found,
                cancel_failure,
                cancel_delay,
                cid_path,
                staging_path,
            })
        }

        fn staged_path(&self) -> Result<PathBuf> {
            Ok(PathBuf::from(fs::read_to_string(&self.staging_path)?))
        }

        fn retry_first_cancel_not_found(&self) -> Result<()> {
            fs::write(&self.cancel_not_found_once, b"retry")?;
            Ok(())
        }

        fn always_return_cancel_not_found(&self, cid: &str) -> Result<()> {
            fs::write(&self.cancel_always_not_found, b"retry")?;
            fs::write(&self.cid_path, cid)?;
            Ok(())
        }

        fn fail_cancel(&self) -> Result<()> {
            fs::write(&self.cancel_failure, b"fail")?;
            Ok(())
        }

        fn delay_cancel(&self, delay_seconds: &str, cid: &str) -> Result<()> {
            fs::write(&self.cancel_delay, delay_seconds)?;
            fs::write(&self.cid_path, cid)?;
            Ok(())
        }

        fn cancel_attempt_count(&self) -> Result<usize> {
            match fs::read(&self.cancel_attempts) {
                Ok(attempts) => Ok(attempts.len()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
                Err(error) => Err(error.into()),
            }
        }
    }

    #[cfg(unix)]
    impl Drop for FakeDownloadRuntime {
        fn drop(&mut self) {
            let _result = fs::remove_file(&self.socket);
        }
    }

    #[cfg(unix)]
    fn shell_path(path: &Path) -> String {
        format!(
            "'{}'",
            path.display().to_string().replace('\'', "'\\\"'\\\"'")
        )
    }

    #[cfg(unix)]
    fn download_module_info(include_event: bool) -> Value {
        json!({
            "name": "storage_module",
            "methods": [
                {
                    "isInvokable": true,
                    "name": "downloadProtocol",
                    "signature": "downloadProtocol()"
                },
                {
                    "isInvokable": true,
                    "name": "downloadToUrlV2",
                    "signature": STORAGE_DOWNLOAD_METHOD_SIGNATURE
                },
                {
                    "isInvokable": true,
                    "name": "downloadCancelV2",
                    "signature": "downloadCancelV2(QString)"
                }
            ],
            "events": if include_event {
                json!([{ "name": "storageDownloadDoneV2", "signature": "storageDownloadDoneV2(QString)" }])
            } else {
                json!([])
            }
        })
    }

    #[cfg(unix)]
    fn download_control(duration: Duration) -> Result<CommandControl> {
        Ok(CommandControl::new(
            CancellationToken::new(),
            Instant::now()
                .checked_add(duration)
                .context("test download deadline overflow")?,
        ))
    }

    #[test]
    fn manifest_poll_correlates_new_upload_by_filename_and_size() -> Result<()> {
        let baseline = HashSet::from(["cid-old".to_owned()]);
        let manifests = json!([
            {"cid":"cid-old","filename":"backup.json","datasetSize":12},
            {"cid":"cid-wrong-size","filename":"backup.json","datasetSize":13},
            {"cid":"cid-new","filename":"backup.json","datasetSize":12}
        ]);

        let cid = new_manifest_cid(&manifests, "backup.json", 12, &baseline);

        anyhow::ensure!(
            cid.as_deref() == Some("cid-new"),
            "manifest correlation drift"
        );
        Ok(())
    }

    #[test]
    fn download_protocol_requires_published_cleanup_deadline() -> Result<()> {
        let valid = json!({
            "protocol": "logos.storage.download",
            "version": 2,
            "moduleOperationIdOwner": "caller",
            "cancelTimeoutMs": 15_000,
            "maxDownloadBytes": 1_073_741_824_u64,
        });
        validate_storage_download_protocol(&valid, 16 * 1024 * 1024)?;

        for (field, value) in [
            ("protocol", json!("logos.storage.download.legacy")),
            ("version", json!(1)),
            ("moduleOperationIdOwner", json!("module")),
            ("cancelTimeoutMs", json!(12_000)),
            ("maxDownloadBytes", json!(16 * 1024 * 1024 - 1)),
        ] {
            let mut incompatible = valid.clone();
            *incompatible
                .get_mut(field)
                .with_context(|| format!("missing protocol fixture field `{field}`"))? = value;
            anyhow::ensure!(
                validate_storage_download_protocol(&incompatible, 16 * 1024 * 1024).is_err(),
                "download protocol accepted incompatible `{field}`"
            );
        }
        Ok(())
    }

    #[test]
    fn backup_download_cancellation_budget_has_strict_settlement_margins() -> Result<()> {
        anyhow::ensure!(
            Duration::from_millis(STORAGE_DOWNLOAD_CANCEL_TIMEOUT_MS)
                < BACKUP_DOWNLOAD_CANCEL_COMMAND_TIMEOUT
                && BACKUP_DOWNLOAD_CANCEL_COMMAND_TIMEOUT
                    < BACKUP_DOWNLOAD_TERMINATION_HANDSHAKE_GRACE,
            "backup download cancellation budget lost its strict ordering"
        );
        Ok(())
    }

    #[test]
    fn backup_cleanup_preserves_unconfirmed_command_stop_reason() -> Result<()> {
        let primary: anyhow::Error = CommandCleanupUnconfirmed::new(
            Some(CommandStopReason::CancelRequested),
            CommandTerminationScope::ProcessGroup,
            "injected command cleanup uncertainty".to_owned(),
        )
        .into();
        let cleanup = BackupDownloadCleanupUnconfirmed::new(
            &primary,
            "injected backup cleanup uncertainty".to_owned(),
        );

        anyhow::ensure!(
            cleanup.stop_reason() == Some(CommandStopReason::CancelRequested),
            "backup cleanup lost unconfirmed command stop reason"
        );
        Ok(())
    }

    #[test]
    fn download_cancel_acknowledgement_preserves_known_effect_identity() -> Result<()> {
        let canceled = json!({
            "protocol": "logos.storage.download",
            "version": 2,
            "moduleOperationId": "operation-1",
            "cid": "cid-1",
            "cancelStatus": "canceled",
        });
        validate_download_cancel_acknowledgement(
            canceled,
            "operation-1",
            "cid-1",
            DownloadCancelExpectation::Accepted,
        )?;
        validate_download_cancel_acknowledgement(
            json!({
                "protocol": "logos.storage.download",
                "version": 2,
                "moduleOperationId": "operation-1",
                "cid": "cid-1",
                "cancelStatus": "already_terminal",
                "terminalOutcome": "succeeded",
            }),
            "operation-1",
            "cid-1",
            DownloadCancelExpectation::Accepted,
        )?;
        anyhow::ensure!(
            validate_download_cancel_acknowledgement(
                json!({
                    "protocol": "logos.storage.download",
                    "version": 2,
                    "moduleOperationId": "operation-1",
                    "cancelStatus": "not_found",
                }),
                "operation-1",
                "cid-1",
                DownloadCancelExpectation::EffectUnknown,
            )? == DownloadCancelSettlement::RetryNotFound,
            "effect-unknown not_found was mistaken for cleanup settlement"
        );

        for invalid in [
            json!({
                "protocol": "logos.storage.download",
                "version": 2,
                "moduleOperationId": "operation-1",
                "cancelStatus": "not_found",
            }),
            json!({
                "protocol": "logos.storage.download",
                "version": 2,
                "moduleOperationId": "operation-1",
                "cid": "foreign-cid",
                "cancelStatus": "canceled",
            }),
            json!({
                "protocol": "logos.storage.download",
                "version": 2,
                "moduleOperationId": "operation-1",
                "cid": "cid-1",
                "cancelStatus": "already_terminal",
            }),
        ] {
            anyhow::ensure!(
                validate_download_cancel_acknowledgement(
                    invalid,
                    "operation-1",
                    "cid-1",
                    DownloadCancelExpectation::Accepted,
                )
                .is_err(),
                "accepted effect lost strict cancellation identity"
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn effect_unknown_cancel_never_settles_on_persistent_not_found() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::new(b"{}", None, &download_module_info(true))?;
        fake.always_return_cancel_not_found("cid-late")?;

        let error = cancel_module_download_controlled(
            &fake.runtime,
            "operation-late",
            "cid-late",
            DownloadCancelExpectation::EffectUnknown,
            download_control(Duration::from_millis(250))?,
        )
        .err()
        .context("persistent not_found should exhaust cleanup control")?;

        anyhow::ensure!(
            error.downcast_ref::<CommandTerminated>().is_some(),
            "persistent not_found lost bounded cleanup termination: {error:#}"
        );
        anyhow::ensure!(
            fake.cancel_attempt_count()? >= 2,
            "persistent not_found was not retried"
        );
        anyhow::ensure!(
            !fake.canceled.exists(),
            "persistent not_found was misreported as confirmed cancellation"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cancel_command_bound_allows_protocol_settlement_margin() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::new(b"{}", None, &download_module_info(true))?;
        let protocol_allowance = Duration::from_millis(80);
        let command_bound = Duration::from_millis(700);
        fake.delay_cancel("0.08", "cid-budget")?;
        let started = Instant::now();

        cancel_module_download_with_timeout(
            &fake.runtime,
            "operation-budget",
            "cid-budget",
            DownloadCancelExpectation::Accepted,
            command_bound,
        )?;

        let elapsed = started.elapsed();
        anyhow::ensure!(
            elapsed >= protocol_allowance && elapsed < command_bound,
            "cancel command did not settle inside staged budget: {elapsed:?}"
        );
        anyhow::ensure!(
            fake.canceled.exists(),
            "delayed cancellation was not settled"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_download_waits_for_ready_correlated_terminal_and_cleans_staging() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let payload = br#"{"kind":"logos-inspector-settings-backup","version":1}"#;
        let fake = FakeDownloadRuntime::new(
            payload,
            Some(SUCCEEDED_TERMINAL),
            &download_module_info(true),
        )?;

        let bytes = module_download_backup_bytes_blocking_controlled(
            &fake.runtime,
            "cid-ready",
            false,
            1024,
            download_control(Duration::from_secs(3))?,
        )?;
        let staged = fake.staged_path()?;

        anyhow::ensure!(bytes == payload, "CLI backup bytes drifted");
        anyhow::ensure!(
            !staged.exists(),
            "CLI download staging survived terminal return: {}",
            staged.display()
        );
        anyhow::ensure!(
            !fake.canceled.exists(),
            "successful CLI download was canceled"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_download_wrong_operation_times_out_cancels_and_cleans() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::new(
            b"{}",
            Some(
                r#"{"protocol":"logos.storage.download","version":2,"moduleOperationId":"foreign-operation","cid":"cid-requested","outcome":"succeeded"}"#,
            ),
            &download_module_info(true),
        )?;
        let result = module_download_backup_bytes_blocking_controlled(
            &fake.runtime,
            "cid-requested",
            false,
            1024,
            download_control(Duration::from_millis(500))?,
        );
        let error = result
            .err()
            .context("wrong-operation CLI download should not complete")?;
        let staged = fake.staged_path()?;

        anyhow::ensure!(
            error
                .downcast_ref::<crate::support::command_runner::CommandTerminated>()
                .is_some(),
            "wrong-operation timeout lost typed interruption: {error:#}"
        );
        anyhow::ensure!(
            fake.canceled.exists(),
            "timeout did not call downloadCancel"
        );
        anyhow::ensure!(!staged.exists(), "timeout left staging file behind");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_download_cancellation_uses_fresh_cleanup_control() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::new(b"{}", None, &download_module_info(true))?;
        let cancellation = CancellationToken::new();
        let cancel_request = cancellation.clone();
        let trigger = fake.trigger.clone();
        let canceler = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            while !trigger.exists() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            let dispatched = trigger.exists();
            if dispatched {
                cancel_request.cancel();
            }
            dispatched
        });
        let control = CommandControl::new(cancellation, Instant::now() + Duration::from_secs(10));

        let result = module_download_backup_bytes_blocking_controlled(
            &fake.runtime,
            "cid-cancel",
            true,
            1024,
            control,
        );
        let dispatched = canceler
            .join()
            .map_err(|_| anyhow::anyhow!("CLI backup canceler panicked"))?;
        anyhow::ensure!(dispatched, "CLI backup download was not dispatched");
        let error = result
            .err()
            .context("canceled CLI download should not complete")?;
        let staged = fake.staged_path()?;

        anyhow::ensure!(
            error
                .downcast_ref::<crate::support::command_runner::CommandTerminated>()
                .is_some(),
            "cancellation lost typed interruption: {error:#}"
        );
        anyhow::ensure!(
            fake.canceled.exists(),
            "cancellation did not run bounded remote cleanup"
        );
        anyhow::ensure!(!staged.exists(), "cancellation left staging file behind");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_download_cancels_effect_when_dispatch_reply_is_interrupted() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::with_call_behavior(
            b"{}",
            None,
            &download_module_info(true),
            None,
            true,
        )?;
        fake.retry_first_cancel_not_found()?;
        let cancellation = CancellationToken::new();
        let cancel_request = cancellation.clone();
        let trigger = fake.trigger.clone();
        let canceler = thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(8);
            while !trigger.exists() && Instant::now() < deadline {
                thread::sleep(Duration::from_millis(5));
            }
            let dispatched = trigger.exists();
            if dispatched {
                cancel_request.cancel();
            }
            dispatched
        });
        let control = CommandControl::new(cancellation, Instant::now() + Duration::from_secs(10));

        let error = module_download_backup_bytes_blocking_controlled(
            &fake.runtime,
            "cid-call-interrupted",
            false,
            1024,
            control,
        )
        .err()
        .context("interrupted dispatch reply should fail")?;
        let dispatched = canceler
            .join()
            .map_err(|_| anyhow::anyhow!("dispatch-reply canceler panicked"))?;
        anyhow::ensure!(dispatched, "CLI backup call was not dispatched");
        let staged = fake.staged_path()?;

        anyhow::ensure!(
            error
                .downcast_ref::<crate::support::command_runner::CommandTerminated>()
                .is_some(),
            "dispatch interruption lost typed termination: {error:#}"
        );
        anyhow::ensure!(
            fake.canceled.exists(),
            "dispatch interruption did not cancel accepted remote effect"
        );
        anyhow::ensure!(
            fake.cancel_attempt_count()? >= 2,
            "effect-unknown cancellation did not retry transient not_found"
        );
        anyhow::ensure!(
            !staged.exists(),
            "dispatch interruption left staging behind"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_pre_spawn_dispatch_interruption_skips_remote_cancel_and_cleans() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::new(b"{}", None, &download_module_info(true))?;
        let cancellation = CancellationToken::new();
        let cancel_request = cancellation.clone();
        let staging_path = fake.staging_path.clone();
        let control = CommandControl::new(cancellation, Instant::now() + Duration::from_secs(3));

        let error = module_download_backup_bytes_blocking_controlled_with_ready_hook(
            &fake.runtime,
            "cid-no-process",
            false,
            1024,
            control,
            move |path| {
                fs::write(&staging_path, path.display().to_string())?;
                cancel_request.cancel();
                Ok(())
            },
        )
        .err()
        .context("pre-spawn dispatch interruption should fail")?;
        let terminated = error
            .downcast_ref::<CommandTerminated>()
            .context("pre-spawn interruption lost typed termination evidence")?;
        let staged = fake.staged_path()?;

        anyhow::ensure!(
            terminated.scope() == CommandTerminationScope::NoProcess,
            "pre-spawn interruption reported wrong scope: {terminated}"
        );
        anyhow::ensure!(
            fake.cancel_attempt_count()? == 0 && !fake.canceled.exists(),
            "no-process interruption invoked remote cancellation"
        );
        anyhow::ensure!(
            !fake.trigger.exists(),
            "no-process interruption dispatched a remote download"
        );
        anyhow::ensure!(
            !staged.exists(),
            "no-process interruption left shared staging behind: {}",
            staged.display()
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_download_cancels_malformed_dispatch_acknowledgement() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::with_call_behavior(
            b"{}",
            None,
            &download_module_info(true),
            Some(r#"{"status":"ok","result":{"success":true,"value":null,"error":null}}"#),
            false,
        )?;

        let error = module_download_backup_bytes_blocking_controlled(
            &fake.runtime,
            "cid-null-ack",
            false,
            1024,
            download_control(Duration::from_secs(3))?,
        )
        .err()
        .context("null download acknowledgement should fail")?;
        let staged = fake.staged_path()?;
        anyhow::ensure!(
            error.to_string().contains("invalid acknowledgement"),
            "unexpected null acknowledgement error: {error:#}"
        );
        anyhow::ensure!(
            fake.canceled.exists(),
            "null acknowledgement was not canceled"
        );
        anyhow::ensure!(!staged.exists(), "null acknowledgement left staging behind");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn malformed_dispatch_with_failed_cancel_is_cleanup_unknown() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let fake = FakeDownloadRuntime::with_call_behavior(
            b"{}",
            None,
            &download_module_info(true),
            Some(r#"{"status":"ok","result":{"success":true,"value":null,"error":null}}"#),
            false,
        )?;
        fake.fail_cancel()?;

        let error = module_download_backup_bytes_blocking_controlled(
            &fake.runtime,
            "cid-cleanup-unknown",
            false,
            1024,
            download_control(Duration::from_secs(3))?,
        )
        .err()
        .context("failed cancellation must not look terminal")?;
        let cleanup = error
            .downcast_ref::<BackupDownloadCleanupUnconfirmed>()
            .context("failed cancellation lost cleanup-unknown type")?;
        anyhow::ensure!(
            cleanup.stop_reason().is_none(),
            "protocol failure invented user stop evidence"
        );
        anyhow::ensure!(
            fake.cancel_attempt_count()? == 1 && !fake.canceled.exists(),
            "failed cancellation was misreported as settled"
        );
        anyhow::ensure!(
            !fake.staged_path()?.exists(),
            "cleanup-unknown transport left removable staging behind"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_download_rejects_malformed_and_failed_terminals() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        for (label, terminal, expected, canceled) in [
            (
                "malformed",
                r#"{not-json"#,
                "terminal payload is invalid JSON",
                true,
            ),
            ("failed", FAILED_TERMINAL, "not found", false),
        ] {
            let cid = format!("cid-{label}");
            let fake =
                FakeDownloadRuntime::new(b"{}", Some(terminal), &download_module_info(true))?;
            let error = module_download_backup_bytes_blocking_controlled(
                &fake.runtime,
                &cid,
                false,
                1024,
                download_control(Duration::from_secs(3))?,
            )
            .err()
            .with_context(|| format!("{label} terminal should fail"))?;
            let staged = fake.staged_path()?;

            anyhow::ensure!(
                error.to_string().contains(expected),
                "unexpected {label} terminal error: {error:#}"
            );
            anyhow::ensure!(
                fake.canceled.exists() == canceled,
                "{label} terminal cleanup mismatch"
            );
            anyhow::ensure!(!staged.exists(), "{label} terminal left staging behind");
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn cli_backup_download_gates_runtime_event_and_bounds_staged_bytes() -> Result<()> {
        let _test_permit = serialize_cli_backup_test()?;
        let unsupported = FakeDownloadRuntime::new(b"{}", None, &download_module_info(false))?;
        let unsupported_error = module_download_backup_bytes_blocking_controlled(
            &unsupported.runtime,
            "cid-unsupported",
            false,
            1024,
            download_control(Duration::from_secs(3))?,
        )
        .err()
        .context("missing terminal event should fail preflight")?;
        anyhow::ensure!(
            unsupported_error
                .to_string()
                .contains("does not expose event `storageDownloadDoneV2(QString)`"),
            "unexpected runtime gate error: {unsupported_error:#}"
        );
        anyhow::ensure!(
            !unsupported.trigger.exists(),
            "unsupported runtime started a download"
        );

        let oversized = FakeDownloadRuntime::new(b"123456789", None, &download_module_info(true))?;
        let started = std::time::Instant::now();
        let oversized_error = module_download_backup_bytes_blocking_controlled(
            &oversized.runtime,
            "cid-oversized",
            false,
            8,
            download_control(Duration::from_secs(3))?,
        )
        .err()
        .context("oversized staged download should fail")?;
        let staged = oversized.staged_path()?;
        anyhow::ensure!(
            oversized_error
                .to_string()
                .contains("exceeded 8 byte limit before terminal completion"),
            "unexpected staged-byte limit error: {oversized_error:#}"
        );
        anyhow::ensure!(
            started.elapsed() < Duration::from_secs(2),
            "oversized download waited for absent terminal instead of canceling early"
        );
        anyhow::ensure!(
            oversized.canceled.exists(),
            "oversized in-flight download was not canceled"
        );
        anyhow::ensure!(!staged.exists(), "oversized download left staging behind");
        Ok(())
    }

    #[tokio::test]
    async fn backup_download_rejects_declared_body_over_limit() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            read_request_headers(&mut stream)?;
            stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n123456789",
            )?;
            Ok(())
        });

        let error = download_bytes(&endpoint, "cid-large", false, 8)
            .await
            .err()
            .context("oversized declared body should fail")?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("declared-body server panicked"))??;

        anyhow::ensure!(
            error
                .to_string()
                .contains("http response body exceeded 8 byte limit"),
            "unexpected declared-body limit error: {error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn backup_download_rejects_chunked_body_over_limit() -> Result<()> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let server = thread::spawn(move || -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            read_request_headers(&mut stream)?;
            stream.write_all(
                b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n5\r\n12345\r\n4\r\n6789\r\n0\r\n\r\n",
            )?;
            Ok(())
        });

        let error = download_bytes(&endpoint, "cid-large", true, 8)
            .await
            .err()
            .context("oversized chunked body should fail")?;
        server
            .join()
            .map_err(|_| anyhow::anyhow!("chunked-body server panicked"))??;

        anyhow::ensure!(
            error
                .to_string()
                .contains("http response body exceeded 8 byte limit"),
            "unexpected chunked-body limit error: {error:#}"
        );
        Ok(())
    }

    fn read_request_headers(stream: &mut std::net::TcpStream) -> Result<()> {
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        let mut request = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let bytes = stream.read(&mut buffer)?;
            if bytes == 0 {
                anyhow::bail!("HTTP request headers were incomplete");
            }
            request.extend_from_slice(
                buffer
                    .get(..bytes)
                    .context("HTTP request header chunk was invalid")?,
            );
            if request.windows(4).any(|window| window == b"\r\n\r\n") {
                return Ok(());
            }
        }
    }
}
