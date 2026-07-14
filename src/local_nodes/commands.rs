use std::process::Command;

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use super::adapters::{NodeCommandPlan, adapter_for};
use super::{NodeAction, NodeKind, process::spawn_detached};
use crate::{
    modules::logos_core::LogoscoreCliRuntime,
    source_routing::{ManagedModuleCallSpec, ManagedNodeContract},
    support::command_runner::CommandControl,
};

#[derive(Debug, Clone)]
pub(super) struct LocalNodeCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub display: String,
    backend: CommandBackend,
}

#[derive(Debug, Clone)]
enum CommandBackend {
    LogosCore {
        contract: &'static ManagedNodeContract,
        call: ManagedModuleCallSpec,
    },
    SpawnProcess,
}

#[must_use]
pub(super) fn command_spec_for(
    kind: NodeKind,
    action: NodeAction,
    config_path: &str,
    _deployment: &str,
) -> Option<LocalNodeCommandSpec> {
    match adapter_for(kind).command_plan(action, config_path)? {
        NodeCommandPlan::ManagedModule { contract, call } => Some(logoscore_spec(contract, call)),
        NodeCommandPlan::DetachedProcess { program, args } => Some(spawn_spec(program, args)),
    }
}

pub(super) fn ensure_module_loaded(
    spec: &LocalNodeCommandSpec,
    runtime: Option<&LogoscoreCliRuntime>,
    control: Option<&CommandControl>,
) -> Result<()> {
    let CommandBackend::LogosCore { contract, .. } = &spec.backend else {
        return Ok(());
    };
    let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
    match control {
        Some(control) => {
            runtime.ensure_module_loaded_controlled(contract.module_id(), control.clone())
        }
        None => contract.ensure_loaded(runtime),
    }
}

pub(super) fn execute_command_spec(
    spec: &LocalNodeCommandSpec,
    runtime: Option<&LogoscoreCliRuntime>,
    control: Option<&CommandControl>,
) -> Result<Value> {
    match &spec.backend {
        CommandBackend::LogosCore { contract, call } => {
            let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
            match control {
                Some(control) => runtime.call_checked_controlled(
                    contract.module_id(),
                    call.method,
                    call.signature,
                    &call.args,
                    control.clone(),
                ),
                None => contract.call(runtime, call),
            }
        }
        CommandBackend::SpawnProcess => {
            if let Some(control) = control {
                control.check_active()?;
            }
            let mut command = Command::new(&spec.program);
            for arg in &spec.args {
                command.arg(arg);
            }
            let pid = spawn_detached(command, &spec.display)?;
            Ok(json!({
                "pid": pid,
                "command": spec.display,
            }))
        }
    }
}

fn logoscore_spec(
    contract: &'static ManagedNodeContract,
    call: ManagedModuleCallSpec,
) -> LocalNodeCommandSpec {
    let module = contract.module_id();
    let mut args = vec!["call".to_owned(), module.to_owned(), call.method.to_owned()];
    args.extend(call.args.iter().cloned());
    args.push("--json".to_owned());
    LocalNodeCommandSpec {
        program: "logoscore".to_owned(),
        display: shell_display("logoscore", &args),
        args,
        backend: CommandBackend::LogosCore { contract, call },
    }
}

fn spawn_spec(program: &str, args: Vec<String>) -> LocalNodeCommandSpec {
    LocalNodeCommandSpec {
        program: program.to_owned(),
        display: shell_display(program, &args),
        args,
        backend: CommandBackend::SpawnProcess,
    }
}

fn shell_display(program: &str, args: &[String]) -> String {
    let mut parts = Vec::with_capacity(args.len() + 1);
    parts.push(program.to_owned());
    parts.extend(args.iter().cloned());
    parts.join(" ")
}

pub(super) fn operation_detail_from_value(value: &Value) -> String {
    value
        .get("value")
        .and_then(|value| value.get("status").or_else(|| value.get("result")))
        .map(Value::to_string)
        .unwrap_or_else(|| "completed".to_owned())
}
