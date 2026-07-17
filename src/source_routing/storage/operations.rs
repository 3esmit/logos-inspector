use anyhow::{Context as _, Result, bail};
use reqwest::Response;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::time::Duration;

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
    args::Args, command_runner::CommandControl, settings_backup::SETTINGS_BACKUP_MAX_BYTES,
};

#[cfg(test)]
use super::BACKUP_CID_MAX_BYTES;
use super::{layer::STORAGE_SOURCE_MODES, parse_backup_cid, transport};

const DEFAULT_BLOCK_SIZE: u64 = 65_536;
const MANIFEST_POLL_INTERVAL: Duration = Duration::from_millis(100);

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
        sync::{Arc, Mutex, atomic::AtomicU8},
    };

    use anyhow::Result;
    use serde_json::json;

    use super::*;
    use crate::modules::logos_core::{
        ModuleCall, ModuleCallFuture, ModuleCallReply, ModuleTransport,
    };

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
