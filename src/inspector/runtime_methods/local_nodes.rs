use anyhow::Result;
use serde_json::Value;

use crate::{local_devnet_list, local_nodes_status, source_routing::Args};

use super::super::bridge::to_value;

pub(super) fn try_handle(method: &str, args: Value) -> Result<Option<Value>> {
    let value = match method {
        "localNodesStatus" => {
            let args = Args::new(args)?;
            to_value(local_nodes_status(
                args.optional_string(0).unwrap_or("default"),
            )?)?
        }
        "localDevnetList" => {
            let args = Args::new(args)?;
            to_value(local_devnet_list(
                args.optional_string(0).unwrap_or("default"),
            )?)?
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}
