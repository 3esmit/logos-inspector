use std::process::Command;

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use super::{NodeAction, NodeKind, process::spawn_detached};
use crate::{
    modules::logos_core::LogoscoreCliRuntime,
    source_routing::{bedrock_layer, execution_zone_layer, messaging_layer, storage_layer},
};

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
        layer: ManagedNodeLayer,
        method: &'static str,
        signature: &'static str,
        call_args: Vec<String>,
    },
    SpawnProcess,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ManagedNodeLayer {
    Bedrock,
    Storage,
    Messaging,
}

impl ManagedNodeLayer {
    const fn module_id(self) -> &'static str {
        match self {
            Self::Bedrock => bedrock_layer::module_id(),
            Self::Storage => storage_layer::module_id(),
            Self::Messaging => messaging_layer::module_id(),
        }
    }

    fn ensure_loaded(self, runtime: &LogoscoreCliRuntime) -> Result<()> {
        match self {
            Self::Bedrock => bedrock_layer::ensure_managed_module(runtime),
            Self::Storage => storage_layer::ensure_managed_module(runtime),
            Self::Messaging => messaging_layer::ensure_managed_module(runtime),
        }
    }

    fn call(
        self,
        runtime: &LogoscoreCliRuntime,
        method: &str,
        signature: &str,
        args: &[String],
    ) -> Result<Value> {
        match self {
            Self::Bedrock => bedrock_layer::call_managed_module(runtime, method, signature, args),
            Self::Storage => storage_layer::call_managed_module(runtime, method, signature, args),
            Self::Messaging => {
                messaging_layer::call_managed_module(runtime, method, signature, args)
            }
        }
    }
}

#[must_use]
pub(super) fn command_spec_for(
    kind: NodeKind,
    action: NodeAction,
    config_path: &str,
    _deployment: &str,
) -> Option<LocalNodeCommandSpec> {
    let config = config_path.to_owned();
    match (kind, action) {
        (NodeKind::Bedrock, NodeAction::Start) => Some(logoscore_spec(
            ManagedNodeLayer::Bedrock,
            "start",
            "start(QString,QString)",
            vec![config, String::new()],
        )),
        (NodeKind::Bedrock, NodeAction::Stop) => Some(logoscore_spec(
            ManagedNodeLayer::Bedrock,
            "stop",
            "stop()",
            Vec::new(),
        )),
        (NodeKind::Storage, NodeAction::Initialize) => Some(logoscore_spec(
            ManagedNodeLayer::Storage,
            "init",
            "init(QString)",
            vec![file_argument(&config)],
        )),
        (NodeKind::Storage, NodeAction::Start) => Some(logoscore_spec(
            ManagedNodeLayer::Storage,
            "start",
            "start()",
            Vec::new(),
        )),
        (NodeKind::Storage, NodeAction::Stop) => Some(logoscore_spec(
            ManagedNodeLayer::Storage,
            "stop",
            "stop()",
            Vec::new(),
        )),
        (NodeKind::Storage, NodeAction::Uninstall | NodeAction::DeleteNetwork) => {
            Some(logoscore_spec(
                ManagedNodeLayer::Storage,
                "destroy",
                "destroy()",
                Vec::new(),
            ))
        }
        (NodeKind::Messaging, NodeAction::Initialize) => Some(logoscore_spec(
            ManagedNodeLayer::Messaging,
            "createNode",
            "createNode(QString)",
            vec![file_argument(&config)],
        )),
        (NodeKind::Messaging, NodeAction::Start) => Some(logoscore_spec(
            ManagedNodeLayer::Messaging,
            "start",
            "start()",
            Vec::new(),
        )),
        (NodeKind::Messaging, NodeAction::Stop) => Some(logoscore_spec(
            ManagedNodeLayer::Messaging,
            "stop",
            "stop()",
            Vec::new(),
        )),
        (NodeKind::Sequencer, NodeAction::Start) => Some(spawn_spec(
            execution_zone_layer::managed_sequencer_program(),
            vec![config],
        )),
        _ => None,
    }
}

pub(super) fn ensure_module_loaded(
    spec: &LocalNodeCommandSpec,
    runtime: Option<&LogoscoreCliRuntime>,
) -> Result<()> {
    let CommandBackend::LogosCore { layer, .. } = &spec.backend else {
        return Ok(());
    };
    let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
    layer.ensure_loaded(runtime)
}

pub(super) fn execute_command_spec(
    spec: &LocalNodeCommandSpec,
    runtime: Option<&LogoscoreCliRuntime>,
) -> Result<Value> {
    match &spec.backend {
        CommandBackend::LogosCore {
            layer,
            method,
            signature,
            call_args,
        } => {
            let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
            layer.call(runtime, method, signature, call_args)
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

fn file_argument(path: &str) -> String {
    format!("@{path}")
}

fn logoscore_spec(
    layer: ManagedNodeLayer,
    method: &'static str,
    signature: &'static str,
    call_args: Vec<String>,
) -> LocalNodeCommandSpec {
    let module = layer.module_id();
    let mut args = vec!["call".to_owned(), module.to_owned(), method.to_owned()];
    args.extend(call_args.iter().cloned());
    args.push("--json".to_owned());
    LocalNodeCommandSpec {
        program: "logoscore".to_owned(),
        display: shell_display("logoscore", &args),
        args,
        backend: CommandBackend::LogosCore {
            layer,
            method,
            signature,
            call_args,
        },
    }
}

#[must_use]
pub(super) fn has_static_module_contract(kind: NodeKind) -> bool {
    kind != NodeKind::Indexer
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
