use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::Method;
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;
use tokio::runtime::Runtime;

use crate::{
    LocalNodeActionRequest, account_lookup, account_lookup_with_idl, blockchain,
    bridge::{
        Args, DeliveryStoreQuery, SourceEndpoint, blocking_value,
        decode_transaction_summary_with_idls, delivery_rest_source, delivery_store_query_url,
        enrich_account_related_transaction_decodes, logoscore_call_value, raw_http_json_url,
        require_mutating_diagnostics, rest_empty_request, rest_json_request, storage_rest_source,
        storage_rest_upload, to_value,
    },
    indexer_block_by_hash, indexer_blocks, indexer_health, indexer_status,
    indexer_transfer_recipients, last_sequencer_block_id, local_nodes_action,
    local_wallet_accounts, local_wallet_command, local_wallet_create_account,
    local_wallet_deploy_program, local_wallet_instruction_submit, local_wallet_send_transaction,
    local_wallet_sync_private, raw_http_json, raw_json_rpc_optional_result, sequencer_block,
    sequencer_blocks, sequencer_program_ids, sequencer_transaction,
    sequencer_transaction_inspection, sequencer_transaction_inspection_with_idl,
    sequencer_transaction_trace, sequencer_transaction_trace_with_idl,
    source_policy::{
        CoreEndpointMode, CoreSourceMode, SourceFamily, default_endpoint_for_domain,
        default_source_mode_for_domain, effective_source_mode, source_mode_is_token,
    },
    state_store::registered_idl_entries,
};

const INDEXER_MODULE: &str = "lez_indexer_module";
const EXECUTION_MODULE: &str = "logos_execution_zone";
const DELIVERY_MODULE: &str = "delivery_module";
const MAX_DELIVERY_STORE_PAGE_SIZE: u64 = 100;

type NodeOperationRegistry = Arc<Mutex<HashMap<String, NodeOperationRecord>>>;

#[derive(Debug)]
pub(crate) struct NodeOperations {
    registry: NodeOperationRegistry,
    next_operation_id: AtomicU64,
}

#[derive(Debug, Clone)]
pub(crate) struct NodeOperationRequest {
    domain: String,
    source_mode: String,
    endpoint: String,
    module: String,
    method: String,
    args: Value,
    mutating_enabled: bool,
    label: String,
}

#[derive(Debug, Clone)]
struct NodeOperation {
    operation_id: String,
    domain: String,
    backend: String,
    method: String,
    status: NodeOperationStatus,
    label: String,
    context: Value,
    external_session_id: Option<String>,
    progress: Option<f64>,
    bytes_written: u64,
    content_length: Option<u64>,
    result: Option<Value>,
    error: Option<String>,
    cancellable: bool,
    started_at: u64,
    updated_at: u64,
}

#[derive(Debug, Clone)]
struct NodeOperationEvent {
    seq: u64,
    operation_id: String,
    domain: String,
    method: String,
    phase: String,
    external_session_id: Option<String>,
    message: String,
    progress: Option<f64>,
    result: Option<Value>,
    error: Option<String>,
    timestamp: u64,
}

#[derive(Debug)]
struct NodeOperationRecord {
    operation: NodeOperation,
    events: Vec<NodeOperationEvent>,
    cancel_requested: Arc<AtomicBool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NodeOperationStatus {
    Running,
    Canceling,
    Completed,
    Failed,
    Canceled,
}

impl Default for NodeOperations {
    fn default() -> Self {
        Self {
            registry: Arc::new(Mutex::new(HashMap::new())),
            next_operation_id: AtomicU64::new(1),
        }
    }
}

impl NodeOperationRequest {
    pub(crate) fn legacy(domain: &str, method: &str, args: Value, label: &str) -> Self {
        Self {
            domain: domain.to_owned(),
            source_mode: String::new(),
            endpoint: String::new(),
            module: String::new(),
            method: method.to_owned(),
            args,
            mutating_enabled: false,
            label: label.to_owned(),
        }
    }

    #[cfg(test)]
    pub(crate) fn args(&self) -> &Value {
        &self.args
    }
}

impl NodeOperationStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Canceling => "canceling",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Canceled => "canceled",
        }
    }

    fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Canceled)
    }
}

impl NodeOperations {
    pub(crate) fn start_from_value(&self, runtime: &Runtime, value: Value) -> Result<Value> {
        let request = node_operation_request_from_value(value)?;
        self.start(runtime, request)
    }

    pub(crate) fn start(&self, runtime: &Runtime, request: NodeOperationRequest) -> Result<Value> {
        let operation_id = format!(
            "{}-{}-{}",
            request.domain,
            normalized_operation_method(&request.method),
            self.next_operation_id.fetch_add(1, Ordering::Relaxed)
        );
        let now = now_millis();
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let operation = NodeOperation {
            operation_id: operation_id.clone(),
            domain: request.domain.clone(),
            backend: node_operation_backend(&request),
            method: request.method.clone(),
            status: NodeOperationStatus::Running,
            label: request.label.clone(),
            context: node_operation_context(&request),
            external_session_id: None,
            progress: None,
            bytes_written: 0,
            content_length: None,
            result: None,
            error: None,
            cancellable: node_operation_cancellable(&request),
            started_at: now,
            updated_at: now,
        };
        {
            let mut operations = self
                .registry
                .lock()
                .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
            if request.domain == "storage"
                && request.method == "storageDownloadToUrl"
                && operations.values().any(active_storage_download_operation)
            {
                bail!("a storage download operation is already running");
            }
            operations.insert(
                operation_id.clone(),
                NodeOperationRecord {
                    operation,
                    events: Vec::new(),
                    cancel_requested: Arc::clone(&cancel_requested),
                },
            );
        }
        update_node_operation(&self.registry, &operation_id, |record| {
            push_node_operation_event_locked(
                record,
                "started",
                "operation started",
                Some(0.0),
                None,
                None,
            );
        });

        let registry = Arc::clone(&self.registry);
        let task_operation_id = operation_id.clone();
        runtime.spawn(async move {
            let result =
                execute_node_operation(request, &registry, &task_operation_id, &cancel_requested)
                    .await;
            finish_node_operation(&registry, &task_operation_id, &cancel_requested, result);
        });

        self.value(&operation_id)
    }

    pub(crate) fn status(&self, operation_id: &str) -> Result<Value> {
        self.value(operation_id)
    }

    pub(crate) fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value> {
        let operations = self
            .registry
            .lock()
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
        let record = operations
            .get(operation_id)
            .with_context(|| format!("node operation `{operation_id}` was not found"))?;
        let events = record
            .events
            .iter()
            .filter(|event| event.seq > after_seq)
            .map(node_operation_event_value)
            .collect::<Vec<_>>();
        let next_seq = record.events.last().map_or(after_seq, |event| event.seq);
        Ok(json!({
            "operation": node_operation_value(&record.operation),
            "events": events,
            "nextSeq": next_seq,
        }))
    }

    pub(crate) fn cancel(&self, operation_id: &str) -> Result<Value> {
        {
            let mut operations = self
                .registry
                .lock()
                .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
            let record = operations
                .get_mut(operation_id)
                .with_context(|| format!("node operation `{operation_id}` was not found"))?;
            if !record.operation.status.is_terminal() && record.operation.cancellable {
                record.cancel_requested.store(true, Ordering::Relaxed);
                record.operation.status = NodeOperationStatus::Canceling;
                record.operation.updated_at = now_millis();
                push_node_operation_event_locked(
                    record,
                    "canceling",
                    "cancel requested",
                    None,
                    None,
                    None,
                );
            } else if !record.operation.status.is_terminal() {
                push_node_operation_event_locked(
                    record,
                    "cancel_ignored",
                    "operation is not cancellable",
                    None,
                    None,
                    None,
                );
            }
        }
        self.value(operation_id)
    }

    pub(crate) fn run_legacy(
        &self,
        runtime: &Runtime,
        domain: &str,
        method: &str,
        args: Value,
        label: &str,
    ) -> Result<Value> {
        let operation = self.start(
            runtime,
            NodeOperationRequest::legacy(domain, method, args, label),
        )?;
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .context("node operation id is missing")?
            .to_owned();
        let result = self.wait_for_result(&operation_id);
        self.remove(&operation_id);
        result
    }

    pub(crate) fn wait_for_result(&self, operation_id: &str) -> Result<Value> {
        loop {
            let operation = {
                let operations = self
                    .registry
                    .lock()
                    .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
                operations
                    .get(operation_id)
                    .with_context(|| format!("node operation `{operation_id}` was not found"))?
                    .operation
                    .clone()
            };
            if operation.status.is_terminal() {
                return match operation.status {
                    NodeOperationStatus::Completed => Ok(operation.result.unwrap_or(Value::Null)),
                    NodeOperationStatus::Canceled => {
                        bail!(
                            "{}",
                            operation
                                .error
                                .unwrap_or_else(|| "node operation canceled".to_owned())
                        )
                    }
                    NodeOperationStatus::Failed => {
                        bail!(
                            "{}",
                            operation
                                .error
                                .unwrap_or_else(|| "node operation failed".to_owned())
                        )
                    }
                    NodeOperationStatus::Running | NodeOperationStatus::Canceling => {
                        bail!("node operation is still running")
                    }
                };
            }
            thread::sleep(Duration::from_millis(25));
        }
    }

    pub(crate) fn value(&self, operation_id: &str) -> Result<Value> {
        let operations = self
            .registry
            .lock()
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
        let record = operations
            .get(operation_id)
            .with_context(|| format!("node operation `{operation_id}` was not found"))?;
        Ok(node_operation_value(&record.operation))
    }

    fn remove(&self, operation_id: &str) {
        if let Ok(mut operations) = self.registry.lock() {
            operations.remove(operation_id);
        }
    }

    #[cfg(test)]
    pub(crate) fn insert_test_running_operation(
        &self,
        operation_id: &str,
        domain: &str,
        method: &str,
        cancellable: bool,
    ) -> Arc<AtomicBool> {
        let cancel_requested = Arc::new(AtomicBool::new(false));
        let record = test_node_operation_record(
            operation_id,
            domain,
            method,
            NodeOperationStatus::Running,
            cancellable,
            Arc::clone(&cancel_requested),
        );
        if let Ok(mut operations) = self.registry.lock() {
            operations.insert(operation_id.to_owned(), record);
        }
        cancel_requested
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> Result<usize> {
        Ok(self
            .registry
            .lock()
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?
            .len())
    }
}

pub(crate) fn node_operation_request_from_value(value: Value) -> Result<NodeOperationRequest> {
    let object = value
        .as_object()
        .context("node operation request must be a JSON object")?;
    let method = object_string(object, "method")
        .filter(|value| !value.is_empty())
        .context("node operation method is required")?;
    let domain = object_string(object, "domain").unwrap_or_else(|| node_operation_domain(&method));
    let args = object
        .get("args")
        .cloned()
        .unwrap_or_else(|| Value::Array(Vec::new()));
    let mut request = NodeOperationRequest {
        domain,
        source_mode: object_string(object, "sourceMode").unwrap_or_default(),
        endpoint: object_string(object, "endpoint").unwrap_or_default(),
        module: object_string(object, "module").unwrap_or_default(),
        method,
        args,
        mutating_enabled: object
            .get("mutatingEnabled")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        label: object_string(object, "label").unwrap_or_default(),
    };
    request.args = normalized_node_operation_args(&request);
    Ok(request)
}

fn object_string(object: &serde_json::Map<String, Value>, key: &str) -> Option<String> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn node_operation_domain(method: &str) -> String {
    if method.starts_with("storage") {
        "storage".to_owned()
    } else if method.starts_with("delivery") {
        "delivery".to_owned()
    } else if method.starts_with("localNodes") || method.starts_with("localDevnet") {
        "localNodes".to_owned()
    } else if method.starts_with("localWallet") || method.starts_with("bedrockWallet") {
        "wallet".to_owned()
    } else if method.starts_with("indexer") {
        "indexer".to_owned()
    } else if method.starts_with("blockchain") {
        "blockchain".to_owned()
    } else {
        "execution".to_owned()
    }
}

fn node_operation_backend(request: &NodeOperationRequest) -> String {
    if !request.source_mode.is_empty() {
        return request.source_mode.clone();
    }
    if !request.module.is_empty() {
        return request.module.clone();
    }
    if !request.endpoint.is_empty() {
        return request.endpoint.clone();
    }
    request
        .args
        .as_array()
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("direct")
        .to_owned()
}

fn node_operation_context(request: &NodeOperationRequest) -> Value {
    let mut context = serde_json::Map::new();
    if !request.endpoint.is_empty() {
        context.insert("endpoint".to_owned(), json!(request.endpoint));
    }
    if !request.source_mode.is_empty() {
        context.insert("source".to_owned(), json!(request.source_mode));
    }
    if request.mutating_enabled {
        context.insert("mutatingEnabled".to_owned(), json!(true));
    }
    if request.domain == "storage"
        && let Ok(args) = Args::new(request.args.clone())
        && let Ok(source) = storage_rest_source(&args)
    {
        context.insert("endpoint".to_owned(), json!(source.endpoint));
        match request.method.as_str() {
            "storageDownloadToUrl" => {
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
            "storageUploadUrl" => {
                if let Some(path) = args.optional_string(source.next_index + 1) {
                    context.insert("path".to_owned(), json!(path));
                }
            }
            "storageFetch" | "storageRemove" => {
                if let Some(cid) = args.optional_string(source.next_index + 1) {
                    context.insert("cid".to_owned(), json!(cid));
                }
            }
            "storageDownloadManifest" => {
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
    Value::Object(context)
}

fn normalized_node_operation_args(request: &NodeOperationRequest) -> Value {
    if request.source_mode.is_empty() && request.endpoint.is_empty() {
        return request.args.clone();
    }
    let Some(values) = request.args.as_array() else {
        return request.args.clone();
    };
    if node_operation_args_have_source(request, values) {
        return request.args.clone();
    }
    let mode = if request.source_mode.is_empty() {
        default_source_mode_for_domain(&request.domain).to_owned()
    } else {
        request.source_mode.clone()
    };
    let endpoint = if request.endpoint.is_empty() {
        default_endpoint_for_domain(&request.domain).to_owned()
    } else {
        request.endpoint.clone()
    };
    let mut normalized = vec![json!(mode), json!(endpoint)];
    if node_operation_uses_mutating_flag(request) {
        normalized.push(json!(request.mutating_enabled));
    }
    let payload_start = if storage_or_delivery_domain(&request.domain)
        && values
            .first()
            .and_then(Value::as_str)
            .map(str::trim)
            .is_some_and(|first| {
                first == endpoint || first.starts_with("http://") || first.starts_with("https://")
            }) {
        1
    } else {
        0
    };
    normalized.extend(values.iter().skip(payload_start).cloned());
    Value::Array(normalized)
}

fn node_operation_args_have_source(request: &NodeOperationRequest, values: &[Value]) -> bool {
    let Some(first) = values
        .first()
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return false;
    };
    if storage_or_delivery_domain(&request.domain) {
        return source_mode_is_token(SourceFamily::Storage, first)
            || source_mode_is_token(SourceFamily::Delivery, first);
    }
    first == request.endpoint
        || first.starts_with("http://")
        || first.starts_with("https://")
        || CoreSourceMode::from_token(first).is_some()
}

fn storage_or_delivery_domain(domain: &str) -> bool {
    matches!(domain, "storage" | "delivery")
}

fn node_operation_uses_mutating_flag(request: &NodeOperationRequest) -> bool {
    matches!(
        request.method.as_str(),
        "storageFetch"
            | "storageUploadUrl"
            | "storageDownloadToUrl"
            | "storageRemove"
            | "deliverySubscribe"
            | "deliveryUnsubscribe"
            | "deliverySend"
            | "deliveryCreateNode"
            | "deliveryStart"
            | "deliveryStop"
    )
}

fn node_operation_cancellable(request: &NodeOperationRequest) -> bool {
    request.domain == "storage" && request.method == "storageDownloadToUrl"
}

fn normalized_operation_method(method: &str) -> String {
    let normalized = method
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    if normalized.is_empty() {
        "operation".to_owned()
    } else {
        normalized
    }
}

async fn execute_node_operation(
    request: NodeOperationRequest,
    registry: &NodeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    if request.domain == "storage" && storage_module_operation_gated(&request) {
        bail!("{}", storage_module_operation_gated_message());
    }
    match request.method.as_str() {
        "storageManifests" => execute_storage_manifests(&request).await,
        "storageDownloadManifest" => execute_storage_download_manifest(&request).await,
        "storageFetch" => execute_storage_fetch(&request).await,
        "storageUploadUrl" => execute_storage_upload(&request).await,
        "storageDownloadToUrl" => {
            execute_storage_download(&request, registry, operation_id, cancel_requested).await
        }
        "storageRemove" => execute_storage_remove(&request).await,
        "deliverySubscribe" => {
            execute_delivery_subscription(&request, Method::POST, "subscribe").await
        }
        "deliveryUnsubscribe" => {
            execute_delivery_subscription(&request, Method::DELETE, "unsubscribe").await
        }
        "deliverySend" => execute_delivery_send(&request).await,
        "deliveryCreateNode" => execute_delivery_module_action(&request, "createNode").await,
        "deliveryStart" => execute_delivery_module_action(&request, "start").await,
        "deliveryStop" => execute_delivery_module_action(&request, "stop").await,
        "deliveryStoreQuery" => execute_delivery_store_query(&request).await,
        "localNodesAction" => execute_local_nodes_action(&request).await,
        "localWalletCreateAccount" => execute_wallet_create_account(&request).await,
        "localWalletSendTransaction" => execute_wallet_send_transaction(&request).await,
        "localWalletInstructionSubmit" => execute_wallet_instruction_submit(&request).await,
        "localWalletCommand" => execute_wallet_command(&request).await,
        "localWalletDeployProgram" => execute_wallet_deploy_program(&request).await,
        "localWalletSyncPrivate" => execute_wallet_sync_private(&request).await,
        "localWalletAccounts" => execute_wallet_accounts(&request).await,
        "blockchainNode" => execute_blockchain_node(&request).await,
        "blockchainBlocks" => execute_blockchain_blocks(&request).await,
        "blockchainLiveBlocks" => execute_blockchain_live_blocks(&request).await,
        "blockchainBlock" => execute_blockchain_block(&request).await,
        "blockchainTransaction" => execute_blockchain_transaction(&request).await,
        "head" => execute_execution_head(&request).await,
        "programs" => execute_programs(&request).await,
        "block" => execute_sequencer_block(&request).await,
        "sequencerBlocks" => execute_sequencer_blocks(&request).await,
        "transaction" => execute_sequencer_transaction(&request).await,
        "inspectTransaction" => execute_inspect_transaction(&request).await,
        "traceTransaction" => execute_trace_transaction(&request).await,
        "account" => execute_account_operation(&request).await,
        "indexerHealth" => execute_indexer_health_operation(&request).await,
        "indexerStatus" => execute_indexer_status_operation(&request).await,
        "indexerFinalizedHead" => execute_indexer_finalized_head(&request).await,
        "indexerBlocks" => execute_indexer_blocks_operation(&request).await,
        "indexerBlockByHash" => execute_indexer_block_by_hash_operation(&request).await,
        "indexerTransferRecipients" => {
            execute_indexer_transfer_recipients_operation(&request).await
        }
        _ => bail!("unknown node operation method `{}`", request.method),
    }
}

fn storage_module_operation_gated(request: &NodeOperationRequest) -> bool {
    let mode = if !request.source_mode.is_empty() {
        request.source_mode.as_str()
    } else {
        request
            .args
            .as_array()
            .and_then(|values| values.first())
            .and_then(Value::as_str)
            .unwrap_or_default()
    };
    effective_source_mode(SourceFamily::Storage, mode) == "module"
        && matches!(
            request.method.as_str(),
            "storageFetch"
                | "storageUploadUrl"
                | "storageDownloadToUrl"
                | "storageRemove"
                | "storageDownloadManifest"
        )
}

fn storage_module_operation_gated_message() -> &'static str {
    "storage module transfers are gated until module-info lists operation events and dispatch/progress/final events share a stable session id; see local draft issue .3esmit/github/logos-co/logos-storage-module/issues/draft/storage-module-operation-events.md"
}

async fn execute_storage_manifests(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = storage_rest_source(&args)?;
    to_value(raw_http_json(source.endpoint, "/data").await?)
}

async fn execute_storage_download_manifest(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = storage_rest_source(&args)?;
    let cid_index = if matches!(args.value(source.next_index), Some(Value::Bool(_))) {
        source.next_index + 1
    } else {
        source.next_index
    };
    let cid = args.string(cid_index, "CID")?;
    to_value(raw_http_json(source.endpoint, &format!("/data/{cid}/network/manifest")).await?)
}

async fn execute_storage_fetch(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
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

async fn execute_storage_upload(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
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

async fn execute_storage_download(
    request: &NodeOperationRequest,
    registry: &NodeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
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

async fn execute_storage_remove(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
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

async fn execute_delivery_subscription(
    request: &NodeOperationRequest,
    method: Method,
    module_method: &'static str,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = delivery_module_message_args(&args)? {
        require_mutating_diagnostics(&args, module_args.flag_index, "delivery message action")?;
        return blocking_value("delivery module message action", move || {
            logoscore_call_value(
                DELIVERY_MODULE,
                module_method,
                Value::Array(module_args.values),
            )
        })
        .await;
    }
    let source = delivery_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "delivery message action")?;
    let topic = args.string(source.next_index + 1, "content topic")?;
    rest_empty_request(
        method.clone(),
        source.endpoint,
        "/relay/v1/auto/subscriptions",
        Some(json!([topic])),
    )
    .await
    .with_context(|| format!("failed to update relay subscription for {topic}"))?;
    Ok(json!({
        "subscribed": method == Method::POST,
        "contentTopic": topic,
        "endpoint": source.endpoint,
    }))
}

async fn execute_delivery_send(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if let Some(module_args) = delivery_module_message_args(&args)? {
        require_mutating_diagnostics(&args, module_args.flag_index, "delivery message action")?;
        bail!(
            "delivery module send is gated until messageSent/messageError events can be correlated with the dispatch request id; use Delivery REST source for send diagnostics"
        );
    }
    let source = delivery_rest_source(&args)?;
    require_mutating_diagnostics(&args, source.next_index, "delivery message action")?;
    let topic = args.string(source.next_index + 1, "content topic")?;
    let payload = args.string(source.next_index + 2, "message payload")?;
    let body = json!({
        "contentTopic": topic,
        "payload": BASE64_STANDARD.encode(payload.as_bytes()),
    });
    rest_empty_request(
        Method::POST,
        source.endpoint,
        "/relay/v1/auto/messages",
        Some(body),
    )
    .await
    .with_context(|| format!("failed to send relay message on {topic}"))?;
    Ok(json!({
        "sent": true,
        "contentTopic": topic,
        "bytes": payload.len(),
        "endpoint": source.endpoint,
    }))
}

async fn execute_delivery_module_action(
    request: &NodeOperationRequest,
    method: &'static str,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let start_index = if let Some(source) = args.optional_string(0).filter(|source| {
        is_delivery_source_token(source) || is_delivery_module_source_token(source)
    }) {
        if !is_delivery_module_source_token(source) {
            bail!("delivery node lifecycle actions require delivery module source");
        }
        require_mutating_diagnostics(&args, 2, "delivery node lifecycle action")?;
        3
    } else {
        require_mutating_diagnostics(&args, 0, "delivery node lifecycle action")?;
        0
    };
    let call_args = args.iter().skip(start_index).cloned().collect::<Vec<_>>();
    blocking_value("delivery module node action", move || {
        logoscore_call_value(DELIVERY_MODULE, method, Value::Array(call_args))
    })
    .await
}

async fn execute_delivery_store_query(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = delivery_rest_source(&args)?;
    let peer_addr = args.optional_string(source.next_index + 1);
    let content_topics = args.optional_string(source.next_index + 2);
    let pubsub_topic = args.optional_string(source.next_index + 3);
    let cursor = args.optional_string(source.next_index + 4);
    let page_size = args
        .value(source.next_index + 5)
        .and_then(Value::as_u64)
        .unwrap_or(20)
        .clamp(1, MAX_DELIVERY_STORE_PAGE_SIZE);
    let ascending = args.optional_bool(source.next_index + 6);
    let include_data = args.optional_bool(source.next_index + 7);
    let query = delivery_store_query_url(
        source.endpoint,
        DeliveryStoreQuery {
            peer_addr,
            content_topics,
            pubsub_topic,
            cursor,
            page_size,
            ascending,
            include_data,
        },
    )?;
    let value = raw_http_json_url(query.as_str())
        .await
        .context("failed to query Delivery Store")?;
    Ok(json!({
        "endpoint": source.endpoint,
        "includeData": include_data,
        "pageSize": page_size,
        "query": query.as_str(),
        "value": value,
    }))
}

async fn execute_local_nodes_action(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let action_request = serde_json::from_value::<LocalNodeActionRequest>(
        args.value(1)
            .cloned()
            .context("local node action request is required")?,
    )
    .context("failed to parse local node action request")?;
    let profile = args.optional_string(0).unwrap_or("default").to_owned();
    let confirmation = args.optional_string(2).map(ToOwned::to_owned);
    blocking_value("local node action", move || {
        to_value(local_nodes_action(
            &profile,
            action_request,
            confirmation.as_deref(),
        )?)
    })
    .await
}

async fn execute_wallet_create_account(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(3) != Some("confirm-create-account") {
        bail!("wallet account creation requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    let privacy = args.string(1, "account privacy")?.to_owned();
    let label = args.optional_string(2).map(ToOwned::to_owned);
    blocking_value("wallet account creation", move || {
        to_value(local_wallet_create_account(
            profile,
            &privacy,
            label.as_deref(),
        )?)
    })
    .await
}

async fn execute_wallet_send_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-send-transaction") {
        bail!("wallet transaction send requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    let send_request = args
        .value(1)
        .cloned()
        .context("wallet send request is required")?;
    blocking_value("wallet transaction send", move || {
        to_value(local_wallet_send_transaction(profile, send_request)?)
    })
    .await
}

async fn execute_wallet_instruction_submit(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-idl-instruction") {
        bail!("IDL instruction send requires explicit confirmation");
    }
    to_value(
        local_wallet_instruction_submit(
            args.value(0)
                .cloned()
                .context("local wallet profile is required")?,
            args.value(1)
                .cloned()
                .context("IDL instruction request is required")?,
        )
        .await?,
    )
}

async fn execute_wallet_command(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-wallet-command") {
        bail!("wallet command requires explicit confirmation");
    }
    let command_args = serde_json::from_value::<Vec<String>>(
        args.value(1)
            .cloned()
            .context("wallet command arguments are required")?,
    )
    .context("wallet command arguments must be a string array")?;
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    blocking_value("wallet command", move || {
        to_value(local_wallet_command(profile, command_args)?)
    })
    .await
}

async fn execute_wallet_deploy_program(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.optional_string(2) != Some("confirm-deploy-program") {
        bail!("program deployment requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    let program_path = args.string(1, "program path")?.to_owned();
    blocking_value("program deployment", move || {
        to_value(local_wallet_deploy_program(profile, &program_path)?)
    })
    .await
}

async fn execute_wallet_sync_private(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    if args.string(1, "private sync confirmation")? != "confirm-sync-private" {
        bail!("private wallet sync requires explicit confirmation");
    }
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    blocking_value("private wallet sync", move || {
        to_value(local_wallet_sync_private(profile)?)
    })
    .await
}

async fn execute_wallet_accounts(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let profile = args
        .value(0)
        .cloned()
        .context("local wallet profile is required")?;
    blocking_value("wallet accounts", move || {
        to_value(local_wallet_accounts(profile)?)
    })
    .await
}

async fn execute_blockchain_node(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(blockchain::blockchain_node_report(source.endpoint).await)
}

async fn execute_blockchain_blocks(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let slot_from = args.u64(source.next_index, "slot from")?;
    let slot_to = args.u64(source.next_index + 1, "slot to")?;
    if let Some(limit) = args.value(source.next_index + 2).and_then(Value::as_u64) {
        to_value(
            blockchain::blockchain_recent_blocks(source.endpoint, slot_from, slot_to, limit)
                .await?,
        )
    } else {
        to_value(blockchain::blockchain_blocks(source.endpoint, slot_from, slot_to).await?)
    }
}

async fn execute_blockchain_live_blocks(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    let slot_from = args.u64(source.next_index, "slot from")?;
    let slot_to = args.u64(source.next_index + 1, "slot to")?;
    let limit = args
        .value(source.next_index + 2)
        .and_then(Value::as_u64)
        .unwrap_or(50);
    to_value(
        blockchain::blockchain_live_blocks_snapshot(source.endpoint, slot_from, slot_to, limit)
            .await?,
    )
}

async fn execute_blockchain_block(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(
        blockchain::blockchain_block(source.endpoint, args.string(source.next_index, "block id")?)
            .await?,
    )
}

async fn execute_blockchain_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "node endpoint")?;
    to_value(
        blockchain::blockchain_transaction(
            source.endpoint,
            args.string(source.next_index, "transaction id")?,
        )
        .await?,
    )
}

async fn execute_execution_head(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "head")?;
    to_value(last_sequencer_block_id(source.endpoint).await?)
}

async fn execute_programs(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "programs")?;
    to_value(sequencer_program_ids(source.endpoint).await?)
}

async fn execute_sequencer_block(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "block")?;
    to_value(sequencer_block(source.endpoint, args.u64(source.next_index, "block id")?).await?)
}

async fn execute_sequencer_blocks(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "sequencerBlocks")?;
    let before = args.value(source.next_index).and_then(Value::as_u64);
    let limit = args
        .value(source.next_index + 1)
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .min(50);
    to_value(sequencer_blocks(source.endpoint, before, limit).await?)
}

async fn execute_sequencer_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "transaction")?;
    to_value(
        sequencer_transaction(
            source.endpoint,
            args.string(source.next_index, "transaction hash")?,
        )
        .await?,
    )
}

async fn execute_inspect_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "inspectTransaction")?;
    let endpoint = source.endpoint;
    let hash = args.string(source.next_index, "transaction hash")?;
    let idl = args.optional_string(source.next_index + 1);
    if let Some(idl) = idl {
        return to_value(sequencer_transaction_inspection_with_idl(endpoint, hash, idl).await?);
    }
    let inspection = sequencer_transaction_inspection(endpoint, hash).await?;
    let Some(inspection) = inspection else {
        return Ok(Value::Null);
    };
    if let Some(report) =
        decode_transaction_summary_with_idls(&inspection.raw_summary, &registered_idl_entries()?)
    {
        return to_value(Some(report));
    }
    to_value(Some(inspection))
}

async fn execute_trace_transaction(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "sequencer endpoint")?;
    require_rpc_operation_source(&source, "traceTransaction")?;
    let endpoint = source.endpoint;
    let hash = args.string(source.next_index, "transaction hash")?;
    if let Some(idl) = args.optional_string(source.next_index + 1) {
        to_value(sequencer_transaction_trace_with_idl(endpoint, hash, idl).await?)
    } else {
        to_value(sequencer_transaction_trace(endpoint, hash).await?)
    }
}

async fn execute_account_operation(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let account_args = args.account_sources()?;
    if account_args.execution_mode == CoreEndpointMode::Module {
        bail!(
            "{EXECUTION_MODULE} does not expose Inspector account reads; use sequencer RPC for account inspection"
        );
    }
    if account_args.indexer_mode == CoreEndpointMode::Module {
        bail!(
            "{INDEXER_MODULE} account reads do not satisfy Inspector decode/history needs; use indexer RPC for account inspection"
        );
    }
    let idl = args.optional_string(account_args.next_index);
    let mut value = if let Some(idl) = idl {
        to_value(
            account_lookup_with_idl(
                account_args.sequencer_endpoint,
                account_args.indexer_endpoint,
                account_args.account,
                idl,
                args.optional_string(account_args.next_index + 1),
            )
            .await?,
        )?
    } else {
        to_value(
            account_lookup(
                account_args.sequencer_endpoint,
                account_args.indexer_endpoint,
                account_args.account,
            )
            .await?,
        )?
    };
    enrich_account_related_transaction_decodes(&mut value)?;
    Ok(value)
}

async fn execute_indexer_health_operation(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    let health = indexer_health(source.endpoint).await?;
    Ok(json!({
        "status": "healthy",
        "health": health,
    }))
}

async fn execute_indexer_status_operation(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    to_value(indexer_status(source.endpoint).await?)
}

async fn execute_indexer_finalized_head(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    to_value(
        raw_json_rpc_optional_result(
            source.endpoint,
            "getLastFinalizedBlockId",
            Value::Array(vec![]),
        )
        .await?,
    )
}

async fn execute_indexer_blocks_operation(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    let before = args.value(source.next_index).and_then(Value::as_u64);
    let limit = args
        .value(source.next_index + 1)
        .and_then(Value::as_u64)
        .unwrap_or(10)
        .min(50);
    to_value(indexer_blocks(source.endpoint, before, limit).await?)
}

async fn execute_indexer_block_by_hash_operation(request: &NodeOperationRequest) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    to_value(
        indexer_block_by_hash(
            source.endpoint,
            args.string(source.next_index, "block header hash")?,
        )
        .await?,
    )
}

async fn execute_indexer_transfer_recipients_operation(
    request: &NodeOperationRequest,
) -> Result<Value> {
    let args = Args::new(request.args.clone())?;
    let source = args.source_endpoint(0, "indexer endpoint")?;
    let before = args.value(source.next_index).and_then(Value::as_u64);
    let limit = args
        .value(source.next_index + 1)
        .and_then(Value::as_u64)
        .unwrap_or(50)
        .min(50);
    to_value(indexer_transfer_recipients(source.endpoint, before, limit).await?)
}

fn require_rpc_operation_source(source: &SourceEndpoint<'_>, method: &str) -> Result<()> {
    if source.mode == CoreEndpointMode::Rpc {
        return Ok(());
    }
    bail!(
        "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
        source.module
    )
}

fn now_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    u64::try_from(millis).unwrap_or(u64::MAX)
}

fn update_node_operation(
    registry: &NodeOperationRegistry,
    operation_id: &str,
    update: impl FnOnce(&mut NodeOperationRecord),
) {
    if let Ok(mut operations) = registry.lock()
        && let Some(record) = operations.get_mut(operation_id)
    {
        update(record);
    }
}

fn active_storage_download_operation(record: &NodeOperationRecord) -> bool {
    record.operation.domain == "storage"
        && record.operation.method == "storageDownloadToUrl"
        && !record.operation.status.is_terminal()
}

fn update_node_operation_progress(
    registry: &NodeOperationRegistry,
    operation_id: &str,
    bytes_written: u64,
    content_length: Option<u64>,
) {
    update_node_operation(registry, operation_id, |record| {
        record.operation.bytes_written = bytes_written;
        if content_length.is_some() {
            record.operation.content_length = content_length;
        }
        let progress = operation_progress(bytes_written, record.operation.content_length);
        record.operation.progress = progress;
        push_node_operation_event_locked(
            record,
            "progress",
            "operation progress",
            progress,
            None,
            None,
        );
    });
}

fn finish_node_operation(
    registry: &NodeOperationRegistry,
    operation_id: &str,
    cancel_requested: &AtomicBool,
    result: Result<Value>,
) {
    update_node_operation(registry, operation_id, |record| match result {
        Ok(value) => {
            record.operation.status = NodeOperationStatus::Completed;
            record.operation.external_session_id = external_session_id(&value);
            record.operation.result = Some(value.clone());
            record.operation.error = None;
            record.operation.progress = Some(1.0);
            record.operation.updated_at = now_millis();
            push_node_operation_event_locked(
                record,
                "completed",
                "operation completed",
                Some(1.0),
                Some(value),
                None,
            );
        }
        Err(error) if cancel_requested.load(Ordering::Relaxed) => {
            let error_text = error.to_string();
            record.operation.status = NodeOperationStatus::Canceled;
            record.operation.error = Some(error_text.clone());
            record.operation.updated_at = now_millis();
            push_node_operation_event_locked(
                record,
                "canceled",
                "operation canceled",
                record.operation.progress,
                None,
                Some(error_text),
            );
        }
        Err(error) => {
            let error_text = error.to_string();
            record.operation.status = NodeOperationStatus::Failed;
            record.operation.error = Some(error_text.clone());
            record.operation.updated_at = now_millis();
            push_node_operation_event_locked(
                record,
                "failed",
                "operation failed",
                record.operation.progress,
                None,
                Some(error_text),
            );
        }
    });
}

fn push_node_operation_event_locked(
    record: &mut NodeOperationRecord,
    phase: &str,
    message: &str,
    progress: Option<f64>,
    result: Option<Value>,
    error: Option<String>,
) {
    if let Some(value) = progress {
        record.operation.progress = Some(value);
    }
    record.operation.updated_at = now_millis();
    let seq = u64::try_from(record.events.len())
        .unwrap_or(u64::MAX)
        .saturating_add(1);
    record.events.push(NodeOperationEvent {
        seq,
        operation_id: record.operation.operation_id.clone(),
        domain: record.operation.domain.clone(),
        method: record.operation.method.clone(),
        phase: phase.to_owned(),
        external_session_id: record.operation.external_session_id.clone(),
        message: message.to_owned(),
        progress,
        result,
        error,
        timestamp: now_millis(),
    });
}

fn operation_progress(bytes_written: u64, content_length: Option<u64>) -> Option<f64> {
    match content_length {
        Some(total) if total > 0 => Some(bytes_written as f64 / total as f64),
        _ => None,
    }
}

fn external_session_id(value: &Value) -> Option<String> {
    let object = value.as_object()?;
    for key in [
        "sessionId",
        "session_id",
        "operationId",
        "operation_id",
        "requestId",
        "request_id",
    ] {
        if let Some(value) = object
            .get(key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return Some(value.to_owned());
        }
    }
    None
}

fn node_operation_value(operation: &NodeOperation) -> Value {
    let mut value = json!({
        "operationId": operation.operation_id,
        "domain": operation.domain,
        "backend": operation.backend,
        "method": operation.method,
        "status": operation.status.as_str(),
        "label": operation.label,
        "externalSessionId": operation.external_session_id,
        "progress": operation.progress,
        "bytesWritten": operation.bytes_written,
        "contentLength": operation.content_length,
        "result": operation.result,
        "error": operation.error,
        "cancellable": operation.cancellable && !operation.status.is_terminal(),
        "startedAt": operation.started_at,
        "updatedAt": operation.updated_at,
        "context": operation.context,
    });
    if let (Value::Object(target), Value::Object(context)) = (&mut value, &operation.context) {
        for key in ["cid", "path", "endpoint", "source"] {
            if let Some(context_value) = context.get(key) {
                target.insert(key.to_owned(), context_value.clone());
            }
        }
    }
    value
}

fn node_operation_event_value(event: &NodeOperationEvent) -> Value {
    json!({
        "seq": event.seq,
        "operationId": event.operation_id,
        "domain": event.domain,
        "method": event.method,
        "phase": event.phase,
        "externalSessionId": event.external_session_id,
        "message": event.message,
        "progress": event.progress,
        "result": event.result,
        "error": event.error,
        "timestamp": event.timestamp,
    })
}

async fn storage_rest_download_tracked(
    endpoint: &str,
    cid: &str,
    path: &str,
    local_only: bool,
    registry: &NodeOperationRegistry,
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
    let response = reqwest::Client::new()
        .get(rest_url(endpoint, &route))
        .send()
        .await
        .with_context(|| format!("failed to call {}", rest_url(endpoint, &route)))?;
    let status = response.status();
    if !status.is_success() {
        let bytes = response
            .bytes()
            .await
            .context("failed to read storage download error body")?;
        bail!(
            "storage download failed with status {status}: {}",
            response_excerpt_bytes(&bytes)
        );
    }
    update_node_operation_progress(registry, operation_id, 0, response.content_length());
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
            update_node_operation_progress(registry, operation_id, bytes, None);
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

fn is_delivery_source_token(value: &str) -> bool {
    source_mode_is_token(SourceFamily::Delivery, value)
}

fn is_delivery_module_source_token(value: &str) -> bool {
    effective_source_mode(SourceFamily::Delivery, value) == "module"
}

struct DeliveryModuleArgs {
    flag_index: usize,
    values: Vec<Value>,
}

fn delivery_module_message_args(args: &Args) -> Result<Option<DeliveryModuleArgs>> {
    let Some(source) = args
        .optional_string(0)
        .filter(|source| is_delivery_source_token(source))
    else {
        return Ok(None);
    };
    if !is_delivery_module_source_token(source) {
        return Ok(None);
    }
    let values = args.iter().skip(3).cloned().collect::<Vec<_>>();
    if values.is_empty() {
        bail!("delivery module message arguments are required");
    }
    Ok(Some(DeliveryModuleArgs {
        flag_index: 2,
        values,
    }))
}

fn rest_url(endpoint: &str, path: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{endpoint}/{path}")
}

fn response_excerpt_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(400).collect()
}

#[cfg(test)]
fn test_node_operation_record(
    operation_id: &str,
    domain: &str,
    method: &str,
    status: NodeOperationStatus,
    cancellable: bool,
    cancel_requested: Arc<AtomicBool>,
) -> NodeOperationRecord {
    NodeOperationRecord {
        operation: NodeOperation {
            operation_id: operation_id.to_owned(),
            domain: domain.to_owned(),
            backend: "test".to_owned(),
            method: method.to_owned(),
            status,
            label: "Test operation".to_owned(),
            context: Value::Null,
            external_session_id: None,
            progress: None,
            bytes_written: 0,
            content_length: None,
            result: None,
            error: None,
            cancellable,
            started_at: 1,
            updated_at: 1,
        },
        events: Vec::new(),
        cancel_requested,
    }
}
