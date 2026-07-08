use anyhow::{Context as _, Result};
use serde_json::Value;

pub(crate) fn to_value(value: impl serde::Serialize) -> Result<Value> {
    serde_json::to_value(value).context("failed to serialize bridge response")
}

pub(crate) async fn blocking_value(
    label: &'static str,
    task: impl FnOnce() -> Result<Value> + Send + 'static,
) -> Result<Value> {
    tokio::task::spawn_blocking(task)
        .await
        .with_context(|| format!("{label} task failed"))?
}
