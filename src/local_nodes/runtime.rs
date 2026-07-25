use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

#[cfg(target_os = "linux")]
use std::io::Read as _;

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::modules::logos_core::{LogosCoreOutput, LogoscoreCliRuntime};
use crate::support::command_runner::{
    CommandControl, CommandRunPolicy, DEFAULT_COMMAND_CAPTURE_LIMIT, run_command,
    run_command_controlled,
};

use super::process::{find_command, process_is_alive};

const RUNTIME_FILE: &str = "logoscore_runtime.json";
const MANAGED_RUNTIME_ID: &str = "inspector-managed-local";
const ATTACHED_RUNTIME_ID: &str = "local-attached";
const CHANNEL_INDEXER_RUNTIME_ID_PREFIX: &str = "inspector-managed-indexer-";
const PROBE_TIMEOUT: Duration = Duration::from_millis(400);
const ATTACHED_MODULES_VERIFICATION_TIMEOUT: Duration = Duration::from_secs(2);
const LIFECYCLE_TIMEOUT: Duration = Duration::from_secs(5);
const ATTACHED_SERVICE_READINESS_TIMEOUT: Duration = Duration::from_secs(45);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LogoscoreRuntimeOwnership {
    External,
    InspectorManaged,
    LocalAttached,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LogoscoreServiceScope {
    System,
    User,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LogoscoreServiceTarget {
    pub(super) scope: LogoscoreServiceScope,
    pub(super) unit: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LogoscoreServiceAction {
    Start,
    Stop,
}

impl LogoscoreServiceAction {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum LogoscoreServiceStopOutcome {
    Stopped,
    StoppedWithFailure(String),
    StillRunning(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LogoscoreTimeoutProfile {
    Probe,
    Lifecycle,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum LogoscoreRuntimeObservation {
    #[default]
    Verified,
    Unavailable,
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
    #[serde(default)]
    pub(super) observation: LogoscoreRuntimeObservation,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) daemon_process_id: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) service_target: Option<LogoscoreServiceTarget>,
}

impl LogoscoreRuntimeProfile {
    pub(super) fn create_or_restart(
        config_root: &Path,
        existing: Option<&Self>,
        binary_path: Option<&str>,
        modules_dir: Option<&str>,
    ) -> Result<Self> {
        if existing.is_some_and(Self::is_attached) {
            bail!(
                "the local LogosCore daemon is connected; use its verified service controls instead"
            );
        }
        if existing.is_some_and(|profile| profile.ownership == LogoscoreRuntimeOwnership::External)
        {
            bail!("a non-local logoscore runtime cannot be started by Inspector");
        }
        if existing.is_some_and(Self::is_running) {
            bail!("the local logoscore runtime is already running");
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
            observation: LogoscoreRuntimeObservation::Verified,
            daemon_process_id: None,
            service_target: None,
        })
    }

    pub(super) fn create_channel_indexer(
        config_root: &Path,
        network_scope_key: &str,
        channel_id: &str,
        base: &Self,
    ) -> Result<Self> {
        if !base.is_managed() {
            bail!("a local attached logoscore runtime cannot provide an isolated Channel Indexer");
        }
        base.validate_for_config_root(config_root)?;
        validate_runtime_key(network_scope_key, "Channel Indexer network scope")?;
        validate_runtime_key(channel_id, "Channel Indexer ID")?;

        let binary_path = canonical_executable(Some(&base.binary_path))?;
        let modules_dir = canonical_directory(base.modules_dir.as_deref())?;
        let runtime_root = config_root
            .join("channel-indexers")
            .join(network_scope_key)
            .join(channel_id);
        Ok(Self {
            id: format!("{CHANNEL_INDEXER_RUNTIME_ID_PREFIX}{network_scope_key}-{channel_id}"),
            binary_path,
            config_dir: runtime_root.join("logoscore").display().to_string(),
            modules_dir: Some(modules_dir),
            persistence_path: Some(runtime_root.join("node-data").display().to_string()),
            ownership: LogoscoreRuntimeOwnership::InspectorManaged,
            timeout_profile: LogoscoreTimeoutProfile::Lifecycle,
            observation: LogoscoreRuntimeObservation::Verified,
            daemon_process_id: None,
            service_target: None,
        })
    }

    pub(super) fn discover_local() -> Result<Option<Self>> {
        let Ok(binary_path) = canonical_executable(None) else {
            return Ok(None);
        };
        let Some(config_dir) = local_client_config_dir() else {
            return Ok(None);
        };
        if !local_client_transport_is_proven(Path::new(&config_dir)) {
            return Ok(None);
        }
        Ok(Some(Self::discover_proven_local_transport(
            binary_path,
            config_dir,
        )?))
    }

    fn discover_proven_local_transport(binary_path: String, config_dir: String) -> Result<Self> {
        let mut profile = Self::local_attached_profile(binary_path, config_dir);
        // Discovery is a display observation. Reuse a fresh status snapshot so
        // concurrent watcher activity cannot replace a known local service
        // with an unavailable state between UI refreshes. Package mutation
        // always uses its own fresh probe below.
        let output = profile.cli_runtime()?.status_with_timeout(PROBE_TIMEOUT);
        profile.apply_local_status_probe(output);
        Ok(profile)
    }

    fn local_attached_profile(binary_path: String, config_dir: String) -> Self {
        Self {
            id: ATTACHED_RUNTIME_ID.to_owned(),
            binary_path,
            config_dir,
            modules_dir: None,
            persistence_path: None,
            ownership: LogoscoreRuntimeOwnership::LocalAttached,
            timeout_profile: LogoscoreTimeoutProfile::Probe,
            observation: LogoscoreRuntimeObservation::Verified,
            daemon_process_id: None,
            service_target: None,
        }
    }

    fn apply_local_status_probe(&mut self, output: Result<LogosCoreOutput>) {
        self.daemon_process_id = None;
        self.service_target = None;
        self.observation = LogoscoreRuntimeObservation::Verified;
        let Ok(output) = output else {
            self.observation = LogoscoreRuntimeObservation::Unavailable;
            return;
        };
        if let Some(process_id) = running_daemon_process_id(&output.value) {
            self.daemon_process_id = Some(process_id);
            self.service_target = system_service_target(process_id);
        }
    }

    #[must_use]
    pub(super) fn is_managed(&self) -> bool {
        self.ownership == LogoscoreRuntimeOwnership::InspectorManaged
    }

    #[must_use]
    pub(super) fn is_attached(&self) -> bool {
        self.ownership == LogoscoreRuntimeOwnership::LocalAttached
    }

    #[must_use]
    pub(super) fn is_unavailable(&self) -> bool {
        self.is_attached() && self.observation == LogoscoreRuntimeObservation::Unavailable
    }

    #[must_use]
    pub(super) fn is_controllable(&self) -> bool {
        self.is_managed() || self.service_target().is_some()
    }

    #[must_use]
    pub(super) fn is_running(&self) -> bool {
        if self.is_unavailable() {
            return false;
        }
        match self.ownership {
            LogoscoreRuntimeOwnership::InspectorManaged => {
                self.daemon_process_id.is_some_and(process_is_alive)
            }
            // The current user may legitimately lack signal permission for a daemon owned by
            // another local service account. The successful CLI status probe is authoritative.
            LogoscoreRuntimeOwnership::LocalAttached => self.daemon_process_id.is_some(),
            LogoscoreRuntimeOwnership::External => false,
        }
    }

    pub(super) fn validate_attached_module_install_target(&self, requested: &Path) -> Result<()> {
        if !self.is_attached() {
            bail!(
                "only a local attached LogosCore runtime can verify its active modules directory"
            );
        }
        let output = self
            .cli_runtime()?
            .status_probe_with_timeout(ATTACHED_MODULES_VERIFICATION_TIMEOUT)
            .context(
                "cannot verify the local LogosCore daemon before changing installed modules; wait for the connection to recover or stop the service",
            )?;
        let Some(process_id) = running_daemon_process_id(&output.value) else {
            return Ok(());
        };
        let active_modules_dir = self.modules_dir_for_attached_process(process_id);
        validate_active_attached_module_install_target(active_modules_dir.as_deref(), requested)
    }

    /// Returns the module directory from the command line of the daemon that
    /// was just discovered by the local CLI. This is display-only evidence;
    /// package mutations must use [`Self::validate_attached_module_install_target`].
    #[must_use]
    fn observed_attached_modules_dir(&self) -> Option<PathBuf> {
        (self.is_attached() && self.is_running())
            .then_some(self.daemon_process_id)
            .flatten()
            .and_then(running_daemon_modules_dir)
    }

    fn modules_dir_for_attached_process(&self, process_id: u32) -> Option<PathBuf> {
        let modules_dir = running_daemon_modules_dir(process_id)?;
        if self
            .service_target()
            .is_some_and(|target| system_service_main_process_id(target) != Some(process_id))
        {
            return None;
        }
        Some(modules_dir)
    }

    pub(super) fn cli_runtime(&self) -> Result<LogoscoreCliRuntime> {
        if self.binary_path.trim().is_empty() || self.config_dir.trim().is_empty() {
            bail!("local logoscore runtime is missing binary or config path");
        }
        if self.is_attached() {
            return Ok(LogoscoreCliRuntime::local(
                self.binary_path.clone(),
                self.config_dir.clone(),
            ));
        }
        if !self.is_managed() {
            bail!("Inspector cannot control a non-local logoscore runtime");
        }
        Ok(LogoscoreCliRuntime::managed(
            self.binary_path.clone(),
            self.config_dir.clone(),
        ))
    }

    pub(super) fn daemon_command(&self) -> Result<Command> {
        if !self.is_managed() {
            bail!("only an Inspector-managed logoscore runtime can be started directly");
        }
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
        self.wait_until_ready_with_timeout(self.readiness_timeout())
    }

    fn wait_until_ready_with_timeout(&self, timeout: Duration) -> Result<()> {
        let cli = self.cli_runtime()?;
        cli.invalidate_observation_snapshot()?;
        let deadline = Instant::now() + timeout;
        let mut last_error = None;
        while Instant::now() < deadline {
            match cli.status_probe_with_timeout(PROBE_TIMEOUT) {
                Ok(_) => return Ok(()),
                Err(error) => last_error = Some(error),
            }
            thread::sleep(Duration::from_millis(100));
        }
        let detail = last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no probe response".to_owned());
        bail!("local logoscore runtime did not become ready: {detail}")
    }

    pub(super) fn wait_until_ready_controlled(&self, control: &CommandControl) -> Result<()> {
        self.wait_until_ready_controlled_with_timeout(control, self.readiness_timeout())
    }

    fn wait_until_ready_controlled_with_timeout(
        &self,
        control: &CommandControl,
        timeout: Duration,
    ) -> Result<()> {
        let cli = self.cli_runtime()?;
        let lifecycle_deadline = Instant::now() + timeout;
        let mut last_error = None;
        while Instant::now() < lifecycle_deadline {
            control.check_active()?;
            let probe_deadline = match Instant::now().checked_add(PROBE_TIMEOUT) {
                Some(deadline) => deadline,
                None => control.deadline(),
            };
            let probe_control = control.with_deadline(probe_deadline);
            match cli.status_controlled(probe_control) {
                Ok(_) => return Ok(()),
                Err(error) => {
                    control.check_active()?;
                    last_error = Some(error);
                }
            }
            controlled_sleep(control, Duration::from_millis(100))?;
        }
        let detail = last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "no probe response".to_owned());
        bail!("local logoscore runtime did not become ready: {detail}")
    }

    fn readiness_timeout(&self) -> Duration {
        if self.is_attached() {
            ATTACHED_SERVICE_READINESS_TIMEOUT
        } else {
            LIFECYCLE_TIMEOUT
        }
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

    pub(super) fn validate_for_config_root(&self, config_root: &Path) -> Result<()> {
        if self.is_attached() {
            return self.validate_attached();
        }
        if !self.is_managed() {
            return Ok(());
        }
        let (expected_config, expected_persistence) = managed_runtime_paths(config_root, &self.id)?;
        if self.timeout_profile != LogoscoreTimeoutProfile::Lifecycle {
            bail!("managed logoscore runtime profile has an unexpected timeout profile");
        }
        if canonical_executable(Some(&self.binary_path))? != self.binary_path {
            bail!("managed logoscore runtime binary path is not canonical");
        }
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

    fn validate_attached(&self) -> Result<()> {
        if self.id != ATTACHED_RUNTIME_ID {
            bail!("local attached logoscore runtime profile has an unexpected id");
        }
        if self.timeout_profile != LogoscoreTimeoutProfile::Probe {
            bail!("local attached logoscore runtime profile has an unexpected timeout profile");
        }
        if canonical_executable(Some(&self.binary_path))? != self.binary_path {
            bail!("local attached logoscore runtime binary path is not canonical");
        }
        if self.config_dir.trim().is_empty() || !Path::new(&self.config_dir).is_absolute() {
            bail!("local attached logoscore runtime config path must be absolute");
        }
        if self.modules_dir.is_some() || self.persistence_path.is_some() {
            bail!("local attached logoscore runtime must not declare managed paths");
        }
        if let Some(target) = &self.service_target
            && !valid_systemd_unit(&target.unit)
        {
            bail!("local attached logoscore service unit is invalid");
        }
        Ok(())
    }

    #[must_use]
    pub(super) fn service_target(&self) -> Option<&LogoscoreServiceTarget> {
        if self.is_attached() {
            self.service_target.as_ref()
        } else {
            None
        }
    }

    pub(super) fn control_attached_service(
        &self,
        action: LogoscoreServiceAction,
        control: Option<&CommandControl>,
    ) -> Result<String> {
        let target = self
            .service_target()
            .context("local LogosCore daemon has no verified service lifecycle backend")?;
        let (command, display) = service_action_command(target, action);
        let policy = CommandRunPolicy {
            label: "local LogosCore service control",
            timeout: LIFECYCLE_TIMEOUT,
            poll_interval: Duration::from_millis(25),
            redactions: &[],
            output_limit: 4096,
            capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
        };
        match control {
            Some(control) => run_command_controlled(command, policy, control.clone()),
            None => run_command(command, policy),
        }
        .with_context(|| {
            format!(
                "failed to {} local LogosCore service `{}`",
                action.as_str(),
                target.unit
            )
        })?;
        self.cli_runtime()?.invalidate_observation_snapshot()?;
        Ok(display)
    }

    pub(super) fn attached_service_stop_outcome(
        &self,
        control: Option<&CommandControl>,
    ) -> Result<LogoscoreServiceStopOutcome> {
        let target = self
            .service_target()
            .context("local LogosCore daemon has no verified service lifecycle backend")?;
        system_service_stop_status(target, control).map(SystemServiceStopStatus::outcome)
    }

    pub(super) fn refresh_attached_process_id(&mut self) -> Result<()> {
        if !self.is_attached() {
            bail!("only a local attached LogosCore runtime can refresh service state");
        }
        let target = self
            .service_target()
            .context("local LogosCore daemon has no verified service lifecycle backend")?;
        let output = self.cli_runtime()?.status_with_timeout(PROBE_TIMEOUT)?;
        let process_id = running_daemon_process_id(&output.value)
            .context("local LogosCore daemon did not report a running process id")?;
        let service_process_id = system_service_main_process_id(target)
            .context("local LogosCore service did not report a main process id")?;
        if service_process_id != process_id {
            bail!(
                "local LogosCore daemon process id {process_id} does not match service `{}` main process id {service_process_id}",
                target.unit
            );
        }
        self.daemon_process_id = Some(process_id);
        Ok(())
    }

    fn reports_running(&self) -> bool {
        self.cli_runtime()
            .and_then(|runtime| runtime.status_with_timeout(PROBE_TIMEOUT))
            .ok()
            .and_then(|output| running_daemon_process_id(&output.value))
            .is_some()
    }
}

fn managed_runtime_paths(config_root: &Path, id: &str) -> Result<(PathBuf, PathBuf)> {
    if id == MANAGED_RUNTIME_ID {
        let runtime_root = config_root.join("logoscore-runtime");
        return Ok((
            runtime_root.join("logoscore"),
            runtime_root.join("node-data"),
        ));
    }

    let Some(key) = id.strip_prefix(CHANNEL_INDEXER_RUNTIME_ID_PREFIX) else {
        bail!("managed logoscore runtime profile has an unexpected id");
    };
    let Some((network_scope_key, channel_id)) = key.split_once('-') else {
        bail!("Channel Indexer runtime id is invalid");
    };
    validate_runtime_key(network_scope_key, "Channel Indexer network scope")?;
    validate_runtime_key(channel_id, "Channel Indexer ID")?;
    let runtime_root = config_root
        .join("channel-indexers")
        .join(network_scope_key)
        .join(channel_id);
    Ok((
        runtime_root.join("logoscore"),
        runtime_root.join("node-data"),
    ))
}

fn validate_runtime_key(value: &str, label: &str) -> Result<()> {
    if value.len() != 64
        || !value.bytes().all(|byte| {
            byte.is_ascii_digit() || (byte.is_ascii_lowercase() && byte.is_ascii_hexdigit())
        })
    {
        bail!("{label} must be exactly 64 lowercase hexadecimal characters");
    }
    Ok(())
}

fn local_client_config_dir() -> Option<String> {
    let configured = env::var("LOGOSCORE_CONFIG_DIR")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            env::var("LOGOSCORE_HOME")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|home| PathBuf::from(home).join(".logoscore"))
        })
        .or_else(|| {
            env::var("HOME")
                .ok()
                .filter(|value| !value.trim().is_empty())
                .map(|home| PathBuf::from(home).join(".logoscore"))
        })?;
    let absolute = if configured.is_absolute() {
        configured
    } else {
        env::current_dir().ok()?.join(configured)
    };
    Some(absolute.display().to_string())
}

fn local_client_transport_is_proven(config_dir: &Path) -> bool {
    const CLIENT_CONFIG_LIMIT: u64 = 64 * 1024;

    let path = config_dir.join("client/config.json");
    let Ok(metadata) = fs::metadata(&path) else {
        return false;
    };
    if !metadata.is_file() || metadata.len() > CLIENT_CONFIG_LIMIT {
        return false;
    }
    let Ok(bytes) = fs::read(path) else {
        return false;
    };
    let Ok(value) = serde_json::from_slice::<Value>(&bytes) else {
        return false;
    };
    value
        .pointer("/daemon/core_service/transport")
        .and_then(Value::as_str)
        .is_some_and(|transport| transport == "local")
}

fn running_daemon_process_id(status: &Value) -> Option<u32> {
    let daemon = status.get("daemon")?.as_object()?;
    (daemon.get("status")?.as_str()? == "running")
        .then(|| daemon.get("pid")?.as_u64())
        .flatten()
        .and_then(|pid| u32::try_from(pid).ok())
}

fn validate_active_attached_module_install_target(
    active_modules_dir: Option<&Path>,
    requested_modules_dir: &Path,
) -> Result<()> {
    let Some(active_modules_dir) = active_modules_dir else {
        bail!(
            "cannot verify the local LogosCore modules directory while it is running; stop the local LogosCore runtime before changing installed modules"
        );
    };
    if requested_modules_dir == active_modules_dir {
        bail!("stop the local LogosCore runtime before changing installed modules");
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn running_daemon_modules_dir(process_id: u32) -> Option<PathBuf> {
    const COMMAND_LINE_LIMIT: u64 = 64 * 1024;

    let mut bytes = Vec::new();
    fs::File::open(format!("/proc/{process_id}/cmdline"))
        .ok()?
        .take(COMMAND_LINE_LIMIT.saturating_add(1))
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() > COMMAND_LINE_LIMIT as usize {
        return None;
    }
    let mut arguments = Vec::new();
    for value in bytes.split(|byte| *byte == 0) {
        if value.is_empty() {
            continue;
        }
        arguments.push(std::str::from_utf8(value).ok()?);
    }
    modules_dir_from_daemon_arguments(arguments)
}

#[cfg(not(target_os = "linux"))]
fn running_daemon_modules_dir(_process_id: u32) -> Option<PathBuf> {
    None
}

fn modules_dir_from_daemon_arguments(arguments: Vec<&str>) -> Option<PathBuf> {
    let mut next_is_modules_dir = false;
    let mut modules_dir = None;
    for argument in arguments {
        let candidate = if next_is_modules_dir {
            next_is_modules_dir = false;
            Some(argument)
        } else if matches!(argument, "-m" | "--modules-dir") {
            next_is_modules_dir = true;
            None
        } else {
            argument.strip_prefix("--modules-dir=")
        };
        let Some(candidate) = candidate else {
            continue;
        };
        let resolved = canonical_directory(Some(candidate)).ok()?;
        if modules_dir.replace(PathBuf::from(resolved)).is_some() {
            return None;
        }
    }
    (!next_is_modules_dir).then_some(modules_dir).flatten()
}

#[cfg(target_os = "linux")]
fn system_service_target(process_id: u32) -> Option<LogoscoreServiceTarget> {
    let cgroup = fs::read_to_string(format!("/proc/{process_id}/cgroup")).ok()?;
    let target = cgroup.lines().find_map(system_service_target_from_cgroup)?;
    system_service_main_process_id(&target)
        .filter(|main_process_id| *main_process_id == process_id)
        .map(|_| target)
}

#[cfg(not(target_os = "linux"))]
fn system_service_target(_process_id: u32) -> Option<LogoscoreServiceTarget> {
    None
}

#[cfg(any(target_os = "linux", test))]
fn system_service_target_from_cgroup(line: &str) -> Option<LogoscoreServiceTarget> {
    let (_, path) = line.rsplit_once(':')?;
    let unit = path.rsplit('/').next()?.trim();
    if !valid_systemd_unit(unit) {
        return None;
    }
    let scope = if path.starts_with("/user.slice/") {
        LogoscoreServiceScope::User
    } else {
        LogoscoreServiceScope::System
    };
    Some(LogoscoreServiceTarget {
        scope,
        unit: unit.to_owned(),
    })
}

fn valid_systemd_unit(unit: &str) -> bool {
    unit.len() > ".service".len()
        && unit.len() <= 255
        && unit.ends_with(".service")
        && unit
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'@'))
}

fn system_service_main_process_id(target: &LogoscoreServiceTarget) -> Option<u32> {
    let mut command = Command::new("systemctl");
    if target.scope == LogoscoreServiceScope::User {
        command.arg("--user");
    }
    command
        .arg("show")
        .arg("--property=MainPID")
        .arg("--value")
        .arg(&target.unit);
    let output = run_command(
        command,
        CommandRunPolicy {
            label: "local LogosCore service probe",
            timeout: PROBE_TIMEOUT,
            poll_interval: Duration::from_millis(25),
            redactions: &[],
            output_limit: 1024,
            capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
        },
    )
    .ok()?;
    std::str::from_utf8(&output.stdout)
        .ok()?
        .trim()
        .parse::<u32>()
        .ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SystemServiceStopStatus {
    active_state: String,
    sub_state: String,
    result: String,
    exec_main_code: String,
    exec_main_status: String,
}

impl SystemServiceStopStatus {
    fn outcome(self) -> LogoscoreServiceStopOutcome {
        let detail = self.detail();
        if self.active_state == "inactive" && self.sub_state == "dead" && self.result == "success" {
            LogoscoreServiceStopOutcome::Stopped
        } else if matches!(self.active_state.as_str(), "inactive" | "failed") {
            LogoscoreServiceStopOutcome::StoppedWithFailure(detail)
        } else {
            LogoscoreServiceStopOutcome::StillRunning(detail)
        }
    }

    fn detail(&self) -> String {
        let properties = [
            format!("ActiveState={}", self.active_state),
            format!("SubState={}", self.sub_state),
            format!("Result={}", self.result),
            format!("ExecMainCode={}", self.exec_main_code),
            format!("ExecMainStatus={}", self.exec_main_status),
        ];
        properties.join(", ")
    }
}

fn system_service_stop_status(
    target: &LogoscoreServiceTarget,
    control: Option<&CommandControl>,
) -> Result<SystemServiceStopStatus> {
    let command = service_stop_status_command(target);
    let policy = CommandRunPolicy {
        label: "local LogosCore service stop status",
        timeout: LIFECYCLE_TIMEOUT,
        poll_interval: Duration::from_millis(25),
        redactions: &[],
        output_limit: 1024,
        capture_limit: DEFAULT_COMMAND_CAPTURE_LIMIT,
    };
    let output = match control {
        Some(control) => run_command_controlled(
            command,
            policy,
            control.with_deadline(Instant::now() + LIFECYCLE_TIMEOUT),
        ),
        None => run_command(command, policy),
    }
    .with_context(|| {
        format!(
            "failed to inspect local LogosCore service `{}`",
            target.unit
        )
    })?;
    parse_system_service_stop_status(&output.stdout).with_context(|| {
        format!(
            "local LogosCore service `{}` returned incomplete state",
            target.unit
        )
    })
}

fn service_stop_status_command(target: &LogoscoreServiceTarget) -> Command {
    let mut command = Command::new("systemctl");
    if target.scope == LogoscoreServiceScope::User {
        command.arg("--user");
    }
    command.args([
        "show",
        "--property=ActiveState",
        "--property=SubState",
        "--property=Result",
        "--property=ExecMainCode",
        "--property=ExecMainStatus",
    ]);
    command.arg(&target.unit);
    command
}

fn parse_system_service_stop_status(output: &[u8]) -> Result<SystemServiceStopStatus> {
    let text = std::str::from_utf8(output).context("system service status is not UTF-8")?;
    let mut active_state = None;
    let mut sub_state = None;
    let mut result = None;
    let mut exec_main_code = None;
    let mut exec_main_status = None;
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        match key {
            "ActiveState" => set_system_service_property(&mut active_state, key, value)?,
            "SubState" => set_system_service_property(&mut sub_state, key, value)?,
            "Result" => set_system_service_property(&mut result, key, value)?,
            "ExecMainCode" => set_system_service_property(&mut exec_main_code, key, value)?,
            "ExecMainStatus" => set_system_service_property(&mut exec_main_status, key, value)?,
            _ => {}
        }
    }
    Ok(SystemServiceStopStatus {
        active_state: active_state.context("missing ActiveState")?,
        sub_state: sub_state.context("missing SubState")?,
        result: result.context("missing Result")?,
        exec_main_code: exec_main_code.context("missing ExecMainCode")?,
        exec_main_status: exec_main_status.context("missing ExecMainStatus")?,
    })
}

fn set_system_service_property(slot: &mut Option<String>, key: &str, value: &str) -> Result<()> {
    if slot.replace(value.to_owned()).is_some() {
        bail!("duplicate {key}");
    }
    Ok(())
}

fn service_action_command(
    target: &LogoscoreServiceTarget,
    action: LogoscoreServiceAction,
) -> (Command, String) {
    let mut display = String::new();
    let mut command = if target.scope == LogoscoreServiceScope::System {
        display.push_str("sudo -n -- systemctl");
        let mut command = Command::new("sudo");
        command.arg("-n").arg("--").arg("systemctl");
        command
    } else {
        display.push_str("systemctl --user");
        let mut command = Command::new("systemctl");
        command.arg("--user");
        command
    };
    display.push(' ');
    display.push_str(action.as_str());
    display.push(' ');
    display.push_str(&target.unit);
    command.arg(action.as_str()).arg(&target.unit);
    (command, display)
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_unit: Option<String>,
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
            service_unit: None,
            detail: "no local logoscore runtime configured".to_owned(),
        };
    };
    let unavailable = profile.is_unavailable();
    let running = profile.is_running();
    let service_unit = profile.service_target().map(|target| target.unit.clone());
    let observed_modules_dir = profile
        .observed_attached_modules_dir()
        .map(|path| path.display().to_string());
    let modules_dir = profile
        .modules_dir
        .clone()
        .or_else(|| observed_modules_dir.clone());
    LogoscoreRuntimeStatus {
        ownership: match profile.ownership {
            LogoscoreRuntimeOwnership::External => "external",
            LogoscoreRuntimeOwnership::InspectorManaged => "inspector_managed",
            LogoscoreRuntimeOwnership::LocalAttached => "local_attached",
        }
        .to_owned(),
        run_state: if unavailable {
            "unavailable"
        } else if running {
            "running"
        } else {
            "stopped"
        }
        .to_owned(),
        id: Some(profile.id.clone()),
        binary_path: Some(profile.binary_path.clone()),
        config_dir: Some(profile.config_dir.clone()),
        modules_dir,
        persistence_path: profile.persistence_path.clone(),
        process_id: profile.daemon_process_id.filter(|_| running),
        service_unit: service_unit.clone(),
        detail: runtime_status_detail(
            profile,
            running,
            service_unit.as_deref(),
            observed_modules_dir.as_deref(),
        ),
    }
}

fn runtime_status_detail(
    profile: &LogoscoreRuntimeProfile,
    running: bool,
    service_unit: Option<&str>,
    observed_modules_dir: Option<&str>,
) -> String {
    if profile.is_unavailable() {
        let target = service_unit
            .map(|unit| format!(" for system service `{unit}`"))
            .unwrap_or_default();
        return format!(
            "Inspector cannot verify the local LogosCore daemon{target}; package changes stay disabled until the connection recovers"
        );
    }
    let detail = match (profile.ownership, running, service_unit) {
        (LogoscoreRuntimeOwnership::LocalAttached, true, Some(unit)) => {
            format!("local LogosCore daemon is running under system service `{unit}`")
        }
        (LogoscoreRuntimeOwnership::LocalAttached, true, None) => {
            "local LogosCore daemon is running through the local CLI connection".to_owned()
        }
        (LogoscoreRuntimeOwnership::LocalAttached, false, Some(unit)) => {
            format!("local LogosCore daemon is stopped; system service `{unit}` can start it")
        }
        (LogoscoreRuntimeOwnership::LocalAttached, false, None) => {
            "local LogosCore daemon is stopped".to_owned()
        }
        (LogoscoreRuntimeOwnership::InspectorManaged, true, _) => {
            "local LogosCore daemon is running".to_owned()
        }
        (LogoscoreRuntimeOwnership::InspectorManaged, false, _) => {
            "local LogosCore daemon is stopped".to_owned()
        }
        (LogoscoreRuntimeOwnership::External, _, _) => {
            "non-local LogosCore runtime is not controlled by Inspector".to_owned()
        }
    };
    if let Some(modules_dir) = observed_modules_dir {
        return format!("{detail}; observed modules directory `{modules_dir}`");
    }
    detail
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
        if profile.is_managed() && profile.id != MANAGED_RUNTIME_ID {
            bail!("managed logoscore runtime profile has an unexpected global runtime id");
        }
        Ok(Some(profile))
    }

    pub(super) fn load_resolved(&self) -> Result<Option<LogoscoreRuntimeProfile>> {
        let mut persisted = self.load()?;
        if persisted.as_ref().is_some_and(|profile| {
            profile.is_managed() && profile.is_running() && profile.reports_running()
        }) {
            return Ok(persisted);
        }
        if let Some(profile) = persisted.as_mut()
            && (profile.is_managed() || profile.is_attached())
        {
            profile.daemon_process_id = None;
        }
        let discovered = LogoscoreRuntimeProfile::discover_local()?;
        Ok(resolve_local_runtime_observation(discovered, persisted))
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

fn resolve_local_runtime_observation(
    discovered: Option<LogoscoreRuntimeProfile>,
    persisted: Option<LogoscoreRuntimeProfile>,
) -> Option<LogoscoreRuntimeProfile> {
    let Some(mut discovered) = discovered else {
        return persisted.map(|mut profile| {
            if profile.is_attached() {
                profile.observation = LogoscoreRuntimeObservation::Unavailable;
                profile.daemon_process_id = None;
            }
            profile
        });
    };
    if !discovered.is_attached() {
        return Some(discovered);
    }
    let Some(previous) = persisted.filter(|profile| {
        profile.is_attached()
            && profile.binary_path == discovered.binary_path
            && profile.config_dir == discovered.config_dir
    }) else {
        return Some(discovered);
    };
    if discovered.daemon_process_id.is_none() {
        discovered.service_target = previous.service_target;
    }
    Some(discovered)
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
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn attached_profile(
        daemon_process_id: Option<u32>,
        service_target: Option<LogoscoreServiceTarget>,
    ) -> LogoscoreRuntimeProfile {
        LogoscoreRuntimeProfile {
            id: ATTACHED_RUNTIME_ID.to_owned(),
            binary_path: "/bin/sh".to_owned(),
            config_dir: "/tmp/logoscore-client".to_owned(),
            modules_dir: None,
            persistence_path: None,
            ownership: LogoscoreRuntimeOwnership::LocalAttached,
            timeout_profile: LogoscoreTimeoutProfile::Probe,
            observation: LogoscoreRuntimeObservation::Verified,
            daemon_process_id,
            service_target,
        }
    }

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
    fn channel_indexer_profiles_use_distinct_scope_and_channel_roots() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules = tempfile::tempdir()?;
        let base = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some("/bin/sh"),
            Some(&modules.path().display().to_string()),
        )?;
        let scope = "ab".repeat(32);
        let first_channel = "01".repeat(32);
        let second_channel = "88".repeat(32);
        let first = LogoscoreRuntimeProfile::create_channel_indexer(
            directory.path(),
            &scope,
            &first_channel,
            &base,
        )?;
        let second = LogoscoreRuntimeProfile::create_channel_indexer(
            directory.path(),
            &scope,
            &second_channel,
            &base,
        )?;

        anyhow::ensure!(first.id != second.id);
        anyhow::ensure!(first.config_dir != second.config_dir);
        anyhow::ensure!(first.persistence_path != second.persistence_path);
        first.validate_for_config_root(directory.path())?;
        second.validate_for_config_root(directory.path())?;
        Ok(())
    }

    #[test]
    fn global_runtime_store_rejects_a_channel_indexer_profile() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules = tempfile::tempdir()?;
        let base = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some("/bin/sh"),
            Some(&modules.path().display().to_string()),
        )?;
        let profile = LogoscoreRuntimeProfile::create_channel_indexer(
            directory.path(),
            &"ab".repeat(32),
            &"01".repeat(32),
            &base,
        )?;
        let store = LogoscoreRuntimeStore::system(directory.path().to_path_buf());
        store.save(Some(&profile))?;

        let error = match store.load() {
            Ok(_) => anyhow::bail!("global store accepted a Channel runtime"),
            Err(error) => error,
        };
        anyhow::ensure!(error.to_string().contains("unexpected global runtime id"));
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn controlled_readiness_retries_not_configured_until_runtime_is_ready() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let binary = directory.path().join("logoscore");
        fs::write(
            &binary,
            r#"#!/bin/sh
marker="$0.calls"
printf '%s\n' status >> "$marker"
if [ "$(wc -l < "$marker")" -eq 1 ]; then
    printf '%s\n' '{"daemon":{"status":"not_configured"}}'
    exit 1
fi
printf '%s\n' '{"daemon":{"status":"running"}}'
"#,
        )?;
        let mut permissions = fs::metadata(&binary)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions)?;
        let profile = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(&binary.display().to_string()),
            Some(&directory.path().display().to_string()),
        )?;
        let deadline = Instant::now()
            .checked_add(Duration::from_secs(2))
            .context("controlled readiness test deadline overflow")?;
        let control = CommandControl::new(tokio_util::sync::CancellationToken::new(), deadline);

        profile.wait_until_ready_controlled(&control)?;

        let calls = fs::read_to_string(binary.with_extension("calls"))?
            .lines()
            .count();
        if calls != 2 {
            bail!("controlled readiness made {calls} status probes instead of retrying once");
        }
        Ok(())
    }

    #[test]
    fn attached_runtime_readiness_budget_exceeds_managed_lifecycle_budget() {
        let attached = attached_profile(None, None);
        assert_eq!(
            attached.readiness_timeout(),
            ATTACHED_SERVICE_READINESS_TIMEOUT
        );
        assert!(attached.readiness_timeout() > LIFECYCLE_TIMEOUT);

        let mut managed = attached;
        managed.ownership = LogoscoreRuntimeOwnership::InspectorManaged;
        assert_eq!(managed.readiness_timeout(), LIFECYCLE_TIMEOUT);
    }

    #[cfg(unix)]
    #[test]
    fn readiness_wait_accepts_delayed_cli_status_with_supplied_timeout() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let binary = directory.path().join("logoscore");
        fs::write(
            &binary,
            r#"#!/bin/sh
marker="$0.calls"
count=0
if [ -f "$marker" ]; then
    count="$(wc -l < "$marker")"
fi
printf '%s\n' status >> "$marker"
if [ "$count" -lt 2 ]; then
    printf '%s\n' '{"daemon":{"status":"not_configured"}}'
    exit 1
fi
printf '%s\n' '{"daemon":{"status":"running","pid":42}}'
"#,
        )?;
        let mut permissions = fs::metadata(&binary)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&binary, permissions)?;

        let mut profile = attached_profile(None, None);
        profile.binary_path = binary.display().to_string();
        profile
            .wait_until_ready_with_timeout(Duration::from_millis(50))
            .expect_err("short readiness budget unexpectedly accepted delayed CLI status");

        fs::remove_file(binary.with_extension("calls"))?;
        profile.wait_until_ready_with_timeout(Duration::from_millis(500))?;
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
    fn local_client_transport_requires_an_explicit_local_core_service() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let client = directory.path().join("client");
        fs::create_dir_all(&client)?;
        let config = client.join("config.json");
        fs::write(
            &config,
            r#"{"daemon":{"core_service":{"transport":"local"}}}"#,
        )?;
        anyhow::ensure!(local_client_transport_is_proven(directory.path()));

        fs::write(
            &config,
            r#"{"daemon":{"core_service":{"transport":"remote"}}}"#,
        )?;
        anyhow::ensure!(!local_client_transport_is_proven(directory.path()));
        Ok(())
    }

    #[test]
    fn running_daemon_process_id_accepts_only_a_running_u32_process() {
        assert_eq!(
            running_daemon_process_id(&json!({"daemon":{"status":"running","pid":42}})),
            Some(42)
        );
        assert_eq!(
            running_daemon_process_id(&json!({"daemon":{"status":"stopped","pid":42}})),
            None
        );
        assert_eq!(
            running_daemon_process_id(&json!({
                "daemon":{"status":"running","pid":u64::from(u32::MAX) + 1}
            })),
            None
        );
    }

    #[test]
    fn cgroup_service_target_only_accepts_valid_service_units() {
        let system = system_service_target_from_cgroup("0::/system.slice/logos-node.service")
            .expect("system service target");
        assert_eq!(system.scope, LogoscoreServiceScope::System);
        assert_eq!(system.unit, "logos-node.service");

        let user = system_service_target_from_cgroup(
            "0::/user.slice/user-1000.slice/user@1000.service/app.slice/logos-node.service",
        )
        .expect("user service target");
        assert_eq!(user.scope, LogoscoreServiceScope::User);
        assert_eq!(user.unit, "logos-node.service");
        assert!(system_service_target_from_cgroup("0::/system.slice/ssh.service;rm").is_none());
        assert!(system_service_target_from_cgroup("0::/system.slice/session-1.scope").is_none());
    }

    #[test]
    fn service_action_commands_have_fixed_argv() {
        let system = LogoscoreServiceTarget {
            scope: LogoscoreServiceScope::System,
            unit: "logos-node.service".to_owned(),
        };
        let (command, display) = service_action_command(&system, LogoscoreServiceAction::Stop);
        assert_eq!(command.get_program(), "sudo");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["-n", "--", "systemctl", "stop", "logos-node.service"]
        );
        assert_eq!(display, "sudo -n -- systemctl stop logos-node.service");

        let user = LogoscoreServiceTarget {
            scope: LogoscoreServiceScope::User,
            unit: "logos-node.service".to_owned(),
        };
        let (command, display) = service_action_command(&user, LogoscoreServiceAction::Start);
        assert_eq!(command.get_program(), "systemctl");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            ["--user", "start", "logos-node.service"]
        );
        assert_eq!(display, "systemctl --user start logos-node.service");
    }

    #[test]
    fn service_stop_status_command_uses_verified_service_scope() {
        let system = LogoscoreServiceTarget {
            scope: LogoscoreServiceScope::System,
            unit: "logos-node.service".to_owned(),
        };
        let command = service_stop_status_command(&system);
        assert_eq!(command.get_program(), "systemctl");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "show",
                "--property=ActiveState",
                "--property=SubState",
                "--property=Result",
                "--property=ExecMainCode",
                "--property=ExecMainStatus",
                "logos-node.service",
            ]
        );

        let user = LogoscoreServiceTarget {
            scope: LogoscoreServiceScope::User,
            unit: "logos-node.service".to_owned(),
        };
        let command = service_stop_status_command(&user);
        assert_eq!(command.get_program(), "systemctl");
        assert_eq!(
            command
                .get_args()
                .map(|argument| argument.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            [
                "--user",
                "show",
                "--property=ActiveState",
                "--property=SubState",
                "--property=Result",
                "--property=ExecMainCode",
                "--property=ExecMainStatus",
                "logos-node.service",
            ]
        );
    }

    #[test]
    fn service_stop_status_requires_a_clean_terminal_systemd_result() -> Result<()> {
        let clean = parse_system_service_stop_status(
            b"ActiveState=inactive\nSubState=dead\nResult=success\nExecMainCode=exited\nExecMainStatus=0\n",
        )?;
        anyhow::ensure!(clean.outcome() == LogoscoreServiceStopOutcome::Stopped);

        let failed = parse_system_service_stop_status(
            b"ActiveState=failed\nSubState=failed\nResult=signal\nExecMainCode=killed\nExecMainStatus=7\n",
        )?;
        let LogoscoreServiceStopOutcome::StoppedWithFailure(detail) = failed.outcome() else {
            bail!("failed systemd result was not reported as a terminal failure");
        };
        anyhow::ensure!(
            detail
                == "ActiveState=failed, SubState=failed, Result=signal, ExecMainCode=killed, ExecMainStatus=7"
        );

        let still_running = parse_system_service_stop_status(
            b"ActiveState=active\nSubState=running\nResult=success\nExecMainCode=exited\nExecMainStatus=0\n",
        )?;
        anyhow::ensure!(matches!(
            still_running.outcome(),
            LogoscoreServiceStopOutcome::StillRunning(_)
        ));
        Ok(())
    }

    #[test]
    fn service_stop_status_rejects_missing_or_duplicate_properties() {
        let missing = parse_system_service_stop_status(
            b"ActiveState=inactive\nSubState=dead\nResult=success\nExecMainCode=exited\n",
        );
        assert!(missing.is_err());

        let duplicate = parse_system_service_stop_status(
            b"ActiveState=inactive\nActiveState=failed\nSubState=dead\nResult=success\nExecMainCode=exited\nExecMainStatus=0\n",
        );
        assert!(duplicate.is_err());
    }

    #[test]
    fn attached_runtime_status_uses_cli_proven_process_and_service_target() {
        let service_target = LogoscoreServiceTarget {
            scope: LogoscoreServiceScope::System,
            unit: "logos-node.service".to_owned(),
        };
        let running = attached_profile(Some(42), Some(service_target.clone()));
        let report = status(Some(&running));
        assert_eq!(report.ownership, "local_attached");
        assert_eq!(report.run_state, "running");
        assert_eq!(report.process_id, Some(42));
        assert_eq!(report.service_unit.as_deref(), Some("logos-node.service"));
        assert!(running.is_controllable());

        let stopped = attached_profile(None, Some(service_target));
        let report = status(Some(&stopped));
        assert_eq!(report.run_state, "stopped");
        assert!(report.detail.contains("can start it"));
        assert!(stopped.is_controllable());

        let read_only = attached_profile(Some(42), None);
        assert!(read_only.is_running());
        assert!(!read_only.is_controllable());
    }

    #[test]
    fn unresolved_attached_runtime_remains_unavailable_and_retains_its_service_target() {
        let service_target = LogoscoreServiceTarget {
            scope: LogoscoreServiceScope::System,
            unit: "logos-node.service".to_owned(),
        };
        let persisted = attached_profile(Some(42), Some(service_target.clone()));
        let unresolved = resolve_local_runtime_observation(None, Some(persisted))
            .expect("attached runtime observation");

        assert!(unresolved.is_unavailable());
        assert!(!unresolved.is_running());
        assert_eq!(unresolved.daemon_process_id, None);
        assert_eq!(unresolved.service_target, Some(service_target.clone()));
        let report = status(Some(&unresolved));
        assert_eq!(report.run_state, "unavailable");
        assert!(report.detail.contains("package changes stay disabled"));

        let mut discovered = attached_profile(None, None);
        discovered.observation = LogoscoreRuntimeObservation::Unavailable;
        let retained = resolve_local_runtime_observation(
            Some(discovered),
            Some(attached_profile(None, Some(service_target.clone()))),
        )
        .expect("unavailable attached runtime observation");
        assert!(retained.is_unavailable());
        assert_eq!(retained.service_target, Some(service_target));
    }

    #[test]
    fn local_status_probe_failure_creates_an_unavailable_attachment() {
        let mut profile = LogoscoreRuntimeProfile::local_attached_profile(
            "/bin/sh".to_owned(),
            "/tmp/logoscore-client".to_owned(),
        );
        profile.apply_local_status_probe(Err(anyhow::anyhow!("status probe timed out")));
        assert!(profile.is_unavailable());
        assert_eq!(profile.daemon_process_id, None);

        profile.apply_local_status_probe(Ok(LogosCoreOutput {
            runner: "fixture".to_owned(),
            value: json!({"daemon": {"status": "stopped"}}),
            stderr: None,
        }));
        assert!(!profile.is_unavailable());
        assert_eq!(profile.daemon_process_id, None);
    }

    #[cfg(unix)]
    #[test]
    fn proven_local_discovery_reuses_a_fresh_status_snapshot() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let binary = directory.path().join("logoscore");
        fs::write(
            &binary,
            r#"#!/bin/sh
set -eu
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" = "status" ] && [ "$2" = "--json" ]; then
    printf '%s\n' status >> "$config_dir/commands"
    printf '%s\n' '{"daemon":{"status":"stopped"}}'
    exit 0
fi
exit 9
"#,
        )?;
        let mut permissions = fs::metadata(&binary)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&binary, permissions)?;

        let binary_path = binary.display().to_string();
        let config_dir = directory.path().display().to_string();
        LogoscoreCliRuntime::local(binary_path.clone(), config_dir.clone())
            .status_with_timeout(Duration::from_secs(1))?;
        let profile =
            LogoscoreRuntimeProfile::discover_proven_local_transport(binary_path, config_dir)?;

        anyhow::ensure!(
            !profile.is_unavailable(),
            "proven local transport was incorrectly reported as unavailable"
        );
        anyhow::ensure!(
            !profile.is_running(),
            "stopped local daemon was incorrectly reported as running"
        );
        anyhow::ensure!(
            fs::read_to_string(directory.path().join("commands"))?
                .lines()
                .eq(["status"]),
            "display discovery launched another status command instead of reusing a fresh snapshot"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn attached_module_install_requires_a_fresh_liveness_probe() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let binary = directory.path().join("logoscore");
        let modules_dir = directory.path().join("modules");
        fs::create_dir_all(&modules_dir)?;
        fs::write(&binary, "#!/bin/sh\nexit 9\n")?;
        let mut permissions = fs::metadata(&binary)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&binary, permissions.clone())?;

        let mut profile = attached_profile(None, None);
        profile.binary_path = binary.display().to_string();
        profile.config_dir = directory.path().display().to_string();
        profile.observation = LogoscoreRuntimeObservation::Unavailable;
        let error = profile
            .validate_attached_module_install_target(&modules_dir)
            .err()
            .context("unavailable attached runtime authorized a package mutation")?;
        anyhow::ensure!(
            format!("{error:#}").contains("cannot verify the local LogosCore daemon"),
            "unavailable attached runtime returned the wrong error: {error:#}"
        );

        fs::write(
            &binary,
            "#!/bin/sh\nprintf '%s\\n' '{\"daemon\":{\"status\":\"stopped\"}}'\n",
        )?;
        fs::set_permissions(&binary, permissions)?;
        profile.validate_attached_module_install_target(&modules_dir)?;
        Ok(())
    }

    #[test]
    fn active_attached_modules_directory_requires_a_stop_and_allows_a_distinct_target() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let active_modules_dir = directory.path().join("active-modules");
        let other_modules_dir = directory.path().join("other-modules");
        fs::create_dir_all(&active_modules_dir)?;
        fs::create_dir_all(&other_modules_dir)?;

        let error = validate_active_attached_module_install_target(None, &other_modules_dir).err();
        anyhow::ensure!(
            error.is_some_and(|error| error.to_string().contains("cannot verify")),
            "unverified running attached runtime authorized a package mutation"
        );

        let error = validate_active_attached_module_install_target(
            Some(&active_modules_dir),
            &active_modules_dir,
        )
        .err();
        anyhow::ensure!(
            error.is_some_and(|error| error
                .to_string()
                .contains("stop the local LogosCore runtime")),
            "active attached modules directory did not require a stop"
        );

        validate_active_attached_module_install_target(
            Some(&active_modules_dir),
            &other_modules_dir,
        )?;
        Ok(())
    }

    #[test]
    fn daemon_modules_directory_parser_requires_one_absolute_existing_option() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules_dir = directory.path().join("modules");
        fs::create_dir_all(&modules_dir)?;
        let expected = fs::canonicalize(&modules_dir)?;
        let modules_dir = modules_dir.display().to_string();
        let long_form = format!("--modules-dir={modules_dir}");

        for arguments in [
            vec!["logoscore", "-m", modules_dir.as_str(), "-D"],
            vec!["logoscore", "--modules-dir", modules_dir.as_str(), "daemon"],
            vec!["logoscore", long_form.as_str(), "daemon"],
        ] {
            anyhow::ensure!(
                modules_dir_from_daemon_arguments(arguments) == Some(expected.clone()),
                "valid daemon modules directory argument was not resolved"
            );
        }

        for arguments in [
            vec!["logoscore", "-m"],
            vec!["logoscore", "-m", "modules"],
            vec!["logoscore", "--modules-dir=/missing/modules"],
            vec![
                "logoscore",
                "-m",
                modules_dir.as_str(),
                "-m",
                modules_dir.as_str(),
            ],
        ] {
            anyhow::ensure!(
                modules_dir_from_daemon_arguments(arguments).is_none(),
                "invalid daemon modules directory argument was accepted"
            );
        }
        Ok(())
    }

    #[test]
    fn attached_runtime_cannot_be_replaced_by_a_new_managed_runtime() {
        let profile = attached_profile(
            None,
            Some(LogoscoreServiceTarget {
                scope: LogoscoreServiceScope::System,
                unit: "logos-node.service".to_owned(),
            }),
        );
        let error = LogoscoreRuntimeProfile::create_or_restart(
            Path::new("/tmp/logos-inspector-runtime-test"),
            Some(&profile),
            Some("/bin/sh"),
            Some("/tmp"),
        )
        .expect_err("attached local service must not be replaced");
        assert!(
            error
                .to_string()
                .contains("use its verified service controls instead")
        );
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
