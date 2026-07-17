use std::{
    collections::HashSet,
    fmt, fs,
    io::Read as _,
    path::{Path, PathBuf},
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
    modules::logos_core::{
        BoxedModuleEventSubscription, ModuleCallTerminationEvidence, ModuleTransportEvent,
        ModuleTransportKind, SharedModuleTransport,
    },
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
const STORAGE_MODULE_METHODS_METHOD: &str = "getPluginMethods";
const STORAGE_MODULE_EVENTS_METHOD: &str = "getPluginEvents";
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

#[cfg(all(test, unix))]
static TEST_CLI_BACKUP_LOCK: Mutex<()> = Mutex::new(());

#[cfg(all(test, unix))]
pub(crate) fn serialize_cli_backup_test() -> MutexGuard<'static, ()> {
    match TEST_CLI_BACKUP_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

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

struct HostSharedDownload {
    directory: tempfile::TempDir,
    path: PathBuf,
}

impl HostSharedDownload {
    fn new(filename: &str) -> Result<Self> {
        let safe_filename = Path::new(filename)
            .file_name()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty() && *value == filename)
            .context("host download filename is invalid")?;
        let directory = tempfile::Builder::new()
            .prefix("logos-inspector-host-download-")
            .tempdir()
            .context("failed to create host download workspace")?;
        let path = directory.path().join(safe_filename);
        fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(&path)
            .context("failed to create host download staging file")?;
        Ok(Self { directory, path })
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn read_path_bounded(path: &Path, max_bytes: usize) -> Result<Vec<u8>> {
        let metadata = fs::symlink_metadata(path).with_context(|| {
            format!(
                "failed to inspect host download staging file `{}`",
                path.display()
            )
        })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() {
            bail!("host download staging path is not a regular file");
        }
        let max_bytes_u64 =
            u64::try_from(max_bytes).context("storage download byte limit is too large")?;
        if metadata.len() > max_bytes_u64 {
            bail!("host download exceeded {max_bytes} byte limit");
        }
        let capacity = usize::try_from(metadata.len())
            .context("host download length does not fit in memory")?;
        let mut bytes = Vec::with_capacity(capacity);
        fs::File::open(path)
            .with_context(|| {
                format!(
                    "failed to open host download staging file `{}`",
                    path.display()
                )
            })?
            .take(max_bytes_u64.saturating_add(1))
            .read_to_end(&mut bytes)
            .with_context(|| {
                format!(
                    "failed to read host download staging file `{}`",
                    path.display()
                )
            })?;
        if bytes.len() > max_bytes {
            bail!("host download exceeded {max_bytes} byte limit");
        }
        Ok(bytes)
    }

    fn close(self) -> Result<()> {
        let path = self.directory.path().to_path_buf();
        self.directory.close().with_context(|| {
            format!(
                "failed to remove host download workspace `{}`",
                path.display()
            )
        })
    }
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

pub(super) async fn host_module_download_backup_bytes_controlled(
    transport: &SharedModuleTransport,
    cleanup_transport: &SharedModuleTransport,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
    control: CommandControl,
) -> Result<Vec<u8>> {
    anyhow::ensure!(
        transport.kind() == ModuleTransportKind::Module
            && cleanup_transport.kind() == ModuleTransportKind::Module,
        "Basecamp backup download requires the host module transport"
    );
    anyhow::ensure!(
        max_bytes > 0,
        "storage download byte limit must be positive"
    );
    anyhow::ensure!(
        transport.supports_shared_file_staging(),
        "Basecamp host transport does not provide shared file staging"
    );
    anyhow::ensure!(
        transport.native_runtime_module_events_ready(),
        "Basecamp host transport does not own healthy native runtime module-event ingress"
    );
    control.check_active()?;

    let methods = module_call(
        transport,
        ModuleTransportKind::Module,
        STORAGE_MODULE_METHODS_METHOD,
        Vec::new(),
    )
    .await
    .context("failed to inspect Basecamp Storage module methods")?;
    let events = module_call(
        transport,
        ModuleTransportKind::Module,
        STORAGE_MODULE_EVENTS_METHOD,
        Vec::new(),
    )
    .await
    .context("failed to inspect Basecamp Storage module events")?;
    validate_storage_download_interface(&methods, &events)?;

    let protocol = module_call(
        transport,
        ModuleTransportKind::Module,
        STORAGE_DOWNLOAD_PROTOCOL_METHOD,
        Vec::new(),
    )
    .await
    .context("failed to read Basecamp Storage download protocol")?;
    validate_storage_download_protocol(&protocol, max_bytes)?;
    control.check_active()?;

    let staged = HostSharedDownload::new(BACKUP_DOWNLOAD_FILENAME)?;
    let result = execute_staged_host_backup_download(
        transport,
        cleanup_transport,
        cid,
        local_only,
        max_bytes,
        control,
        &staged,
    )
    .await;
    let cleanup = staged.close();
    combine_host_download_cleanup(result, cleanup)
}

fn validate_storage_download_interface(methods: &Value, events: &Value) -> Result<()> {
    let methods = methods
        .as_array()
        .context("Storage module method metadata is not an array")?;
    let events = events
        .as_array()
        .context("Storage module event metadata is not an array")?;
    for (name, signature) in [
        (STORAGE_DOWNLOAD_PROTOCOL_METHOD, "downloadProtocol()"),
        (STORAGE_DOWNLOAD_METHOD, STORAGE_DOWNLOAD_METHOD_SIGNATURE),
        (STORAGE_DOWNLOAD_CANCEL_METHOD, "downloadCancelV2(QString)"),
    ] {
        anyhow::ensure!(
            methods.iter().any(|method| {
                method.get("name").and_then(Value::as_str) == Some(name)
                    && method.get("signature").and_then(Value::as_str) == Some(signature)
                    && method.get("isInvokable").and_then(Value::as_bool) == Some(true)
                    && method
                        .get("type")
                        .and_then(Value::as_str)
                        .is_none_or(|kind| kind == "method")
            }),
            "Storage module does not expose exact method `{signature}`"
        );
    }
    anyhow::ensure!(
        events.iter().any(|event| {
            event.get("name").and_then(Value::as_str) == Some(STORAGE_DOWNLOAD_DONE_EVENT)
                && event.get("signature").and_then(Value::as_str)
                    == Some("storageDownloadDoneV2(QString)")
                && event
                    .get("type")
                    .and_then(Value::as_str)
                    .is_none_or(|kind| kind == "event")
        }),
        "Storage module does not expose exact event `storageDownloadDoneV2(QString)`"
    );
    Ok(())
}

async fn execute_staged_host_backup_download(
    transport: &SharedModuleTransport,
    cleanup_transport: &SharedModuleTransport,
    cid: &str,
    local_only: bool,
    max_bytes: usize,
    control: CommandControl,
    staged: &HostSharedDownload,
) -> Result<Vec<u8>> {
    let operation_id = new_storage_download_operation_id()?;
    let subscription = transport
        .subscribe_module_event(super::layer::module_id(), STORAGE_DOWNLOAD_DONE_EVENT)
        .context("Basecamp host transport cannot observe Storage download completion")?;
    let path = staged
        .path()
        .to_str()
        .context("host download staging path is not UTF-8")?
        .to_owned();
    let max_bytes_arg =
        i64::try_from(max_bytes).context("storage download byte limit is too large")?;
    let acknowledgement = match module_call(
        transport,
        ModuleTransportKind::Module,
        STORAGE_DOWNLOAD_METHOD,
        vec![
            json!(cid),
            json!(path),
            json!(local_only),
            json!(BACKUP_DOWNLOAD_CHUNK_SIZE),
            json!(&operation_id),
            json!(max_bytes_arg),
        ],
    )
    .await
    {
        Ok(acknowledgement) => acknowledgement,
        Err(error)
            if error
                .downcast_ref::<crate::modules::logos_core::ModuleCallTerminated>()
                .is_some_and(|terminated| {
                    terminated.evidence() == ModuleCallTerminationEvidence::NotStarted
                }) =>
        {
            return Err(error);
        }
        Err(error) => {
            drop(subscription);
            return cleanup_active_host_download_error(
                cleanup_transport,
                &operation_id,
                cid,
                DownloadCancelExpectation::EffectUnknown,
                error,
            )
            .await;
        }
    };
    if let Err(error) =
        decode_download_dispatch_acknowledgement(&acknowledgement, &operation_id, cid)
    {
        drop(subscription);
        return cleanup_active_host_download_error(
            cleanup_transport,
            &operation_id,
            cid,
            DownloadCancelExpectation::EffectUnknown,
            error,
        )
        .await;
    }

    let worker_guard = match control.blocking_worker_guard() {
        Ok(worker_guard) => worker_guard,
        Err(error) => {
            return cleanup_active_host_download_error(
                cleanup_transport,
                &operation_id,
                cid,
                DownloadCancelExpectation::Accepted,
                error,
            )
            .await;
        }
    };
    let staged_path = staged.path().to_path_buf();
    let wait_operation_id = operation_id.clone();
    let wait_cid = cid.to_owned();
    let wait_control = control.clone();
    let terminal = blocking_module_call("Basecamp Storage backup terminal wait", move || {
        let _worker_guard = worker_guard;
        wait_for_host_download_terminal(
            subscription,
            &wait_operation_id,
            &wait_cid,
            &staged_path,
            max_bytes,
            &wait_control,
        )
    })
    .await;
    let terminal = match terminal {
        Ok(terminal) => terminal,
        Err(error) => {
            return cleanup_active_host_download_error(
                cleanup_transport,
                &operation_id,
                cid,
                DownloadCancelExpectation::Accepted,
                error,
            )
            .await;
        }
    };
    match terminal {
        DownloadTerminalEvent::Succeeded => {
            let read_worker_guard = control.blocking_worker_guard()?;
            let staged_path = staged.path().to_path_buf();
            blocking_module_call("Basecamp Storage backup staged read", move || {
                let _worker_guard = read_worker_guard;
                HostSharedDownload::read_path_bounded(&staged_path, max_bytes)
            })
            .await
        }
        DownloadTerminalEvent::Canceled => {
            bail!("storage_module download was canceled for CID `{cid}`")
        }
        DownloadTerminalEvent::Failed(error) => {
            bail!("storage_module download failed for CID `{cid}`: {error}")
        }
        DownloadTerminalEvent::Unrelated => {
            bail!("storage download terminal wait returned an unrelated event")
        }
    }
}

fn wait_for_host_download_terminal(
    mut subscription: BoxedModuleEventSubscription,
    operation_id: &str,
    cid: &str,
    staged_path: &Path,
    max_bytes: usize,
    control: &CommandControl,
) -> Result<DownloadTerminalEvent> {
    let mut unrelated = 0_usize;
    loop {
        control.check_active()?;
        ensure_staged_download_within_limit(staged_path, max_bytes)?;
        let Some(event) = subscription.next_within(BACKUP_DOWNLOAD_SIZE_POLL_INTERVAL)? else {
            continue;
        };
        match decode_host_download_terminal_event(&event, operation_id, cid)? {
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

async fn cleanup_active_host_download_error<T>(
    cleanup_transport: &SharedModuleTransport,
    operation_id: &str,
    cid: &str,
    expectation: DownloadCancelExpectation,
    primary: anyhow::Error,
) -> Result<T> {
    match cancel_host_module_download(cleanup_transport, operation_id, cid, expectation).await {
        Ok(()) => {
            if let Some(terminated) =
                primary.downcast_ref::<crate::modules::logos_core::ModuleCallTerminated>()
            {
                return Err(crate::modules::logos_core::ModuleCallTerminated::new(
                    terminated.reason(),
                    ModuleCallTerminationEvidence::RemoteEffectTerminationConfirmed,
                )
                .into());
            }
            Err(primary)
        }
        Err(cleanup) => Err(BackupDownloadCleanupUnconfirmed::new(
            &primary,
            format!("{primary}; Basecamp storage download cleanup was not confirmed: {cleanup:#}"),
        )
        .into()),
    }
}

async fn cancel_host_module_download(
    transport: &SharedModuleTransport,
    operation_id: &str,
    cid: &str,
    expectation: DownloadCancelExpectation,
) -> Result<()> {
    let deadline = Instant::now()
        .checked_add(BACKUP_DOWNLOAD_CANCEL_COMMAND_TIMEOUT)
        .context("Basecamp storage download cleanup deadline overflow")?;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        anyhow::ensure!(
            !remaining.is_zero(),
            "Basecamp storage download cancellation timed out"
        );
        let call = module_call(
            transport,
            ModuleTransportKind::Module,
            STORAGE_DOWNLOAD_CANCEL_METHOD,
            vec![json!(operation_id)],
        );
        let acknowledgement = tokio::time::timeout(remaining, call)
            .await
            .context("Basecamp storage download cancellation timed out")??;
        match validate_download_cancel_acknowledgement(
            acknowledgement,
            operation_id,
            cid,
            expectation,
        )? {
            DownloadCancelSettlement::Settled => return Ok(()),
            DownloadCancelSettlement::RetryNotFound => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                anyhow::ensure!(
                    !remaining.is_zero(),
                    "Basecamp storage download cancellation timed out"
                );
                tokio::time::sleep(BACKUP_DOWNLOAD_CANCEL_RETRY_INTERVAL.min(remaining)).await;
            }
        }
    }
}

fn combine_host_download_cleanup(result: Result<Vec<u8>>, cleanup: Result<()>) -> Result<Vec<u8>> {
    match (result, cleanup) {
        (Ok(bytes), Ok(())) => Ok(bytes),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(cleanup)) => {
            let primary = cleanup.context("host download staging cleanup failed");
            let message = primary.to_string();
            Err(BackupDownloadCleanupUnconfirmed::new(&primary, message).into())
        }
        (Err(error), Err(cleanup)) => {
            if let Some(unconfirmed) = error.downcast_ref::<BackupDownloadCleanupUnconfirmed>() {
                return Err(unconfirmed
                    .append(format_args!(
                        "host download staging cleanup failed: {cleanup:#}"
                    ))
                    .into());
            }
            let message = format!("{error}; host download staging cleanup failed: {cleanup:#}");
            Err(BackupDownloadCleanupUnconfirmed::new(&error, message).into())
        }
    }
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
                control,
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
            control,
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
                control,
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
        "storage download exceeded {max_bytes} byte limit before terminal completion"
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
    decode_download_terminal_payload(payload_text, expected_operation_id, expected_cid)
}

fn decode_host_download_terminal_event(
    event: &ModuleTransportEvent,
    expected_operation_id: &str,
    expected_cid: &str,
) -> Result<DownloadTerminalEvent> {
    anyhow::ensure!(
        event.module() == super::layer::module_id(),
        "host download subscription returned an event for the wrong module"
    );
    anyhow::ensure!(
        event.event() == STORAGE_DOWNLOAD_DONE_EVENT,
        "host download subscription returned the wrong event type"
    );
    let [payload] = event.args() else {
        bail!("host download terminal event must contain exactly one payload argument");
    };
    let payload_text = payload
        .as_str()
        .context("host download terminal payload must be a JSON string")?;
    decode_download_terminal_payload(payload_text, expected_operation_id, expected_cid)
}

fn decode_download_terminal_payload(
    payload_text: &str,
    expected_operation_id: &str,
    expected_cid: &str,
) -> Result<DownloadTerminalEvent> {
    let payload: DownloadDonePayload = serde_json::from_str(payload_text)
        .context("storage download terminal payload is invalid JSON")?;
    anyhow::ensure!(
        payload.protocol == STORAGE_DOWNLOAD_PROTOCOL
            && payload.version == STORAGE_DOWNLOAD_PROTOCOL_VERSION,
        "storage download terminal payload has an incompatible protocol"
    );
    let operation_id = payload.operation_id.trim();
    let cid = payload.cid.trim();
    if operation_id.is_empty() {
        bail!("storage download terminal payload has no operation ID");
    }
    if cid.is_empty() {
        bail!("storage download terminal payload has no CID");
    }
    if operation_id != expected_operation_id {
        return Ok(DownloadTerminalEvent::Unrelated);
    }
    anyhow::ensure!(
        cid == expected_cid,
        "storage download terminal payload returned the wrong CID for its operation ID"
    );
    anyhow::ensure!(
        operation_id != cid,
        "storage download terminal operation ID must differ from its CID"
    );
    let error = payload.error.as_deref().map(str::trim).unwrap_or_default();
    match payload.outcome {
        DownloadTerminalOutcome::Succeeded => {
            if !error.is_empty() {
                bail!("successful storage download terminal payload contains an error");
            }
            Ok(DownloadTerminalEvent::Succeeded)
        }
        DownloadTerminalOutcome::Failed => {
            if error.is_empty() {
                bail!("failed storage download terminal payload contains no error");
            }
            Ok(DownloadTerminalEvent::Failed(error.to_owned()))
        }
        DownloadTerminalOutcome::Canceled => {
            if !error.is_empty() {
                bail!("canceled storage download terminal payload contains an error");
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
    parent_control: &CommandControl,
) -> Result<T> {
    let cancel = cancel_module_download(runtime, operation_id, cid, expectation, parent_control);
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
    parent_control: &CommandControl,
) -> Result<()> {
    let control =
        backup_download_cleanup_control(parent_control, BACKUP_DOWNLOAD_CANCEL_COMMAND_TIMEOUT)?;
    cancel_module_download_controlled(runtime, operation_id, expected_cid, expectation, control)
}

fn backup_download_cleanup_control(
    parent_control: &CommandControl,
    timeout: Duration,
) -> Result<CommandControl> {
    let deadline = Instant::now()
        .checked_add(timeout)
        .context("storage download cleanup deadline overflow")?;
    let control = CommandControl::new(CancellationToken::new(), deadline);
    Ok(if let Some(budget) = parent_control.command_budget() {
        control.with_command_budget(budget)
    } else {
        control
    })
}

#[cfg(test)]
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

pub(super) fn logoscore_cli_call_value_controlled_with_runtime(
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
        sync::{
            Arc, Mutex,
            atomic::{AtomicU8, AtomicUsize, Ordering},
            mpsc,
        },
        thread,
        time::{Duration, SystemTime, UNIX_EPOCH},
    };

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum HostDownloadBehavior {
        Succeed,
        WrongOperationThenSucceed,
        WrongCid,
        MalformedTerminal,
        DuplicateSuccess,
        PendingAcknowledgement,
        NoTerminal,
        CloseBeforeTerminal,
        Oversized,
        WrongInterface,
    }

    struct FakeHostSubscription {
        receiver: mpsc::Receiver<ModuleTransportEvent>,
    }

    impl crate::modules::logos_core::ModuleEventSubscription for FakeHostSubscription {
        fn next_within(
            &mut self,
            timeout: Duration,
        ) -> crate::modules::logos_core::ModuleTransportResult<Option<ModuleTransportEvent>>
        {
            match self.receiver.recv_timeout(timeout) {
                Ok(event) => Ok(Some(event)),
                Err(mpsc::RecvTimeoutError::Timeout) => Ok(None),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    Err(crate::modules::logos_core::ModuleTransportClosed::new(
                        "fake Basecamp host transport closed",
                    )
                    .into())
                }
            }
        }
    }

    struct FakeHostDownloadTransport {
        behavior: HostDownloadBehavior,
        payload: Vec<u8>,
        subscriptions: Mutex<Vec<mpsc::SyncSender<ModuleTransportEvent>>>,
        download_calls: AtomicUsize,
        cancel_calls: AtomicUsize,
        staged_paths: Mutex<Vec<PathBuf>>,
    }

    struct ControlledHostDownloadTransport {
        transport: Arc<FakeHostDownloadTransport>,
        control: crate::modules::logos_core::ModuleCallControl,
    }

    impl crate::modules::logos_core::ModuleTransport for ControlledHostDownloadTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(
            &self,
            call: crate::modules::logos_core::ModuleCall,
        ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
            self.transport.call_controlled(call, self.control.clone())
        }

        fn subscribe_module_event(
            &self,
            module: &str,
            event: &str,
        ) -> crate::modules::logos_core::ModuleTransportResult<BoxedModuleEventSubscription>
        {
            self.transport.subscribe_module_event(module, event)
        }

        fn supports_shared_file_staging(&self) -> bool {
            true
        }

        fn native_runtime_module_events_ready(&self) -> bool {
            true
        }
    }

    impl FakeHostDownloadTransport {
        fn new(behavior: HostDownloadBehavior, payload: &[u8]) -> Self {
            Self {
                behavior,
                payload: payload.to_vec(),
                subscriptions: Mutex::new(Vec::new()),
                download_calls: AtomicUsize::new(0),
                cancel_calls: AtomicUsize::new(0),
                staged_paths: Mutex::new(Vec::new()),
            }
        }

        fn emit(&self, payload: Value) -> Result<()> {
            let event = ModuleTransportEvent::new(
                super::super::layer::module_id(),
                STORAGE_DOWNLOAD_DONE_EVENT,
                vec![Value::String(payload.to_string())],
            )?;
            let subscriptions = self
                .subscriptions
                .lock()
                .map_err(|error| anyhow::anyhow!("fake subscription lock failed: {error}"))?;
            for subscription in subscriptions.iter() {
                subscription
                    .try_send(event.clone())
                    .map_err(|error| anyhow::anyhow!("fake event delivery failed: {error}"))?;
            }
            Ok(())
        }

        fn clear_subscriptions(&self) -> Result<()> {
            self.subscriptions
                .lock()
                .map_err(|error| anyhow::anyhow!("fake subscription lock failed: {error}"))?
                .clear();
            Ok(())
        }

        fn exact_methods(&self) -> Value {
            let download_signature = if self.behavior == HostDownloadBehavior::WrongInterface {
                "downloadToUrlV2(QString)"
            } else {
                STORAGE_DOWNLOAD_METHOD_SIGNATURE
            };
            json!([
                {
                    "type": "method",
                    "isInvokable": true,
                    "name": STORAGE_DOWNLOAD_PROTOCOL_METHOD,
                    "signature": "downloadProtocol()"
                },
                {
                    "type": "method",
                    "isInvokable": true,
                    "name": STORAGE_DOWNLOAD_METHOD,
                    "signature": download_signature
                },
                {
                    "type": "method",
                    "isInvokable": true,
                    "name": STORAGE_DOWNLOAD_CANCEL_METHOD,
                    "signature": "downloadCancelV2(QString)"
                }
            ])
        }

        fn download_call(&self, call: &crate::modules::logos_core::ModuleCall) -> Result<Value> {
            self.download_calls.fetch_add(1, Ordering::AcqRel);
            let cid = call
                .args()
                .first()
                .and_then(Value::as_str)
                .context("fake host download CID missing")?;
            let path = call
                .args()
                .get(1)
                .and_then(Value::as_str)
                .map(PathBuf::from)
                .context("fake host download path missing")?;
            let operation_id = call
                .args()
                .get(4)
                .and_then(Value::as_str)
                .context("fake host download operation ID missing")?;
            self.staged_paths
                .lock()
                .map_err(|error| anyhow::anyhow!("fake staged path lock failed: {error}"))?
                .push(path.clone());
            let bytes = if self.behavior == HostDownloadBehavior::Oversized {
                vec![b'x'; self.payload.len().saturating_add(1)]
            } else {
                self.payload.clone()
            };
            fs::write(path, bytes)?;

            let terminal = |operation_id: &str, cid: &str| {
                json!({
                    "protocol": STORAGE_DOWNLOAD_PROTOCOL,
                    "version": STORAGE_DOWNLOAD_PROTOCOL_VERSION,
                    "moduleOperationId": operation_id,
                    "cid": cid,
                    "outcome": "succeeded"
                })
            };
            match self.behavior {
                HostDownloadBehavior::Succeed => self.emit(terminal(operation_id, cid))?,
                HostDownloadBehavior::WrongOperationThenSucceed => {
                    self.emit(terminal("foreign-operation", cid))?;
                    self.emit(terminal(operation_id, cid))?;
                }
                HostDownloadBehavior::WrongCid => {
                    self.emit(terminal(operation_id, "wrong-cid"))?;
                }
                HostDownloadBehavior::MalformedTerminal => {
                    self.emit(json!({ "protocol": STORAGE_DOWNLOAD_PROTOCOL }))?;
                }
                HostDownloadBehavior::DuplicateSuccess => {
                    self.emit(terminal(operation_id, cid))?;
                    self.emit(terminal(operation_id, cid))?;
                }
                HostDownloadBehavior::CloseBeforeTerminal => self.clear_subscriptions()?,
                HostDownloadBehavior::NoTerminal
                | HostDownloadBehavior::PendingAcknowledgement
                | HostDownloadBehavior::Oversized
                | HostDownloadBehavior::WrongInterface => {}
            }
            Ok(json!({
                "protocol": STORAGE_DOWNLOAD_PROTOCOL,
                "version": STORAGE_DOWNLOAD_PROTOCOL_VERSION,
                "accepted": true,
                "moduleOperationId": operation_id,
                "cid": cid,
            }))
        }
    }

    impl crate::modules::logos_core::ModuleTransport for FakeHostDownloadTransport {
        fn kind(&self) -> ModuleTransportKind {
            ModuleTransportKind::Module
        }

        fn call(
            &self,
            call: crate::modules::logos_core::ModuleCall,
        ) -> crate::modules::logos_core::ModuleCallFuture<'_> {
            let acknowledgement_pending = self.behavior
                == HostDownloadBehavior::PendingAcknowledgement
                && call.method() == STORAGE_DOWNLOAD_METHOD;
            let result = match call.method() {
                STORAGE_MODULE_METHODS_METHOD => Ok(self.exact_methods()),
                STORAGE_MODULE_EVENTS_METHOD => Ok(json!([{
                    "type": "event",
                    "name": STORAGE_DOWNLOAD_DONE_EVENT,
                    "signature": "storageDownloadDoneV2(QString)"
                }])),
                STORAGE_DOWNLOAD_PROTOCOL_METHOD => Ok(json!({
                    "protocol": STORAGE_DOWNLOAD_PROTOCOL,
                    "version": STORAGE_DOWNLOAD_PROTOCOL_VERSION,
                    "moduleOperationIdOwner": "caller",
                    "cancelTimeoutMs": STORAGE_DOWNLOAD_CANCEL_TIMEOUT_MS,
                    "maxDownloadBytes": 1_073_741_824_u64,
                })),
                STORAGE_DOWNLOAD_METHOD => self.download_call(&call),
                STORAGE_DOWNLOAD_CANCEL_METHOD => {
                    self.cancel_calls.fetch_add(1, Ordering::AcqRel);
                    let operation_id = call
                        .args()
                        .first()
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    Ok(json!({
                        "protocol": STORAGE_DOWNLOAD_PROTOCOL,
                        "version": STORAGE_DOWNLOAD_PROTOCOL_VERSION,
                        "moduleOperationId": operation_id,
                        "cid": "cid-host",
                        "cancelStatus": "canceled",
                    }))
                }
                method => Err(anyhow::anyhow!("unexpected fake host method `{method}`")),
            };
            Box::pin(async move {
                let value = result?;
                if acknowledgement_pending {
                    std::future::pending::<()>().await;
                }
                Ok(crate::modules::logos_core::ModuleCallReply::new(
                    ModuleTransportKind::Module,
                    value,
                ))
            })
        }

        fn subscribe_module_event(
            &self,
            module: &str,
            event: &str,
        ) -> crate::modules::logos_core::ModuleTransportResult<BoxedModuleEventSubscription>
        {
            anyhow::ensure!(
                module == super::super::layer::module_id() && event == STORAGE_DOWNLOAD_DONE_EVENT,
                "unexpected fake host subscription"
            );
            let (sender, receiver) = mpsc::sync_channel(8);
            self.subscriptions
                .lock()
                .map_err(|error| anyhow::anyhow!("fake subscription lock failed: {error}"))?
                .push(sender);
            Ok(Box::new(FakeHostSubscription { receiver }))
        }

        fn supports_shared_file_staging(&self) -> bool {
            true
        }

        fn native_runtime_module_events_ready(&self) -> bool {
            true
        }
    }

    fn host_download_control(timeout: Duration) -> CommandControl {
        CommandControl::new(CancellationToken::new(), Instant::now() + timeout)
    }

    #[test]
    fn download_terminal_payload_requires_every_identity_and_outcome_field() -> Result<()> {
        let complete = json!({
            "protocol": STORAGE_DOWNLOAD_PROTOCOL,
            "version": STORAGE_DOWNLOAD_PROTOCOL_VERSION,
            "moduleOperationId": "operation-required-fields",
            "cid": "cid-required-fields",
            "outcome": "succeeded",
        });
        for field in ["protocol", "version", "moduleOperationId", "cid", "outcome"] {
            let mut incomplete = complete.clone();
            incomplete
                .as_object_mut()
                .context("terminal fixture is not an object")?
                .remove(field);
            let error = decode_download_terminal_payload(
                &incomplete.to_string(),
                "operation-required-fields",
                "cid-required-fields",
            )
            .err()
            .with_context(|| format!("terminal without `{field}` should fail"))?;
            anyhow::ensure!(
                error
                    .to_string()
                    .contains("terminal payload is invalid JSON")
                    && format!("{error:#}").contains("missing field"),
                "terminal without `{field}` returned unrelated error: {error:#}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn host_backup_download_correlates_terminal_and_cleans_staging() -> Result<()> {
        for behavior in [
            HostDownloadBehavior::Succeed,
            HostDownloadBehavior::WrongOperationThenSucceed,
            HostDownloadBehavior::DuplicateSuccess,
        ] {
            let fake = Arc::new(FakeHostDownloadTransport::new(behavior, b"backup"));
            let transport: SharedModuleTransport = fake.clone();
            let bytes = host_module_download_backup_bytes_controlled(
                &transport,
                &transport,
                "cid-host",
                false,
                64,
                host_download_control(Duration::from_secs(2)),
            )
            .await?;

            anyhow::ensure!(bytes == b"backup", "host backup bytes drifted");
            anyhow::ensure!(
                fake.cancel_calls.load(Ordering::Acquire) == 0,
                "successful host backup was canceled"
            );
            let staged_paths = fake
                .staged_paths
                .lock()
                .map_err(|error| anyhow::anyhow!("fake staged path lock failed: {error}"))?;
            anyhow::ensure!(
                staged_paths.iter().all(|path| !path.exists()),
                "successful host backup retained staging state: {staged_paths:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn host_backup_download_cancels_malformed_wrong_cid_and_oversized_effects() -> Result<()>
    {
        for (behavior, payload, max_bytes, expected) in [
            (
                HostDownloadBehavior::WrongCid,
                b"backup".as_slice(),
                64,
                "wrong CID",
            ),
            (
                HostDownloadBehavior::MalformedTerminal,
                b"backup".as_slice(),
                64,
                "invalid JSON",
            ),
            (
                HostDownloadBehavior::Oversized,
                b"12345678".as_slice(),
                8,
                "exceeded 8 byte limit",
            ),
        ] {
            let fake = Arc::new(FakeHostDownloadTransport::new(behavior, payload));
            let transport: SharedModuleTransport = fake.clone();
            let error = host_module_download_backup_bytes_controlled(
                &transport,
                &transport,
                "cid-host",
                false,
                max_bytes,
                host_download_control(Duration::from_secs(2)),
            )
            .await
            .err()
            .with_context(|| format!("{behavior:?} host download should fail"))?;

            anyhow::ensure!(
                format!("{error:#}").contains(expected),
                "{behavior:?} returned unrelated error: {error:#}"
            );
            anyhow::ensure!(
                fake.cancel_calls.load(Ordering::Acquire) == 1,
                "{behavior:?} did not cancel the accepted effect"
            );
            let staged_paths = fake
                .staged_paths
                .lock()
                .map_err(|error| anyhow::anyhow!("fake staged path lock failed: {error}"))?;
            anyhow::ensure!(
                staged_paths.iter().all(|path| !path.exists()),
                "{behavior:?} retained staging state: {staged_paths:?}"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn host_backup_download_timeout_and_close_cancel_the_exact_effect() -> Result<()> {
        for behavior in [
            HostDownloadBehavior::NoTerminal,
            HostDownloadBehavior::CloseBeforeTerminal,
        ] {
            let fake = Arc::new(FakeHostDownloadTransport::new(behavior, b"backup"));
            let transport: SharedModuleTransport = fake.clone();
            let error = host_module_download_backup_bytes_controlled(
                &transport,
                &transport,
                "cid-host",
                false,
                64,
                host_download_control(Duration::from_millis(75)),
            )
            .await
            .err()
            .with_context(|| format!("{behavior:?} host download should fail"))?;

            if behavior == HostDownloadBehavior::NoTerminal {
                anyhow::ensure!(
                    error
                        .downcast_ref::<crate::support::command_runner::CommandTerminated>()
                        .is_some(),
                    "host timeout lost command termination evidence: {error:#}"
                );
            } else {
                anyhow::ensure!(
                    format!("{error:#}").contains("fake Basecamp host transport closed"),
                    "host close returned unrelated error: {error:#}"
                );
            }
            anyhow::ensure!(
                fake.cancel_calls.load(Ordering::Acquire) == 1,
                "{behavior:?} did not cancel the exact accepted effect"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn host_backup_download_external_cancellation_uses_fresh_cleanup_control() -> Result<()> {
        let fake = Arc::new(FakeHostDownloadTransport::new(
            HostDownloadBehavior::NoTerminal,
            b"backup",
        ));
        let transport: SharedModuleTransport = fake.clone();
        let task_transport = Arc::clone(&transport);
        let cancellation = CancellationToken::new();
        let control = CommandControl::new(
            cancellation.clone(),
            Instant::now() + Duration::from_secs(2),
        );
        let task = tokio::spawn(async move {
            host_module_download_backup_bytes_controlled(
                &task_transport,
                &task_transport,
                "cid-host",
                false,
                64,
                control,
            )
            .await
        });
        let wait_deadline = Instant::now() + Duration::from_secs(1);
        while fake.download_calls.load(Ordering::Acquire) == 0 {
            anyhow::ensure!(
                Instant::now() < wait_deadline,
                "host download was not dispatched before cancellation"
            );
            tokio::task::yield_now().await;
        }
        cancellation.cancel();
        let error = task
            .await
            .context("host cancellation test task failed")?
            .err()
            .context("canceled host download should fail")?;

        anyhow::ensure!(
            error
                .downcast_ref::<crate::support::command_runner::CommandTerminated>()
                .is_some(),
            "host cancellation lost command termination evidence: {error:#}"
        );
        anyhow::ensure!(
            fake.cancel_calls.load(Ordering::Acquire) == 1,
            "host cancellation reused the canceled operation control"
        );
        Ok(())
    }

    #[tokio::test]
    async fn host_backup_download_confirms_cancellation_during_dispatch_acknowledgement()
    -> Result<()> {
        let fake = Arc::new(FakeHostDownloadTransport::new(
            HostDownloadBehavior::PendingAcknowledgement,
            b"backup",
        ));
        let cancellation = CancellationToken::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        let controlled: SharedModuleTransport = Arc::new(ControlledHostDownloadTransport {
            transport: Arc::clone(&fake),
            control: crate::modules::logos_core::ModuleCallControl::new(
                cancellation.clone(),
                tokio::time::Instant::from_std(deadline),
                Arc::new(AtomicU8::new(1)),
            ),
        });
        let cleanup: SharedModuleTransport = fake.clone();
        let task_cancellation = cancellation.clone();
        let task = tokio::spawn(async move {
            host_module_download_backup_bytes_controlled(
                &controlled,
                &cleanup,
                "cid-host",
                false,
                64,
                CommandControl::new(task_cancellation, deadline),
            )
            .await
        });
        let wait_deadline = Instant::now() + Duration::from_secs(1);
        while fake.download_calls.load(Ordering::Acquire) == 0 {
            anyhow::ensure!(
                Instant::now() < wait_deadline,
                "host download did not enter acknowledgement wait"
            );
            tokio::task::yield_now().await;
        }
        cancellation.cancel();
        let error = task
            .await
            .context("dispatch-cancellation task failed")?
            .err()
            .context("dispatch cancellation should not complete")?;
        let terminated = error
            .downcast_ref::<crate::modules::logos_core::ModuleCallTerminated>()
            .context("dispatch cancellation lost typed module termination")?;

        anyhow::ensure!(
            terminated.reason()
                == crate::modules::logos_core::ModuleCallStopReason::CancelRequested
                && terminated.evidence()
                    == ModuleCallTerminationEvidence::RemoteEffectTerminationConfirmed,
            "dispatch cancellation lost confirmed remote settlement: {error:#}"
        );
        anyhow::ensure!(
            fake.download_calls.load(Ordering::Acquire) == 1
                && fake.cancel_calls.load(Ordering::Acquire) == 1,
            "dispatch cancellation did not settle the exact host effect"
        );
        let staged_paths = fake
            .staged_paths
            .lock()
            .map_err(|error| anyhow::anyhow!("fake staged path lock failed: {error}"))?;
        anyhow::ensure!(
            staged_paths.iter().all(|path| !path.exists()),
            "dispatch cancellation retained staging: {staged_paths:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn host_backup_download_cancels_when_terminal_wait_cannot_be_supervised() -> Result<()> {
        let fake = Arc::new(FakeHostDownloadTransport::new(
            HostDownloadBehavior::Succeed,
            b"backup",
        ));
        let transport: SharedModuleTransport = fake.clone();
        let blocking_work = crate::support::work_tracker::BlockingWorkTracker::new();
        blocking_work.stop_accepting();
        let control = CommandControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_secs(2),
        )
        .with_blocking_work_tracker(blocking_work);

        let error = host_module_download_backup_bytes_controlled(
            &transport, &transport, "cid-host", false, 64, control,
        )
        .await
        .err()
        .context("unsupervised terminal wait should fail")?;

        anyhow::ensure!(
            error.to_string() == "blocking work tracker is closed",
            "terminal supervision failure drifted: {error:#}"
        );
        anyhow::ensure!(
            fake.download_calls.load(Ordering::Acquire) == 1
                && fake.cancel_calls.load(Ordering::Acquire) == 1,
            "accepted host effect escaped exact cancellation"
        );
        let staged_paths = fake
            .staged_paths
            .lock()
            .map_err(|error| anyhow::anyhow!("fake staged path lock failed: {error}"))?;
        anyhow::ensure!(
            staged_paths.iter().all(|path| !path.exists()),
            "unsupervised host download retained staging: {staged_paths:?}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn host_backup_download_rejects_interface_mismatch_before_dispatch() -> Result<()> {
        let fake = Arc::new(FakeHostDownloadTransport::new(
            HostDownloadBehavior::WrongInterface,
            b"backup",
        ));
        let transport: SharedModuleTransport = fake.clone();
        let error = host_module_download_backup_bytes_controlled(
            &transport,
            &transport,
            "cid-host",
            false,
            64,
            host_download_control(Duration::from_secs(2)),
        )
        .await
        .err()
        .context("mismatched host interface should fail")?;

        anyhow::ensure!(
            error
                .to_string()
                .contains("downloadToUrlV2(QString,QString,bool,int,QString,int)"),
            "host interface mismatch returned unrelated error: {error:#}"
        );
        anyhow::ensure!(
            fake.download_calls.load(Ordering::Acquire) == 0
                && fake.cancel_calls.load(Ordering::Acquire) == 0,
            "host interface mismatch reached a transfer effect"
        );
        Ok(())
    }

    #[cfg(unix)]
    const SUCCEEDED_TERMINAL: &str = r#"{"protocol":"logos.storage.download","version":2,"moduleOperationId":"__OPERATION_ID__","cid":"__CID__","outcome":"succeeded"}"#;

    #[cfg(unix)]
    const FAILED_TERMINAL: &str = r#"{"protocol":"logos.storage.download","version":2,"moduleOperationId":"__OPERATION_ID__","cid":"__CID__","outcome":"failed","error":"not found"}"#;

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
    fn backup_download_cleanup_control_keeps_budget_but_uses_fresh_lifecycle() -> Result<()> {
        let parent_cancellation = CancellationToken::new();
        let parent = CommandControl::new(parent_cancellation.clone(), Instant::now())
            .with_isolated_test_budget();
        let cleanup = backup_download_cleanup_control(&parent, Duration::from_secs(30))?;

        anyhow::ensure!(
            parent.shares_command_budget_with(&cleanup),
            "backup cleanup did not retain its parent command budget"
        );
        parent_cancellation.cancel();
        cleanup.check_active()?;

        let ordinary = CommandControl::new(
            CancellationToken::new(),
            Instant::now() + Duration::from_secs(30),
        );
        let ordinary_cleanup = backup_download_cleanup_control(&ordinary, Duration::from_secs(30))?;
        anyhow::ensure!(
            ordinary.shares_command_budget_with(&ordinary_cleanup),
            "ordinary backup cleanup left the production command budget"
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
        let _test_permit = serialize_cli_backup_test();
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
