use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use crate::support::command_runner::CommandControl;
use crate::support::{confirmation::ConfirmationPolicy, state_store::config_dir};

use super::action_workspace::LocalNodeActionWorkspace;
use super::adapters::{NodeStatusContext, adapter_for};
use super::lifecycle::acquire_state_lock;
use super::model::{
    LocalDevnetListReport, LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord,
    LocalNodeOperationReport, LocalNodeReport, LocalNodeStatus, LocalNodeSummary, LocalNodeTools,
    LocalNodesState, NodeAction, NodeKind, ToolStatus,
};
use super::presentation;
use super::process::{find_command, process_group_has_live_members};
use super::runtime::{self, LogoscoreRuntimeProfile, LogoscoreRuntimeStore};
use super::workflow::{LocalNodeWorkflow, normalized_profile};

const STATE_FILE: &str = "local_nodes.json";

#[derive(Debug, Clone)]
pub(super) struct LocalNodeActionEngine {
    pub(super) store: LocalNodeStore,
    runtime_store: LogoscoreRuntimeStore,
    projector: LocalNodeReportProjector,
    workspace: LocalNodeActionWorkspace,
}

impl LocalNodeActionEngine {
    pub(super) fn system() -> Result<Self> {
        let config_dir = config_dir()?;
        Ok(Self {
            store: LocalNodeStore::for_config_dir(config_dir.clone()),
            runtime_store: LogoscoreRuntimeStore::system(config_dir),
            projector: LocalNodeReportProjector::system(),
            workspace: LocalNodeActionWorkspace::system(),
        })
    }

    pub(super) fn status(&self, profile: &str) -> Result<LocalNodeReport> {
        let _state_lock = acquire_state_lock()?;
        let state = self.store.load()?;
        let runtime = self.runtime_store.load()?;
        Ok(self.projector.report(profile, &state, runtime.as_ref()))
    }

    pub(super) fn devnets(&self, profile: &str) -> Result<LocalDevnetListReport> {
        let _state_lock = acquire_state_lock()?;
        let state = self.store.load()?;
        Ok(LocalDevnetListReport {
            profile: normalized_profile(profile).to_owned(),
            active_devnet: state.active_devnet.clone(),
            workspace_root: state.managed_workspace_root.clone(),
            devnets: state.devnets.clone(),
        })
    }

    pub(super) fn apply(
        &self,
        profile: &str,
        request: LocalNodeActionRequest,
        confirmation: Option<&str>,
    ) -> Result<LocalNodeReport> {
        self.apply_inner(profile, request, confirmation, None)
    }

    pub(super) fn apply_controlled(
        &self,
        profile: &str,
        request: LocalNodeActionRequest,
        confirmation: Option<&str>,
        control: CommandControl,
    ) -> Result<LocalNodeReport> {
        self.apply_inner(profile, request, confirmation, Some(control))
    }

    fn apply_inner(
        &self,
        profile: &str,
        request: LocalNodeActionRequest,
        confirmation: Option<&str>,
        control: Option<CommandControl>,
    ) -> Result<LocalNodeReport> {
        ConfirmationPolicy::LocalNodeAction.require(confirmation)?;

        let _state_lock = acquire_state_lock()?;
        let mut state = self.store.load()?;
        let mut runtime = self.runtime_store.load()?;
        let workflow = LocalNodeWorkflow::for_state(profile, &state);
        workflow.validate_request(&request)?;

        let operation = self.workspace.apply(
            &mut state,
            &mut runtime,
            self.runtime_store.config_root(),
            workflow.profile(),
            &request,
            control.as_ref(),
        );
        state.push_operation(operation.report);
        self.store.save(&state)?;
        self.runtime_store.save(runtime.as_ref())?;
        if let Some(interruption) = operation.interruption {
            return Err(interruption);
        }
        Ok(self.projector.report(profile, &state, runtime.as_ref()))
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LocalNodeReportProjector;

#[cfg(test)]
pub(super) fn report_for_state(profile: &str, state: &LocalNodesState) -> LocalNodeReport {
    LocalNodeReportProjector::system().report(profile, state, None)
}

impl LocalNodeReportProjector {
    fn system() -> Self {
        Self
    }

    fn report(
        self,
        profile: &str,
        state: &LocalNodesState,
        runtime_profile: Option<&LogoscoreRuntimeProfile>,
    ) -> LocalNodeReport {
        let workflow = LocalNodeWorkflow::for_state(profile, state);
        let profile = workflow.profile();
        let active = state.active_topology(profile);
        let tools = self.tool_statuses(runtime_profile);
        let nodes = workflow
            .node_set()
            .into_iter()
            .map(|kind| self.node_status(workflow, state, active, runtime_profile, &tools, kind))
            .collect::<Vec<_>>();
        let installed = nodes
            .iter()
            .filter(|node| node.install_state == "installed")
            .count();
        let running = nodes
            .iter()
            .filter(|node| node.run_state == "running")
            .count();
        let needs_configuration = nodes
            .iter()
            .filter(|node| node.install_state == "needs_configuration")
            .count();
        LocalNodeReport {
            profile: profile.to_owned(),
            mode: presentation::mode_for_profile(profile).to_owned(),
            available_network_actions: if runtime_profile
                .is_some_and(LogoscoreRuntimeProfile::is_running)
            {
                Vec::new()
            } else {
                workflow.network_actions()
            },
            available_runtime_actions: runtime_actions(profile, runtime_profile),
            primary_problem: presentation::primary_problem(profile, &tools, &nodes),
            active_devnet: active.map(|topology| topology.id.clone()),
            workspace_root: state.managed_workspace_root.clone(),
            summary: LocalNodeSummary {
                total: nodes.len(),
                installed,
                running,
                needs_configuration,
            },
            nodes,
            operations: state.operations.clone(),
            tools,
            runtime: runtime::status(runtime_profile),
        }
    }

    fn node_status(
        self,
        workflow: LocalNodeWorkflow,
        state: &LocalNodesState,
        active: Option<&LocalDevnetRecord>,
        runtime: Option<&LogoscoreRuntimeProfile>,
        tools: &LocalNodeTools,
        kind: NodeKind,
    ) -> LocalNodeStatus {
        let adapter = adapter_for(kind);
        let config = active.and_then(|devnet| node_config(devnet, kind));
        let process_id = config.and_then(|node| node.process_id);
        let process_running = process_id.is_some_and(process_group_has_live_members);
        let executable_available = adapter
            .required_executable()
            .is_some_and(|command| find_command(command).is_some());
        let status = adapter.project_status(NodeStatusContext {
            config,
            runtime,
            tools,
            process_running,
            executable_available,
            workflow_actions: workflow.node_actions(kind),
        });
        let last_action = last_operation_for(state, kind);
        LocalNodeStatus {
            kind,
            key: kind.as_str().to_owned(),
            label: adapter.label().to_owned(),
            install_state: status.install_state.to_owned(),
            run_state: status.run_state.to_owned(),
            endpoint: config
                .and_then(|node| node.endpoint.clone())
                .or_else(|| adapter.endpoint(adapter.default_port())),
            data_dir: config.map(|node| node.data_dir.clone()),
            config_path: config.map(|node| node.config_path.clone()),
            package_path: config.and_then(|node| node.package_path.clone()),
            process_id,
            last_action,
            available_actions: status.available_actions,
            detail: status.detail,
        }
    }

    fn tool_statuses(self, runtime: Option<&LogoscoreRuntimeProfile>) -> LocalNodeTools {
        LocalNodeTools {
            logoscore: runtime.map_or_else(
                || self.tool_status("logoscore"),
                |profile| ToolStatus {
                    available: Path::new(&profile.binary_path).is_file(),
                    command: profile.binary_path.clone(),
                    path: Some(profile.binary_path.clone()),
                },
            ),
            lgpm: self.tool_status("lgpm"),
        }
    }

    fn tool_status(self, command: &str) -> ToolStatus {
        ToolStatus {
            available: find_command(command).is_some(),
            command: command.to_owned(),
            path: find_command(command),
        }
    }
}

fn runtime_actions(_profile: &str, runtime: Option<&LogoscoreRuntimeProfile>) -> Vec<NodeAction> {
    match runtime {
        Some(runtime) if runtime.is_managed() && runtime.is_running() => {
            vec![NodeAction::StopRuntime]
        }
        Some(runtime) if runtime.is_managed() => vec![NodeAction::StartRuntime],
        None => vec![NodeAction::StartRuntime],
        Some(_) => Vec::new(),
    }
}

fn last_operation_for(state: &LocalNodesState, kind: NodeKind) -> Option<LocalNodeOperationReport> {
    state
        .operations
        .iter()
        .rev()
        .find(|operation| operation.node == Some(kind))
        .cloned()
}

fn node_config(record: &LocalDevnetRecord, kind: NodeKind) -> Option<&LocalNodeConfigRecord> {
    record.nodes.iter().find(|node| node.kind == kind)
}

#[derive(Debug, Clone)]
pub(super) struct LocalNodeStore {
    config_dir: PathBuf,
}

impl LocalNodeStore {
    pub(super) fn for_config_dir(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub(super) fn load(&self) -> Result<LocalNodesState> {
        let path = self.state_path();
        let mut state: LocalNodesState = if path.is_file() {
            let text = fs::read_to_string(&path).with_context(|| {
                format!("failed to read local node state from {}", path.display())
            })?;
            serde_json::from_str(&text).with_context(|| {
                format!("failed to parse local node state from {}", path.display())
            })?
        } else {
            LocalNodesState::default_for_config_dir(&self.config_dir)
        };
        let mut changed = !path.is_file();
        if state.managed_workspace_root.trim().is_empty() {
            state.managed_workspace_root =
                self.config_dir.join("local-nodes").display().to_string();
            changed = true;
        }
        if state.version < 3 {
            state.version = 3;
            changed = true;
        }
        if super::action_workspace::ensure_testnet_topology(&mut state)? {
            changed = true;
        }
        if state.infer_unambiguous_module_context_topologies() {
            changed = true;
        }
        if changed {
            self.save(&state)?;
        }
        Ok(state)
    }

    pub(super) fn save(&self, state: &LocalNodesState) -> Result<()> {
        let path = self.state_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("failed to create config directory {}", parent.display())
            })?;
        }
        let text =
            serde_json::to_string_pretty(state).context("failed to serialize local node state")?;
        fs::write(&path, text)
            .with_context(|| format!("failed to write local node state to {}", path.display()))?;
        Ok(())
    }

    fn state_path(&self) -> PathBuf {
        state_path_for_config(&self.config_dir)
    }
}

fn state_path_for_config(config: &Path) -> PathBuf {
    config.join(STATE_FILE)
}
