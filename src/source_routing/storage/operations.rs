use anyhow::{Context as _, Result, bail};
use reqwest::Response;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use std::time::Duration;

use crate::modules::logos_core::{ModuleTransportKind, SharedModuleTransport};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StorageOperation {
    Manifests,
    DownloadManifest,
    Fetch,
    Upload,
    Download,
    Remove,
}

impl StorageOperation {
    const fn mutating(self) -> bool {
        matches!(
            self,
            Self::Fetch | Self::Upload | Self::Download | Self::Remove
        )
    }

    const fn action_label(self) -> &'static str {
        match self {
            Self::Manifests => "storage manifests",
            Self::DownloadManifest => "storage manifest download",
            Self::Fetch => "storage network action",
            Self::Upload => "storage upload action",
            Self::Download => "storage download action",
            Self::Remove => "storage remove action",
        }
    }
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
                    .runtime();
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
                    .runtime();
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
        if operation.mutating() {
            request.require_mutating(operation.action_label())?;
        }
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
) -> Result<StorageOperationOutput> {
    execute_plan(request.plan, module_transport).await
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
            plan_for_client(client, "fetch", vec![json!(cid)], vec![("cid", cid)], false)
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
) -> Result<StorageOperationOutput> {
    let value = match plan {
        StorageOperationPlan::Module {
            transport: transport_kind,
            method,
            args,
            context,
            dispatch,
        } => {
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
        request.require_mutating("settings backup action")?;
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
        request.require_mutating("storage payload upload")?;
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
    use std::sync::Arc;

    use anyhow::Result;
    use serde_json::json;

    use super::*;

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
    fn payload_upload_request_requires_mutating_diagnostics() -> Result<()> {
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

        let error = StoragePayloadUploadRequest::parse_request(&request)
            .err()
            .context("disabled payload upload should fail")?;

        anyhow::ensure!(
            error
                .to_string()
                .contains("requires mutating diagnostics to be enabled"),
            "unexpected payload upload error: {error:#}"
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
    fn module_fetch_plan_is_an_observed_call() -> Result<()> {
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
            dispatch: false,
        };
        anyhow::ensure!(request.plan == expected, "unexpected Storage fetch plan");
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
    fn mutation_plan_rejects_disabled_diagnostics() -> Result<()> {
        let request = request(json!({
            "adapter": {
                "source_mode": "rest",
                "inputs": { "rest_endpoint": "http://storage" }
            },
            "mutating_enabled": false,
            "payload": { "cid": "cid-a" }
        }))?;

        let Err(error) = StorageOperationRequest::parse(&request, StorageOperation::Remove) else {
            anyhow::bail!("disabled Storage mutation was accepted");
        };
        anyhow::ensure!(
            error
                .to_string()
                .contains("requires mutating diagnostics to be enabled"),
            "unexpected Storage mutation error: {error:#}"
        );
        Ok(())
    }
}
