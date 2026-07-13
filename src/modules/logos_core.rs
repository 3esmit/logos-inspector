use std::{env, process::Command, time::Duration};

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::Value;

use crate::support::command_runner::{CommandRunPolicy, output_text, run_command};

const LOGOSCORE_POLL_INTERVAL: Duration = Duration::from_millis(25);
const LOGOSCORE_OUTPUT_LIMIT: usize = 4096;

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

pub trait ModuleTransport {
    fn call(&self, module: &str, method: &str, args: &[String]) -> Result<Value>;
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

impl LogoscoreCliTransport {
    #[must_use]
    fn from_runtime(runtime: LogoscoreCliRuntime) -> Self {
        Self { runtime }
    }
}

impl ModuleTransport for LogoscoreCliTransport {
    fn call(&self, module: &str, method: &str, args: &[String]) -> Result<Value> {
        serde_json::to_value(self.runtime.call(module, method, args)?)
            .context("failed to serialize logoscore call output")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LogoscoreCliRuntime {
    runner: LogosCoreRunner,
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
        LogoscoreCliTransport::from_runtime(self.clone()).call(module, method, args)
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
    let stdout = output_text(&output.stdout, &[], LOGOSCORE_OUTPUT_LIMIT);
    let stderr = output_text(&output.stderr, &[], LOGOSCORE_OUTPUT_LIMIT);
    let value = serde_json::from_str(&stdout).with_context(|| {
        format!(
            "{} returned non-json output: {}",
            runner.label,
            stdout.chars().take(400).collect::<String>()
        )
    })?;
    let stderr = (!stderr.is_empty()).then_some(stderr);
    Ok(LogosCoreOutput {
        runner: runner.label.clone(),
        value,
        stderr,
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
    let Ok(parsed) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    *call_value = parsed;
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

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
