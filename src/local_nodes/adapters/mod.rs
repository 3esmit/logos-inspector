use serde_json::Value;

use crate::source_routing::{ManagedModuleCallSpec, ManagedNodeAction, ManagedNodeContract};

use super::{
    NodeAction, NodeKind,
    model::{LocalNodeConfigRecord, LocalNodeTools},
    runtime::LogoscoreRuntimeProfile,
};

mod bedrock;
mod indexer;
mod messaging;
mod sequencer;
mod storage;

use bedrock::BEDROCK_ADAPTER;
use indexer::INDEXER_ADAPTER;
use messaging::MESSAGING_ADAPTER;
use sequencer::SEQUENCER_ADAPTER;
use storage::STORAGE_ADAPTER;

const ALL_ADAPTERS: [&dyn LocalNodeAdapter; 5] = [
    &BEDROCK_ADAPTER,
    &SEQUENCER_ADAPTER,
    &INDEXER_ADAPTER,
    &STORAGE_ADAPTER,
    &MESSAGING_ADAPTER,
];

#[derive(Debug, Clone, Copy)]
pub(super) enum NodeLifecycle {
    RuntimeOwnedModule(&'static ManagedNodeContract),
    InitializedModule(&'static ManagedNodeContract),
    RegisteredProcess { program: &'static str },
    Unavailable { reason: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NodeActionPolicy {
    RegisterExecutable {
        program: &'static str,
    },
    RemoveExecutableRegistration,
    ExecuteManaged {
        ensure_loaded: bool,
        requires_installed_context: bool,
    },
    ExecuteDetached,
    PurgeData {
        requires_removed_context: bool,
    },
    Unavailable {
        reason: &'static str,
    },
    Unsupported {
        reason: &'static str,
    },
}

impl NodeActionPolicy {
    #[must_use]
    pub(super) const fn blocked_reason(self) -> Option<&'static str> {
        match self {
            Self::Unavailable { reason } | Self::Unsupported { reason } => Some(reason),
            Self::RegisterExecutable { .. }
            | Self::RemoveExecutableRegistration
            | Self::ExecuteManaged { .. }
            | Self::ExecuteDetached
            | Self::PurgeData { .. } => None,
        }
    }
}

#[derive(Debug, Clone)]
pub(super) enum NodeCommandPlan {
    ManagedModule {
        contract: &'static ManagedNodeContract,
        call: ManagedModuleCallSpec,
    },
    DetachedProcess {
        program: &'static str,
        args: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NodeConfigContext<'a> {
    pub network_id: &'a str,
    pub data_dir: &'a str,
    pub endpoint: Option<&'a str>,
    pub port: Option<u16>,
}

pub(super) struct NodeStatusContext<'a> {
    pub config: Option<&'a LocalNodeConfigRecord>,
    pub runtime: Option<&'a LogoscoreRuntimeProfile>,
    pub tools: &'a LocalNodeTools,
    pub process_running: bool,
    pub executable_available: bool,
    pub workflow_actions: Vec<NodeAction>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NodeStatusProjection {
    pub install_state: &'static str,
    pub run_state: &'static str,
    pub available_actions: Vec<NodeAction>,
    pub detail: String,
}

pub(super) trait LocalNodeAdapter: std::fmt::Debug + Sync {
    fn kind(&self) -> NodeKind;

    fn label(&self) -> &'static str;

    fn default_port(&self) -> Option<u16>;

    fn lifecycle(&self) -> NodeLifecycle;

    fn workflow_actions(&self) -> &'static [NodeAction];

    fn build_config(&self, context: NodeConfigContext<'_>) -> Value;

    fn available_in_profile(&self, _profile: &str) -> bool {
        true
    }

    fn endpoint(&self, port: Option<u16>) -> Option<String> {
        port.map(|value| format!("http://127.0.0.1:{value}/"))
    }

    fn required_executable(&self) -> Option<&'static str> {
        match self.lifecycle() {
            NodeLifecycle::RegisteredProcess { program } => Some(program),
            NodeLifecycle::RuntimeOwnedModule(_)
            | NodeLifecycle::InitializedModule(_)
            | NodeLifecycle::Unavailable { .. } => None,
        }
    }

    fn managed_contract(&self) -> Option<&'static ManagedNodeContract> {
        match self.lifecycle() {
            NodeLifecycle::RuntimeOwnedModule(contract)
            | NodeLifecycle::InitializedModule(contract) => Some(contract),
            NodeLifecycle::RegisteredProcess { .. } | NodeLifecycle::Unavailable { .. } => None,
        }
    }

    fn command_plan(&self, action: NodeAction, config_path: &str) -> Option<NodeCommandPlan> {
        match self.lifecycle() {
            NodeLifecycle::RuntimeOwnedModule(contract)
            | NodeLifecycle::InitializedModule(contract) => {
                let call = contract.call_spec(managed_action(action)?, config_path)?;
                Some(NodeCommandPlan::ManagedModule { contract, call })
            }
            NodeLifecycle::RegisteredProcess { program } if action == NodeAction::Start => {
                Some(NodeCommandPlan::DetachedProcess {
                    program,
                    args: vec![config_path.to_owned()],
                })
            }
            NodeLifecycle::RegisteredProcess { .. } | NodeLifecycle::Unavailable { .. } => None,
        }
    }

    fn action_policy(&self, action: NodeAction) -> NodeActionPolicy {
        let lifecycle = self.lifecycle();
        if let NodeLifecycle::Unavailable { reason } = lifecycle {
            return NodeActionPolicy::Unavailable { reason };
        }
        match action {
            NodeAction::Install => match lifecycle {
                NodeLifecycle::RegisteredProcess { program } => {
                    NodeActionPolicy::RegisterExecutable { program }
                }
                NodeLifecycle::RuntimeOwnedModule(_) | NodeLifecycle::InitializedModule(_) => {
                    NodeActionPolicy::Unsupported {
                        reason: "module nodes must be initialized before they can start",
                    }
                }
                NodeLifecycle::Unavailable { reason } => NodeActionPolicy::Unavailable { reason },
            },
            NodeAction::Initialize => match lifecycle {
                NodeLifecycle::InitializedModule(_) => NodeActionPolicy::ExecuteManaged {
                    ensure_loaded: true,
                    requires_installed_context: false,
                },
                NodeLifecycle::RegisteredProcess { .. } => NodeActionPolicy::Unsupported {
                    reason: "this process node uses install registration instead of module initialization",
                },
                NodeLifecycle::RuntimeOwnedModule(_) => NodeActionPolicy::Unsupported {
                    reason: "this module is initialized by its runtime-owned start lifecycle",
                },
                NodeLifecycle::Unavailable { reason } => NodeActionPolicy::Unavailable { reason },
            },
            NodeAction::Uninstall => match lifecycle {
                NodeLifecycle::RegisteredProcess { .. } => {
                    NodeActionPolicy::RemoveExecutableRegistration
                }
                NodeLifecycle::RuntimeOwnedModule(contract)
                | NodeLifecycle::InitializedModule(contract) => {
                    if contract.call_spec(ManagedNodeAction::Destroy, "").is_some() {
                        NodeActionPolicy::ExecuteManaged {
                            ensure_loaded: false,
                            requires_installed_context: true,
                        }
                    } else {
                        NodeActionPolicy::Unsupported {
                            reason: "this module has no verified context-destroy contract; stop the managed runtime to clear it",
                        }
                    }
                }
                NodeLifecycle::Unavailable { reason } => NodeActionPolicy::Unavailable { reason },
            },
            NodeAction::Start => match lifecycle {
                NodeLifecycle::RegisteredProcess { .. } => NodeActionPolicy::ExecuteDetached,
                NodeLifecycle::RuntimeOwnedModule(_) => NodeActionPolicy::ExecuteManaged {
                    ensure_loaded: true,
                    requires_installed_context: false,
                },
                NodeLifecycle::InitializedModule(_) => NodeActionPolicy::ExecuteManaged {
                    ensure_loaded: false,
                    requires_installed_context: true,
                },
                NodeLifecycle::Unavailable { reason } => NodeActionPolicy::Unavailable { reason },
            },
            NodeAction::Stop => match lifecycle {
                NodeLifecycle::RegisteredProcess { .. } => NodeActionPolicy::ExecuteDetached,
                NodeLifecycle::RuntimeOwnedModule(_) | NodeLifecycle::InitializedModule(_) => {
                    NodeActionPolicy::ExecuteManaged {
                        ensure_loaded: false,
                        requires_installed_context: true,
                    }
                }
                NodeLifecycle::Unavailable { reason } => NodeActionPolicy::Unavailable { reason },
            },
            NodeAction::Purge => NodeActionPolicy::PurgeData {
                requires_removed_context: !matches!(
                    lifecycle,
                    NodeLifecycle::RegisteredProcess { .. }
                ),
            },
            NodeAction::StartRuntime
            | NodeAction::StopRuntime
            | NodeAction::NewNetwork
            | NodeAction::LoadNetwork
            | NodeAction::DeleteNetwork
            | NodeAction::ResetNetwork => NodeActionPolicy::Unsupported {
                reason: "action does not target an individual node adapter",
            },
        }
    }

    fn resets_with_runtime(&self) -> bool {
        matches!(
            self.lifecycle(),
            NodeLifecycle::RuntimeOwnedModule(_) | NodeLifecycle::InitializedModule(_)
        )
    }

    fn unavailable_reason(&self) -> Option<&'static str> {
        match self.lifecycle() {
            NodeLifecycle::Unavailable { reason } => Some(reason),
            NodeLifecycle::RuntimeOwnedModule(_)
            | NodeLifecycle::InitializedModule(_)
            | NodeLifecycle::RegisteredProcess { .. } => None,
        }
    }

    fn project_status(&self, context: NodeStatusContext<'_>) -> NodeStatusProjection {
        let runtime_running = context
            .runtime
            .is_some_and(LogoscoreRuntimeProfile::is_running);
        let installed = match self.lifecycle() {
            NodeLifecycle::RegisteredProcess { .. } => {
                context.config.is_some_and(|node| node.installed) || context.executable_available
            }
            NodeLifecycle::Unavailable { .. } => false,
            NodeLifecycle::RuntimeOwnedModule(_) | NodeLifecycle::InitializedModule(_) => {
                runtime_running && context.config.is_some_and(|node| node.installed)
            }
        };
        let install_state = if installed {
            "installed"
        } else {
            "needs_configuration"
        };
        let run_state = match self.lifecycle() {
            NodeLifecycle::RegisteredProcess { .. } if context.process_running => "running",
            NodeLifecycle::RegisteredProcess { .. }
                if context.config.and_then(|node| node.process_id).is_some() =>
            {
                "stale_pid"
            }
            NodeLifecycle::RegisteredProcess { .. } => "stopped",
            NodeLifecycle::Unavailable { .. } => "not_initialized",
            NodeLifecycle::RuntimeOwnedModule(_) | NodeLifecycle::InitializedModule(_)
                if !runtime_running =>
            {
                if context.config.is_some_and(|node| node.installed) {
                    "stopped"
                } else {
                    "not_initialized"
                }
            }
            NodeLifecycle::RuntimeOwnedModule(_) | NodeLifecycle::InitializedModule(_) => context
                .config
                .map(|node| node.lifecycle_state.as_str())
                .unwrap_or("not_initialized"),
        };
        let available_actions = self.available_actions(&context);
        let detail = self.status_detail(install_state, run_state, &context);
        NodeStatusProjection {
            install_state,
            run_state,
            available_actions,
            detail,
        }
    }

    fn available_actions(&self, context: &NodeStatusContext<'_>) -> Vec<NodeAction> {
        match self.lifecycle() {
            NodeLifecycle::Unavailable { .. } => Vec::new(),
            NodeLifecycle::RegisteredProcess { .. } => context.workflow_actions.clone(),
            NodeLifecycle::RuntimeOwnedModule(_) => {
                if !context
                    .runtime
                    .is_some_and(|profile| profile.is_managed() && profile.is_running())
                    || context.config.is_none()
                    || context
                        .config
                        .is_some_and(|node| node.lifecycle_state.is_pending())
                {
                    return Vec::new();
                }
                context.workflow_actions.clone()
            }
            NodeLifecycle::InitializedModule(_) => {
                if !context
                    .runtime
                    .is_some_and(|profile| profile.is_managed() && profile.is_running())
                    || context.config.is_none()
                    || context
                        .config
                        .is_some_and(|node| node.lifecycle_state.is_pending())
                {
                    return Vec::new();
                }
                let mut actions = context.workflow_actions.clone();
                if context.config.is_some_and(|node| node.installed) {
                    actions.retain(|action| *action != NodeAction::Initialize);
                } else {
                    actions.retain(|action| *action == NodeAction::Initialize);
                }
                actions
            }
        }
    }

    fn status_detail(
        &self,
        install_state: &str,
        run_state: &str,
        context: &NodeStatusContext<'_>,
    ) -> String {
        if install_state == "needs_configuration" {
            if let Some(reason) = self.unavailable_reason() {
                return reason.to_owned();
            }
            if let Some(executable) = self.required_executable()
                && !context.executable_available
            {
                return format!("{executable} not found");
            }
            if !context.tools.logoscore.available {
                return "logoscore not found".to_owned();
            }
            if context.runtime.is_none() {
                return "start an Inspector-managed logoscore runtime".to_owned();
            }
            if !context
                .runtime
                .is_some_and(LogoscoreRuntimeProfile::is_running)
            {
                return "Inspector-managed logoscore daemon is stopped".to_owned();
            }
            return "module context is not initialized".to_owned();
        }
        if run_state == "stale_pid" {
            return "recorded process id is not running".to_owned();
        }
        match run_state {
            "starting" | "stopping" => "waiting for module lifecycle event".to_owned(),
            "unknown" => "module dispatch has no verified lifecycle observer".to_owned(),
            "failed" => "latest module lifecycle event reported failure".to_owned(),
            _ => "ready".to_owned(),
        }
    }
}

#[must_use]
pub(super) fn adapter_for(kind: NodeKind) -> &'static dyn LocalNodeAdapter {
    match kind {
        NodeKind::Bedrock => &BEDROCK_ADAPTER,
        NodeKind::Sequencer => &SEQUENCER_ADAPTER,
        NodeKind::Indexer => &INDEXER_ADAPTER,
        NodeKind::Storage => &STORAGE_ADAPTER,
        NodeKind::Messaging => &MESSAGING_ADAPTER,
    }
}

#[must_use]
pub(super) fn adapters_for_profile(profile: &str) -> Vec<&'static dyn LocalNodeAdapter> {
    ALL_ADAPTERS
        .iter()
        .copied()
        .filter(|adapter| adapter.available_in_profile(profile))
        .collect()
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use anyhow::{Result, bail};

    use super::*;

    fn assert_adapter_contract(adapter: &dyn LocalNodeAdapter) -> Result<()> {
        if adapter.label().trim().is_empty() {
            bail!("{} adapter has no label", adapter.kind().as_str());
        }
        let actions = adapter.workflow_actions();
        let mut seen_actions = Vec::new();
        for action in actions {
            if seen_actions.contains(action) {
                bail!(
                    "{} adapter exposes duplicate actions",
                    adapter.kind().as_str()
                );
            }
            seen_actions.push(*action);
        }
        if matches!(adapter.lifecycle(), NodeLifecycle::Unavailable { .. }) && !actions.is_empty() {
            bail!(
                "unavailable {} adapter exposes actions",
                adapter.kind().as_str()
            );
        }

        let config = adapter.build_config(NodeConfigContext {
            network_id: "contract-test",
            data_dir: "/tmp/contract-test",
            endpoint: adapter.endpoint(adapter.default_port()).as_deref(),
            port: adapter.default_port(),
        });
        if !config.is_object() {
            bail!(
                "{} adapter did not build an object",
                adapter.kind().as_str()
            );
        }

        for action in actions {
            if adapter.action_policy(*action).blocked_reason().is_some() {
                bail!(
                    "{} adapter exposes blocked {:?} action",
                    adapter.kind().as_str(),
                    action
                );
            }
            let command = adapter.command_plan(*action, "/tmp/config.json");
            if matches!(
                adapter.action_policy(*action),
                NodeActionPolicy::ExecuteManaged { .. }
            ) && command.is_none()
            {
                bail!(
                    "{} adapter has no {:?} command",
                    adapter.kind().as_str(),
                    action
                );
            }
        }
        Ok(())
    }

    #[test]
    fn every_node_adapter_satisfies_shared_contract() -> Result<()> {
        let mut kinds = BTreeSet::new();
        for adapter in ALL_ADAPTERS {
            if !kinds.insert(adapter.kind()) {
                bail!("duplicate {:?} adapter", adapter.kind());
            }
            assert_adapter_contract(adapter)?;
        }
        if kinds.len() != 5 {
            bail!("expected five node adapters, found {}", kinds.len());
        }
        Ok(())
    }

    #[test]
    fn managed_module_adapters_share_module_contract() -> Result<()> {
        for adapter in ALL_ADAPTERS {
            if !matches!(
                adapter.lifecycle(),
                NodeLifecycle::RuntimeOwnedModule(_) | NodeLifecycle::InitializedModule(_)
            ) {
                continue;
            }
            if adapter.managed_contract().is_none() {
                bail!("{} lost managed module contract", adapter.kind().as_str());
            }
            if adapter
                .command_plan(NodeAction::Start, "/tmp/config.json")
                .is_none()
            {
                bail!("{} lost start command", adapter.kind().as_str());
            }
        }
        Ok(())
    }
}
