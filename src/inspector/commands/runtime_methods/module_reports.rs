use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    capabilities::{
        CapabilityBuildMode, capability_registry_report_with_value as inspect_capability_registry,
    },
    modules::{
        capabilities_report as inspect_capabilities_report, logoscore_status_report, modules_report,
    },
    source_routing::{
        bedrock_layer, delivery_source_report as inspect_delivery_source_report, messaging_layer,
        source_policy_report, storage_layer,
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
    to_value(bedrock_layer::diagnostic_report(args.optional_string(0)))
}

pub(super) fn storage_report(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(storage_layer::module_report(
        args.optional_string(0),
        args.optional_bool(1),
    ))
}

pub(super) fn storage_source_report(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let inputs = storage_layer::report_inputs(&args);
    to_value(runtime.block_on(inspect_storage_source_report(
        inputs.source_mode,
        inputs.rest_endpoint,
        inputs.metrics_endpoint,
        inputs.cid,
        inputs.privileged_debug_enabled,
    )))
}

pub(super) fn delivery_report(args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    to_value(messaging_layer::module_report(args.optional_string(0)))
}

pub(super) fn delivery_source_report(runtime: &Runtime, args: Value) -> Result<Value> {
    let args = Args::new(args)?;
    let inputs = messaging_layer::report_inputs(&args);
    to_value(runtime.block_on(inspect_delivery_source_report(
        inputs.source_mode,
        inputs.rest_endpoint,
        inputs.metrics_endpoint,
    )))
}
