use std::{
    env,
    path::{Path, PathBuf},
};

const STANDALONE_GUI_ENV: &str = "LOGOS_INSPECTOR_STANDALONE_GUI";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GuiLaunchPlan {
    pub(crate) target: GuiLaunchTarget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GuiLaunchTarget {
    StandaloneProgram(PathBuf),
    Nix { flake_ref: String },
}

#[derive(Debug, Clone)]
struct GuiLaunchInputs {
    standalone_override: Option<PathBuf>,
    current_exe: Option<PathBuf>,
    manifest_dir: PathBuf,
}

pub(crate) fn plan_launch() -> GuiLaunchPlan {
    plan_launch_from_inputs(&GuiLaunchInputs {
        standalone_override: env::var_os(STANDALONE_GUI_ENV).map(PathBuf::from),
        current_exe: env::current_exe().ok(),
        manifest_dir: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
    })
}

fn plan_launch_from_inputs(inputs: &GuiLaunchInputs) -> GuiLaunchPlan {
    let target = standalone_program(inputs)
        .map(GuiLaunchTarget::StandaloneProgram)
        .unwrap_or_else(|| GuiLaunchTarget::Nix {
            flake_ref: format!("path:{}#standalone", inputs.manifest_dir.display()),
        });
    GuiLaunchPlan { target }
}

fn standalone_program(inputs: &GuiLaunchInputs) -> Option<PathBuf> {
    if let Some(program) = &inputs.standalone_override
        && program.is_file()
    {
        return Some(program.clone());
    }

    let exe = inputs.current_exe.as_deref()?;
    sibling_standalone_program(exe)
}

fn sibling_standalone_program(exe: &Path) -> Option<PathBuf> {
    let sibling = exe.with_file_name(standalone_binary_name());
    if sibling.is_file() && sibling_is_current_enough(exe, &sibling) {
        return Some(sibling);
    }
    None
}

fn sibling_is_current_enough(exe: &Path, sibling: &Path) -> bool {
    let exe_mtime = exe.metadata().and_then(|metadata| metadata.modified()).ok();
    let sibling_mtime = sibling
        .metadata()
        .and_then(|metadata| metadata.modified())
        .ok();
    match (exe_mtime, sibling_mtime) {
        (Some(exe_mtime), Some(sibling_mtime)) => sibling_mtime >= exe_mtime,
        _ => true,
    }
}

fn standalone_binary_name() -> &'static str {
    if cfg!(windows) {
        "logos-inspector-standalone-gui.exe"
    } else {
        "logos-inspector-standalone-gui"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use anyhow::{Result, bail, ensure};
    use std::{
        fs, thread,
        time::{Duration, SystemTime},
    };

    #[test]
    fn launch_plan_selects_standalone_override() -> Result<()> {
        let dir = temp_test_dir("override");
        fs::create_dir_all(&dir)?;
        let override_program = dir.join("custom-gui");
        touch(&override_program)?;
        let inputs = GuiLaunchInputs {
            standalone_override: Some(override_program.clone()),
            current_exe: None,
            manifest_dir: dir.clone(),
        };

        let plan = plan_launch_from_inputs(&inputs);
        let target = plan.target;

        cleanup_temp_dir(&dir)?;
        ensure!(
            target == GuiLaunchTarget::StandaloneProgram(override_program),
            "unexpected launch target"
        );
        Ok(())
    }

    #[test]
    fn launch_plan_falls_back_to_nix_without_standalone_program() {
        let inputs = GuiLaunchInputs {
            standalone_override: None,
            current_exe: None,
            manifest_dir: PathBuf::from("/repo"),
        };

        let plan = plan_launch_from_inputs(&inputs);

        assert_eq!(
            plan.target,
            GuiLaunchTarget::Nix {
                flake_ref: "path:/repo#standalone".to_owned()
            }
        );
    }

    #[test]
    fn sibling_standalone_program_rejects_older_sibling() -> Result<()> {
        let dir = temp_test_dir("older");
        fs::create_dir_all(&dir)?;
        let exe = dir.join("logos-inspector");
        let sibling = dir.join(standalone_binary_name());
        touch(&sibling)?;
        thread::sleep(Duration::from_millis(20));
        touch(&exe)?;

        let selected = sibling_standalone_program(&exe);

        cleanup_temp_dir(&dir)?;
        if selected.is_some() {
            bail!("older standalone sibling should not be selected, got {selected:?}");
        }
        Ok(())
    }

    #[test]
    fn sibling_standalone_program_accepts_newer_sibling() -> Result<()> {
        let dir = temp_test_dir("newer");
        fs::create_dir_all(&dir)?;
        let exe = dir.join("logos-inspector");
        let sibling = dir.join(standalone_binary_name());
        touch(&exe)?;
        thread::sleep(Duration::from_millis(20));
        touch(&sibling)?;

        let selected = sibling_standalone_program(&exe);

        cleanup_temp_dir(&dir)?;
        if selected.as_deref() != Some(sibling.as_path()) {
            bail!("newer standalone sibling should be selected, got {selected:?}");
        }
        Ok(())
    }

    fn touch(path: &Path) -> Result<()> {
        fs::write(path, [])?;
        Ok(())
    }

    fn temp_test_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        env::temp_dir().join(format!(
            "logos-inspector-gui-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    fn cleanup_temp_dir(path: &Path) -> Result<()> {
        fs::remove_dir_all(path)?;
        Ok(())
    }
}
