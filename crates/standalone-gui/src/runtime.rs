use std::{
    env,
    path::{Path, PathBuf},
};

use anyhow::{Result, bail};

pub(crate) const QML_DIR_ENV: &str = "LOGOS_INSPECTOR_QML_DIR";

const QML_ENTRY_FILE: &str = "StandaloneMain.qml";

pub(crate) fn qml_entry() -> Result<PathBuf> {
    RuntimeLayoutInputs::default().qml_entry()
}

fn default_qml_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../qml")
}

#[derive(Debug, Clone)]
struct RuntimeLayoutInputs {
    qml_dir_override: Option<PathBuf>,
    current_exe: Option<PathBuf>,
    dev_qml_dir: PathBuf,
}

impl Default for RuntimeLayoutInputs {
    fn default() -> Self {
        Self {
            qml_dir_override: env::var_os(QML_DIR_ENV).map(PathBuf::from),
            current_exe: env::current_exe().ok(),
            dev_qml_dir: default_qml_dir(),
        }
    }
}

impl RuntimeLayoutInputs {
    fn qml_entry(&self) -> Result<PathBuf> {
        if let Some(qml_dir) = self.qml_dir_override.as_ref() {
            return canonical_qml_entry(qml_dir, "configured QML directory");
        }

        let mut searched = Vec::new();
        if let Some(exe) = self
            .current_exe
            .as_ref()
            .and_then(|exe| installed_qml_dir(exe))
        {
            match canonical_qml_entry(&exe, "installed QML directory") {
                Ok(entry) => return Ok(entry),
                Err(error) => searched.push(format!("{error:#}")),
            }
        }
        match canonical_qml_entry(&self.dev_qml_dir, "development QML directory") {
            Ok(entry) => Ok(entry),
            Err(error) => {
                searched.push(format!("{error:#}"));
                bail!(
                    "failed to locate QML entry; searched {}",
                    searched.join("; ")
                )
            }
        }
    }
}

fn canonical_qml_entry(qml_dir: &Path, label: &str) -> Result<PathBuf> {
    let entry = qml_dir.join(QML_ENTRY_FILE);
    match entry.canonicalize() {
        Ok(entry) => Ok(entry),
        Err(error) => bail!("{label} missing {}: {error}", entry.display()),
    }
}

fn installed_qml_dir(current_exe: &Path) -> Option<PathBuf> {
    let bin_dir = current_exe.parent()?;
    let prefix = bin_dir.parent()?;
    Some(prefix.join("share/logos-inspector/qml"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };

    use anyhow::{Result, bail};

    #[test]
    fn qml_entry_uses_configured_directory() -> Result<()> {
        let configured_dir = temp_dir("configured")?;
        let fallback_dir = temp_dir("fallback")?;
        create_qml_entry(&configured_dir)?;
        let expected = configured_dir.join(QML_ENTRY_FILE).canonicalize()?;

        let entry = RuntimeLayoutInputs {
            qml_dir_override: Some(configured_dir.clone()),
            current_exe: None,
            dev_qml_dir: fallback_dir.clone(),
        }
        .qml_entry()?;

        cleanup(&configured_dir)?;
        cleanup(&fallback_dir)?;
        if entry != expected {
            bail!("configured QML directory was not selected");
        }
        Ok(())
    }

    #[test]
    fn qml_entry_prefers_installed_directory_before_dev_directory() -> Result<()> {
        let prefix = temp_dir("prefix")?;
        let installed_dir = prefix.join("share/logos-inspector/qml");
        let dev_dir = temp_dir("dev")?;
        create_qml_entry(&installed_dir)?;
        create_qml_entry(&dev_dir)?;
        let expected = installed_dir.join(QML_ENTRY_FILE).canonicalize()?;

        let entry = RuntimeLayoutInputs {
            qml_dir_override: None,
            current_exe: Some(prefix.join("bin/logos-inspector-standalone-gui")),
            dev_qml_dir: dev_dir.clone(),
        }
        .qml_entry()?;

        cleanup(&prefix)?;
        cleanup(&dev_dir)?;
        if entry != expected {
            bail!("installed QML directory was not selected");
        }
        Ok(())
    }

    #[test]
    fn qml_entry_uses_dev_directory_when_installed_layout_missing() -> Result<()> {
        let prefix = temp_dir("missing-prefix")?;
        let dev_dir = temp_dir("dev")?;
        create_qml_entry(&dev_dir)?;
        let expected = dev_dir.join(QML_ENTRY_FILE).canonicalize()?;

        let entry = RuntimeLayoutInputs {
            qml_dir_override: None,
            current_exe: Some(prefix.join("bin/logos-inspector-standalone-gui")),
            dev_qml_dir: dev_dir.clone(),
        }
        .qml_entry()?;

        cleanup(&prefix)?;
        cleanup(&dev_dir)?;
        if entry != expected {
            bail!("development QML directory was not selected");
        }
        Ok(())
    }

    #[test]
    fn qml_entry_reports_missing_entry_path() -> Result<()> {
        let prefix = temp_dir("prefix")?;
        let fallback_dir = temp_dir("missing")?;
        fs::create_dir_all(&prefix)?;
        fs::create_dir_all(&fallback_dir)?;

        let result = RuntimeLayoutInputs {
            qml_dir_override: None,
            current_exe: Some(prefix.join("bin/logos-inspector-standalone-gui")),
            dev_qml_dir: fallback_dir.clone(),
        }
        .qml_entry();

        cleanup(&prefix)?;
        cleanup(&fallback_dir)?;
        let Err(error) = result else {
            bail!("missing QML entry unexpectedly resolved");
        };
        let message = format!("{error:#}");
        if !message.contains(QML_ENTRY_FILE) {
            bail!("missing QML entry error omitted file path: {message}");
        }
        Ok(())
    }

    fn create_qml_entry(dir: &Path) -> Result<()> {
        fs::create_dir_all(dir)?;
        fs::write(dir.join(QML_ENTRY_FILE), [])?;
        Ok(())
    }

    fn temp_dir(label: &str) -> Result<PathBuf> {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos();
        Ok(env::temp_dir().join(format!(
            "logos-inspector-standalone-runtime-{label}-{stamp}"
        )))
    }

    fn cleanup(path: &Path) -> Result<()> {
        if path.exists() {
            fs::remove_dir_all(path)?;
        }
        Ok(())
    }
}
