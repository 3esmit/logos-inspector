use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    modules::{
        blockchain_module_report, delivery_report, logoscore_status_report, modules_report,
        storage_report,
    },
    network_profiles,
    source_routing::{Args, delivery_source_report, source_policy_report, storage_source_report},
};

use super::super::bridge::to_value;

pub(super) fn try_handle(runtime: &Runtime, method: &str, args: Value) -> Result<Option<Value>> {
    let value = match method {
        "sourcePolicy" => to_value(source_policy_report(network_profiles()))?,
        "modules" => to_value(modules_report())?,
        "logoscoreStatus" => to_value(logoscore_status_report())?,
        "blockchainModuleReport" => {
            let args = Args::new(args)?;
            to_value(blockchain_module_report(args.optional_string(0)))?
        }
        "storageReport" => {
            let args = Args::new(args)?;
            to_value(storage_report(
                args.optional_string(0),
                args.optional_bool(1),
            ))?
        }
        "storageSourceReport" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(storage_source_report(
                args.optional_string(0).unwrap_or("rest"),
                args.optional_string(1),
                args.optional_string(2),
                args.optional_string(3),
                args.optional_bool(4),
            )))?
        }
        "deliveryReport" => {
            let args = Args::new(args)?;
            to_value(delivery_report(args.optional_string(0)))?
        }
        "deliverySourceReport" => {
            let args = Args::new(args)?;
            to_value(runtime.block_on(delivery_source_report(
                args.optional_string(0).unwrap_or("rest"),
                args.optional_string(1),
                args.optional_string(2),
            )))?
        }
        _ => return Ok(None),
    };
    Ok(Some(value))
}
