use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    modules::{
        capabilities_report as inspect_capabilities_report, logos_core::SharedModuleTransport,
        logoscore_status_report, modules_report,
    },
    source_routing::{
        bedrock_layer, delivery_source_report_with_runtime_metrics, messaging_layer,
        source_policy_report, storage_layer,
        storage_source_report as inspect_storage_source_report,
    },
    support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] = &[
    RuntimeMethodEntry::no_args("sourcePolicy", source_policy),
    RuntimeMethodEntry::with_module_transport("modules", modules),
    RuntimeMethodEntry::with_module_transport("capabilitiesReport", capabilities_report),
    RuntimeMethodEntry::with_module_transport("logoscoreStatus", logoscore_status),
    RuntimeMethodEntry::with_module_transport("blockchainModuleReport", blockchain_module_report),
    RuntimeMethodEntry::with_module_transport("storageReport", storage_report),
    RuntimeMethodEntry::with_module_transport("storageSourceReport", storage_source_report),
    RuntimeMethodEntry::with_module_transport("deliveryReport", delivery_report),
    RuntimeMethodEntry::with_module_transport("deliverySourceReport", delivery_source_report),
];

pub(super) fn source_policy() -> Result<Value> {
    to_value(source_policy_report())
}

pub(super) fn modules(
    runtime: &Runtime,
    _args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    to_value(runtime.block_on(modules_report(&module_transport)))
}

pub(super) fn capabilities_report(
    runtime: &Runtime,
    _args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let adapter = module_transport.kind();
    to_value(runtime.block_on(inspect_capabilities_report(&module_transport, adapter)))
}

pub(super) fn logoscore_status(
    runtime: &Runtime,
    _args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    to_value(runtime.block_on(logoscore_status_report(&module_transport)))
}

pub(super) fn blockchain_module_report(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let adapter = module_transport.kind();
    to_value(runtime.block_on(bedrock_layer::diagnostic_report(
        &module_transport,
        adapter,
        args.optional_string(0),
    )))
}

pub(super) fn storage_report(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let adapter = module_transport.kind();
    to_value(runtime.block_on(storage_layer::module_report(
        &module_transport,
        adapter,
        args.optional_string(0),
        args.optional_bool(1),
        args.optional_bool(2),
    )))
}

pub(super) fn storage_source_report(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let inputs = storage_layer::report_inputs(&args)?;
    to_value(runtime.block_on(inspect_storage_source_report(
        &inputs.source_mode,
        inputs.rest_endpoint.as_deref(),
        inputs.metrics_endpoint.as_deref(),
        inputs.cid.as_deref(),
        inputs.privileged_debug_enabled,
        inputs.runtime_diagnostics_enabled,
        &module_transport,
    )))
}

pub(super) fn delivery_report(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let adapter = module_transport.kind();
    to_value(runtime.block_on(messaging_layer::module_report(
        &module_transport,
        adapter,
        args.optional_string(0),
        args.optional_bool(1),
        false,
    )))
}

pub(super) fn delivery_source_report(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let inputs = messaging_layer::report_inputs(&args)?;
    to_value(
        runtime.block_on(delivery_source_report_with_runtime_metrics(
            &inputs.source_mode,
            inputs.rest_endpoint.as_deref(),
            inputs.metrics_endpoint.as_deref(),
            inputs.runtime_diagnostics_enabled,
            inputs.runtime_metrics_enabled,
            &module_transport,
        )),
    )
}
