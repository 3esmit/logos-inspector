use std::{env, path::PathBuf};

use anyhow::{Context as _, Result};

pub(crate) const QML_DIR_ENV: &str = "LOGOS_INSPECTOR_QML_DIR";

const QML_ENTRY_FILE: &str = "StandaloneMain.qml";

pub(crate) fn qml_entry() -> Result<PathBuf> {
    qml_entry_from(
        env::var_os(QML_DIR_ENV).map(PathBuf::from),
        default_qml_dir(),
    )
}

fn default_qml_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../qml")
}

fn qml_entry_from(
    configured_qml_dir: Option<PathBuf>,
    fallback_qml_dir: PathBuf,
) -> Result<PathBuf> {
    let qml_dir = configured_qml_dir.unwrap_or(fallback_qml_dir);
    let entry = qml_dir.join(QML_ENTRY_FILE);
    entry
        .canonicalize()
        .with_context(|| format!("failed to locate QML entry at {}", entry.display()))
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

        let entry = qml_entry_from(Some(configured_dir.clone()), fallback_dir.clone())?;

        cleanup(&configured_dir)?;
        cleanup(&fallback_dir)?;
        if entry != expected {
            bail!("configured QML directory was not selected");
        }
        Ok(())
    }

    #[test]
    fn qml_entry_uses_fallback_directory() -> Result<()> {
        let fallback_dir = temp_dir("fallback")?;
        create_qml_entry(&fallback_dir)?;
        let expected = fallback_dir.join(QML_ENTRY_FILE).canonicalize()?;

        let entry = qml_entry_from(None, fallback_dir.clone())?;

        cleanup(&fallback_dir)?;
        if entry != expected {
            bail!("fallback QML directory was not selected");
        }
        Ok(())
    }

    #[test]
    fn qml_entry_reports_missing_entry_path() -> Result<()> {
        let fallback_dir = temp_dir("missing")?;
        fs::create_dir_all(&fallback_dir)?;

        let result = qml_entry_from(None, fallback_dir.clone());

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
