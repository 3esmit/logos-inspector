use std::{
    env, fs,
    path::{Component, Path, PathBuf},
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context as _, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{logoscore, state_store::config_dir};

const STATE_FILE: &str = "local_nodes.json";
const MANIFEST_FILE: &str = "local-network.json";
const CONFIRMATION_TOKEN: &str = "confirm-local-node-action";
const BLOCKCHAIN_MODULE: &str = "blockchain_module";
const INDEXER_MODULE: &str = "lez_indexer_module";
const STORAGE_MODULE: &str = "storage_module";
const DELIVERY_MODULE: &str = "delivery_module";
const DEFAULT_DEPLOYMENT: &str = "local";
const HISTORY_LIMIT: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Bedrock,
    Sequencer,
    Indexer,
    Storage,
    Messaging,
}

impl NodeKind {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Bedrock => "bedrock",
            Self::Sequencer => "sequencer",
            Self::Indexer => "indexer",
            Self::Storage => "storage",
            Self::Messaging => "messaging",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Bedrock => "Bedrock",
            Self::Sequencer => "Local Sequencer",
            Self::Indexer => "Indexer",
            Self::Storage => "Storage",
            Self::Messaging => "Messaging",
        }
    }

    fn default_port(self) -> Option<u16> {
        match self {
            Self::Bedrock => Some(8080),
            Self::Sequencer => Some(3040),
            Self::Indexer => Some(8779),
            Self::Storage => Some(8081),
            Self::Messaging => Some(8645),
        }
    }

    fn endpoint(self, port: Option<u16>) -> Option<String> {
        port.map(|value| match self {
            Self::Storage => format!("http://127.0.0.1:{value}/api/storage/v1"),
            _ => format!("http://127.0.0.1:{value}/"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeAction {
    Install,
    Uninstall,
    NewNetwork,
    LoadNetwork,
    DeleteNetwork,
    ResetNetwork,
    Start,
    Stop,
    Purge,
}

impl NodeAction {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Uninstall => "uninstall",
            Self::NewNetwork => "new_network",
            Self::LoadNetwork => "load_network",
            Self::DeleteNetwork => "delete_network",
            Self::ResetNetwork => "reset_network",
            Self::Start => "start",
            Self::Stop => "stop",
            Self::Purge => "purge",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Install => "Install",
            Self::Uninstall => "Uninstall",
            Self::NewNetwork => "New network",
            Self::LoadNetwork => "Load network",
            Self::DeleteNetwork => "Delete network",
            Self::ResetNetwork => "Reset network",
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Purge => "Purge",
        }
    }

    fn is_network_action(self) -> bool {
        matches!(
            self,
            Self::NewNetwork | Self::LoadNetwork | Self::DeleteNetwork | Self::ResetNetwork
        )
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct LocalNodeActionRequest {
    pub action: NodeAction,
    #[serde(default)]
    pub node: Option<NodeKind>,
    #[serde(default)]
    pub network_id: Option<String>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalNodeReport {
    pub profile: String,
    pub mode: String,
    pub active_devnet: Option<String>,
    pub workspace_root: String,
    pub summary: LocalNodeSummary,
    pub nodes: Vec<LocalNodeStatus>,
    pub operations: Vec<LocalNodeOperationReport>,
    pub tools: LocalNodeTools,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalNodeSummary {
    pub total: usize,
    pub installed: usize,
    pub running: usize,
    pub needs_configuration: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalNodeStatus {
    pub kind: NodeKind,
    pub key: String,
    pub label: String,
    pub install_state: String,
    pub run_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_dir: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_action: Option<LocalNodeOperationReport>,
    pub available_actions: Vec<NodeAction>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalNodeOperationReport {
    pub id: String,
    pub time: String,
    pub timestamp_millis: u64,
    pub action: NodeAction,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub node: Option<NodeKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network_id: Option<String>,
    pub status: String,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalDevnetRecord {
    pub id: String,
    pub label: String,
    pub workspace: String,
    pub manifest_path: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub nodes: Vec<LocalNodeConfigRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalDevnetListReport {
    pub profile: String,
    pub active_devnet: Option<String>,
    pub workspace_root: String,
    pub devnets: Vec<LocalDevnetRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalNodeTools {
    pub logoscore: ToolStatus,
    pub lgpm: ToolStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolStatus {
    pub available: bool,
    pub command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LocalNodeConfigRecord {
    pub kind: NodeKind,
    pub config_path: String,
    pub data_dir: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub module_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub process_id: Option<u32>,
    #[serde(default)]
    pub installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalNodeCommandSpec {
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

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalNodesState {
    version: u32,
    active_devnet: Option<String>,
    managed_workspace_root: String,
    devnets: Vec<LocalDevnetRecord>,
    operations: Vec<LocalNodeOperationReport>,
}

impl LocalNodesState {
    fn default_for_config_dir(config: &Path) -> Self {
        Self {
            version: 1,
            active_devnet: None,
            managed_workspace_root: config.join("local-nodes").display().to_string(),
            devnets: Vec::new(),
            operations: Vec::new(),
        }
    }

    fn active_devnet(&self) -> Option<&LocalDevnetRecord> {
        let active = self.active_devnet.as_deref()?;
        self.devnets.iter().find(|record| record.id == active)
    }

    fn active_devnet_mut(&mut self) -> Option<&mut LocalDevnetRecord> {
        let active = self.active_devnet.as_deref()?;
        self.devnets.iter_mut().find(|record| record.id == active)
    }

    fn devnet_mut(&mut self, network_id: &str) -> Option<&mut LocalDevnetRecord> {
        self.devnets
            .iter_mut()
            .find(|record| record.id == network_id)
    }

    fn push_operation(&mut self, operation: LocalNodeOperationReport) {
        self.operations.push(operation);
        if self.operations.len() <= HISTORY_LIMIT {
            return;
        }
        let keep_from = self.operations.len().saturating_sub(HISTORY_LIMIT);
        self.operations.drain(0..keep_from);
    }
}

pub fn local_nodes_status(profile: &str) -> Result<LocalNodeReport> {
    let state = load_state()?;
    Ok(report_for_state(profile, &state))
}

pub fn local_devnet_list(profile: &str) -> Result<LocalDevnetListReport> {
    let state = load_state()?;
    Ok(LocalDevnetListReport {
        profile: normalized_profile(profile).to_owned(),
        active_devnet: state.active_devnet.clone(),
        workspace_root: state.managed_workspace_root.clone(),
        devnets: state.devnets.clone(),
    })
}

pub fn local_nodes_action(
    profile: &str,
    request: LocalNodeActionRequest,
    confirmation: Option<&str>,
) -> Result<LocalNodeReport> {
    if confirmation != Some(CONFIRMATION_TOKEN) {
        bail!("local node action requires explicit confirmation");
    }

    let normalized_profile = normalized_profile(profile);
    let local_mode = normalized_profile == "local";
    if !action_allowed(
        normalized_profile,
        request.action,
        request.node,
        state_has_active()?,
    ) {
        bail!(
            "{} is not available for profile `{normalized_profile}`",
            request.action.label()
        );
    }

    if request.action.is_network_action() && !local_mode {
        bail!("local network actions require the local network profile");
    }

    let mut state = load_state()?;
    let operation = match request.action {
        NodeAction::NewNetwork => new_network(&mut state, &request),
        NodeAction::LoadNetwork => load_network(&mut state, &request),
        NodeAction::DeleteNetwork => delete_network(&mut state, &request),
        NodeAction::ResetNetwork => reset_network(&mut state, &request),
        NodeAction::Install => node_install(&mut state, normalized_profile, &request),
        NodeAction::Uninstall => node_uninstall(&mut state, normalized_profile, &request),
        NodeAction::Start => node_start(&mut state, normalized_profile, &request),
        NodeAction::Stop => node_stop(&mut state, normalized_profile, &request),
        NodeAction::Purge => node_purge(&mut state, normalized_profile, &request),
    };
    state.push_operation(operation);
    save_state(&state)?;
    Ok(report_for_state(profile, &state))
}

#[must_use]
pub fn node_set_for_profile(profile: &str) -> Vec<NodeKind> {
    if normalized_profile(profile) == "local" {
        vec![
            NodeKind::Bedrock,
            NodeKind::Sequencer,
            NodeKind::Indexer,
            NodeKind::Storage,
            NodeKind::Messaging,
        ]
    } else {
        vec![
            NodeKind::Bedrock,
            NodeKind::Indexer,
            NodeKind::Storage,
            NodeKind::Messaging,
        ]
    }
}

#[must_use]
pub fn available_actions_for(
    profile: &str,
    node: Option<NodeKind>,
    has_active_devnet: bool,
) -> Vec<NodeAction> {
    let local_mode = normalized_profile(profile) == "local";
    if node.is_none() {
        if local_mode {
            let mut actions = vec![NodeAction::NewNetwork, NodeAction::LoadNetwork];
            if has_active_devnet {
                actions.extend([NodeAction::ResetNetwork, NodeAction::DeleteNetwork]);
            }
            return actions;
        }
        return Vec::new();
    }

    let Some(kind) = node else {
        return Vec::new();
    };
    if !node_set_for_profile(profile).contains(&kind) {
        return Vec::new();
    }

    if local_mode && !has_active_devnet {
        return vec![NodeAction::Install];
    }

    let mut actions = vec![
        NodeAction::Install,
        NodeAction::Start,
        NodeAction::Stop,
        NodeAction::Uninstall,
    ];
    if local_mode {
        actions.push(NodeAction::Purge);
    }
    actions
}

#[must_use]
pub fn command_spec_for(
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

#[must_use]
pub fn path_is_inside(parent: &Path, child: &Path) -> bool {
    match (
        normalized_absolute_path(parent),
        normalized_absolute_path(child),
    ) {
        (Ok(parent), Ok(child)) => child != parent && child.starts_with(parent),
        _ => false,
    }
}

fn state_has_active() -> Result<bool> {
    Ok(load_state()?.active_devnet.is_some())
}

fn report_for_state(profile: &str, state: &LocalNodesState) -> LocalNodeReport {
    let profile = normalized_profile(profile);
    let active = state.active_devnet();
    let tools = tool_statuses();
    let nodes = node_set_for_profile(profile)
        .into_iter()
        .map(|kind| node_status(profile, state, active, &tools, kind))
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
        mode: if profile == "local" {
            "localnet".to_owned()
        } else {
            "public_testnet".to_owned()
        },
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
    profile: &str,
    state: &LocalNodesState,
    active: Option<&LocalDevnetRecord>,
    tools: &LocalNodeTools,
    kind: NodeKind,
) -> LocalNodeStatus {
    let config = active.and_then(|devnet| node_config(devnet, kind));
    let process_id = config.and_then(|node| node.process_id);
    let process_running = process_id.is_some_and(process_is_alive);
    let installed =
        config.is_some_and(|node| node.installed) || tool_backing_available(tools, kind);
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
        available_actions: available_actions_for(profile, Some(kind), active.is_some()),
        detail: node_status_detail(kind, install_state, run_state, tools),
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

fn action_allowed(
    profile: &str,
    action: NodeAction,
    node: Option<NodeKind>,
    has_active_devnet: bool,
) -> bool {
    available_actions_for(profile, node, has_active_devnet).contains(&action)
}

fn normalized_profile(profile: &str) -> &str {
    match profile.trim().to_ascii_lowercase().as_str() {
        "local" | "localnet" | "devnet" => "local",
        _ => "default",
    }
}

fn new_network(
    state: &mut LocalNodesState,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
        let id = request
            .network_id
            .as_deref()
            .map(sanitize_network_id)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("devnet-{}", now_millis()));
        if state.devnets.iter().any(|record| record.id == id) {
            bail!("local network `{id}` already exists");
        }

        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let workspace = workspace_root.join(&id);
        fs::create_dir_all(&workspace)
            .with_context(|| format!("failed to create workspace {}", workspace.display()))?;
        let now = now_millis();
        let record = LocalDevnetRecord {
            id: id.clone(),
            label: request
                .label
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| id.clone()),
            workspace: workspace.display().to_string(),
            manifest_path: workspace.join(MANIFEST_FILE).display().to_string(),
            created_at: now,
            updated_at: now,
            nodes: node_set_for_profile("local")
                .into_iter()
                .map(|kind| default_node_config(&workspace, kind))
                .collect(),
        };
        generate_devnet_files(&record)?;
        write_devnet_manifest(&record)?;
        state.active_devnet = Some(record.id.clone());
        state.devnets.push(record);
        Ok(OperationOutcome {
            status: "created".to_owned(),
            detail: format!("created local network `{id}`"),
            command: None,
        })
    })
}

fn load_network(
    state: &mut LocalNodesState,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
        let workspace = request
            .workspace_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .context("workspace path is required")?;
        let manifest_path = Path::new(workspace).join(MANIFEST_FILE);
        let text = fs::read_to_string(&manifest_path)
            .with_context(|| format!("failed to read {}", manifest_path.display()))?;
        let mut record: LocalDevnetRecord = serde_json::from_str(&text)
            .with_context(|| format!("failed to parse {}", manifest_path.display()))?;
        record.workspace = Path::new(workspace).display().to_string();
        record.manifest_path = manifest_path.display().to_string();
        record.updated_at = now_millis();
        if let Some(existing) = state.devnet_mut(&record.id) {
            *existing = record.clone();
        } else {
            state.devnets.push(record.clone());
        }
        state.active_devnet = Some(record.id.clone());
        Ok(OperationOutcome {
            status: "loaded".to_owned(),
            detail: format!("loaded local network `{}`", record.id),
            command: None,
        })
    })
}

fn delete_network(
    state: &mut LocalNodesState,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
        let network_id = target_network_id(state, request)?;
        stop_all_owned_processes(state, &network_id);
        let Some(position) = state
            .devnets
            .iter()
            .position(|record| record.id == network_id)
        else {
            bail!("local network `{network_id}` was not found");
        };
        let record = state.devnets.remove(position);
        remove_dir_inside(
            Path::new(&state.managed_workspace_root),
            Path::new(&record.workspace),
        )?;
        if state.active_devnet.as_deref() == Some(&network_id) {
            state.active_devnet = None;
        }
        Ok(OperationOutcome {
            status: "deleted".to_owned(),
            detail: format!("deleted local network `{network_id}`"),
            command: None,
        })
    })
}

fn reset_network(
    state: &mut LocalNodesState,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
        let network_id = target_network_id(state, request)?;
        stop_all_owned_processes(state, &network_id);
        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let Some(record) = state.devnet_mut(&network_id) else {
            bail!("local network `{network_id}` was not found");
        };
        let workspace = PathBuf::from(&record.workspace);
        for node in &mut record.nodes {
            remove_dir_inside(&workspace_root, Path::new(&node.data_dir))?;
            node.process_id = None;
            fs::create_dir_all(&node.data_dir)
                .with_context(|| format!("failed to recreate {}", node.data_dir))?;
        }
        record.updated_at = now_millis();
        generate_devnet_files(record)?;
        write_devnet_manifest(record)?;
        if !path_is_inside(&workspace_root, &workspace) {
            bail!("workspace is outside managed local node root");
        }
        Ok(OperationOutcome {
            status: "reset".to_owned(),
            detail: format!("reset local network `{network_id}`"),
            command: None,
        })
    })
}

fn node_install(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        if kind == NodeKind::Sequencer {
            let Some(binary) = find_command("sequencer_service") else {
                return Ok(OperationOutcome {
                    status: "needs_configuration".to_owned(),
                    detail: "sequencer_service not found".to_owned(),
                    command: None,
                });
            };
            if let Some(record) = state.active_devnet_mut()
                && let Some(config) = node_config_mut(record, kind)
            {
                config.package_path = Some(binary);
                config.installed = true;
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
            }
            return Ok(OperationOutcome {
                status: "installed".to_owned(),
                detail: "sequencer_service registered".to_owned(),
                command: None,
            });
        }
        if !tool_statuses().logoscore.available {
            return Ok(OperationOutcome {
                status: "needs_configuration".to_owned(),
                detail: "logoscore not found".to_owned(),
                command: None,
            });
        }
        if let Some(record) = state.active_devnet_mut()
            && let Some(config) = node_config_mut(record, kind)
        {
            if profile == "local"
                && let Some(spec) = command_spec_for(
                    kind,
                    NodeAction::Install,
                    &config.config_path,
                    DEFAULT_DEPLOYMENT,
                )
            {
                match execute_command_spec(&spec) {
                    Ok(value) => {
                        config.installed = true;
                        config.package_path = Some(spec.program.clone());
                        record.updated_at = now_millis();
                        write_devnet_manifest(record)?;
                        return Ok(OperationOutcome {
                            status: "installed".to_owned(),
                            detail: operation_detail_from_value(&value),
                            command: Some(spec.display),
                        });
                    }
                    Err(error) => {
                        return Ok(OperationOutcome {
                            status: "failed".to_owned(),
                            detail: error.to_string(),
                            command: Some(spec.display),
                        });
                    }
                }
            }
            config.installed = true;
            config.package_path = Some("logoscore".to_owned());
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
        }
        Ok(OperationOutcome {
            status: "installed".to_owned(),
            detail: "logoscore module available".to_owned(),
            command: None,
        })
    })
}

fn node_uninstall(
    state: &mut LocalNodesState,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let mut command = None;
        let mut detail = "node registration removed".to_owned();
        if let Some(record) = state.active_devnet_mut()
            && let Some(config) = node_config_mut(record, kind)
        {
            stop_owned_process(config);
            if let Some(spec) = command_spec_for(
                kind,
                NodeAction::Uninstall,
                &config.config_path,
                DEFAULT_DEPLOYMENT,
            ) {
                command = Some(spec.display.clone());
                if let Err(error) = execute_command_spec(&spec) {
                    detail = error.to_string();
                }
            }
            config.installed = false;
            config.package_path = None;
            config.module_path = None;
            config.process_id = None;
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
        }
        Ok(OperationOutcome {
            status: if detail == "node registration removed" {
                "uninstalled"
            } else {
                "failed"
            }
            .to_owned(),
            detail,
            command,
        })
    })
}

fn node_start(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let Some(record) = state.active_devnet_mut() else {
            if profile == "local" {
                bail!("active local network is required");
            }
            return start_external_node(kind);
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", kind.label());
        };
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to create {}", config.data_dir))?;
        let spec = command_spec_for(
            kind,
            NodeAction::Start,
            &config.config_path,
            DEFAULT_DEPLOYMENT,
        )
        .with_context(|| format!("{} start is not implemented", kind.label()))?;
        match execute_command_spec(&spec) {
            Ok(value) => {
                if kind == NodeKind::Sequencer {
                    config.process_id = value
                        .get("pid")
                        .and_then(Value::as_u64)
                        .and_then(|pid| u32::try_from(pid).ok());
                }
                config.installed = true;
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "started".to_owned(),
                    detail: operation_detail_from_value(&value),
                    command: Some(spec.display),
                })
            }
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: error.to_string(),
                command: Some(spec.display),
            }),
        }
    })
}

fn start_external_node(kind: NodeKind) -> Result<OperationOutcome> {
    if !tool_statuses().logoscore.available {
        return Ok(OperationOutcome {
            status: "needs_configuration".to_owned(),
            detail: "logoscore not found".to_owned(),
            command: None,
        });
    }
    let config = "";
    let spec = command_spec_for(kind, NodeAction::Start, config, DEFAULT_DEPLOYMENT)
        .with_context(|| format!("{} start is not implemented", kind.label()))?;
    match execute_command_spec(&spec) {
        Ok(value) => Ok(OperationOutcome {
            status: "started".to_owned(),
            detail: operation_detail_from_value(&value),
            command: Some(spec.display),
        }),
        Err(error) => Ok(OperationOutcome {
            status: "failed".to_owned(),
            detail: error.to_string(),
            command: Some(spec.display),
        }),
    }
}

fn node_stop(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let Some(record) = state.active_devnet_mut() else {
            if profile == "local" {
                bail!("active local network is required");
            }
            return stop_external_node(kind);
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", kind.label());
        };
        let mut command = None;
        if kind == NodeKind::Sequencer {
            stop_owned_process(config);
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: "stopped recorded sequencer process".to_owned(),
                command,
            });
        }
        if let Some(spec) = command_spec_for(
            kind,
            NodeAction::Stop,
            &config.config_path,
            DEFAULT_DEPLOYMENT,
        ) {
            command = Some(spec.display.clone());
            match execute_command_spec(&spec) {
                Ok(value) => {
                    config.process_id = None;
                    record.updated_at = now_millis();
                    write_devnet_manifest(record)?;
                    return Ok(OperationOutcome {
                        status: "stopped".to_owned(),
                        detail: operation_detail_from_value(&value),
                        command,
                    });
                }
                Err(error) => {
                    return Ok(OperationOutcome {
                        status: "failed".to_owned(),
                        detail: error.to_string(),
                        command,
                    });
                }
            }
        }
        Ok(OperationOutcome {
            status: "stopped".to_owned(),
            detail: "no stop adapter configured".to_owned(),
            command,
        })
    })
}

fn stop_external_node(kind: NodeKind) -> Result<OperationOutcome> {
    if !tool_statuses().logoscore.available {
        return Ok(OperationOutcome {
            status: "needs_configuration".to_owned(),
            detail: "logoscore not found".to_owned(),
            command: None,
        });
    }
    let spec = command_spec_for(kind, NodeAction::Stop, "", DEFAULT_DEPLOYMENT)
        .with_context(|| format!("{} stop is not implemented", kind.label()))?;
    match execute_command_spec(&spec) {
        Ok(value) => Ok(OperationOutcome {
            status: "stopped".to_owned(),
            detail: operation_detail_from_value(&value),
            command: Some(spec.display),
        }),
        Err(error) => Ok(OperationOutcome {
            status: "failed".to_owned(),
            detail: error.to_string(),
            command: Some(spec.display),
        }),
    }
}

fn node_purge(
    state: &mut LocalNodesState,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let Some(record) = state.active_devnet_mut() else {
            bail!("active local network is required");
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", kind.label());
        };
        stop_owned_process(config);
        remove_dir_inside(&workspace_root, Path::new(&config.data_dir))?;
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to recreate {}", config.data_dir))?;
        config.process_id = None;
        record.updated_at = now_millis();
        write_devnet_manifest(record)?;
        Ok(OperationOutcome {
            status: "purged".to_owned(),
            detail: format!("purged {} data directory", kind.label()),
            command: None,
        })
    })
}

struct OperationOutcome {
    status: String,
    detail: String,
    command: Option<String>,
}

fn operation_result(
    request: &LocalNodeActionRequest,
    node: Option<NodeKind>,
    operation: impl FnOnce() -> Result<OperationOutcome>,
) -> LocalNodeOperationReport {
    let timestamp = now_millis();
    match operation() {
        Ok(outcome) => LocalNodeOperationReport {
            id: format!("op-{timestamp}"),
            time: timestamp.to_string(),
            timestamp_millis: timestamp,
            action: request.action,
            node,
            network_id: request.network_id.clone(),
            status: outcome.status,
            detail: outcome.detail,
            command: outcome.command,
        },
        Err(error) => LocalNodeOperationReport {
            id: format!("op-{timestamp}"),
            time: timestamp.to_string(),
            timestamp_millis: timestamp,
            action: request.action,
            node,
            network_id: request.network_id.clone(),
            status: "failed".to_owned(),
            detail: error.to_string(),
            command: None,
        },
    }
}

fn required_node(request: &LocalNodeActionRequest) -> Result<NodeKind> {
    request.node.context("node kind is required")
}

fn target_network_id(state: &LocalNodesState, request: &LocalNodeActionRequest) -> Result<String> {
    request
        .network_id
        .clone()
        .or_else(|| state.active_devnet.clone())
        .context("local network id is required")
}

fn default_node_config(workspace: &Path, kind: NodeKind) -> LocalNodeConfigRecord {
    let port = kind.default_port();
    LocalNodeConfigRecord {
        kind,
        config_path: workspace
            .join("configs")
            .join(format!("{}.json", kind.as_str()))
            .display()
            .to_string(),
        data_dir: workspace
            .join("data")
            .join(kind.as_str())
            .display()
            .to_string(),
        endpoint: kind.endpoint(port),
        port,
        package_path: None,
        module_path: None,
        process_id: None,
        installed: false,
    }
}

fn generate_devnet_files(record: &LocalDevnetRecord) -> Result<()> {
    for node in &record.nodes {
        fs::create_dir_all(&node.data_dir)
            .with_context(|| format!("failed to create {}", node.data_dir))?;
        let config_path = PathBuf::from(&node.config_path);
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        let value = json!({
            "network_id": record.id,
            "node": node.kind.as_str(),
            "data_dir": node.data_dir,
            "endpoint": node.endpoint,
            "port": node.port,
        });
        let text = serde_json::to_string_pretty(&value)
            .context("failed to serialize local node config")?;
        fs::write(&node.config_path, text)
            .with_context(|| format!("failed to write {}", node.config_path))?;
    }
    Ok(())
}

fn write_devnet_manifest(record: &LocalDevnetRecord) -> Result<()> {
    let path = PathBuf::from(&record.manifest_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(record)
        .context("failed to serialize local network manifest")?;
    fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
}

fn node_config(record: &LocalDevnetRecord, kind: NodeKind) -> Option<&LocalNodeConfigRecord> {
    record.nodes.iter().find(|node| node.kind == kind)
}

fn node_config_mut(
    record: &mut LocalDevnetRecord,
    kind: NodeKind,
) -> Option<&mut LocalNodeConfigRecord> {
    record.nodes.iter_mut().find(|node| node.kind == kind)
}

fn stop_all_owned_processes(state: &mut LocalNodesState, network_id: &str) {
    let Some(record) = state.devnet_mut(network_id) else {
        return;
    };
    for node in &mut record.nodes {
        stop_owned_process(node);
    }
}

fn stop_owned_process(node: &mut LocalNodeConfigRecord) {
    let Some(pid) = node.process_id else {
        return;
    };
    if process_is_alive(pid) {
        let _ignored = stop_process(pid);
    }
    node.process_id = None;
}

fn execute_command_spec(spec: &LocalNodeCommandSpec) -> Result<Value> {
    match &spec.backend {
        CommandBackend::LogosCore {
            module,
            method,
            call_args,
        } => {
            let output = logoscore::call(module, method, call_args)?;
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
            command
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            #[cfg(unix)]
            {
                use std::os::unix::process::CommandExt as _;
                command.process_group(0);
            }
            let child = command
                .spawn()
                .with_context(|| format!("failed to start {}", spec.display))?;
            Ok(json!({
                "pid": child.id(),
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

fn operation_detail_from_value(value: &Value) -> String {
    value
        .get("value")
        .and_then(|value| value.get("status").or_else(|| value.get("result")))
        .map(Value::to_string)
        .unwrap_or_else(|| "completed".to_owned())
}

fn tool_backing_available(tools: &LocalNodeTools, kind: NodeKind) -> bool {
    match kind {
        NodeKind::Sequencer => find_command("sequencer_service").is_some(),
        _ => tools.logoscore.available,
    }
}

fn tool_statuses() -> LocalNodeTools {
    LocalNodeTools {
        logoscore: tool_status("logoscore"),
        lgpm: tool_status("lgpm"),
    }
}

fn tool_status(command: &str) -> ToolStatus {
    ToolStatus {
        available: find_command(command).is_some(),
        command: command.to_owned(),
        path: find_command(command),
    }
}

fn find_command(command: &str) -> Option<String> {
    if command.contains(std::path::MAIN_SEPARATOR) {
        let path = Path::new(command);
        return path.is_file().then(|| path.display().to_string());
    }
    let path_var = env::var_os("PATH")?;
    env::split_paths(&path_var)
        .map(|path| path.join(command))
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
}

fn load_state() -> Result<LocalNodesState> {
    let config = config_dir()?;
    let path = state_path_for_config(&config);
    if !path.is_file() {
        return Ok(LocalNodesState::default_for_config_dir(&config));
    }
    let text = fs::read_to_string(&path)
        .with_context(|| format!("failed to read local node state from {}", path.display()))?;
    let mut state: LocalNodesState = serde_json::from_str(&text)
        .with_context(|| format!("failed to parse local node state from {}", path.display()))?;
    if state.managed_workspace_root.trim().is_empty() {
        state.managed_workspace_root = config.join("local-nodes").display().to_string();
    }
    if state.version == 0 {
        state.version = 1;
    }
    Ok(state)
}

fn save_state(state: &LocalNodesState) -> Result<()> {
    let path = state_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    }
    let text =
        serde_json::to_string_pretty(state).context("failed to serialize local node state")?;
    fs::write(&path, text)
        .with_context(|| format!("failed to write local node state to {}", path.display()))?;
    Ok(())
}

fn state_path() -> Result<PathBuf> {
    Ok(state_path_for_config(&config_dir()?))
}

fn state_path_for_config(config: &Path) -> PathBuf {
    config.join(STATE_FILE)
}

fn sanitize_network_id(value: &str) -> String {
    value
        .trim()
        .chars()
        .filter_map(|ch| {
            if ch.is_ascii_alphanumeric() {
                Some(ch.to_ascii_lowercase())
            } else if ch == '-' || ch == '_' {
                Some(ch)
            } else if ch.is_ascii_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

fn remove_dir_inside(root: &Path, target: &Path) -> Result<()> {
    if !path_is_inside(root, target) {
        bail!(
            "refusing to remove {} because it is outside managed workspace {}",
            target.display(),
            root.display()
        );
    }
    if target.exists() {
        fs::remove_dir_all(target)
            .with_context(|| format!("failed to remove {}", target.display()))?;
    }
    Ok(())
}

fn normalized_absolute_path(path: &Path) -> Result<PathBuf> {
    let mut normalized = if path.is_absolute() {
        PathBuf::new()
    } else {
        env::current_dir().context("failed to read current directory")?
    };
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    bail!("path {} escapes filesystem root", path.display());
                }
            }
            Component::Normal(value) => normalized.push(value),
        }
    }
    Ok(normalized)
}

fn process_is_alive(pid: u32) -> bool {
    Command::new("kill")
        .arg("-0")
        .arg(pid.to_string())
        .status()
        .is_ok_and(|status| status.success())
}

fn stop_process(pid: u32) -> Result<()> {
    #[cfg(unix)]
    let target = format!("-{pid}");
    #[cfg(not(unix))]
    let target = pid.to_string();
    let status = Command::new("kill")
        .arg("-TERM")
        .arg(target)
        .status()
        .with_context(|| format!("failed to stop process {pid}"))?;
    if !status.success() {
        bail!("process {pid} stop exited with {status}");
    }
    Ok(())
}

fn now_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_profile_includes_sequencer_and_network_actions() {
        let nodes = node_set_for_profile("local");

        assert!(nodes.contains(&NodeKind::Sequencer));
        assert!(available_actions_for("local", None, false).contains(&NodeAction::NewNetwork));
        assert!(!available_actions_for("default", None, false).contains(&NodeAction::NewNetwork));
    }

    #[test]
    fn testnet_profile_excludes_local_sequencer_and_purge() {
        let nodes = node_set_for_profile("default");
        let actions = available_actions_for("default", Some(NodeKind::Bedrock), true);

        assert!(!nodes.contains(&NodeKind::Sequencer));
        assert!(!actions.contains(&NodeAction::Purge));
    }

    #[test]
    fn command_specs_match_module_adapters() -> Result<()> {
        let bedrock = command_spec_for(
            NodeKind::Bedrock,
            NodeAction::Start,
            "/tmp/bedrock.json",
            "local",
        )
        .context("missing bedrock command")?;
        let expected_bedrock = vec![
            "call",
            "blockchain_module",
            "start",
            "/tmp/bedrock.json",
            "local",
            "--json",
        ];
        if bedrock.args != expected_bedrock {
            bail!("unexpected bedrock command: {:?}", bedrock.args);
        }

        let indexer = command_spec_for(
            NodeKind::Indexer,
            NodeAction::Start,
            "/tmp/indexer.json",
            "local",
        )
        .context("missing indexer command")?;
        let expected_indexer = vec![
            "call",
            "lez_indexer_module",
            "start_indexer",
            "/tmp/indexer.json",
            "--json",
        ];
        if indexer.args != expected_indexer {
            bail!("unexpected indexer command: {:?}", indexer.args);
        }

        let messaging = command_spec_for(
            NodeKind::Messaging,
            NodeAction::Install,
            "/tmp/delivery.json",
            "local",
        )
        .context("missing messaging command")?;
        let expected_messaging = vec![
            "call",
            "delivery_module",
            "createNode",
            "/tmp/delivery.json",
            "--json",
        ];
        if messaging.args != expected_messaging {
            bail!("unexpected messaging command: {:?}", messaging.args);
        }
        Ok(())
    }

    #[test]
    fn state_serialization_round_trips() -> Result<()> {
        let config = env::temp_dir().join(format!(
            "logos-inspector-local-nodes-state-{}",
            now_millis()
        ));
        let state = LocalNodesState::default_for_config_dir(&config);

        let text = serde_json::to_string(&state)?;
        let parsed: LocalNodesState = serde_json::from_str(&text)?;

        if parsed.version != 1 {
            bail!("unexpected state version");
        }
        if !parsed.managed_workspace_root.ends_with("local-nodes") {
            bail!("managed workspace root was not migrated");
        }
        Ok(())
    }

    #[test]
    fn path_safety_rejects_sibling_and_parent_escape() {
        let root = Path::new("/tmp/logos-inspector/root");

        assert!(path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root/devnet/data")
        ));
        assert!(!path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root")
        ));
        assert!(!path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root/../other")
        ));
        assert!(!path_is_inside(
            root,
            Path::new("/tmp/logos-inspector/root2/data")
        ));
    }
}
