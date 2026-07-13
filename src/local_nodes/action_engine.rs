use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result};

use crate::support::{confirmation::ConfirmationPolicy, state_store::config_dir};

use super::action_workspace::LocalNodeActionWorkspace;
use super::commands::has_static_module_contract;
use super::lifecycle::{
    LifecycleTarget, acquire_state_lock, apply_event, cancel_event_watch, has_event_contract,
    start_event_watch,
};
use super::model::{
    LocalDevnetListReport, LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord,
    LocalNodeOperationReport, LocalNodeReport, LocalNodeStatus, LocalNodeSummary, LocalNodeTools,
    LocalNodesState, NodeAction, NodeKind, ToolStatus,
};
use super::presentation;
use super::process::{find_command, process_is_alive};
use super::runtime::{self, LogoscoreRuntimeProfile, LogoscoreRuntimeStore};
use super::workflow::{LocalNodeWorkflow, normalized_profile};

const STATE_FILE: &str = "local_nodes.json";

#[derive(Debug, Clone)]
pub(super) struct LocalNodeActionEngine {
    store: LocalNodeStore,
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
        ConfirmationPolicy::LocalNodeAction.require(confirmation)?;

        let _state_lock = acquire_state_lock()?;
        let mut state = self.store.load()?;
        let mut runtime = self.runtime_store.load()?;
        let workflow = LocalNodeWorkflow::for_state(profile, &state);
        workflow.validate_request(&request)?;

        let watch = self.start_lifecycle_watch(&state, runtime.as_ref(), &request)?;
        let operation = self.workspace.apply(
            &mut state,
            &mut runtime,
            self.runtime_store.config_root(),
            workflow.profile(),
            &request,
        );
        if !matches!(operation.status.as_str(), "starting" | "stopping")
            && let Some(watch) = watch
        {
            cancel_event_watch(watch);
        }
        state.push_operation(operation);
        self.store.save(&state)?;
        self.runtime_store.save(runtime.as_ref())?;
        Ok(self.projector.report(profile, &state, runtime.as_ref()))
    }

    fn start_lifecycle_watch(
        &self,
        state: &LocalNodesState,
        runtime: Option<&LogoscoreRuntimeProfile>,
        request: &LocalNodeActionRequest,
    ) -> Result<Option<super::lifecycle::LifecycleWatchRegistration>> {
        let Some(kind) = request.node else {
            return Ok(None);
        };
        if !matches!(request.action, NodeAction::Start | NodeAction::Stop)
            || !has_event_contract(kind, request.action)
        {
            return Ok(None);
        }
        let Some(runtime) = runtime.filter(|profile| profile.is_managed() && profile.is_running())
        else {
            return Ok(None);
        };
        let Some(network_id) = state.active_devnet.clone() else {
            return Ok(None);
        };
        let store = self.store.clone();
        let target = LifecycleTarget {
            network_id,
            kind,
            action: request.action,
        };
        start_event_watch(runtime, target, move |event| {
            let _state_lock = acquire_state_lock()?;
            let mut state = store.load()?;
            if apply_event(&mut state, &event) {
                store.save(&state)?;
            }
            Ok(())
        })
        .map(Some)
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
        let active = state.active_devnet();
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
        let config = active.and_then(|devnet| node_config(devnet, kind));
        let process_id = config.and_then(|node| node.process_id);
        let process_running = process_id.is_some_and(|pid| self.process_is_alive(pid));
        let runtime_running = runtime.is_some_and(LogoscoreRuntimeProfile::is_running);
        let installed = match kind {
            NodeKind::Sequencer => {
                config.is_some_and(|node| node.installed)
                    || self.tool_backing_available(tools, kind)
            }
            NodeKind::Indexer => false,
            _ => runtime_running && config.is_some_and(|node| node.installed),
        };
        let install_state = if installed {
            "installed"
        } else {
            "needs_configuration"
        };
        let run_state = if kind == NodeKind::Sequencer {
            if process_running {
                "running"
            } else if process_id.is_some() {
                "stale_pid"
            } else {
                "stopped"
            }
        } else if !runtime_running {
            if config.is_some_and(|node| node.installed) {
                "stopped"
            } else {
                "not_initialized"
            }
        } else {
            config
                .map(|node| node.lifecycle_state.as_str())
                .unwrap_or("not_initialized")
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
            available_actions: available_node_actions(workflow, kind, config, runtime),
            detail: node_status_detail(kind, install_state, run_state, runtime, tools),
        }
    }

    fn process_is_alive(self, pid: u32) -> bool {
        process_is_alive(pid)
    }

    fn tool_backing_available(self, tools: &LocalNodeTools, kind: NodeKind) -> bool {
        match kind {
            NodeKind::Sequencer => find_command("sequencer_service").is_some(),
            NodeKind::Indexer => false,
            _ => tools.logoscore.available,
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

fn runtime_actions(profile: &str, runtime: Option<&LogoscoreRuntimeProfile>) -> Vec<NodeAction> {
    if profile != "local" {
        return Vec::new();
    }
    match runtime {
        Some(runtime) if runtime.is_managed() && runtime.is_running() => {
            vec![NodeAction::StopRuntime]
        }
        Some(runtime) if runtime.is_managed() => vec![NodeAction::StartRuntime],
        None => vec![NodeAction::StartRuntime],
        Some(_) => Vec::new(),
    }
}

fn available_node_actions(
    workflow: LocalNodeWorkflow,
    kind: NodeKind,
    config: Option<&LocalNodeConfigRecord>,
    runtime: Option<&LogoscoreRuntimeProfile>,
) -> Vec<NodeAction> {
    if !has_static_module_contract(kind) {
        return Vec::new();
    }
    if kind == NodeKind::Sequencer {
        return workflow.node_actions(kind);
    }
    if !runtime.is_some_and(|profile| profile.is_managed() && profile.is_running())
        || config.is_none()
        || config.is_some_and(|node| node.lifecycle_state.is_pending())
    {
        return Vec::new();
    }
    let mut actions = workflow.node_actions(kind);
    if matches!(kind, NodeKind::Storage | NodeKind::Messaging) {
        if config.is_some_and(|node| node.installed) {
            actions.retain(|action| *action != NodeAction::Initialize);
        } else {
            actions.retain(|action| *action == NodeAction::Initialize);
        }
    }
    actions
}

fn node_status_detail(
    kind: NodeKind,
    install_state: &str,
    run_state: &str,
    runtime: Option<&LogoscoreRuntimeProfile>,
    tools: &LocalNodeTools,
) -> String {
    if install_state == "needs_configuration" {
        if !has_static_module_contract(kind) {
            return "no verified logoscore module lifecycle contract".to_owned();
        }
        if kind == NodeKind::Sequencer {
            return "sequencer_service not found".to_owned();
        }
        if !tools.logoscore.available {
            return "logoscore not found".to_owned();
        }
        if runtime.is_none() {
            return "start an Inspector-managed logoscore runtime".to_owned();
        }
        if !runtime.is_some_and(LogoscoreRuntimeProfile::is_running) {
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
