use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

mod decode;
mod local_nodes;
mod module_reports;
mod network;
mod state;
mod storage;
mod wallet;

pub(super) fn try_handle(runtime: &Runtime, method: &str, args: Value) -> Result<Option<Value>> {
    if let Some(value) = decode::try_handle(method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = network::try_handle(runtime, method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = wallet::try_handle(runtime, method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = local_nodes::try_handle(method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = state::try_handle(method, args.clone())? {
        return Ok(Some(value));
    }
    if let Some(value) = module_reports::try_handle(runtime, method, args.clone())? {
        return Ok(Some(value));
    }
    storage::try_handle(runtime, method, args)
}
