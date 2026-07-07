use anyhow::{Result, bail};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    channel_scan, channel_state, overview, raw_rpc_report,
    source_routing::{Args, CoreEndpointMode, SourceEndpoint},
};

use super::super::bridge::to_value;

pub(super) fn try_handle(runtime: &Runtime, method: &str, args: Value) -> Result<Option<Value>> {
    let value = match method {
        "overview" => {
            let args = Args::new(args)?;
            let value = runtime.block_on(overview(
                args.string(0, "sequencer endpoint")?,
                args.string(1, "indexer endpoint")?,
                args.string(2, "node endpoint")?,
            ));
            to_value(value)?
        }
        "channelScan" => {
            let args = Args::new(args)?;
            let source = args.source_endpoint(0, "node endpoint")?;
            require_rpc_source(&source, "channelScan")?;
            to_value(runtime.block_on(channel_scan(
                source.endpoint,
                args.u64(source.next_index, "slot from")?,
                args.u64(source.next_index + 1, "slot to")?,
            ))?)?
        }
        "channelState" => {
            let args = Args::new(args)?;
            let source = args.source_endpoint(0, "node endpoint")?;
            require_rpc_source(&source, "channelState")?;
            to_value(runtime.block_on(channel_state(
                source.endpoint,
                args.string(source.next_index, "channel id")?,
            ))?)?
        }
        "rawRpc" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(raw_rpc_report(
                args.string(0, "RPC endpoint")?,
                args.string(1, "RPC method")?,
                args.json_or_empty_array(2)?,
            ))?)?
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
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
