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
struct LogosCoreRunner {
    program: String,
    sudo_user: Option<String>,
    home: Option<String>,
    label: String,
}

pub fn status() -> Result<LogosCoreOutput> {
    run_json(["status", "--json"])
}

pub fn module_info(module: &str) -> Result<LogosCoreOutput> {
    if module.trim().is_empty() {
        bail!("module name is required");
    }
    run_json_prefer_service(["module-info", module, "--json"])
}

pub fn call(module: &str, method: &str, args: &[String]) -> Result<LogosCoreOutput> {
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

    let mut output = run_json_prefer_service(command_args)?;
    normalize_call_value(&mut output.value);
    Ok(output)
}

fn run_json<I, S>(args: I) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<str>,
{
    let mut errors = Vec::new();
    for runner in ordered_runners(false) {
        match run_json_with(&runner, args.clone()) {
            Ok(output) => return Ok(output),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    bail!("logoscore failed: {}", errors.join("; "))
}

fn run_json_prefer_service<I, S>(args: I) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<str>,
{
    let mut errors = Vec::new();
    for runner in ordered_runners(true) {
        match run_json_with(&runner, args.clone()) {
            Ok(output) => return Ok(output),
            Err(error) => errors.push(format!("{error:#}")),
        }
    }
    bail!("logoscore failed: {}", errors.join("; "))
}

fn run_json_with<I, S>(runner: &LogosCoreRunner, args: I) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let command = command_for_runner(runner, args);
    let timeout = command_timeout();
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

fn ordered_runners(prefer_service: bool) -> Vec<LogosCoreRunner> {
    let runners = runners();
    if !prefer_service {
        return runners;
    }

    let mut configured = Vec::new();
    let mut service = Vec::new();
    let mut plain = Vec::new();
    for runner in runners {
        if runner.label == "configured logoscore" {
            configured.push(runner);
        } else if runner.sudo_user.is_some() {
            service.push(runner);
        } else {
            plain.push(runner);
        }
    }
    configured.into_iter().chain(service).chain(plain).collect()
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
        for arg in args {
            command.arg(arg.as_ref());
        }
        command
    } else {
        let mut command = Command::new(&runner.program);
        if let Some(home) = &runner.home {
            command.env("HOME", home);
        }
        for arg in args {
            command.arg(arg.as_ref());
        }
        command
    }
}

fn runners() -> Vec<LogosCoreRunner> {
    let program = env::var("LOGOSCORE_BIN").unwrap_or_else(|_| "logoscore".to_owned());
    let env_user = env::var("LOGOSCORE_USER")
        .ok()
        .filter(|value| !value.is_empty());
    let env_home = env::var("LOGOSCORE_HOME")
        .ok()
        .filter(|value| !value.is_empty());
    let mut runners = Vec::new();

    if env_user.is_some() || env_home.is_some() || env::var("LOGOSCORE_BIN").is_ok() {
        runners.push(LogosCoreRunner {
            program: program.clone(),
            sudo_user: env_user.clone(),
            home: env_home.clone(),
            label: "configured logoscore".to_owned(),
        });
    }

    runners.push(LogosCoreRunner {
        program: program.clone(),
        sudo_user: None,
        home: None,
        label: "plain logoscore".to_owned(),
    });

    if env::var("LOGOSCORE_DISABLE_SUDO_FALLBACK").is_err() {
        runners.push(LogosCoreRunner {
            program,
            sudo_user: Some(env_user.unwrap_or_else(|| "logos".to_owned())),
            home: Some(env_home.unwrap_or_else(|| "/var/lib/logos-node".to_owned())),
            label: "service user logoscore".to_owned(),
        });
    }

    dedupe_runners(runners)
}

fn dedupe_runners(runners: Vec<LogosCoreRunner>) -> Vec<LogosCoreRunner> {
    let mut unique = Vec::new();
    for runner in runners {
        if !unique.iter().any(|existing| existing == &runner) {
            unique.push(runner);
        }
    }
    unique
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
}
