use std::{env, path::PathBuf};

use anyhow::{Result, bail};

pub(crate) fn settings_state_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

pub(crate) fn config_dir() -> Result<PathBuf> {
    if let Some(value) = env::var_os("LOGOS_INSPECTOR_CONFIG_DIR") {
        return Ok(PathBuf::from(value));
    }
    if let Some(value) = env::var_os("XDG_CONFIG_HOME") {
        return Ok(PathBuf::from(value).join("logos-inspector"));
    }
    if cfg!(windows)
        && let Some(value) = env::var_os("APPDATA")
    {
        return Ok(PathBuf::from(value).join("Logos Inspector"));
    }
    if cfg!(target_os = "macos")
        && let Some(value) = env::var_os("HOME")
    {
        return Ok(PathBuf::from(value)
            .join("Library")
            .join("Application Support")
            .join("Logos Inspector"));
    }
    if let Some(value) = env::var_os("HOME") {
        return Ok(PathBuf::from(value).join(".config").join("logos-inspector"));
    }
    bail!("could not determine config directory")
}
