use std::{
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};

use crate::inspection::NetworkScope;
use crate::support::command_runner::CommandControl;
use crate::support::{confirmation::ConfirmationPolicy, state_store::config_dir};

use super::action_workspace::{LocalNodeActionControls, LocalNodeActionWorkspace};
use super::adapters::{NodeStatusContext, adapter_for};
use super::config::{LocalNodeConfigSnapshot, LocalNodeConfigValidation};
use super::lifecycle::acquire_state_lock;
use super::model::{
    LOCAL_NODES_STATE_VERSION, LocalDevnetListReport, LocalDevnetRecord, LocalNodeActionRequest,
    LocalNodeConfigRecord, LocalNodeOperationReport, LocalNodeReport, LocalNodeStatus,
    LocalNodeSummary, LocalNodeTools, LocalNodesState, NodeAction, NodeKind, NodeLifecycleState,
    ToolStatus,
};
use super::presentation;
use super::process::{find_command, process_group_has_live_members};
use super::runtime::{self, LogoscoreRuntimeProfile, LogoscoreRuntimeStore};
use super::workflow::{LocalNodeWorkflow, normalized_profile};
use super::{ChannelIndexerActionRequest, LocalNodePackageCommit};

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
        let runtime = self.runtime_store.load_resolved()?;
        Ok(self.projector.report(profile, &state, runtime.as_ref()))
    }

    pub(super) fn channel_indexer_status(
        &self,
        profile: &str,
        network_scope: &NetworkScope,
        channel_id: &str,
    ) -> Result<LocalNodeReport> {
        let _state_lock = acquire_state_lock()?;
        let state = self.store.load()?;
        let runtime = self.runtime_store.load()?;
        super::channel_indexer::status(
            self.runtime_store.config_root(),
            profile,
            &state,
            runtime.as_ref(),
            self.projector,
            network_scope,
            channel_id,
        )
    }

    pub(super) fn runtime_profile(&self) -> Result<Option<LogoscoreRuntimeProfile>> {
        self.runtime_store.load_resolved()
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

    pub(super) fn config_snapshot(
        &self,
        profile: &str,
        node: NodeKind,
    ) -> Result<LocalNodeConfigSnapshot> {
        let _state_lock = acquire_state_lock()?;
        let state = self.store.load()?;
        let runtime = self.runtime_store.load_resolved()?;
        super::config::snapshot(&state, runtime.as_ref(), profile, node)
    }

    pub(super) fn config_validate(
        &self,
        profile: &str,
        node: NodeKind,
        text: &str,
    ) -> Result<LocalNodeConfigValidation> {
        let _state_lock = acquire_state_lock()?;
        let state = self.store.load()?;
        super::config::validate(&state, profile, node, text)
    }

    pub(super) fn save_config(
        &self,
        profile: &str,
        node: NodeKind,
        text: &str,
        expected_revision: &str,
        confirmation: Option<&str>,
    ) -> Result<LocalNodeConfigSnapshot> {
        ConfirmationPolicy::LocalNodeAction.require(confirmation)?;

        let _state_lock = acquire_state_lock()?;
        let mut state = self.store.load()?;
        let runtime = self.runtime_store.load_resolved()?;
        super::config::save(
            &mut state,
            runtime.as_ref(),
            profile,
            node,
            text,
            expected_revision,
            |updated_state| self.store.save(updated_state),
        )?;
        super::config::snapshot(&state, runtime.as_ref(), profile, node)
    }

    pub(super) fn apply(
        &self,
        profile: &str,
        request: LocalNodeActionRequest,
        confirmation: Option<&str>,
    ) -> Result<LocalNodeReport> {
        self.apply_inner(profile, request, confirmation, None, None)
    }

    pub(super) fn apply_controlled(
        &self,
        profile: &str,
        request: LocalNodeActionRequest,
        confirmation: Option<&str>,
        control: CommandControl,
        package_commit: LocalNodePackageCommit,
    ) -> Result<LocalNodeReport> {
        self.apply_inner(
            profile,
            request,
            confirmation,
            Some(control),
            Some(package_commit),
        )
    }

    pub(super) fn channel_indexer_action_controlled(
        &self,
        profile: &str,
        request: ChannelIndexerActionRequest,
        confirmation: Option<&str>,
        control: CommandControl,
    ) -> Result<LocalNodeReport> {
        ConfirmationPolicy::LocalNodeAction.require(confirmation)?;

        let _state_lock = acquire_state_lock()?;
        let state = self.store.load()?;
        let runtime = self.runtime_store.load()?;
        super::channel_indexer::apply(
            self.runtime_store.config_root(),
            profile,
            &state,
            runtime.as_ref(),
            self.projector,
            request,
            Some(&control),
        )
    }

    fn apply_inner(
        &self,
        profile: &str,
        request: LocalNodeActionRequest,
        confirmation: Option<&str>,
        control: Option<CommandControl>,
        mut package_commit: Option<LocalNodePackageCommit>,
    ) -> Result<LocalNodeReport> {
        ConfirmationPolicy::LocalNodeAction.require(confirmation)?;

        let _state_lock = acquire_state_lock()?;
        let mut state = self.store.load()?;
        let mut runtime = self.runtime_store.load_resolved()?;
        let workflow = LocalNodeWorkflow::for_state(profile, &state);
        workflow.validate_request(&request)?;

        let operation = self.workspace.apply_with_package_commit(
            &mut state,
            &mut runtime,
            self.runtime_store.config_root(),
            workflow.profile(),
            &request,
            LocalNodeActionControls {
                command: control.as_ref(),
                package_commit: package_commit.as_mut(),
            },
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
pub(super) struct LocalNodeReportProjector;

#[cfg(test)]
pub(super) fn report_for_state(profile: &str, state: &LocalNodesState) -> LocalNodeReport {
    LocalNodeReportProjector::system().report(profile, state, None)
}

impl LocalNodeReportProjector {
    pub(super) fn system() -> Self {
        Self
    }

    pub(super) fn report(
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
        let package_config =
            config.filter(|node| adapter.package_installation_matches_runtime(node, runtime));
        let last_action = last_operation_for(state, kind);
        LocalNodeStatus {
            kind,
            key: kind.as_str().to_owned(),
            label: adapter.label().to_owned(),
            install_state: status.install_state.to_owned(),
            run_state: status.run_state.to_owned(),
            ownership: if status.install_state == "installed" {
                "inspector_managed"
            } else {
                "external"
            }
            .to_owned(),
            endpoint: config
                .and_then(|node| node.endpoint.clone())
                .or_else(|| adapter.endpoint(adapter.default_port())),
            data_dir: config.map(|node| node.data_dir.clone()),
            config_path: config.map(|node| node.config_path.clone()),
            package_path: package_config.and_then(|node| node.package_path.clone()),
            package_version: package_config.and_then(|node| node.package_version.clone()),
            managed_channel_id: config
                .filter(|node| node.kind == NodeKind::Indexer)
                .and_then(|node| {
                    super::action_workspace::indexer_channel_from_config(&node.config_path)
                        .ok()
                        .flatten()
                }),
            indexer_state: config.and_then(|node| node.indexer_state.clone()),
            indexer_head: config.and_then(|node| node.indexer_head.clone()),
            indexer_error: config.and_then(|node| node.indexer_error.clone()),
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
            lgpd: self.tool_status("lgpd"),
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
        Some(runtime) if runtime.is_controllable() && runtime.is_running() => {
            vec![NodeAction::StopRuntime]
        }
        Some(runtime) if runtime.is_managed() => vec![NodeAction::StartRuntime],
        Some(runtime) if runtime.is_attached() && runtime.service_target().is_some() => {
            vec![NodeAction::StartRuntime]
        }
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
        if migrate_local_nodes_state(&mut state)? {
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
        let text =
            serde_json::to_string_pretty(state).context("failed to serialize local node state")?;
        atomic_write_local_node_state(&path, text.as_bytes())
    }

    fn state_path(&self) -> PathBuf {
        state_path_for_config(&self.config_dir)
    }
}

fn atomic_write_local_node_state(path: &Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .context("local node state path has no parent directory")?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create config directory {}", parent.display()))?;
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            bail!("local node state must be a regular file")
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error)
                .with_context(|| format!("failed to inspect local node state {}", path.display()));
        }
    }
    let mut staged = tempfile::Builder::new()
        .prefix(".local-nodes-")
        .suffix(".tmp")
        .tempfile_in(parent)
        .context("failed to stage local node state")?;
    staged
        .write_all(bytes)
        .context("failed to write staged local node state")?;
    staged
        .as_file_mut()
        .flush()
        .context("failed to flush staged local node state")?;
    staged
        .as_file()
        .sync_all()
        .context("failed to sync staged local node state")?;
    staged
        .persist(path)
        .map_err(|error| error.error)
        .with_context(|| format!("failed to replace local node state {}", path.display()))?;
    sync_local_node_state_directory(parent)
}

#[cfg(unix)]
fn sync_local_node_state_directory(path: &Path) -> Result<()> {
    fs::File::open(path)
        .context("failed to open local node state directory")?
        .sync_all()
        .context("failed to sync local node state directory")
}

#[cfg(not(unix))]
fn sync_local_node_state_directory(_path: &Path) -> Result<()> {
    Ok(())
}

fn migrate_local_nodes_state(state: &mut LocalNodesState) -> Result<bool> {
    if state.version >= LOCAL_NODES_STATE_VERSION {
        return Ok(false);
    }
    if let Some(record) = state.testnet.as_mut()
        && record
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Indexer)
    {
        normalize_indexer_module_record(record);
        super::action_workspace::write_devnet_manifest(record)?;
    }
    for record in &mut state.devnets {
        if record
            .nodes
            .iter()
            .any(|node| node.kind == NodeKind::Indexer)
        {
            normalize_indexer_module_record(record);
            super::action_workspace::write_devnet_manifest(record)?;
        }
    }
    let bound_indexer_topology = state
        .module_context_topology_by_kind
        .get(&NodeKind::Indexer)
        .cloned();
    if bound_indexer_topology
        .as_deref()
        .is_some_and(|topology_id| {
            !topology_has_package_managed_indexer_context(state, topology_id)
        })
    {
        state
            .module_context_topology_by_kind
            .remove(&NodeKind::Indexer);
    }
    state.version = LOCAL_NODES_STATE_VERSION;
    Ok(true)
}

pub(super) fn normalize_indexer_module_record(record: &mut LocalDevnetRecord) {
    for node in &mut record.nodes {
        if node.kind != NodeKind::Indexer {
            continue;
        }
        node.endpoint = None;
        node.port = None;
        node.process_id = None;
        if package_managed_indexer_record(node) {
            continue;
        }
        node.package_path = None;
        node.package_version = None;
        node.package_root_hash = None;
        node.module_path = None;
        node.indexer_state = None;
        node.indexer_head = None;
        node.indexer_error = None;
        node.installed = false;
        node.lifecycle_state = NodeLifecycleState::NotInitialized;
        node.pending_lifecycle_action = None;
    }
}

fn package_managed_indexer_record(node: &LocalNodeConfigRecord) -> bool {
    node.module_path.as_deref() == Some("lez_indexer_module")
        && node
            .package_path
            .as_deref()
            .and_then(super::package::package_path_modules_dir)
            .is_some()
        && nonempty(node.package_version.as_deref())
        && node.package_root_hash.as_deref().is_some_and(|root_hash| {
            root_hash.len() == 64 && root_hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        })
}

fn nonempty(value: Option<&str>) -> bool {
    value.is_some_and(|value| !value.trim().is_empty())
}

fn topology_has_package_managed_indexer_context(
    state: &LocalNodesState,
    topology_id: &str,
) -> bool {
    state
        .testnet
        .iter()
        .chain(state.devnets.iter())
        .find(|record| record.id == topology_id)
        .and_then(|record| {
            record
                .nodes
                .iter()
                .find(|node| node.kind == NodeKind::Indexer)
        })
        .is_some_and(|node| {
            package_managed_indexer_record(node) && node.lifecycle_state.has_module_context()
        })
}

fn state_path_for_config(config: &Path) -> PathBuf {
    config.join(STATE_FILE)
}

#[cfg(test)]
mod tests {
    use std::{collections::BTreeMap, fs};

    use anyhow::{Context as _, Result};

    use super::*;
    use crate::local_nodes::model::LocalNodeDeployment;
    use crate::local_nodes::runtime::{
        LogoscoreRuntimeOwnership, LogoscoreServiceScope, LogoscoreServiceTarget,
        LogoscoreTimeoutProfile,
    };

    fn attached_runtime(
        daemon_process_id: Option<u32>,
        service_target: Option<LogoscoreServiceTarget>,
    ) -> LogoscoreRuntimeProfile {
        LogoscoreRuntimeProfile {
            id: "local-attached".to_owned(),
            binary_path: "/bin/sh".to_owned(),
            config_dir: "/tmp/logoscore-client".to_owned(),
            modules_dir: None,
            persistence_path: None,
            ownership: LogoscoreRuntimeOwnership::LocalAttached,
            timeout_profile: LogoscoreTimeoutProfile::Probe,
            daemon_process_id,
            service_target,
        }
    }

    #[test]
    fn attached_runtime_actions_require_a_verified_service_target() {
        let target = LogoscoreServiceTarget {
            scope: LogoscoreServiceScope::System,
            unit: "logos-node.service".to_owned(),
        };
        let running = attached_runtime(Some(42), Some(target.clone()));
        assert_eq!(
            runtime_actions("default", Some(&running)),
            [NodeAction::StopRuntime]
        );

        let stopped = attached_runtime(None, Some(target));
        assert_eq!(
            runtime_actions("default", Some(&stopped)),
            [NodeAction::StartRuntime]
        );

        let read_only = attached_runtime(None, None);
        assert!(runtime_actions("default", Some(&read_only)).is_empty());
    }

    fn state_with_indexer(
        config_dir: &Path,
        config_path: &Path,
        manifest_path: &Path,
        node: LocalNodeConfigRecord,
    ) -> LocalNodesState {
        LocalNodesState {
            version: 3,
            active_devnet: None,
            module_context_topology_by_kind: BTreeMap::from([(
                NodeKind::Indexer,
                "logos-testnet".to_owned(),
            )]),
            testnet: Some(LocalDevnetRecord {
                deployment: LocalNodeDeployment::PublicTestnet,
                id: "logos-testnet".to_owned(),
                label: "Logos Testnet".to_owned(),
                workspace: config_path
                    .parent()
                    .and_then(Path::parent)
                    .unwrap_or(config_dir)
                    .display()
                    .to_string(),
                manifest_path: manifest_path.display().to_string(),
                created_at: 1,
                updated_at: 2,
                nodes: vec![node],
            }),
            managed_workspace_root: config_dir.join("local-nodes").display().to_string(),
            devnets: Vec::new(),
            operations: Vec::new(),
        }
    }

    fn indexer_node(config_path: &Path, data_dir: &Path) -> LocalNodeConfigRecord {
        LocalNodeConfigRecord {
            kind: NodeKind::Indexer,
            config_path: config_path.display().to_string(),
            initialization_config_path: None,
            data_dir: data_dir.display().to_string(),
            endpoint: Some("http://127.0.0.1:8779/".to_owned()),
            port: Some(8779),
            package_path: Some("/usr/local/bin/indexer_service".to_owned()),
            package_version: None,
            package_root_hash: None,
            indexer_state: None,
            indexer_head: None,
            indexer_error: None,
            module_path: None,
            process_id: Some(4242),
            installed: true,
            lifecycle_state: NodeLifecycleState::Running,
            pending_lifecycle_action: None,
        }
    }

    #[test]
    fn v4_migration_removes_process_era_indexer_registration_without_rewriting_config() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let workspace = directory.path().join("local-nodes/logos-testnet");
        let config_path = workspace.join("configs/indexer.json");
        let data_dir = workspace.join("data/indexer");
        let manifest_path = workspace.join("local-network.json");
        fs::create_dir_all(
            config_path
                .parent()
                .context("Indexer config has no parent")?,
        )?;
        let config = format!(
            "{{\"channel_id\":\"{}\",\"sentinel\":\"legacy\"}}",
            "01".repeat(32)
        );
        fs::write(&config_path, &config)?;
        let state = state_with_indexer(
            directory.path(),
            &config_path,
            &manifest_path,
            indexer_node(&config_path, &data_dir),
        );
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        store.save(&state)?;

        let migrated = store.load()?;

        let node = migrated
            .testnet
            .as_ref()
            .and_then(|record| record.nodes.first())
            .context("missing migrated Indexer")?;
        anyhow::ensure!(migrated.version == LOCAL_NODES_STATE_VERSION);
        anyhow::ensure!(
            node.endpoint.is_none()
                && node.port.is_none()
                && node.process_id.is_none()
                && node.package_path.is_none()
                && node.package_version.is_none()
                && node.package_root_hash.is_none()
                && node.module_path.is_none()
                && !node.installed
                && node.lifecycle_state == NodeLifecycleState::NotInitialized
                && node.pending_lifecycle_action.is_none()
        );
        anyhow::ensure!(
            !migrated
                .module_context_topology_by_kind
                .contains_key(&NodeKind::Indexer)
        );
        anyhow::ensure!(fs::read_to_string(&config_path)? == config);
        let manifest: LocalDevnetRecord =
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        let manifest_node = manifest
            .nodes
            .first()
            .context("missing migrated Indexer manifest record")?;
        anyhow::ensure!(
            !manifest_node.installed
                && manifest_node.endpoint.is_none()
                && manifest_node.port.is_none()
                && manifest_node.process_id.is_none()
                && manifest_node.package_path.is_none()
                && manifest_node.lifecycle_state == NodeLifecycleState::NotInitialized
        );
        Ok(())
    }

    #[test]
    fn v4_migration_preserves_package_managed_indexer_identity_and_channel_config() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let workspace = directory.path().join("local-nodes/logos-testnet");
        let config_path = workspace.join("configs/indexer.json");
        let data_dir = workspace.join("data/indexer");
        let manifest_path = workspace.join("local-network.json");
        let package_path = directory
            .path()
            .join("modules/lez_indexer_module/lez_indexer_module.so");
        fs::create_dir_all(
            config_path
                .parent()
                .context("Indexer config has no parent")?,
        )?;
        fs::create_dir_all(
            package_path
                .parent()
                .context("Indexer package has no parent")?,
        )?;
        let config = format!(
            "{{\"channel_id\":\"{}\",\"sentinel\":\"package\"}}",
            "02".repeat(32)
        );
        fs::write(&config_path, &config)?;
        fs::write(&package_path, b"module")?;
        let root_hash = "ab".repeat(32);
        let mut node = indexer_node(&config_path, &data_dir);
        node.package_path = Some(package_path.display().to_string());
        node.package_version = Some("0.2.0".to_owned());
        node.package_root_hash = Some(root_hash.clone());
        node.module_path = Some("lez_indexer_module".to_owned());
        node.indexer_state = Some("caught_up".to_owned());
        node.indexer_head = Some("691337".to_owned());
        node.lifecycle_state = NodeLifecycleState::Stopped;
        let state = state_with_indexer(directory.path(), &config_path, &manifest_path, node);
        let store = LocalNodeStore::for_config_dir(directory.path().to_path_buf());
        store.save(&state)?;

        let migrated = store.load()?;

        let node = migrated
            .testnet
            .as_ref()
            .and_then(|record| record.nodes.first())
            .context("missing migrated Indexer")?;
        anyhow::ensure!(migrated.version == LOCAL_NODES_STATE_VERSION);
        anyhow::ensure!(
            node.endpoint.is_none() && node.port.is_none() && node.process_id.is_none()
        );
        anyhow::ensure!(
            node.installed
                && node.package_path.as_deref() == Some(package_path.to_string_lossy().as_ref())
                && node.package_version.as_deref() == Some("0.2.0")
                && node.package_root_hash.as_deref() == Some(root_hash.as_str())
                && node.module_path.as_deref() == Some("lez_indexer_module")
                && node.indexer_state.as_deref() == Some("caught_up")
                && node.indexer_head.as_deref() == Some("691337")
                && node.lifecycle_state == NodeLifecycleState::Stopped
        );
        anyhow::ensure!(
            migrated
                .module_context_topology_by_kind
                .get(&NodeKind::Indexer)
                .map(String::as_str)
                == Some("logos-testnet")
        );
        anyhow::ensure!(fs::read_to_string(&config_path)? == config);
        let manifest: LocalDevnetRecord =
            serde_json::from_str(&fs::read_to_string(&manifest_path)?)?;
        let manifest_node = manifest
            .nodes
            .first()
            .context("missing package-managed Indexer manifest record")?;
        anyhow::ensure!(
            manifest_node.installed
                && manifest_node.endpoint.is_none()
                && manifest_node.port.is_none()
                && manifest_node.process_id.is_none()
                && manifest_node.package_path.as_deref()
                    == Some(package_path.to_string_lossy().as_ref())
                && manifest_node.module_path.as_deref() == Some("lez_indexer_module")
        );
        Ok(())
    }
}
