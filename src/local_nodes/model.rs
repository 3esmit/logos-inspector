use std::path::Path;

use serde::{Deserialize, Serialize};

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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeAction {
    StartRuntime,
    StopRuntime,
    Install,
    Initialize,
    Uninstall,
    NewNetwork,
    LoadNetwork,
    DeleteNetwork,
    ResetNetwork,
    Start,
    Stop,
    Purge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LocalNodeProblemCode {
    MissingLogoscore,
    MissingSequencerBinary,
}

impl NodeAction {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::StartRuntime => "start_runtime",
            Self::StopRuntime => "stop_runtime",
            Self::Install => "install",
            Self::Initialize => "initialize",
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

    pub(super) fn label(self) -> &'static str {
        match self {
            Self::StartRuntime => "Start Local Runtime",
            Self::StopRuntime => "Stop Local Runtime",
            Self::Install => "Install",
            Self::Initialize => "Initialize",
            Self::Uninstall => "Uninstall",
            Self::NewNetwork => "New Local Devnet",
            Self::LoadNetwork => "Load Local Devnet",
            Self::DeleteNetwork => "Delete Local Devnet",
            Self::ResetNetwork => "Reset Local Devnet",
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Purge => "Purge",
        }
    }

    pub(super) fn is_network_action(self) -> bool {
        matches!(
            self,
            Self::NewNetwork | Self::LoadNetwork | Self::DeleteNetwork | Self::ResetNetwork
        )
    }

    #[must_use]
    pub(super) fn is_runtime_action(self) -> bool {
        matches!(self, Self::StartRuntime | Self::StopRuntime)
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
    pub runtime_modules_dir: Option<String>,
    #[serde(default)]
    pub runtime_binary_path: Option<String>,
    #[serde(default)]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalNodeReport {
    pub profile: String,
    pub mode: String,
    pub available_network_actions: Vec<NodeAction>,
    pub available_runtime_actions: Vec<NodeAction>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary_problem: Option<LocalNodeProblemCode>,
    pub active_devnet: Option<String>,
    pub workspace_root: String,
    pub summary: LocalNodeSummary,
    pub nodes: Vec<LocalNodeStatus>,
    pub operations: Vec<LocalNodeOperationReport>,
    pub tools: LocalNodeTools,
    pub runtime: super::runtime::LogoscoreRuntimeStatus,
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
    #[serde(default)]
    pub lifecycle_state: NodeLifecycleState,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pending_lifecycle_action: Option<NodeAction>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeLifecycleState {
    #[default]
    NotInitialized,
    Initializing,
    Starting,
    Running,
    Stopping,
    Stopped,
    Unknown,
    Failed,
}

impl NodeLifecycleState {
    #[must_use]
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::NotInitialized => "not_initialized",
            Self::Initializing => "initializing",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Stopped => "stopped",
            Self::Unknown => "unknown",
            Self::Failed => "failed",
        }
    }

    #[must_use]
    pub(super) fn is_pending(self) -> bool {
        matches!(self, Self::Initializing | Self::Starting | Self::Stopping)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct LocalNodesState {
    pub(super) version: u32,
    pub(super) active_devnet: Option<String>,
    pub(super) managed_workspace_root: String,
    pub(super) devnets: Vec<LocalDevnetRecord>,
    pub(super) operations: Vec<LocalNodeOperationReport>,
}

impl LocalNodesState {
    pub(super) fn default_for_config_dir(config: &Path) -> Self {
        Self {
            version: 1,
            active_devnet: None,
            managed_workspace_root: config.join("local-nodes").display().to_string(),
            devnets: Vec::new(),
            operations: Vec::new(),
        }
    }

    pub(super) fn active_devnet(&self) -> Option<&LocalDevnetRecord> {
        let active = self.active_devnet.as_deref()?;
        self.devnets.iter().find(|record| record.id == active)
    }

    pub(super) fn active_devnet_mut(&mut self) -> Option<&mut LocalDevnetRecord> {
        let active = self.active_devnet.as_deref()?;
        self.devnets.iter_mut().find(|record| record.id == active)
    }

    pub(super) fn devnet_mut(&mut self, network_id: &str) -> Option<&mut LocalDevnetRecord> {
        self.devnets
            .iter_mut()
            .find(|record| record.id == network_id)
    }

    pub(super) fn push_operation(&mut self, operation: LocalNodeOperationReport) {
        self.operations.push(operation);
        if self.operations.len() <= HISTORY_LIMIT {
            return;
        }
        let keep_from = self.operations.len().saturating_sub(HISTORY_LIMIT);
        self.operations.drain(0..keep_from);
    }
}
