use anyhow::{Context as _, Result, bail};
use reqwest::Response;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::{
    fmt, fs,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::Duration,
};

use crate::modules::logos_core::{
    ModuleCallControl, ModuleCallStopReason, ModuleCallTerminated, ModuleCallTerminationEvidence,
    ModuleTransportKind, SharedModuleTransport,
};
use crate::source_routing::{
    AdapterInitialization, ModuleDispatchIdentityRole, ModuleDispatchReceipt,
    ModuleEventCorrelationKind, ModuleTerminalEventContract, NodeOperationOutcome,
    NodeOperationRequest, ObservableOperationAcceptance, StorageSourceMode,
};
use crate::support::{
    args::Args,
    command_runner::{
        CommandControl, CommandStopReason, CommandTerminated, CommandTerminationScope,
    },
    settings_backup::SETTINGS_BACKUP_MAX_BYTES,
};

#[cfg(test)]
use super::BACKUP_CID_MAX_BYTES;
use super::{layer::STORAGE_SOURCE_MODES, parse_backup_cid, transport};

const DEFAULT_BLOCK_SIZE: u64 = 65_536;
const MANIFEST_POLL_INTERVAL: Duration = Duration::from_millis(100);
const STORAGE_UPLOAD_DONE_EVENT: &str = "storageUploadDone";
const MAX_UNRELATED_UPLOAD_EVENTS: usize = 64;

#[derive(Debug)]
pub(crate) struct StorageUploadSettlementUnconfirmed {
    message: String,
}

impl StorageUploadSettlementUnconfirmed {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl fmt::Display for StorageUploadSettlementUnconfirmed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for StorageUploadSettlementUnconfirmed {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StorageOperation {
    Manifests,
    DownloadManifest,
    Fetch,
    Upload,
    Download,
    Remove,
}

pub(crate) enum StorageOperationOutput {
    Outcome(NodeOperationOutcome),
    Download(StorageDownloadRequest),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageDownloadRequest {
    endpoint: String,
    cid: String,
    path: String,
    local_only: bool,
}

impl StorageDownloadRequest {
    pub(crate) fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub(crate) fn cid(&self) -> &str {
        &self.cid
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn local_only(&self) -> bool {
        self.local_only
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StorageOperationAdapter {
    Module(ModuleTransportKind),
    Rest { endpoint: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StorageClient {
    adapter: StorageOperationAdapter,
}

impl StorageClient {
    pub(crate) fn from_initialization(value: &Value) -> Result<Self> {
        let initialization = AdapterInitialization::parse(value, STORAGE_SOURCE_MODES, "rest")?;
        let adapter = match StorageSourceMode::from_token(initialization.source_mode()) {
            StorageSourceMode::Module => {
                StorageOperationAdapter::Module(ModuleTransportKind::Module)
            }
            StorageSourceMode::LogoscoreCli => {
                StorageOperationAdapter::Module(ModuleTransportKind::LogoscoreCli)
            }
            StorageSourceMode::Rest => StorageOperationAdapter::Rest {
                endpoint: initialization
                    .input("rest_endpoint")
                    .context("Storage REST URL is required")?
                    .to_owned(),
            },
            StorageSourceMode::Metrics => {
                bail!("Storage data actions require storage REST or module source, not metrics")
            }
            StorageSourceMode::Unsupported => bail!(
                "storage source mode `{}` is not supported",
                initialization.source_mode()
            ),
        };
        Ok(Self { adapter })
    }

    #[cfg(test)]
    pub(crate) fn endpoint(&self) -> Option<&str> {
        match &self.adapter {
            StorageOperationAdapter::Module(_) => None,
            StorageOperationAdapter::Rest { endpoint } => Some(endpoint),
        }
    }

    pub(crate) fn source(&self) -> &str {
        match &self.adapter {
            StorageOperationAdapter::Module(ModuleTransportKind::Module) => "module storage_module",
            StorageOperationAdapter::Module(ModuleTransportKind::LogoscoreCli) => {
                "logoscore call storage_module"
            }
            StorageOperationAdapter::Rest { endpoint } => endpoint,
        }
    }

    pub(crate) const fn confirms_backup_download_stop(&self) -> bool {
        matches!(
            self.adapter,
            StorageOperationAdapter::Module(
                ModuleTransportKind::Module | ModuleTransportKind::LogoscoreCli
            )
        )
    }

    pub(crate) const fn backup_download_termination_handshake_grace(&self) -> Option<Duration> {
        if matches!(
            self.adapter,
            StorageOperationAdapter::Module(
                ModuleTransportKind::Module | ModuleTransportKind::LogoscoreCli
            )
        ) {
            Some(transport::BACKUP_DOWNLOAD_TERMINATION_HANDSHAKE_GRACE)
        } else {
            None
        }
    }

    pub(crate) async fn exists(
        &self,
        module_transport: &SharedModuleTransport,
        cid: &str,
    ) -> Result<Value> {
        match &self.adapter {
            StorageOperationAdapter::Module(transport_kind) => {
                transport::module_call(
                    module_transport,
                    *transport_kind,
                    "exists",
                    vec![json!(cid)],
                )
                .await
            }
            StorageOperationAdapter::Rest { endpoint } => transport::exists(endpoint, cid).await,
        }
    }

    pub(crate) async fn upload_bytes_controlled(
        &self,
        module_transport: &SharedModuleTransport,
        filename: &str,
        bytes: &[u8],
        block_size: u64,
        control: CommandControl,
    ) -> Result<Value> {
        self.ensure_managed_byte_upload_supported(module_transport)?;
        match &self.adapter {
            StorageOperationAdapter::Module(ModuleTransportKind::Module) => {
                bail!("Basecamp module source does not support Inspector-managed byte staging")
            }
            StorageOperationAdapter::Module(ModuleTransportKind::LogoscoreCli) => {
                let runtime = module_transport
                    .logoscore_cli_transport()
                    .context("active LogosCore CLI transport does not expose its runtime")?
                    .runtime()?;
                transport::module_upload_bytes_controlled(
                    runtime, filename, bytes, block_size, control,
                )
                .await
            }
            StorageOperationAdapter::Rest { endpoint } => {
                transport::upload_bytes_controlled(endpoint, filename, bytes, block_size, control)
                    .await
            }
        }
    }

    pub(crate) fn ensure_managed_byte_upload_supported(
        &self,
        module_transport: &SharedModuleTransport,
    ) -> Result<()> {
        match &self.adapter {
            StorageOperationAdapter::Module(ModuleTransportKind::Module) => {
                bail!("Basecamp module source does not support Inspector-managed byte staging")
            }
            StorageOperationAdapter::Module(ModuleTransportKind::LogoscoreCli)
                if module_transport.kind() != ModuleTransportKind::LogoscoreCli =>
            {
                bail!(
                    "resolved module transport `logoscore_cli` is unavailable; active transport is `{}`",
                    module_transport.kind().as_str()
                )
            }
            StorageOperationAdapter::Module(ModuleTransportKind::LogoscoreCli)
            | StorageOperationAdapter::Rest { .. } => Ok(()),
        }
    }

    pub(crate) fn ensure_managed_backup_download_supported(
        &self,
        module_transport: &SharedModuleTransport,
    ) -> Result<()> {
        if let StorageOperationAdapter::Module(expected) = &self.adapter {
            anyhow::ensure!(
                module_transport.kind() == *expected,
                "resolved module transport `{}` is unavailable; active transport is `{}`",
                expected.as_str(),
                module_transport.kind().as_str()
            );
            if *expected == ModuleTransportKind::Module {
                anyhow::ensure!(
                    module_transport.supports_shared_file_staging(),
                    "Basecamp host transport does not provide shared file staging"
                );
                anyhow::ensure!(
                    module_transport.native_runtime_module_events_ready(),
                    "Basecamp host transport does not own healthy native runtime module-event ingress"
                );
            }
        }
        Ok(())
    }

    pub(crate) async fn download_bytes_bounded(
        &self,
        cid: &str,
        local_only: bool,
        module_error: &str,
        max_bytes: usize,
    ) -> Result<Vec<u8>> {
        match &self.adapter {
            StorageOperationAdapter::Module(_) => bail!("{module_error}"),
            StorageOperationAdapter::Rest { endpoint } => {
                transport::download_bytes(endpoint, cid, local_only, max_bytes).await
            }
        }
    }

    pub(crate) async fn download_backup_bytes_controlled(
        &self,
        module_transport: &SharedModuleTransport,
        cleanup_module_transport: &SharedModuleTransport,
        cid: &str,
        local_only: bool,
        control: CommandControl,
    ) -> Result<Vec<u8>> {
        match &self.adapter {
            StorageOperationAdapter::Module(ModuleTransportKind::Module) => {
                transport::host_module_download_backup_bytes_controlled(
                    module_transport,
                    cleanup_module_transport,
                    cid,
                    local_only,
                    SETTINGS_BACKUP_MAX_BYTES,
                    control,
                )
                .await
            }
            StorageOperationAdapter::Module(ModuleTransportKind::LogoscoreCli) => {
                let runtime = module_transport
                    .logoscore_cli_transport()
                    .context("active LogosCore CLI transport does not expose its runtime")?
                    .runtime()?;
                transport::module_download_backup_bytes_controlled(
                    runtime,
                    cid,
                    local_only,
                    SETTINGS_BACKUP_MAX_BYTES,
                    control,
                )
                .await
            }
            StorageOperationAdapter::Rest { endpoint } => {
                transport::download_bytes_controlled(
                    endpoint,
                    cid,
                    local_only,
                    SETTINGS_BACKUP_MAX_BYTES,
                    control,
                )
                .await
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct StorageOperationRequest {
    plan: StorageOperationPlan,
    context: Map<String, Value>,
}

impl StorageOperationRequest {
    pub(crate) fn parse(
        request: &NodeOperationRequest,
        operation: StorageOperation,
    ) -> Result<Self> {
        let client = StorageClient::from_initialization(request.adapter())?;
        let (plan, context) = operation_plan(request, operation, client)?;
        Ok(Self { plan, context })
    }

    #[must_use]
    pub(crate) fn context(&self) -> &Map<String, Value> {
        &self.context
    }
}

pub(crate) async fn execute_operation(
    request: StorageOperationRequest,
    module_transport: SharedModuleTransport,
    control: ModuleCallControl,
) -> Result<StorageOperationOutput> {
    execute_plan(request.plan, module_transport, control).await
}

pub(crate) async fn download_response(request: &StorageDownloadRequest) -> Result<Response> {
    transport::download_response(request.endpoint(), request.cid(), request.local_only()).await
}

#[derive(Debug, Clone, PartialEq)]
enum StorageOperationPlan {
    Module {
        transport: ModuleTransportKind,
        method: &'static str,
        args: Vec<Value>,
        context: Vec<(&'static str, String)>,
        dispatch: bool,
    },
    Rest(StorageRestOperation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StorageRestOperation {
    Manifests {
        endpoint: String,
    },
    DownloadManifest {
        endpoint: String,
        cid: String,
    },
    Fetch {
        endpoint: String,
        cid: String,
    },
    Upload {
        endpoint: String,
        path: String,
        block_size: u64,
    },
    Download(StorageDownloadRequest),
    Remove {
        endpoint: String,
        cid: String,
    },
}

#[derive(Debug, Default, Deserialize)]
struct EmptyPayload {}

#[derive(Debug, Deserialize)]
struct CidPayload {
    cid: String,
}

#[derive(Debug, Deserialize)]
struct UploadPayload {
    path: String,
    #[serde(default = "default_block_size")]
    block_size: u64,
}

#[derive(Debug, Deserialize)]
struct DownloadPayload {
    cid: String,
    path: String,
    #[serde(default)]
    local_only: bool,
    #[serde(default = "default_block_size")]
    block_size: u64,
}

fn operation_plan(
    request: &NodeOperationRequest,
    operation: StorageOperation,
    client: StorageClient,
) -> Result<(StorageOperationPlan, Map<String, Value>)> {
    match operation {
        StorageOperation::Manifests => {
            let _payload: EmptyPayload = request.payload("storage manifests")?;
            plan_for_client(client, "manifests", Vec::new(), Vec::new(), false)
        }
        StorageOperation::DownloadManifest => {
            let payload: CidPayload = request.payload("storage manifest")?;
            let cid = required_text(payload.cid, "CID")?;
            plan_for_client(
                client,
                "downloadManifest",
                vec![json!(cid)],
                vec![("cid", cid)],
                true,
            )
        }
        StorageOperation::Fetch => {
            let payload: CidPayload = request.payload("storage fetch")?;
            let cid = required_text(payload.cid, "CID")?;
            plan_for_client(client, "fetch", vec![json!(cid)], vec![("cid", cid)], true)
        }
        StorageOperation::Upload => {
            let payload: UploadPayload = request.payload("storage upload")?;
            let path = required_text(payload.path, "file path")?;
            if path.starts_with("http://") || path.starts_with("https://") {
                bail!("storage REST upload expects a local file path")
            }
            let block_size = payload.block_size.max(1);
            plan_for_client(
                client,
                "uploadUrl",
                vec![json!(path), json!(block_size)],
                vec![("path", path)],
                true,
            )
        }
        StorageOperation::Download => {
            let payload: DownloadPayload = request.payload("storage download")?;
            let cid = required_text(payload.cid, "CID")?;
            let path = required_text(payload.path, "download path")?;
            let mut context = context_map(&[("cid", cid.clone()), ("path", path.clone())]);
            context.insert(
                "source".to_owned(),
                json!(if payload.local_only {
                    "local"
                } else {
                    "network"
                }),
            );
            match client.adapter {
                StorageOperationAdapter::Module(transport) => Ok((
                    StorageOperationPlan::Module {
                        transport,
                        method: "downloadToUrl",
                        args: vec![
                            json!(cid),
                            json!(path),
                            json!(payload.local_only),
                            json!(payload.block_size.max(1)),
                        ],
                        context: vec![("cid", cid), ("path", path)],
                        dispatch: true,
                    },
                    context,
                )),
                StorageOperationAdapter::Rest { endpoint } => Ok((
                    StorageOperationPlan::Rest(StorageRestOperation::Download(
                        StorageDownloadRequest {
                            endpoint,
                            cid,
                            path,
                            local_only: payload.local_only,
                        },
                    )),
                    context,
                )),
            }
        }
        StorageOperation::Remove => {
            let payload: CidPayload = request.payload("storage remove")?;
            let cid = required_text(payload.cid, "CID")?;
            plan_for_client(client, "remove", vec![json!(cid)], vec![("cid", cid)], true)
        }
    }
}

fn plan_for_client(
    client: StorageClient,
    method: &'static str,
    args: Vec<Value>,
    context: Vec<(&'static str, String)>,
    dispatch: bool,
) -> Result<(StorageOperationPlan, Map<String, Value>)> {
    let context_map = context_map(&context);
    match client.adapter {
        StorageOperationAdapter::Module(transport) => Ok((
            StorageOperationPlan::Module {
                transport,
                method,
                args,
                context,
                dispatch,
            },
            context_map,
        )),
        StorageOperationAdapter::Rest { endpoint } => {
            let operation = match method {
                "manifests" => StorageRestOperation::Manifests {
                    endpoint: endpoint.clone(),
                },
                "downloadManifest" => StorageRestOperation::DownloadManifest {
                    endpoint: endpoint.clone(),
                    cid: value_string(&args, 0),
                },
                "fetch" => StorageRestOperation::Fetch {
                    endpoint: endpoint.clone(),
                    cid: value_string(&args, 0),
                },
                "uploadUrl" => StorageRestOperation::Upload {
                    endpoint: endpoint.clone(),
                    path: value_string(&args, 0),
                    block_size: args
                        .get(1)
                        .and_then(Value::as_u64)
                        .unwrap_or(DEFAULT_BLOCK_SIZE),
                },
                "remove" => StorageRestOperation::Remove {
                    endpoint: endpoint.clone(),
                    cid: value_string(&args, 0),
                },
                _ => bail!("storage REST operation `{method}` is not supported"),
            };
            let mut context_map = context_map;
            context_map.insert("endpoint".to_owned(), json!(endpoint));
            Ok((StorageOperationPlan::Rest(operation), context_map))
        }
    }
}

async fn execute_plan(
    plan: StorageOperationPlan,
    module_transport: SharedModuleTransport,
    control: ModuleCallControl,
) -> Result<StorageOperationOutput> {
    let value = match plan {
        StorageOperationPlan::Module {
            transport: transport_kind,
            method,
            args,
            context,
            dispatch,
        } => {
            if method == "downloadManifest" {
                let cid = context
                    .iter()
                    .find_map(|(key, value)| (*key == "cid").then_some(value.as_str()))
                    .context("storage manifest fetch has no CID context")?;
                let value = module_manifest_by_cid(
                    &module_transport,
                    transport_kind,
                    cid,
                    args,
                    &context,
                    &control,
                )
                .await?;
                return Ok(StorageOperationOutput::Outcome(
                    NodeOperationOutcome::Completed(value),
                ));
            }
            if method == "uploadUrl" && transport_kind == ModuleTransportKind::LogoscoreCli {
                let outcome = logoscore_cli_upload_by_terminal_event(
                    &module_transport,
                    args,
                    &context,
                    &control,
                )
                .await?;
                return Ok(StorageOperationOutput::Outcome(outcome));
            }
            if dispatch {
                let identity_role = match method {
                    "uploadUrl" => ModuleDispatchIdentityRole::Session,
                    _ => ModuleDispatchIdentityRole::None,
                };
                let receipt = transport::module_dispatch(
                    &module_transport,
                    transport_kind,
                    method,
                    args,
                    &context,
                    identity_role,
                )
                .await?;
                return Ok(StorageOperationOutput::Outcome(
                    storage_module_dispatch_outcome(method, receipt)?,
                ));
            } else {
                let value =
                    transport::module_call(&module_transport, transport_kind, method, args).await?;
                return Ok(StorageOperationOutput::Outcome(
                    NodeOperationOutcome::Completed(value),
                ));
            }
        }
        StorageOperationPlan::Rest(StorageRestOperation::Manifests { endpoint }) => {
            transport::manifests(&endpoint).await?
        }
        StorageOperationPlan::Rest(StorageRestOperation::DownloadManifest { endpoint, cid }) => {
            transport::manifest(&endpoint, &cid).await?
        }
        StorageOperationPlan::Rest(StorageRestOperation::Fetch { endpoint, cid }) => {
            let acknowledgement = transport::fetch(&endpoint, &cid)
                .await
                .with_context(|| format!("failed to start storage network fetch for {cid}"))?;
            return Ok(StorageOperationOutput::Outcome(
                NodeOperationOutcome::Dispatched(acknowledgement),
            ));
        }
        StorageOperationPlan::Rest(StorageRestOperation::Upload {
            endpoint,
            path,
            block_size,
        }) => transport::upload(&endpoint, &path, block_size)
            .await
            .with_context(|| format!("failed to upload `{path}` through storage REST"))?,
        StorageOperationPlan::Rest(StorageRestOperation::Download(request)) => {
            return Ok(StorageOperationOutput::Download(request));
        }
        StorageOperationPlan::Rest(StorageRestOperation::Remove { endpoint, cid }) => {
            transport::remove(&endpoint, &cid)
                .await
                .with_context(|| format!("failed to remove storage CID {cid}"))?
        }
    };
    Ok(StorageOperationOutput::Outcome(
        NodeOperationOutcome::Completed(value),
    ))
}

async fn logoscore_cli_upload_by_terminal_event(
    module_transport: &SharedModuleTransport,
    dispatch_args: Vec<Value>,
    context: &[(&'static str, String)],
    control: &ModuleCallControl,
) -> Result<NodeOperationOutcome> {
    ensure_manifest_poll_active(control, false)?;
    let path = context
        .iter()
        .find_map(|(key, value)| (*key == "path").then_some(value.as_str()))
        .context("storage upload has no path context")?;
    let upload_path = Path::new(path);
    anyhow::ensure!(
        upload_path.is_absolute(),
        "storage upload file path must be absolute"
    );
    let metadata = fs::metadata(path)
        .with_context(|| format!("failed to inspect storage upload file `{path}`"))?;
    anyhow::ensure!(
        metadata.is_file(),
        "storage upload path is not a file: `{path}`"
    );
    let dispatch_path = dispatch_args
        .first()
        .and_then(Value::as_str)
        .context("storage upload has no path argument")?;
    anyhow::ensure!(
        dispatch_path == path,
        "storage upload path argument does not match operation context"
    );
    let block_size = dispatch_args
        .get(1)
        .and_then(Value::as_u64)
        .filter(|value| *value > 0)
        .context("storage upload has no valid block size")?;
    let runtime = module_transport
        .logoscore_cli_transport()
        .context("active LogosCore CLI transport does not expose its runtime")?
        .runtime()?;
    let command_control = control.command_control();
    let worker_guard = control.blocking_worker_guard()?;
    let path = path.to_owned();
    let effect_may_have_started = Arc::new(AtomicBool::new(false));
    let worker_effect_may_have_started = Arc::clone(&effect_may_have_started);
    let context = context
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect::<Vec<_>>();
    let result = tokio::task::spawn_blocking(move || {
        let _worker_guard = worker_guard;
        logoscore_cli_upload_by_terminal_event_blocking(
            &runtime,
            &path,
            block_size,
            &context,
            command_control,
            &worker_effect_may_have_started,
        )
    })
    .await;
    match result {
        Ok(Err(error)) if error.downcast_ref::<CommandTerminated>().is_some() => {
            Err(normalize_upload_predispatch_stop(error, control))
        }
        Ok(result) => result,
        Err(error) if effect_may_have_started.load(Ordering::Acquire) => {
            Err(StorageUploadSettlementUnconfirmed::new(format!(
                "Storage upload worker ended after dispatch may have started and before authoritative settlement: {error}"
            ))
            .into())
        }
        Err(error) => Err(anyhow::anyhow!(
            "Storage upload worker failed before dispatch: {error}"
        )),
    }
}

#[derive(Debug, PartialEq, Eq)]
enum UploadTerminalEvent {
    Unrelated,
    Succeeded { cid: String },
    Failed { error: String },
}

fn logoscore_cli_upload_by_terminal_event_blocking(
    runtime: &crate::modules::logos_core::LogoscoreCliRuntime,
    path: &str,
    block_size: u64,
    context: &[(&'static str, String)],
    control: CommandControl,
    effect_may_have_started: &AtomicBool,
) -> Result<NodeOperationOutcome> {
    let mut watch = runtime
        .start_event_watch(
            super::layer::module_id(),
            STORAGE_UPLOAD_DONE_EVENT,
            &control,
        )
        .context("failed to start authoritative Storage upload event watch")?;
    if let Err(error) = watch.wait_ready(&control) {
        return cleanup_upload_watch_before_dispatch(error, &mut watch);
    }

    if let Err(error) = transport::logoscore_cli_call_value_controlled_with_runtime(
        runtime,
        super::layer::module_id(),
        "manifests",
        &[],
        control.clone(),
    ) {
        return cleanup_upload_watch_before_dispatch(
            error.context("failed Storage upload subscription ordering barrier"),
            &mut watch,
        );
    }

    let block_size = block_size.to_string();
    effect_may_have_started.store(true, Ordering::Release);
    let dispatch = transport::logoscore_cli_call_value_controlled_with_runtime(
        runtime,
        super::layer::module_id(),
        "uploadUrl",
        &[path.to_owned(), block_size],
        control.clone(),
    );
    let raw_session = match dispatch {
        Ok(value) => value,
        Err(error)
            if error
                .downcast_ref::<CommandTerminated>()
                .is_some_and(|terminated| {
                    terminated.scope() == CommandTerminationScope::NoProcess
                }) =>
        {
            return cleanup_upload_watch_before_dispatch(error, &mut watch);
        }
        Err(error) => {
            return upload_settlement_unconfirmed(
                error.context("Storage upload dispatch settlement is unknown"),
                &mut watch,
            );
        }
    };
    let session_id = match upload_session_id(&raw_session) {
        Some(session_id) => session_id,
        None => {
            return upload_settlement_unconfirmed(
                anyhow::anyhow!("storage_module.uploadUrl returned no session ID"),
                &mut watch,
            );
        }
    };

    let terminal = match wait_for_upload_terminal(&mut watch, &session_id, &control) {
        Ok(terminal) => terminal,
        Err(error) => {
            return upload_settlement_unconfirmed(
                error.context(format!(
                    "Storage upload session `{session_id}` lost authoritative settlement"
                )),
                &mut watch,
            );
        }
    };
    let result = match terminal {
        UploadTerminalEvent::Succeeded { cid } => Ok(NodeOperationOutcome::Completed(json!({
            "success": true,
            "sessionId": session_id,
            "cid": cid,
            "completion": STORAGE_UPLOAD_DONE_EVENT,
            "path": context
                .iter()
                .find_map(|(key, value)| (*key == "path").then_some(value)),
        }))),
        UploadTerminalEvent::Failed { error } => Err(anyhow::anyhow!(
            "storage_module upload session `{session_id}` failed: {error}"
        )),
        UploadTerminalEvent::Unrelated => Err(anyhow::anyhow!(
            "Storage upload terminal wait returned an unrelated event"
        )),
    };
    complete_upload_watch(&mut watch, result)
}

fn normalize_upload_predispatch_stop(
    error: anyhow::Error,
    control: &ModuleCallControl,
) -> anyhow::Error {
    let Some(terminated) = error.downcast_ref::<CommandTerminated>() else {
        return error;
    };
    let reason = match terminated.reason() {
        CommandStopReason::CancelRequested => control.stop_reason(),
        CommandStopReason::DeadlineExceeded => ModuleCallStopReason::DeadlineExceeded,
    };
    ModuleCallTerminated::new(reason, ModuleCallTerminationEvidence::NotStarted).into()
}

fn upload_session_id(value: &Value) -> Option<String> {
    value
        .as_str()
        .or_else(|| {
            value
                .as_object()
                .and_then(|object| object.get("sessionId").or_else(|| object.get("session_id")))
                .and_then(Value::as_str)
        })
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn wait_for_upload_terminal(
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
    session_id: &str,
    control: &CommandControl,
) -> Result<UploadTerminalEvent> {
    let mut unrelated = 0_usize;
    loop {
        let Some(value) = watch.next_value_within(control, Duration::from_millis(100))? else {
            continue;
        };
        match decode_upload_terminal_event(&value, session_id)? {
            UploadTerminalEvent::Unrelated => {
                unrelated = unrelated.saturating_add(1);
                if unrelated > MAX_UNRELATED_UPLOAD_EVENTS {
                    bail!(
                        "Storage upload received more than {MAX_UNRELATED_UPLOAD_EVENTS} unrelated terminal events"
                    );
                }
            }
            terminal => return Ok(terminal),
        }
    }
}

fn decode_upload_terminal_event(
    value: &Value,
    expected_session_id: &str,
) -> Result<UploadTerminalEvent> {
    anyhow::ensure!(
        value.get("module").and_then(Value::as_str) == Some(super::layer::module_id()),
        "logoscore upload watcher returned an event for the wrong module"
    );
    anyhow::ensure!(
        value.get("event").and_then(Value::as_str) == Some(STORAGE_UPLOAD_DONE_EVENT),
        "logoscore upload watcher returned the wrong event type"
    );
    let data = value
        .get("data")
        .and_then(Value::as_object)
        .context("logoscore upload terminal event has no data object")?;
    anyhow::ensure!(
        data.len() == 1 && data.contains_key("arg0"),
        "logoscore upload terminal event must contain exactly one payload argument"
    );
    let payload = match data.get("arg0") {
        Some(Value::String(payload)) => serde_json::from_str::<Value>(payload)
            .context("logoscore upload terminal payload is not valid JSON")?,
        Some(Value::Object(payload)) => Value::Object(payload.clone()),
        _ => bail!("logoscore upload terminal payload must be a JSON object or string"),
    };
    let payload = payload
        .as_object()
        .context("logoscore upload terminal payload is not an object")?;
    let session_id = payload
        .get("sessionId")
        .or_else(|| payload.get("session_id"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("logoscore upload terminal payload has no session ID")?;
    if session_id != expected_session_id {
        return Ok(UploadTerminalEvent::Unrelated);
    }
    let success = payload
        .get("success")
        .and_then(Value::as_bool)
        .context("logoscore upload terminal payload has no success flag")?;
    if success {
        let cid = payload
            .get("cid")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("successful logoscore upload terminal payload has no CID")?;
        return Ok(UploadTerminalEvent::Succeeded {
            cid: cid.to_owned(),
        });
    }
    let error = payload
        .get("error")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("failed logoscore upload terminal payload has no error")?;
    Ok(UploadTerminalEvent::Failed {
        error: error.to_owned(),
    })
}

fn cleanup_upload_watch_before_dispatch<T>(
    primary: anyhow::Error,
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
) -> Result<T> {
    match watch.stop() {
        Ok(()) => Err(primary),
        Err(cleanup) => Err(anyhow::anyhow!(
            "{primary:#}; pre-dispatch Storage upload watch cleanup failed: {cleanup:#}"
        )),
    }
}

fn upload_settlement_unconfirmed<T>(
    primary: anyhow::Error,
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
) -> Result<T> {
    let message = match watch.stop() {
        Ok(()) => format!("{primary:#}"),
        Err(cleanup) => {
            format!("{primary:#}; Storage upload watch cleanup failed: {cleanup:#}")
        }
    };
    Err(StorageUploadSettlementUnconfirmed::new(message).into())
}

fn complete_upload_watch<T>(
    watch: &mut crate::modules::logos_core::LogoscoreEventWatch,
    terminal_result: Result<T>,
) -> Result<T> {
    match (terminal_result, watch.stop()) {
        (result, Ok(())) => result,
        (Ok(_), Err(cleanup)) => Err(anyhow::anyhow!(
            "Storage upload settled successfully but event-watch cleanup failed: {cleanup:#}"
        )),
        (Err(primary), Err(cleanup)) => Err(anyhow::anyhow!(
            "{primary:#}; Storage upload terminal watch cleanup failed: {cleanup:#}"
        )),
    }
}

async fn module_manifest_by_cid(
    module_transport: &SharedModuleTransport,
    transport_kind: ModuleTransportKind,
    cid: &str,
    dispatch_args: Vec<Value>,
    context: &[(&'static str, String)],
    control: &ModuleCallControl,
) -> Result<Value> {
    ensure_manifest_poll_active(control, false)?;
    let _receipt = transport::module_dispatch(
        module_transport,
        transport_kind,
        "downloadManifest",
        dispatch_args,
        context,
        ModuleDispatchIdentityRole::None,
    )
    .await
    .with_context(|| format!("failed to dispatch Storage manifest fetch for {cid}"))?;

    loop {
        ensure_manifest_poll_active(control, true)?;
        let manifests =
            transport::module_call(module_transport, transport_kind, "manifests", Vec::new())
                .await
                .with_context(|| format!("failed to poll Storage manifest fetch for {cid}"))?;
        if let Some(manifest) = exact_manifest(&manifests, cid)? {
            return Ok(manifest);
        }
        wait_for_manifest_poll(control).await?;
    }
}

fn exact_manifest(manifests: &Value, cid: &str) -> Result<Option<Value>> {
    let manifests = manifests
        .as_array()
        .context("storage manifests response is not an array")?;
    let mut matches = manifests
        .iter()
        .filter(|manifest| manifest.get("cid").and_then(Value::as_str) == Some(cid));
    let Some(manifest) = matches.next() else {
        return Ok(None);
    };
    validate_manifest(manifest, cid)?;
    for duplicate in matches {
        validate_manifest(duplicate, cid)?;
        anyhow::ensure!(
            duplicate == manifest,
            "storage manifests returned conflicting rows for CID `{cid}`"
        );
    }
    Ok(Some(manifest.clone()))
}

fn validate_manifest(manifest: &Value, cid: &str) -> Result<()> {
    let manifest = manifest
        .as_object()
        .with_context(|| format!("storage manifest for CID `{cid}` is not an object"))?;
    for field in ["treeCid", "filename", "mimetype"] {
        anyhow::ensure!(
            manifest
                .get(field)
                .and_then(Value::as_str)
                .is_some_and(|value| !value.trim().is_empty()),
            "storage manifest for CID `{cid}` has no `{field}`"
        );
    }
    anyhow::ensure!(
        manifest
            .get("datasetSize")
            .and_then(Value::as_u64)
            .is_some(),
        "storage manifest for CID `{cid}` has invalid `datasetSize`"
    );
    anyhow::ensure!(
        manifest
            .get("blockSize")
            .and_then(Value::as_u64)
            .is_some_and(|value| value > 0),
        "storage manifest for CID `{cid}` has invalid `blockSize`"
    );
    Ok(())
}

fn ensure_manifest_poll_active(control: &ModuleCallControl, effect_started: bool) -> Result<()> {
    let evidence = if effect_started {
        ModuleCallTerminationEvidence::LocallyAbandoned
    } else {
        ModuleCallTerminationEvidence::NotStarted
    };
    if control.cancellation().is_cancelled() {
        return Err(manifest_poll_interrupted(control.stop_reason(), evidence).into());
    }
    if tokio::time::Instant::now() >= control.deadline() {
        return Err(
            manifest_poll_interrupted(ModuleCallStopReason::DeadlineExceeded, evidence).into(),
        );
    }
    Ok(())
}

fn manifest_poll_interrupted(
    reason: ModuleCallStopReason,
    evidence: ModuleCallTerminationEvidence,
) -> ModuleCallTerminated {
    ModuleCallTerminated::new(reason, evidence)
}

async fn wait_for_manifest_poll(control: &ModuleCallControl) -> Result<()> {
    ensure_manifest_poll_active(control, true)?;
    tokio::select! {
        biased;
        () = control.cancellation().cancelled() => {
            Err(manifest_poll_interrupted(
                control.stop_reason(),
                ModuleCallTerminationEvidence::LocallyAbandoned,
            ).into())
        },
        () = tokio::time::sleep_until(control.deadline()) => {
            Err(manifest_poll_interrupted(
                ModuleCallStopReason::DeadlineExceeded,
                ModuleCallTerminationEvidence::LocallyAbandoned,
            ).into())
        },
        () = tokio::time::sleep(MANIFEST_POLL_INTERVAL) => {
            ensure_manifest_poll_active(control, true)
        }
    }
}

fn storage_module_dispatch_outcome(
    method: &str,
    receipt: ModuleDispatchReceipt,
) -> Result<NodeOperationOutcome> {
    let accepted = match method {
        "uploadUrl" => receipt.session_correlation().map(|correlation| {
            (
                correlation,
                ModuleTerminalEventContract::new(
                    super::layer::module_id(),
                    Some("storageUploadProgress"),
                    "storageUploadDone",
                    None,
                    ModuleEventCorrelationKind::Session,
                ),
            )
        }),
        _ => None,
    };
    let acknowledgement = receipt.into_acknowledgement();
    match accepted {
        Some((correlation, terminal_event)) => Ok(NodeOperationOutcome::Accepted(Box::new(
            ObservableOperationAcceptance::new(acknowledgement, correlation, terminal_event),
        ))),
        None if method == "uploadUrl" => {
            bail!("storage module `{method}` returned no session ID")
        }
        None => Ok(NodeOperationOutcome::Dispatched(acknowledgement)),
    }
}

#[derive(Debug, Deserialize)]
struct ExistsPayload {
    cid: String,
}

pub(crate) struct StorageExistsRequest {
    client: StorageClient,
    cid: String,
}

impl StorageExistsRequest {
    pub(crate) fn parse(args: &Args) -> Result<Self> {
        let request = NodeOperationRequest::from_bridge_args(args)?;
        let payload: ExistsPayload = request.payload("storage exists")?;
        Ok(Self {
            client: StorageClient::from_initialization(request.adapter())?,
            cid: required_text(payload.cid, "CID")?,
        })
    }

    pub(crate) async fn execute(&self, module_transport: &SharedModuleTransport) -> Result<Value> {
        self.client.exists(module_transport, &self.cid).await
    }
}

#[derive(Debug, Deserialize)]
struct BackupUploadPayload {
    backup_catalog_id: String,
    #[serde(default = "default_block_size")]
    block_size: u64,
}

pub(crate) struct StorageBackupUploadRequest {
    client: StorageClient,
    backup_catalog_id: String,
    block_size: u64,
}

impl StorageBackupUploadRequest {
    pub(crate) fn parse_request(request: &NodeOperationRequest) -> Result<Self> {
        let payload: BackupUploadPayload = request.payload("settings backup")?;
        Ok(Self {
            client: StorageClient::from_initialization(request.adapter())?,
            backup_catalog_id: required_text(payload.backup_catalog_id, "backup catalog id")?,
            block_size: payload.block_size.max(1),
        })
    }

    pub(crate) fn backup_catalog_id(&self) -> &str {
        &self.backup_catalog_id
    }

    pub(crate) const fn block_size(&self) -> u64 {
        self.block_size
    }

    pub(crate) fn client(&self) -> &StorageClient {
        &self.client
    }
}

#[derive(Debug, Deserialize)]
struct PayloadUploadPayload {
    filename: String,
    payload: Value,
    #[serde(default = "default_block_size")]
    block_size: u64,
}

pub(crate) struct StoragePayloadUploadRequest {
    client: StorageClient,
    filename: String,
    payload: Value,
    block_size: u64,
}

impl StoragePayloadUploadRequest {
    pub(crate) fn parse_request(request: &NodeOperationRequest) -> Result<Self> {
        let payload: PayloadUploadPayload = request.payload("storage payload upload")?;
        Ok(Self {
            client: StorageClient::from_initialization(request.adapter())?,
            filename: required_text(payload.filename, "payload filename")?,
            payload: payload.payload,
            block_size: payload.block_size.max(1),
        })
    }

    pub(crate) fn client(&self) -> &StorageClient {
        &self.client
    }

    pub(crate) fn filename(&self) -> &str {
        &self.filename
    }

    pub(crate) fn payload(&self) -> &Value {
        &self.payload
    }

    pub(crate) const fn block_size(&self) -> u64 {
        self.block_size
    }
}

#[derive(Debug, Deserialize)]
struct RestorePayload {
    cid: String,
    #[serde(default)]
    local_only: bool,
}

pub(crate) struct StorageBackupDownloadRequest {
    client: StorageClient,
    cid: String,
    local_only: bool,
}

impl StorageBackupDownloadRequest {
    pub(crate) fn parse_request(request: &NodeOperationRequest) -> Result<Self> {
        let payload: RestorePayload = request.payload("settings backup download")?;
        let cid = parse_backup_cid(payload.cid)?;
        Ok(Self {
            client: StorageClient::from_initialization(request.adapter())?,
            cid,
            local_only: payload.local_only,
        })
    }

    pub(crate) fn client(&self) -> &StorageClient {
        &self.client
    }

    pub(crate) fn cid(&self) -> &str {
        &self.cid
    }

    pub(crate) const fn local_only(&self) -> bool {
        self.local_only
    }

    pub(crate) const fn download_scope(&self) -> &'static str {
        if self.local_only { "local" } else { "network" }
    }
}

fn required_text(value: String, label: &str) -> Result<String> {
    let value = value.trim();
    if value.is_empty() {
        bail!("{label} is required")
    }
    Ok(value.to_owned())
}

fn context_map(values: &[(&'static str, String)]) -> Map<String, Value> {
    values
        .iter()
        .map(|(key, value)| ((*key).to_owned(), json!(value)))
        .collect()
}

fn value_string(values: &[Value], index: usize) -> String {
    values
        .get(index)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

const fn default_block_size() -> u64 {
    DEFAULT_BLOCK_SIZE
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        fs,
        sync::{Arc, Mutex, atomic::AtomicU8},
    };

    use anyhow::Result;
    use serde_json::json;
    #[cfg(unix)]
    use tokio::sync::Mutex as AsyncMutex;

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport,
    };

    #[cfg(unix)]
    static TEST_CLI_UPLOAD_LOCK: AsyncMutex<()> = AsyncMutex::const_new(());

    #[cfg(unix)]
    async fn serialize_cli_upload_test() -> tokio::sync::MutexGuard<'static, ()> {
        TEST_CLI_UPLOAD_LOCK.lock().await
    }

    struct ManifestPollTransport {
        kind: ModuleTransportKind,
        calls: Mutex<Vec<String>>,
        manifest_replies: Mutex<VecDeque<Value>>,
    }

    impl ManifestPollTransport {
        fn new(kind: ModuleTransportKind, manifest_replies: Vec<Value>) -> Self {
            Self {
                kind,
                calls: Mutex::new(Vec::new()),
                manifest_replies: Mutex::new(manifest_replies.into()),
            }
        }

        fn calls(&self) -> Result<Vec<String>> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|_| anyhow::anyhow!("manifest poll calls lock failed"))
        }
    }

    impl ModuleTransport for ManifestPollTransport {
        fn kind(&self) -> ModuleTransportKind {
            self.kind
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let result = (|| {
                anyhow::ensure!(call.module() == super::super::layer::module_id());
                self.calls
                    .lock()
                    .map_err(|_| anyhow::anyhow!("manifest poll calls lock failed"))?
                    .push(call.method().to_owned());
                let value = match call.method() {
                    "downloadManifest" => Value::Null,
                    "manifests" => self
                        .manifest_replies
                        .lock()
                        .map_err(|_| anyhow::anyhow!("manifest replies lock failed"))?
                        .pop_front()
                        .context("unexpected extra manifest poll")?,
                    method => anyhow::bail!("unexpected Storage method `{method}`"),
                };
                Ok(ModuleCallReply::new(self.kind, value))
            })();
            Box::pin(async move { result })
        }
    }

    struct UploadRecordingTransport {
        kind: ModuleTransportKind,
        calls: Mutex<Vec<(String, Vec<Value>)>>,
    }

    impl UploadRecordingTransport {
        fn new(kind: ModuleTransportKind) -> Self {
            Self {
                kind,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Result<Vec<(String, Vec<Value>)>> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|_| anyhow::anyhow!("upload recording calls lock failed"))
        }
    }

    impl ModuleTransport for UploadRecordingTransport {
        fn kind(&self) -> ModuleTransportKind {
            self.kind
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let result = (|| {
                anyhow::ensure!(call.module() == super::super::layer::module_id());
                self.calls
                    .lock()
                    .map_err(|_| anyhow::anyhow!("upload recording calls lock failed"))?
                    .push((call.method().to_owned(), call.args().to_vec()));
                let value = match call.method() {
                    "uploadUrl" => json!("session-upload-1"),
                    method => anyhow::bail!("unexpected Storage method `{method}`"),
                };
                Ok(ModuleCallReply::new(self.kind, value))
            })();
            Box::pin(async move { result })
        }
    }

    #[cfg(unix)]
    struct FakeUploadRuntime {
        _directory: tempfile::TempDir,
        transport: SharedModuleTransport,
        calls_path: std::path::PathBuf,
        upload_count_path: std::path::PathBuf,
        upload_path: std::path::PathBuf,
    }

    #[cfg(unix)]
    impl FakeUploadRuntime {
        fn new(mode: &str) -> Result<Self> {
            use std::os::unix::fs::PermissionsExt as _;

            let directory = tempfile::tempdir()?;
            let root = directory.path();
            let program = root.join("logoscore-upload-fixture");
            let config_dir = root.join("config");
            fs::create_dir_all(&config_dir)?;
            fs::write(config_dir.join("mode"), mode)?;
            fs::write(
                &program,
                r#"#!/bin/sh
if [ "$1" != "--config-dir" ]; then exit 90; fi
state="$2"
shift 2
mode="$(cat "$state/mode")"
case "$1" in
  watch)
    if [ "$mode" = "watch_failure" ]; then
      printf '%s\n' 'watch protocol unsupported' >&2
      exit 2
    fi
    if [ "$mode" = "watch_hang" ]; then
      while :; do sleep 1; done
    fi
    printf '%s\n' '{"type":"subscription_ready","protocol":"logoscore.watch","version":1,"module":"storage_module","event":"storageUploadDone"}'
    while [ ! -f "$state/dispatched" ]; do sleep 0.01; done
    if [ "$mode" = "foreign_success" ]; then
      printf '%s\n' '{"type":"event","protocol":"logoscore.watch","version":1,"timestamp":"2026-07-16T12:00:00Z","module":"storage_module","event":"storageUploadDone","data":{"arg0":"{\"success\":true,\"sessionId\":\"session-foreign\",\"cid\":\"cid-foreign\"}"}}'
    fi
    case "$mode" in
      success|foreign_success)
        printf '%s\n' '{"type":"event","protocol":"logoscore.watch","version":1,"timestamp":"2026-07-16T12:00:01Z","module":"storage_module","event":"storageUploadDone","data":{"arg0":"{\"success\":true,\"sessionId\":\"session-upload-1\",\"cid\":\"cid-upload-1\"}"}}'
        ;;
      terminal_failure)
        printf '%s\n' '{"type":"event","protocol":"logoscore.watch","version":1,"timestamp":"2026-07-16T12:00:01Z","module":"storage_module","event":"storageUploadDone","data":{"arg0":"{\"success\":false,\"sessionId\":\"session-upload-1\",\"error\":\"fixture upload failed\"}"}}'
        ;;
      malformed_terminal)
        printf '%s\n' '{"type":"event","protocol":"logoscore.watch","version":1,"timestamp":"2026-07-16T12:00:01Z","module":"storage_module","event":"storageUploadDone","data":{"arg0":"{\"success\":true,\"sessionId\":\"session-upload-1\"}"}}'
        ;;
    esac
    while :; do sleep 1; done
    ;;
  call)
    printf '%s\n' "$3" >> "$state/calls"
    case "$3" in
      manifests)
        if [ "$mode" = "barrier_failure" ]; then exit 7; fi
        if [ "$mode" = "barrier_hang" ]; then
          while :; do sleep 1; done
        fi
        printf '%s\n' '{"status":"ok","result":{"success":true,"value":[],"error":null}}'
        ;;
      uploadUrl)
        printf x >> "$state/upload-count"
        touch "$state/dispatched"
        if [ "$mode" = "dispatch_failure" ]; then exit 8; fi
        printf '%s\n' '{"status":"ok","result":{"success":true,"value":"session-upload-1","error":null}}'
        ;;
      *) exit 91 ;;
    esac
    ;;
  *) exit 92 ;;
esac
"#,
            )?;
            let mut permissions = fs::metadata(&program)?.permissions();
            permissions.set_mode(0o700);
            fs::set_permissions(&program, permissions)?;
            let upload_path = root.join("upload.bin");
            fs::write(&upload_path, b"fixture upload")?;
            let transport: SharedModuleTransport =
                Arc::new(crate::modules::logos_core::LogoscoreCliTransport::managed(
                    program.display().to_string(),
                    config_dir.display().to_string(),
                ));
            Ok(Self {
                _directory: directory,
                transport,
                calls_path: config_dir.join("calls"),
                upload_count_path: config_dir.join("upload-count"),
                upload_path,
            })
        }

        fn request(&self) -> Result<StorageOperationRequest> {
            let path = self
                .upload_path
                .to_str()
                .context("fake upload path is not UTF-8")?;
            let request = request(json!({
                "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
                "mutating_enabled": true,
                "payload": { "path": path }
            }))?;
            StorageOperationRequest::parse(&request, StorageOperation::Upload)
        }

        fn transport(&self) -> SharedModuleTransport {
            self.transport.clone()
        }

        fn calls(&self) -> Result<Vec<String>> {
            match fs::read_to_string(&self.calls_path) {
                Ok(calls) => Ok(calls.lines().map(ToOwned::to_owned).collect()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
                Err(error) => Err(error.into()),
            }
        }

        fn upload_count(&self) -> Result<usize> {
            match fs::read(&self.upload_count_path) {
                Ok(count) => Ok(count.len()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
                Err(error) => Err(error.into()),
            }
        }
    }

    struct CacheDispatchTransport {
        kind: ModuleTransportKind,
        calls: Mutex<Vec<(String, Vec<Value>)>>,
    }

    impl CacheDispatchTransport {
        fn new(kind: ModuleTransportKind) -> Self {
            Self {
                kind,
                calls: Mutex::new(Vec::new()),
            }
        }

        fn calls(&self) -> Result<Vec<(String, Vec<Value>)>> {
            self.calls
                .lock()
                .map(|calls| calls.clone())
                .map_err(|_| anyhow::anyhow!("cache dispatch calls lock failed"))
        }
    }

    impl ModuleTransport for CacheDispatchTransport {
        fn kind(&self) -> ModuleTransportKind {
            self.kind
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            let result = (|| {
                anyhow::ensure!(call.module() == super::super::layer::module_id());
                self.calls
                    .lock()
                    .map_err(|_| anyhow::anyhow!("cache dispatch calls lock failed"))?
                    .push((call.method().to_owned(), call.args().to_vec()));
                anyhow::ensure!(call.method() == "fetch", "unexpected Storage method");
                Ok(ModuleCallReply::new(self.kind, Value::Null))
            })();
            Box::pin(async move { result })
        }
    }

    fn request(value: Value) -> Result<NodeOperationRequest> {
        NodeOperationRequest::from_value(&value)
    }

    fn command_control() -> Result<CommandControl> {
        let deadline = std::time::Instant::now()
            .checked_add(std::time::Duration::from_secs(30))
            .context("storage test deadline overflow")?;
        Ok(CommandControl::new(
            tokio_util::sync::CancellationToken::new(),
            deadline,
        ))
    }

    fn module_call_control(duration: Duration) -> ModuleCallControl {
        ModuleCallControl::new(
            tokio_util::sync::CancellationToken::new(),
            tokio::time::Instant::now() + duration,
            Arc::new(AtomicU8::new(0)),
        )
    }

    #[test]
    fn rest_download_plan_owns_transport_inputs() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": true,
            "payload": {
                "cid": "cid-a",
                "path": "/tmp/a",
                "local_only": true
            }
        }))?;

        let request = StorageOperationRequest::parse(&request, StorageOperation::Download)?;

        let expected =
            StorageOperationPlan::Rest(StorageRestOperation::Download(StorageDownloadRequest {
                endpoint: "http://storage".to_owned(),
                cid: "cid-a".to_owned(),
                path: "/tmp/a".to_owned(),
                local_only: true,
            }));
        anyhow::ensure!(
            request.plan == expected,
            "unexpected Storage download plan: {:?}",
            request.plan
        );
        Ok(())
    }

    #[test]
    fn module_upload_plan_supplies_module_default() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/a" }
        }))?;

        let request = StorageOperationRequest::parse(&request, StorageOperation::Upload)?;

        let expected = StorageOperationPlan::Module {
            transport: ModuleTransportKind::Module,
            method: "uploadUrl",
            args: vec![json!("/tmp/a"), json!(DEFAULT_BLOCK_SIZE)],
            context: vec![("path", "/tmp/a".to_owned())],
            dispatch: true,
        };
        anyhow::ensure!(request.plan == expected, "unexpected Storage upload plan");
        Ok(())
    }

    #[test]
    fn logoscore_cli_upload_plan_preserves_cli_transport() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/a" }
        }))?;

        let request = StorageOperationRequest::parse(&request, StorageOperation::Upload)?;

        anyhow::ensure!(
            matches!(
                request.plan,
                StorageOperationPlan::Module {
                    transport: ModuleTransportKind::LogoscoreCli,
                    ..
                }
            ),
            "Storage LogosCore CLI plan lost transport identity"
        );
        Ok(())
    }

    #[test]
    fn backup_upload_request_preserves_typed_identity_and_block_size() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": {
                "backup_catalog_id": "backup-1",
                "block_size": 32768
            }
        }))?;

        let request = StorageBackupUploadRequest::parse_request(&request)?;

        anyhow::ensure!(
            request.backup_catalog_id() == "backup-1"
                && request.block_size() == 32_768
                && request.client().source() == "logoscore call storage_module",
            "backup upload request lost typed input identity"
        );
        Ok(())
    }

    #[test]
    fn backup_download_request_preserves_typed_rest_identity_and_scope() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": false,
            "payload": { "cid": " cid-1 ", "local_only": true }
        }))?;

        let request = StorageBackupDownloadRequest::parse_request(&request)?;

        anyhow::ensure!(
            request.cid() == "cid-1"
                && request.local_only()
                && request.download_scope() == "local"
                && request.client().endpoint() == Some("http://storage")
                && request.client().source() == "http://storage",
            "backup download request lost typed REST identity"
        );
        Ok(())
    }

    #[test]
    fn backup_download_request_rejects_blank_cid_and_metrics_source() -> Result<()> {
        let blank = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": false,
            "payload": { "cid": "  " }
        }))?;
        let metrics = request(json!({
            "adapter": {
                "source_mode": "metrics",
                "inputs": { "metrics_endpoint": "http://metrics" }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-1" }
        }))?;

        anyhow::ensure!(
            StorageBackupDownloadRequest::parse_request(&blank)
                .err()
                .is_some_and(|error| error.to_string().contains("backup CID is required")),
            "blank backup CID was accepted"
        );
        anyhow::ensure!(
            StorageBackupDownloadRequest::parse_request(&metrics)
                .err()
                .is_some_and(|error| error.to_string().contains(
                    "Storage data actions require storage REST or module source, not metrics"
                )),
            "metrics source was accepted for backup download"
        );
        Ok(())
    }

    #[test]
    fn backup_download_request_rejects_route_breaking_and_oversized_cids() -> Result<()> {
        let invalid_cids = [
            ".",
            "..",
            "cid/child",
            "cid\\child",
            "cid?query",
            "cid#fragment",
            "cid%2fchild",
            "cid%5Cchild",
            "cid%00tail",
            "cid\ncontrol",
            "cid with space",
        ];
        for cid in invalid_cids {
            let request = request(json!({
                "adapter": {
                    "source_mode": "rest",
                    "inputs": { "rest_endpoint": "http://storage" }
                },
                "mutating_enabled": false,
                "payload": { "cid": cid }
            }))?;
            let error = StorageBackupDownloadRequest::parse_request(&request)
                .err()
                .with_context(|| format!("route-breaking backup CID `{cid:?}` was accepted"))?;
            anyhow::ensure!(
                error.to_string().contains("backup CID"),
                "route-breaking backup CID returned unrelated error: {error:#}"
            );
        }

        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": false,
            "payload": { "cid": "a".repeat(BACKUP_CID_MAX_BYTES + 1) }
        }))?;
        let error = StorageBackupDownloadRequest::parse_request(&request)
            .err()
            .context("oversized backup CID was accepted")?;
        anyhow::ensure!(
            error
                .to_string()
                .contains("backup CID exceeds 256 byte limit"),
            "oversized backup CID returned unexpected error: {error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_backup_download_fails_without_host_file_staging() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": false,
            "payload": { "cid": "cid-1" }
        }))?;
        let request = StorageBackupDownloadRequest::parse_request(&request)?;
        let module_transport: crate::modules::logos_core::SharedModuleTransport = Arc::new(
            crate::modules::logos_core::UnavailableModuleTransport::basecamp_host_not_configured(),
        );
        let error = request
            .client()
            .download_backup_bytes_controlled(
                &module_transport,
                &module_transport,
                request.cid(),
                request.local_only(),
                command_control()?,
            )
            .await
            .err()
            .context("Basecamp backup download should fail")?;

        anyhow::ensure!(
            error.to_string() == "Basecamp host transport does not provide shared file staging",
            "adapter identity collapsed: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn payload_upload_request_preserves_typed_payload_and_filename() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": true,
            "payload": {
                "filename": "shared-idl.json",
                "payload": { "kind": "shared-idl" },
                "block_size": 32768
            }
        }))?;

        let request = StoragePayloadUploadRequest::parse_request(&request)?;

        anyhow::ensure!(
            request.filename() == "shared-idl.json"
                && request.payload() == &json!({ "kind": "shared-idl" })
                && request.block_size() == 32_768
                && request.client().source() == "http://storage",
            "payload upload request lost typed input identity"
        );
        Ok(())
    }

    #[test]
    fn payload_upload_request_enables_legacy_mutating_flag() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": false,
            "payload": {
                "filename": "shared-idl.json",
                "payload": { "kind": "shared-idl" }
            }
        }))?;

        let parsed = StoragePayloadUploadRequest::parse_request(&request)?;

        anyhow::ensure!(
            parsed.filename() == "shared-idl.json",
            "legacy mutation flag prevented payload upload parsing"
        );
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_managed_byte_upload_fails_closed_before_transport() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": {
                "backup_catalog_id": "backup-1",
                "block_size": 65536
            }
        }))?;
        let request = StorageBackupUploadRequest::parse_request(&request)?;
        let module_transport: SharedModuleTransport = Arc::new(
            crate::modules::logos_core::UnavailableModuleTransport::basecamp_host_not_configured(),
        );

        let error = request
            .client()
            .upload_bytes_controlled(
                &module_transport,
                "backup.json",
                b"payload",
                65_536,
                command_control()?,
            )
            .await
            .err()
            .ok_or_else(|| anyhow::anyhow!("Basecamp managed byte upload should fail"))?;

        anyhow::ensure!(
            error.to_string()
                == "Basecamp module source does not support Inspector-managed byte staging",
            "unexpected Basecamp byte-staging error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn module_fetch_plan_is_an_unobservable_dispatch() -> Result<()> {
        let request = request(json!({
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "cid": "cid-a" }
        }))?;

        let request = StorageOperationRequest::parse(&request, StorageOperation::Fetch)?;

        let expected = StorageOperationPlan::Module {
            transport: ModuleTransportKind::Module,
            method: "fetch",
            args: vec![json!("cid-a")],
            context: vec![("cid", "cid-a".to_owned())],
            dispatch: true,
        };
        anyhow::ensure!(request.plan == expected, "unexpected Storage fetch plan");
        Ok(())
    }

    #[tokio::test]
    async fn module_cache_returns_one_unobservable_dispatch_receipt() -> Result<()> {
        for (source_mode, kind) in [
            ("module", ModuleTransportKind::Module),
            ("logoscore_cli", ModuleTransportKind::LogoscoreCli),
        ] {
            let transport = Arc::new(CacheDispatchTransport::new(kind));
            let shared: SharedModuleTransport = transport.clone();
            let request = request(json!({
                "adapter": { "source_mode": source_mode, "inputs": {} },
                "mutating_enabled": true,
                "payload": { "cid": "cid-cache" }
            }))?;
            let request = StorageOperationRequest::parse(&request, StorageOperation::Fetch)?;

            let output = execute_operation(
                request,
                shared,
                module_call_control(Duration::from_secs(30)),
            )
            .await?;

            anyhow::ensure!(
                matches!(
                    output,
                    StorageOperationOutput::Outcome(NodeOperationOutcome::Dispatched(value))
                        if value == json!({
                            "adapter": kind,
                            "cid": "cid-cache",
                            "dispatched": true,
                            "method": "fetch",
                            "module": "storage_module",
                            "value": null
                        })
                ),
                "Storage Cache did not return its exact dispatch acknowledgement"
            );
            anyhow::ensure!(
                transport.calls()? == [("fetch".to_owned(), vec![json!("cid-cache")])],
                "Storage Cache issued calls beyond one exact-CID fetch"
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn logoscore_cli_upload_completes_from_exact_terminal_session() -> Result<()> {
        let _test_permit = serialize_cli_upload_test().await;
        let fake = FakeUploadRuntime::new("success")?;
        let output = execute_operation(
            fake.request()?,
            fake.transport(),
            module_call_control(Duration::from_secs(5)),
        )
        .await?;

        anyhow::ensure!(
            matches!(
                output,
                StorageOperationOutput::Outcome(NodeOperationOutcome::Completed(value))
                    if value.get("success") == Some(&json!(true))
                        && value.get("sessionId") == Some(&json!("session-upload-1"))
                        && value.get("cid") == Some(&json!("cid-upload-1"))
                        && value.get("completion") == Some(&json!("storageUploadDone"))
                        && value.get("path")
                            == fake.upload_path.to_str().map(Value::from).as_ref()
            ),
            "CLI upload lost exact terminal event result"
        );
        anyhow::ensure!(
            fake.calls()? == ["manifests", "uploadUrl"] && fake.upload_count()? == 1,
            "CLI upload lost source barrier or exactly-once dispatch"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn logoscore_cli_upload_ignores_foreign_terminal_session() -> Result<()> {
        let _test_permit = serialize_cli_upload_test().await;
        let fake = FakeUploadRuntime::new("foreign_success")?;
        let output = execute_operation(
            fake.request()?,
            fake.transport(),
            module_call_control(Duration::from_secs(5)),
        )
        .await?;

        anyhow::ensure!(
            matches!(
                output,
                StorageOperationOutput::Outcome(NodeOperationOutcome::Completed(value))
                    if value.get("cid") == Some(&json!("cid-upload-1"))
                        && value.get("sessionId") == Some(&json!("session-upload-1"))
            ),
            "foreign upload terminal event captured the active operation"
        );
        anyhow::ensure!(fake.upload_count()? == 1, "CLI upload was retried");
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn logoscore_cli_upload_exact_failure_is_terminal() -> Result<()> {
        let _test_permit = serialize_cli_upload_test().await;
        let fake = FakeUploadRuntime::new("terminal_failure")?;
        let error = execute_operation(
            fake.request()?,
            fake.transport(),
            module_call_control(Duration::from_secs(5)),
        )
        .await
        .err()
        .context("failed upload terminal event should fail the operation")?;

        anyhow::ensure!(
            error.to_string().contains("fixture upload failed")
                && error
                    .downcast_ref::<StorageUploadSettlementUnconfirmed>()
                    .is_none(),
            "authoritative upload failure was treated as unsettled: {error:#}"
        );
        anyhow::ensure!(fake.upload_count()? == 1, "failed CLI upload was retried");
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn logoscore_cli_upload_preflight_failures_never_dispatch() -> Result<()> {
        let _test_permit = serialize_cli_upload_test().await;
        for (mode, expected_calls) in [
            ("watch_failure", Vec::<String>::new()),
            ("barrier_failure", vec!["manifests".to_owned()]),
        ] {
            let fake = FakeUploadRuntime::new(mode)?;
            let error = execute_operation(
                fake.request()?,
                fake.transport(),
                module_call_control(Duration::from_secs(5)),
            )
            .await
            .err()
            .with_context(|| format!("{mode} should fail before upload dispatch"))?;
            anyhow::ensure!(
                error
                    .downcast_ref::<StorageUploadSettlementUnconfirmed>()
                    .is_none(),
                "pre-dispatch {mode} failure retained the upload lease: {error:#}"
            );
            anyhow::ensure!(
                fake.calls()? == expected_calls && fake.upload_count()? == 0,
                "pre-dispatch {mode} failure reached uploadUrl"
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn logoscore_cli_upload_predispatch_stops_are_confirmed_not_started() -> Result<()> {
        let _test_permit = serialize_cli_upload_test().await;
        let watch = FakeUploadRuntime::new("watch_hang")?;
        let watch_error = execute_operation(
            watch.request()?,
            watch.transport(),
            module_call_control(Duration::from_millis(100)),
        )
        .await
        .err()
        .context("watch_hang should stop before upload dispatch")?;
        let watch_terminated = watch_error
            .downcast_ref::<ModuleCallTerminated>()
            .context("watch_hang did not preserve module stop evidence")?;
        anyhow::ensure!(
            watch_terminated.reason() == ModuleCallStopReason::DeadlineExceeded
                && watch_terminated.evidence() == ModuleCallTerminationEvidence::NotStarted,
            "watch_hang stop evidence drifted: {watch_terminated}"
        );
        anyhow::ensure!(
            watch.calls()?.is_empty() && watch.upload_count()? == 0,
            "watch_hang reached a Storage call"
        );

        let barrier = FakeUploadRuntime::new("barrier_hang")?;
        let cancellation = tokio_util::sync::CancellationToken::new();
        let control = ModuleCallControl::new(
            cancellation.clone(),
            tokio::time::Instant::now() + Duration::from_secs(5),
            Arc::new(AtomicU8::new(3)),
        );
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            cancellation.cancel();
        });
        let barrier_error = execute_operation(barrier.request()?, barrier.transport(), control)
            .await
            .err()
            .context("barrier_hang should stop before upload dispatch")?;
        let barrier_terminated = barrier_error
            .downcast_ref::<ModuleCallTerminated>()
            .context("barrier_hang did not preserve module stop evidence")?;
        anyhow::ensure!(
            barrier_terminated.reason() == ModuleCallStopReason::Shutdown
                && barrier_terminated.evidence() == ModuleCallTerminationEvidence::NotStarted,
            "barrier_hang stop evidence drifted: {barrier_terminated}"
        );
        anyhow::ensure!(
            barrier.calls()? == ["manifests".to_owned()] && barrier.upload_count()? == 0,
            "barrier_hang did not stop inside the ordering barrier"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn logoscore_cli_upload_dispatch_failure_retains_uncertain_settlement() -> Result<()> {
        let _test_permit = serialize_cli_upload_test().await;
        let fake = FakeUploadRuntime::new("dispatch_failure")?;
        let error = execute_operation(
            fake.request()?,
            fake.transport(),
            module_call_control(Duration::from_secs(5)),
        )
        .await
        .err()
        .context("failed upload dispatch should preserve uncertainty")?;

        anyhow::ensure!(
            error
                .downcast_ref::<StorageUploadSettlementUnconfirmed>()
                .is_some(),
            "post-spawn upload failure released uncertain settlement: {error:#}"
        );
        anyhow::ensure!(
            fake.upload_count()? == 1,
            "failed dispatch retried uploadUrl"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn logoscore_cli_upload_deadline_retains_uncertain_settlement() -> Result<()> {
        let _test_permit = serialize_cli_upload_test().await;
        let fake = FakeUploadRuntime::new("no_terminal")?;
        let error = execute_operation(
            fake.request()?,
            fake.transport(),
            module_call_control(Duration::from_secs(5)),
        )
        .await
        .err()
        .context("unsettled upload should reach its deadline")?;

        anyhow::ensure!(
            error
                .downcast_ref::<StorageUploadSettlementUnconfirmed>()
                .is_some(),
            "post-dispatch deadline released uncertain settlement: {error:#}"
        );
        anyhow::ensure!(fake.upload_count()? == 1, "timed-out upload was retried");
        Ok(())
    }

    #[tokio::test]
    async fn logoscore_cli_upload_rejects_relative_path_before_transport() -> Result<()> {
        let transport = Arc::new(UploadRecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
        ));
        let shared: SharedModuleTransport = transport.clone();
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "relative-upload.bin" }
        }))?;
        let request = StorageOperationRequest::parse(&request, StorageOperation::Upload)?;

        let error = execute_operation(request, shared, module_call_control(Duration::from_secs(5)))
            .await
            .err()
            .context("relative upload path should fail before transport")?;
        anyhow::ensure!(
            error.to_string() == "storage upload file path must be absolute"
                && transport.calls()?.is_empty(),
            "relative upload path reached transport: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn upload_terminal_decoder_accepts_object_payload_and_rejects_missing_cid() -> Result<()> {
        let success = json!({
            "module": "storage_module",
            "event": "storageUploadDone",
            "data": {
                "arg0": {
                    "success": true,
                    "sessionId": "session-upload-1",
                    "cid": "cid-upload-1"
                }
            }
        });
        anyhow::ensure!(
            decode_upload_terminal_event(&success, "session-upload-1")?
                == UploadTerminalEvent::Succeeded {
                    cid: "cid-upload-1".to_owned()
                },
            "object upload terminal payload was not decoded"
        );
        let malformed = json!({
            "module": "storage_module",
            "event": "storageUploadDone",
            "data": { "arg0": "{\"success\":true,\"sessionId\":\"session-upload-1\"}" }
        });
        anyhow::ensure!(
            decode_upload_terminal_event(&malformed, "session-upload-1")
                .is_err_and(|error| error.to_string().contains("has no CID")),
            "successful upload terminal without CID was accepted"
        );
        Ok(())
    }

    #[tokio::test]
    async fn basecamp_upload_keeps_native_terminal_event_contract() -> Result<()> {
        let transport = Arc::new(UploadRecordingTransport::new(ModuleTransportKind::Module));
        let shared: SharedModuleTransport = transport.clone();
        let request = request(json!({
            "adapter": { "source_mode": "module", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "path": "/tmp/basecamp-upload.bin" }
        }))?;
        let request = StorageOperationRequest::parse(&request, StorageOperation::Upload)?;

        let output = execute_operation(
            request,
            shared,
            module_call_control(Duration::from_secs(30)),
        )
        .await?;

        anyhow::ensure!(
            matches!(
                output,
                StorageOperationOutput::Outcome(NodeOperationOutcome::Accepted(acceptance))
                    if acceptance
                        .correlation()
                        .session_id()
                        .map(|session| session.as_str())
                        == Some("session-upload-1")
                        && acceptance.terminal_event().success_event() == "storageUploadDone"
            ),
            "Basecamp upload lost native event correlation"
        );
        anyhow::ensure!(
            transport.calls()?
                == [(
                    "uploadUrl".to_owned(),
                    vec![json!("/tmp/basecamp-upload.bin"), json!(DEFAULT_BLOCK_SIZE)]
                )],
            "Basecamp upload unexpectedly changed its native dispatch"
        );
        Ok(())
    }

    #[tokio::test]
    async fn module_manifest_fetch_polls_until_exact_manifest_is_visible() -> Result<()> {
        let manifest = json!({
            "cid": "cid-a",
            "treeCid": "tree-a",
            "datasetSize": 42,
            "blockSize": 65_536,
            "filename": "a.json",
            "mimetype": "application/json"
        });
        for (source_mode, kind) in [
            ("module", ModuleTransportKind::Module),
            ("logoscore_cli", ModuleTransportKind::LogoscoreCli),
        ] {
            let transport = Arc::new(ManifestPollTransport::new(
                kind,
                vec![
                    json!([{ "cid": "cid-a-near-match" }]),
                    json!([manifest.clone()]),
                ],
            ));
            let shared: SharedModuleTransport = transport.clone();
            let request = request(json!({
                "adapter": { "source_mode": source_mode, "inputs": {} },
                "mutating_enabled": true,
                "payload": { "cid": "cid-a" }
            }))?;
            let request =
                StorageOperationRequest::parse(&request, StorageOperation::DownloadManifest)?;

            let output = execute_operation(
                request,
                shared,
                module_call_control(Duration::from_secs(30)),
            )
            .await?;

            let StorageOperationOutput::Outcome(NodeOperationOutcome::Completed(actual)) = output
            else {
                anyhow::bail!("Storage manifest fetch did not complete with a manifest");
            };
            anyhow::ensure!(actual == manifest, "fetched manifest payload drifted");
            anyhow::ensure!(
                transport.calls()? == ["downloadManifest", "manifests", "manifests"],
                "manifest fetch did not poll after dispatch"
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn module_manifest_fetch_dispatches_before_returning_exact_manifest() -> Result<()> {
        let manifest = json!({
            "cid": "cid-local",
            "treeCid": "tree-local",
            "datasetSize": 7,
            "blockSize": 65_536,
            "filename": "local.bin",
            "mimetype": "application/octet-stream"
        });
        let transport = Arc::new(ManifestPollTransport::new(
            ModuleTransportKind::LogoscoreCli,
            vec![json!([manifest.clone()])],
        ));
        let shared: SharedModuleTransport = transport.clone();
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "cid": "cid-local" }
        }))?;
        let request = StorageOperationRequest::parse(&request, StorageOperation::DownloadManifest)?;

        let output = execute_operation(
            request,
            shared,
            module_call_control(Duration::from_secs(30)),
        )
        .await?;

        anyhow::ensure!(
            matches!(
                output,
                StorageOperationOutput::Outcome(NodeOperationOutcome::Completed(value))
                    if value == manifest
            ),
            "preexisting manifest was not returned"
        );
        anyhow::ensure!(
            transport.calls()? == ["downloadManifest", "manifests"],
            "manifest fetch did not dispatch before reading its result"
        );
        Ok(())
    }

    #[tokio::test]
    async fn module_manifest_fetch_stops_at_operation_deadline() -> Result<()> {
        let transport = Arc::new(ManifestPollTransport::new(
            ModuleTransportKind::LogoscoreCli,
            vec![json!([])],
        ));
        let shared: SharedModuleTransport = transport.clone();
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "cid": "cid-missing" }
        }))?;
        let request = StorageOperationRequest::parse(&request, StorageOperation::DownloadManifest)?;
        let control = module_call_control(Duration::from_millis(20));

        let error = execute_operation(request, shared, control)
            .await
            .err()
            .context("missing manifest should reach the operation deadline")?;

        let terminated = error
            .downcast_ref::<ModuleCallTerminated>()
            .context("manifest timeout lost module-call interruption type")?;
        anyhow::ensure!(
            terminated.reason() == ModuleCallStopReason::DeadlineExceeded
                && terminated.evidence() == ModuleCallTerminationEvidence::LocallyAbandoned,
            "manifest timeout lost local-only deadline evidence: {terminated:?}"
        );
        anyhow::ensure!(
            transport.calls()? == ["downloadManifest", "manifests"],
            "manifest deadline path issued unexpected calls"
        );
        Ok(())
    }

    #[tokio::test]
    async fn module_manifest_fetch_rejects_incomplete_exact_row() -> Result<()> {
        let transport = Arc::new(ManifestPollTransport::new(
            ModuleTransportKind::LogoscoreCli,
            vec![json!([{
                "cid": "cid-malformed",
                "datasetSize": 1,
                "blockSize": 65_536,
                "filename": "bad.bin",
                "mimetype": "application/octet-stream"
            }])],
        ));
        let shared: SharedModuleTransport = transport.clone();
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "cid": "cid-malformed" }
        }))?;
        let request = StorageOperationRequest::parse(&request, StorageOperation::DownloadManifest)?;

        let error = execute_operation(
            request,
            shared,
            module_call_control(Duration::from_secs(30)),
        )
        .await
        .err()
        .context("incomplete manifest row was accepted")?;

        anyhow::ensure!(
            error.to_string().contains("has no `treeCid`"),
            "incomplete manifest returned unrelated error: {error:#}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn module_manifest_fetch_rejects_conflicting_exact_rows() -> Result<()> {
        let first = json!({
            "cid": "cid-conflict",
            "treeCid": "tree-a",
            "datasetSize": 1,
            "blockSize": 65_536,
            "filename": "a.bin",
            "mimetype": "application/octet-stream"
        });
        let mut second = first.clone();
        *second
            .get_mut("filename")
            .context("test manifest has no filename")? = json!("b.bin");
        let transport = Arc::new(ManifestPollTransport::new(
            ModuleTransportKind::LogoscoreCli,
            vec![json!([first, second])],
        ));
        let shared: SharedModuleTransport = transport;
        let request = request(json!({
            "adapter": { "source_mode": "logoscore_cli", "inputs": {} },
            "mutating_enabled": true,
            "payload": { "cid": "cid-conflict" }
        }))?;
        let request = StorageOperationRequest::parse(&request, StorageOperation::DownloadManifest)?;

        let error = execute_operation(
            request,
            shared,
            module_call_control(Duration::from_secs(30)),
        )
        .await
        .err()
        .context("conflicting manifest rows were accepted")?;

        anyhow::ensure!(
            error.to_string().contains("conflicting rows"),
            "conflicting manifests returned unrelated error: {error:#}"
        );
        Ok(())
    }

    #[test]
    fn module_dispatch_outcomes_keep_session_role_and_unobservable_actions_distinct() -> Result<()>
    {
        let upload = storage_module_dispatch_outcome(
            "uploadUrl",
            ModuleDispatchReceipt::new(
                json!({ "dispatched": true }),
                &json!("session-1"),
                ModuleDispatchIdentityRole::Session,
            )
            .with_bridge_callback(crate::source_routing::BridgeCallbackId::new(31)),
        )?;
        let NodeOperationOutcome::Accepted(acceptance) = upload else {
            anyhow::bail!("module upload was not accepted");
        };
        anyhow::ensure!(
            acceptance.correlation().session_id().map(|id| id.as_str()) == Some("session-1")
                && acceptance.correlation().request_id().is_none()
                && acceptance
                    .correlation()
                    .bridge_callback_id()
                    .map(crate::source_routing::BridgeCallbackId::value)
                    == Some(31)
                && acceptance.terminal_event().correlation()
                    == &ModuleEventCorrelationKind::Session,
            "module upload identity role drifted"
        );

        let manifest = storage_module_dispatch_outcome(
            "downloadManifest",
            ModuleDispatchReceipt::new(
                json!({ "dispatched": true }),
                &Value::Null,
                ModuleDispatchIdentityRole::None,
            ),
        )?;
        anyhow::ensure!(
            matches!(manifest, NodeOperationOutcome::Dispatched(_)),
            "reusable CID correlation was treated as operation-unique"
        );
        Ok(())
    }

    #[test]
    fn download_cid_is_not_accepted_as_unique_session() -> Result<()> {
        let outcome = storage_module_dispatch_outcome(
            "downloadToUrl",
            ModuleDispatchReceipt::new(
                json!({ "dispatched": true, "value": "cid-1" }),
                &json!("cid-1"),
                ModuleDispatchIdentityRole::None,
            ),
        )?;

        anyhow::ensure!(
            matches!(outcome, NodeOperationOutcome::Dispatched(_)),
            "download CID labelled as a session was treated as operation-unique"
        );
        Ok(())
    }

    #[test]
    fn observable_storage_dispatch_rejects_missing_session_identity() -> Result<()> {
        let Err(error) = storage_module_dispatch_outcome(
            "uploadUrl",
            ModuleDispatchReceipt::new(
                json!({ "dispatched": true }),
                &Value::Null,
                ModuleDispatchIdentityRole::Session,
            ),
        ) else {
            anyhow::bail!("observable storage dispatch accepted no correlation identity");
        };

        anyhow::ensure!(error.to_string() == "storage module `uploadUrl` returned no session ID");
        Ok(())
    }

    #[test]
    fn mutation_plan_enables_legacy_mutating_flag() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-a" }
        }))?;

        let parsed = StorageOperationRequest::parse(&request, StorageOperation::Remove)?;
        anyhow::ensure!(
            parsed.context().get("cid") == Some(&json!("cid-a")),
            "legacy mutation flag prevented Storage removal planning"
        );
        Ok(())
    }
}
