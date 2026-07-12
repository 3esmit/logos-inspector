use anyhow::{Context as _, Result};
use serde_json::Value;

use crate::inspector::command_surface::InspectorCommandSurface;

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
