use std::process::Command;

use anyhow::{Context as _, Result, bail};
use logos_inspector::local_indexer::bootstrap_default_local_indexer;

pub fn run() -> Result<()> {
    bootstrap_default_local_indexer()?;

    let flake_ref = format!("path:{}#standalone", env!("CARGO_MANIFEST_DIR"));
    let status = Command::new("nix")
        .args(["run", &flake_ref])
        .status()
        .context("failed to launch QML UI with nix")?;

    if !status.success() {
        bail!("QML UI exited with {status}");
    }

    Ok(())
}
