use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};

use crate::modules::logos_core::LogoscoreCliRuntime;
use crate::support::command_runner::{CommandControl, CommandStopReason, CommandTerminated};

use super::process::{find_command, process_is_alive};

const RUNTIME_FILE: &str = "logoscore_runtime.json";
const MANAGED_RUNTIME_ID: &str = "inspector-managed-local";
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LogoscoreRuntimeOwnership {
    External,
    InspectorManaged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LogoscoreTimeoutProfile {
    Probe,
    Lifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LogoscoreRuntimeProfile {
    pub(super) id: String,
    pub(super) binary_path: String,
    pub(super) config_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) modules_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) persistence_path: Option<String>,
    pub(super) ownership: LogoscoreRuntimeOwnership,
    pub(super) timeout_profile: LogoscoreTimeoutProfile,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) daemon_process_id: Option<u32>,
}

impl LogoscoreRuntimeProfile {
    pub(super) fn create_or_restart(
        config_root: &Path,
        existing: Option<&Self>,
        binary_path: Option<&str>,
        modules_dir: Option<&str>,
    ) -> Result<Self> {
        if let Some(existing) = existing
            && existing.ownership != LogoscoreRuntimeOwnership::InspectorManaged
        {
            bail!("an external logoscore runtime cannot be started or owned by Inspector");
        }
        if existing.is_some_and(Self::is_running) {
            bail!("the Inspector-managed logoscore runtime is already running");
        }

        let binary_path = canonical_executable(
            binary_path.or_else(|| existing.map(|profile| profile.binary_path.as_str())),
        )?;
        let modules_dir = canonical_directory(
            modules_dir.or_else(|| existing.and_then(|profile| profile.modules_dir.as_deref())),
        )?;
        let runtime_root = config_root.join("logoscore-runtime");
        Ok(Self {
            id: MANAGED_RUNTIME_ID.to_owned(),
            binary_path,
            config_dir: runtime_root.join("logoscore").display().to_string(),
            modules_dir: Some(modules_dir),
            persistence_path: Some(runtime_root.join("node-data").display().to_string()),
            ownership: LogoscoreRuntimeOwnership::InspectorManaged,
            timeout_profile: LogoscoreTimeoutProfile::Lifecycle,
            daemon_process_id: None,
        })
    }

    #[must_use]
    pub(super) fn is_managed(&self) -> bool {
        self.ownership == LogoscoreRuntimeOwnership::InspectorManaged
    }

    #[must_use]
    pub(super) fn is_running(&self) -> bool {
        self.is_managed() && self.daemon_process_id.is_some_and(process_is_alive)
    }

    pub(super) fn cli_runtime(&self) -> Result<LogoscoreCliRuntime> {
        if !self.is_managed() {
            bail!("Inspector only controls explicitly managed logoscore runtimes");
        }
        if self.binary_path.trim().is_empty() || self.config_dir.trim().is_empty() {
            bail!("managed logoscore runtime is missing binary or config path");
        }
        Ok(LogoscoreCliRuntime::managed(
            self.binary_path.clone(),
            self.config_dir.clone(),
        ))
    }

    pub(super) fn daemon_command(&self) -> Result<Command> {
        let modules_dir = self
            .modules_dir
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .context("managed logoscore runtime has no modules directory")?;
        let persistence_path = self
            .persistence_path
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .context("managed logoscore runtime has no persistence path")?;
        Ok(self
            .cli_runtime()?
            .daemon_command(persistence_path, modules_dir))
    }

    pub(super) fn wait_until_ready(&self) -> Result<()> {
        let cli = self.cli_runtime()?;
        let deadline = Instant::now() + LIFECYCLE_TIMEOUT;
        let mut last_error = None;
        while Instant::now() < deadline {
            match cli.status_with_timeout(PROBE_TIMEOUT) {
                Ok(_) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
            thread::sleep(Duration::from_millis(100));
        }
        let detail = last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no probe response".to_owned());
        bail!("managed logoscore runtime did not become ready: {detail}")
    }

    pub(super) fn wait_until_ready_controlled(&self, control: &CommandControl) -> Result<()> {
        let cli = self.cli_runtime()?;
        let lifecycle_deadline = Instant::now() + LIFECYCLE_TIMEOUT;
        let mut last_error = None;
        while Instant::now() < lifecycle_deadline {
            control.check_active()?;
            let probe_deadline = match Instant::now().checked_add(PROBE_TIMEOUT) {
                Some(deadline) => deadline,
                None => control.deadline(),
            };
            let probe_control = control.with_deadline(probe_deadline);
            let probe_deadline = probe_control.deadline();
            match cli.status_controlled(probe_control) {
                Ok(_) => return Ok(()),
                Err(error) if probe_deadline < control.deadline() && is_probe_deadline(&error) => {
                    last_error = Some(error);
                }
                Err(error) => return Err(error),
            }
            controlled_sleep(control, Duration::from_millis(100))?;
        }
        let detail = last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no probe response".to_owned());
        bail!("managed logoscore runtime did not become ready: {detail}")
    }

    pub(super) fn wait_until_stopped(&self) -> bool {
        let deadline = Instant::now() + LIFECYCLE_TIMEOUT;
        while Instant::now() < deadline {
            if !self.daemon_process_id.is_some_and(process_is_alive) {
                return true;
            }
            thread::sleep(Duration::from_millis(100));
        }
        false
    }

    pub(super) fn wait_until_stopped_controlled(&self, control: &CommandControl) -> Result<bool> {
        let lifecycle_deadline = Instant::now() + LIFECYCLE_TIMEOUT;
        while Instant::now() < lifecycle_deadline {
            control.check_active()?;
            if !self.daemon_process_id.is_some_and(process_is_alive) {
                return Ok(true);
            }
            controlled_sleep(control, Duration::from_millis(100))?;
        }
        Ok(false)
    }

    fn validate_for_config_root(&self, config_root: &Path) -> Result<()> {
        if !self.is_managed() {
            return Ok(());
        }
        if self.id != MANAGED_RUNTIME_ID {
            bail!("managed logoscore runtime profile has an unexpected id");
        }
        if self.timeout_profile != LogoscoreTimeoutProfile::Lifecycle {
            bail!("managed logoscore runtime profile has an unexpected timeout profile");
        }
        if canonical_executable(Some(&self.binary_path))? != self.binary_path {
            bail!("managed logoscore runtime binary path is not canonical");
        }
        let runtime_root = config_root.join("logoscore-runtime");
        let expected_config = runtime_root.join("logoscore");
        let expected_persistence = runtime_root.join("node-data");
        if Path::new(&self.config_dir) != expected_config {
            bail!("managed logoscore runtime config path is outside the Inspector runtime root");
        }
        if self.persistence_path.as_deref().map(Path::new) != Some(expected_persistence.as_path()) {
            bail!("managed logoscore persistence path is outside the Inspector runtime root");
        }
        if self.modules_dir.as_deref().is_none_or(str::is_empty) {
            bail!("managed logoscore runtime has no modules directory");
        }
        Ok(())
    }
}

fn is_probe_deadline(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<CommandTerminated>()
        .is_some_and(|terminated| terminated.reason() == CommandStopReason::DeadlineExceeded)
}

fn controlled_sleep(control: &CommandControl, duration: Duration) -> Result<()> {
    control.check_active()?;
    let remaining = control.deadline().saturating_duration_since(Instant::now());
    thread::sleep(duration.min(remaining));
    control.check_active().map_err(Into::into)
}

#[derive(Debug, Clone, Serialize)]
pub struct LogoscoreRuntimeStatus {
    pub ownership: String,
    pub run_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modules_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub persistence_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_id: Option<u32>,
    pub detail: String,
}

#[must_use]
pub(super) fn status(profile: Option<&LogoscoreRuntimeProfile>) -> LogoscoreRuntimeStatus {
    let Some(profile) = profile else {
        return LogoscoreRuntimeStatus {
            ownership: "external".to_owned(),
            run_state: "not_configured".to_owned(),
            id: None,
            binary_path: None,
            config_dir: None,
            modules_dir: None,
            persistence_path: None,
            process_id: None,
            detail: "no Inspector-managed logoscore runtime configured".to_owned(),
        };
    };
    let running = profile.is_running();
    LogoscoreRuntimeStatus {
        ownership: match profile.ownership {
            LogoscoreRuntimeOwnership::External => "external",
            LogoscoreRuntimeOwnership::InspectorManaged => "inspector_managed",
        }
        .to_owned(),
        run_state: if running { "running" } else { "stopped" }.to_owned(),
        id: Some(profile.id.clone()),
        binary_path: Some(profile.binary_path.clone()),
        config_dir: Some(profile.config_dir.clone()),
        modules_dir: profile.modules_dir.clone(),
        persistence_path: profile.persistence_path.clone(),
        process_id: profile.daemon_process_id.filter(|_| running),
        detail: if running {
            "Inspector-managed logoscore daemon process is running".to_owned()
        } else if profile.is_managed() {
            "Inspector-managed logoscore daemon is stopped".to_owned()
        } else {
            "external logoscore runtime is not controlled by Inspector".to_owned()
        },
    }
}

#[derive(Debug, Clone)]
pub(super) struct LogoscoreRuntimeStore {
    config_dir: PathBuf,
}

impl LogoscoreRuntimeStore {
    pub(super) fn system(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub(super) fn load(&self) -> Result<Option<LogoscoreRuntimeProfile>> {
        let path = self.state_path();
        if !path.is_file() {
            return Ok(None);
        }
        let text = fs::read_to_string(&path).with_context(|| {
            format!(
                "failed to read logoscore runtime profile from {}",
                path.display()
            )
        })?;
        let profile: LogoscoreRuntimeProfile = serde_json::from_str(&text).with_context(|| {
            format!(
                "failed to parse logoscore runtime profile from {}",
                path.display()
            )
        })?;
        profile.validate_for_config_root(&self.config_dir)?;
        Ok(Some(profile))
    }

    pub(super) fn save(&self, profile: Option<&LogoscoreRuntimeProfile>) -> Result<()> {
        let path = self.state_path();
        let Some(profile) = profile else {
            if path.is_file() {
                fs::remove_file(&path).with_context(|| {
                    format!(
                        "failed to remove logoscore runtime profile {}",
                        path.display()
                    )
                })?;
            }
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let text = serde_json::to_string_pretty(profile)
            .context("failed to serialize logoscore runtime profile")?;
        fs::write(&path, text).with_context(|| {
            format!(
                "failed to write logoscore runtime profile to {}",
                path.display()
            )
        })
    }

    #[must_use]
    pub(super) fn config_root(&self) -> &Path {
        &self.config_dir
    }

    fn state_path(&self) -> PathBuf {
        self.config_dir.join(RUNTIME_FILE)
    }
}

fn canonical_executable(requested: Option<&str>) -> Result<String> {
    let requested = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            env::var("LOGOSCORE_BIN")
                .ok()
                .filter(|value| !value.trim().is_empty())
        })
        .or_else(|| find_command("logoscore"))
        .context("logoscore binary path is required")?;
    let path = fs::canonicalize(&requested)
        .with_context(|| format!("failed to resolve logoscore binary {requested}"))?;
    if !path.is_file() {
        bail!("logoscore binary is not a file: {}", path.display());
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;

        if fs::metadata(&path)?.permissions().mode() & 0o111 == 0 {
            bail!("logoscore binary is not executable: {}", path.display());
        }
    }
    if !path.is_absolute() {
        bail!("logoscore binary path must be absolute");
    }
    Ok(path.display().to_string())
}

fn canonical_directory(requested: Option<&str>) -> Result<String> {
    let requested = requested
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("logoscore modules directory is required")?;
    let path = fs::canonicalize(requested)
        .with_context(|| format!("failed to resolve logoscore modules directory {requested}"))?;
    if !path.is_dir() {
        bail!(
            "logoscore modules directory is not a directory: {}",
            path.display()
        );
    }
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn managed_runtime_uses_isolated_paths_and_daemon_argv() -> Result<()> {
        let root = env::temp_dir().join("logos-inspector-runtime-profile");
        let modules = env::temp_dir();
        let profile = LogoscoreRuntimeProfile::create_or_restart(
            &root,
            None,
            Some("/bin/sh"),
            Some(&modules.display().to_string()),
        )?;

        let command = profile.daemon_command()?;
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        let expected_config = root
            .join("logoscore-runtime/logoscore")
            .display()
            .to_string();
        let expected_persistence = root
            .join("logoscore-runtime/node-data")
            .display()
            .to_string();
        let expected_modules = fs::canonicalize(modules)?.display().to_string();

        let expected = vec![
            "--config-dir".to_owned(),
            expected_config,
            "--persistence-path".to_owned(),
            expected_persistence,
            "daemon".to_owned(),
            "--modules-dir".to_owned(),
            expected_modules,
        ];
        if args != expected
            || !profile.is_managed()
            || profile.timeout_profile != LogoscoreTimeoutProfile::Lifecycle
        {
            bail!("unexpected managed runtime daemon command: {args:?}");
        }
        Ok(())
    }

    #[test]
    fn unconfigured_runtime_is_external_and_not_configured() {
        let report = status(None);

        assert_eq!(report.ownership, "external");
        assert_eq!(report.run_state, "not_configured");
        assert!(report.process_id.is_none());
    }

    #[test]
    fn managed_runtime_profile_rejects_paths_outside_its_config_root() -> Result<()> {
        let root = env::temp_dir().join("logos-inspector-runtime-profile-boundary");
        let modules = env::temp_dir();
        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            &root,
            None,
            Some("/bin/sh"),
            Some(&modules.display().to_string()),
        )?;
        profile.config_dir = "/tmp/unmanaged-logoscore".to_owned();

        if profile.validate_for_config_root(&root).is_ok() {
            bail!("managed runtime accepted a config path outside its root");
        }
        Ok(())
    }
}
