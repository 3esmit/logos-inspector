use std::{
    env, fs,
    future::Future,
    path::{Path, PathBuf},
    pin::Pin,
    process::Command,
    sync::Arc,
    time::Duration,
};

use anyhow::{Context as _, Result, bail};
use serde::{Serialize, Serializer};
use serde_json::Value;
use tempfile::TempDir;

use crate::support::command_runner::{CommandRunPolicy, output_text, run_command};

const LOGOSCORE_POLL_INTERVAL: Duration = Duration::from_millis(25);
const LOGOSCORE_OUTPUT_LIMIT: usize = 4096;
const LOGOSCORE_JSON_OUTPUT_LIMIT: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize)]
pub struct LogosCoreOutput {
    pub runner: String,
    pub value: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogoscoreModuleMethod {
    name: String,
    signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogoscoreModuleDiscovery {
    module: String,
    methods: Vec<LogoscoreModuleMethod>,
}

impl LogoscoreModuleDiscovery {
    pub(crate) fn require_method(&self, method: &str, signature: &str) -> Result<()> {
        let Some(found) = self
            .methods
            .iter()
            .find(|candidate| candidate.name == method)
        else {
            bail!(
                "logoscore module `{}` does not expose invokable method `{method}`",
                self.module
            );
        };
        if found.signature != signature {
            bail!(
                "logoscore module `{}` method `{method}` signature mismatch: expected `{signature}`, found `{}`",
                self.module,
                found.signature
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleTransportKind {
    Module,
    LogoscoreCli,
}

impl ModuleTransportKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Module => "module",
            Self::LogoscoreCli => "logoscore_cli",
        }
    }
}

impl Serialize for ModuleTransportKind {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCall {
    transport: ModuleTransportKind,
    module: String,
    method: String,
    args: Vec<Value>,
}

impl ModuleCall {
    pub fn new(
        transport: ModuleTransportKind,
        module: impl Into<String>,
        method: impl Into<String>,
        args: Vec<Value>,
    ) -> Result<Self> {
        let module = module.into();
        let method = method.into();
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        if method.trim().is_empty() {
            bail!("method name is required");
        }
        Ok(Self {
            transport,
            module,
            method,
            args,
        })
    }

    #[must_use]
    pub const fn transport(&self) -> ModuleTransportKind {
        self.transport
    }

    #[must_use]
    pub fn module(&self) -> &str {
        &self.module
    }

    #[must_use]
    pub fn method(&self) -> &str {
        &self.method
    }

    #[must_use]
    pub fn args(&self) -> &[Value] {
        &self.args
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleCallReply {
    transport: ModuleTransportKind,
    value: Value,
}

impl ModuleCallReply {
    #[must_use]
    pub const fn new(transport: ModuleTransportKind, value: Value) -> Self {
        Self { transport, value }
    }

    #[must_use]
    pub const fn transport(&self) -> ModuleTransportKind {
        self.transport
    }

    #[must_use]
    pub fn into_value(self) -> Value {
        self.value
    }
}

pub type ModuleCallFuture<'a> = Pin<Box<dyn Future<Output = Result<ModuleCallReply>> + Send + 'a>>;
pub type ModuleDiagnosticFuture<'a> = Pin<Box<dyn Future<Output = Result<Value>> + Send + 'a>>;
pub type SharedModuleTransport = Arc<dyn ModuleTransport>;

pub trait ModuleTransport: Send + Sync {
    fn kind(&self) -> ModuleTransportKind;

    fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_>;

    fn status(&self) -> ModuleDiagnosticFuture<'_> {
        let adapter = self.kind();
        Box::pin(async move {
            Ok(unsupported_diagnostic(
                adapter,
                "transport status is unavailable through this adapter",
            ))
        })
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        let adapter = self.kind();
        Box::pin(async move {
            Ok(unsupported_diagnostic(
                adapter,
                format!("module metadata for `{module}` is unavailable through this adapter"),
            ))
        })
    }
}

fn unsupported_diagnostic(adapter: ModuleTransportKind, reason: impl Into<String>) -> Value {
    serde_json::json!({
        "supported": false,
        "adapter": adapter,
        "reason": reason.into(),
    })
}

#[derive(Debug, Clone)]
pub struct UnavailableModuleTransport {
    reason: String,
}

impl UnavailableModuleTransport {
    #[must_use]
    pub fn basecamp_protocol_gate() -> Self {
        Self {
            reason: "Basecamp host module transport is unavailable: the pinned protocol does not provide safe async error and close semantics".to_owned(),
        }
    }
}

impl ModuleTransport for UnavailableModuleTransport {
    fn kind(&self) -> ModuleTransportKind {
        ModuleTransportKind::Module
    }

    fn call(&self, _call: ModuleCall) -> ModuleCallFuture<'_> {
        Box::pin(async move { bail!(self.reason.clone()) })
    }
}

pub async fn dispatch_module_call(
    transport: &dyn ModuleTransport,
    call: ModuleCall,
) -> Result<ModuleCallReply> {
    let expected = call.transport();
    let actual = transport.kind();
    if expected != actual {
        bail!(
            "resolved module transport `{}` is unavailable; active transport is `{}`",
            expected.as_str(),
            actual.as_str()
        );
    }
    let reply = transport.call(call).await?;
    if reply.transport() != actual {
        bail!(
            "module transport `{}` returned reply identity `{}`",
            actual.as_str(),
            reply.transport().as_str()
        );
    }
    Ok(reply)
}

#[derive(Debug, Clone)]
pub struct LogoscoreCliTransport {
    runtime: LogoscoreCliRuntime,
}

impl Default for LogoscoreCliTransport {
    fn default() -> Self {
        Self {
            runtime: configured_runtime(),
        }
    }
}

impl ModuleTransport for LogoscoreCliTransport {
    fn kind(&self) -> ModuleTransportKind {
        ModuleTransportKind::LogoscoreCli
    }

    fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let transport = call.transport();
            if transport != ModuleTransportKind::LogoscoreCli {
                bail!(
                    "LogosCore CLI transport cannot execute `{}` calls",
                    transport.as_str()
                );
            }
            let module = call.module().to_owned();
            let method = call.method().to_owned();
            let args = call
                .args()
                .iter()
                .cloned()
                .map(|value| match value {
                    Value::String(value) => value,
                    value => value.to_string(),
                })
                .collect::<Vec<_>>();
            let module_label = module.clone();
            let method_label = method.clone();
            let output = tokio::task::spawn_blocking(move || runtime.call(&module, &method, &args))
                .await
                .context("LogosCore CLI module-call worker failed")??;
            let value = normalize_module_call_value(&module_label, &method_label, output.value)?;
            Ok(ModuleCallReply::new(
                ModuleTransportKind::LogoscoreCli,
                value,
            ))
        })
    }

    fn status(&self) -> ModuleDiagnosticFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let output = tokio::task::spawn_blocking(move || runtime.status())
                .await
                .context("LogosCore CLI status worker failed")??;
            serde_json::to_value(output).context("failed to serialize LogosCore CLI status")
        })
    }

    fn module_info(&self, module: String) -> ModuleDiagnosticFuture<'_> {
        let runtime = self.runtime.clone();
        Box::pin(async move {
            let module_label = module.clone();
            let output = tokio::task::spawn_blocking(move || runtime.module_info(&module))
                .await
                .with_context(|| {
                    format!("LogosCore CLI module-info worker failed for `{module_label}`")
                })??;
            serde_json::to_value(output)
                .with_context(|| format!("failed to serialize module info for `{module_label}`"))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogoscoreCliRuntime {
    runner: LogosCoreRunner,
}

pub(crate) struct LogoscoreSharedFile {
    _directory: TempDir,
    path: PathBuf,
}

impl LogoscoreSharedFile {
    pub(crate) fn path(&self) -> &Path {
        &self.path
    }
}

impl LogoscoreCliRuntime {
    #[must_use]
    pub(crate) fn managed(binary_path: String, config_dir: String) -> Self {
        Self {
            runner: LogosCoreRunner {
                program: binary_path,
                sudo_user: None,
                home: None,
                config_dir: Some(config_dir),
                label: "Inspector-managed logoscore".to_owned(),
            },
        }
    }

    pub(crate) fn status(&self) -> Result<LogosCoreOutput> {
        self.run_json(["status", "--json"], command_timeout())
    }

    pub(crate) fn status_with_timeout(&self, timeout: Duration) -> Result<LogosCoreOutput> {
        self.run_json(["status", "--json"], timeout)
    }

    pub(crate) fn list_modules(&self) -> Result<LogosCoreOutput> {
        self.run_json(["list-modules", "--json"], command_timeout())
    }

    pub(crate) fn module_info(&self, module: &str) -> Result<LogosCoreOutput> {
        if module.trim().is_empty() {
            bail!("module name is required");
        }
        self.run_json(["module-info", module, "--json"], command_timeout())
    }

    pub(crate) fn require_module_method(
        &self,
        module: &str,
        method: &str,
        signature: &str,
    ) -> Result<()> {
        let modules = self
            .list_modules()
            .context("failed to list logoscore modules")?;
        let module_info = self
            .module_info(module)
            .with_context(|| format!("failed to inspect logoscore module `{module}`"))?;
        module_discovery(module, &modules.value, &module_info.value)?
            .require_method(method, signature)
    }

    pub(crate) fn ensure_module_loaded(&self, module: &str) -> Result<()> {
        let modules = self
            .list_modules()
            .context("failed to list logoscore modules")?;
        let rows = module_rows(&modules.value)?;
        let Some(row) = rows
            .iter()
            .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some(module))
        else {
            bail!("logoscore module `{module}` is not listed");
        };
        if row.get("status").and_then(Value::as_str) == Some("loaded") {
            return Ok(());
        }

        self.run_json(["load-module", module, "--json"], command_timeout())
            .with_context(|| format!("failed to load logoscore module `{module}`"))?;
        Ok(())
    }

    pub(crate) fn call(
        &self,
        module: &str,
        method: &str,
        args: &[String],
    ) -> Result<LogosCoreOutput> {
        let command_args = call_arguments(module, method, args)?;
        let mut output = self.run_json(command_args, command_timeout())?;
        normalize_call_value(&mut output.value);
        Ok(output)
    }

    pub(crate) fn call_checked(
        &self,
        module: &str,
        method: &str,
        signature: &str,
        args: &[String],
    ) -> Result<Value> {
        self.require_module_method(module, method, signature)?;
        serde_json::to_value(self.call(module, method, args)?)
            .context("failed to serialize logoscore call output")
    }

    #[must_use]
    pub(crate) fn daemon_command(&self, persistence_path: &str, modules_dir: &str) -> Command {
        command_for_runner(
            &self.runner,
            [
                "--persistence-path",
                persistence_path,
                "daemon",
                "--modules-dir",
                modules_dir,
            ],
        )
    }

    #[must_use]
    pub(crate) fn watch_command(&self, module: &str, event: &str) -> Command {
        command_for_runner(&self.runner, ["watch", module, "--event", event, "--json"])
    }

    pub(crate) fn stop(&self) -> Result<LogosCoreOutput> {
        self.run_json(["stop", "--json"], command_timeout())
    }

    pub(crate) fn stage_shared_file(
        &self,
        filename: &str,
        bytes: &[u8],
    ) -> Result<LogoscoreSharedFile> {
        let directory = tempfile::Builder::new()
            .prefix("logos-inspector-upload-")
            .tempdir()
            .context("failed to create logoscore upload workspace")?;
        #[cfg(unix)]
        share_with_local_daemon(&self.runner, directory.path())?;
        let path = directory.path().join(filename);
        fs::write(&path, bytes).context("failed to write logoscore upload payload")?;
        #[cfg(unix)]
        share_file_with_local_daemon(&self.runner, &path)?;
        Ok(LogoscoreSharedFile {
            _directory: directory,
            path,
        })
    }

    fn run_json<I, S>(&self, args: I, timeout: Duration) -> Result<LogosCoreOutput>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        run_json_with(&self.runner, args, timeout)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LogosCoreRunner {
    program: String,
    sudo_user: Option<String>,
    home: Option<String>,
    config_dir: Option<String>,
    label: String,
}

pub fn status() -> Result<LogosCoreOutput> {
    configured_runtime().status()
}

pub fn module_info(module: &str) -> Result<LogosCoreOutput> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    configured_runtime().module_info(module)
}

pub(crate) fn require_module_method(module: &str, method: &str, signature: &str) -> Result<()> {
    configured_runtime().require_module_method(module, method, signature)
}

pub(crate) fn stage_shared_file(filename: &str, bytes: &[u8]) -> Result<LogoscoreSharedFile> {
    configured_runtime().stage_shared_file(filename, bytes)
}

#[cfg(unix)]
fn share_with_local_daemon(runner: &LogosCoreRunner, path: &Path) -> Result<()> {
    use std::os::unix::fs::{PermissionsExt as _, chown};

    let group = local_daemon_group(runner)?;
    chown(path, None, Some(group)).context("failed to assign logoscore upload directory group")?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o750))
        .context("failed to secure logoscore upload directory")
}

#[cfg(unix)]
fn share_file_with_local_daemon(runner: &LogosCoreRunner, path: &Path) -> Result<()> {
    use std::os::unix::fs::{PermissionsExt as _, chown};

    let group = local_daemon_group(runner)?;
    chown(path, None, Some(group)).context("failed to assign logoscore upload file group")?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o640))
        .context("failed to secure logoscore upload file")
}

#[cfg(unix)]
fn local_daemon_group(runner: &LogosCoreRunner) -> Result<u32> {
    use std::os::unix::fs::MetadataExt as _;

    let config_dir = runner_config_dir(runner)?;
    let config_path = config_dir.join("client").join("config.json");
    let config_bytes = fs::read(&config_path).with_context(|| {
        format!(
            "failed to read logoscore client config `{}`",
            config_path.display()
        )
    })?;
    let config: Value = serde_json::from_slice(&config_bytes)
        .context("logoscore client config contains invalid JSON")?;
    let instance_id = local_transport_instance_id(&config)?;
    let socket = env::temp_dir().join(format!("logos_core_service_{instance_id}"));
    fs::metadata(&socket)
        .with_context(|| {
            format!(
                "logoscore local transport socket is unavailable at `{}`",
                socket.display()
            )
        })
        .map(|metadata| metadata.gid())
}

#[cfg(unix)]
fn runner_config_dir(runner: &LogosCoreRunner) -> Result<PathBuf> {
    if let Some(config_dir) = runner.config_dir.as_deref() {
        return Ok(PathBuf::from(config_dir));
    }
    let home = runner
        .home
        .clone()
        .or_else(|| env::var("HOME").ok())
        .filter(|value| !value.trim().is_empty())
        .context("HOME is required to locate logoscore client config")?;
    Ok(PathBuf::from(home).join(".logoscore"))
}

#[cfg(unix)]
fn local_transport_instance_id(config: &Value) -> Result<&str> {
    let transport = config
        .pointer("/daemon/core_service/transport")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if transport != "local" {
        bail!(
            "storage_module uploadUrl requires local logoscore transport with a shared filesystem"
        );
    }
    config
        .get("instance_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .context("logoscore client config has no instance_id")
}

fn module_discovery(
    module: &str,
    modules_value: &Value,
    module_info_value: &Value,
) -> Result<LogoscoreModuleDiscovery> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    let modules = module_rows(modules_value)?;
    let Some(module_row) = modules
        .iter()
        .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some(module))
    else {
        bail!("logoscore module `{module}` is not listed");
    };
    let status = module_row
        .get("status")
        .and_then(Value::as_str)
        .context("logoscore module listing has no status")?;
    if status != "loaded" {
        bail!("logoscore module `{module}` is not loaded (status `{status}`)");
    }
    if module_info_value.get("name").and_then(Value::as_str) != Some(module) {
        bail!("logoscore module-info did not identify module `{module}`");
    }
    let methods = module_info_value
        .get("methods")
        .and_then(Value::as_array)
        .context("logoscore module-info response does not contain a methods array")?
        .iter()
        .filter(|method| method.get("isInvokable").and_then(Value::as_bool) == Some(true))
        .filter_map(|method| {
            Some(LogoscoreModuleMethod {
                name: method.get("name")?.as_str()?.to_owned(),
                signature: method.get("signature")?.as_str()?.to_owned(),
            })
        })
        .collect();
    Ok(LogoscoreModuleDiscovery {
        module: module.to_owned(),
        methods,
    })
}

fn module_rows(modules_value: &Value) -> Result<&Vec<Value>> {
    modules_value
        .as_array()
        .or_else(|| modules_value.get("modules").and_then(Value::as_array))
        .context("logoscore list-modules response does not contain a modules array")
}

pub fn call(module: &str, method: &str, args: &[String]) -> Result<LogosCoreOutput> {
    configured_runtime().call(module, method, args)
}

fn call_arguments(module: &str, method: &str, args: &[String]) -> Result<Vec<String>> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    if method.trim().is_empty() {
        bail!("method name is required");
    }

    let mut command_args = Vec::with_capacity(args.len() + 4);
    command_args.push("call".to_owned());
    command_args.push(module.to_owned());
    command_args.push(method.to_owned());
    command_args.extend(args.iter().cloned());
    command_args.push("--json".to_owned());
    Ok(command_args)
}

fn run_json_with<I, S>(
    runner: &LogosCoreRunner,
    args: I,
    timeout: Duration,
) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let command = command_for_runner(runner, args);
    let output = run_command(
        command,
        CommandRunPolicy {
            label: &runner.label,
            timeout,
            poll_interval: LOGOSCORE_POLL_INTERVAL,
            redactions: &[],
            output_limit: LOGOSCORE_OUTPUT_LIMIT,
        },
    )?;
    let stderr = output_text(&output.stderr, &[], LOGOSCORE_OUTPUT_LIMIT);
    let value = parse_json_stdout(&runner.label, &output.stdout)?;
    let stderr = (!stderr.is_empty()).then_some(stderr);
    Ok(LogosCoreOutput {
        runner: runner.label.clone(),
        value,
        stderr,
    })
}

fn parse_json_stdout(label: &str, stdout: &[u8]) -> Result<Value> {
    if stdout.len() > LOGOSCORE_JSON_OUTPUT_LIMIT {
        bail!(
            "{label} JSON output exceeded {} bytes",
            LOGOSCORE_JSON_OUTPUT_LIMIT
        );
    }
    let text = std::str::from_utf8(stdout).with_context(|| {
        format!(
            "{label} returned non-UTF-8 output: {}",
            output_text(stdout, &[], 400)
        )
    })?;
    serde_json::from_str(text.trim()).with_context(|| {
        format!(
            "{label} returned non-json output: {}",
            text.chars().take(400).collect::<String>()
        )
    })
}

fn command_timeout() -> Duration {
    env::var("LOGOSCORE_TIMEOUT_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .map(Duration::from_millis)
        .unwrap_or_else(|| Duration::from_secs(5))
}

fn command_for_runner<I, S>(runner: &LogosCoreRunner, args: I) -> Command
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    if let Some(user) = &runner.sudo_user {
        let mut command = Command::new("sudo");
        command.arg("-n").arg("-u").arg(user).arg("env");
        if let Some(home) = &runner.home {
            command.arg(format!("HOME={home}"));
        }
        command.arg(&runner.program);
        if let Some(config_dir) = &runner.config_dir {
            command.arg("--config-dir").arg(config_dir);
        }
        for arg in args {
            command.arg(arg.as_ref());
        }
        command
    } else {
        let mut command = Command::new(&runner.program);
        if let Some(home) = &runner.home {
            command.env("HOME", home);
        }
        if let Some(config_dir) = &runner.config_dir {
            command.arg("--config-dir").arg(config_dir);
        }
        for arg in args {
            command.arg(arg.as_ref());
        }
        command
    }
}

fn configured_runtime() -> LogoscoreCliRuntime {
    let env_program = env::var("LOGOSCORE_BIN")
        .ok()
        .filter(|value| !value.is_empty());
    let program = env_program
        .clone()
        .unwrap_or_else(|| "logoscore".to_owned());
    let env_user = env::var("LOGOSCORE_USER")
        .ok()
        .filter(|value| !value.is_empty());
    let env_home = env::var("LOGOSCORE_HOME")
        .ok()
        .filter(|value| !value.is_empty());
    let config_dir = env::var("LOGOSCORE_CONFIG_DIR")
        .ok()
        .filter(|value| !value.is_empty());
    let configured =
        env_program.is_some() || env_user.is_some() || env_home.is_some() || config_dir.is_some();

    LogoscoreCliRuntime {
        runner: LogosCoreRunner {
            program,
            sudo_user: env_user,
            home: env_home,
            config_dir,
            label: if configured {
                "configured logoscore".to_owned()
            } else {
                "plain logoscore".to_owned()
            },
        },
    }
}

fn normalize_call_value(value: &mut Value) {
    let Some(call_value) = value
        .get_mut("result")
        .and_then(|result| result.get_mut("value"))
    else {
        return;
    };
    let Some(raw) = call_value.as_str() else {
        return;
    };
    let trimmed = raw.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return;
    }
    let Ok(parsed) = serde_json::from_str::<Value>(trimmed) else {
        return;
    };
    *call_value = parsed;
}

pub(crate) fn normalize_module_call_value(
    module: &str,
    method: &str,
    value: Value,
) -> Result<Value> {
    let status = value
        .get("status")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !status.is_empty() && status != "ok" {
        bail!(
            "{module}.{method} returned status `{status}`: {}",
            crate::response_excerpt(&value.to_string())
        );
    }

    let Some(result) = value.get("result") else {
        return Ok(parse_module_json_string(value));
    };
    if let Some(object) = result.as_object()
        && let Some(success) = object.get("success").and_then(Value::as_bool)
    {
        if !success {
            let error = object
                .get("error")
                .map(module_value_error_text)
                .filter(|error| !error.is_empty())
                .unwrap_or_else(|| "module call failed".to_owned());
            bail!("{module}.{method} failed: {error}");
        }
        return Ok(object
            .get("value")
            .cloned()
            .map(parse_module_json_string)
            .unwrap_or(Value::Null));
    }
    Ok(parse_module_json_string(result.clone()))
}

fn parse_module_json_string(value: Value) -> Value {
    let Value::String(text) = value else {
        return value;
    };
    let trimmed = text.trim();
    if !(trimmed.starts_with('{') || trimmed.starts_with('[')) {
        return Value::String(text);
    }
    serde_json::from_str(trimmed).unwrap_or(Value::String(text))
}

fn module_value_error_text(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Null => String::new(),
        value => value.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Mutex,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;
    use serde_json::json;

    struct RecordingTransport {
        kind: ModuleTransportKind,
        reply_kind: ModuleTransportKind,
        calls: AtomicUsize,
        last_call: Mutex<Option<ModuleCall>>,
    }

    impl RecordingTransport {
        fn new(kind: ModuleTransportKind, reply_kind: ModuleTransportKind) -> Self {
            Self {
                kind,
                reply_kind,
                calls: AtomicUsize::new(0),
                last_call: Mutex::new(None),
            }
        }
    }

    impl ModuleTransport for RecordingTransport {
        fn kind(&self) -> ModuleTransportKind {
            self.kind
        }

        fn call(&self, call: ModuleCall) -> ModuleCallFuture<'_> {
            self.calls.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut last_call) = self.last_call.lock() {
                *last_call = Some(call.clone());
            }
            let reply_kind = self.reply_kind;
            Box::pin(async move {
                Ok(ModuleCallReply::new(
                    reply_kind,
                    json!({
                        "module": call.module(),
                        "method": call.method(),
                        "args": call.args(),
                    }),
                ))
            })
        }
    }

    #[tokio::test]
    async fn dispatch_preserves_json_arguments_and_transport_identity() -> Result<()> {
        let transport = RecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
        );
        let args = vec![json!({ "nested": [true, 7] }), json!("0")];
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "storage_module",
            "get",
            args.clone(),
        )?;

        let reply = dispatch_module_call(&transport, call).await?;

        anyhow::ensure!(reply.transport() == ModuleTransportKind::LogoscoreCli);
        anyhow::ensure!(reply.into_value().get("args") == Some(&json!(args)));
        let recorded = transport
            .last_call
            .lock()
            .map_err(|error| anyhow::anyhow!("recording transport lock failed: {error}"))?
            .clone()
            .context("recording transport did not receive call")?;
        anyhow::ensure!(recorded.transport() == ModuleTransportKind::LogoscoreCli);
        anyhow::ensure!(recorded.args() == args);
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_rejects_transport_mismatch_before_invocation() -> Result<()> {
        let transport = RecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::LogoscoreCli,
        );
        let call = ModuleCall::new(ModuleTransportKind::Module, "storage_module", "get", vec![])?;

        let Err(error) = dispatch_module_call(&transport, call).await else {
            bail!("transport mismatch unexpectedly succeeded");
        };

        anyhow::ensure!(
            error
                .to_string()
                .contains("resolved module transport `module` is unavailable")
        );
        anyhow::ensure!(transport.calls.load(Ordering::Relaxed) == 0);
        Ok(())
    }

    #[tokio::test]
    async fn dispatch_rejects_reply_identity_mismatch() -> Result<()> {
        let transport = RecordingTransport::new(
            ModuleTransportKind::LogoscoreCli,
            ModuleTransportKind::Module,
        );
        let call = ModuleCall::new(
            ModuleTransportKind::LogoscoreCli,
            "storage_module",
            "get",
            vec![],
        )?;

        let Err(error) = dispatch_module_call(&transport, call).await else {
            bail!("reply identity mismatch unexpectedly succeeded");
        };

        anyhow::ensure!(
            error
                .to_string()
                .contains("returned reply identity `module`")
        );
        anyhow::ensure!(transport.calls.load(Ordering::Relaxed) == 1);
        Ok(())
    }

    #[test]
    fn module_call_value_unwraps_logos_result_json_string() -> Result<()> {
        let value = normalize_module_call_value(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": {
                    "success": true,
                    "value": "{\"slot\":7}",
                    "error": null
                }
            }),
        )?;

        anyhow::ensure!(value.get("slot").and_then(Value::as_u64) == Some(7));
        Ok(())
    }

    #[test]
    fn module_call_value_unwraps_plain_json_string_result() -> Result<()> {
        let value = normalize_module_call_value(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": "[{\"id\":1}]"
            }),
        )?;

        anyhow::ensure!(value.as_array().map(Vec::len) == Some(1));
        Ok(())
    }

    #[test]
    fn module_call_value_reports_module_failure() {
        let result = normalize_module_call_value(
            "module",
            "method",
            json!({
                "status": "ok",
                "result": {
                    "success": false,
                    "value": null,
                    "error": "not started"
                }
            }),
        );

        assert!(result.is_err_and(|error| error.to_string().contains("not started")));
    }

    #[test]
    fn normalizes_nested_json_call_value() {
        let mut value = json!({
            "result": {
                "value": "{\"height\":1}"
            }
        });

        normalize_call_value(&mut value);

        let height = value
            .pointer("/result/value/height")
            .and_then(Value::as_u64);
        assert_eq!(height, Some(1));
    }

    #[test]
    fn keeps_non_json_call_value() {
        let mut value = json!({
            "result": {
                "value": "@[Version, Metrics]"
            }
        });

        normalize_call_value(&mut value);

        let value = value.pointer("/result/value").and_then(Value::as_str);
        assert_eq!(value, Some("@[Version, Metrics]"));
    }

    #[test]
    fn keeps_scalar_json_text_as_module_string() {
        let mut value = json!({
            "result": {
                "value": "0"
            }
        });

        normalize_call_value(&mut value);

        let value = value.pointer("/result/value").and_then(Value::as_str);
        assert_eq!(value, Some("0"));
    }

    #[test]
    fn parses_json_larger_than_error_excerpt_limit() -> Result<()> {
        let expected = json!({ "payload": "x".repeat(LOGOSCORE_OUTPUT_LIMIT * 3) });
        let encoded = serde_json::to_vec(&expected)?;

        let parsed = parse_json_stdout("logoscore test", &encoded)?;

        anyhow::ensure!(parsed == expected, "large logoscore JSON was truncated");
        Ok(())
    }

    #[test]
    fn cli_transport_builds_logoscore_call_arguments() -> Result<()> {
        let args = vec!["alpha".to_owned(), "42".to_owned()];

        let command_args = call_arguments("storage_module", "get", &args)?;

        if command_args != ["call", "storage_module", "get", "alpha", "42", "--json"] {
            bail!("unexpected logoscore call arguments: {command_args:?}");
        }
        Ok(())
    }

    #[test]
    fn configured_runtime_arguments_precede_call_arguments() {
        let runner = LogosCoreRunner {
            program: "logoscore".to_owned(),
            sudo_user: None,
            home: Some("/tmp/home".to_owned()),
            config_dir: Some("/tmp/logoscore".to_owned()),
            label: "configured logoscore".to_owned(),
        };
        let command = command_for_runner(&runner, ["call", "storage_module", "get", "--json"]);
        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();

        assert_eq!(
            args,
            [
                "--config-dir",
                "/tmp/logoscore",
                "call",
                "storage_module",
                "get",
                "--json"
            ]
        );
    }

    #[test]
    fn module_discovery_accepts_matching_loaded_method_contract() -> Result<()> {
        let modules = json!([{"name": "storage_module", "status": "loaded"}]);
        let info = json!({
            "name": "storage_module",
            "methods": [
                {"isInvokable": true, "name": "init", "signature": "init(QString)"},
                {"isInvokable": true, "name": "start", "signature": "start()"}
            ]
        });

        let discovery = module_discovery("storage_module", &modules, &info)?;

        discovery.require_method("init", "init(QString)")
    }

    #[test]
    fn module_discovery_rejects_missing_unloaded_and_mismatched_contracts() -> Result<()> {
        let missing = module_discovery("storage_module", &json!([]), &json!({}));
        let Err(error) = missing else {
            bail!("missing module discovery unexpectedly succeeded");
        };
        if !error.to_string().contains("is not listed") {
            bail!("unexpected missing module error: {error:#}");
        }

        let unloaded = module_discovery(
            "storage_module",
            &json!([{"name": "storage_module", "status": "not_loaded"}]),
            &json!({}),
        );
        let Err(error) = unloaded else {
            bail!("unloaded module discovery unexpectedly succeeded");
        };
        if !error.to_string().contains("is not loaded") {
            bail!("unexpected unloaded module error: {error:#}");
        }

        let methods = json!({
            "name": "storage_module",
            "methods": [
                {"isInvokable": true, "name": "start", "signature": "start(QString)"}
            ]
        });
        let discovery = module_discovery(
            "storage_module",
            &json!([{"name": "storage_module", "status": "loaded"}]),
            &methods,
        )?;
        let mismatch = discovery.require_method("start", "start()");
        let Err(error) = mismatch else {
            bail!("signature mismatch unexpectedly succeeded");
        };
        if !error.to_string().contains("signature mismatch") {
            bail!("unexpected signature mismatch error: {error:#}");
        }

        let absent = discovery.require_method("stop", "stop()");
        let Err(error) = absent else {
            bail!("missing method unexpectedly succeeded");
        };
        if !error
            .to_string()
            .contains("does not expose invokable method")
        {
            bail!("unexpected missing method error: {error:#}");
        }
        Ok(())
    }
}
