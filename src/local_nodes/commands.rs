use std::process::Command;

use anyhow::{Context as _, Result};
use serde_json::{Value, json};

use super::{NodeAction, NodeKind, process::spawn_detached};
use crate::{
    modules::logos_core::LogoscoreCliRuntime,
    source_routing::{
        ManagedModuleCallSpec, ManagedNodeAction, bedrock_layer, execution_zone_layer,
        messaging_layer, storage_layer,
    },
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
        call: ManagedModuleCallSpec,
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

    fn call(self, runtime: &LogoscoreCliRuntime, spec: &ManagedModuleCallSpec) -> Result<Value> {
        match self {
            Self::Bedrock => {
                bedrock_layer::call_managed_module(runtime, spec.method, spec.signature, &spec.args)
            }
            Self::Storage => {
                storage_layer::call_managed_module(runtime, spec.method, spec.signature, &spec.args)
            }
            Self::Messaging => messaging_layer::call_managed_module(
                runtime,
                spec.method,
                spec.signature,
                &spec.args,
            ),
        }
    }

    fn call_spec(
        self,
        action: ManagedNodeAction,
        config_path: &str,
    ) -> Option<ManagedModuleCallSpec> {
        match self {
            Self::Bedrock => bedrock_layer::managed_call_spec(action, config_path),
            Self::Storage => storage_layer::managed_call_spec(action, config_path),
            Self::Messaging => messaging_layer::managed_call_spec(action, config_path),
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
    match (kind, action) {
        (NodeKind::Sequencer, NodeAction::Start) => Some(spawn_spec(
            execution_zone_layer::managed_sequencer_program(),
            vec![config_path.to_owned()],
        )),
        _ => {
            let layer = managed_layer(kind)?;
            let call = layer.call_spec(managed_action(action)?, config_path)?;
            Some(logoscore_spec(layer, call))
        }
    }
}

#[must_use]
pub(super) const fn managed_action(action: NodeAction) -> Option<ManagedNodeAction> {
    match action {
        NodeAction::Initialize => Some(ManagedNodeAction::Initialize),
        NodeAction::Start => Some(ManagedNodeAction::Start),
        NodeAction::Stop => Some(ManagedNodeAction::Stop),
        NodeAction::Uninstall | NodeAction::DeleteNetwork => Some(ManagedNodeAction::Destroy),
        NodeAction::StartRuntime
        | NodeAction::StopRuntime
        | NodeAction::NewNetwork
        | NodeAction::LoadNetwork
        | NodeAction::ResetNetwork
        | NodeAction::Install
        | NodeAction::Purge => None,
    }
}

const fn managed_layer(kind: NodeKind) -> Option<ManagedNodeLayer> {
    match kind {
        NodeKind::Bedrock => Some(ManagedNodeLayer::Bedrock),
        NodeKind::Storage => Some(ManagedNodeLayer::Storage),
        NodeKind::Messaging => Some(ManagedNodeLayer::Messaging),
        NodeKind::Sequencer | NodeKind::Indexer => None,
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
        CommandBackend::LogosCore { layer, call } => {
            let runtime = runtime.context("an Inspector-managed logoscore runtime is required")?;
            layer.call(runtime, call)
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

fn logoscore_spec(layer: ManagedNodeLayer, call: ManagedModuleCallSpec) -> LocalNodeCommandSpec {
    let module = layer.module_id();
    let mut args = vec!["call".to_owned(), module.to_owned(), call.method.to_owned()];
    args.extend(call.args.iter().cloned());
    args.push("--json".to_owned());
    LocalNodeCommandSpec {
        program: "logoscore".to_owned(),
        display: shell_display("logoscore", &args),
        args,
        backend: CommandBackend::LogosCore { layer, call },
    }
}

#[must_use]
pub(super) fn has_static_module_contract(kind: NodeKind) -> bool {
    matches!(
        kind,
        NodeKind::Bedrock | NodeKind::Storage | NodeKind::Messaging | NodeKind::Sequencer
    )
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
