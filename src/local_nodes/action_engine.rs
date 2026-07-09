use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use crate::support::{confirmation::ConfirmationPolicy, state_store::config_dir};

use super::action_workspace::LocalNodeActionWorkspace;
use super::model::{
    LocalDevnetListReport, LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord,
    LocalNodeOperationReport, LocalNodeReport, LocalNodeStatus, LocalNodeSummary, LocalNodeTools,
    LocalNodesState, NodeKind, ToolStatus,
};
use super::presentation;
use super::process::{find_command, process_is_alive};
use super::workflow::{LocalNodeWorkflow, normalized_profile};

const STATE_FILE: &str = "local_nodes.json";

#[derive(Debug, Clone)]
pub(super) struct LocalNodeActionEngine {
    store: LocalNodeStore,
    projector: LocalNodeReportProjector,
    workspace: LocalNodeActionWorkspace,
}

impl LocalNodeActionEngine {
    pub(super) fn system() -> Result<Self> {
        Ok(Self {
            store: LocalNodeStore::system()?,
            projector: LocalNodeReportProjector::system(),
            workspace: LocalNodeActionWorkspace::system(),
        })
    }

    pub(super) fn status(&self, profile: &str) -> Result<LocalNodeReport> {
        let state = self.store.load()?;
        Ok(self.projector.report(profile, &state))
    }

    pub(super) fn devnets(&self, profile: &str) -> Result<LocalDevnetListReport> {
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
        ConfirmationPolicy::LocalNodeAction.require(confirmation)?;

        let mut state = self.store.load()?;
        let workflow = LocalNodeWorkflow::for_state(profile, &state);
        workflow.validate_request(&request)?;

        let operation = self
            .workspace
            .apply(&mut state, workflow.profile(), &request);
        state.push_operation(operation);
        self.store.save(&state)?;
        Ok(self.projector.report(profile, &state))
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct LocalNodeReportProjector;

#[cfg(test)]
pub(super) fn report_for_state(profile: &str, state: &LocalNodesState) -> LocalNodeReport {
    LocalNodeReportProjector::system().report(profile, state)
}

impl LocalNodeReportProjector {
    fn system() -> Self {
        Self
    }

    fn report(self, profile: &str, state: &LocalNodesState) -> LocalNodeReport {
        let workflow = LocalNodeWorkflow::for_state(profile, state);
        let profile = workflow.profile();
        let active = state.active_devnet();
        let tools = self.tool_statuses();
        let nodes = workflow
            .node_set()
            .into_iter()
            .map(|kind| self.node_status(workflow, state, active, &tools, kind))
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
            available_network_actions: workflow.network_actions(),
            primary_problem: presentation::primary_problem(profile, &tools, &nodes),
            active_devnet: state.active_devnet.clone(),
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
        }
    }

    fn node_status(
        self,
        workflow: LocalNodeWorkflow,
        state: &LocalNodesState,
        active: Option<&LocalDevnetRecord>,
        tools: &LocalNodeTools,
        kind: NodeKind,
    ) -> LocalNodeStatus {
        let config = active.and_then(|devnet| node_config(devnet, kind));
        let process_id = config.and_then(|node| node.process_id);
        let process_running = process_id.is_some_and(|pid| self.process_is_alive(pid));
        let installed =
            config.is_some_and(|node| node.installed) || self.tool_backing_available(tools, kind);
        let install_state = if installed {
            "installed"
        } else {
            "needs_configuration"
        };
        let run_state = if process_running {
            "running"
        } else if process_id.is_some() {
            "stale_pid"
        } else {
            "stopped"
        };
        let last_action = last_operation_for(state, kind);
        LocalNodeStatus {
            kind,
            key: kind.as_str().to_owned(),
            label: kind.label().to_owned(),
            install_state: install_state.to_owned(),
            run_state: run_state.to_owned(),
            endpoint: config
                .and_then(|node| node.endpoint.clone())
                .or_else(|| kind.endpoint(kind.default_port())),
            data_dir: config.map(|node| node.data_dir.clone()),
            config_path: config.map(|node| node.config_path.clone()),
            package_path: config.and_then(|node| node.package_path.clone()),
            process_id,
            last_action,
            available_actions: workflow.node_actions(kind),
            detail: node_status_detail(kind, install_state, run_state, tools),
        }
    }

    fn process_is_alive(self, pid: u32) -> bool {
        process_is_alive(pid)
    }

    fn tool_backing_available(self, tools: &LocalNodeTools, kind: NodeKind) -> bool {
        match kind {
            NodeKind::Sequencer => find_command("sequencer_service").is_some(),
            _ => tools.logoscore.available,
        }
    }

    fn tool_statuses(self) -> LocalNodeTools {
        LocalNodeTools {
            logoscore: self.tool_status("logoscore"),
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

fn node_status_detail(
    kind: NodeKind,
    install_state: &str,
    run_state: &str,
    tools: &LocalNodeTools,
) -> String {
    if install_state == "needs_configuration" {
        if kind == NodeKind::Sequencer {
            return "sequencer_service not found".to_owned();
        }
        if !tools.logoscore.available {
            return "logoscore not found".to_owned();
        }
        return "module package path not registered".to_owned();
    }
    if run_state == "stale_pid" {
        return "recorded process id is not running".to_owned();
    }
    "ready".to_owned()
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
    fn system() -> Result<Self> {
        Ok(Self {
            config_dir: config_dir()?,
        })
    }

    #[cfg(test)]
    pub(super) fn for_config_dir(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    pub(super) fn load(&self) -> Result<LocalNodesState> {
        let path = self.state_path();
        if !path.is_file() {
            return Ok(LocalNodesState::default_for_config_dir(&self.config_dir));
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("failed to read local node state from {}", path.display()))?;
        let mut state: LocalNodesState = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse local node state from {}", path.display()))?;
        if state.managed_workspace_root.trim().is_empty() {
            state.managed_workspace_root =
                self.config_dir.join("local-nodes").display().to_string();
        }
        if state.version == 0 {
            state.version = 1;
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
