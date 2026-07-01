use anyhow::{Context as _, Result, bail};
use serde_json::{Value, json};
use tokio::runtime::Runtime;

use crate::{
    account_lookup, account_lookup_with_idl, blockchain, channels,
    decode_account_data_hex_with_idl, decode_event_data_hex_with_idl, last_sequencer_block_id,
    logoscore,
    modules::{
        blockchain_module_report, capabilities_report, delivery_report, logoscore_status_report,
        modules_report, storage_report,
    },
    overview, program_file_info, raw_json_rpc, raw_rpc_report, sequencer_block,
    sequencer_program_ids, sequencer_transaction, sequencer_transaction_inspection,
    sequencer_transaction_inspection_with_idl, sequencer_transaction_trace,
    sequencer_transaction_trace_with_idl,
    spel::spel_idl_report,
};

pub const INSPECTOR_MODULE: &str = "logos_inspector";

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
            "channelScan" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(channels::channel_scan(
                    args.string(0, "node endpoint")?,
                    args.u64(1, "slot from")?,
                    args.u64(2, "slot to")?,
                ))?)
            }
            "indexerHealth" => {
                let args = Args::new(args)?;
                let head = self.runtime.block_on(raw_json_rpc(
                    args.string(0, "indexer endpoint")?,
                    "getLastFinalizedBlockId",
                    Value::Array(vec![]),
                ))?;
                Ok(json!({
                    "status": "reachable",
                    "head": head,
                }))
            }
            "indexerFinalizedHead" => {
                let args = Args::new(args)?;
                to_value(self.runtime.block_on(raw_json_rpc(
                    args.string(0, "indexer endpoint")?,
                    "getLastFinalizedBlockId",
                    Value::Array(vec![]),
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
            "modules" => to_value(modules_report()),
            "logoscoreStatus" => to_value(logoscore_status_report()),
            "blockchainModuleReport" => {
                let args = Args::new(args)?;
                to_value(blockchain_module_report(args.optional_string(0)))
            }
            "storageReport" => {
                let args = Args::new(args)?;
                to_value(storage_report(args.optional_string(0)))
            }
            "deliveryReport" => {
                let args = Args::new(args)?;
                to_value(delivery_report(args.optional_string(0)))
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
            to_value(
                self.runtime
                    .block_on(sequencer_transaction_inspection(endpoint, hash))?,
            )
        }
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
        if let Some(idl) = idl {
            to_value(self.runtime.block_on(account_lookup_with_idl(
                sequencer,
                indexer,
                account,
                idl,
                args.optional_string(4),
            ))?)
        } else {
            to_value(
                self.runtime
                    .block_on(account_lookup(sequencer, indexer, account))?,
            )
        }
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
}

fn to_value(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).context("failed to serialize bridge response")
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
