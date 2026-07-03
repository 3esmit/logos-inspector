use std::{env, path::PathBuf, process::Command};

use anyhow::{Context as _, Result, bail};
use logos_inspector::local_indexer::bootstrap_default_local_indexer;

const ENABLE_INDEXER_AUTO_BOOTSTRAP_ENV: &str = "LOGOS_INSPECTOR_ENABLE_INDEXER_AUTO_BOOTSTRAP";

pub fn run() -> Result<()> {
    if env::var_os(ENABLE_INDEXER_AUTO_BOOTSTRAP_ENV).is_some() {
        bootstrap_default_local_indexer()?;
    }

    if let Some(program) = standalone_program() {
        let status = Command::new(&program)
            .status()
            .with_context(|| format!("failed to launch standalone GUI at {}", program.display()))?;

        if !status.success() {
            bail!("standalone GUI exited with {status}");
        }
        return Ok(());
    }

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

fn standalone_program() -> Option<PathBuf> {
    if let Some(program) = env::var_os("LOGOS_INSPECTOR_STANDALONE_GUI") {
        let program = PathBuf::from(program);
        if program.is_file() {
            return Some(program);
        }
    }

    let exe = env::current_exe().ok()?;
    let binary_name = if cfg!(windows) {
        "logos-inspector-standalone-gui.exe"
    } else {
        "logos-inspector-standalone-gui"
    };
    let sibling = exe.with_file_name(binary_name);
    sibling.is_file().then_some(sibling)
}
