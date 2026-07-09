use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    capabilities::{
        CapabilityBuildMode, capability_registry_report_with_value as inspect_capability_registry,
    },
    lez::block_list_report as inspect_lez_block_list_report,
    modules::{
        blockchain_module_report as inspect_blockchain_module_report,
        capabilities_report as inspect_capabilities_report,
        delivery_report as inspect_delivery_report, logoscore_status_report, modules_report,
        storage_report as inspect_storage_report,
    },
    source_routing::{
        delivery_source_report as inspect_delivery_source_report, source_policy_report,
        storage_source_report as inspect_storage_source_report,
    },
    support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::no_args("sourcePolicy", source_policy),
    RuntimeMethodEntry::no_args("modules", modules),
    RuntimeMethodEntry::no_args("capabilitiesReport", capabilities_report),
    RuntimeMethodEntry::sync("capabilityRegistryReport", capability_registry_report),
    RuntimeMethodEntry::no_args("logoscoreStatus", logoscore_status),
    RuntimeMethodEntry::sync("blockchainModuleReport", blockchain_module_report),
    RuntimeMethodEntry::sync("storageReport", storage_report),
    RuntimeMethodEntry::with_runtime("storageSourceReport", storage_source_report),
    RuntimeMethodEntry::sync("deliveryReport", delivery_report),
    RuntimeMethodEntry::with_runtime("deliverySourceReport", delivery_source_report),
    RuntimeMethodEntry::sync("lezBlockListReport", lez_block_list_report),
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

pub(super) fn capability_registry_report(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let build_mode = CapabilityBuildMode::from_prefers_basecamp(args.optional_bool(0));
    let runtime_inputs = args
        .value(1)
        .filter(|value| value.is_object())
        .context("capability runtime inputs are required")?;
    to_value(inspect_capability_registry(
        build_mode,
        Some(runtime_inputs),
    ))
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

pub(super) fn lez_block_list_report(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let sequencer_blocks = args.value(0).cloned().unwrap_or(Value::Array(vec![]));
    let indexer_blocks = args.value(1).cloned().unwrap_or(Value::Array(vec![]));
    let limit = args.value(2).and_then(optional_usize).unwrap_or(0);
    Ok(inspect_lez_block_list_report(
        &sequencer_blocks,
        &indexer_blocks,
        limit,
    ))
}

fn optional_usize(value: &Value) -> Option<usize> {
    value
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .or_else(|| value.as_str()?.trim().parse::<usize>().ok())
}
