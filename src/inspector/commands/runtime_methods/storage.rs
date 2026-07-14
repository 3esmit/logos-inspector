use anyhow::Result;
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    modules::logos_core::SharedModuleTransport, source_routing::storage_layer, support::args::Args,
};

use super::super::value::to_value;
use super::RuntimeMethodEntry;

pub(super) const METHOD_CATALOG: &[RuntimeMethodEntry] =
    &[RuntimeMethodEntry::with_module_transport(
        "storageExists",
        storage_exists,
    )];

pub(super) fn storage_exists(
    runtime: &Runtime,
    args: Value,
    module_transport: SharedModuleTransport,
) -> Result<Value> {
    let args = Args::new(args)?;
    let request = storage_layer::StorageExistsRequest::parse(&args)?;
    to_value(runtime.block_on(request.execute(&module_transport))?)
}
