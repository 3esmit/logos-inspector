use std::path::Path;

use anyhow::{Context as _, Result, bail};
use reqwest::{Method, StatusCode, Url, header};
use serde_json::{Value, json};
use tokio::runtime::Runtime;
use tokio_util::io::ReaderStream;

use crate::{
    AccountTransactionSummary, TransactionIdlInspectionReport, TransactionSummary, account_lookup,
    account_lookup_with_idl, bedrock_wallet_balance, blockchain, channels,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl, indexer_block_by_hash,
    indexer_blocks, indexer_health, indexer_status, indexer_transfer_recipients,
    inspect_transaction_summary_with_idl,
    inspector_commands::{OperationRunner, handle_operation_command},
    last_sequencer_block_id, local_devnet_list, local_nodes_status,
    local_wallet_instruction_preview, local_wallet_profile_status, logoscore,
    modules::{
        blockchain_module_report, delivery_report, delivery_source_report, logoscore_status_report,
        storage_report, storage_source_report,
    },
    network_profiles,
    node_operations::{NodeOperationRequest, NodeOperations},
    normalize_program_id_hex, overview, program_file_info, raw_http_json,
    raw_json_rpc_optional_result, raw_rpc_report, response_excerpt, sequencer_block,
    sequencer_blocks, sequencer_program_ids, sequencer_transaction,
    sequencer_transaction_inspection, sequencer_transaction_inspection_with_idl,
    sequencer_transaction_trace, sequencer_transaction_trace_with_idl,
    settings_backup::{export_app_settings_backup, restore_app_settings_backup},
    social::social_messages_from_store,
    source_policy::{
        CoreEndpointMode, CoreSourceMode, DEFAULT_DELIVERY_REST_ENDPOINT,
        DEFAULT_STORAGE_REST_ENDPOINT, DeliverySourceMode, StorageSourceMode, source_policy_report,
    },
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

#[derive(Debug, serde::Serialize)]
struct BridgeResponse {
    ok: bool,
    value: Value,
    text: String,
    error: String,
}

pub struct InspectorBridge {
    runtime: Runtime,
    node_operations: NodeOperations,
}

struct BridgeOperationRunner<'a> {
    runtime: &'a Runtime,
    node_operations: &'a NodeOperations,
}

impl OperationRunner for BridgeOperationRunner<'_> {
    fn start_from_value(&self, value: Value) -> Result<Value> {
        self.node_operations.start_from_value(self.runtime, value)
    }

    fn status(&self, operation_id: &str) -> Result<Value> {
        self.node_operations.status(operation_id)
    }

    fn events(&self, operation_id: &str, after_seq: u64) -> Result<Value> {
        self.node_operations.events(operation_id, after_seq)
    }

    fn cancel(&self, operation_id: &str) -> Result<Value> {
        self.node_operations.cancel(operation_id)
    }

    fn run_legacy(&self, domain: &str, method: &str, args: Value, label: &str) -> Result<Value> {
        self.node_operations
            .run_legacy(self.runtime, domain, method, args, label)
    }

    fn start_legacy(&self, domain: &str, method: &str, args: Value, label: &str) -> Result<Value> {
        self.node_operations.start(
            self.runtime,
            NodeOperationRequest::legacy(domain, method, args, label),
        )
    }
}

impl InspectorBridge {
    pub fn new() -> Result<Self> {
        Ok(Self {
            runtime: Runtime::new().context("failed to create tokio runtime")?,
            node_operations: NodeOperations::default(),
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
        let operation_runner = BridgeOperationRunner {
            runtime: &self.runtime,
            node_operations: &self.node_operations,
        };
        if let Some(value) = handle_operation_command(&operation_runner, method, &args)? {
            return Ok(value);
        }

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
            "sourcePolicy" => to_value(source_policy_report(network_profiles())),
            "localWalletProfileStatus" => {
                let args = Args::new(args)?;
                to_value(local_wallet_profile_status(
                    args.value(0)
                        .cloned()
                        .context("local wallet profile is required")?,
                )?)
            }
            "localWalletInstructionPreview" => {
                let args = Args::new(args)?;
                to_value(local_wallet_instruction_preview(
                    args.value(0)
                        .cloned()
                        .context("IDL instruction request is required")?,
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
            "storageExists" => self.storage_exists(args),
            "storageBackupSettings" => self.storage_backup_settings(args),
            "storageRestoreSettings" => self.storage_restore_settings(args),
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
        if source.mode == CoreEndpointMode::Rpc {
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

pub(crate) fn to_value(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).context("failed to serialize bridge response")
}

pub(crate) async fn blocking_value(
    label: &'static str,
    task: impl FnOnce() -> Result<Value> + Send + 'static,
) -> Result<Value> {
    tokio::task::spawn_blocking(task)
        .await
        .with_context(|| format!("{label} task failed"))?
}

pub(crate) fn logoscore_call_value(module: &str, method: &str, args: Value) -> Result<Value> {
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

pub(crate) fn decode_transaction_summary_with_idls(
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

pub(crate) fn enrich_account_related_transaction_decodes(value: &mut Value) -> Result<()> {
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

pub(crate) struct Args {
    values: Vec<Value>,
}

pub(crate) struct SourceEndpoint<'a> {
    pub(crate) mode: CoreEndpointMode,
    pub(crate) endpoint: &'a str,
    pub(crate) next_index: usize,
    pub(crate) module: &'static str,
}

pub(crate) struct AccountSources<'a> {
    pub(crate) execution_mode: CoreEndpointMode,
    pub(crate) sequencer_endpoint: &'a str,
    pub(crate) indexer_mode: CoreEndpointMode,
    pub(crate) indexer_endpoint: &'a str,
    pub(crate) account: &'a str,
    pub(crate) next_index: usize,
}

pub(crate) struct RestSource<'a> {
    pub(crate) endpoint: &'a str,
    pub(crate) next_index: usize,
}

pub(crate) struct DeliveryStoreQuery<'a> {
    pub(crate) peer_addr: Option<&'a str>,
    pub(crate) content_topics: Option<&'a str>,
    pub(crate) pubsub_topic: Option<&'a str>,
    pub(crate) cursor: Option<&'a str>,
    pub(crate) page_size: u64,
    pub(crate) ascending: bool,
    pub(crate) include_data: bool,
}

impl Args {
    pub(crate) fn new(value: Value) -> Result<Self> {
        let values = value
            .as_array()
            .context("bridge args must be a JSON array")?
            .clone();
        Ok(Self { values })
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &Value> {
        self.values.iter()
    }

    pub(crate) fn value(&self, index: usize) -> Option<&Value> {
        self.values.get(index)
    }

    pub(crate) fn string(&self, index: usize, label: &str) -> Result<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .with_context(|| format!("{label} is required"))
    }

    pub(crate) fn optional_string(&self, index: usize) -> Option<&str> {
        self.value(index)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
    }

    pub(crate) fn optional_bool(&self, index: usize) -> bool {
        match self.value(index) {
            Some(Value::Bool(value)) => *value,
            Some(Value::String(value)) => matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            ),
            _ => false,
        }
    }

    pub(crate) fn u64(&self, index: usize, label: &str) -> Result<u64> {
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

    pub(crate) fn json_or_empty_array(&self, index: usize) -> Result<Value> {
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

    pub(crate) fn source_endpoint(&self, index: usize, label: &str) -> Result<SourceEndpoint<'_>> {
        let first = self.string(index, label)?;
        if let Some(mode) = CoreSourceMode::from_token(first) {
            return Ok(SourceEndpoint {
                mode: mode.effective(),
                endpoint: self.string(index + 1, label)?,
                next_index: index + 2,
                module: source_module_for_label(label),
            });
        }
        Ok(SourceEndpoint {
            mode: CoreEndpointMode::Rpc,
            endpoint: first,
            next_index: index + 1,
            module: source_module_for_label(label),
        })
    }

    pub(crate) fn account_sources(&self) -> Result<AccountSources<'_>> {
        let first = self.string(0, "sequencer endpoint")?;
        if let Some(execution_mode) = CoreSourceMode::from_token(first) {
            let indexer_mode = CoreSourceMode::from_token(self.string(2, "indexer source mode")?)
                .context("indexer source mode must be `rpc` or `module`")?;
            return Ok(AccountSources {
                execution_mode: execution_mode.effective(),
                sequencer_endpoint: self.string(1, "sequencer endpoint")?,
                indexer_mode: indexer_mode.effective(),
                indexer_endpoint: self.string(3, "indexer endpoint")?,
                account: self.string(4, "account id")?,
                next_index: 5,
            });
        }
        Ok(AccountSources {
            execution_mode: CoreEndpointMode::Rpc,
            sequencer_endpoint: first,
            indexer_mode: CoreEndpointMode::Rpc,
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

pub(crate) fn storage_rest_source(args: &Args) -> Result<RestSource<'_>> {
    rest_source(
        args,
        DEFAULT_STORAGE_REST_ENDPOINT,
        "storage",
        "Storage REST data actions",
    )
}

pub(crate) fn delivery_rest_source(args: &Args) -> Result<RestSource<'_>> {
    rest_source(
        args,
        DEFAULT_DELIVERY_REST_ENDPOINT,
        "delivery",
        "Delivery REST message actions",
    )
}

pub(crate) fn require_mutating_diagnostics(args: &Args, index: usize, label: &str) -> Result<()> {
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
    let normalized = match source_name {
        "storage" => StorageSourceMode::from_token(mode).effective().as_str(),
        "delivery" => DeliverySourceMode::from_token(mode).effective().as_str(),
        _ => "unsupported",
    };
    match normalized {
        "rest" => Ok(RestSource {
            endpoint: args.optional_string(1).unwrap_or(default_endpoint),
            next_index: 2,
        }),
        "module" => {
            bail!("{action_name} require {source_name} REST source, not module")
        }
        "metrics" => bail!("{action_name} require {source_name} REST source, not metrics"),
        _ => bail!("{source_name} source mode `{mode}` is not supported"),
    }
}

pub(crate) async fn storage_rest_upload(
    endpoint: &str,
    path: &str,
    block_size: u64,
) -> Result<Value> {
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

pub(crate) fn delivery_store_query_url(
    endpoint: &str,
    store_query: DeliveryStoreQuery<'_>,
) -> Result<Url> {
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

pub(crate) async fn raw_http_json_url(url: &str) -> Result<Value> {
    let text = send_text(reqwest::Client::new().get(url), url).await?;
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::Null);
    }
    serde_json::from_str(trimmed)
        .with_context(|| format!("invalid JSON response: {}", response_excerpt(trimmed)))
}

pub(crate) async fn rest_json_request(
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

pub(crate) async fn rest_empty_request(
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

fn response_excerpt_bytes(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).chars().take(400).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn source_policy_bridge_exposes_defaults_profiles_and_modes() -> Result<()> {
        let bridge = InspectorBridge::new()?;

        let value = bridge.call_module(INSPECTOR_MODULE, "sourcePolicy", json!([]))?;

        if value
            .pointer("/defaults/storage_rest_endpoint")
            .and_then(Value::as_str)
            != Some(DEFAULT_STORAGE_REST_ENDPOINT)
            || value
                .pointer("/defaults/delivery_rest_endpoint")
                .and_then(Value::as_str)
                != Some(DEFAULT_DELIVERY_REST_ENDPOINT)
        {
            bail!("unexpected source policy defaults: {value}");
        }

        let Some(profiles) = value.get("network_profiles").and_then(Value::as_array) else {
            bail!("source policy missing network profiles: {value}");
        };
        if !profiles
            .iter()
            .any(|profile| profile.get("id").and_then(Value::as_str) == Some("default"))
        {
            bail!("source policy missing default profile: {value}");
        }

        let Some(storage_modes) = value
            .pointer("/source_modes/storage")
            .and_then(Value::as_array)
        else {
            bail!("source policy missing storage modes: {value}");
        };
        if !storage_modes
            .iter()
            .any(|mode| mode.get("key").and_then(Value::as_str) == Some("module"))
        {
            bail!("source policy missing storage module mode: {value}");
        }
        Ok(())
    }

    #[test]
    fn source_endpoint_accepts_existing_rpc_shape() -> Result<()> {
        let args = Args::new(json!(["http://127.0.0.1:8080", 1, 2]))?;
        let source = args.source_endpoint(0, "node endpoint")?;

        if source.mode != CoreEndpointMode::Rpc
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

        if source.mode != CoreEndpointMode::Module
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

        if sources.execution_mode != CoreEndpointMode::Rpc
            || sources.sequencer_endpoint != "https://testnet.lez.logos.co/"
            || sources.indexer_mode != CoreEndpointMode::Module
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
                page_size: 100,
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
        let cancel_requested = bridge.node_operations.insert_test_running_operation(
            "existing",
            "storage",
            "storageDownloadToUrl",
            true,
        );

        let value =
            bridge.call_module(INSPECTOR_MODULE, "nodeOperationCancel", json!(["existing"]))?;

        if value.get("status").and_then(Value::as_str) != Some("canceling") {
            bail!("expected canceling status: {value}");
        }
        if !cancel_requested.load(std::sync::atomic::Ordering::Relaxed) {
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
        let request = crate::node_operations::node_operation_request_from_value(json!({
            "domain": "storage",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8080/api/storage/v1",
            "method": "storageDownloadManifest",
            "args": ["http://127.0.0.1:8080/api/storage/v1", "z-storage"]
        }))?;

        if request.args() != &json!(["rest", "http://127.0.0.1:8080/api/storage/v1", "z-storage"]) {
            bail!("unexpected normalized args: {}", request.args());
        }
        Ok(())
    }

    #[test]
    fn node_operation_request_normalizes_delivery_endpoint_first_args() -> Result<()> {
        let request = crate::node_operations::node_operation_request_from_value(json!({
            "domain": "delivery",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8645",
            "method": "deliverySend",
            "mutatingEnabled": true,
            "args": ["http://127.0.0.1:8645", "/waku/2/default/proto", "hello"]
        }))?;

        if request.args()
            != &json!([
                "rest",
                "http://127.0.0.1:8645",
                true,
                "/waku/2/default/proto",
                "hello"
            ])
        {
            bail!("unexpected normalized args: {}", request.args());
        }
        Ok(())
    }

    #[test]
    fn node_operation_request_keeps_delivery_store_query_read_only_args() -> Result<()> {
        let request = crate::node_operations::node_operation_request_from_value(json!({
            "domain": "delivery",
            "sourceMode": "rest",
            "endpoint": "http://127.0.0.1:8645",
            "method": "deliveryStoreQuery",
            "args": ["peer-a", "/topic/1/a/proto", "/waku/2/default-waku/proto", "cursor-a", 10, true, true]
        }))?;

        if request.args()
            != &json!([
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
            bail!("unexpected normalized args: {}", request.args());
        }
        Ok(())
    }

    #[test]
    fn node_operation_start_rejects_second_storage_download() -> Result<()> {
        let bridge = InspectorBridge::new()?;
        bridge.node_operations.insert_test_running_operation(
            "storage-download-existing",
            "storage",
            "storageDownloadToUrl",
            true,
        );

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
        let result = bridge.call_module(INSPECTOR_MODULE, "localWalletCreateAccount", json!([]));

        let Err(error) = result else {
            bail!("expected wallet operation to fail before execution");
        };
        if !error
            .to_string()
            .contains("wallet account creation requires explicit confirmation")
        {
            bail!("unexpected error: {error:#}");
        }
        let operations_len = bridge.node_operations.len()?;
        if operations_len != 0 {
            bail!("expected legacy operation registry cleanup, found {operations_len}",);
        }
        Ok(())
    }
}
