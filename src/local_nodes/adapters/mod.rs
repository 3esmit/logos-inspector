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
    InitializedModule(&'static ManagedNodeContract),
    RegisteredProcess { program: &'static str },
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
    Unsupported {
        reason: &'static str,
    },
}

impl NodeActionPolicy {
    #[must_use]
    pub(super) const fn blocked_reason(self) -> Option<&'static str> {
        match self {
            Self::Unsupported { reason } => Some(reason),
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
    pub config_path: &'a str,
    pub data_dir: &'a str,
    pub endpoint: Option<&'a str>,
    pub port: Option<u16>,
    pub public_testnet: bool,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct NodeCommandContext<'a> {
    pub config_path: &'a str,
    pub data_dir: &'a str,
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

    fn preserve_generated_config_on_runtime_reset(&self) -> bool {
        false
    }

    fn ensure_loaded_before_start(&self) -> bool {
        false
    }

    fn startup_rpc_health_method(&self) -> Option<&'static str> {
        None
    }

    fn endpoint(&self, port: Option<u16>) -> Option<String> {
        port.map(|value| format!("http://127.0.0.1:{value}/"))
    }

    fn required_executable(&self) -> Option<&'static str> {
        match self.lifecycle() {
            NodeLifecycle::RegisteredProcess { program } => Some(program),
            NodeLifecycle::InitializedModule(_) => None,
        }
    }

    fn managed_contract(&self) -> Option<&'static ManagedNodeContract> {
        match self.lifecycle() {
            NodeLifecycle::InitializedModule(contract) => Some(contract),
            NodeLifecycle::RegisteredProcess { .. } => None,
        }
    }

    fn command_plan(
        &self,
        action: NodeAction,
        context: NodeCommandContext<'_>,
    ) -> Option<NodeCommandPlan> {
        match self.lifecycle() {
            NodeLifecycle::InitializedModule(contract) => {
                let call = contract.call_spec(managed_action(action)?, context.config_path)?;
                Some(NodeCommandPlan::ManagedModule { contract, call })
            }
            NodeLifecycle::RegisteredProcess { program } if action == NodeAction::Start => {
                Some(NodeCommandPlan::DetachedProcess {
                    program,
                    args: vec![context.config_path.to_owned()],
                })
            }
            NodeLifecycle::RegisteredProcess { .. } => None,
        }
    }

    fn action_policy(&self, action: NodeAction) -> NodeActionPolicy {
        let lifecycle = self.lifecycle();
        match action {
            NodeAction::Install => match lifecycle {
                NodeLifecycle::RegisteredProcess { program } => {
                    NodeActionPolicy::RegisterExecutable { program }
                }
                NodeLifecycle::InitializedModule(_) => NodeActionPolicy::Unsupported {
                    reason: "module nodes must be initialized before they can start",
                },
            },
            NodeAction::Initialize => match lifecycle {
                NodeLifecycle::InitializedModule(_) => NodeActionPolicy::ExecuteManaged {
                    ensure_loaded: true,
                    requires_installed_context: false,
                },
                NodeLifecycle::RegisteredProcess { .. } => NodeActionPolicy::Unsupported {
                    reason: "this process node uses install registration instead of module initialization",
                },
            },
            NodeAction::Uninstall => match lifecycle {
                NodeLifecycle::RegisteredProcess { .. } => {
                    NodeActionPolicy::RemoveExecutableRegistration
                }
                NodeLifecycle::InitializedModule(contract) => {
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
            },
            NodeAction::Start => match lifecycle {
                NodeLifecycle::RegisteredProcess { .. } => NodeActionPolicy::ExecuteDetached,
                NodeLifecycle::InitializedModule(_) => NodeActionPolicy::ExecuteManaged {
                    ensure_loaded: self.ensure_loaded_before_start(),
                    requires_installed_context: true,
                },
            },
            NodeAction::Stop => match lifecycle {
                NodeLifecycle::RegisteredProcess { .. } => NodeActionPolicy::ExecuteDetached,
                NodeLifecycle::InitializedModule(_) => NodeActionPolicy::ExecuteManaged {
                    ensure_loaded: false,
                    requires_installed_context: true,
                },
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
        matches!(self.lifecycle(), NodeLifecycle::InitializedModule(_))
    }

    fn project_status(&self, context: NodeStatusContext<'_>) -> NodeStatusProjection {
        let runtime_running = context
            .runtime
            .is_some_and(LogoscoreRuntimeProfile::is_running);
        let installed = match self.lifecycle() {
            NodeLifecycle::RegisteredProcess { .. } => {
                context.config.is_some_and(|node| node.installed) || context.executable_available
            }
            NodeLifecycle::InitializedModule(_) => {
                runtime_running && context.config.is_some_and(|node| node.installed)
            }
        };
        let install_state = if installed {
            "installed"
        } else {
            "needs_configuration"
        };
        let run_state = match self.lifecycle() {
            NodeLifecycle::RegisteredProcess { .. }
                if context.config.is_some_and(|node| {
                    matches!(
                        node.lifecycle_state,
                        super::NodeLifecycleState::Initializing
                            | super::NodeLifecycleState::Starting
                            | super::NodeLifecycleState::Stopping
                            | super::NodeLifecycleState::Unknown
                            | super::NodeLifecycleState::Failed
                    )
                }) =>
            {
                context
                    .config
                    .map(|node| node.lifecycle_state.as_str())
                    .unwrap_or("unknown")
            }
            NodeLifecycle::RegisteredProcess { .. } if context.process_running => "running",
            NodeLifecycle::RegisteredProcess { .. }
                if context.config.and_then(|node| node.process_id).is_some() =>
            {
                "stale_pid"
            }
            NodeLifecycle::RegisteredProcess { .. } => "stopped",
            NodeLifecycle::InitializedModule(_) if !runtime_running => {
                if context.config.is_some_and(|node| node.installed) {
                    "stopped"
                } else {
                    "not_initialized"
                }
            }
            NodeLifecycle::InitializedModule(_) => context
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
            NodeLifecycle::RegisteredProcess { .. } => {
                let Some(config) = context.config else {
                    return Vec::new();
                };
                if config.lifecycle_state.is_pending() {
                    return Vec::new();
                }
                let mut actions = context.workflow_actions.clone();
                if !config.installed {
                    actions.retain(|action| *action == NodeAction::Install);
                    return actions;
                }
                actions.retain(|action| match action {
                    NodeAction::Install => false,
                    NodeAction::Start => !context.process_running,
                    NodeAction::Stop => context.process_running && config.process_id.is_some(),
                    NodeAction::Uninstall | NodeAction::Purge => true,
                    _ => false,
                });
                actions
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
            "starting" => "waiting for endpoint confirmation".to_owned(),
            "stopping" => "waiting for endpoint shutdown confirmation".to_owned(),
            "unknown" => "endpoint liveness is not confirmed".to_owned(),
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

    fn registered_process_config(
        installed: bool,
        process_id: Option<u32>,
    ) -> LocalNodeConfigRecord {
        LocalNodeConfigRecord {
            kind: NodeKind::Indexer,
            config_path: "/tmp/indexer.json".to_owned(),
            initialization_config_path: None,
            data_dir: "/tmp/indexer".to_owned(),
            endpoint: Some("http://127.0.0.1:8779/".to_owned()),
            port: Some(8779),
            package_path: Some("/usr/bin/indexer_service".to_owned()),
            module_path: None,
            process_id,
            installed,
            lifecycle_state: super::super::NodeLifecycleState::Stopped,
            pending_lifecycle_action: None,
        }
    }

    fn configured_tools() -> LocalNodeTools {
        LocalNodeTools {
            logoscore: super::super::ToolStatus {
                available: true,
                command: "logoscore".to_owned(),
                path: Some("/usr/bin/logoscore".to_owned()),
            },
            lgpm: super::super::ToolStatus {
                available: false,
                command: "lgpm".to_owned(),
                path: None,
            },
        }
    }

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
        let config = adapter.build_config(NodeConfigContext {
            network_id: "contract-test",
            config_path: "/tmp/contract-test.json",
            data_dir: "/tmp/contract-test",
            endpoint: adapter.endpoint(adapter.default_port()).as_deref(),
            port: adapter.default_port(),
            public_testnet: false,
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
            let command = adapter.command_plan(
                *action,
                NodeCommandContext {
                    config_path: "/tmp/config.json",
                    data_dir: "/tmp/data",
                    port: adapter.default_port(),
                },
            );
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
            if !matches!(adapter.lifecycle(), NodeLifecycle::InitializedModule(_)) {
                continue;
            }
            if adapter.managed_contract().is_none() {
                bail!("{} lost managed module contract", adapter.kind().as_str());
            }
            if adapter
                .command_plan(
                    NodeAction::Start,
                    NodeCommandContext {
                        config_path: "/tmp/config.json",
                        data_dir: "/tmp/data",
                        port: adapter.default_port(),
                    },
                )
                .is_none()
            {
                bail!("{} lost start command", adapter.kind().as_str());
            }
        }
        Ok(())
    }

    #[test]
    fn registered_process_actions_require_owned_process_for_stop() {
        let adapter = adapter_for(NodeKind::Indexer);
        let tools = configured_tools();
        let unowned = registered_process_config(true, None);

        let unowned_status = adapter.project_status(NodeStatusContext {
            config: Some(&unowned),
            runtime: None,
            tools: &tools,
            process_running: false,
            executable_available: true,
            workflow_actions: adapter.workflow_actions().to_vec(),
        });

        assert!(
            unowned_status
                .available_actions
                .contains(&NodeAction::Start)
        );
        assert!(!unowned_status.available_actions.contains(&NodeAction::Stop));

        let owned = registered_process_config(true, Some(42));
        let owned_status = adapter.project_status(NodeStatusContext {
            config: Some(&owned),
            runtime: None,
            tools: &tools,
            process_running: true,
            executable_available: true,
            workflow_actions: adapter.workflow_actions().to_vec(),
        });

        assert!(!owned_status.available_actions.contains(&NodeAction::Start));
        assert!(owned_status.available_actions.contains(&NodeAction::Stop));
    }

    #[test]
    fn registered_process_projection_surfaces_watcher_transitions() {
        let adapter = adapter_for(NodeKind::Indexer);
        let tools = configured_tools();
        for (lifecycle_state, expected_run_state) in [
            (super::super::NodeLifecycleState::Starting, "starting"),
            (super::super::NodeLifecycleState::Stopping, "stopping"),
            (super::super::NodeLifecycleState::Unknown, "unknown"),
            (super::super::NodeLifecycleState::Failed, "failed"),
        ] {
            let mut config = registered_process_config(true, Some(42));
            config.lifecycle_state = lifecycle_state;
            let status = adapter.project_status(NodeStatusContext {
                config: Some(&config),
                runtime: None,
                tools: &tools,
                process_running: true,
                executable_available: true,
                workflow_actions: adapter.workflow_actions().to_vec(),
            });

            assert_eq!(status.run_state, expected_run_state);
        }
    }
}
