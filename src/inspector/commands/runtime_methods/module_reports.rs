use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    modules::{
        blockchain_module_report as inspect_blockchain_module_report,
        capabilities_report as inspect_capabilities_report,
        delivery_report as inspect_delivery_report, logoscore_status_report, modules_report,
        storage_report as inspect_storage_report,
    },
    source_routing::{
        Args, delivery_source_report as inspect_delivery_source_report, source_policy_report,
        storage_source_report as inspect_storage_source_report,
    },
};

use super::super::value::to_value;
use super::{RuntimeMethod, RuntimeMethodEntry};

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::new(RuntimeMethod::SourcePolicy, "sourcePolicy"),
    RuntimeMethodEntry::new(RuntimeMethod::Modules, "modules"),
    RuntimeMethodEntry::new(RuntimeMethod::CapabilitiesReport, "capabilitiesReport"),
    RuntimeMethodEntry::new(RuntimeMethod::LogoscoreStatus, "logoscoreStatus"),
    RuntimeMethodEntry::new(
        RuntimeMethod::BlockchainModuleReport,
        "blockchainModuleReport",
    ),
    RuntimeMethodEntry::new(RuntimeMethod::StorageReport, "storageReport"),
    RuntimeMethodEntry::new(RuntimeMethod::StorageSourceReport, "storageSourceReport"),
    RuntimeMethodEntry::new(RuntimeMethod::DeliveryReport, "deliveryReport"),
    RuntimeMethodEntry::new(RuntimeMethod::DeliverySourceReport, "deliverySourceReport"),
];

pub(super) fn source_policy() -> Result<Value> {
    to_value(source_policy_report())
}

pub(super) fn modules() -> Result<Value> {
    to_value(modules_report())
}

pub(super) fn capabilities_report() -> Result<Value> {
    to_value(inspect_capabilities_report())
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
