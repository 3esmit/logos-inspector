use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    AccountTransactionSummary, IndexerBlockReport, ProbeReport, TransactionIdlInspectionReport,
    TransactionSummary, account_lookup, account_lookup_with_idl, bedrock_wallet_balance,
    blockchain, channels, decode_account_data_hex_with_idl, decode_event_data_hex_with_idl,
    indexer_block_by_hash, indexer_blocks, indexer_health, indexer_status,
    indexer_transfer_recipients, inspect_transaction_summary_with_idl, last_sequencer_block_id,
    local_wallet_deploy_program, local_wallet_profile_status, logoscore,
    modules::{
        blockchain_module_report, capabilities_report, delivery_report, delivery_source_report,
        logoscore_status_report, modules_report, storage_report, storage_source_report,
    },
    normalize_program_id_hex, overview, program_file_info, raw_http_json,
    raw_json_rpc_optional_result, raw_rpc_report, sequencer_block, sequencer_blocks,
    sequencer_program_ids, sequencer_transaction, sequencer_transaction_inspection,
    sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
    sequencer_transaction_trace_with_idl,
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
const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";
const DEFAULT_STORAGE_REST_ENDPOINT: &str = "http://127.0.0.1:8080/api/storage/v1";

#[derive(Debug, serde::Serialize)]
struct BridgeResponse {
    ok: bool,
    value: Value,
    text: String,
    error: String,
}

pub struct InspectorBridge {
    runtime: Runtime,
}

impl InspectorBridge {
    pub fn new() -> Result<Self> {
        Ok(Self {
            runtime: Runtime::new().context("failed to create tokio runtime")?,
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
                if source.mode == SourceMode::Module {
                    return self.blockchain_module_blocks(slot_from, slot_to);
                }
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
                if source.mode == SourceMode::Module {
                    return self.blockchain_module_live_blocks(slot_from, slot_to, limit);
                }
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
                if source.mode == SourceMode::Module {
                    return self
                        .blockchain_module_block(args.string(source.next_index, "block id")?);
                }
                to_value(self.runtime.block_on(blockchain::blockchain_block(
                    source.endpoint,
                    args.string(source.next_index, "block id")?,
                ))?)
            }
            "blockchainTransaction" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "node endpoint")?;
                if source.mode == SourceMode::Module {
                    return self.blockchain_module_transaction(
                        args.string(source.next_index, "transaction id")?,
                    );
                }
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
                if source.mode == SourceMode::Module {
                    return self.indexer_module_health();
                }
                let health = self.runtime.block_on(indexer_health(source.endpoint))?;
                Ok(json!({
                    "status": "healthy",
                    "health": health,
                }))
            }
            "indexerStatus" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                if source.mode == SourceMode::Module {
                    return self.indexer_module_status();
                }
                to_value(self.runtime.block_on(indexer_status(source.endpoint))?)
            }
            "indexerFinalizedHead" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                if source.mode == SourceMode::Module {
                    return self.indexer_module_finalized_head();
                }
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
                if source.mode == SourceMode::Module {
                    return self.indexer_module_blocks(before, limit);
                }
                to_value(
                    self.runtime
                        .block_on(indexer_blocks(source.endpoint, before, limit))?,
                )
            }
            "indexerBlockByHash" => {
                let args = Args::new(args)?;
                let source = args.source_endpoint(0, "indexer endpoint")?;
                if source.mode == SourceMode::Module {
                    return self.indexer_module_block_by_hash(
                        args.string(source.next_index, "block header hash")?,
                    );
                }
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
                if source.mode == SourceMode::Module {
                    return self.indexer_module_transfer_recipients(before, limit);
                }
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
            "localWalletDeployProgram" => {
                let args = Args::new(args)?;
                to_value(local_wallet_deploy_program(
                    args.value(0)
                        .cloned()
                        .context("local wallet profile is required")?,
                    args.string(1, "program path")?,
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
            "modules" => to_value(modules_report()),
            "logoscoreStatus" => to_value(logoscore_status_report()),
            "blockchainModuleReport" => {
                let args = Args::new(args)?;
                to_value(blockchain_module_report(args.optional_string(0)))
            }
            "storageReport" => {
                let args = Args::new(args)?;
                to_value(storage_report(args.optional_string(0), false))
            }
            "storageSourceReport" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(storage_source_report(
                    args.optional_string(0).unwrap_or("module"),
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
                    args.optional_string(0).unwrap_or("module"),
                    args.optional_string(1),
                    args.optional_string(2),
                    args.optional_string(3),
                )))
            }
            "storageManifests" => self.storage_manifests(args),
            "storageExists" => self.storage_exists(args),
            "storageDownloadManifest" => {
                let args = Args::new(args)?;
                self.require_storage_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    STORAGE_MODULE,
                    "downloadManifest",
                    json!([args.string(2, "CID")?]),
                )
            }
            "storageFetch" => {
                let args = Args::new(args)?;
                self.require_storage_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(STORAGE_MODULE, "fetch", json!([args.string(2, "CID")?]))
            }
            "storageUploadUrl" => {
                let args = Args::new(args)?;
                self.require_storage_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    STORAGE_MODULE,
                    "uploadUrl",
                    json!([
                        args.string(2, "file path or URL")?,
                        args.u64(3, "chunk size")?
                    ]),
                )
            }
            "storageDownloadToUrl" => {
                let args = Args::new(args)?;
                self.require_storage_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    STORAGE_MODULE,
                    "downloadToUrl",
                    json!([
                        args.string(2, "CID")?,
                        args.string(3, "destination path")?,
                        args.optional_bool(4),
                        args.u64(5, "chunk size")?
                    ]),
                )
            }
            "storageRemove" => {
                let args = Args::new(args)?;
                self.require_storage_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    STORAGE_MODULE,
                    "remove",
                    json!([args.string(2, "CID")?]),
                )
            }
            "deliveryCreateNode" => {
                let args = Args::new(args)?;
                self.require_delivery_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    DELIVERY_MODULE,
                    "createNode",
                    json!([args.string(2, "node configuration")?]),
                )
            }
            "deliveryStart" => {
                let args = Args::new(args)?;
                self.require_delivery_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(DELIVERY_MODULE, "start", json!([]))
            }
            "deliveryStop" => {
                let args = Args::new(args)?;
                self.require_delivery_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(DELIVERY_MODULE, "stop", json!([]))
            }
            "deliverySubscribe" => {
                let args = Args::new(args)?;
                self.require_delivery_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    DELIVERY_MODULE,
                    "subscribe",
                    json!([args.string(2, "content topic")?]),
                )
            }
            "deliveryUnsubscribe" => {
                let args = Args::new(args)?;
                self.require_delivery_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    DELIVERY_MODULE,
                    "unsubscribe",
                    json!([args.string(2, "content topic")?]),
                )
            }
            "deliverySend" => {
                let args = Args::new(args)?;
                self.require_delivery_module_source(args.optional_string(0).unwrap_or("module"))?;
                self.call_logoscore_module(
                    DELIVERY_MODULE,
                    "send",
                    json!([args.string(2, "content topic")?, args.string(3, "payload")?]),
                )
            }
            "capabilitiesReport" => to_value(capabilities_report()),
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
        if account_args.execution_mode == SourceMode::Module
            && account_args.indexer_mode != SourceMode::Module
        {
            bail!(
                "{EXECUTION_MODULE} does not expose sequencer account reads; set execution source to RPC or use the indexer module source"
            );
        }
        if account_args.indexer_mode == SourceMode::Module {
            return self.indexer_module_account(account_args.account);
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

    fn call_logoscore_module_value(
        &self,
        module: &str,
        method: &str,
        args: Value,
    ) -> Result<Value> {
        let args = Args::new(args)?
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| value.to_string())
            })
            .collect::<Vec<_>>();
        let output = logoscore::call(module, method, &args)?;
        parse_json_string_value(unwrap_logoscore_call_value(output.value))
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
        if source.mode == SourceMode::Rpc {
            return to_value(
                self.runtime
                    .block_on(blockchain::blockchain_node_report(source.endpoint)),
            );
        }

        let cryptarchia_info = self
            .call_logoscore_module_value(BLOCKCHAIN_MODULE, "get_cryptarchia_info", json!([]))
            .map(normalize_module_cryptarchia_info);
        let peer_id = self.call_logoscore_module_value(BLOCKCHAIN_MODULE, "get_peer_id", json!([]));
        to_value(blockchain::BlockchainNodeReport {
            endpoint: BLOCKCHAIN_MODULE.to_owned(),
            cryptarchia_info: ProbeReport::from_result(
                "cryptarchia info",
                "logoscore call blockchain_module get_cryptarchia_info",
                cryptarchia_info,
            ),
            headers: ProbeReport::err(
                "headers",
                "blockchain_module",
                "blockchain_module does not expose cryptarchia headers",
            ),
            network_info: ProbeReport::from_result(
                "network info",
                "logoscore call blockchain_module get_peer_id",
                peer_id.map(|value| json!({ "peer_id": value })),
            ),
            mantle_metrics: ProbeReport::err(
                "mantle metrics",
                "blockchain_module",
                "blockchain_module does not expose mantle metrics",
            ),
        })
    }

    fn blockchain_module_blocks(&self, slot_from: u64, slot_to: u64) -> Result<Value> {
        if slot_from > slot_to {
            bail!("slot_from must be less than or equal to slot_to");
        }
        self.call_logoscore_module_value(
            BLOCKCHAIN_MODULE,
            "get_blocks",
            json!([slot_from, slot_to]),
        )
    }

    fn blockchain_module_live_blocks(
        &self,
        slot_from: u64,
        slot_to: u64,
        limit: u64,
    ) -> Result<Value> {
        let blocks = self.blockchain_module_blocks(slot_from, slot_to)?;
        let blocks = value_array(blocks)
            .into_iter()
            .take(limit.clamp(1, 500) as usize)
            .collect::<Vec<_>>();
        to_value(blockchain::BlockchainLiveBlocksReport {
            endpoint: BLOCKCHAIN_MODULE.to_owned(),
            source: "blockchain_module.get_blocks".to_owned(),
            blocks,
            unknown_events: Vec::new(),
        })
    }

    fn blockchain_module_block(&self, block_id: &str) -> Result<Value> {
        self.call_logoscore_module_value(BLOCKCHAIN_MODULE, "get_block", json!([block_id]))
    }

    fn blockchain_module_transaction(&self, tx_hash: &str) -> Result<Value> {
        self.call_logoscore_module_value(BLOCKCHAIN_MODULE, "get_transaction", json!([tx_hash]))
    }

    fn indexer_module_health(&self) -> Result<Value> {
        let status = self.indexer_module_status_value()?;
        Ok(json!({
            "status": "healthy",
            "health": status,
        }))
    }

    fn indexer_module_status(&self) -> Result<Value> {
        to_value(crate::indexer::summarize_indexer_status_response(&json!({
            "result": self.indexer_module_status_value()?
        })))
    }

    fn indexer_module_status_value(&self) -> Result<Value> {
        let value = self.call_logoscore_module_value(INDEXER_MODULE, "getStatus", json!([]))?;
        if value.as_str().is_some_and(|text| text.trim().is_empty()) {
            return Ok(json!({
                "state": "unavailable",
                "lastError": "indexer module returned empty status",
            }));
        }
        Ok(value)
    }

    fn indexer_module_finalized_head(&self) -> Result<Value> {
        self.call_logoscore_module_value(INDEXER_MODULE, "getLastFinalizedBlockId", json!([]))
    }

    fn indexer_module_block_reports(
        &self,
        before: Option<u64>,
        limit: u64,
    ) -> Result<Vec<IndexerBlockReport>> {
        let before = before.map_or_else(|| "0".to_owned(), |value| value.to_string());
        let blocks =
            self.call_logoscore_module_value(INDEXER_MODULE, "getBlocks", json!([before, limit]))?;
        let blocks = blocks
            .as_array()
            .context("lez_indexer_module getBlocks result was not an array")?;
        Ok(blocks
            .iter()
            .map(crate::indexer::summarize_indexer_block)
            .collect())
    }

    fn indexer_module_blocks(&self, before: Option<u64>, limit: u64) -> Result<Value> {
        to_value(self.indexer_module_block_reports(before, limit)?)
    }

    fn indexer_module_block_by_hash(&self, header_hash: &str) -> Result<Value> {
        let value = self.call_logoscore_module_value(
            INDEXER_MODULE,
            "getBlockByHash",
            json!([header_hash]),
        )?;
        if value.is_null() {
            return Ok(Value::Null);
        }
        to_value(crate::indexer::summarize_indexer_block(&value))
    }

    fn indexer_module_account(&self, account: &str) -> Result<Value> {
        self.call_logoscore_module_value(INDEXER_MODULE, "getAccount", json!([account]))
    }

    fn indexer_module_transfer_recipients(&self, before: Option<u64>, limit: u64) -> Result<Value> {
        let blocks = self.indexer_module_block_reports(before, limit)?;
        to_value(crate::transfers::TransferActivityPage {
            next_before_block: crate::indexer::next_indexer_blocks_cursor(&blocks),
            recipients: crate::transfers::transfer_recipient_summaries_from_blocks(&blocks),
        })
    }

    fn execution_head(&self, source: &SourceEndpoint<'_>) -> Result<Value> {
        if source.mode == SourceMode::Rpc {
            return to_value(
                self.runtime
                    .block_on(last_sequencer_block_id(source.endpoint))?,
            );
        }
        self.call_logoscore_module_value(EXECUTION_MODULE, "get_current_block_height", json!([]))
    }

    fn storage_manifests(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        if storage_source_is_rest(args.optional_string(0).unwrap_or("module")) {
            return to_value(
                self.runtime.block_on(raw_http_json(
                    args.optional_string(1)
                        .unwrap_or(DEFAULT_STORAGE_REST_ENDPOINT),
                    "/data",
                ))?,
            );
        }
        self.require_storage_module_source(args.optional_string(0).unwrap_or("module"))?;
        self.call_logoscore_module(STORAGE_MODULE, "manifests", json!([]))
    }

    fn storage_exists(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let cid = args.string(2, "CID")?;
        if storage_source_is_rest(args.optional_string(0).unwrap_or("module")) {
            return to_value(
                self.runtime.block_on(raw_http_json(
                    args.optional_string(1)
                        .unwrap_or(DEFAULT_STORAGE_REST_ENDPOINT),
                    &format!("/data/{cid}/exists"),
                ))?,
            );
        }
        self.require_storage_module_source(args.optional_string(0).unwrap_or("module"))?;
        self.call_logoscore_module(STORAGE_MODULE, "exists", json!([cid]))
    }

    fn require_storage_module_source(&self, source_mode: &str) -> Result<()> {
        if storage_source_is_module(source_mode) {
            return Ok(());
        }
        bail!(
            "storage operation requires the storage module source; current source mode is `{}`",
            source_mode.trim()
        )
    }

    fn require_delivery_module_source(&self, source_mode: &str) -> Result<()> {
        if delivery_source_is_module(source_mode) {
            return Ok(());
        }
        bail!(
            "delivery operation requires the delivery module source; current source mode is `{}`",
            source_mode.trim()
        )
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

fn storage_source_is_module(source_mode: &str) -> bool {
    matches!(
        source_mode.trim().to_ascii_lowercase().as_str(),
        "module" | "basecamp" | "basecamp-module" | "basecamp module"
    )
}

fn storage_source_is_rest(source_mode: &str) -> bool {
    matches!(
        source_mode.trim().to_ascii_lowercase().as_str(),
        "rest" | "standalone-rest" | "standalone rest" | "direct-rest"
    )
}

fn delivery_source_is_module(source_mode: &str) -> bool {
    matches!(
        source_mode.trim().to_ascii_lowercase().as_str(),
        "module" | "basecamp" | "basecamp-module" | "basecamp module"
    )
}

fn unwrap_logoscore_call_value(value: Value) -> Value {
    if let Some(inner) = value
        .get("result")
        .and_then(|result| result.get("value"))
        .cloned()
    {
        inner
    } else {
        value
    }
}

fn parse_json_string_value(value: Value) -> Result<Value> {
    let Value::String(text) = value else {
        return Ok(value);
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Value::String(text));
    }
    Ok(serde_json::from_str(trimmed).unwrap_or(Value::String(text)))
}

fn normalize_module_cryptarchia_info(value: Value) -> Value {
    if value
        .get("cryptarchia_info")
        .is_some_and(|value| !value.is_null())
    {
        return value;
    }
    json!({ "cryptarchia_info": value })
}

fn value_array(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        Value::Object(mut object) => object
            .remove("blocks")
            .or_else(|| object.remove("items"))
            .or_else(|| object.remove("result"))
            .and_then(|value| match value {
                Value::Array(items) => Some(items),
                _ => None,
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

fn to_value(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).context("failed to serialize bridge response")
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
    fn unwrap_logoscore_call_value_returns_nested_value() {
        let value = unwrap_logoscore_call_value(json!({
            "result": {
                "value": {
                    "height": 7
                }
            }
        }));

        assert_eq!(value.pointer("/height").and_then(Value::as_u64), Some(7));
    }
}
