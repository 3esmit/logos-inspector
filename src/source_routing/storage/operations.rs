use anyhow::{Context as _, Result, bail};
use reqwest::Response;
use serde::Deserialize;
use serde_json::{Map, Value, json};

use crate::source_routing::{
    AdapterInitialization, ModuleCorrelation, ModuleDispatchIdentityRole, ModuleDispatchReceipt,
    ModuleEventCorrelationKind, ModuleTerminalEventContract, NodeOperationOutcome,
    NodeOperationRequest, ObservableOperationAcceptance, StorageSourceMode,
};
use crate::support::args::Args;

use super::{layer::STORAGE_SOURCE_MODES, transport};

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
    Module,
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
            StorageSourceMode::Module | StorageSourceMode::LogoscoreCli => {
                StorageOperationAdapter::Module
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

    pub(crate) fn endpoint(&self) -> Option<&str> {
        match &self.adapter {
            StorageOperationAdapter::Module => None,
            StorageOperationAdapter::Rest { endpoint } => Some(endpoint),
        }
    }

    pub(crate) fn source(&self) -> &str {
        match &self.adapter {
            StorageOperationAdapter::Module => "logoscore call storage_module",
            StorageOperationAdapter::Rest { endpoint } => endpoint,
        }
    }

    pub(crate) async fn exists(&self, cid: &str) -> Result<Value> {
        match &self.adapter {
            StorageOperationAdapter::Module => {
                transport::module_call("exists", vec![json!(cid)]).await
            }
            StorageOperationAdapter::Rest { endpoint } => transport::exists(endpoint, cid).await,
        }
    }

    pub(crate) async fn upload_bytes(
        &self,
        filename: &str,
        bytes: &[u8],
        block_size: u64,
    ) -> Result<Value> {
        match &self.adapter {
            StorageOperationAdapter::Module => {
                transport::module_upload_bytes(filename, bytes, block_size).await
            }
            StorageOperationAdapter::Rest { endpoint } => {
                transport::upload_bytes(endpoint, filename, bytes, block_size).await
            }
        }
    }

    pub(crate) async fn download_bytes(
        &self,
        cid: &str,
        local_only: bool,
        module_error: &str,
    ) -> Result<Vec<u8>> {
        match &self.adapter {
            StorageOperationAdapter::Module => bail!("{module_error}"),
            StorageOperationAdapter::Rest { endpoint } => {
                transport::download_bytes(endpoint, cid, local_only).await
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
) -> Result<StorageOperationOutput> {
    execute_plan(request.plan).await
}

pub(crate) async fn download_response(request: &StorageDownloadRequest) -> Result<Response> {
    transport::download_response(request.endpoint(), request.cid(), request.local_only()).await
}

#[derive(Debug, Clone, PartialEq)]
enum StorageOperationPlan {
    Module {
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
                StorageOperationAdapter::Module => Ok((
                    StorageOperationPlan::Module {
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
        StorageOperationAdapter::Module => Ok((
            StorageOperationPlan::Module {
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

async fn execute_plan(plan: StorageOperationPlan) -> Result<StorageOperationOutput> {
    let value = match plan {
        StorageOperationPlan::Module {
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
                let receipt =
                    transport::module_dispatch(method, args, &context, identity_role).await?;
                return Ok(StorageOperationOutput::Outcome(
                    storage_module_dispatch_outcome(method, receipt)?,
                ));
            } else {
                let value = transport::module_call(method, args).await?;
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
        "uploadUrl" => receipt.session_id().map(|session_id| {
            (
                ModuleCorrelation::with_session(session_id),
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

    pub(crate) async fn execute(&self) -> Result<Value> {
        self.client.exists(&self.cid).await
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
    pub(crate) fn parse(args: &Args) -> Result<Self> {
        let request = NodeOperationRequest::from_bridge_args(args)?;
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
    pub(crate) fn parse(args: &Args) -> Result<Self> {
        let request = NodeOperationRequest::from_bridge_args(args)?;
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

pub(crate) struct StorageRestoreRequest {
    client: StorageClient,
    cid: String,
    local_only: bool,
}

impl StorageRestoreRequest {
    pub(crate) fn parse(args: &Args) -> Result<Self> {
        let request = NodeOperationRequest::from_bridge_args(args)?;
        let payload: RestorePayload = request.payload("settings restore")?;
        Ok(Self {
            client: StorageClient::from_initialization(request.adapter())?,
            cid: required_text(payload.cid, "backup CID")?,
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
    use anyhow::Result;
    use serde_json::json;

    use super::*;

    fn request(value: Value) -> Result<NodeOperationRequest> {
        NodeOperationRequest::from_value(&value)
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
            method: "uploadUrl",
            args: vec![json!("/tmp/a"), json!(DEFAULT_BLOCK_SIZE)],
            context: vec![("path", "/tmp/a".to_owned())],
            dispatch: true,
        };
        anyhow::ensure!(request.plan == expected, "unexpected Storage upload plan");
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
            ),
        )?;
        let NodeOperationOutcome::Accepted(acceptance) = upload else {
            anyhow::bail!("module upload was not accepted");
        };
        anyhow::ensure!(
            acceptance.correlation().session_id().map(|id| id.as_str()) == Some("session-1")
                && acceptance.correlation().request_id().is_none()
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
