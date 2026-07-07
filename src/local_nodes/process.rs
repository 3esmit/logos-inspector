use std::{env, path::Path, process::Command};

use anyhow::{Context as _, Result, bail};

pub(super) fn find_command(command: &str) -> Option<String> {
    if command.contains(std::path::MAIN_SEPARATOR) {
        let path = Path::new(command);
        return path.is_file().then(|| path.display().to_string());
    }
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|path| path.join(command))
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
}

pub(super) fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .is_ok_and(|status| status.success())
}

pub(super) fn stop_process(pid: u32) -> Result<()> {
    #[cfg(unix)]
    let target = format!("-{pid}");
    #[cfg(not(unix))]
    let target = pid.to_string();
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(target)
        .status()
        .with_context(|| format!("failed to stop process {pid}"))?;
    if !status.success() {
        bail!("process {pid} stop exited with {status}");
    }
    Ok(())
}
