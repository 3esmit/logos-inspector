use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context as _, Result, bail};
use serde_json::Value;

use crate::support::time::now_millis;

use super::adapters::{NodeActionPolicy, NodeConfigContext, adapter_for};
use super::commands::{
    command_spec_for, ensure_module_loaded, execute_command_spec, operation_detail_from_value,
};
use super::lifecycle::{has_event_contract, reset_module_contexts};
use super::model::{
    LocalDevnetRecord, LocalNodeActionRequest, LocalNodeConfigRecord, LocalNodeOperationReport,
    LocalNodesState, NodeAction, NodeKind, NodeLifecycleState,
};
use super::paths::{path_is_inside, remove_dir_inside};
use super::process::{find_command, process_is_alive, stop_process};
use super::runtime::LogoscoreRuntimeProfile;
use super::workflow::node_set_for_profile;

const MANIFEST_FILE: &str = "local-network.json";
const DEFAULT_DEPLOYMENT: &str = "local";

#[derive(Debug, Clone, Copy, Default)]
pub(super) struct LocalNodeActionWorkspace;

impl LocalNodeActionWorkspace {
    pub(super) fn system() -> Self {
        Self
    }

    pub(super) fn apply(
        self,
        state: &mut LocalNodesState,
        runtime: &mut Option<LogoscoreRuntimeProfile>,
        runtime_config_root: &Path,
        normalized_profile: &str,
        request: &LocalNodeActionRequest,
    ) -> LocalNodeOperationReport {
        dispatch_action(
            state,
            runtime,
            runtime_config_root,
            normalized_profile,
            request,
        )
    }
}

fn dispatch_action(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    runtime_config_root: &Path,
    normalized_profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    match request.action {
        NodeAction::StartRuntime => runtime_start(state, runtime, runtime_config_root, request),
        NodeAction::StopRuntime => runtime_stop(state, runtime, request),
        NodeAction::NewNetwork => new_network(state, runtime.as_ref(), request),
        NodeAction::LoadNetwork => load_network(state, runtime.as_ref(), request),
        NodeAction::DeleteNetwork => delete_network(state, runtime.as_ref(), request),
        NodeAction::ResetNetwork => reset_network(state, runtime.as_ref(), request),
        NodeAction::Install => node_install(state, normalized_profile, request),
        NodeAction::Initialize => {
            node_initialize(state, runtime.as_ref(), normalized_profile, request)
        }
        NodeAction::Uninstall => {
            node_uninstall(state, runtime.as_ref(), normalized_profile, request)
        }
        NodeAction::Start => node_start(state, runtime.as_ref(), normalized_profile, request),
        NodeAction::Stop => node_stop(state, runtime.as_ref(), normalized_profile, request),
        NodeAction::Purge => node_purge(state, normalized_profile, request),
    }
}

fn new_network(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
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
) -> LocalNodeOperationReport {
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
        record.updated_at = now_millis();
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
) -> LocalNodeOperationReport {
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
) -> LocalNodeOperationReport {
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
) -> LocalNodeOperationReport {
    operation_result(request, None, || {
        let mut profile = LogoscoreRuntimeProfile::create_or_restart(
            runtime_config_root,
            runtime.as_ref(),
            request.runtime_binary_path.as_deref(),
            request.runtime_modules_dir.as_deref(),
        )?;
        let command = profile.daemon_command()?;
        let display = command_display(&command);
        let process_id =
            super::process::spawn_detached(command, "Inspector-managed logoscore daemon")?;
        profile.daemon_process_id = Some(process_id);
        reset_module_contexts(state);
        let readiness = profile.wait_until_ready();
        let still_running = profile.is_running();
        *runtime = Some(profile);
        match readiness {
            Ok(()) => Ok(OperationOutcome {
                status: "started".to_owned(),
                detail: "Inspector-managed logoscore daemon is ready".to_owned(),
                command: Some(display),
            }),
            Err(error) if still_running => Ok(OperationOutcome {
                status: "starting".to_owned(),
                detail: error.to_string(),
                command: Some(display),
            }),
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: error.to_string(),
                command: Some(display),
            }),
        }
    })
}

fn runtime_stop(
    state: &mut LocalNodesState,
    runtime: &mut Option<LogoscoreRuntimeProfile>,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
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
            profile.daemon_process_id = None;
            reset_module_contexts(state);
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: "Inspector-managed logoscore daemon is already stopped".to_owned(),
                command: None,
            });
        }
        let cli = profile.cli_runtime()?;
        let value = cli.stop()?.value;
        if profile.wait_until_stopped() {
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

fn node_install(
    state: &mut LocalNodesState,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Install);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
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
        let record = active_devnet_mut(state)?;
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

fn node_initialize(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
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
        let Some(runtime) = managed_runtime(runtime) else {
            return Ok(needs_configuration(
                "start an Inspector-managed logoscore runtime before initializing a module node",
            ));
        };
        let record = active_devnet_mut(state)?;
        let config = required_node_config(record, kind)?;
        let spec = command_spec_for(
            kind,
            NodeAction::Initialize,
            &config.config_path,
            DEFAULT_DEPLOYMENT,
        )
        .with_context(|| format!("{} initialization is not implemented", adapter.label()))?;
        let cli = runtime.cli_runtime()?;
        if ensure_loaded {
            ensure_module_loaded(&spec, Some(&cli))?;
        }
        match execute_command_spec(&spec, Some(&cli)) {
            Ok(value) => {
                config.installed = true;
                config.package_path = Some(spec.program.clone());
                config.lifecycle_state = NodeLifecycleState::Stopped;
                config.pending_lifecycle_action = None;
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "initialized".to_owned(),
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

fn node_uninstall(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Uninstall);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let record = active_devnet_mut(state)?;
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
            DEFAULT_DEPLOYMENT,
        ) else {
            return Ok(needs_configuration(
                "this module has no verified context-destroy contract; stop the managed runtime to clear it",
            ));
        };
        let cli = runtime.cli_runtime()?;
        match execute_command_spec(&spec, Some(&cli)) {
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
            Err(error) => Ok(OperationOutcome {
                status: "failed".to_owned(),
                detail: error.to_string(),
                command: Some(spec.display),
            }),
        }
    })
}

fn node_start(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Start);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let record = active_devnet_mut(state)?;
        let config = required_node_config(record, kind)?;
        fs::create_dir_all(&config.data_dir)
            .with_context(|| format!("failed to create {}", config.data_dir))?;
        let spec = command_spec_for(
            kind,
            NodeAction::Start,
            &config.config_path,
            DEFAULT_DEPLOYMENT,
        )
        .with_context(|| format!("{} start is not implemented", adapter.label()))?;
        if policy == NodeActionPolicy::ExecuteDetached {
            return match execute_command_spec(&spec, None) {
                Ok(value) => {
                    config.process_id = value
                        .get("pid")
                        .and_then(Value::as_u64)
                        .and_then(|pid| u32::try_from(pid).ok());
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
            };
        }
        let NodeActionPolicy::ExecuteManaged {
            ensure_loaded,
            requires_installed_context,
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid start policy",
                adapter.label()
            );
        };
        let Some(runtime) = managed_runtime(runtime) else {
            return Ok(needs_configuration(
                "start an Inspector-managed logoscore runtime before starting a module node",
            ));
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
            ensure_module_loaded(&spec, Some(&cli))?;
        }
        match execute_command_spec(&spec, Some(&cli)) {
            Ok(value) => {
                config.installed = true;
                if has_event_contract(kind, NodeAction::Start) {
                    config.lifecycle_state = NodeLifecycleState::Starting;
                    config.pending_lifecycle_action = Some(NodeAction::Start);
                } else {
                    config.lifecycle_state = NodeLifecycleState::Unknown;
                    config.pending_lifecycle_action = None;
                }
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "starting".to_owned(),
                    detail: lifecycle_dispatch_detail(kind, NodeAction::Start, &value),
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

fn node_stop(
    state: &mut LocalNodesState,
    runtime: Option<&LogoscoreRuntimeProfile>,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
    let node = request.node;
    operation_result(request, node, || {
        let kind = required_node(request)?;
        let adapter = adapter_for(kind);
        let policy = adapter.action_policy(NodeAction::Stop);
        if let Some(reason) = policy.blocked_reason() {
            return Ok(needs_configuration(reason));
        }
        let record = active_devnet_mut(state)?;
        let config = required_node_config(record, kind)?;
        if policy == NodeActionPolicy::ExecuteDetached {
            stop_owned_process(config);
            record.updated_at = now_millis();
            write_devnet_manifest(record)?;
            return Ok(OperationOutcome {
                status: "stopped".to_owned(),
                detail: format!("stopped recorded {} process", adapter.label()),
                command: None,
            });
        }
        let NodeActionPolicy::ExecuteManaged {
            requires_installed_context,
            ..
        } = policy
        else {
            bail!(
                "{} adapter returned an invalid stop policy",
                adapter.label()
            );
        };
        let Some(runtime) = managed_runtime(runtime) else {
            return Ok(needs_configuration(
                "start an Inspector-managed logoscore runtime before stopping a module node",
            ));
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
        let spec = command_spec_for(
            kind,
            NodeAction::Stop,
            &config.config_path,
            DEFAULT_DEPLOYMENT,
        )
        .with_context(|| format!("{} stop is not implemented", adapter.label()))?;
        let cli = runtime.cli_runtime()?;
        match execute_command_spec(&spec, Some(&cli)) {
            Ok(value) => {
                if has_event_contract(kind, NodeAction::Stop) {
                    config.lifecycle_state = NodeLifecycleState::Stopping;
                    config.pending_lifecycle_action = Some(NodeAction::Stop);
                } else {
                    config.lifecycle_state = NodeLifecycleState::Unknown;
                    config.pending_lifecycle_action = None;
                }
                record.updated_at = now_millis();
                write_devnet_manifest(record)?;
                Ok(OperationOutcome {
                    status: "stopping".to_owned(),
                    detail: lifecycle_dispatch_detail(kind, NodeAction::Stop, &value),
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

fn node_purge(
    state: &mut LocalNodesState,
    _profile: &str,
    request: &LocalNodeActionRequest,
) -> LocalNodeOperationReport {
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
        let Some(record) = state.active_devnet_mut() else {
            bail!("active devnet is required");
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

fn needs_configuration(detail: &str) -> OperationOutcome {
    OperationOutcome {
        status: "needs_configuration".to_owned(),
        detail: detail.to_owned(),
        command: None,
    }
}

fn active_devnet_mut(state: &mut LocalNodesState) -> Result<&mut LocalDevnetRecord> {
    state
        .active_devnet_mut()
        .context("active local devnet is required")
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
    config.module_path = None;
    config.process_id = None;
    config.lifecycle_state = NodeLifecycleState::NotInitialized;
    config.pending_lifecycle_action = None;
}

fn lifecycle_dispatch_detail(kind: NodeKind, action: NodeAction, value: &Value) -> String {
    let result = operation_detail_from_value(value);
    if has_event_contract(kind, action) {
        format!("{result}; waiting for module lifecycle event")
    } else {
        format!("{result}; no verified module lifecycle observer")
    }
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
        .context("local devnet id is required")
}

fn default_node_config(workspace: &Path, kind: NodeKind) -> LocalNodeConfigRecord {
    let adapter = adapter_for(kind);
    let port = adapter.default_port();
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
        endpoint: adapter.endpoint(port),
        port,
        package_path: None,
        module_path: None,
        process_id: None,
        installed: false,
        lifecycle_state: NodeLifecycleState::NotInitialized,
        pending_lifecycle_action: None,
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
        let value = generated_node_config(record, node);
        let text = serde_json::to_string_pretty(&value)
            .context("failed to serialize local node config")?;
        fs::write(&node.config_path, text)
            .with_context(|| format!("failed to write {}", node.config_path))?;
    }
    Ok(())
}

fn generated_node_config(record: &LocalDevnetRecord, node: &LocalNodeConfigRecord) -> Value {
    adapter_for(node.kind).build_config(NodeConfigContext {
        network_id: &record.id,
        data_dir: &node.data_dir,
        endpoint: node.endpoint.as_deref(),
        port: node.port,
    })
}

fn write_devnet_manifest(record: &LocalDevnetRecord) -> Result<()> {
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
    let Some(pid) = node.process_id else {
        return;
    };
    if process_is_alive(pid) {
        let _ignored = stop_process(pid);
    }
    node.process_id = None;
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
