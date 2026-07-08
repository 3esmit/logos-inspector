use std::process::Command;

use anyhow::Result;
use serde_json::{Value, json};

use super::{NodeAction, NodeKind};
use crate::{modules::logos_core, support::command_runner::spawn_detached};

const BLOCKCHAIN_MODULE: &str = "blockchain_module";
const INDEXER_MODULE: &str = "lez_indexer_module";
const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LocalNodeCommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub display: String,
    backend: CommandBackend,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum CommandBackend {
    LogosCore {
        module: &'static str,
        method: &'static str,
        call_args: Vec<String>,
    },
    SpawnProcess,
}

#[must_use]
pub(super) fn command_spec_for(
    kind: NodeKind,
    action: NodeAction,
    config_path: &str,
    deployment: &str,
) -> Option<LocalNodeCommandSpec> {
    let config = config_path.to_owned();
    let deployment = deployment.to_owned();
    match (kind, action) {
        (NodeKind::Bedrock, NodeAction::Start) => Some(logoscore_spec(
            BLOCKCHAIN_MODULE,
            "start",
            vec![config, deployment],
        )),
        (NodeKind::Bedrock, NodeAction::Stop) => {
            Some(logoscore_spec(BLOCKCHAIN_MODULE, "stop", vec![config]))
        }
        (NodeKind::Indexer, NodeAction::Start) => Some(logoscore_spec(
            INDEXER_MODULE,
            "start_indexer",
            vec![config],
        )),
        (NodeKind::Indexer, NodeAction::Stop) => {
            Some(logoscore_spec(INDEXER_MODULE, "stop_indexer", vec![config]))
        }
        (NodeKind::Indexer, NodeAction::Purge | NodeAction::ResetNetwork) => Some(logoscore_spec(
            INDEXER_MODULE,
            "reset_storage",
            vec![config],
        )),
        (NodeKind::Storage, NodeAction::Install) => {
            Some(logoscore_spec(STORAGE_MODULE, "init", vec![config]))
        }
        (NodeKind::Storage, NodeAction::Start) => {
            Some(logoscore_spec(STORAGE_MODULE, "start", vec![config]))
        }
        (NodeKind::Storage, NodeAction::Stop) => {
            Some(logoscore_spec(STORAGE_MODULE, "stop", vec![config]))
        }
        (NodeKind::Storage, NodeAction::Uninstall | NodeAction::DeleteNetwork) => {
            Some(logoscore_spec(STORAGE_MODULE, "destroy", vec![config]))
        }
        (NodeKind::Messaging, NodeAction::Install) => {
            Some(logoscore_spec(DELIVERY_MODULE, "createNode", vec![config]))
        }
        (NodeKind::Messaging, NodeAction::Start) => {
            Some(logoscore_spec(DELIVERY_MODULE, "start", vec![config]))
        }
        (NodeKind::Messaging, NodeAction::Stop) => {
            Some(logoscore_spec(DELIVERY_MODULE, "stop", vec![config]))
        }
        (NodeKind::Sequencer, NodeAction::Start) => {
            Some(spawn_spec("sequencer_service", vec![config]))
        }
        _ => None,
    }
}

pub(super) fn execute_command_spec(spec: &LocalNodeCommandSpec) -> Result<Value> {
    match &spec.backend {
        CommandBackend::LogosCore {
            module,
            method,
            call_args,
        } => {
            let output = logos_core::call(module, method, call_args)?;
            Ok(json!({
                "runner": output.runner,
                "value": output.value,
                "stderr": output.stderr,
            }))
        }
        CommandBackend::SpawnProcess => {
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
    module: &'static str,
    method: &'static str,
    call_args: Vec<String>,
) -> LocalNodeCommandSpec {
    let mut args = vec!["call".to_owned(), module.to_owned(), method.to_owned()];
    args.extend(call_args.iter().cloned());
    args.push("--json".to_owned());
    LocalNodeCommandSpec {
        program: "logoscore".to_owned(),
        display: shell_display("logoscore", &args),
        args,
        backend: CommandBackend::LogosCore {
            module,
            method,
            call_args,
        },
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
