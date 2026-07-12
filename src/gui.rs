mod planner;

use anyhow::{Context as _, Result, bail};
use planner::{GuiLaunchTarget, plan_launch};
use std::{path::Path, process::Command};

pub fn run() -> Result<()> {
    let plan = plan_launch();

    match plan.target {
        GuiLaunchTarget::StandaloneProgram(program) => {
            run_program(&program, "standalone GUI")?;
        }
        GuiLaunchTarget::Nix { flake_ref } => {
            let status = Command::new("nix")
                .args(["run", &flake_ref])
                .status()
                .context("failed to launch QML UI with nix")?;

            if !status.success() {
                bail!("QML UI exited with {status}");
            }
        }
    }

    Ok(())
}

fn run_program(program: &Path, label: &str) -> Result<()> {
    let status = Command::new(program)
        .status()
        .with_context(|| format!("failed to launch {label} at {}", program.display()))?;

    if !status.success() {
        bail!("{label} exited with {status}");
    }
    Ok(())
}
