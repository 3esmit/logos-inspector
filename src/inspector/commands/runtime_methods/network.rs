use anyhow::{Result, bail};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    source_routing::{CoreEndpointMode, SourceEndpoint, bedrock_layer},
    support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::with_runtime("channelScan", channel_scan),
    RuntimeMethodEntry::with_runtime("channelState", channel_state),
    RuntimeMethodEntry::with_runtime("rawRpc", raw_rpc),
];

pub(super) fn channel_scan(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let source = args.source_endpoint(0, "node endpoint")?;
    require_rpc_source(&source, "channelScan")?;
    to_value(runtime.block_on(bedrock_layer::channel_scan(
        source.endpoint,
        args.canonical_decimal_u64(source.next_index, "slot from")?,
        args.canonical_decimal_u64(source.next_index + 1, "slot to")?,
    ))?)
}

pub(super) fn channel_state(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let source = args.source_endpoint(0, "node endpoint")?;
    require_rpc_source(&source, "channelState")?;
    to_value(runtime.block_on(bedrock_layer::channel_state(
        source.endpoint,
        args.string(source.next_index, "channel id")?,
    ))?)
}

pub(super) fn raw_rpc(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(runtime.block_on(bedrock_layer::raw_rpc(
        args.string(0, "RPC endpoint")?,
        args.string(1, "RPC method")?,
        args.json_or_empty_array(2)?,
    ))?)
}

fn require_rpc_source(source: &SourceEndpoint<'_>, method: &str) -> Result<()> {
    if source.mode == CoreEndpointMode::Rpc {
        return Ok(());
    }
    bail!(
        "`{method}` is not exposed by the selected Basecamp module source `{}`; use RPC source for this call",
        source.module
    )
}

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::json;

    use super::channel_scan;

    #[test]
    fn channel_scan_rejects_noncanonical_slot_strings_before_transport() -> Result<()> {
        let runtime = tokio::runtime::Runtime::new()?;
        for slot_from in ["1e3", "1.0", "0x10", "+1", "-1", "01", " 1", "1 "] {
            let result = channel_scan(
                &runtime,
                json!(["rpc", "http://127.0.0.1:1", slot_from, "1000"]),
            );
            let Err(error) = result else {
                bail!("noncanonical channel scan slot `{slot_from}` should fail");
            };
            if !error.to_string().contains("invalid slot from") {
                bail!("unexpected channel scan slot error: {error:#}");
            }
        }
        Ok(())
    }
}
