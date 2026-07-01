use std::process::Command;

use anyhow::{Context as _, Result, bail};

pub fn run() -> Result<()> {
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
