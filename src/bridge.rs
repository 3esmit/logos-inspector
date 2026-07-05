use std::path::Path;

use anyhow::{Context as _, Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use reqwest::{Method, StatusCode, header};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    AccountTransactionSummary, TransactionIdlInspectionReport, TransactionSummary, account_lookup,
    account_lookup_with_idl, bedrock_wallet_balance, blockchain, channels,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl, indexer_block_by_hash,
    indexer_blocks, indexer_health, indexer_status, indexer_transfer_recipients,
    inspect_transaction_summary_with_idl, last_sequencer_block_id, local_wallet_accounts,
    local_wallet_profile_status, local_wallet_sync_private, logoscore,
    modules::{
        blockchain_module_report, delivery_source_report, logoscore_status_report,
        storage_source_report,
    },
    normalize_program_id_hex, overview, program_file_info, raw_http_json,
    raw_json_rpc_optional_result, raw_rpc_report, response_excerpt, sequencer_block,
    sequencer_blocks, sequencer_program_ids, sequencer_transaction,
    sequencer_transaction_inspection, sequencer_transaction_inspection_with_idl,
    sequencer_transaction_trace, sequencer_transaction_trace_with_idl,
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
            "localWalletDeployProgram" => {
                bail!("wallet program deployment is disabled by the read-only wallet policy")
            }
            "localWalletSyncPrivate" => {
                let args = Args::new(args)?;
                to_value(local_wallet_sync_private(
                    args.value(0)
                        .cloned()
                        .context("local wallet profile is required")?,
                )?)
            }
            "localWalletAccounts" => {
                let args = Args::new(args)?;
                to_value(local_wallet_accounts(
                    args.value(0)
                        .cloned()
                        .context("local wallet profile is required")?,
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
            "storageReport" => bail!(
                "storage_module does not satisfy Inspector storage requirements; use storageSourceReport over REST"
            ),
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
            "deliveryReport" => bail!(
                "delivery_module does not satisfy Inspector messaging diagnostics; use deliverySourceReport over REST"
            ),
            "deliverySourceReport" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(delivery_source_report(
                    args.optional_string(0).unwrap_or("rest"),
                    args.optional_string(1),
                    args.optional_string(2),
                )))
            }
            "storageManifests" => self.storage_manifests(args),
            "storageExists" => self.storage_exists(args),
            "storageDownloadManifest" => self.storage_download_manifest(args),
            "storageFetch" => self.storage_fetch(args),
            "storageUploadUrl" => self.storage_upload_path(args),
            "storageDownloadToUrl" => self.storage_download_to_path(args),
            "storageRemove" => self.storage_remove(args),
            "deliveryCreateNode" => self.delivery_module_action(args, "createNode"),
            "deliveryStart" => self.delivery_module_action(args, "start"),
            "deliveryStop" => self.delivery_module_action(args, "stop"),
            "deliverySubscribe" => self.delivery_subscription(args, Method::POST, "subscribe"),
            "deliveryUnsubscribe" => {
                self.delivery_subscription(args, Method::DELETE, "unsubscribe")
            }
            "deliverySend" => self.delivery_send(args),
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

    fn storage_manifests(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        to_value(
            self.runtime
                .block_on(raw_http_json(source.endpoint, "/data"))?,
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

    fn storage_download_manifest(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        let cid_index = if matches!(args.value(source.next_index), Some(Value::Bool(_))) {
            source.next_index + 1
        } else {
            source.next_index
        };
        let cid = args.string(cid_index, "CID")?;
        to_value(self.runtime.block_on(raw_http_json(
            source.endpoint,
            &format!("/data/{cid}/network/manifest"),
        ))?)
    }

    fn storage_fetch(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        require_mutating_diagnostics(&args, source.next_index, "storage network action")?;
        let cid = args.string(source.next_index + 1, "CID")?;
        self.runtime
            .block_on(rest_json_request(
                Method::POST,
                source.endpoint,
                &format!("/data/{cid}/network"),
                None,
            ))
            .with_context(|| format!("failed to start storage network fetch for {cid}"))
    }

    fn storage_upload_path(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
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
        self.runtime
            .block_on(storage_rest_upload(source.endpoint, path, block_size))
            .with_context(|| format!("failed to upload `{path}` through storage REST"))
    }

    fn storage_download_to_path(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        require_mutating_diagnostics(&args, source.next_index, "storage download action")?;
        let cid = args.string(source.next_index + 1, "CID")?;
        let path = args.string(source.next_index + 2, "download path")?;
        let local_only = args.optional_bool(source.next_index + 3);
        self.runtime
            .block_on(storage_rest_download(
                source.endpoint,
                cid,
                path,
                local_only,
            ))
            .with_context(|| format!("failed to download storage CID {cid} to `{path}`"))
    }

    fn storage_remove(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        let source = storage_rest_source(&args)?;
        require_mutating_diagnostics(&args, source.next_index, "storage remove action")?;
        let cid = args.string(source.next_index + 1, "CID")?;
        self.runtime
            .block_on(rest_empty_request(
                Method::DELETE,
                source.endpoint,
                &format!("/data/{cid}"),
                None,
            ))
            .with_context(|| format!("failed to remove storage CID {cid}"))?;
        Ok(json!({
            "removed": true,
            "cid": cid,
            "endpoint": source.endpoint,
        }))
    }

    fn delivery_subscription(
        &self,
        args: Value,
        method: Method,
        module_method: &str,
    ) -> Result<Value> {
        let args = Args::new(args)?;
        if let Some(module_args) = delivery_module_message_args(&args)? {
            require_mutating_diagnostics(&args, module_args.flag_index, "delivery message action")?;
            return self.call_logoscore_module(
                DELIVERY_MODULE,
                module_method,
                Value::Array(module_args.values),
            );
        }
        let source = delivery_rest_source(&args)?;
        require_mutating_diagnostics(&args, source.next_index, "delivery message action")?;
        let topic = args.string(source.next_index + 1, "content topic")?;
        self.runtime
            .block_on(rest_empty_request(
                method.clone(),
                source.endpoint,
                "/relay/v1/auto/subscriptions",
                Some(json!([topic])),
            ))
            .with_context(|| format!("failed to update relay subscription for {topic}"))?;
        Ok(json!({
            "subscribed": method == Method::POST,
            "contentTopic": topic,
            "endpoint": source.endpoint,
        }))
    }

    fn delivery_send(&self, args: Value) -> Result<Value> {
        let args = Args::new(args)?;
        if let Some(module_args) = delivery_module_message_args(&args)? {
            require_mutating_diagnostics(&args, module_args.flag_index, "delivery message action")?;
            return self.call_logoscore_module(
                DELIVERY_MODULE,
                "send",
                Value::Array(module_args.values),
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
        self.runtime
            .block_on(rest_empty_request(
                Method::POST,
                source.endpoint,
                "/relay/v1/auto/messages",
                Some(body),
            ))
            .with_context(|| format!("failed to send relay message on {topic}"))?;
        Ok(json!({
            "sent": true,
            "contentTopic": topic,
            "bytes": payload.len(),
            "endpoint": source.endpoint,
        }))
    }

    fn delivery_module_action(&self, args: Value, method: &str) -> Result<Value> {
        let args = Args::new(args)?;
        let start_index = if let Some(source) = args.optional_string(0).filter(|source| {
            is_delivery_source_token(source) || is_delivery_module_source_token(source)
        }) {
            if !is_delivery_module_source_token(source) {
                bail!("delivery node lifecycle actions require delivery module source");
            }
            3
        } else {
            0
        };
        let call_args = args.iter().skip(start_index).cloned().collect::<Vec<_>>();
        self.call_logoscore_module(DELIVERY_MODULE, method, Value::Array(call_args))
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
    let bytes = tokio::fs::read(path)
        .await
        .with_context(|| format!("failed to read upload file `{path}`"))?;
    let filename = Path::new(path)
        .file_name()
        .map(|value| value.to_string_lossy().into_owned())
        .filter(|value| !value.is_empty());
    let mut request = reqwest::Client::new()
        .post(rest_url(endpoint, &format!("/data?blockSize={block_size}")))
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(bytes);
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
        "bytes": tokio::fs::metadata(path).await.map(|metadata| metadata.len()).unwrap_or(0),
        "endpoint": endpoint,
    }))
}

async fn storage_rest_download(
    endpoint: &str,
    cid: &str,
    path: &str,
    local_only: bool,
) -> Result<Value> {
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
        .context("failed to read storage download response body")?;
    if !status.is_success() {
        bail!(
            "storage download failed with status {status}: {}",
            response_excerpt_bytes(&bytes)
        );
    }
    tokio::fs::write(path, &bytes)
        .await
        .with_context(|| format!("failed to write download file `{path}`"))?;
    Ok(json!({
        "cid": cid,
        "path": path,
        "bytes": bytes.len(),
        "source": if local_only { "local" } else { "network" },
        "endpoint": endpoint,
    }))
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
}
