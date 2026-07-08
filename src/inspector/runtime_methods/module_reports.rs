use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    modules::{
        blockchain_module_report as inspect_blockchain_module_report,
        delivery_report as inspect_delivery_report, logoscore_status_report, modules_report,
        storage_report as inspect_storage_report,
    },
    network_profiles,
    source_routing::{
        Args, delivery_source_report as inspect_delivery_source_report, source_policy_report,
        storage_source_report as inspect_storage_source_report,
    },
};

use super::super::bridge::to_value;

pub(super) fn source_policy() -> Result<Value> {
    to_value(source_policy_report(network_profiles()))
}

pub(super) fn modules() -> Result<Value> {
    to_value(modules_report())
}

pub(super) fn logoscore_status() -> Result<Value> {
    to_value(logoscore_status_report())
}

pub(super) fn blockchain_module_report(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(inspect_blockchain_module_report(args.optional_string(0)))
}

pub(super) fn storage_report(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(inspect_storage_report(
        args.optional_string(0),
        args.optional_bool(1),
    ))
}

pub(super) fn storage_source_report(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(runtime.block_on(inspect_storage_source_report(
        args.optional_string(0).unwrap_or("rest"),
        args.optional_string(1),
        args.optional_string(2),
        args.optional_string(3),
        args.optional_bool(4),
    )))
}

pub(super) fn delivery_report(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(inspect_delivery_report(args.optional_string(0)))
}

pub(super) fn delivery_source_report(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(runtime.block_on(inspect_delivery_source_report(
        args.optional_string(0).unwrap_or("rest"),
        args.optional_string(1),
        args.optional_string(2),
    )))
}
