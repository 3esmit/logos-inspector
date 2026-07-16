use std::process::Command;

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use super::adapters::{NodeCommandContext, NodeCommandPlan, RpcStartupReadiness, adapter_for};
use super::{
    NodeAction, NodeKind,
    process::{spawn_detached, spawn_rpc_ready},
};
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
    data_dir: &str,
    port: Option<u16>,
) -> Option<LocalNodeCommandSpec> {
    let context = NodeCommandContext {
        config_path,
        data_dir,
        port,
    };
    match adapter_for(kind).command_plan(action, context)? {
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
    if let Some(control) = control {
        preflight_command_spec(spec, runtime, Some(control))?;
        return execute_preflighted_command_spec(spec, runtime, Some(control));
    }
    match &spec.backend {
        CommandBackend::LogosCore { contract, call } => {
            let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
            contract.call(runtime, call)
        }
        CommandBackend::SpawnProcess => execute_preflighted_command_spec(spec, runtime, None),
    }
}

pub(super) fn preflight_command_spec(
    spec: &LocalNodeCommandSpec,
    runtime: Option<&LogoscoreCliRuntime>,
    control: Option<&CommandControl>,
) -> Result<()> {
    match &spec.backend {
        CommandBackend::LogosCore { contract, call } => {
            let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
            match control {
                Some(control) => runtime.require_module_method_controlled(
                    contract.module_id(),
                    call.method,
                    call.signature,
                    control.clone(),
                ),
                None => {
                    runtime.require_module_method(contract.module_id(), call.method, call.signature)
                }
            }
        }
        CommandBackend::SpawnProcess => Ok(()),
    }
}

pub(super) fn preflight_command_spec_once(
    spec: &LocalNodeCommandSpec,
    runtime: Option<&LogoscoreCliRuntime>,
    control: &CommandControl,
) -> Result<()> {
    match &spec.backend {
        CommandBackend::LogosCore { contract, call } => {
            let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
            runtime.require_module_method_controlled_once(
                contract.module_id(),
                call.method,
                call.signature,
                control.clone(),
            )
        }
        CommandBackend::SpawnProcess => Ok(()),
    }
}

pub(super) fn execute_preflighted_command_spec(
    spec: &LocalNodeCommandSpec,
    runtime: Option<&LogoscoreCliRuntime>,
    control: Option<&CommandControl>,
) -> Result<Value> {
    match &spec.backend {
        CommandBackend::LogosCore { contract, call } => {
            let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
            let output = match control {
                Some(control) => runtime.call_controlled(
                    contract.module_id(),
                    call.method,
                    &call.args,
                    control.clone(),
                ),
                None => return contract.call(runtime, call),
            }?;
            serde_json::to_value(output).context("failed to serialize logoscore call output")
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

pub(super) fn execute_ready_process_spec(
    spec: &LocalNodeCommandSpec,
    endpoint: &str,
    readiness: RpcStartupReadiness,
    control: Option<&CommandControl>,
) -> Result<Value> {
    if !matches!(&spec.backend, CommandBackend::SpawnProcess) {
        anyhow::bail!("{} is not a registered process command", spec.display);
    }
    if let Some(control) = control {
        control.check_active()?;
    }
    let mut command = Command::new(&spec.program);
    for arg in &spec.args {
        command.arg(arg);
    }
    let pid = spawn_rpc_ready(command, &spec.display, endpoint, readiness, control)?;
    Ok(json!({
        "pid": pid,
        "command": spec.display,
        "readiness": "rpc",
    }))
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
