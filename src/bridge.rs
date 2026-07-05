use std::{
    collections::HashMap,
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::{Method, StatusCode, Url, header};
use serde_json::{Value, json};
use tokio::io::AsyncWriteExt as _;
use tokio::runtime::Runtime;
use tokio_util::io::ReaderStream;

use crate::{
    AccountTransactionSummary, LocalNodeActionRequest, TransactionIdlInspectionReport,
    TransactionSummary, account_lookup, account_lookup_with_idl, bedrock_wallet_balance,
    blockchain, channels, decode_account_data_hex_with_idl, decode_event_data_hex_with_idl,
    indexer_block_by_hash, indexer_blocks, indexer_health, indexer_status,
    indexer_transfer_recipients, inspect_transaction_summary_with_idl, last_sequencer_block_id,
    local_devnet_list, local_nodes_action, local_nodes_status, local_wallet_accounts,
    local_wallet_command, local_wallet_create_account, local_wallet_deploy_program,
    local_wallet_instruction_preview, local_wallet_instruction_submit, local_wallet_profile_status,
    local_wallet_send_transaction, local_wallet_sync_private, logoscore,
    modules::{
        blockchain_module_report, delivery_report, delivery_source_report, logoscore_status_report,
        storage_report, storage_source_report,
    },
    normalize_program_id_hex, overview, program_file_info, raw_http_json,
    raw_json_rpc_optional_result, raw_rpc_report, response_excerpt, sequencer_block,
    sequencer_blocks, sequencer_program_ids, sequencer_transaction,
    sequencer_transaction_inspection, sequencer_transaction_inspection_with_idl,
    sequencer_transaction_trace, sequencer_transaction_trace_with_idl,
    settings_backup::{export_app_settings_backup, restore_app_settings_backup},
    social::social_messages_from_store,
    spel::spel_idl_report,
    state_store::{
        RegisteredIdlEntry, load_idl_state, load_settings_state, load_wallet_state,
        registered_idl_entries, save_idl_state, save_settings_state, save_wallet_state,
    },
    wallet::detected_wallet_profile,
};

pub const INSPECTOR_MODULE: &str = "logos_inspector";
const BLOCKCHAIN_MODULE: &str = "blockchain_module";
const INDEXER_MODULE: &str = "lez_indexer_module";
const EXECUTION_MODULE: &str = "logos_execution_zone";
const DELIVERY_MODULE: &str = "delivery_module";
const DEFAULT_STORAGE_REST_ENDPOINT: &str = "http://127.0.0.1:8080/api/storage/v1";
const DEFAULT_DELIVERY_REST_ENDPOINT: &str = "http://127.0.0.1:8645";
const MAX_DELIVERY_STORE_PAGE_SIZE: u64 = 100;

#[derive(Debug, serde::Serialize)]
struct BridgeResponse {
    ok: bool,
    value: Value,
    text: String,
    error: String,
}

pub struct InspectorBridge {
    runtime: Runtime,
    node_operations: NodeOperationRegistry,
    next_node_operation_id: AtomicU64,
}

type NodeOperationRegistry = Arc<Mutex<HashMap<String, NodeOperationRecord>>>;

#[derive(Debug, Clone)]
struct NodeOperationRequest {
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

impl InspectorBridge {
    pub fn new() -> Result<Self> {
        Ok(Self {
            runtime: Runtime::new().context("failed to create tokio runtime")?,
            node_operations: Arc::new(Mutex::new(HashMap::new())),
            next_node_operation_id: AtomicU64::new(1),
        })
    }

    pub fn call_module(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        if module == INSPECTOR_MODULE {
            self.call_inspector(method, args)
        } else {
            self.call_logoscore_module(module, method, args)
        }
    }

    fn call_inspector(&self, method: &str, args: Value) -> Result<Value> {
        match method {
            "overview" => {
                let args = Args::new(args)?;
                let value = self.runtime.block_on(overview(
                    args.string(0, "sequencer endpoint")?,
                    args.string(1, "indexer endpoint")?,
                    args.string(2, "node endpoint")?,
                ));
                to_value(value)
            }
            "head" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "sequencer endpoint")?;
                self.execution_head(&source)
            }
            "programs" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "sequencer endpoint")?;
                self.require_rpc_source(&source, "programs")?;
                to_value(
                    self.runtime
                        .block_on(sequencer_program_ids(source.endpoint))?,
                )
            }
            "block" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "sequencer endpoint")?;
                self.require_rpc_source(&source, "block")?;
                to_value(self.runtime.block_on(sequencer_block(
                    source.endpoint,
                    args.u64(source.next_index, "block id")?,
                ))?)
            }
            "sequencerBlocks" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "sequencer endpoint")?;
                self.require_rpc_source(&source, "sequencerBlocks")?;
                let before = args.value(source.next_index).and_then(Value::as_u64);
                let limit = args
                    .value(source.next_index + 1)
                    .and_then(Value::as_u64)
                    .unwrap_or(10)
                    .min(50);
                to_value(
                    self.runtime
                        .block_on(sequencer_blocks(source.endpoint, before, limit))?,
                )
            }
            "transaction" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "sequencer endpoint")?;
                self.require_rpc_source(&source, "transaction")?;
                to_value(self.runtime.block_on(sequencer_transaction(
                    source.endpoint,
                    args.string(source.next_index, "transaction hash")?,
                ))?)
            }
            "inspectTransaction" => self.inspect_transaction(args),
            "traceTransaction" => self.trace_transaction(args),
            "decodeTransactionSummary" => {
                let args = Args::new(args)?;
                let summary: TransactionSummary = serde_json::from_value(
                    args.value(0)
                        .cloned()
                        .context("transaction summary is required")?,
                )
                .context("failed to parse transaction summary")?;
                to_value(inspect_transaction_summary_with_idl(
                    &summary,
                    args.string(1, "IDL JSON")?,
                )?)
            }
            "account" => self.account(args),
            "decodeAccount" => {
                let args = Args::new(args)?;
                to_value(decode_account_data_hex_with_idl(
                    args.string(1, "IDL JSON")?,
                    args.optional_string(2),
                    args.string(0, "account data hex")?,
                    None,
                )?)
            }
            "decodeEvent" => {
                let args = Args::new(args)?;
                to_value(decode_event_data_hex_with_idl(
                    args.string(1, "IDL JSON")?,
                    args.optional_string(2),
                    args.string(0, "event data hex")?,
                )?)
            }
            "blockchainNode" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                self.blockchain_node_report(&source)
            }
            "blockchainBlocks" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                let slot_from = args.u64(source.next_index, "slot from")?;
                let slot_to = args.u64(source.next_index + 1, "slot to")?;
                if let Some(limit) = args.value(source.next_index + 2).and_then(Value::as_u64) {
                    to_value(self.runtime.block_on(blockchain::blockchain_recent_blocks(
                        source.endpoint,
                        slot_from,
                        slot_to,
                        limit,
                    ))?)
                } else {
                    to_value(self.runtime.block_on(blockchain::blockchain_blocks(
                        source.endpoint,
                        slot_from,
                        slot_to,
                    ))?)
                }
            }
            "blockchainLiveBlocks" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                let slot_from = args.u64(source.next_index, "slot from")?;
                let slot_to = args.u64(source.next_index + 1, "slot to")?;
                let limit = args
                    .value(source.next_index + 2)
                    .and_then(Value::as_u64)
                    .unwrap_or(50);
                to_value(
                    self.runtime
                        .block_on(blockchain::blockchain_live_blocks_snapshot(
                            source.endpoint,
                            slot_from,
                            slot_to,
                            limit,
                        ))?,
                )
            }
            "blockchainBlock" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                to_value(self.runtime.block_on(blockchain::blockchain_block(
                    source.endpoint,
                    args.string(source.next_index, "block id")?,
                ))?)
            }
            "blockchainTransaction" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                to_value(self.runtime.block_on(blockchain::blockchain_transaction(
                    source.endpoint,
                    args.string(source.next_index, "transaction id")?,
                ))?)
            }
            "channelScan" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                self.require_rpc_source(&source, "channelScan")?;
                to_value(self.runtime.block_on(channels::channel_scan(
                    source.endpoint,
                    args.u64(source.next_index, "slot from")?,
                    args.u64(source.next_index + 1, "slot to")?,
                ))?)
            }
            "channelState" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                self.require_rpc_source(&source, "channelState")?;
                to_value(self.runtime.block_on(channels::channel_state(
                    source.endpoint,
                    args.string(source.next_index, "channel id")?,
                ))?)
            }
            "indexerHealth" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                let health = self.runtime.block_on(indexer_health(source.endpoint))?;
                Ok(json!({
                    "status": "healthy",
                    "health": health,
                }))
            }
            "indexerStatus" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                to_value(self.runtime.block_on(indexer_status(source.endpoint))?)
            }
            "indexerFinalizedHead" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                to_value(self.runtime.block_on(raw_json_rpc_optional_result(
                    source.endpoint,
                    "getLastFinalizedBlockId",
                    Value::Array(vec![]),
                ))?)
            }
            "indexerBlocks" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                let before = args.value(source.next_index).and_then(Value::as_u64);
                let limit = args
                    .value(source.next_index + 1)
                    .and_then(Value::as_u64)
                    .unwrap_or(10)
                    .min(50);
                to_value(
                    self.runtime
                        .block_on(indexer_blocks(source.endpoint, before, limit))?,
                )
            }
            "indexerBlockByHash" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                to_value(self.runtime.block_on(indexer_block_by_hash(
                    source.endpoint,
                    args.string(source.next_index, "block header hash")?,
                ))?)
            }
            "indexerTransferRecipients" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                let before = args.value(source.next_index).and_then(Value::as_u64);
                let limit = args
                    .value(source.next_index + 1)
                    .and_then(Value::as_u64)
                    .unwrap_or(50)
                    .min(50);
                to_value(self.runtime.block_on(indexer_transfer_recipients(
                    source.endpoint,
                    before,
                    limit,
                ))?)
            }
            "rawRpc" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(raw_rpc_report(
                    args.string(0, "RPC endpoint")?,
                    args.string(1, "RPC method")?,
                    args.json_or_empty_array(2)?,
                ))?)
            }
            "spelIdl" => {
                let args = Args::new(args)?;
                to_value(spel_idl_report(args.string(0, "IDL JSON")?)?)
            }
            "programFile" => {
                let args = Args::new(args)?;
                to_value(program_file_info(args.string(0, "program path")?)?)
            }
            "normalizeProgramId" => {
                let args = Args::new(args)?;
                to_value(normalize_program_id_hex(args.string(0, "program id")?)?)
            }
            "localWalletProfileStatus" => {
                let args = Args::new(args)?;
                to_value(local_wallet_profile_status(
                    args.value(0)
                        .cloned()
                        .context("local wallet profile is required")?,
                )?)
            }
            "localWalletCreateAccount" => self.run_legacy_node_operation(
                "wallet",
                "localWalletCreateAccount",
                args,
                "Wallet account",
            ),
            "localWalletSendTransaction" => self.run_legacy_node_operation(
                "wallet",
                "localWalletSendTransaction",
                args,
                "Wallet send",
            ),
            "localWalletInstructionPreview" => {
                let args = Args::new(args)?;
                to_value(local_wallet_instruction_preview(
                    args.value(0)
                        .cloned()
                        .context("IDL instruction request is required")?,
                )?)
            }
            "localWalletInstructionSubmit" => self.run_legacy_node_operation(
                "wallet",
                "localWalletInstructionSubmit",
                args,
                "IDL instruction",
            ),
            "localWalletCommand" => self.run_legacy_node_operation(
                "wallet",
                "localWalletCommand",
                args,
                "Wallet command",
            ),
            "localWalletDeployProgram" => self.run_legacy_node_operation(
                "wallet",
                "localWalletDeployProgram",
                args,
                "Program deploy",
            ),
            "localWalletSyncPrivate" => self.run_legacy_node_operation(
                "wallet",
                "localWalletSyncPrivate",
                args,
                "Private sync",
            ),
            "localWalletAccounts" => {
                let args = Args::new(args)?;
                to_value(local_wallet_accounts(
                    args.value(0)
                        .cloned()
                        .context("local wallet profile is required")?,
                )?)
            }
            "localNodesStatus" => {
                let args = Args::new(args)?;
                to_value(local_nodes_status(
                    args.optional_string(0).unwrap_or("default"),
                )?)
            }
            "localDevnetList" => {
                let args = Args::new(args)?;
                to_value(local_devnet_list(
                    args.optional_string(0).unwrap_or("default"),
                )?)
            }
            "localNodesAction" => self.run_legacy_node_operation(
                "localNodes",
                "localNodesAction",
                args,
                "Local node action",
            ),
            "bedrockWalletBalance" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(bedrock_wallet_balance(
                    args.string(0, "node endpoint")?,
                    args.string(1, "wallet public key")?,
                    args.optional_string(2),
                ))?)
            }
            "loadIdlState" => load_idl_state(),
            "saveIdlState" => {
                let args = Args::new(args)?;
                save_idl_state(args.value(0).context("IDL state is required")?)
            }
            "loadWalletState" => load_wallet_state(),
            "detectWalletProfile" => Ok(detected_wallet_profile()),
            "saveWalletState" => {
                let args = Args::new(args)?;
                save_wallet_state(args.value(0).context("wallet state is required")?)
            }
            "loadSettingsState" => load_settings_state(),
            "saveSettingsState" => {
                let args = Args::new(args)?;
                save_settings_state(args.value(0).context("settings state is required")?)
            }
            "modules" => bail!("Basecamp module reports are not supported as Inspector sources"),
            "logoscoreStatus" => to_value(logoscore_status_report()),
            "blockchainModuleReport" => {
                let args = Args::new(args)?;
                to_value(blockchain_module_report(args.optional_string(0)))
            }
            "storageReport" => {
                let args = Args::new(args)?;
                to_value(storage_report(
                    args.optional_string(0),
                    args.optional_bool(1),
                ))
            }
            "storageSourceReport" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(storage_source_report(
                    args.optional_string(0).unwrap_or("rest"),
                    args.optional_string(1),
                    args.optional_string(2),
                    args.optional_string(3),
                    args.optional_bool(4),
                )))
            }
            "nodeOperationStart" => self.node_operation_start(args),
            "nodeOperationStatus" => self.node_operation_status(args),
            "nodeOperationEvents" => self.node_operation_events(args),
            "nodeOperationCancel" => self.node_operation_cancel(args),
            "deliveryReport" => {
                let args = Args::new(args)?;
                to_value(delivery_report(args.optional_string(0)))
            }
            "deliverySourceReport" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(delivery_source_report(
                    args.optional_string(0).unwrap_or("rest"),
                    args.optional_string(1),
                    args.optional_string(2),
                )))
            }
            "storageManifests" => self.run_legacy_node_operation(
                "storage",
                "storageManifests",
                args,
                "Storage manifests",
            ),
            "storageExists" => self.storage_exists(args),
            "storageDownloadManifest" => self.run_legacy_node_operation(
                "storage",
                "storageDownloadManifest",
                args,
                "Storage manifest",
            ),
            "storageFetch" => {
                self.run_legacy_node_operation("storage", "storageFetch", args, "Storage fetch")
            }
            "storageUploadUrl" => self.run_legacy_node_operation(
                "storage",
                "storageUploadUrl",
                args,
                "Storage upload",
            ),
            "storageBackupSettings" => self.storage_backup_settings(args),
            "storageRestoreSettings" => self.storage_restore_settings(args),
            "storageDownloadToUrl" => self.run_legacy_node_operation(
                "storage",
                "storageDownloadToUrl",
                args,
                "Storage download",
            ),
            "storageDownloadStart" => self.storage_download_start(args),
            "storageOperationStatus" => self.storage_operation_status(args),
            "storageOperationCancel" => self.storage_operation_cancel(args),
            "storageRemove" => {
                self.run_legacy_node_operation("storage", "storageRemove", args, "Storage remove")
            }
            "deliveryCreateNode" => self.run_legacy_node_operation(
                "delivery",
                "deliveryCreateNode",
                args,
                "Delivery create node",
            ),
            "deliveryStart" => {
                self.run_legacy_node_operation("delivery", "deliveryStart", args, "Delivery start")
            }
            "deliveryStop" => {
                self.run_legacy_node_operation("delivery", "deliveryStop", args, "Delivery stop")
            }
            "deliverySubscribe" => self.run_legacy_node_operation(
                "delivery",
                "deliverySubscribe",
                args,
                "Delivery subscribe",
            ),
            "deliveryUnsubscribe" => self.run_legacy_node_operation(
                "delivery",
                "deliveryUnsubscribe",
                args,
                "Delivery unsubscribe",
            ),
            "deliverySend" => {
                self.run_legacy_node_operation("delivery", "deliverySend", args, "Delivery send")
            }
            "deliveryStoreQuery" => self.delivery_store_query(args),
            "socialMessagesFromStore" => self.social_messages_from_store(args),
            "capabilitiesReport" => {
                bail!("capability_module does not expose Inspector capability listing")
            }
            "callModule" => {
                let args = Args::new(args)?;
                let module = args.string(0, "module name")?;
                let method = args.string(1, "method name")?;
                let call_args = args.value(2).cloned().unwrap_or_else(|| json!([]));
                self.call_logoscore_module(module, method, call_args)
            }
            _ => bail!("unknown inspector method `{method}`"),
        }
    }

    fn inspect_transaction(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = args.source_endpoint(0, "sequencer endpoint")?;
        self.require_rpc_source(&source, "inspectTransaction")?;
        let endpoint = source.endpoint;
        let hash = args.string(source.next_index, "transaction hash")?;
        let idl = args.optional_string(source.next_index + 1);
        if let Some(idl) = idl {
            to_value(
                self.runtime
                    .block_on(sequencer_transaction_inspection_with_idl(
                        endpoint, hash, idl,
                    ))?,
            )
        } else {
            self.inspect_transaction_with_registered_idls(endpoint, hash)
        }
    }

    fn inspect_transaction_with_registered_idls(
        &self,
        endpoint: &str,
        hash: &str,
    ) -> Result<Value> {
        let inspection = self
            .runtime
            .block_on(sequencer_transaction_inspection(endpoint, hash))?;
        let Some(inspection) = inspection else {
            return Ok(Value::Null);
        };

        if let Some(report) = decode_transaction_summary_with_idls(
            &inspection.raw_summary,
            &registered_idl_entries()?,
        ) {
            return to_value(Some(report));
        }

        to_value(Some(inspection))
    }

    fn trace_transaction(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = args.source_endpoint(0, "sequencer endpoint")?;
        self.require_rpc_source(&source, "traceTransaction")?;
        let endpoint = source.endpoint;
        let hash = args.string(source.next_index, "transaction hash")?;
        let idl = args.optional_string(source.next_index + 1);
        if let Some(idl) = idl {
            to_value(
                self.runtime
                    .block_on(sequencer_transaction_trace_with_idl(endpoint, hash, idl))?,
            )
        } else {
            to_value(
                self.runtime
                    .block_on(sequencer_transaction_trace(endpoint, hash))?,
            )
        }
    }

    fn account(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let account_args = args.account_sources()?;
        if account_args.execution_mode == SourceMode::Module {
            bail!(
                "{EXECUTION_MODULE} does not expose Inspector account reads; use sequencer RPC for account inspection"
            );
        }
        if account_args.indexer_mode == SourceMode::Module {
            bail!(
                "{INDEXER_MODULE} account reads do not satisfy Inspector decode/history needs; use indexer RPC for account inspection"
            );
        }
        let sequencer = account_args.sequencer_endpoint;
        let indexer = account_args.indexer_endpoint;
        let account = account_args.account;
        let idl = args.optional_string(account_args.next_index);
        let mut value = if let Some(idl) = idl {
            to_value(self.runtime.block_on(account_lookup_with_idl(
                sequencer,
                indexer,
                account,
                idl,
                args.optional_string(account_args.next_index + 1),
            ))?)?
        } else {
            to_value(
                self.runtime
                    .block_on(account_lookup(sequencer, indexer, account))?,
            )?
        };
        enrich_account_related_transaction_decodes(&mut value)?;
        Ok(value)
    }

    fn call_logoscore_module(&self, module: &str, method: &str, args: Value) -> Result<Value> {
        let args = Args::new(args)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .collect::<Vec<_>>();
        to_value(logoscore::call(module, method, &args)?)
    }

    fn require_rpc_source(&self, source: &SourceEndpoint<'_>, method: &str) -> Result<()> {
        if source.mode == SourceMode::Rpc {
            return Ok(());
        }
        bail!(
            "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
            source.module
        )
    }

    fn blockchain_node_report(&self, source: &SourceEndpoint<'_>) -> Result<Value> {
        to_value(
            self.runtime
                .block_on(blockchain::blockchain_node_report(source.endpoint)),
        )
    }

    fn execution_head(&self, source: &SourceEndpoint<'_>) -> Result<Value> {
        self.require_rpc_source(source, "head")?;
        to_value(
            self.runtime
                .block_on(last_sequencer_block_id(source.endpoint))?,
        )
    }

    fn node_operation_start(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let request = node_operation_request_from_value(
            args.value(0)
                .cloned()
                .context("node operation request is required")?,
        )?;
        self.start_node_operation(request)
    }

    fn node_operation_status(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        self.node_operation_value(args.string(0, "node operation id")?)
    }

    fn node_operation_events(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let operation_id = args.string(0, "node operation id")?;
        let after_seq = args.value(1).and_then(Value::as_u64).unwrap_or(0);
        let operations = self
            .node_operations
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

    fn node_operation_cancel(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let operation_id = args.string(0, "node operation id")?;
        {
            let mut operations = self
                .node_operations
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
        self.node_operation_value(operation_id)
    }

    fn start_node_operation(&self, request: NodeOperationRequest) -> Result<Value> {
        let operation_id = format!(
            "{}-{}-{}",
            request.domain,
            normalized_operation_method(&request.method),
            self.next_node_operation_id.fetch_add(1, Ordering::Relaxed)
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
                .node_operations
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
        update_node_operation(&self.node_operations, &operation_id, |record| {
            push_node_operation_event_locked(
                record,
                "started",
                "operation started",
                Some(0.0),
                None,
                None,
            );
        });

        let registry = Arc::clone(&self.node_operations);
        let task_operation_id = operation_id.clone();
        self.runtime.spawn(async move {
            let result =
                execute_node_operation(request, &registry, &task_operation_id, &cancel_requested)
                    .await;
            finish_node_operation(&registry, &task_operation_id, &cancel_requested, result);
        });

        self.node_operation_value(&operation_id)
    }

    fn run_legacy_node_operation(
        &self,
        domain: &str,
        method: &str,
        args: Value,
        label: &str,
    ) -> Result<Value> {
        let request = NodeOperationRequest {
            domain: domain.to_owned(),
            source_mode: String::new(),
            endpoint: String::new(),
            module: String::new(),
            method: method.to_owned(),
            args,
            mutating_enabled: false,
            label: label.to_owned(),
        };
        let operation = self.start_node_operation(request)?;
        let operation_id = operation
            .get("operationId")
            .and_then(Value::as_str)
            .context("node operation id is missing")?
            .to_owned();
        let result = self.wait_for_node_operation_result(&operation_id);
        self.remove_node_operation(&operation_id);
        result
    }

    fn wait_for_node_operation_result(&self, operation_id: &str) -> Result<Value> {
        loop {
            let operation = {
                let operations = self
                    .node_operations
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

    fn node_operation_value(&self, operation_id: &str) -> Result<Value> {
        let operations = self
            .node_operations
            .lock()
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
        let record = operations
            .get(operation_id)
            .with_context(|| format!("node operation `{operation_id}` was not found"))?;
        Ok(node_operation_value(&record.operation))
    }

    fn remove_node_operation(&self, operation_id: &str) {
        if let Ok(mut operations) = self.node_operations.lock() {
            operations.remove(operation_id);
        }
    }

    fn storage_exists(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        let cid = args.string(source.next_index, "CID")?;
        to_value(self.runtime.block_on(raw_http_json(
            source.endpoint,
            &format!("/data/{cid}/exists"),
        ))?)
    }

    fn storage_backup_settings(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        require_mutating_diagnostics(&args, source.next_index, "settings backup action")?;
        let encrypted = args.optional_bool(source.next_index + 1);
        let wallet_profile = args.value(source.next_index + 2);
        let block_size = args
            .value(source.next_index + 3)
            .and_then(Value::as_u64)
            .unwrap_or(65_536);
        let payload = export_app_settings_backup(encrypted, wallet_profile)?;
        let bytes =
            serde_json::to_vec_pretty(&payload).context("failed to serialize settings backup")?;
        let upload = self
            .runtime
            .block_on(storage_rest_upload_bytes(
                source.endpoint,
                "logos-inspector-settings-backup.json",
                &bytes,
                block_size,
            ))
            .context("failed to upload settings backup through storage REST")?;
        let cid = upload
            .get("cid")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        Ok(json!({
            "cid": cid,
            "bytes": bytes.len(),
            "endpoint": source.endpoint,
            "encrypted": encrypted,
            "upload": upload,
        }))
    }

    fn storage_restore_settings(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        require_mutating_diagnostics(&args, source.next_index, "settings restore action")?;
        let cid = args.string(source.next_index + 1, "backup CID")?;
        let wallet_profile = args.value(source.next_index + 2);
        let local_only = args.optional_bool(source.next_index + 3);
        let bytes = self
            .runtime
            .block_on(storage_rest_download_bytes(
                source.endpoint,
                cid,
                local_only,
            ))
            .with_context(|| format!("failed to download settings backup CID {cid}"))?;
        let payload: Value = serde_json::from_slice(&bytes)
            .with_context(|| format!("settings backup CID {cid} did not contain JSON"))?;
        let summary = restore_app_settings_backup(&payload, wallet_profile)?;
        Ok(json!({
            "restored": true,
            "cid": cid,
            "bytes": bytes.len(),
            "endpoint": source.endpoint,
            "source": if local_only { "local" } else { "network" },
            "encrypted": summary.encrypted,
            "settings": summary.settings_restored,
            "idls": summary.idl_restored,
            "wallet": summary.wallet_restored,
            "favorites": summary.favorites_count,
            "idl_count": summary.idl_count,
        }))
    }

    fn storage_download_start(&self, args: Value) -> Result<Value> {
        self.start_node_operation(NodeOperationRequest {
            domain: "storage".to_owned(),
            source_mode: String::new(),
            endpoint: String::new(),
            module: String::new(),
            method: "storageDownloadToUrl".to_owned(),
            args,
            mutating_enabled: false,
            label: "Storage download".to_owned(),
        })
    }

    fn storage_operation_status(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        self.node_operation_value(args.string(0, "storage operation id")?)
    }

    fn storage_operation_cancel(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        self.node_operation_cancel(json!([args.string(0, "storage operation id")?]))
    }

    fn delivery_store_query(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
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
        let value = self
            .runtime
            .block_on(raw_http_json_url(query.as_str()))
            .context("failed to query Delivery Store")?;
        Ok(json!({
            "endpoint": source.endpoint,
            "includeData": include_data,
            "pageSize": page_size,
            "query": query.as_str(),
            "value": value,
        }))
    }

    fn social_messages_from_store(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let topic = args.string(0, "social topic")?;
        let value = args
            .value(1)
            .context("Delivery Store response is required")?;
        let expected_account = args.optional_string(2);
        to_value(social_messages_from_store(topic, value, expected_account))
    }
}

pub fn call_module_response_json(
    bridge: &InspectorBridge,
    module: &str,
    method: &str,
    args_json: &str,
) -> String {
    let result = serde_json::from_str(args_json)
        .context("failed to parse bridge args")
        .and_then(|args| bridge.call_module(module, method, args));
    bridge_response_json(result)
}

pub fn call_inspector_response_json(
    bridge: &InspectorBridge,
    method: &str,
    args_json: &str,
) -> String {
    call_module_response_json(bridge, INSPECTOR_MODULE, method, args_json)
}

fn bridge_response_json(result: Result<Value>) -> String {
    let response = match result {
        Ok(value) => BridgeResponse {
            ok: true,
            text: format_bridge_value(&value),
            value,
            error: String::new(),
        },
        Err(error) => BridgeResponse {
            ok: false,
            value: Value::Null,
            text: String::new(),
            error: format!("{error:#}"),
        },
    };

    match serde_json::to_string(&response) {
        Ok(value) => value,
        Err(error) => {
            let fallback = json!({
                "ok": false,
                "value": null,
                "text": "",
                "error": format!("failed to serialize bridge response: {error}"),
            });
            fallback.to_string()
        }
    }
}

fn format_bridge_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        value => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn to_value(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).context("failed to serialize bridge response")
}

fn node_operation_request_from_value(value: Value) -> Result<NodeOperationRequest> {
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
        default_source_mode_for_domain(&request.domain)
    } else {
        request.source_mode.clone()
    };
    let endpoint = if request.endpoint.is_empty() {
        default_endpoint_for_domain(&request.domain)
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
        return is_storage_source_token(first) || is_delivery_source_token(first);
    }
    first == request.endpoint
        || first.starts_with("http://")
        || first.starts_with("https://")
        || SourceMode::from_token(first).is_some()
}

fn storage_or_delivery_domain(domain: &str) -> bool {
    matches!(domain, "storage" | "delivery")
}

fn is_storage_source_token(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "auto"
            | "rest"
            | "standalone"
            | "standalone-rest"
            | "standalone rest"
            | "direct-rest"
            | "direct rest"
            | "module"
            | "basecamp"
            | "basecamp-module"
            | "basecamp module"
            | "metrics"
    )
}

fn default_source_mode_for_domain(domain: &str) -> String {
    match domain {
        "delivery" | "storage" => "rest".to_owned(),
        _ => "rpc".to_owned(),
    }
}

fn default_endpoint_for_domain(domain: &str) -> String {
    match domain {
        "delivery" => DEFAULT_DELIVERY_REST_ENDPOINT.to_owned(),
        "storage" => DEFAULT_STORAGE_REST_ENDPOINT.to_owned(),
        _ => String::new(),
    }
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
    matches!(
        mode.trim().to_ascii_lowercase().as_str(),
        "module" | "basecamp" | "basecamp-module" | "basecamp module"
    ) && matches!(
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
    if account_args.execution_mode == SourceMode::Module {
        bail!(
            "{EXECUTION_MODULE} does not expose Inspector account reads; use sequencer RPC for account inspection"
        );
    }
    if account_args.indexer_mode == SourceMode::Module {
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
    if source.mode == SourceMode::Rpc {
        return Ok(());
    }
    bail!(
        "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
        source.module
    )
}

async fn blocking_value(
    label: &'static str,
    task: impl FnOnce() -> Result<Value> + Send + 'static,
) -> Result<Value> {
    tokio::task::spawn_blocking(task)
        .await
        .with_context(|| format!("{label} task failed"))?
}

fn logoscore_call_value(module: &str, method: &str, args: Value) -> Result<Value> {
    let args = Args::new(args)?
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(ToOwned::to_owned)
                .unwrap_or_else(|| value.to_string())
        })
        .collect::<Vec<_>>();
    to_value(logoscore::call(module, method, &args)?)
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

fn decode_transaction_summary_with_idls(
    summary: &TransactionSummary,
    idl_entries: &[RegisteredIdlEntry],
) -> Option<TransactionIdlInspectionReport> {
    let summary_program_id = summary
        .program_id_hex
        .as_deref()
        .and_then(|value| normalize_program_id_hex(value).ok())
        .filter(|value| !value.is_empty())?;
    let mut partial = None;
    for entry in idl_entries
        .iter()
        .filter(|entry| entry.program_id_hex == summary_program_id)
    {
        let Ok(report) = inspect_transaction_summary_with_idl(summary, &entry.json) else {
            continue;
        };
        if let Some(decoded) = &report.decoded_instruction {
            if decoded.decode_error.is_none() && decoded.remaining_words.is_empty() {
                return Some(report);
            }
            if partial.is_none() {
                partial = Some(report);
            }
        }
    }
    partial
}

fn enrich_account_related_transaction_decodes(value: &mut Value) -> Result<()> {
    let idl_entries = registered_idl_entries()?;
    if idl_entries.is_empty() {
        return Ok(());
    }
    if let Some(account) = value.get_mut("account") {
        enrich_account_report_related_transaction_decodes(account, &idl_entries)?;
    } else {
        enrich_account_report_related_transaction_decodes(value, &idl_entries)?;
    }
    Ok(())
}

fn enrich_account_report_related_transaction_decodes(
    account: &mut Value,
    idl_entries: &[RegisteredIdlEntry],
) -> Result<()> {
    let Some(transactions) = account
        .get_mut("related_transactions")
        .and_then(Value::as_array_mut)
    else {
        return Ok(());
    };

    for transaction in transactions {
        if transaction.get("decoded_instruction").is_some() {
            continue;
        }
        let Ok(summary) = serde_json::from_value::<AccountTransactionSummary>(transaction.clone())
        else {
            continue;
        };
        let summary = TransactionSummary::from(&summary);
        if summary.kind != "Public" || summary.instruction_data.is_empty() {
            continue;
        }
        let Some(report) = decode_transaction_summary_with_idls(&summary, idl_entries) else {
            continue;
        };
        let Some(decoded) = report.decoded_instruction else {
            continue;
        };
        if let Some(object) = transaction.as_object_mut() {
            object.insert(
                "decoded_instruction".to_owned(),
                serde_json::to_value(decoded).context("failed to serialize transaction decode")?,
            );
        }
    }
    Ok(())
}

struct Args {
    values: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SourceMode {
    Rpc,
    Module,
}

impl SourceMode {
    fn from_token(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "rpc" | "direct-rpc" | "direct rpc" | "standalone" | "standalone-rpc"
            | "standalone rpc" | "auto" => Some(Self::Rpc),
            "module" | "basecamp" | "basecamp-module" | "basecamp module" => Some(Self::Module),
            _ => None,
        }
    }
}

struct SourceEndpoint<'a> {
    mode: SourceMode,
    endpoint: &'a str,
    next_index: usize,
    module: &'static str,
}

struct AccountSources<'a> {
    execution_mode: SourceMode,
    sequencer_endpoint: &'a str,
    indexer_mode: SourceMode,
    indexer_endpoint: &'a str,
    account: &'a str,
    next_index: usize,
}

struct RestSource<'a> {
    endpoint: &'a str,
    next_index: usize,
}

struct DeliveryStoreQuery<'a> {
    peer_addr: Option<&'a str>,
    content_topics: Option<&'a str>,
    pubsub_topic: Option<&'a str>,
    cursor: Option<&'a str>,
    page_size: u64,
    ascending: bool,
    include_data: bool,
}

impl Args {
    fn new(value: Value) -> Result<Self> {
        let values = value
            .as_array()
            .context("bridge args must be a JSON array")?
            .clone();
        Ok(Self { values })
    }

    fn iter(&self) -> impl Iterator<Item = &Value> {
        self.values.iter()
    }

    fn value(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    fn string(&self, index: usize, label: &str) -> Result<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .with_context(|| format!("{label} is required"))
    }

    fn optional_string(&self, index: usize) -> Option<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    fn optional_bool(&self, index: usize) -> bool {
        match self.value(index) {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) => matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        }
    }

    fn u64(&self, index: usize, label: &str) -> Result<u64> {
        let value = self
            .value(index)
            .with_context(|| format!("{label} is required"))?;
        if let Some(value) = value.as_u64() {
            return Ok(value);
        }
        self.string(index, label)?
            .parse::<u64>()
            .with_context(|| format!("invalid {label}"))
    }

    fn json_or_empty_array(&self, index: usize) -> Result<Value> {
        let Some(value) = self.value(index) else {
            return Ok(Value::Array(vec![]));
        };
        match value {
            Value::String(raw) if raw.trim().is_empty() => Ok(Value::Array(vec![])),
            Value::String(raw) => {
                serde_json::from_str(raw).context("failed to parse JSON argument")
            }
            value => Ok(value.clone()),
        }
    }

    fn source_endpoint(&self, index: usize, label: &str) -> Result<SourceEndpoint<'_>> {
        let first = self.string(index, label)?;
        if let Some(mode) = SourceMode::from_token(first) {
            return Ok(SourceEndpoint {
                mode,
                endpoint: self.string(index + 1, label)?,
                next_index: index + 2,
                module: source_module_for_label(label),
            });
        }
        Ok(SourceEndpoint {
            mode: SourceMode::Rpc,
            endpoint: first,
            next_index: index + 1,
            module: source_module_for_label(label),
        })
    }

    fn account_sources(&self) -> Result<AccountSources<'_>> {
        let first = self.string(0, "sequencer endpoint")?;
        if let Some(execution_mode) = SourceMode::from_token(first) {
            let indexer_mode = SourceMode::from_token(self.string(2, "indexer source mode")?)
                .context("indexer source mode must be `rpc` or `module`")?;
            return Ok(AccountSources {
                execution_mode,
                sequencer_endpoint: self.string(1, "sequencer endpoint")?,
                indexer_mode,
                indexer_endpoint: self.string(3, "indexer endpoint")?,
                account: self.string(4, "account id")?,
                next_index: 5,
            });
        }
        Ok(AccountSources {
            execution_mode: SourceMode::Rpc,
            sequencer_endpoint: first,
            indexer_mode: SourceMode::Rpc,
            indexer_endpoint: self.string(1, "indexer endpoint")?,
            account: self.string(2, "account id")?,
            next_index: 3,
        })
    }
}

fn source_module_for_label(label: &str) -> &'static str {
    if label.contains("indexer") {
        INDEXER_MODULE
    } else if label.contains("sequencer") {
        EXECUTION_MODULE
    } else {
        BLOCKCHAIN_MODULE
    }
}

fn storage_rest_source(args: &Args) -> Result<RestSource<'_>> {
    rest_source(
        args,
        DEFAULT_STORAGE_REST_ENDPOINT,
        "storage",
        "Storage REST data actions",
    )
}

fn delivery_rest_source(args: &Args) -> Result<RestSource<'_>> {
    rest_source(
        args,
        DEFAULT_DELIVERY_REST_ENDPOINT,
        "delivery",
        "Delivery REST message actions",
    )
}

fn require_mutating_diagnostics(args: &Args, index: usize, label: &str) -> Result<()> {
    if args.optional_bool(index) {
        return Ok(());
    }
    bail!("{label} requires mutating diagnostics to be enabled")
}

fn rest_source<'a>(
    args: &'a Args,
    default_endpoint: &'static str,
    source_name: &str,
    action_name: &str,
) -> Result<RestSource<'a>> {
    let mode = args.optional_string(0).unwrap_or("rest");
    let normalized = mode.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "auto" | "rest" | "standalone" | "standalone-rest" | "standalone rest" | "direct-rest"
        | "direct rest" => Ok(RestSource {
            endpoint: args.optional_string(1).unwrap_or(default_endpoint),
            next_index: 2,
        }),
        "module" | "basecamp" | "basecamp-module" | "basecamp module" => {
            bail!("{action_name} require {source_name} REST source, not module")
        }
        "metrics" => bail!("{action_name} require {source_name} REST source, not metrics"),
        _ => bail!("{source_name} source mode `{mode}` is not supported"),
    }
}

async fn storage_rest_upload(endpoint: &str, path: &str, block_size: u64) -> Result<Value> {
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
        .post(rest_url(endpoint, &format!("/data?blockSize={block_size}")))
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
    let text = send_text(request, "storage upload").await?;
    Ok(json!({
        "cid": text.trim(),
        "path": path,
        "bytes": bytes,
        "endpoint": endpoint,
    }))
}

async fn storage_rest_upload_bytes(
    endpoint: &str,
    filename: &str,
    bytes: &[u8],
    block_size: u64,
) -> Result<Value> {
    let text = send_text(
        reqwest::Client::new()
            .post(rest_url(endpoint, &format!("/data?blockSize={block_size}")))
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

async fn storage_rest_download_bytes(
    endpoint: &str,
    cid: &str,
    local_only: bool,
) -> Result<Vec<u8>> {
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
    let bytes = response
        .bytes()
        .await
        .context("failed to read storage backup download body")?;
    if !status.is_success() {
        bail!(
            "storage backup download failed with status {status}: {}",
            response_excerpt_bytes(&bytes)
        );
    }
    Ok(bytes.to_vec())
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

fn delivery_store_query_url(endpoint: &str, store_query: DeliveryStoreQuery<'_>) -> Result<Url> {
    let mut url = Url::parse(&rest_url(endpoint, "/store/v3/messages"))
        .context("invalid Delivery REST endpoint")?;
    {
        let mut query = url.query_pairs_mut();
        if let Some(peer_addr) = store_query.peer_addr {
            query.append_pair("peerAddr", peer_addr);
        }
        if let Some(content_topics) = store_query.content_topics {
            query.append_pair("contentTopics", content_topics);
        }
        if let Some(pubsub_topic) = store_query.pubsub_topic {
            query.append_pair("pubsubTopic", pubsub_topic);
        }
        if let Some(cursor) = store_query.cursor {
            query.append_pair("cursor", cursor);
        }
        query.append_pair(
            "includeData",
            if store_query.include_data {
                "true"
            } else {
                "false"
            },
        );
        query.append_pair("pageSize", &store_query.page_size.to_string());
        query.append_pair(
            "ascending",
            if store_query.ascending {
                "true"
            } else {
                "false"
            },
        );
    }
    Ok(url)
}

async fn raw_http_json_url(url: &str) -> Result<Value> {
    let text = send_text(reqwest::Client::new().get(url), url).await?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(trimmed)
        .with_context(|| format!("invalid JSON response: {}", response_excerpt(trimmed)))
}

async fn rest_json_request(
    method: Method,
    endpoint: &str,
    path: &str,
    body: Option<Value>,
) -> Result<Value> {
    let url = rest_url(endpoint, path);
    let mut request = reqwest::Client::new().request(method, &url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let text = send_text(request, &url).await?;
    serde_json::from_str(&text)
        .with_context(|| format!("invalid JSON response: {}", response_excerpt(&text)))
}

async fn rest_empty_request(
    method: Method,
    endpoint: &str,
    path: &str,
    body: Option<Value>,
) -> Result<()> {
    let url = rest_url(endpoint, path);
    let mut request = reqwest::Client::new().request(method, &url);
    if let Some(body) = body {
        request = request.json(&body);
    }
    let _ = send_text(request, &url).await?;
    Ok(())
}

async fn send_text(request: reqwest::RequestBuilder, label: &str) -> Result<String> {
    let response = request
        .send()
        .await
        .with_context(|| format!("failed to call {label}"))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .context("failed to read http response body")?;
    if !status.is_success() && status != StatusCode::NO_CONTENT {
        bail!(
            "http call `{label}` failed with status {status}: {}",
            response_excerpt(&text)
        );
    }
    Ok(text)
}

fn rest_url(endpoint: &str, path: &str) -> String {
    let endpoint = endpoint.trim_end_matches('/');
    let path = path.trim_start_matches('/');
    format!("{endpoint}/{path}")
}

fn is_delivery_source_token(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "auto"
            | "rest"
            | "direct-rest"
            | "direct waku rest"
            | "waku-rest"
            | "metrics"
            | "metrics-only"
            | "metrics only"
            | "module"
            | "basecamp"
            | "basecamp-module"
            | "basecamp module"
            | "network-monitor"
            | "network monitor"
            | "discovery-crawler"
            | "discovery crawler"
            | "crawler"
            | "unsupported"
    )
}

fn is_delivery_module_source_token(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "module" | "basecamp" | "basecamp-module" | "basecamp module"
    )
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

fn response_excerpt_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn indexer_status_bridge_requires_endpoint_argument() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let result = bridge.call_module(INSPECTOR_MODULE, "indexerStatus", json!([]));

        let Err(error) = result else {
            bail!("expected missing indexer endpoint to fail");
        };
        if !error.to_string().contains("indexer endpoint is required") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn blockchain_live_blocks_bridge_requires_slot_arguments() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "blockchainLiveBlocks",
            json!(["http://127.0.0.1:8080"]),
        );

        let Err(error) = result else {
            bail!("expected missing slot argument to fail");
        };
        if !error.to_string().contains("slot from is required") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn source_endpoint_accepts_existing_rpc_shape() -> Result<()> {
        let args = Args::new(json!(["http://127.0.0.1:8080", 1, 2]))?;
        let source = args.source_endpoint(0, "node endpoint")?;

        if source.mode != SourceMode::Rpc
            || source.endpoint != "http://127.0.0.1:8080"
            || source.next_index != 1
            || source.module != BLOCKCHAIN_MODULE
        {
            bail!("unexpected source endpoint");
        }
        Ok(())
    }

    #[test]
    fn source_endpoint_accepts_module_shape() -> Result<()> {
        let args = Args::new(json!(["module", "http://127.0.0.1:8779", 42]))?;
        let source = args.source_endpoint(0, "indexer endpoint")?;

        if source.mode != SourceMode::Module
            || source.endpoint != "http://127.0.0.1:8779"
            || source.next_index != 2
            || source.module != INDEXER_MODULE
        {
            bail!("unexpected source endpoint");
        }
        Ok(())
    }

    #[test]
    fn account_sources_accepts_mixed_source_shape() -> Result<()> {
        let args = Args::new(json!([
            "rpc",
            "https://testnet.lez.logos.co/",
            "module",
            "http://127.0.0.1:8779/",
            "account-1"
        ]))?;
        let sources = args.account_sources()?;

        if sources.execution_mode != SourceMode::Rpc
            || sources.sequencer_endpoint != "https://testnet.lez.logos.co/"
            || sources.indexer_mode != SourceMode::Module
            || sources.indexer_endpoint != "http://127.0.0.1:8779/"
            || sources.account != "account-1"
            || sources.next_index != 5
        {
            bail!("unexpected account sources");
        }
        Ok(())
    }

    #[test]
    fn storage_rest_source_rejects_module_mode_for_rest_actions() -> Result<()> {
        let args = Args::new(json!([
            "module",
            "http://127.0.0.1:8080/api/storage/v1",
            "cid"
        ]))?;
        let result = storage_rest_source(&args);

        let Err(error) = result else {
            bail!("expected module source to fail");
        };
        if !error.to_string().contains("require storage REST source") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn delivery_module_message_args_accepts_module_source_shape() -> Result<()> {
        let args = Args::new(json!(["module", "", true, "/app/1/chat/proto", "hello"]))?;
        let Some(module_args) = delivery_module_message_args(&args)? else {
            bail!("expected module args");
        };

        if module_args.flag_index != 2
            || module_args.values != vec![json!("/app/1/chat/proto"), json!("hello")]
        {
            bail!("unexpected module args");
        }
        Ok(())
    }

    #[test]
    fn delivery_rest_source_rejects_metrics_for_message_actions() -> Result<()> {
        let args = Args::new(json!(["metrics", "http://127.0.0.1:8008/metrics", "topic"]))?;
        let result = delivery_rest_source(&args);

        let Err(error) = result else {
            bail!("expected metrics source to fail");
        };
        if !error.to_string().contains("require delivery REST source") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn delivery_store_query_url_defaults_to_hashes_only_and_caps_page_size() -> Result<()> {
        let url = delivery_store_query_url(
            "http://127.0.0.1:8645/",
            DeliveryStoreQuery {
                peer_addr: Some("/ip4/127.0.0.1/tcp/60001/p2p/peer-a"),
                content_topics: Some("/app/1/chat/proto"),
                pubsub_topic: None,
                cursor: None,
                page_size: MAX_DELIVERY_STORE_PAGE_SIZE,
                ascending: true,
                include_data: false,
            },
        )?;
        let text = url.as_str();

        if !text.contains("/store/v3/messages?") {
            bail!("unexpected store path: {text}");
        }
        if !text.contains("includeData=false") || !text.contains("pageSize=100") {
            bail!("unexpected safe query parameters: {text}");
        }
        if !text.contains("peerAddr=%2Fip4%2F127.0.0.1") {
            bail!("peer address was not url encoded: {text}");
        }
        Ok(())
    }

    #[test]
    fn delivery_store_query_url_supports_comment_cursor_and_payloads() -> Result<()> {
        let url = delivery_store_query_url(
            "http://127.0.0.1:8645/",
            DeliveryStoreQuery {
                peer_addr: None,
                content_topics: Some("/lez/account/account-1/comments"),
                pubsub_topic: None,
                cursor: Some("cursor-1"),
                page_size: 25,
                ascending: true,
                include_data: true,
            },
        )?;
        let text = url.as_str();

        if !text.contains("contentTopics=%2Flez%2Faccount%2Faccount-1%2Fcomments")
            || !text.contains("cursor=cursor-1")
            || !text.contains("pageSize=25")
            || !text.contains("includeData=true")
        {
            bail!("unexpected comment store query parameters: {text}");
        }
        Ok(())
    }

    #[test]
    fn delivery_mutations_require_mutating_diagnostics_flag() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "deliverySend",
            json!([
                "rest",
                "http://127.0.0.1:8645",
                false,
                "/app/1/chat/proto",
                "hello"
            ]),
        );

        let Err(error) = result else {
            bail!("expected disabled mutating diagnostics to fail");
        };
        if !error
            .to_string()
            .contains("requires mutating diagnostics to be enabled")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn storage_mutations_require_mutating_diagnostics_flag() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "storageFetch",
            json!([
                "rest",
                "http://127.0.0.1:8080/api/storage/v1",
                false,
                "zDvtest"
            ]),
        );

        let Err(error) = result else {
            bail!("expected disabled mutating diagnostics to fail");
        };
        if !error
            .to_string()
            .contains("requires mutating diagnostics to be enabled")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_deploy_program_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "localWalletDeployProgram",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                "program.bin"
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing deployment confirmation to fail");
        };
        if !error
            .to_string()
            .contains("program deployment requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_deploy_program_reaches_wallet_validation_after_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "localWalletDeployProgram",
            json!([
                {
                    "wallet_binary": "",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                "program.bin",
                "confirm-deploy-program"
            ]),
        );

        let Err(error) = result else {
            bail!("expected wallet validation to fail");
        };
        if !error
            .to_string()
            .contains("wallet binary is required to deploy program binary")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_create_account_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "localWalletCreateAccount",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                "public",
                ""
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing create confirmation to fail");
        };
        if !error
            .to_string()
            .contains("wallet account creation requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_send_transaction_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "localWalletSendTransaction",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                {
                    "from": "Public/source",
                    "to": "Public/recipient",
                    "amount": "1"
                }
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing send confirmation to fail");
        };
        if !error
            .to_string()
            .contains("wallet transaction send requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_instruction_submit_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "localWalletInstructionSubmit",
            json!([
                {
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                {
                    "idl_json": "{}",
                    "program_id_hex": "00",
                    "instruction": "set"
                }
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing IDL instruction confirmation to fail");
        };
        if !error
            .to_string()
            .contains("IDL instruction send requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_wallet_command_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "localWalletCommand",
            json!([
                {
                    "wallet_binary": "wallet",
                    "wallet_home": ".",
                    "network_profile": "local"
                },
                ["account", "list"]
            ]),
        );

        let Err(error) = result else {
            bail!("expected missing wallet command confirmation to fail");
        };
        if !error
            .to_string()
            .contains("wallet command requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn local_nodes_action_requires_confirmation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "localNodesAction",
            json!(["local", { "action": "new_network", "network_id": "devnet-test" }]),
        );

        let Err(error) = result else {
            bail!("expected missing local node confirmation to fail");
        };
        if !error
            .to_string()
            .contains("local node action requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn node_operation_cancel_marks_cancelable_operation() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let cancel_requested = Arc::new(AtomicBool::new(false));
        {
            let mut operations = bridge
                .node_operations
                .lock()
                .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
            operations.insert(
                "existing".to_owned(),
                NodeOperationRecord {
                    operation: NodeOperation {
                        operation_id: "existing".to_owned(),
                        domain: "storage".to_owned(),
                        backend: "rest".to_owned(),
                        method: "storageDownloadToUrl".to_owned(),
                        status: NodeOperationStatus::Running,
                        label: "Storage download".to_owned(),
                        context: json!({
                            "cid": "cid-a",
                            "path": "/tmp/a",
                            "endpoint": "http://127.0.0.1:8080/api/storage/v1",
                            "source": "network"
                        }),
                        external_session_id: None,
                        progress: None,
                        bytes_written: 0,
                        content_length: None,
                        result: None,
                        error: None,
                        cancellable: true,
                        started_at: 1,
                        updated_at: 1,
                    },
                    events: Vec::new(),
                    cancel_requested: Arc::clone(&cancel_requested),
                },
            );
        }

        let value =
            bridge.call_module(INSPECTOR_MODULE, "nodeOperationCancel", json!(["existing"]))?;

        if value.get("status").and_then(Value::as_str) != Some("canceling") {
            bail!("expected canceling status: {value}");
        }
        if !cancel_requested.load(Ordering::Relaxed) {
            bail!("expected cancel flag to be set");
        }
        Ok(())
    }

    #[test]
    fn node_operation_start_accepts_storage_download_request() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let value = bridge.call_module(
            INSPECTOR_MODULE,
            "nodeOperationStart",
            json!([{
                "domain": "storage",
                "sourceMode": "rest",
                "endpoint": "http://127.0.0.1:8080/api/storage/v1",
                "method": "storageDownloadToUrl",
                "args": ["cid-b", "/tmp/b", false],
                "mutatingEnabled": true,
                "label": "Storage download"
            }]),
        )?;

        if value.get("domain").and_then(Value::as_str) != Some("storage")
            || value.get("method").and_then(Value::as_str) != Some("storageDownloadToUrl")
            || value.get("cancellable").and_then(Value::as_bool) != Some(true)
            || value.get("cid").and_then(Value::as_str) != Some("cid-b")
        {
            bail!("unexpected operation value: {value}");
        }
        Ok(())
    }

    #[test]
    fn node_operation_request_normalizes_storage_endpoint_first_args() -> Result<()> {
        let request = node_operation_request_from_value(json!({
            "domain": "storage",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8080/api/storage/v1",
            "method": "storageDownloadManifest",
            "args": ["http://127.0.0.1:8080/api/storage/v1", "z-storage"]
        }))?;

        if request.args != json!(["rest", "http://127.0.0.1:8080/api/storage/v1", "z-storage"]) {
            bail!("unexpected normalized args: {}", request.args);
        }
        Ok(())
    }

    #[test]
    fn node_operation_request_normalizes_delivery_endpoint_first_args() -> Result<()> {
        let request = node_operation_request_from_value(json!({
            "domain": "delivery",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8645",
            "method": "deliverySend",
            "mutatingEnabled": true,
            "args": ["http://127.0.0.1:8645", "/waku/2/default/proto", "hello"]
        }))?;

        if request.args
            != json!([
                "rest",
                "http://127.0.0.1:8645",
                true,
                "/waku/2/default/proto",
                "hello"
            ])
        {
            bail!("unexpected normalized args: {}", request.args);
        }
        Ok(())
    }

    #[test]
    fn node_operation_request_keeps_delivery_store_query_read_only_args() -> Result<()> {
        let request = node_operation_request_from_value(json!({
            "domain": "delivery",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8645",
            "method": "deliveryStoreQuery",
            "args": ["peer-a", "/topic/1/a/proto", "/waku/2/default-waku/proto", "cursor-a", 10, true, true]
        }))?;

        if request.args
            != json!([
                "rest",
                "http://127.0.0.1:8645",
                "peer-a",
                "/topic/1/a/proto",
                "/waku/2/default-waku/proto",
                "cursor-a",
                10,
                true,
                true
            ])
        {
            bail!("unexpected normalized args: {}", request.args);
        }
        Ok(())
    }

    #[test]
    fn node_operation_start_rejects_second_storage_download() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        {
            let mut operations = bridge
                .node_operations
                .lock()
                .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
            operations.insert(
                "storage-download-existing".to_owned(),
                test_node_operation_record(
                    "storage-download-existing",
                    "storage",
                    "storageDownloadToUrl",
                    NodeOperationStatus::Running,
                    true,
                    Arc::new(AtomicBool::new(false)),
                ),
            );
        }

        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "nodeOperationStart",
            json!([{
                "domain": "storage",
                "sourceMode": "rest",
                "endpoint": "http://127.0.0.1:8080/api/storage/v1",
                "method": "storageDownloadToUrl",
                "args": ["cid-c", "/tmp/c", false],
                "mutatingEnabled": true,
                "label": "Storage download"
            }]),
        );

        let Err(error) = result else {
            bail!("expected duplicate storage download to fail");
        };
        if !error.to_string().contains("storage download operation") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn delivery_module_send_is_gated_until_events_are_correlated() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.call_module(
            INSPECTOR_MODULE,
            "deliverySend",
            json!(["module", "", true, "/waku/2/default/proto", "hello"]),
        );

        let Err(error) = result else {
            bail!("expected module delivery send to be gated");
        };
        if !error.to_string().contains("delivery module send is gated") {
            bail!("unexpected error: {error:#}");
        }
        Ok(())
    }

    #[test]
    fn legacy_wallet_operation_record_is_removed_after_wait() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        let result = bridge.run_legacy_node_operation(
            "wallet",
            "localWalletCreateAccount",
            json!([]),
            "Wallet account",
        );

        let Err(error) = result else {
            bail!("expected wallet operation to fail before execution");
        };
        if !error
            .to_string()
            .contains("wallet account creation requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        let operations = bridge
            .node_operations
            .lock()
            .map_err(|_| anyhow::anyhow!("node operation registry is unavailable"))?;
        if !operations.is_empty() {
            bail!(
                "expected legacy operation registry cleanup, found {operations_len}",
                operations_len = operations.len()
            );
        }
        Ok(())
    }
}
