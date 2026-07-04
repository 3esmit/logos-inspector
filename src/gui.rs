use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context as _, Result, bail};
use logos_inspector::local_indexer::bootstrap_default_local_indexer_for_saved_settings;

const ENABLE_INDEXER_AUTO_BOOTSTRAP_ENV: &str = "LOGOS_INSPECTOR_ENABLE_INDEXER_AUTO_BOOTSTRAP";

pub fn run() -> Result<()> {
    if env::var_os(ENABLE_INDEXER_AUTO_BOOTSTRAP_ENV).is_some() {
        bootstrap_default_local_indexer_for_saved_settings()?;
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
    sibling_standalone_program(&exe)
}

fn sibling_standalone_program(exe: &Path) -> Option<PathBuf> {
    let sibling = exe.with_file_name(standalone_binary_name());
    if sibling.is_file() {
        return Some(sibling);
    }
    None
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

    use std::{
        fs, thread,
        time::{Duration, SystemTime},
    };

    #[test]
    fn sibling_standalone_program_accepts_older_sibling() -> Result<()> {
        let dir = temp_test_dir("older");
        fs::create_dir_all(&dir)?;
        let exe = dir.join("logos-inspector");
        let sibling = dir.join(standalone_binary_name());
        touch(&sibling)?;
        thread::sleep(Duration::from_millis(20));
        touch(&exe)?;

        let selected = sibling_standalone_program(&exe);

        cleanup_temp_dir(&dir)?;
        if selected.as_deref() != Some(sibling.as_path()) {
            bail!("older standalone sibling should be selected, got {selected:?}");
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
