use std::env;

use anyhow::{Context as _, Result};
use serde_json::Value;
use tokio::runtime::Runtime;

use crate::{
    inspector::{
        command_surface::{DispatchContext, dispatch_inspector_command},
        commands::operations::RuntimeOperationInterface,
        value::to_value,
    },
    local_nodes::{bootstrap_default_local_indexer, is_default_local_indexer_endpoint},
    modules::logos_core,
    source_routing::Args as SourceArgs,
};

pub(super) struct CliCommandRuntime {
    runtime: Runtime,
    operations: RuntimeOperationInterface,
}

impl CliCommandRuntime {
    pub(super) fn new() -> Result<Self> {
        Ok(Self {
            runtime: Runtime::new().context("failed to create tokio runtime")?,
            operations: RuntimeOperationInterface::default(),
        })
    }

    pub(super) fn call(&self, method: &str, args: Value) -> Result<Value> {
        let call_core_module = |module: &str, method: &str, args: Value| {
            let args = SourceArgs::new(args)?
                .iter()
                .map(|value| {
                    value
                        .as_str()
                        .map(ToOwned::to_owned)
                        .unwrap_or_else(|| value.to_string())
                })
                .collect::<Vec<_>>();
            to_value(logos_core::call(module, method, &args)?)
        };
        let context = DispatchContext {
            runtime: &self.runtime,
            operations: &self.operations,
            call_core_module: &call_core_module,
        };
        dispatch_inspector_command(&context, method, args)?
            .with_context(|| format!("unknown inspector method `{method}`"))
    }
}

pub(super) fn maybe_bootstrap_default_local_indexer(endpoint: &str) -> Result<()> {
    if env::var_os("LOGOS_INSPECTOR_ENABLE_INDEXER_AUTO_BOOTSTRAP").is_some()
        && is_default_local_indexer_endpoint(endpoint)
    {
        bootstrap_default_local_indexer()?;
    }
    Ok(())
}
