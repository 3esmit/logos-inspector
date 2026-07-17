use std::process::Command;

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use super::adapters::{
    ManagedCommandResultContract, NodeCommandContext, NodeCommandPlan, RpcStartupReadiness,
    adapter_for,
};
use super::{
    NodeAction, NodeKind,
    process::{spawn_detached, spawn_rpc_ready},
};
use crate::{
    modules::logos_core::{LogoscoreCliRuntime, normalize_module_call_value},
    source_routing::{ManagedModuleCallSpec, ManagedNodeContract},
    support::command_runner::CommandControl,
};

#[derive(Debug, Clone)]
pub(super) struct LocalNodeCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub display: String,
    backend: CommandBackend,
    result_contract: ManagedCommandResultContract,
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
    _data_dir: &str,
    _port: Option<u16>,
) -> Option<LocalNodeCommandSpec> {
    let context = NodeCommandContext { config_path };
    match adapter_for(kind).command_plan(action, context)? {
        NodeCommandPlan::ManagedModule {
            contract,
            call,
            result_contract,
        } => Some(logoscore_spec(contract, call, result_contract)),
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
            let value = contract.call(runtime, call)?;
            validate_managed_call_value(
                spec.result_contract,
                contract.module_id(),
                call.method,
                &value,
            )?;
            Ok(value)
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
                None => {
                    let value = contract.call(runtime, call)?;
                    validate_managed_call_value(
                        spec.result_contract,
                        contract.module_id(),
                        call.method,
                        &value,
                    )?;
                    return Ok(value);
                }
            }?;
            if spec.result_contract != ManagedCommandResultContract::Any {
                let value = normalize_module_call_value(
                    contract.module_id(),
                    call.method,
                    output.value.clone(),
                )?;
                validate_managed_result(
                    spec.result_contract,
                    contract.module_id(),
                    call.method,
                    &value,
                )?;
            }
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
    result_contract: ManagedCommandResultContract,
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
        result_contract,
    }
}

fn spawn_spec(program: &str, args: Vec<String>) -> LocalNodeCommandSpec {
    LocalNodeCommandSpec {
        program: program.to_owned(),
        display: shell_display(program, &args),
        args,
        backend: CommandBackend::SpawnProcess,
        result_contract: ManagedCommandResultContract::Any,
    }
}

fn validate_managed_result(
    result_contract: ManagedCommandResultContract,
    module: &str,
    method: &str,
    value: &Value,
) -> Result<()> {
    if result_contract == ManagedCommandResultContract::Any {
        return Ok(());
    }
    let status = value.as_i64().with_context(|| {
        format!(
            "{module}.{method} returned an invalid OperationStatus: {}",
            crate::response_excerpt(&value.to_string())
        )
    })?;
    anyhow::ensure!(
        status == 0,
        "{module}.{method} failed with OperationStatus {status}"
    );
    Ok(())
}

fn validate_managed_call_value(
    result_contract: ManagedCommandResultContract,
    module: &str,
    method: &str,
    value: &Value,
) -> Result<()> {
    if result_contract == ManagedCommandResultContract::Any {
        return Ok(());
    }
    let payload = value.get("value").cloned().unwrap_or_else(|| value.clone());
    let result = normalize_module_call_value(module, method, payload)?;
    validate_managed_result(result_contract, module, method, &result)
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

#[cfg(test)]
mod tests {
    use anyhow::{Result, bail};
    use serde_json::json;

    use super::*;

    #[test]
    fn indexer_lifecycle_requires_zero_operation_status() -> Result<()> {
        for action in [NodeAction::Start, NodeAction::Stop, NodeAction::Purge] {
            let spec = command_spec_for(
                NodeKind::Indexer,
                action,
                "/tmp/indexer.json",
                "/tmp/indexer",
                None,
            )
            .ok_or_else(|| anyhow::anyhow!("missing Indexer {action:?} command"))?;
            anyhow::ensure!(
                spec.result_contract == ManagedCommandResultContract::OperationStatusZero
            );
        }
        let result_contract = ManagedCommandResultContract::OperationStatusZero;
        let success = json!({
            "runner": "Inspector-managed logoscore",
            "value": {
                "status": "ok",
                "module": "lez_indexer_module",
                "method": "start_indexer",
                "result": 0,
            }
        });
        validate_managed_call_value(
            result_contract,
            "lez_indexer_module",
            "start_indexer",
            &success,
        )?;

        let failure = json!({
            "runner": "Inspector-managed logoscore",
            "value": {
                "status": "ok",
                "module": "lez_indexer_module",
                "method": "start_indexer",
                "result": 2,
            }
        });
        let Err(error) = validate_managed_call_value(
            result_contract,
            "lez_indexer_module",
            "start_indexer",
            &failure,
        ) else {
            bail!("nonzero Indexer OperationStatus was accepted");
        };
        anyhow::ensure!(
            error
                .to_string()
                .contains("start_indexer failed with OperationStatus 2")
        );

        let Err(error) = validate_managed_result(
            result_contract,
            "lez_indexer_module",
            "start_indexer",
            &json!("0"),
        ) else {
            bail!("string OperationStatus was accepted");
        };
        anyhow::ensure!(error.to_string().contains("invalid OperationStatus"));
        Ok(())
    }

    #[test]
    fn operation_status_contract_does_not_change_other_modules() -> Result<()> {
        let spec = command_spec_for(
            NodeKind::Storage,
            NodeAction::Start,
            "/tmp/storage.json",
            "/tmp/storage",
            None,
        )
        .ok_or_else(|| anyhow::anyhow!("missing Storage start command"))?;
        anyhow::ensure!(spec.result_contract == ManagedCommandResultContract::Any);
        validate_managed_result(
            spec.result_contract,
            "storage_module",
            "start",
            &json!({ "accepted": true }),
        )
    }
}
