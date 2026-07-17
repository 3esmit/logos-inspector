use std::{
    fs,
    net::{Ipv4Addr, SocketAddr, TcpStream},
    path::{Path, PathBuf},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::modules::logos_core::{LogoscoreCliRuntime, normalize_module_call_value};
use crate::support::{
    command_runner::{CommandControl, CommandTerminated},
    time::now_millis,
};

use super::adapters::{NodeActionPolicy, NodeConfigContext, adapter_for};
use super::commands::{
    command_spec_for, ensure_module_loaded, execute_command_spec, execute_preflighted_command_spec,
    execute_ready_process_spec, operation_detail_from_value, preflight_command_spec,
    preflight_command_spec_once,
};
use super::lifecycle::reset_module_contexts;
use super::model::{
    LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord, LocalNodeDeployment,
    LocalNodeOperationReport, LocalNodesState, NodeAction, NodeKind, NodeLifecycleState,
};
use super::package::{
    canonical_modules_dir, download_official_indexer_module, install_official_indexer_module,
    installed_package_modules_dir, local_node_package_catalog, package_path_modules_dir,
};
use super::paths::{path_is_inside, remove_dir_inside};
use super::process::{find_command, process_group_has_live_members, spawn_detached, stop_process};
use super::runtime::LogoscoreRuntimeProfile;
use super::workflow::node_set_for_profile;
use super::{INDEXER_PACKAGE_INSTALL_TIMEOUT, LocalNodePackageCommit};

const MANIFEST_FILE: &str = "local-network.json";
const TESTNET_ID: &str = "logos-testnet";
const MESSAGING_CONTEXT_PROBE_ATTEMPTS: usize = 20;
const MESSAGING_CONTEXT_PROBE_INTERVAL: Duration = Duration::from_millis(250);
const MESSAGING_CONTEXT_RUNTIME_RESTARTS: usize = 1;
const INITIALIZATION_PREFLIGHT_RETRY_DELAY: Duration = Duration::from_millis(500);
const RUNTIME_PROCESS_GROUP_REAP_TIMEOUT: Duration = Duration::from_secs(5);
const RUNTIME_PROCESS_GROUP_REAP_POLL_INTERVAL: Duration = Duration::from_millis(25);
const MESSAGING_UNLOAD_CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(5);
const MESSAGING_UNLOAD_CONFIRMATION_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LocalNodeActionWorkspace;

pub(super) struct LocalNodeActionResult {
    pub(super) report: LocalNodeOperationReport,
    pub(super) interruption: Option<anyhow::Error>,
}

pub(super) struct LocalNodeActionControls<'a> {
    pub(super) command: Option<&'a CommandControl>,
    pub(super) package_commit: Option<&'a mut LocalNodePackageCommit>,
}

impl LocalNodeActionWorkspace {
    pub(super) fn system() -> Self {
        Self
    }

    #[cfg(test)]
    pub(super) fn apply(
        self,
        state: &mut LocalNodesState,
        runtime: &mut Option<LogoscoreRuntimeProfile>,
        runtime_config_root: &Path,
        normalized_profile: &str,
        request: &LocalNodeActionRequest,
        control: Option<&CommandControl>,
    ) -> LocalNodeActionResult {
        self.apply_with_package_commit(
            state,
            runtime,
            runtime_config_root,
            normalized_profile,
            request,
            LocalNodeActionControls {
                command: control,
                package_commit: None,
            },
        )
    }

    pub(super) fn apply_with_package_commit(
        self,
        state: &mut LocalNodesState,
        runtime: &mut Option<LogoscoreRuntimeProfile>,
        runtime_config_root: &Path,
        normalized_profile: &str,
        request: &LocalNodeActionRequest,
        controls: LocalNodeActionControls<'_>,
    ) -> LocalNodeActionResult {
        let target_network_id = request
            .action
            .is_network_action()
            .then(|| {
                request
                    .network_id
                    .clone()
                    .or_else(|| state.active_devnet.clone())
            })
            .flatten();
        let result = dispatch_action(
            state,
            runtime,
            runtime_config_root,
            normalized_profile,
            request,
            controls.command,
            controls.package_commit,
        );
        update_module_context_binding(
            state,
            normalized_profile,
            request,
            target_network_id.as_deref(),
            &result,
        );
        result
    }
}

fn update_module_context_binding(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
    target_network_id: Option<&str>,
    result: &LocalNodeActionResult,
) {
    let status = result.report.status.as_str();
    match request.action {
        NodeAction::Initialize if status == "initialized" => {
            if let Some(kind) = request.node
                && adapter_for(kind).managed_contract().is_some()
            {
                state.set_module_context_topology_for_profile(kind, profile);
            }
        }
        NodeAction::Start if status == "starting" && request.node == Some(NodeKind::Indexer) => {
            state.set_module_context_topology_for_profile(NodeKind::Indexer, profile);
        }
        NodeAction::Uninstall if status == "uninstalled" => {
            if let Some(kind) = request.node
                && adapter_for(kind).managed_contract().is_some()
            {
                state.clear_module_context_topology(kind);
            }
        }
        NodeAction::Stop if status == "stopped" && request.node == Some(NodeKind::Messaging) => {
            state.clear_module_context_topology(NodeKind::Messaging);
        }
        NodeAction::Purge if status == "purged" => {
            if let Some(kind) = request.node
                && adapter_for(kind).managed_contract().is_some()
            {
                state.clear_module_context_topology(kind);
            }
        }
        NodeAction::DeleteNetwork | NodeAction::ResetNetwork
            if matches!(status, "deleted" | "reset") =>
        {
            if let Some(network_id) = target_network_id {
                state.clear_module_context_topologies_for_network(network_id);
            }
        }
        _ => {}
    }
}

fn dispatch_action(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    runtime_config_root: &Path,
    normalized_profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
    package_commit: Option<&mut LocalNodePackageCommit>,
) -> LocalNodeActionResult {
    if let Some(control) = control
        && let Err(error) = control.check_active()
    {
        return interrupted_operation(request, request.node, error.into());
    }
    match request.action {
        NodeAction::StartRuntime => {
            runtime_start(state, runtime, runtime_config_root, request, control)
        }
        NodeAction::StopRuntime => runtime_stop(state, runtime, request, control),
        NodeAction::NewNetwork => new_network(state, runtime.as_ref(), request),
        NodeAction::LoadNetwork => load_network(state, runtime.as_ref(), request),
        NodeAction::DeleteNetwork => delete_network(state, runtime.as_ref(), request),
        NodeAction::ResetNetwork => reset_network(state, runtime.as_ref(), request),
        NodeAction::Install => node_install(
            state,
            runtime.as_ref(),
            normalized_profile,
            request,
            control,
            package_commit,
        ),
        NodeAction::Initialize => {
            node_initialize(state, runtime, normalized_profile, request, control)
        }
        NodeAction::Uninstall => node_uninstall(
            state,
            runtime.as_ref(),
            normalized_profile,
            request,
            control,
        ),
        NodeAction::Start => node_start(
            state,
            runtime.as_ref(),
            normalized_profile,
            request,
            control,
        ),
        NodeAction::Stop => node_stop(
            state,
            runtime.as_ref(),
            normalized_profile,
            request,
            control,
        ),
        NodeAction::Purge => node_purge(state, normalized_profile, request),
    }
}

fn new_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
        let id = request
            .network_id
            .as_deref()
            .map(sanitize_network_id)
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| format!("devnet-{}", now_millis()));
        if state.devnets.iter().any(|record| record.id == id) {
            bail!("local devnet `{id}` already exists");
        }

        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let workspace = workspace_root.join(&id);
        fs::create_dir_all(&workspace)
            .with_context(|| format!("failed to create workspace {}", workspace.display()))?;
        let now = now_millis();
        let record = LocalDevnetRecord {
            deployment: LocalNodeDeployment::LocalDevnet,
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
            detail: format!("created local devnet `{id}`"),
            command: None,
        })
    })
}

fn load_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
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
        super::action_engine::normalize_indexer_module_record(&mut record);
        record.updated_at = now_millis();
        write_devnet_manifest(&record)?;
        if let Some(existing) = state.devnet_mut(&record.id) {
            *existing = record.clone();
        } else {
            state.devnets.push(record.clone());
        }
        state.active_devnet = Some(record.id.clone());
        Ok(OperationOutcome {
            status: "loaded".to_owned(),
            detail: format!("loaded local devnet `{}`", record.id),
            command: None,
        })
    })
}

fn delete_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
        let network_id = target_network_id(state, request)?;
        stop_all_owned_processes(state, &network_id);
        let Some(position) = state
            .devnets
            .iter()
            .position(|record| record.id == network_id)
        else {
            bail!("local devnet `{network_id}` was not found");
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
            detail: format!("deleted local devnet `{network_id}`"),
            command: None,
        })
    })
}

fn reset_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        require_runtime_stopped(runtime)?;
        let network_id = target_network_id(state, request)?;
        stop_all_owned_processes(state, &network_id);
        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let Some(record) = state.devnet_mut(&network_id) else {
            bail!("local devnet `{network_id}` was not found");
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
            detail: format!("reset local devnet `{network_id}`"),
            command: None,
        })
    })
}

fn runtime_start(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    runtime_config_root: &Path,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        if let Some(existing) = runtime.as_ref()
            && existing.is_managed()
            && !existing.is_running()
        {
            reap_runtime_process_group(existing)?;
            if let Some(process_id) = existing.daemon_process_id {
                wait_for_runtime_process_group_exit(process_id, control)?;
            }
        }
        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            runtime_config_root,
            runtime.as_ref(),
            request.runtime_binary_path.as_deref(),
            request.runtime_modules_dir.as_deref(),
        )?;
        reconcile_indexer_runtime_modules_dir(state, &profile)?;
        let command = profile.daemon_command()?;
        let display = command_display(&command);
        let process_id =
            super::process::spawn_detached(command, "Inspector-managed logoscore daemon")?;
        profile.daemon_process_id = Some(process_id);
        reset_module_contexts(state);
        let readiness = match control {
            Some(control) => profile.wait_until_ready_controlled(control),
            None => profile.wait_until_ready(),
        };
        let still_running = profile.is_running();
        *runtime = Some(profile);
        match readiness {
            Ok(()) => Ok(OperationOutcome {
                status: "started".to_owned(),
                detail: "Inspector-managed logoscore daemon is ready".to_owned(),
                command: Some(display),
            }),
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) if still_running => Ok(OperationOutcome {
                status: "starting".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(display),
            }),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(display),
            }),
        }
    })
}

fn runtime_stop(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    operation_result(request, None, || {
        let Some(profile) = runtime.as_mut() else {
            return Ok(OperationOutcome {
                status: "needs_configuration".to_owned(),
                detail: "no Inspector-managed logoscore runtime is configured".to_owned(),
                command: None,
            });
        };
        if !profile.is_managed() {
            return Ok(OperationOutcome {
                status: "needs_configuration".to_owned(),
                detail: "external logoscore runtimes are never stopped by Inspector".to_owned(),
                command: None,
            });
        }
        if !profile.is_running() {
            reap_runtime_process_group(profile)?;
            if let Some(process_id) = profile.daemon_process_id {
                wait_for_runtime_process_group_exit(process_id, control)?;
            }
            profile.daemon_process_id = None;
            reset_module_contexts(state);
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: "Inspector-managed logoscore daemon is already stopped".to_owned(),
                command: None,
            });
        }
        let cli = profile.cli_runtime()?;
        let value = match control {
            Some(control) => cli.stop_controlled(control.clone())?,
            None => cli.stop()?,
        }
        .value;
        let stopped = match control {
            Some(control) => profile.wait_until_stopped_controlled(control)?,
            None => profile.wait_until_stopped(),
        };
        if stopped {
            reap_runtime_process_group(profile)?;
            if let Some(process_id) = profile.daemon_process_id {
                wait_for_runtime_process_group_exit(process_id, control)?;
            }
            profile.daemon_process_id = None;
            reset_module_contexts(state);
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: operation_detail_from_value(&value),
                command: Some("logoscore --config-dir <managed> stop --json".to_owned()),
            });
        }
        Ok(OperationOutcome {
            status: "stopping".to_owned(),
            detail: "stop request accepted; waiting for managed daemon exit".to_owned(),
            command: Some("logoscore --config-dir <managed> stop --json".to_owned()),
        })
    })
}

fn reap_runtime_process_group(profile: &LogoscoreRuntimeProfile) -> Result<()> {
    let Some(process_id) = profile.daemon_process_id else {
        return Ok(());
    };
    if !process_group_has_live_members(process_id) {
        return Ok(());
    }
    stop_process(process_id).with_context(|| {
        format!(
            "failed to stop remaining processes owned by Inspector-managed logoscore runtime {process_id}"
        )
    })
}

fn restart_managed_runtime_for_messaging_context(
    runtime: &mut LogoscoreRuntimeProfile,
    control: Option<&CommandControl>,
) -> Result<()> {
    if runtime.is_running() {
        return Ok(());
    }
    if let Some(process_id) = runtime.daemon_process_id {
        reap_runtime_process_group(runtime)?;
        wait_for_runtime_process_group_exit(process_id, control)?;
    }
    if let Some(control) = control {
        control.check_active()?;
    }
    runtime.daemon_process_id = None;
    let command = runtime.daemon_command()?;
    let process_id = spawn_detached(command, "Inspector-managed logoscore daemon")?;
    runtime.daemon_process_id = Some(process_id);
    match control {
        Some(control) => runtime.wait_until_ready_controlled(control),
        None => runtime.wait_until_ready(),
    }
    .context("restarted Inspector-managed logoscore daemon did not become ready")?;

    let cli = runtime.cli_runtime()?;
    match control {
        Some(control) => cli.ensure_module_loaded_controlled("delivery_module", control.clone()),
        None => cli.ensure_module_loaded("delivery_module"),
    }
    .context("restarted Inspector-managed logoscore daemon did not load delivery_module")
}

fn wait_for_runtime_process_group_exit(
    process_id: u32,
    control: Option<&CommandControl>,
) -> Result<()> {
    let deadline = Instant::now() + RUNTIME_PROCESS_GROUP_REAP_TIMEOUT;
    while process_group_has_live_members(process_id) {
        if let Some(control) = control {
            control.check_active()?;
        }
        if Instant::now() >= deadline {
            bail!(
                "Inspector-managed logoscore process group {process_id} still has live members after termination"
            );
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(RUNTIME_PROCESS_GROUP_REAP_POLL_INTERVAL.min(remaining));
    }
    Ok(())
}

fn node_install(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
    package_commit: Option<&mut LocalNodePackageCommit>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Install);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        if policy == NodeActionPolicy::InstallPackage {
            return install_indexer_package(
                state,
                runtime,
                profile,
                request,
                control,
                package_commit,
            );
        }
        let NodeActionPolicy::RegisterExecutable {
            program: executable,
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid install policy",
                adapter.label()
            );
        };
        let Some(binary) = find_command(executable) else {
            return Ok(needs_configuration(&format!("{executable} not found")));
        };
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        config.package_path = Some(binary);
        config.installed = true;
        record.updated_at = now_millis();
        write_devnet_manifest(record)?;
        Ok(OperationOutcome {
            status: "installed".to_owned(),
            detail: format!("{executable} registered"),
            command: None,
        })
    })
}

fn install_indexer_package(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
    package_commit: Option<&mut LocalNodePackageCommit>,
) -> Result<OperationOutcome> {
    if request.node != Some(NodeKind::Indexer) {
        bail!("package installation is only implemented for Indexer");
    }
    require_runtime_stopped(runtime)?;
    let version = required_trimmed_request_value(
        request.package_version.as_deref(),
        "Indexer package version",
    )?;
    let root_hash = required_trimmed_request_value(
        request.package_root_hash.as_deref(),
        "Indexer package root hash",
    )?;
    let catalog = local_node_package_catalog(request.runtime_modules_dir.as_deref())?;
    validate_runtime_modules_dir(runtime, &catalog.modules_dir)?;
    let release = catalog
        .package
        .versions
        .iter()
        .find(|release| release.version == version && release.root_hash == root_hash)
        .with_context(|| {
            format!(
                "official lez_indexer_module release `{version}` with root hash `{root_hash}` is unavailable"
            )
        })?;

    if let Some(installed) = catalog.installed.as_ref()
        && installed.version == release.version
        && installed.root_hash == release.root_hash
    {
        record_indexer_package(state, profile, installed)?;
        return Ok(OperationOutcome {
            status: "installed".to_owned(),
            detail: format!(
                "lez_indexer_module {} is already installed in {}",
                installed.version, catalog.modules_dir
            ),
            command: None,
        });
    }

    let temporary = tempfile::Builder::new()
        .prefix("logos-inspector-indexer-package-")
        .tempdir()
        .context("failed to create Indexer package download directory")?;
    let package_control = control.cloned().unwrap_or_else(|| {
        CommandControl::new(
            CancellationToken::new(),
            Instant::now() + INDEXER_PACKAGE_INSTALL_TIMEOUT,
        )
    });
    let downloaded =
        download_official_indexer_module(release, temporary.path(), package_control.clone())?;
    if let Some(control) = control {
        control.check_active()?;
    }
    let install_control = match package_commit {
        Some(package_commit) => package_commit.begin()?,
        None => package_control,
    };
    let installed = install_official_indexer_module(
        &downloaded,
        Path::new(&catalog.modules_dir),
        install_control,
    )?;
    record_indexer_package(state, profile, &installed)?;
    Ok(OperationOutcome {
        status: "installed".to_owned(),
        detail: format!(
            "installed lez_indexer_module {} in {}; start or restart the managed LogosCore runtime before using it",
            installed.version, catalog.modules_dir
        ),
        command: Some(
            "lgpd download lez_indexer_module && lgpm install lez_indexer_module".to_owned(),
        ),
    })
}

fn record_indexer_package(
    state: &mut LocalNodesState,
    profile: &str,
    installed: &super::package::LocalNodeInstalledPackageReport,
) -> Result<()> {
    let active_topology_id = state
        .active_topology(profile)
        .map(|record| record.id.clone())
        .context("active local node topology is required")?;
    let modules_dir = installed_package_modules_dir(installed)?;
    let mut reconciled = state.clone();
    let mut changed_records = Vec::new();
    let mut active_updated = false;
    for record in reconciled
        .testnet
        .iter_mut()
        .chain(reconciled.devnets.iter_mut())
    {
        let is_active = record.id == active_topology_id;
        let Some(config) = node_config_mut(record, NodeKind::Indexer) else {
            continue;
        };
        let recorded_modules_dir = config
            .package_path
            .as_deref()
            .and_then(package_path_modules_dir);
        if !is_active
            && recorded_modules_dir
                .as_ref()
                .is_some_and(|recorded| recorded != &modules_dir)
        {
            continue;
        }
        config.package_path = Some(installed.main_file_path.clone());
        config.package_version = Some(installed.version.clone());
        config.package_root_hash = Some(installed.root_hash.clone());
        config.module_path = Some("lez_indexer_module".to_owned());
        config.indexer_state = None;
        config.indexer_head = None;
        config.indexer_error = None;
        config.process_id = None;
        config.installed = true;
        config.lifecycle_state = NodeLifecycleState::Stopped;
        config.pending_lifecycle_action = None;
        record.updated_at = now_millis();
        active_updated |= is_active;
        changed_records.push(record.clone());
    }
    if !active_updated {
        bail!("active local node topology has no Indexer config");
    }
    for record in &changed_records {
        write_devnet_manifest(record)?;
    }
    *state = reconciled;
    Ok(())
}

fn reconcile_indexer_runtime_modules_dir(
    state: &mut LocalNodesState,
    runtime: &LogoscoreRuntimeProfile,
) -> Result<()> {
    let runtime_modules_dir = runtime
        .modules_dir
        .as_deref()
        .context("managed LogosCore runtime has no modules directory")?;
    let runtime_modules_dir = canonical_modules_dir(Path::new(runtime_modules_dir))?;
    let mut reconciled = state.clone();
    let mut changed_records = Vec::new();
    for record in reconciled
        .testnet
        .iter_mut()
        .chain(reconciled.devnets.iter_mut())
    {
        let Some(config) = node_config_mut(record, NodeKind::Indexer) else {
            continue;
        };
        let has_package_identity = config.installed
            || config.package_path.is_some()
            || config.package_version.is_some()
            || config.package_root_hash.is_some();
        if !has_package_identity {
            continue;
        }
        let matches_runtime = config
            .package_path
            .as_deref()
            .and_then(package_path_modules_dir)
            .is_some_and(|modules_dir| modules_dir == runtime_modules_dir);
        if matches_runtime {
            continue;
        }
        clear_module_context(config);
        record.updated_at = now_millis();
        changed_records.push(record.clone());
    }
    for record in &changed_records {
        write_devnet_manifest(record)?;
    }
    *state = reconciled;
    Ok(())
}

fn validate_runtime_modules_dir(
    runtime: Option<&LogoscoreRuntimeProfile>,
    requested_modules_dir: &str,
) -> Result<()> {
    let Some(configured) = runtime
        .and_then(|profile| profile.modules_dir.as_deref())
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(());
    };
    let configured_modules_dir = canonical_modules_dir(Path::new(configured))?;
    let requested_modules_dir = canonical_modules_dir(Path::new(requested_modules_dir))?;
    if configured_modules_dir != requested_modules_dir {
        bail!(
            "selected modules directory `{}` differs from the managed LogosCore runtime directory `{configured}`",
            requested_modules_dir.display()
        );
    }
    Ok(())
}

fn required_trimmed_request_value<'a>(value: Option<&'a str>, label: &str) -> Result<&'a str> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{label} is required"))
}

fn node_initialize(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Initialize);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let NodeActionPolicy::ExecuteManaged { ensure_loaded, .. } = policy else {
            bail!(
                "{} adapter returned an invalid initialize policy",
                adapter.label()
            );
        };
        if let Some(detail) = managed_context_initialization_problem(state, profile, kind) {
            return Ok(needs_configuration(&detail));
        }
        let cli = {
            let Some(runtime) = managed_runtime(runtime.as_ref()) else {
                return Ok(needs_configuration(
                    "start an Inspector-managed logoscore runtime before initializing a module node",
                ));
            };
            runtime.cli_runtime()?
        };
        let (spec, reuse_generated_config) = {
            let record = active_topology_mut(state, profile)?;
            let config = required_node_config(record, kind)?;
            let action_config_path = action_config_path(config, NodeAction::Initialize);
            let spec = command_spec_for(
                kind,
                NodeAction::Initialize,
                action_config_path,
                &config.data_dir,
                config.port,
            )
            .with_context(|| format!("{} initialization is not implemented", adapter.label()))?;
            (spec, reusable_generated_config(adapter, config))
        };
        if ensure_loaded {
            ensure_module_loaded(&spec, Some(&cli), control)?;
        }
        if reuse_generated_config {
            mark_node_initialized(state, profile, kind, &spec.program)?;
            return Ok(OperationOutcome {
                status: "initialized".to_owned(),
                detail: "reused existing generated configuration; module is loaded".to_owned(),
                command: adapter
                    .managed_contract()
                    .map(|contract| format!("logoscore module load {}", contract.module_id())),
            });
        }
        let execution = match control {
            Some(control) => match retry_initialization_preflight(&spec, &cli, control) {
                Ok(()) => execute_preflighted_command_spec(&spec, Some(&cli), Some(control)),
                Err(error) if is_control_interruption(&error) => return Err(error),
                Err(error) => {
                    return Ok(OperationOutcome {
                        status: "failed".to_owned(),
                        detail: operation_error_detail(&error),
                        command: Some(spec.display.clone()),
                    });
                }
            },
            None => execute_command_spec(&spec, Some(&cli), None),
        };
        match execution {
            Ok(value) => {
                if kind == NodeKind::Messaging {
                    let runtime = runtime
                        .as_mut()
                        .filter(|profile| profile.is_managed())
                        .context(
                            "Inspector-managed logoscore runtime disappeared during Messaging verification",
                        )?;
                    match verify_messaging_context(state, runtime, control) {
                        Ok(verification) => {
                            mark_node_initialized(state, profile, kind, &spec.program)?;
                            return Ok(OperationOutcome {
                                status: "initialized".to_owned(),
                                detail: format!(
                                    "{}; verified Messaging context with MyPeerId `{}`",
                                    operation_detail_from_value(&value),
                                    verification.peer_id
                                ),
                                command: Some(spec.display),
                            });
                        }
                        Err(probe_error) if is_control_interruption(&probe_error) => {
                            return Err(probe_error);
                        }
                        Err(probe_error) => {
                            return Ok(OperationOutcome {
                                status: "failed".to_owned(),
                                detail: format!(
                                    "Messaging initialization response was accepted, but context verification failed: {probe_error:#}"
                                ),
                                command: Some(spec.display),
                            });
                        }
                    }
                }
                mark_node_initialized(state, profile, kind, &spec.program)?;
                Ok(OperationOutcome {
                    status: "initialized".to_owned(),
                    detail: operation_detail_from_value(&value),
                    command: Some(spec.display),
                })
            }
            Err(error)
                if kind == NodeKind::Messaging && is_ambiguous_messaging_create_error(&error) =>
            {
                let runtime = runtime
                    .as_mut()
                    .filter(|profile| profile.is_managed())
                    .context(
                        "Inspector-managed logoscore runtime disappeared during Messaging recovery",
                    )?;
                match verify_messaging_context(state, runtime, control) {
                    Ok(verification) => {
                        mark_node_initialized(state, profile, kind, &spec.program)?;
                        Ok(OperationOutcome {
                            status: "initialized".to_owned(),
                            detail: messaging_context_recovery_detail(&verification),
                            command: Some(spec.display),
                        })
                    }
                    Err(probe_error) if is_control_interruption(&probe_error) => Err(probe_error),
                    Err(probe_error) => Ok(OperationOutcome {
                        status: "failed".to_owned(),
                        detail: format!(
                            "{error:#}; Messaging context verification failed: {probe_error:#}"
                        ),
                        command: Some(spec.display),
                    }),
                }
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn retry_initialization_preflight(
    spec: &super::commands::LocalNodeCommandSpec,
    cli: &crate::modules::logos_core::LogoscoreCliRuntime,
    control: &CommandControl,
) -> Result<()> {
    match preflight_command_spec(spec, Some(cli), Some(control)) {
        Ok(()) => Ok(()),
        Err(error) if is_control_interruption(&error) => Err(error),
        Err(first_error) => {
            sleep_with_control(control, INITIALIZATION_PREFLIGHT_RETRY_DELAY)?;
            preflight_command_spec_once(spec, Some(cli), control).with_context(|| {
                format!("module initialization preflight retry after: {first_error:#}")
            })
        }
    }
}

fn sleep_with_control(control: &CommandControl, duration: Duration) -> Result<()> {
    control.check_active()?;
    let remaining = control.deadline().saturating_duration_since(Instant::now());
    thread::sleep(duration.min(remaining));
    control.check_active().map_err(Into::into)
}

fn mark_node_initialized(
    state: &mut LocalNodesState,
    profile: &str,
    kind: NodeKind,
    package_path: &str,
) -> Result<()> {
    let record = active_topology_mut(state, profile)?;
    let config = required_node_config(record, kind)?;
    config.installed = true;
    config.package_path = Some(package_path.to_owned());
    config.lifecycle_state = NodeLifecycleState::Stopped;
    config.pending_lifecycle_action = None;
    record.updated_at = now_millis();
    write_devnet_manifest(record)
}

fn is_ambiguous_messaging_create_error(error: &anyhow::Error) -> bool {
    error.to_string().contains("RPC_FAILED")
}

struct MessagingContextVerification {
    peer_id: String,
    restarted_runtime: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MessagingContextProbe {
    Available,
    Absent,
    Unknown,
}

pub(super) fn probe_messaging_context(
    cli: &LogoscoreCliRuntime,
    control: CommandControl,
) -> MessagingContextProbe {
    let args = ["MyPeerId".to_owned()];
    let Ok(response) = cli.call_controlled("delivery_module", "getNodeInfo", &args, control) else {
        return MessagingContextProbe::Unknown;
    };
    messaging_context_probe_from_response(&response.value)
}

fn messaging_context_probe_from_response(response: &Value) -> MessagingContextProbe {
    if response.get("status").and_then(Value::as_str) != Some("ok") {
        return MessagingContextProbe::Unknown;
    }
    let Some(result) = response.get("result").and_then(Value::as_object) else {
        return MessagingContextProbe::Unknown;
    };
    match result.get("success").and_then(Value::as_bool) {
        Some(true) => {
            if result
                .get("value")
                .and_then(Value::as_str)
                .is_some_and(|peer_id| !peer_id.trim().is_empty())
            {
                MessagingContextProbe::Available
            } else {
                MessagingContextProbe::Unknown
            }
        }
        Some(false) => {
            if result
                .get("error")
                .and_then(Value::as_str)
                .is_some_and(|error| {
                    error
                        .to_ascii_lowercase()
                        .contains("context not initialized")
                })
            {
                MessagingContextProbe::Absent
            } else {
                MessagingContextProbe::Unknown
            }
        }
        None => MessagingContextProbe::Unknown,
    }
}

fn messaging_context_recovery_detail(verification: &MessagingContextVerification) -> String {
    if verification.restarted_runtime {
        format!(
            "createNode response was lost; restarted the Inspector-managed LogosCore runtime and verified Messaging context with MyPeerId `{}`",
            verification.peer_id
        )
    } else {
        format!(
            "createNode response was lost; verified Messaging context with MyPeerId `{}`",
            verification.peer_id
        )
    }
}

fn verify_messaging_context(
    state: &mut LocalNodesState,
    runtime: &mut LogoscoreRuntimeProfile,
    control: Option<&CommandControl>,
) -> Result<MessagingContextVerification> {
    let deadline = control.map_or_else(
        || {
            Instant::now()
                + MESSAGING_CONTEXT_PROBE_ATTEMPTS as u32 * MESSAGING_CONTEXT_PROBE_INTERVAL
        },
        CommandControl::deadline,
    );
    let mut last_error = None;
    let mut runtime_restarts = 0;
    for attempt in 0..MESSAGING_CONTEXT_PROBE_ATTEMPTS {
        if let Some(control) = control {
            control.check_active()?;
        }
        if !runtime.is_running() {
            if runtime_restarts >= MESSAGING_CONTEXT_RUNTIME_RESTARTS {
                bail!(
                    "Messaging context verification found the restarted Inspector-managed LogosCore runtime stopped"
                );
            }
            reset_module_contexts(state);
            restart_managed_runtime_for_messaging_context(runtime, control)?;
            runtime_restarts += 1;
        }
        match messaging_peer_id(runtime, control) {
            Ok(peer_id) => {
                return Ok(MessagingContextVerification {
                    peer_id,
                    restarted_runtime: runtime_restarts > 0,
                });
            }
            Err(error) => last_error = Some(error),
        }
        if attempt + 1 == MESSAGING_CONTEXT_PROBE_ATTEMPTS {
            break;
        }
        if let Some(control) = control {
            control.check_active()?;
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(MESSAGING_CONTEXT_PROBE_INTERVAL.min(remaining));
    }
    let detail = last_error
        .map(|error| error.to_string())
        .unwrap_or_else(|| "no getNodeInfo response".to_owned());
    bail!("Messaging context verification did not find MyPeerId: {detail}")
}

fn messaging_peer_id(
    runtime: &LogoscoreRuntimeProfile,
    control: Option<&CommandControl>,
) -> Result<String> {
    let cli = runtime.cli_runtime()?;
    let args = ["MyPeerId".to_owned()];
    let response = match control {
        Some(control) => cli.call_checked_controlled(
            "delivery_module",
            "getNodeInfo",
            "getNodeInfo(QString)",
            &args,
            control.clone(),
        ),
        None => cli.call_checked(
            "delivery_module",
            "getNodeInfo",
            "getNodeInfo(QString)",
            &args,
        ),
    }?;
    let value = response
        .get("value")
        .cloned()
        .context("Messaging getNodeInfo returned no LogosCore value")?;
    let peer_id = normalize_module_call_value("delivery_module", "getNodeInfo", value)?;
    peer_id
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .context("Messaging getNodeInfo returned an empty MyPeerId")
}

fn node_uninstall(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Uninstall);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        if policy == NodeActionPolicy::RemoveExecutableRegistration {
            stop_owned_process(config);
            config.installed = false;
            config.package_path = None;
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
            return Ok(OperationOutcome {
                status: "uninstalled".to_owned(),
                detail: format!("{} registration removed", adapter.label()),
                command: None,
            });
        }
        if !matches!(policy, NodeActionPolicy::ExecuteManaged { .. }) {
            bail!(
                "{} adapter returned an invalid uninstall policy",
                adapter.label()
            );
        }
        let Some(runtime) = managed_runtime(runtime) else {
            return Ok(needs_configuration(
                "start the Inspector-managed logoscore runtime before removing a module context",
            ));
        };
        if config.lifecycle_state.is_pending()
            || config.lifecycle_state == NodeLifecycleState::Running
        {
            return Ok(needs_configuration(
                "stop the module node and wait for lifecycle confirmation before removing its context",
            ));
        }
        let Some(spec) = command_spec_for(
            kind,
            NodeAction::Uninstall,
            &config.config_path,
            &config.data_dir,
            config.port,
        ) else {
            return Ok(needs_configuration(
                "this module has no verified context-destroy contract; stop the managed runtime to clear it",
            ));
        };
        let cli = runtime.cli_runtime()?;
        match execute_command_spec(&spec, Some(&cli), control) {
            Ok(value) => {
                clear_module_context(config);
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "uninstalled".to_owned(),
                    detail: operation_detail_from_value(&value),
                    command: Some(spec.display),
                })
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn node_start(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Start);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let managed_options = match policy {
            NodeActionPolicy::ExecuteDetached => None,
            NodeActionPolicy::ExecuteManaged {
                ensure_loaded,
                requires_installed_context,
            } => Some((ensure_loaded, requires_installed_context)),
            _ => bail!(
                "{} adapter returned an invalid start policy",
                adapter.label()
            ),
        };
        let managed_runtime_profile = managed_runtime(runtime);
        if managed_options.is_some() {
            if managed_runtime_profile.is_none() {
                return Ok(needs_configuration(
                    "start an Inspector-managed logoscore runtime before starting a module node",
                ));
            }
            if let Some(detail) = managed_start_context_binding_problem(state, profile, kind) {
                return Ok(needs_configuration(&detail));
            }
            if kind == NodeKind::Indexer
                && let Some(problem) = prepare_indexer_start_config(state, profile, request)?
            {
                return Ok(needs_configuration(&problem));
            }
        }
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to create {}", config.data_dir))?;
        let spec = command_spec_for(
            kind,
            NodeAction::Start,
            &config.config_path,
            &config.data_dir,
            config.port,
        )
        .with_context(|| format!("{} start is not implemented", adapter.label()))?;
        if policy == NodeActionPolicy::ExecuteDetached {
            if config.lifecycle_state.is_pending() {
                return Ok(needs_configuration(
                    "a process lifecycle action is already pending confirmation",
                ));
            }
            if config
                .process_id
                .is_some_and(process_group_has_live_members)
            {
                return Ok(needs_configuration(&format!(
                    "an Inspector-owned {} process is already running",
                    adapter.label()
                )));
            }
            let execution = match adapter.startup_rpc_readiness() {
                Some(readiness) => execute_ready_process_spec(
                    &spec,
                    config
                        .endpoint
                        .as_deref()
                        .context("registered process RPC endpoint is required")?,
                    readiness,
                    control,
                ),
                None => execute_command_spec(&spec, None, control),
            };
            return match execution {
                Ok(value) => {
                    config.process_id = value
                        .get("pid")
                        .and_then(Value::as_u64)
                        .and_then(|pid| u32::try_from(pid).ok());
                    config.installed = true;
                    config.lifecycle_state = NodeLifecycleState::Starting;
                    config.pending_lifecycle_action = Some(NodeAction::Start);
                    record.updated_at = now_millis();
                    write_devnet_manifest(record)?;
                    Ok(OperationOutcome {
                        status: "starting".to_owned(),
                        detail: format!(
                            "{}; waiting for Inspector process confirmation",
                            operation_detail_from_value(&value)
                        ),
                        command: Some(spec.display),
                    })
                }
                Err(error) if is_control_interruption(&error) => Err(error),
                Err(error) => Ok(OperationOutcome {
                    status: "failed".to_owned(),
                    detail: operation_error_detail(&error),
                    command: Some(spec.display),
                }),
            };
        }
        let Some((ensure_loaded, requires_installed_context)) = managed_options else {
            bail!("{} adapter has no managed start policy", adapter.label());
        };
        let Some(runtime) = managed_runtime_profile else {
            bail!(
                "{} managed runtime disappeared before start",
                adapter.label()
            );
        };
        if config.lifecycle_state.is_pending() {
            return Ok(needs_configuration(
                "a module lifecycle action is already pending confirmation",
            ));
        }
        if requires_installed_context && !config.installed {
            return Ok(needs_configuration(
                "initialize the module node before starting it",
            ));
        }
        let cli = runtime.cli_runtime()?;
        if ensure_loaded {
            ensure_module_loaded(&spec, Some(&cli), control)?;
        }
        match execute_command_spec(&spec, Some(&cli), control) {
            Ok(value) => {
                config.installed = true;
                if kind == NodeKind::Indexer {
                    config.indexer_state = None;
                    config.indexer_head = None;
                    config.indexer_error = None;
                }
                config.lifecycle_state = NodeLifecycleState::Starting;
                config.pending_lifecycle_action = Some(NodeAction::Start);
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "starting".to_owned(),
                    detail: lifecycle_dispatch_detail(&value),
                    command: Some(spec.display),
                })
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn node_stop(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    profile: &str,
    request: &LocalNodeActionRequest,
    control: Option<&CommandControl>,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Stop);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let requires_installed_context = match policy {
            NodeActionPolicy::ExecuteDetached => None,
            NodeActionPolicy::ExecuteManaged {
                requires_installed_context,
                ..
            } => Some(requires_installed_context),
            _ => bail!(
                "{} adapter returned an invalid stop policy",
                adapter.label()
            ),
        };
        let managed_runtime_profile = managed_runtime(runtime);
        if requires_installed_context.is_some() {
            if managed_runtime_profile.is_none() {
                return Ok(needs_configuration(
                    "start an Inspector-managed logoscore runtime before stopping a module node",
                ));
            }
            if kind == NodeKind::Indexer
                && let Some(problem) = indexer_stop_configuration_problem(state, profile, request)?
            {
                return Ok(needs_configuration(&problem));
            }
            if let Some(detail) = managed_context_binding_problem(state, profile, kind) {
                return Ok(needs_configuration(&detail));
            }
        }
        let record = active_topology_mut(state, profile)?;
        let config = required_node_config(record, kind)?;
        if policy == NodeActionPolicy::ExecuteDetached {
            if config.lifecycle_state.is_pending() {
                return Ok(needs_configuration(
                    "a process lifecycle action is already pending confirmation",
                ));
            }
            if config.process_id.is_none() {
                return Ok(needs_configuration(&format!(
                    "no Inspector-owned {} process is recorded",
                    adapter.label()
                )));
            }
            request_owned_process_stop(config)?;
            config.lifecycle_state = NodeLifecycleState::Stopping;
            config.pending_lifecycle_action = Some(NodeAction::Stop);
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
            return Ok(OperationOutcome {
                status: "stopping".to_owned(),
                detail: format!(
                    "requested stop for Inspector-owned {} process; waiting for Inspector process confirmation",
                    adapter.label()
                ),
                command: None,
            });
        }
        let Some(requires_installed_context) = requires_installed_context else {
            bail!("{} adapter has no managed stop policy", adapter.label());
        };
        let Some(runtime) = managed_runtime_profile else {
            bail!(
                "{} managed runtime disappeared before stop",
                adapter.label()
            );
        };
        if config.lifecycle_state.is_pending() {
            return Ok(needs_configuration(
                "a module lifecycle action is already pending confirmation",
            ));
        }
        if requires_installed_context && !config.installed {
            return Ok(needs_configuration(
                "initialize the module node before stopping it",
            ));
        }
        if kind == NodeKind::Messaging {
            let cli = runtime.cli_runtime()?;
            return match unload_messaging_context(&cli, config.port, control) {
                Ok(value) => {
                    clear_module_context(config);
                    record.updated_at = now_millis();
                    write_devnet_manifest(record)?;
                    Ok(OperationOutcome {
                        status: "stopped".to_owned(),
                        detail: format!(
                            "{}; unloaded Delivery and cleared its Messaging context; initialize Messaging before starting it again",
                            operation_detail_from_value(&value)
                        ),
                        command: Some("logoscore unload-module delivery_module --json".to_owned()),
                    })
                }
                Err(error) if is_control_interruption(&error) => Err(error),
                Err(error) => Ok(OperationOutcome {
                    status: "failed".to_owned(),
                    detail: operation_error_detail(&error),
                    command: Some("logoscore unload-module delivery_module --json".to_owned()),
                }),
            };
        }
        let spec = command_spec_for(
            kind,
            NodeAction::Stop,
            &config.config_path,
            &config.data_dir,
            config.port,
        )
        .with_context(|| format!("{} stop is not implemented", adapter.label()))?;
        let cli = runtime.cli_runtime()?;
        match execute_command_spec(&spec, Some(&cli), control) {
            Ok(value) => {
                config.lifecycle_state = NodeLifecycleState::Stopping;
                config.pending_lifecycle_action = Some(NodeAction::Stop);
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "stopping".to_owned(),
                    detail: lifecycle_dispatch_detail(&value),
                    command: Some(spec.display),
                })
            }
            Err(error) if is_control_interruption(&error) => Err(error),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: operation_error_detail(&error),
                command: Some(spec.display),
            }),
        }
    })
}

fn prepare_indexer_start_config(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> Result<Option<String>> {
    let channel_id =
        required_trimmed_request_value(request.channel_id.as_deref(), "Indexer Channel ID")?;
    validate_channel_id(channel_id)?;
    let bedrock_endpoint = normalized_bedrock_endpoint(required_trimmed_request_value(
        request.bedrock_endpoint.as_deref(),
        "Indexer Bedrock endpoint",
    )?)?;
    let record = active_topology_mut(state, profile)?;
    let config = required_node_config(record, NodeKind::Indexer)?;
    if !config.installed {
        return Ok(Some(
            "install lez_indexer_module before starting a Channel Indexer".to_owned(),
        ));
    }
    let configured_channel = indexer_channel_from_config(&config.config_path)?;
    if !matches!(
        config.lifecycle_state,
        NodeLifecycleState::Stopped | NodeLifecycleState::NotInitialized
    ) {
        if let Some(configured_channel) = configured_channel
            && configured_channel != channel_id
        {
            return Ok(Some(format!(
                "Indexer is bound to Channel `{configured_channel}`; stop it before starting Channel `{channel_id}`"
            )));
        }
        return Ok(Some(
            "Indexer must be stopped before it can be started".to_owned(),
        ));
    }
    config.indexer_state = None;
    config.indexer_head = None;
    config.indexer_error = None;
    let config_value = crate::source_routing::execution_zone_layer::managed_indexer_channel_config(
        channel_id,
        &bedrock_endpoint,
    );
    let config_text = serde_json::to_string_pretty(&config_value)
        .context("failed to serialize Channel Indexer config")?;
    let config_path = Path::new(&config.config_path);
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    fs::write(config_path, config_text)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    record.updated_at = now_millis();
    write_devnet_manifest(record)?;
    Ok(None)
}

fn indexer_stop_configuration_problem(
    state: &LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> Result<Option<String>> {
    let requested_channel =
        required_trimmed_request_value(request.channel_id.as_deref(), "Indexer Channel ID")?;
    validate_channel_id(requested_channel)?;
    let configured = state
        .active_topology(profile)
        .and_then(|record| {
            record
                .nodes
                .iter()
                .find(|node| node.kind == NodeKind::Indexer)
        })
        .context("Indexer config is not available")?;
    let Some(configured_channel) = indexer_channel_from_config(&configured.config_path)? else {
        return Ok(Some("Indexer has no configured Channel".to_owned()));
    };
    if configured_channel != requested_channel {
        return Ok(Some(format!(
            "Indexer is bound to Channel `{configured_channel}`, not `{requested_channel}`"
        )));
    }
    Ok(None)
}

pub(super) fn indexer_channel_from_config(path: &str) -> Result<Option<String>> {
    let path = Path::new(path);
    if !path.is_file() {
        return Ok(None);
    }
    let value: Value = serde_json::from_str(
        &fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?,
    )
    .with_context(|| format!("failed to parse {}", path.display()))?;
    Ok(value
        .get("channel_id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned))
}

fn validate_channel_id(channel_id: &str) -> Result<()> {
    // `ChannelId` is serialized as a 32-byte hex value. The module's storage
    // reset contract enforces the same representation before deriving its path.
    anyhow::ensure!(
        channel_id.len() == 64 && channel_id.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "Indexer Channel ID must be exactly 64 hexadecimal characters"
    );
    Ok(())
}

fn normalized_bedrock_endpoint(endpoint: &str) -> Result<String> {
    let parsed = Url::parse(endpoint).context("Indexer Bedrock endpoint is invalid")?;
    if !matches!(parsed.scheme(), "http" | "https")
        || parsed.host_str().is_none()
        || !parsed.username().is_empty()
        || parsed.password().is_some()
        || parsed.path() != "/"
        || parsed.query().is_some()
        || parsed.fragment().is_some()
    {
        bail!(
            "Indexer Bedrock endpoint must be an HTTP(S) origin without credentials, query, or fragment"
        );
    }
    Ok(endpoint.trim_end_matches('/').to_owned())
}

fn unload_messaging_context(
    cli: &LogoscoreCliRuntime,
    configured_port: Option<u16>,
    control: Option<&CommandControl>,
) -> Result<Value> {
    let output = match control {
        Some(control) => cli.unload_module_controlled("delivery_module", control.clone()),
        None => cli.unload_module("delivery_module"),
    }
    .context("failed to unload Delivery while stopping Messaging")?;
    let modules = match control {
        Some(control) => cli.list_modules_controlled(control.clone()),
        None => cli.list_modules(),
    }
    .context("failed to confirm Delivery unload while stopping Messaging")?;
    if !module_is_unloaded(&modules.value, "delivery_module") {
        bail!("Delivery remained loaded after its stop teardown request");
    }
    wait_for_messaging_rest_close(configured_port.unwrap_or(8645), control)?;
    Ok(output.value)
}

fn module_is_unloaded(value: &Value, module: &str) -> bool {
    value
        .as_array()
        .or_else(|| value.get("modules").and_then(Value::as_array))
        .and_then(|modules| {
            modules
                .iter()
                .find(|candidate| candidate.get("name").and_then(Value::as_str) == Some(module))
        })
        .and_then(|candidate| candidate.get("status").and_then(Value::as_str))
        .is_some_and(|status| status == "not_loaded")
}

fn wait_for_messaging_rest_close(port: u16, control: Option<&CommandControl>) -> Result<()> {
    let deadline = control.map_or_else(
        || Instant::now() + MESSAGING_UNLOAD_CONFIRMATION_TIMEOUT,
        CommandControl::deadline,
    );
    while messaging_rest_is_open(port) {
        if let Some(control) = control {
            control.check_active()?;
        }
        if Instant::now() >= deadline {
            bail!("Messaging REST endpoint on port {port} remained open after Delivery unload");
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        thread::sleep(MESSAGING_UNLOAD_CONFIRMATION_INTERVAL.min(remaining));
    }
    Ok(())
}

fn messaging_rest_is_open(port: u16) -> bool {
    TcpStream::connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port)),
        Duration::from_millis(250),
    )
    .is_ok()
}

fn node_purge(
    state: &mut LocalNodesState,
    profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeActionResult {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Purge);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let NodeActionPolicy::PurgeData {
            requires_removed_context,
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid purge policy",
                adapter.label()
            );
        };
        let workspace_root = PathBuf::from(&state.managed_workspace_root);
        let Some(record) = state.active_topology_mut(profile) else {
            bail!("active local node topology is required");
        };
        let Some(config) = node_config_mut(record, kind) else {
            bail!("{} config is not available", adapter.label());
        };
        if requires_removed_context && config.installed {
            return Ok(needs_configuration(
                "remove the module context before purging its data directory",
            ));
        }
        stop_owned_process(config);
        remove_dir_inside(&workspace_root, Path::new(&config.data_dir))?;
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to recreate {}", config.data_dir))?;
        config.process_id = None;
        config.lifecycle_state = NodeLifecycleState::NotInitialized;
        config.pending_lifecycle_action = None;
        record.updated_at = now_millis();
        write_devnet_manifest(record)?;
        Ok(OperationOutcome {
            status: "purged".to_owned(),
            detail: format!("purged {} data directory", adapter.label()),
            command: None,
        })
    })
}

fn require_runtime_stopped(runtime: Option<&LogoscoreRuntimeProfile>) -> Result<()> {
    if runtime.is_some_and(LogoscoreRuntimeProfile::is_running) {
        bail!(
            "stop the Inspector-managed logoscore runtime before changing Local Devnet workspaces"
        );
    }
    Ok(())
}

fn managed_runtime(runtime: Option<&LogoscoreRuntimeProfile>) -> Option<&LogoscoreRuntimeProfile> {
    runtime.filter(|profile| profile.is_managed() && profile.is_running())
}

fn managed_context_binding_problem(
    state: &LocalNodesState,
    profile: &str,
    kind: NodeKind,
) -> Option<String> {
    let topology = state.active_topology(profile)?;
    match state.module_context_topology_id(kind) {
        Some(topology_id) if topology_id == topology.id => None,
        Some(topology_id) if kind == NodeKind::Indexer => Some(format!(
            "Indexer module context is bound to `{topology_id}`; select that topology and stop its Channel Indexer before controlling `{}`",
            topology.id,
        )),
        Some(topology_id) => Some(format!(
            "{} module context is bound to `{topology_id}`; initialize it for `{}` before controlling it",
            adapter_for(kind).label(),
            topology.id,
        )),
        None if kind == NodeKind::Indexer => None,
        None => Some(format!(
            "initialize {} to bind its LogosCore module context to `{}` before controlling it",
            adapter_for(kind).label(),
            topology.id,
        )),
    }
}

fn managed_start_context_binding_problem(
    state: &LocalNodesState,
    profile: &str,
    kind: NodeKind,
) -> Option<String> {
    let problem = managed_context_binding_problem(state, profile, kind);
    if problem.is_some()
        && kind == NodeKind::Indexer
        && stopped_indexer_context_can_rebind(state, profile)
    {
        return None;
    }
    problem
}

fn stopped_indexer_context_can_rebind(state: &LocalNodesState, profile: &str) -> bool {
    let Some(active_topology_id) = state
        .active_topology(profile)
        .map(|record| record.id.as_str())
    else {
        return false;
    };
    let Some(bound_topology_id) = state.module_context_topology_id(NodeKind::Indexer) else {
        return false;
    };
    if bound_topology_id == active_topology_id {
        return false;
    }
    state
        .testnet
        .iter()
        .chain(state.devnets.iter())
        .find(|record| record.id == bound_topology_id)
        .and_then(|record| {
            record
                .nodes
                .iter()
                .find(|node| node.kind == NodeKind::Indexer)
        })
        .is_some_and(|config| {
            config.pending_lifecycle_action.is_none()
                && matches!(
                    config.lifecycle_state,
                    NodeLifecycleState::Stopped | NodeLifecycleState::NotInitialized
                )
        })
}

fn managed_context_initialization_problem(
    state: &LocalNodesState,
    profile: &str,
    kind: NodeKind,
) -> Option<String> {
    let topology = state.active_topology(profile)?;
    if let Some(topology_id) = state.module_context_topology_id(kind) {
        return (topology_id != topology.id).then(|| {
            format!(
                "{} module context is bound to `{topology_id}`; stop the managed runtime before initializing it for `{}`",
                adapter_for(kind).label(),
                topology.id,
            )
        });
    }

    let another_initialized_context = state
        .testnet
        .iter()
        .chain(state.devnets.iter())
        .filter(|record| record.id != topology.id)
        .any(|record| {
            record.nodes.iter().any(|node| {
                node.kind == kind && node.installed && node.lifecycle_state.has_module_context()
            })
        });
    another_initialized_context.then(|| {
        format!(
            "{} has an unbound initialized context in another topology; stop the managed runtime before initializing it for `{}`",
            adapter_for(kind).label(),
            topology.id,
        )
    })
}

fn needs_configuration(detail: &str) -> OperationOutcome {
    OperationOutcome {
        status: "needs_configuration".to_owned(),
        detail: detail.to_owned(),
        command: None,
    }
}

fn active_topology_mut<'a>(
    state: &'a mut LocalNodesState,
    profile: &str,
) -> Result<&'a mut LocalDevnetRecord> {
    state
        .active_topology_mut(profile)
        .context("active local node topology is required")
}

fn action_config_path(config: &LocalNodeConfigRecord, action: NodeAction) -> &str {
    if action == NodeAction::Initialize {
        config
            .initialization_config_path
            .as_deref()
            .unwrap_or(&config.config_path)
    } else {
        &config.config_path
    }
}

fn reusable_generated_config(
    adapter: &dyn super::adapters::LocalNodeAdapter,
    config: &LocalNodeConfigRecord,
) -> bool {
    adapter.preserve_generated_config_on_runtime_reset() && Path::new(&config.config_path).is_file()
}

fn required_node_config(
    record: &mut LocalDevnetRecord,
    kind: NodeKind,
) -> Result<&mut LocalNodeConfigRecord> {
    node_config_mut(record, kind)
        .with_context(|| format!("{} config is not available", adapter_for(kind).label()))
}

fn clear_module_context(config: &mut LocalNodeConfigRecord) {
    config.installed = false;
    config.package_path = None;
    config.package_version = None;
    config.package_root_hash = None;
    config.indexer_state = None;
    config.indexer_head = None;
    config.indexer_error = None;
    config.module_path = None;
    config.process_id = None;
    config.lifecycle_state = NodeLifecycleState::NotInitialized;
    config.pending_lifecycle_action = None;
}

fn lifecycle_dispatch_detail(value: &Value) -> String {
    let result = operation_detail_from_value(value);
    format!("{result}; waiting for Inspector endpoint confirmation")
}

fn command_display(command: &std::process::Command) -> String {
    let mut parts = vec![command.get_program().to_string_lossy().into_owned()];
    parts.extend(
        command
            .get_args()
            .map(|argument| argument.to_string_lossy().into_owned()),
    );
    parts.join(" ")
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
) -> LocalNodeActionResult {
    let timestamp = now_millis();
    match operation() {
        Ok(outcome) => LocalNodeActionResult {
            report: LocalNodeOperationReport {
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
            interruption: None,
        },
        Err(error) => interrupted_operation(request, node, error),
    }
}

fn interrupted_operation(
    request: &LocalNodeActionRequest,
    node: Option<NodeKind>,
    error: anyhow::Error,
) -> LocalNodeActionResult {
    let timestamp = now_millis();
    let detail = error.to_string();
    let interruption = is_control_interruption(&error).then_some(error);
    LocalNodeActionResult {
        report: LocalNodeOperationReport {
            id: format!("op-{timestamp}"),
            time: timestamp.to_string(),
            timestamp_millis: timestamp,
            action: request.action,
            node,
            network_id: request.network_id.clone(),
            status: "failed".to_owned(),
            detail,
            command: None,
        },
        interruption,
    }
}

fn is_control_interruption(error: &anyhow::Error) -> bool {
    error.downcast_ref::<CommandTerminated>().is_some()
}

fn operation_error_detail(error: &anyhow::Error) -> String {
    format!("{error:#}")
}

fn required_node(request: &LocalNodeActionRequest) -> Result<NodeKind> {
    request.node.context("node kind is required")
}

fn target_network_id(state: &LocalNodesState, request: &LocalNodeActionRequest) -> Result<String> {
    request
        .network_id
        .clone()
        .or_else(|| state.active_devnet.clone())
        .context("local devnet id is required")
}

fn default_node_config(workspace: &Path, kind: NodeKind) -> LocalNodeConfigRecord {
    let adapter = adapter_for(kind);
    let port = adapter.default_port();
    let config_name = if kind == NodeKind::Bedrock {
        "bedrock.yaml".to_owned()
    } else {
        format!("{}.json", kind.as_str())
    };
    LocalNodeConfigRecord {
        kind,
        config_path: workspace
            .join("configs")
            .join(config_name)
            .display()
            .to_string(),
        initialization_config_path: (kind == NodeKind::Bedrock).then(|| {
            workspace
                .join("configs/bedrock.init.json")
                .display()
                .to_string()
        }),
        data_dir: workspace
            .join("data")
            .join(kind.as_str())
            .display()
            .to_string(),
        endpoint: adapter.endpoint(port),
        port,
        package_path: None,
        package_version: None,
        package_root_hash: None,
        indexer_state: None,
        indexer_head: None,
        indexer_error: None,
        module_path: None,
        process_id: None,
        installed: false,
        lifecycle_state: NodeLifecycleState::NotInitialized,
        pending_lifecycle_action: None,
    }
}

fn generate_devnet_files(record: &LocalDevnetRecord) -> Result<()> {
    generate_topology_files(record, true)
}

fn generate_topology_files(record: &LocalDevnetRecord, overwrite: bool) -> Result<()> {
    for node in &record.nodes {
        fs::create_dir_all(&node.data_dir)
            .with_context(|| format!("failed to create {}", node.data_dir))?;
        let write_path = node
            .initialization_config_path
            .as_deref()
            .unwrap_or(&node.config_path);
        let config_path = PathBuf::from(write_path);
        if let Some(parent) = config_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        if !overwrite && config_path.is_file() {
            continue;
        }
        let value = generated_node_config(record, node);
        let text = serde_json::to_string_pretty(&value)
            .context("failed to serialize local node config")?;
        fs::write(&config_path, text)
            .with_context(|| format!("failed to write {}", config_path.display()))?;
    }
    Ok(())
}

fn generated_node_config(record: &LocalDevnetRecord, node: &LocalNodeConfigRecord) -> Value {
    adapter_for(node.kind).build_config(NodeConfigContext {
        network_id: &record.id,
        config_path: &node.config_path,
        data_dir: &node.data_dir,
        endpoint: node.endpoint.as_deref(),
        port: node.port,
        public_testnet: record.deployment == LocalNodeDeployment::PublicTestnet,
    })
}

pub(super) fn ensure_testnet_topology(state: &mut LocalNodesState) -> Result<bool> {
    if let Some(record) = state.testnet.as_ref() {
        generate_topology_files(record, false)?;
        return Ok(false);
    }

    let workspace = PathBuf::from(&state.managed_workspace_root).join(TESTNET_ID);
    fs::create_dir_all(&workspace)
        .with_context(|| format!("failed to create workspace {}", workspace.display()))?;
    let now = now_millis();
    let record = LocalDevnetRecord {
        deployment: LocalNodeDeployment::PublicTestnet,
        id: TESTNET_ID.to_owned(),
        label: "Logos Testnet".to_owned(),
        workspace: workspace.display().to_string(),
        manifest_path: workspace.join(MANIFEST_FILE).display().to_string(),
        created_at: now,
        updated_at: now,
        nodes: node_set_for_profile("default")
            .into_iter()
            .map(|kind| default_node_config(&workspace, kind))
            .collect(),
    };
    generate_topology_files(&record, false)?;
    write_devnet_manifest(&record)?;
    state.testnet = Some(record);
    state.version = super::model::LOCAL_NODES_STATE_VERSION;
    Ok(true)
}

pub(super) fn write_devnet_manifest(record: &LocalDevnetRecord) -> Result<()> {
    let path = PathBuf::from(&record.manifest_path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }
    let text = serde_json::to_string_pretty(record)
        .context("failed to serialize local devnet manifest")?;
    fs::write(&path, text).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(())
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
    let _ignored = request_owned_process_stop(node);
    node.process_id = None;
}

fn request_owned_process_stop(node: &LocalNodeConfigRecord) -> Result<()> {
    let Some(pid) = node.process_id else {
        return Ok(());
    };
    if process_group_has_live_members(pid) {
        stop_process(pid)
            .with_context(|| format!("failed to stop Inspector-owned process group {pid}"))?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::{
        fs,
        os::unix::{fs::PermissionsExt as _, process::CommandExt as _},
        process::{self, Command},
        thread,
        time::{Duration, Instant},
    };

    use anyhow::{Context as _, Result, bail};

    #[cfg(unix)]
    use super::super::process::{process_group_is_alive, process_is_alive};
    use super::*;

    #[cfg(unix)]
    struct ProcessGroupGuard {
        process_id: u32,
    }

    #[cfg(unix)]
    impl Drop for ProcessGroupGuard {
        fn drop(&mut self) {
            if process_group_is_alive(self.process_id) {
                // INTENTIONAL: test cleanup must not mask the assertion that detects an orphan.
                let _ignored = stop_process(self.process_id);
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn runtime_stop_reaps_module_hosts_after_daemon_exit() -> Result<()> {
        runtime_stop_reaps_module_hosts(false)
    }

    #[test]
    fn operation_error_detail_keeps_nested_cause() -> Result<()> {
        let error = anyhow::anyhow!("inner CLI failure").context("outer operation failure");
        let detail = operation_error_detail(&error);
        if !detail.contains("outer operation failure") || !detail.contains("inner CLI failure") {
            bail!("operation error detail lost a cause: {detail}");
        }
        Ok(())
    }

    #[test]
    fn loading_legacy_manifest_cannot_restore_process_era_indexer() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let workspace = directory.path().join("legacy-network");
        let manifest_path = workspace.join(MANIFEST_FILE);
        let mut indexer = default_node_config(&workspace, NodeKind::Indexer);
        indexer.endpoint = Some("http://127.0.0.1:8779/".to_owned());
        indexer.port = Some(8779);
        indexer.package_path = Some("/usr/local/bin/indexer_service".to_owned());
        indexer.process_id = Some(4242);
        indexer.installed = true;
        indexer.lifecycle_state = NodeLifecycleState::Running;
        let record = LocalDevnetRecord {
            deployment: LocalNodeDeployment::LocalDevnet,
            id: "legacy-network".to_owned(),
            label: "Legacy network".to_owned(),
            workspace: workspace.display().to_string(),
            manifest_path: manifest_path.display().to_string(),
            created_at: 1,
            updated_at: 2,
            nodes: vec![indexer],
        };
        write_devnet_manifest(&record)?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        let request = LocalNodeActionRequest {
            action: NodeAction::LoadNetwork,
            node: None,
            network_id: None,
            workspace_path: Some(workspace.display().to_string()),
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };

        let result = load_network(&mut state, None, &request);

        anyhow::ensure!(result.report.status == "loaded");
        let loaded = state
            .devnets
            .first()
            .and_then(|record| record.nodes.first())
            .context("missing loaded Indexer")?;
        anyhow::ensure!(
            loaded.endpoint.is_none()
                && loaded.port.is_none()
                && loaded.process_id.is_none()
                && loaded.package_path.is_none()
                && !loaded.installed
                && loaded.lifecycle_state == NodeLifecycleState::NotInitialized
        );
        let persisted: LocalDevnetRecord =
            serde_json::from_str(&std::fs::read_to_string(&manifest_path)?)?;
        let persisted_indexer = persisted
            .nodes
            .first()
            .context("missing persisted Indexer")?;
        anyhow::ensure!(
            persisted_indexer.endpoint.is_none()
                && persisted_indexer.port.is_none()
                && persisted_indexer.process_id.is_none()
                && persisted_indexer.package_path.is_none()
                && !persisted_indexer.installed
        );
        Ok(())
    }

    #[test]
    fn indexer_start_config_uses_selected_channel_and_bedrock_origin() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let indexer = state
            .testnet
            .as_mut()
            .and_then(|record| node_config_mut(record, NodeKind::Indexer))
            .context("missing Indexer node")?;
        indexer.installed = true;
        indexer.lifecycle_state = NodeLifecycleState::Stopped;
        let config_path = indexer.config_path.clone();
        let channel_a = "01".repeat(32);
        let channel_b = "02".repeat(32);
        let request = LocalNodeActionRequest {
            action: NodeAction::Start,
            node: Some(NodeKind::Indexer),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: Some(channel_a.clone()),
            bedrock_endpoint: Some("http://127.0.0.1:8080/".to_owned()),
            label: None,
        };

        anyhow::ensure!(prepare_indexer_start_config(&mut state, "default", &request)?.is_none());
        let config: Value = serde_json::from_str(&fs::read_to_string(&config_path)?)?;
        anyhow::ensure!(
            config.get("channel_id").and_then(Value::as_str) == Some(channel_a.as_str())
                && config
                    .pointer("/bedrock_config/addr")
                    .and_then(Value::as_str)
                    == Some("http://127.0.0.1:8080")
                && config
                    .get("consensus_info_polling_interval")
                    .and_then(Value::as_str)
                    == Some("1s"),
            "unexpected managed Indexer config: {config}"
        );

        let indexer = state
            .testnet
            .as_mut()
            .and_then(|record| node_config_mut(record, NodeKind::Indexer))
            .context("missing Indexer node")?;
        indexer.lifecycle_state = NodeLifecycleState::Running;
        let other_channel = LocalNodeActionRequest {
            channel_id: Some(channel_b.clone()),
            ..request
        };
        let problem = prepare_indexer_start_config(&mut state, "default", &other_channel)?
            .context("active Indexer Channel switch was not blocked")?;
        anyhow::ensure!(
            problem.contains(&format!("stop it before starting Channel `{channel_b}`"))
                && indexer_channel_from_config(&config_path)?.as_deref()
                    == Some(channel_a.as_str()),
            "unexpected Channel switch result: {problem}"
        );
        Ok(())
    }

    #[test]
    fn indexer_start_preflight_does_not_mutate_non_stopped_context() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let channel = "01".repeat(32);
        let config_path = state
            .testnet
            .as_mut()
            .and_then(|record| node_config_mut(record, NodeKind::Indexer))
            .map(|indexer| {
                indexer.installed = true;
                indexer.config_path.clone()
            })
            .context("missing Indexer node")?;
        let config = crate::source_routing::execution_zone_layer::managed_indexer_channel_config(
            &channel,
            "http://127.0.0.1:8080",
        );
        fs::write(&config_path, serde_json::to_string_pretty(&config)?)?;
        let request = LocalNodeActionRequest {
            action: NodeAction::Start,
            node: Some(NodeKind::Indexer),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: Some(channel),
            bedrock_endpoint: Some("http://127.0.0.1:18080".to_owned()),
            label: None,
        };

        for (lifecycle_state, pending_lifecycle_action) in [
            (NodeLifecycleState::Running, None),
            (NodeLifecycleState::Starting, Some(NodeAction::Start)),
        ] {
            let record = state.testnet.as_mut().context("missing testnet")?;
            let indexer = node_config_mut(record, NodeKind::Indexer).context("missing Indexer")?;
            indexer.lifecycle_state = lifecycle_state;
            indexer.pending_lifecycle_action = pending_lifecycle_action;
            indexer.indexer_state = Some("caught_up".to_owned());
            indexer.indexer_head = Some("42".to_owned());
            let before_node = indexer.clone();
            let before_updated_at = record.updated_at;
            let before_config = fs::read(&config_path)?;

            let problem = prepare_indexer_start_config(&mut state, "default", &request)?
                .context("non-stopped Indexer start was accepted")?;

            let record = state.testnet.as_ref().context("missing testnet")?;
            let indexer = record
                .nodes
                .iter()
                .find(|node| node.kind == NodeKind::Indexer)
                .context("missing Indexer")?;
            anyhow::ensure!(problem.contains("must be stopped"));
            anyhow::ensure!(indexer == &before_node);
            anyhow::ensure!(record.updated_at == before_updated_at);
            anyhow::ensure!(fs::read(&config_path)? == before_config);
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn stopped_indexer_context_rebinds_when_another_topology_starts() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"lez_indexer_module","status":"loaded"}]}'
        ;;
    module-info)
        printf '%s\n' '{"name":"lez_indexer_module","methods":[{"isInvokable":true,"name":"start_indexer","signature":"start_indexer(QString)"}]}'
        ;;
    call)
        printf '%s\n' "$3" >> "$config_dir/calls"
        printf '%s\n' '{"status":"ok","module":"lez_indexer_module","method":"start_indexer","result":0}'
        ;;
esac
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;
        let modules_dir = directory.path().join("modules");
        fs::create_dir_all(&modules_dir)?;
        let mut runtime = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            cli.to_str(),
            modules_dir.to_str(),
        )?;
        fs::create_dir_all(&runtime.config_dir)?;
        runtime.daemon_process_id = Some(process::id());
        let mut runtime = Some(runtime);

        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let create = LocalNodeActionRequest {
            action: NodeAction::NewNetwork,
            node: None,
            network_id: Some("second-zone".to_owned()),
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };
        anyhow::ensure!(new_network(&mut state, None, &create).report.status == "created");
        let bound = state.testnet.as_mut().context("missing testnet")?;
        let bound_indexer =
            node_config_mut(bound, NodeKind::Indexer).context("missing bound Indexer")?;
        bound_indexer.installed = true;
        bound_indexer.lifecycle_state = NodeLifecycleState::Stopped;
        state
            .module_context_topology_by_kind
            .insert(NodeKind::Indexer, TESTNET_ID.to_owned());
        let active = state.active_devnet_mut().context("missing active devnet")?;
        let active_id = active.id.clone();
        let active_indexer =
            node_config_mut(active, NodeKind::Indexer).context("missing active Indexer")?;
        active_indexer.installed = true;
        active_indexer.lifecycle_state = NodeLifecycleState::Stopped;
        let request = LocalNodeActionRequest {
            action: NodeAction::Start,
            node: Some(NodeKind::Indexer),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: Some("01".repeat(32)),
            bedrock_endpoint: Some("http://127.0.0.1:8080".to_owned()),
            label: None,
        };

        let result = LocalNodeActionWorkspace::system().apply(
            &mut state,
            &mut runtime,
            directory.path(),
            "local",
            &request,
            None,
        );

        anyhow::ensure!(
            result.report.status == "starting",
            "{}",
            result.report.detail
        );
        anyhow::ensure!(
            state.module_context_topology_id(NodeKind::Indexer) == Some(active_id.as_str())
        );
        let active_indexer = state
            .active_devnet()
            .and_then(|record| {
                record
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Indexer)
            })
            .context("missing started Indexer")?;
        anyhow::ensure!(
            active_indexer.lifecycle_state == NodeLifecycleState::Starting
                && active_indexer.pending_lifecycle_action == Some(NodeAction::Start)
        );
        let calls = fs::read_to_string(
            Path::new(&runtime.as_ref().context("runtime disappeared")?.config_dir).join("calls"),
        )?;
        anyhow::ensure!(calls.lines().eq(["start_indexer"]));
        Ok(())
    }

    #[test]
    fn running_indexer_foreign_binding_requires_stopping_bound_channel() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let create = LocalNodeActionRequest {
            action: NodeAction::NewNetwork,
            node: None,
            network_id: Some("second-zone".to_owned()),
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };
        anyhow::ensure!(new_network(&mut state, None, &create).report.status == "created");
        let bound = state.testnet.as_mut().context("missing testnet")?;
        let bound_indexer =
            node_config_mut(bound, NodeKind::Indexer).context("missing bound Indexer")?;
        bound_indexer.installed = true;
        bound_indexer.lifecycle_state = NodeLifecycleState::Running;
        state
            .module_context_topology_by_kind
            .insert(NodeKind::Indexer, TESTNET_ID.to_owned());
        let active_id = state
            .active_devnet()
            .map(|record| record.id.clone())
            .context("missing active devnet")?;

        let problem = managed_start_context_binding_problem(&state, "local", NodeKind::Indexer)
            .context("foreign running Indexer binding was accepted")?;

        anyhow::ensure!(
            problem
                == format!(
                    "Indexer module context is bound to `{TESTNET_ID}`; select that topology and stop its Channel Indexer before controlling `{active_id}`"
                ),
            "unexpected foreign Indexer binding guidance: {problem}"
        );
        anyhow::ensure!(
            !problem.contains("initialize"),
            "Indexer guidance exposed an unavailable Initialize action: {problem}"
        );
        Ok(())
    }

    #[test]
    fn indexer_channel_id_requires_exact_32_byte_hex_serialization() -> Result<()> {
        validate_channel_id(&"ab".repeat(32))?;
        validate_channel_id(&"AB".repeat(32))?;

        for invalid in [
            String::new(),
            "ab".repeat(31),
            format!("{}a", "ab".repeat(32)),
            format!("{}g", "ab".repeat(31)) + "0",
        ] {
            let Err(error) = validate_channel_id(&invalid) else {
                anyhow::bail!("invalid Channel ID was accepted: {invalid}");
            };
            anyhow::ensure!(
                error
                    .to_string()
                    .contains("exactly 64 hexadecimal characters")
            );
        }
        Ok(())
    }

    #[test]
    fn indexer_bedrock_endpoint_requires_http_origin() -> Result<()> {
        anyhow::ensure!(
            normalized_bedrock_endpoint("http://127.0.0.1:8080/")? == "http://127.0.0.1:8080"
        );
        for endpoint in [
            "http://user@127.0.0.1:8080",
            "http://127.0.0.1:8080?token=secret",
            "http://127.0.0.1:8080/api",
            "file:///tmp/bedrock.sock",
        ] {
            anyhow::ensure!(
                normalized_bedrock_endpoint(endpoint).is_err(),
                "unsafe Bedrock endpoint was accepted: {endpoint}"
            );
        }
        Ok(())
    }

    #[test]
    fn messaging_context_probe_requires_a_successful_nonempty_peer_id() {
        let available = serde_json::json!({
            "status": "ok",
            "result": {"success": true, "value": "peer-test"}
        });
        let absent = serde_json::json!({
            "status": "ok",
            "result": {"success": false, "error": "Context not initialized", "value": null}
        });
        let malformed = serde_json::json!({
            "result": {"success": true, "value": "peer-test"}
        });

        assert_eq!(
            messaging_context_probe_from_response(&available),
            MessagingContextProbe::Available
        );
        assert_eq!(
            messaging_context_probe_from_response(&absent),
            MessagingContextProbe::Absent
        );
        assert_eq!(
            messaging_context_probe_from_response(&malformed),
            MessagingContextProbe::Unknown
        );
    }

    #[test]
    fn module_unload_confirmation_accepts_array_and_wrapped_module_rows() {
        let array = serde_json::json!([
            {"name": "delivery_module", "status": "not_loaded"}
        ]);
        let wrapped = serde_json::json!({
            "modules": [
                {"name": "delivery_module", "status": "not_loaded"}
            ]
        });
        let still_loaded = serde_json::json!({
            "modules": [
                {"name": "delivery_module", "status": "loaded"}
            ]
        });

        assert!(module_is_unloaded(&array, "delivery_module"));
        assert!(module_is_unloaded(&wrapped, "delivery_module"));
        assert!(!module_is_unloaded(&still_loaded, "delivery_module"));
    }

    #[cfg(unix)]
    #[test]
    fn messaging_stop_unloads_delivery_context_and_clears_binding() -> Result<()> {
        // Arrange
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    unload-module)
        printf '%s\n' unload-module >> "$config_dir/calls"
        printf '%s\n' '{"module":"delivery_module","status":"ok"}'
        ;;
    list-modules)
        printf '%s\n' list-modules >> "$config_dir/calls"
        printf '%s\n' '{"modules":[{"name":"delivery_module","status":"not_loaded"}]}'
        ;;
    *)
        printf '%s\n' "unexpected command: $1" >&2
        exit 1
        ;;
esac
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let port = {
            let listener = std::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
            listener.local_addr()?.port()
        };
        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        fs::create_dir_all(&profile.config_dir)?;
        profile.daemon_process_id = Some(process::id());

        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let messaging = state
            .testnet
            .as_mut()
            .and_then(|record| node_config_mut(record, NodeKind::Messaging))
            .context("missing Messaging node")?;
        messaging.installed = true;
        messaging.package_path = Some("delivery_module".to_owned());
        messaging.module_path = Some("delivery_module".to_owned());
        messaging.port = Some(port);
        messaging.lifecycle_state = NodeLifecycleState::Running;
        state.set_module_context_topology_for_profile(NodeKind::Messaging, "default");
        let request = LocalNodeActionRequest {
            action: NodeAction::Stop,
            node: Some(NodeKind::Messaging),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };
        let mut runtime = Some(profile);

        // Act
        let result = LocalNodeActionWorkspace::system().apply(
            &mut state,
            &mut runtime,
            directory.path(),
            "default",
            &request,
            None,
        );

        // Assert
        anyhow::ensure!(
            result.report.status == "stopped",
            "Messaging stop did not complete: {}",
            result.report.detail
        );
        anyhow::ensure!(
            result
                .report
                .detail
                .contains("unloaded Delivery and cleared its Messaging context"),
            "Messaging stop did not disclose context teardown: {}",
            result.report.detail
        );
        anyhow::ensure!(
            result.report.command.as_deref()
                == Some("logoscore unload-module delivery_module --json"),
            "Messaging stop did not report the unload command: {:?}",
            result.report.command
        );
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        anyhow::ensure!(
            !messaging.installed
                && messaging.package_path.is_none()
                && messaging.module_path.is_none()
                && messaging.process_id.is_none()
                && messaging.lifecycle_state == NodeLifecycleState::NotInitialized
                && messaging.pending_lifecycle_action.is_none(),
            "Messaging stop retained its module context: {messaging:?}"
        );
        anyhow::ensure!(
            state
                .module_context_topology_id(NodeKind::Messaging)
                .is_none(),
            "Messaging stop retained its topology binding"
        );
        let calls = fs::read_to_string(
            Path::new(
                &runtime
                    .as_ref()
                    .context("Messaging stop removed the managed runtime")?
                    .config_dir,
            )
            .join("calls"),
        )?;
        anyhow::ensure!(
            calls.lines().eq(["unload-module", "list-modules"]),
            "Messaging stop used unexpected CLI calls: {calls:?}"
        );
        Ok(())
    }

    #[test]
    fn detached_lifecycle_actions_wait_for_watcher_confirmation() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        let create = LocalNodeActionRequest {
            action: NodeAction::NewNetwork,
            node: None,
            network_id: Some("detached-lifecycle".to_owned()),
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };
        let created = new_network(&mut state, None, &create);
        anyhow::ensure!(created.report.status == "created");
        let sequencer = state
            .active_devnet_mut()
            .and_then(|record| node_config_mut(record, NodeKind::Sequencer))
            .context("missing Sequencer node")?;
        sequencer.installed = true;
        sequencer.process_id = Some(u32::MAX);
        sequencer.lifecycle_state = NodeLifecycleState::Running;

        let stop = LocalNodeActionRequest {
            action: NodeAction::Stop,
            node: Some(NodeKind::Sequencer),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };
        let stopped = node_stop(&mut state, None, "local", &stop, None);

        anyhow::ensure!(stopped.report.status == "stopping");
        let sequencer = state
            .active_devnet()
            .and_then(|record| {
                record
                    .nodes
                    .iter()
                    .find(|node| node.kind == NodeKind::Sequencer)
            })
            .context("missing Sequencer node after stop")?;
        anyhow::ensure!(
            sequencer.lifecycle_state == NodeLifecycleState::Stopping
                && sequencer.pending_lifecycle_action == Some(NodeAction::Stop)
                && sequencer.process_id == Some(u32::MAX)
        );

        let repeated_stop = node_stop(&mut state, None, "local", &stop, None);
        anyhow::ensure!(repeated_stop.report.status == "needs_configuration");

        let start = LocalNodeActionRequest {
            action: NodeAction::Start,
            ..stop
        };
        let repeated_start = node_start(&mut state, None, "local", &start, None);
        anyhow::ensure!(repeated_start.report.status == "needs_configuration");
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn runtime_stop_reaps_module_hosts_after_daemon_already_exited() -> Result<()> {
        runtime_stop_reaps_module_hosts(true)
    }

    #[cfg(unix)]
    #[test]
    fn runtime_start_reaps_module_hosts_after_daemon_already_exited() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
while [ "$#" -gt 0 ]; do
    case "$1" in
        --config-dir|--persistence-path|--modules-dir)
            shift 2
            ;;
        *)
            break
            ;;
    esac
done
case "$1" in
    daemon)
        trap 'exit 0' TERM INT
        while :; do sleep 1; done
        ;;
    status)
        printf '%s\n' '{"daemon":{"status":"running"}}'
        ;;
esac
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let child_path = directory.path().join("module-host.pid");
        let mut daemon_command = Command::new("/bin/sh");
        daemon_command
            .args([
                "-c",
                "sleep 30 & printf '%s' \"$!\" > \"$1\"; wait",
                "sh",
                child_path
                    .to_str()
                    .context("test child path is not valid UTF-8")?,
            ])
            .process_group(0);
        let mut daemon = daemon_command.spawn()?;
        let daemon_process_id = daemon.id();
        let _old_cleanup = ProcessGroupGuard {
            process_id: daemon_process_id,
        };
        let child_process_id = wait_for_process_id(&child_path)?;
        let status = Command::new("kill")
            .arg("-TERM")
            .arg(daemon_process_id.to_string())
            .status()
            .context("failed to terminate test daemon without its module host")?;
        anyhow::ensure!(
            status.success(),
            "test daemon termination exited with {status}"
        );
        anyhow::ensure!(
            wait_until_stopped(daemon_process_id),
            "test daemon {daemon_process_id} did not stop"
        );
        anyhow::ensure!(
            process_is_alive(child_process_id),
            "test module host {child_process_id} stopped with daemon"
        );

        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        profile.daemon_process_id = Some(daemon_process_id);
        let mut runtime = Some(profile);
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        let request = LocalNodeActionRequest {
            action: NodeAction::StartRuntime,
            node: None,
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };

        let result = runtime_start(&mut state, &mut runtime, directory.path(), &request, None);

        anyhow::ensure!(
            result.report.status == "started",
            "runtime start did not complete: {}",
            result.report.detail
        );
        anyhow::ensure!(
            wait_until_stopped(child_process_id),
            "runtime start left module host {child_process_id} running"
        );
        let replacement_process_id = runtime
            .as_ref()
            .and_then(|profile| profile.daemon_process_id)
            .context("runtime start did not record a replacement daemon")?;
        let _replacement_cleanup = ProcessGroupGuard {
            process_id: replacement_process_id,
        };
        anyhow::ensure!(
            replacement_process_id != daemon_process_id && process_is_alive(replacement_process_id),
            "runtime start did not replace the exited daemon"
        );
        let _status = daemon.wait()?;
        Ok(())
    }

    #[cfg(unix)]
    fn runtime_stop_reaps_module_hosts(daemon_already_exited: bool) -> Result<()> {
        // Arrange
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
if [ "$1" = "stop" ]; then
    kill -TERM "$(cat "$config_dir/daemon.pid")"
    printf '%s\n' '{"status":"stopped"}'
fi
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let child_path = directory.path().join("module-host.pid");
        let mut daemon_command = Command::new("/bin/sh");
        daemon_command
            .args([
                "-c",
                "sleep 30 & printf '%s' \"$!\" > \"$1\"; wait",
                "sh",
                child_path
                    .to_str()
                    .context("test child path is not valid UTF-8")?,
            ])
            .process_group(0);
        let mut daemon = daemon_command.spawn()?;
        let daemon_process_id = daemon.id();
        let _cleanup = ProcessGroupGuard {
            process_id: daemon_process_id,
        };
        let child_process_id = wait_for_process_id(&child_path)?;

        if daemon_already_exited {
            let status = Command::new("kill")
                .arg("-TERM")
                .arg(daemon_process_id.to_string())
                .status()
                .context("failed to terminate test daemon without its module host")?;
            if !status.success() {
                bail!("test daemon termination exited with {status}");
            }
            if !wait_until_stopped(daemon_process_id) {
                bail!("test daemon {daemon_process_id} did not stop");
            }
            if !process_is_alive(child_process_id) {
                bail!("test module host {child_process_id} stopped with daemon");
            }
        }

        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        fs::create_dir_all(&profile.config_dir)?;
        fs::write(
            Path::new(&profile.config_dir).join("daemon.pid"),
            daemon_process_id.to_string(),
        )?;
        profile.daemon_process_id = Some(daemon_process_id);
        let mut runtime = Some(profile);
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        let request = LocalNodeActionRequest {
            action: NodeAction::StopRuntime,
            node: None,
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };

        // Act
        let result = runtime_stop(&mut state, &mut runtime, &request, None);

        // Assert
        if result.report.status != "stopped" {
            bail!("runtime stop did not complete: {}", result.report.detail);
        }
        if runtime
            .as_ref()
            .and_then(|profile| profile.daemon_process_id)
            .is_some()
        {
            bail!("runtime stop retained the daemon process id");
        }
        if !wait_until_stopped(child_process_id) {
            bail!("runtime stop left module host {child_process_id} running");
        }
        let _status = daemon.wait()?;
        Ok(())
    }

    #[cfg(unix)]
    fn wait_for_process_id(path: &Path) -> Result<u32> {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if let Ok(value) = fs::read_to_string(path)
                && let Ok(process_id) = value.trim().parse::<u32>()
            {
                return Ok(process_id);
            }
            thread::sleep(Duration::from_millis(10));
        }
        bail!("test module host did not publish its process id")
    }

    #[cfg(unix)]
    fn wait_until_stopped(process_id: u32) -> bool {
        let deadline = Instant::now() + Duration::from_secs(1);
        while Instant::now() < deadline {
            if !process_is_alive(process_id) {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_recovers_context_after_lost_create_reply() -> Result<()> {
        // Arrange
        let (result, state, calls) = messaging_initialize_test_case("ready")?;

        // Act & Assert
        if result.report.status != "initialized" {
            bail!(
                "Messaging initialization did not recover: {}",
                result.report.detail
            );
        }
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        if !messaging.installed || messaging.lifecycle_state != NodeLifecycleState::Stopped {
            bail!("Messaging context was not persisted after recovery: {messaging:?}");
        }
        assert_single_messaging_create(&calls)?;
        if !result
            .report
            .detail
            .contains("createNode response was lost")
        {
            bail!(
                "Messaging recovery detail did not explain reply loss: {}",
                result.report.detail
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_preserves_failure_when_context_is_absent() -> Result<()> {
        // Arrange
        let (result, state, calls) = messaging_initialize_test_case("absent")?;

        // Act & Assert
        if result.report.status != "failed" {
            bail!(
                "Messaging initialization unexpectedly recovered: {}",
                result.report.detail
            );
        }
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        if messaging.installed || messaging.lifecycle_state != NodeLifecycleState::NotInitialized {
            bail!("Messaging persisted an absent context: {messaging:?}");
        }
        assert_single_messaging_create(&calls)?;
        if !result.report.detail.contains("RPC_FAILED")
            || !result
                .report
                .detail
                .contains("Messaging context verification failed")
        {
            bail!(
                "Messaging failure lost original or recovery diagnostics: {}",
                result.report.detail
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_rejects_accepted_create_without_context() -> Result<()> {
        let (result, state, calls) = messaging_initialize_test_case("accepted-absent")?;

        anyhow::ensure!(
            result.report.status == "failed",
            "Messaging initialization accepted an unverified context: {}",
            result.report.detail
        );
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        anyhow::ensure!(
            !messaging.installed && messaging.lifecycle_state == NodeLifecycleState::NotInitialized,
            "Messaging persisted a context that getNodeInfo could not verify: {messaging:?}"
        );
        assert_single_messaging_create(&calls)?;
        anyhow::ensure!(
            result
                .report
                .detail
                .contains("response was accepted, but context verification failed"),
            "Messaging initialization did not expose verification failure: {}",
            result.report.detail
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_retries_preflight_before_single_create() -> Result<()> {
        // Arrange
        let (result, state, calls) = messaging_initialize_test_case("preflight-retry")?;

        // Act & Assert
        if result.report.status != "initialized" {
            bail!(
                "Messaging initialization did not recover after preflight retry: {}",
                result.report.detail
            );
        }
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        if !messaging.installed || messaging.lifecycle_state != NodeLifecycleState::Stopped {
            bail!("Messaging context was not persisted after preflight retry: {messaging:?}");
        }
        let module_info_calls = calls
            .iter()
            .filter(|call| call.as_str() == "module-info")
            .count();
        if module_info_calls != 5 {
            bail!(
                "expected four preflight attempts and one context verification, found {module_info_calls}: {calls:?}"
            );
        }
        let create_calls = calls
            .iter()
            .filter(|call| call.as_str() == "createNode")
            .count();
        if create_calls != 1 {
            bail!("Messaging createNode was replayed after preflight retry: {calls:?}");
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_stops_after_one_extra_preflight_attempt() -> Result<()> {
        // Arrange
        let (result, state, calls) = messaging_initialize_test_case("preflight-exhausted")?;

        // Act & Assert
        if result.report.status != "failed" {
            bail!(
                "Messaging initialization unexpectedly recovered: {}",
                result.report.detail
            );
        }
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        if messaging.installed || messaging.lifecycle_state != NodeLifecycleState::NotInitialized {
            bail!("Messaging persisted a failed preflight: {messaging:?}");
        }
        let module_info_calls = calls
            .iter()
            .filter(|call| call.as_str() == "module-info")
            .count();
        if module_info_calls != 4 {
            bail!(
                "expected three initial and one retry preflight attempts, found {module_info_calls}: {calls:?}"
            );
        }
        if calls.iter().any(|call| call == "createNode") {
            bail!("Messaging createNode ran after failed preflight: {calls:?}");
        }
        if !result
            .report
            .detail
            .contains("module initialization preflight retry after")
            || !result.report.detail.contains("RPC_FAILED")
        {
            bail!(
                "Messaging preflight failure lost retry diagnostics: {}",
                result.report.detail
            );
        }
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn messaging_initialize_restarts_crashed_runtime_without_replaying_create() -> Result<()> {
        use std::os::unix::fs::PermissionsExt as _;

        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
config_dir=""
while [ "$#" -gt 0 ]; do
    case "$1" in
        --config-dir)
            config_dir="$2"
            shift 2
            ;;
        --persistence-path|--modules-dir)
            shift 2
            ;;
        *)
            break
            ;;
    esac
done
case "$1" in
    daemon)
        starts_path="$config_dir/daemon-starts"
        starts=0
        if [ -f "$starts_path" ]; then
            starts="$(cat "$starts_path")"
        fi
        starts=$((starts + 1))
        printf '%s' "$starts" > "$starts_path"
        printf '%s' "$$" > "$config_dir/daemon.pid"
        touch "$config_dir/daemon-ready"
        trap 'exit 0' TERM INT
        while :; do sleep 1; done
        ;;
    status)
        printf '%s\n' '{"daemon":{"status":"running"}}'
        ;;
    list-modules)
        printf '%s\n' '{"modules":[{"name":"delivery_module","status":"loaded"}]}'
        ;;
    module-info)
        printf '%s\n' '{"name":"delivery_module","methods":[{"isInvokable":true,"name":"createNode","signature":"createNode(QString)"},{"isInvokable":true,"name":"getNodeInfo","signature":"getNodeInfo(QString)"}]}'
        ;;
    call)
        case "$3" in
            createNode)
                printf '%s\n' createNode >> "$config_dir/calls"
                kill -TERM "$(cat "$config_dir/daemon.pid")"
                printf '%s\n' '{"code":"RPC_FAILED","message":"delivery create response lost","status":"error"}'
                exit 4
                ;;
            getNodeInfo)
                printf '%s\n' getNodeInfo >> "$config_dir/calls"
                if [ "$(cat "$config_dir/daemon-starts")" -ge 2 ]; then
                    printf '%s\n' '{"module":"delivery_module","method":"getNodeInfo","result":{"success":true,"value":"peer-after-restart"},"status":"ok"}'
                else
                    printf '%s\n' '{"code":"CONTEXT_UNAVAILABLE","message":"runtime restart required","status":"error"}'
                    exit 4
                fi
                ;;
        esac
        ;;
esac
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        fs::create_dir_all(&profile.config_dir)?;
        let initial_process_id = spawn_detached(
            profile.daemon_command()?,
            "test Inspector-managed logoscore daemon",
        )?;
        profile.daemon_process_id = Some(initial_process_id);
        let _initial_cleanup = ProcessGroupGuard {
            process_id: initial_process_id,
        };
        let daemon_pid_path = Path::new(&profile.config_dir).join("daemon.pid");
        let daemon_start_deadline = Instant::now() + Duration::from_secs(1);
        while !daemon_pid_path.is_file() {
            if Instant::now() >= daemon_start_deadline {
                bail!("test Inspector-managed logoscore daemon did not start");
            }
            thread::sleep(Duration::from_millis(10));
        }

        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let storage = state
            .testnet
            .as_mut()
            .and_then(|record| {
                record
                    .nodes
                    .iter_mut()
                    .find(|node| node.kind == NodeKind::Storage)
            })
            .context("missing Storage node for runtime recovery test")?;
        storage.installed = true;
        storage.lifecycle_state = NodeLifecycleState::Running;
        storage.pending_lifecycle_action = Some(NodeAction::Start);
        let request = LocalNodeActionRequest {
            action: NodeAction::Initialize,
            node: Some(NodeKind::Messaging),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };
        let control = CommandControl::new(
            tokio_util::sync::CancellationToken::new(),
            Instant::now() + Duration::from_secs(10),
        );
        let mut runtime = Some(profile);

        let result = node_initialize(
            &mut state,
            &mut runtime,
            "default",
            &request,
            Some(&control),
        );

        let profile = runtime
            .as_ref()
            .context("Messaging recovery removed the managed runtime profile")?;
        let recovered_process_id = profile
            .daemon_process_id
            .context("Messaging recovery did not record a replacement daemon")?;
        let _replacement_cleanup = ProcessGroupGuard {
            process_id: recovered_process_id,
        };
        if result.report.status != "initialized" {
            bail!(
                "Messaging initialization did not recover after daemon crash: {}",
                result.report.detail
            );
        }
        anyhow::ensure!(
            recovered_process_id != initial_process_id,
            "Messaging recovery retained the crashed daemon process id"
        );
        anyhow::ensure!(
            fs::read_to_string(Path::new(&profile.config_dir).join("daemon-starts"))?.trim() == "2",
            "Messaging recovery did not start exactly one replacement daemon"
        );
        let calls = fs::read_to_string(Path::new(&profile.config_dir).join("calls"))?;
        assert_single_messaging_create(&calls.lines().map(ToOwned::to_owned).collect::<Vec<_>>())?;
        let messaging = testnet_node(&state, NodeKind::Messaging)?;
        anyhow::ensure!(
            messaging.installed && messaging.lifecycle_state == NodeLifecycleState::Stopped,
            "Messaging context was not persisted after restart recovery: {messaging:?}"
        );
        let storage = testnet_node(&state, NodeKind::Storage)?;
        anyhow::ensure!(
            !storage.installed
                && storage.lifecycle_state == NodeLifecycleState::NotInitialized
                && storage.pending_lifecycle_action.is_none(),
            "runtime recovery retained a stale Storage module context: {storage:?}"
        );
        anyhow::ensure!(
            result
                .report
                .detail
                .contains("restarted the Inspector-managed LogosCore runtime"),
            "Messaging recovery did not disclose runtime restart: {}",
            result.report.detail
        );
        Ok(())
    }

    #[cfg(unix)]
    fn messaging_initialize_test_case(
        recovery_mode: &str,
    ) -> Result<(LocalNodeActionResult, LocalNodesState, Vec<String>)> {
        let directory = tempfile::tempdir()?;
        let cli = directory.path().join("logoscore");
        fs::write(
            &cli,
            r#"#!/bin/sh
if [ "$1" = "--config-dir" ]; then
    config_dir="$2"
    shift 2
fi
case "$1" in
    list-modules)
        printf '%s\n' '{"modules":[{"name":"delivery_module","status":"loaded"}]}'
        ;;
    module-info)
        printf '%s\n' module-info >> "$config_dir/calls"
        recovery_mode="$(cat "$config_dir/recovery-mode")"
        if [ "$recovery_mode" = preflight-retry ] || [ "$recovery_mode" = preflight-exhausted ]; then
            count_path="$config_dir/module-info-count"
            count=0
            if [ -f "$count_path" ]; then
                count="$(cat "$count_path")"
            fi
            count=$((count + 1))
            printf '%s' "$count" > "$count_path"
            if [ "$recovery_mode" = preflight-exhausted ] || [ "$count" -lt 4 ]; then
                printf '%s\n' '{"code":"RPC_FAILED","message":"delivery replica is starting","status":"error"}'
                exit 4
            fi
        fi
        printf '%s\n' '{"name":"delivery_module","methods":[{"isInvokable":true,"name":"createNode","signature":"createNode(QString)"},{"isInvokable":true,"name":"getNodeInfo","signature":"getNodeInfo(QString)"}]}'
        ;;
    call)
        case "$3" in
            createNode)
                printf '%s\n' createNode >> "$config_dir/calls"
                if [ "$(cat "$config_dir/recovery-mode")" = preflight-retry ] || [ "$(cat "$config_dir/recovery-mode")" = accepted-absent ]; then
                    printf '%s\n' '{"module":"delivery_module","method":"createNode","result":{"success":true,"value":"created"},"status":"ok"}'
                    exit 0
                fi
                printf '%s\n' '{"code":"RPC_FAILED","message":"delivery create response lost","status":"error"}'
                exit 4
                ;;
            getNodeInfo)
                printf '%s\n' getNodeInfo >> "$config_dir/calls"
                if [ "$(cat "$config_dir/recovery-mode")" = ready ] || [ "$(cat "$config_dir/recovery-mode")" = preflight-retry ]; then
                    printf '%s\n' '{"module":"delivery_module","method":"getNodeInfo","result":{"success":true,"value":"peer-test"},"status":"ok"}'
                else
                    printf '%s\n' '{"code":"CONTEXT_MISSING","message":"no delivery context","status":"error"}'
                    exit 4
                fi
                ;;
        esac
        ;;
esac
"#,
        )?;
        let mut permissions = fs::metadata(&cli)?.permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&cli, permissions)?;

        let mut runtime = LogoscoreRuntimeProfile::create_or_restart(
            directory.path(),
            None,
            Some(
                cli.to_str()
                    .context("test LogosCore CLI path is not valid UTF-8")?,
            ),
            Some(
                directory
                    .path()
                    .to_str()
                    .context("test modules path is not valid UTF-8")?,
            ),
        )?;
        fs::create_dir_all(&runtime.config_dir)?;
        fs::write(
            Path::new(&runtime.config_dir).join("recovery-mode"),
            recovery_mode,
        )?;
        runtime.daemon_process_id = Some(process::id());

        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        ensure_testnet_topology(&mut state)?;
        let request = LocalNodeActionRequest {
            action: NodeAction::Initialize,
            node: Some(NodeKind::Messaging),
            network_id: None,
            workspace_path: None,
            runtime_modules_dir: None,
            runtime_binary_path: None,
            package_version: None,
            package_root_hash: None,
            channel_id: None,
            bedrock_endpoint: None,
            label: None,
        };

        let deadline = Instant::now()
            .checked_add(Duration::from_secs(30))
            .context("Messaging recovery test deadline overflow")?;
        let control = CommandControl::new(tokio_util::sync::CancellationToken::new(), deadline);
        let mut runtime = Some(runtime);
        let result = node_initialize(
            &mut state,
            &mut runtime,
            "default",
            &request,
            Some(&control),
        );
        let config_dir = runtime
            .as_ref()
            .context("Messaging test runtime disappeared")?
            .config_dir
            .clone();
        let calls_path = Path::new(&config_dir).join("calls");
        let calls = calls_path
            .is_file()
            .then(|| fs::read_to_string(calls_path))
            .transpose()?
            .unwrap_or_default()
            .lines()
            .map(ToOwned::to_owned)
            .collect();
        Ok((result, state, calls))
    }

    #[cfg(unix)]
    #[test]
    fn runtime_modules_dir_validation_uses_canonical_identity() -> Result<()> {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir()?;
        let modules_dir = directory.path().join("modules");
        let modules_alias = directory.path().join("modules-alias");
        let other_modules_dir = directory.path().join("other-modules");
        fs::create_dir_all(&modules_dir)?;
        fs::create_dir_all(&other_modules_dir)?;
        symlink(&modules_dir, &modules_alias)?;
        let runtime = LogoscoreRuntimeProfile {
            id: "test-runtime".to_owned(),
            binary_path: "/usr/bin/logoscore".to_owned(),
            config_dir: directory.path().join("runtime").display().to_string(),
            modules_dir: Some(modules_alias.display().to_string()),
            persistence_path: Some(directory.path().join("data").display().to_string()),
            ownership: super::super::runtime::LogoscoreRuntimeOwnership::InspectorManaged,
            timeout_profile: super::super::runtime::LogoscoreTimeoutProfile::Lifecycle,
            daemon_process_id: None,
        };

        validate_runtime_modules_dir(Some(&runtime), modules_dir.to_string_lossy().as_ref())?;
        anyhow::ensure!(
            validate_runtime_modules_dir(
                Some(&runtime),
                other_modules_dir.to_string_lossy().as_ref()
            )
            .is_err()
        );
        Ok(())
    }

    #[test]
    fn indexer_package_install_reconciles_every_topology_for_canonical_modules_dir() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules_a = directory.path().join("modules-a");
        let modules_b = directory.path().join("modules-b");
        let package_a = create_indexer_package_file(&modules_a)?;
        let package_b = create_indexer_package_file(&modules_b)?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        state.testnet = Some(indexer_topology(
            directory.path(),
            "logos-testnet",
            LocalNodeDeployment::PublicTestnet,
        )?);
        state.devnets = vec![
            indexer_topology(
                directory.path(),
                "same-directory",
                LocalNodeDeployment::LocalDevnet,
            )?,
            indexer_topology(
                directory.path(),
                "other-directory",
                LocalNodeDeployment::LocalDevnet,
            )?,
        ];
        let same_directory = state
            .devnets
            .first_mut()
            .and_then(|record| node_config_mut(record, NodeKind::Indexer))
            .context("missing same-directory Indexer")?;
        set_test_indexer_package(same_directory, &package_a, "0.9.0", "a");
        let other_directory = state
            .devnets
            .get_mut(1)
            .and_then(|record| node_config_mut(record, NodeKind::Indexer))
            .context("missing other-directory Indexer")?;
        set_test_indexer_package(other_directory, &package_b, "2.0.0", "b");
        let installed = test_installed_package(&modules_a, &package_a, "1.0.0", "c");

        record_indexer_package(&mut state, "default", &installed)?;

        let testnet = testnet_node(&state, NodeKind::Indexer)?;
        let same_directory = state
            .devnets
            .first()
            .and_then(|record| record.nodes.first())
            .context("missing reconciled same-directory Indexer")?;
        let other_directory = state
            .devnets
            .get(1)
            .and_then(|record| record.nodes.first())
            .context("missing preserved other-directory Indexer")?;
        anyhow::ensure!(
            testnet.package_version.as_deref() == Some("1.0.0")
                && same_directory.package_version.as_deref() == Some("1.0.0")
                && testnet.package_path.as_deref() == Some(installed.main_file_path.as_str())
                && same_directory.package_path.as_deref()
                    == Some(installed.main_file_path.as_str())
        );
        anyhow::ensure!(
            other_directory.package_version.as_deref() == Some("2.0.0")
                && other_directory.package_path.as_deref() == package_b.to_str()
        );
        let same_manifest: LocalDevnetRecord = serde_json::from_str(&fs::read_to_string(
            &state
                .devnets
                .first()
                .context("missing same topology")?
                .manifest_path,
        )?)?;
        anyhow::ensure!(
            same_manifest
                .nodes
                .first()
                .and_then(|node| node.package_version.as_deref())
                == Some("1.0.0")
        );
        Ok(())
    }

    #[test]
    fn runtime_modules_dir_reconciliation_clears_only_mismatched_indexer_packages() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let modules_a = directory.path().join("modules-a");
        let modules_b = directory.path().join("modules-b");
        let package_a = create_indexer_package_file(&modules_a)?;
        let package_b = create_indexer_package_file(&modules_b)?;
        let mut state = LocalNodesState::default_for_config_dir(directory.path());
        state.testnet = Some(indexer_topology(
            directory.path(),
            "logos-testnet",
            LocalNodeDeployment::PublicTestnet,
        )?);
        state.devnets = vec![indexer_topology(
            directory.path(),
            "matching",
            LocalNodeDeployment::LocalDevnet,
        )?];
        set_test_indexer_package(
            state
                .testnet
                .as_mut()
                .and_then(|record| node_config_mut(record, NodeKind::Indexer))
                .context("missing mismatched Testnet Indexer")?,
            &package_a,
            "1.0.0",
            "a",
        );
        set_test_indexer_package(
            state
                .devnets
                .first_mut()
                .and_then(|record| node_config_mut(record, NodeKind::Indexer))
                .context("missing matching local Indexer")?,
            &package_b,
            "2.0.0",
            "b",
        );
        let runtime = LogoscoreRuntimeProfile {
            id: "test-runtime".to_owned(),
            binary_path: "/usr/bin/logoscore".to_owned(),
            config_dir: directory.path().join("runtime").display().to_string(),
            modules_dir: Some(modules_b.display().to_string()),
            persistence_path: Some(directory.path().join("data").display().to_string()),
            ownership: super::super::runtime::LogoscoreRuntimeOwnership::InspectorManaged,
            timeout_profile: super::super::runtime::LogoscoreTimeoutProfile::Lifecycle,
            daemon_process_id: None,
        };

        reconcile_indexer_runtime_modules_dir(&mut state, &runtime)?;

        let mismatched = testnet_node(&state, NodeKind::Indexer)?;
        let matching = state
            .devnets
            .first()
            .and_then(|record| record.nodes.first())
            .context("missing matching local Indexer")?;
        anyhow::ensure!(
            !mismatched.installed
                && mismatched.package_path.is_none()
                && mismatched.package_version.is_none()
                && mismatched.package_root_hash.is_none()
                && mismatched.lifecycle_state == NodeLifecycleState::NotInitialized
        );
        anyhow::ensure!(
            matching.installed
                && matching.package_path.as_deref() == package_b.to_str()
                && matching.package_version.as_deref() == Some("2.0.0")
        );
        Ok(())
    }

    fn indexer_topology(
        root: &Path,
        id: &str,
        deployment: LocalNodeDeployment,
    ) -> Result<LocalDevnetRecord> {
        let workspace = root.join(id);
        let record = LocalDevnetRecord {
            deployment,
            id: id.to_owned(),
            label: id.to_owned(),
            workspace: workspace.display().to_string(),
            manifest_path: workspace.join(MANIFEST_FILE).display().to_string(),
            created_at: 1,
            updated_at: 1,
            nodes: vec![default_node_config(&workspace, NodeKind::Indexer)],
        };
        generate_topology_files(&record, false)?;
        write_devnet_manifest(&record)?;
        Ok(record)
    }

    fn create_indexer_package_file(modules_dir: &Path) -> Result<PathBuf> {
        let package_dir = modules_dir.join("lez_indexer_module");
        let package_path = package_dir.join("lez_indexer_module_plugin.so");
        fs::create_dir_all(&package_dir)?;
        fs::write(&package_path, b"module")?;
        Ok(package_path)
    }

    fn set_test_indexer_package(
        config: &mut LocalNodeConfigRecord,
        package_path: &Path,
        version: &str,
        root_hash_marker: &str,
    ) {
        config.package_path = Some(package_path.display().to_string());
        config.package_version = Some(version.to_owned());
        config.package_root_hash = Some(root_hash_marker.repeat(64));
        config.module_path = Some("lez_indexer_module".to_owned());
        config.installed = true;
        config.lifecycle_state = NodeLifecycleState::Stopped;
    }

    fn test_installed_package(
        modules_dir: &Path,
        package_path: &Path,
        version: &str,
        root_hash_marker: &str,
    ) -> super::super::package::LocalNodeInstalledPackageReport {
        super::super::package::LocalNodeInstalledPackageReport {
            name: "lez_indexer_module".to_owned(),
            version: version.to_owned(),
            description: "Indexer".to_owned(),
            package_type: "core".to_owned(),
            category: "blockchain".to_owned(),
            author: String::new(),
            install_type: "user".to_owned(),
            install_dir: modules_dir.join("lez_indexer_module").display().to_string(),
            main_file_path: package_path.display().to_string(),
            root_hash: root_hash_marker.repeat(64),
        }
    }

    fn testnet_node(state: &LocalNodesState, kind: NodeKind) -> Result<&LocalNodeConfigRecord> {
        state
            .testnet
            .as_ref()
            .and_then(|record| record.nodes.iter().find(|node| node.kind == kind))
            .with_context(|| format!("missing {kind:?} Testnet node"))
    }

    #[cfg(unix)]
    fn assert_single_messaging_create(calls: &[String]) -> Result<()> {
        let creates = calls
            .iter()
            .filter(|call| call.as_str() == "createNode")
            .count();
        if creates != 1 || !calls.iter().any(|call| call == "getNodeInfo") {
            bail!("unexpected Messaging recovery calls: {calls:?}");
        }
        Ok(())
    }
}
