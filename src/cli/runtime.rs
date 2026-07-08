use std::env;

use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::{
    inspector::command_surface::InspectorCommandSurface,
    local_nodes::{bootstrap_default_local_indexer, is_default_local_indexer_endpoint},
};

pub(super) struct CliCommandRuntime {
    surface: InspectorCommandSurface,
}

impl CliCommandRuntime {
    pub(super) fn new() -> Result<Self> {
        Ok(Self {
            surface: InspectorCommandSurface::new()
                .context("failed to create CLI command surface")?,
        })
    }

    pub(super) fn call(&self, method: &str, args: Value) -> Result<Value> {
        self.surface.call_inspector(method, args)
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
