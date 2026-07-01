use std::{env, process::Command};

use anyhow::{Context as _, Result, bail};
use serde::Serialize;
use serde_json::Value;

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
    run_json(["module-info", module, "--json"])
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

    let mut output = run_json(command_args)?;
    normalize_call_value(&mut output.value);
    Ok(output)
}

fn run_json<I, S>(args: I) -> Result<LogosCoreOutput>
where
    I: IntoIterator<Item = S> + Clone,
    S: AsRef<str>,
{
    let mut errors = Vec::new();
    for runner in runners() {
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
    let output = command_for_runner(runner, args)
        .output()
        .with_context(|| format!("failed to run {}", runner.label))?;
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if !output.status.success() {
        let message = if stderr.is_empty() { &stdout } else { &stderr };
        bail!(
            "{} exited with {}: {}",
            runner.label,
            output.status,
            message
        );
    }
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
