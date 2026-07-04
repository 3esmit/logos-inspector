use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    AccountTransactionSummary, TransactionIdlInspectionReport, TransactionSummary, account_lookup,
    account_lookup_with_idl, bedrock_wallet_balance, blockchain, channels,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl, indexer_block_by_hash,
    indexer_blocks, indexer_health, indexer_transfer_recipients,
    inspect_transaction_summary_with_idl, last_sequencer_block_id, local_wallet_deploy_program,
    local_wallet_profile_status, logoscore,
    modules::{
        blockchain_module_report, capabilities_report, delivery_report, delivery_source_report,
        logoscore_status_report, modules_report, storage_report, storage_source_report,
    },
    normalize_program_id_hex, overview, program_file_info, raw_http_json,
    raw_json_rpc_optional_result, raw_rpc_report, sequencer_block, sequencer_program_ids,
    sequencer_transaction, sequencer_transaction_inspection,
    sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
    sequencer_transaction_trace_with_idl,
    spel::spel_idl_report,
};

pub const INSPECTOR_MODULE: &str = "logos_inspector";
const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";
const DEFAULT_STORAGE_REST_ENDPOINT: &str = "http://127.0.0.1:8080/api/storage/v1";

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
                to_value(self.runtime.block_on(last_sequencer_block_id(
                    args.string(0, "sequencer endpoint")?,
                ))?)
            }
            "programs" => {
                let args = Args::new(args)?;
                to_value(
                    self.runtime
                        .block_on(sequencer_program_ids(args.string(0, "sequencer endpoint")?))?,
                )
            }
            "block" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(sequencer_block(
                    args.string(0, "sequencer endpoint")?,
                    args.u64(1, "block id")?,
                ))?)
            }
            "transaction" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(sequencer_transaction(
                    args.string(0, "sequencer endpoint")?,
                    args.string(1, "transaction hash")?,
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
                to_value(self.runtime.block_on(blockchain::blockchain_node_report(
                    args.string(0, "node endpoint")?,
                )))
            }
            "blockchainBlocks" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(blockchain::blockchain_blocks(
                    args.string(0, "node endpoint")?,
                    args.u64(1, "slot from")?,
                    args.u64(2, "slot to")?,
                ))?)
            }
            "blockchainBlock" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(blockchain::blockchain_block(
                    args.string(0, "node endpoint")?,
                    args.string(1, "block id")?,
                ))?)
            }
            "blockchainTransaction" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(blockchain::blockchain_transaction(
                    args.string(0, "node endpoint")?,
                    args.string(1, "transaction id")?,
                ))?)
            }
            "channelScan" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(channels::channel_scan(
                    args.string(0, "node endpoint")?,
                    args.u64(1, "slot from")?,
                    args.u64(2, "slot to")?,
                ))?)
            }
            "channelState" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(channels::channel_state(
                    args.string(0, "node endpoint")?,
                    args.string(1, "channel id")?,
                ))?)
            }
            "indexerHealth" => {
                let args = Args::new(args)?;
                let health = self
                    .runtime
                    .block_on(indexer_health(args.string(0, "indexer endpoint")?))?;
                Ok(json!({
                    "status": "healthy",
                    "health": health,
                }))
            }
            "indexerFinalizedHead" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(raw_json_rpc_optional_result(
                    args.string(0, "indexer endpoint")?,
                    "getLastFinalizedBlockId",
                    Value::Array(vec![]),
                ))?)
            }
            "indexerBlocks" => {
                let args = Args::new(args)?;
                let before = args.value(1).and_then(Value::as_u64);
                let limit = args.value(2).and_then(Value::as_u64).unwrap_or(10).min(50);
                to_value(self.runtime.block_on(indexer_blocks(
                    args.string(0, "indexer endpoint")?,
                    before,
                    limit,
                ))?)
            }
            "indexerBlockByHash" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(indexer_block_by_hash(
                    args.string(0, "indexer endpoint")?,
                    args.string(1, "block header hash")?,
                ))?)
            }
            "indexerTransferRecipients" => {
                let args = Args::new(args)?;
                let before = args.value(1).and_then(Value::as_u64);
                let limit = args.value(2).and_then(Value::as_u64).unwrap_or(50).min(50);
                to_value(self.runtime.block_on(indexer_transfer_recipients(
                    args.string(0, "indexer endpoint")?,
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
        let endpoint = args.string(0, "sequencer endpoint")?;
        let hash = args.string(1, "transaction hash")?;
        let idl = args.optional_string(2);
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
        let endpoint = args.string(0, "sequencer endpoint")?;
        let hash = args.string(1, "transaction hash")?;
        let idl = args.optional_string(2);
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
        let sequencer = args.string(0, "sequencer endpoint")?;
        let indexer = args.string(1, "indexer endpoint")?;
        let account = args.string(2, "account id")?;
        let idl = args.optional_string(3);
        let mut value = if let Some(idl) = idl {
            to_value(self.runtime.block_on(account_lookup_with_idl(
                sequencer,
                indexer,
                account,
                idl,
                args.optional_string(4),
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

fn to_value(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).context("failed to serialize bridge response")
}

fn load_idl_state() -> Result<Value> {
    let path = idl_state_path()?;
    if !path.is_file() {
        return Ok(default_idl_state());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read IDL state from {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse IDL state from {}", path.display()))
}

#[derive(Debug, Clone)]
struct RegisteredIdlEntry {
    program_id_hex: String,
    json: String,
}

fn registered_idl_entries() -> Result<Vec<RegisteredIdlEntry>> {
    let state = load_idl_state()?;
    Ok(state
        .get("idls")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| {
            let json = entry.get("json").and_then(Value::as_str)?.trim();
            if json.is_empty() {
                return None;
            }
            let program_id_hex = registered_idl_program_id_hex(entry);
            if program_id_hex.is_empty() {
                return None;
            }
            Some(RegisteredIdlEntry {
                program_id_hex,
                json: json.to_owned(),
            })
        })
        .collect())
}

fn registered_idl_program_id_hex(entry: &Value) -> String {
    entry
        .get("programIdHex")
        .or_else(|| entry.get("program_id_hex"))
        .and_then(Value::as_str)
        .and_then(normalized_program_id_hex_text)
        .or_else(|| {
            entry
                .get("programId")
                .or_else(|| entry.get("program_id"))
                .and_then(Value::as_str)
                .and_then(normalized_program_id_hex_text)
        })
        .unwrap_or_default()
}

fn normalized_program_id_hex_text(value: &str) -> Option<String> {
    normalize_program_id_hex(value)
        .ok()
        .filter(|text| !text.is_empty())
}

fn decode_transaction_summary_with_idls(
    summary: &TransactionSummary,
    idl_entries: &[RegisteredIdlEntry],
) -> Option<TransactionIdlInspectionReport> {
    let summary_program_id = summary
        .program_id_hex
        .as_deref()
        .and_then(normalized_program_id_hex_text)?;
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

fn save_idl_state(state: &Value) -> Result<Value> {
    let path = idl_state_path()?;
    let parent = path
        .parent()
        .context("IDL state path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    let text = serde_json::to_string_pretty(state).context("failed to serialize IDL state")?;
    fs::write(&path, text)
        .with_context(|| format!("failed to write IDL state to {}", path.display()))?;
    Ok(json!({
        "saved": true,
        "path": path.display().to_string(),
    }))
}

fn default_idl_state() -> Value {
    json!({
        "version": 1,
        "idls": [],
        "account_idl_selections": {},
    })
}

fn idl_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("idls.json"))
}

fn load_wallet_state() -> Result<Value> {
    let path = wallet_state_path()?;
    if !path.is_file() {
        return Ok(wallet_state_with_detected_profile(default_wallet_state()));
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read wallet state from {}", path.display()))?;
    let state: Value = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse wallet state from {}", path.display()))?;
    Ok(wallet_state_with_detected_profile(state))
}

fn save_wallet_state(state: &Value) -> Result<Value> {
    let path = wallet_state_path()?;
    let parent = path
        .parent()
        .context("wallet state path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    let text = serde_json::to_string_pretty(state).context("failed to serialize wallet state")?;
    fs::write(&path, text)
        .with_context(|| format!("failed to write wallet state to {}", path.display()))?;
    Ok(json!({
        "saved": true,
        "path": path.display().to_string(),
    }))
}

fn load_settings_state() -> Result<Value> {
    let path = settings_state_path()?;
    if !path.is_file() {
        return Ok(default_settings_state());
    }

    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read settings state from {}", path.display()))?;
    serde_json::from_str(&text)
        .with_context(|| format!("failed to parse settings state from {}", path.display()))
}

fn save_settings_state(state: &Value) -> Result<Value> {
    let path = settings_state_path()?;
    let parent = path
        .parent()
        .context("settings state path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    let text = serde_json::to_string_pretty(state).context("failed to serialize settings state")?;
    fs::write(&path, text)
        .with_context(|| format!("failed to write settings state to {}", path.display()))?;
    Ok(json!({
        "saved": true,
        "path": path.display().to_string(),
    }))
}

fn default_settings_state() -> Value {
    json!({
        "version": 1
    })
}

fn default_wallet_state() -> Value {
    json!({
        "version": 1,
        "profile": {
            "label": "Local wallet",
            "wallet_binary": "",
            "wallet_home": "",
            "network_profile": "",
            "public_key_probe": ""
        },
        "operations": []
    })
}

fn wallet_state_with_detected_profile(mut state: Value) -> Value {
    let detected = detected_wallet_profile();
    let profile = match state.as_object_mut() {
        Some(root) => root
            .entry("profile")
            .or_insert_with(|| json!({ "label": "Local wallet" })),
        None => return state,
    };
    let Some(profile) = profile.as_object_mut() else {
        return state;
    };

    fill_blank_field(profile, &detected, "wallet_binary");
    fill_blank_field(profile, &detected, "wallet_home");
    state
}

fn fill_blank_field(profile: &mut serde_json::Map<String, Value>, detected: &Value, key: &str) {
    let current = profile.get(key).and_then(Value::as_str).unwrap_or_default();
    if !current.trim().is_empty() {
        return;
    }
    let Some(value) = detected.get(key).and_then(Value::as_str) else {
        return;
    };
    if value.trim().is_empty() {
        return;
    }
    profile.insert(key.to_owned(), Value::String(value.to_owned()));
}

fn detected_wallet_profile() -> Value {
    json!({
        "label": "Local wallet",
        "wallet_binary": detect_wallet_binary()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
        "wallet_home": detect_wallet_home()
            .map(|path| path.display().to_string())
            .unwrap_or_default(),
    })
}

fn detect_wallet_binary() -> Option<PathBuf> {
    if let Some(path) = env_path_if_file("LOGOS_WALLET_BINARY") {
        return Some(path);
    }

    if let Some(path) = find_binary_in_path("wallet") {
        return Some(path);
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    [
        home.join(".cargo").join("bin").join(binary_name("wallet")),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("logos-execution-zone")
            .join("target")
            .join("release")
            .join(binary_name("wallet")),
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
            .join("logos-execution-zone")
            .join("target")
            .join("debug")
            .join(binary_name("wallet")),
    ]
    .into_iter()
    .find(|path| path.is_file())
}

fn detect_wallet_home() -> Option<PathBuf> {
    if let Some(path) = env_path_if_wallet_home("NSSA_WALLET_HOME_DIR") {
        return Some(path);
    }
    if let Some(path) = env_path_if_wallet_home("LEE_WALLET_HOME_DIR") {
        return Some(path);
    }

    let home = env::var_os("HOME").map(PathBuf::from)?;
    [
        home.join(".nssa").join("wallet"),
        home.join(".lee").join("wallet"),
    ]
    .into_iter()
    .find(|path| wallet_home_is_configured(path))
}

fn env_path_if_file(variable: &str) -> Option<PathBuf> {
    let path = env::var_os(variable).map(PathBuf::from)?;
    path.is_file().then_some(path)
}

fn env_path_if_wallet_home(variable: &str) -> Option<PathBuf> {
    let path = env::var_os(variable).map(PathBuf::from)?;
    wallet_home_is_configured(&path).then_some(path)
}

fn wallet_home_is_configured(path: &Path) -> bool {
    path.is_dir() && path.join("wallet_config.json").is_file()
}

fn find_binary_in_path(binary: &str) -> Option<PathBuf> {
    let binary = binary_name(binary);
    env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| env::split_paths(&paths).collect::<Vec<_>>())
        .map(|path| path.join(&binary))
        .find(|path| path.is_file())
}

fn binary_name(binary: &str) -> String {
    if cfg!(windows) {
        format!("{binary}.exe")
    } else {
        binary.to_owned()
    }
}

fn wallet_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("wallet.json"))
}

fn settings_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

fn config_dir() -> Result<PathBuf> {
    if let Some(value) = env::var_os("LOGOS_INSPECTOR_CONFIG_DIR") {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(value).join("logos-inspector"));
    }
    if cfg!(windows)
        && let Some(value) = env::var_os("APPDATA")
    {
        return Ok(PathBuf::from(value).join("Logos Inspector"));
    }
    if cfg!(target_os = "macos")
        && let Some(value) = env::var_os("HOME")
    {
        return Ok(PathBuf::from(value)
            .join("Library")
            .join("Application Support")
            .join("Logos Inspector"));
    }
    if let Some(value) = env::var_os("HOME") {
        return Ok(PathBuf::from(value).join(".config").join("logos-inspector"));
    }
    bail!("could not determine config directory")
}

struct Args {
    values: Vec<Value>,
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
}
